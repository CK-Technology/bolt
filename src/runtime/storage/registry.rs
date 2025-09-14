use anyhow::{Result, Context};
use tracing::{info, warn, debug};
use serde::{Deserialize, Serialize};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, ACCEPT, CONTENT_TYPE};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tar::Archive;
use flate2::read::GzDecoder;

#[derive(Debug, Clone)]
pub struct RegistryClient {
    pub base_url: String,
    client: reqwest::Client,
    auth_token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ImageManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub media_type: Option<String>,
    pub config: ConfigDescriptor,
    pub layers: Vec<LayerDescriptor>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConfigDescriptor {
    pub media_type: String,
    pub size: u64,
    pub digest: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LayerDescriptor {
    pub media_type: String,
    pub size: u64,
    pub digest: String,
    pub urls: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    token: String,
}

#[derive(Debug, Deserialize)]
struct DockerHubToken {
    access_token: String,
}

impl RegistryClient {
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("bolt-container-runtime/0.1.0")
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .unwrap();

        Self {
            base_url,
            client,
            auth_token: None,
        }
    }

    pub async fn pull_image(&mut self, image_ref: &str) -> Result<PathBuf> {
        info!("📥 Pulling image from registry: {}", image_ref);

        // Parse image reference
        let (registry, namespace, image, tag) = self.parse_image_reference(image_ref)?;

        // Authenticate with registry
        self.authenticate(&registry, &namespace, &image).await?;

        // Fetch manifest
        let manifest = self.fetch_manifest(&registry, &namespace, &image, &tag).await?;

        // Create local storage directory
        let storage_dir = PathBuf::from("/var/lib/bolt/images");
        fs::create_dir_all(&storage_dir).await
            .context("Failed to create image storage directory")?;

        let image_dir = storage_dir.join(format!("{}-{}-{}", namespace, image, tag));
        fs::create_dir_all(&image_dir).await
            .context("Failed to create image directory")?;

        // Download config
        info!("📄 Downloading image config");
        let config_data = self.fetch_blob(&registry, &namespace, &image, &manifest.config.digest).await?;
        let config_path = image_dir.join("config.json");
        fs::write(&config_path, &config_data).await
            .context("Failed to write config")?;

        // Download and extract layers
        info!("📦 Downloading {} layers", manifest.layers.len());
        for (idx, layer) in manifest.layers.iter().enumerate() {
            info!("  Layer {}/{}: {} ({}MB)",
                idx + 1,
                manifest.layers.len(),
                layer.digest,
                layer.size / 1_000_000
            );

            let layer_data = self.fetch_blob(&registry, &namespace, &image, &layer.digest).await?;

            // Save layer tarball
            let layer_path = image_dir.join(format!("layer_{}.tar.gz", idx));
            fs::write(&layer_path, &layer_data).await
                .context("Failed to write layer")?;

            // Extract layer to rootfs
            let rootfs_dir = image_dir.join("rootfs");
            fs::create_dir_all(&rootfs_dir).await?;

            self.extract_layer(&layer_path, &rootfs_dir).await?;
        }

        info!("✅ Image pulled successfully: {}", image_ref);
        Ok(image_dir)
    }

    async fn authenticate(&mut self, registry: &str, namespace: &str, image: &str) -> Result<()> {
        info!("🔐 Authenticating with registry: {}", registry);

        match registry {
            "docker.io" | "registry-1.docker.io" => {
                // Docker Hub authentication
                let auth_url = format!(
                    "https://auth.docker.io/token?service=registry.docker.io&scope=repository:{}/{}:pull",
                    namespace, image
                );

                let response = self.client
                    .get(&auth_url)
                    .send()
                    .await
                    .context("Failed to authenticate with Docker Hub")?;

                if response.status().is_success() {
                    let auth: AuthResponse = response.json().await
                        .context("Failed to parse auth response")?;
                    self.auth_token = Some(format!("Bearer {}", auth.token));
                    info!("✅ Authenticated with Docker Hub");
                } else {
                    warn!("⚠️  Authentication failed, trying anonymous access");
                }
            }
            _ => {
                // Generic registry - try anonymous first
                info!("Using anonymous access for registry: {}", registry);
            }
        }

        Ok(())
    }

    async fn fetch_manifest(&self, registry: &str, namespace: &str, image: &str, tag: &str) -> Result<ImageManifest> {
        info!("📋 Fetching manifest for {}:{}", image, tag);

        let manifest_url = format!(
            "https://{}/v2/{}/{}/manifests/{}",
            registry, namespace, image, tag
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.docker.distribution.manifest.v2+json")
        );

        if let Some(token) = &self.auth_token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(token).context("Invalid auth token")?
            );
        }

        let response = self.client
            .get(&manifest_url)
            .headers(headers)
            .send()
            .await
            .context("Failed to fetch manifest")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to fetch manifest: HTTP {}",
                response.status()
            ));
        }

        let manifest: ImageManifest = response.json().await
            .context("Failed to parse manifest")?;

        debug!("Manifest: {} layers, config: {}",
            manifest.layers.len(),
            manifest.config.digest
        );

        Ok(manifest)
    }

    async fn fetch_blob(&self, registry: &str, namespace: &str, image: &str, digest: &str) -> Result<Vec<u8>> {
        debug!("⬇️  Fetching blob: {}", digest);

        let blob_url = format!(
            "https://{}/v2/{}/{}/blobs/{}",
            registry, namespace, image, digest
        );

        let mut headers = HeaderMap::new();
        if let Some(token) = &self.auth_token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(token).context("Invalid auth token")?
            );
        }

        let response = self.client
            .get(&blob_url)
            .headers(headers)
            .send()
            .await
            .context("Failed to fetch blob")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to fetch blob: HTTP {}",
                response.status()
            ));
        }

        let data = response.bytes().await
            .context("Failed to read blob data")?;

        Ok(data.to_vec())
    }

    async fn extract_layer(&self, layer_path: &Path, rootfs_dir: &Path) -> Result<()> {
        debug!("📂 Extracting layer to rootfs");

        // Read the compressed layer
        let layer_file = std::fs::File::open(layer_path)
            .context("Failed to open layer file")?;

        // Decompress and extract
        let gz_decoder = GzDecoder::new(layer_file);
        let mut archive = Archive::new(gz_decoder);

        // Extract to rootfs
        archive.unpack(rootfs_dir)
            .context("Failed to extract layer")?;

        Ok(())
    }

    fn parse_image_reference(&self, image_ref: &str) -> Result<(String, String, String, String)> {
        // Parse image reference: [registry/]namespace/image[:tag]
        let parts: Vec<&str> = image_ref.split('/').collect();

        let (registry, namespace, image_with_tag) = match parts.len() {
            1 => {
                // Just image name (e.g., "nginx")
                ("registry-1.docker.io", "library", parts[0])
            }
            2 => {
                // namespace/image (e.g., "myuser/myimage")
                ("registry-1.docker.io", parts[0], parts[1])
            }
            3 => {
                // registry/namespace/image
                (parts[0], parts[1], parts[2])
            }
            _ => {
                return Err(anyhow::anyhow!("Invalid image reference: {}", image_ref));
            }
        };

        // Split image and tag
        let (image, tag) = if let Some(colon_pos) = image_with_tag.find(':') {
            (
                &image_with_tag[..colon_pos],
                &image_with_tag[colon_pos + 1..],
            )
        } else {
            (image_with_tag, "latest")
        };

        Ok((
            registry.to_string(),
            namespace.to_string(),
            image.to_string(),
            tag.to_string(),
        ))
    }

    pub async fn push_image(&self, image: &str, _data: &[u8]) -> Result<()> {
        info!("📤 Pushing image to registry: {}", image);
        warn!("Image push not yet implemented");
        Ok(())
    }
}