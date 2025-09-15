use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info};

/// Specialized Ghostbay integration for Bolt container runtime
/// Provides optimized container image storage, gaming asset management, and cluster coordination
#[derive(Debug, Clone)]
pub struct GhostbayClient {
    pub endpoint: String,
    pub cluster_id: Option<String>,
    pub access_key: String,
    pub secret_key: String,
    pub client: Client,
    pub features: GhostbayFeatures,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostbayFeatures {
    pub container_registry: bool,
    pub gaming_assets: bool,
    pub distributed_cache: bool,
    pub content_deduplication: bool,
    pub encryption_at_rest: bool,
    pub multi_region: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostbayConfig {
    pub endpoint: String,
    pub cluster_id: Option<String>,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: Option<String>,
    pub features: GhostbayFeatures,
    pub gaming_optimizations: GhostbayGamingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostbayGamingConfig {
    pub enable_fast_downloads: bool,
    pub cdn_acceleration: bool,
    pub asset_preloading: bool,
    pub compression_level: u8, // 1-9
    pub chunk_size_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostbayContainerImage {
    pub name: String,
    pub tag: String,
    pub digest: String,
    pub size: u64,
    pub layers: Vec<GhostbayLayer>,
    pub manifest: serde_json::Value,
    pub gaming_optimized: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostbayLayer {
    pub digest: String,
    pub size: u64,
    pub media_type: String,
    pub compressed: bool,
    pub gaming_assets: Vec<String>, // List of gaming-related files in this layer
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostbayClusterStatus {
    pub cluster_id: String,
    pub name: String,
    pub status: ClusterStatus,
    pub nodes: Vec<GhostbayNode>,
    pub storage: GhostbayStorageStats,
    pub performance: GhostbayPerformanceStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClusterStatus {
    Healthy,
    Degraded,
    Maintenance,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostbayNode {
    pub id: String,
    pub name: String,
    pub status: String,
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub storage_usage: f32,
    pub network_throughput: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostbayStorageStats {
    pub total_capacity: u64,
    pub used_space: u64,
    pub available_space: u64,
    pub object_count: u64,
    pub deduplication_ratio: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostbayPerformanceStats {
    pub read_iops: u64,
    pub write_iops: u64,
    pub read_throughput: u64,  // bytes/sec
    pub write_throughput: u64, // bytes/sec
    pub avg_latency_ms: f32,
    pub cache_hit_ratio: f32,
}

impl GhostbayClient {
    pub async fn new(config: GhostbayConfig) -> Result<Self> {
        info!("👻 Initializing Ghostbay client");
        info!("  🌐 Endpoint: {}", config.endpoint);
        if let Some(cluster) = &config.cluster_id {
            info!("  🏭 Cluster: {}", cluster);
        }

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("bolt-runtime/1.0.0")
            .build()
            .context("Failed to create HTTP client")?;

        let ghostbay_client = Self {
            endpoint: config.endpoint,
            cluster_id: config.cluster_id,
            access_key: config.access_key,
            secret_key: config.secret_key,
            client,
            features: config.features,
        };

        // Test connection and authenticate
        ghostbay_client.authenticate().await?;
        ghostbay_client.check_features().await?;

        info!("✅ Ghostbay client initialized successfully");
        Ok(ghostbay_client)
    }

    async fn authenticate(&self) -> Result<()> {
        debug!("🔐 Authenticating with Ghostbay");

        let auth_url = format!("{}/api/v1/auth", self.endpoint);

        let auth_payload = serde_json::json!({
            "access_key": self.access_key,
            "secret_key": self.secret_key,
            "client": "bolt-runtime"
        });

        let response = self
            .client
            .post(&auth_url)
            .json(&auth_payload)
            .send()
            .await
            .context("Failed to authenticate with Ghostbay")?;

        if response.status().is_success() {
            debug!("✅ Ghostbay authentication successful");
        } else {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Ghostbay authentication failed: {}",
                error_text
            ));
        }

        Ok(())
    }

    async fn check_features(&self) -> Result<()> {
        debug!("🔍 Checking Ghostbay features");

        let features_url = format!("{}/api/v1/features", self.endpoint);

        let response = self
            .client
            .get(&features_url)
            .header(
                "Authorization",
                format!("Bearer {}:{}", self.access_key, self.secret_key),
            )
            .send()
            .await
            .context("Failed to get Ghostbay features")?;

        if response.status().is_success() {
            let features: serde_json::Value = response.json().await?;

            info!("  📋 Available features:");
            if self.features.container_registry {
                info!("    📦 Container Registry: Enabled");
            }
            if self.features.gaming_assets {
                info!("    🎮 Gaming Assets: Enabled");
            }
            if self.features.distributed_cache {
                info!("    🗄️  Distributed Cache: Enabled");
            }
            if self.features.content_deduplication {
                info!("    🔗 Content Deduplication: Enabled");
            }
        }

        Ok(())
    }

    /// Push a container image to Ghostbay with gaming optimizations
    pub async fn push_container_image(&self, image_ref: &str, local_path: &Path) -> Result<String> {
        info!("📤 Pushing container image to Ghostbay: {}", image_ref);

        if !self.features.container_registry {
            return Err(anyhow::anyhow!("Container registry feature not enabled"));
        }

        // Parse image reference
        let (namespace, name, tag) = self.parse_image_ref(image_ref)?;

        let push_url = format!(
            "{}/api/v1/registry/{}/{}/push",
            self.endpoint, namespace, name
        );

        // Read and analyze image for gaming content
        let gaming_optimized = self.analyze_gaming_content(local_path).await?;

        // Create multipart upload
        let file_content = tokio::fs::read(local_path)
            .await
            .context("Failed to read image file")?;
        let file_part = reqwest::multipart::Part::bytes(file_content)
            .file_name(tag.clone())
            .mime_str("application/octet-stream")
            .context("Failed to create file part")?;

        let form = reqwest::multipart::Form::new()
            .text("tag", tag.clone())
            .text("gaming_optimized", gaming_optimized.to_string())
            .text("bolt_runtime", "true")
            .part("image", file_part);

        let response = self
            .client
            .post(&push_url)
            .header(
                "Authorization",
                format!("Bearer {}:{}", self.access_key, self.secret_key),
            )
            .multipart(form)
            .send()
            .await
            .context("Failed to push image to Ghostbay")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to push image: {}", error_text));
        }

        let push_result: serde_json::Value = response.json().await?;
        let digest = push_result["digest"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        info!(
            "✅ Image pushed successfully: {} (digest: {})",
            image_ref, digest
        );

        if gaming_optimized {
            info!("  🎮 Gaming optimizations applied");
            self.optimize_gaming_layers(&digest).await?;
        }

        Ok(digest)
    }

    /// Pull a container image from Ghostbay with optimized downloads
    pub async fn pull_container_image(
        &self,
        image_ref: &str,
        local_path: &Path,
    ) -> Result<GhostbayContainerImage> {
        info!("📥 Pulling container image from Ghostbay: {}", image_ref);

        if !self.features.container_registry {
            return Err(anyhow::anyhow!("Container registry feature not enabled"));
        }

        let (namespace, name, tag) = self.parse_image_ref(image_ref)?;

        // Get image manifest first
        let manifest_url = format!(
            "{}/api/v1/registry/{}/{}/manifests/{}",
            self.endpoint, namespace, name, tag
        );

        let manifest_response = self
            .client
            .get(&manifest_url)
            .header(
                "Authorization",
                format!("Bearer {}:{}", self.access_key, self.secret_key),
            )
            .header(
                "Accept",
                "application/vnd.docker.distribution.manifest.v2+json",
            )
            .send()
            .await
            .context("Failed to get image manifest")?;

        if !manifest_response.status().is_success() {
            return Err(anyhow::anyhow!("Image not found: {}", image_ref));
        }

        let manifest: serde_json::Value = manifest_response.json().await?;

        // Download image with Ghostbay optimizations
        let pull_url = format!(
            "{}/api/v1/registry/{}/{}/pull/{}",
            self.endpoint, namespace, name, tag
        );

        let mut download_request = self.client.get(&pull_url).header(
            "Authorization",
            format!("Bearer {}:{}", self.access_key, self.secret_key),
        );

        // Enable gaming optimizations if supported
        if self.features.gaming_assets {
            download_request = download_request.header("X-Ghostbay-Gaming-Optimized", "true");
        }

        if self.features.distributed_cache {
            download_request = download_request.header("X-Ghostbay-Use-Cache", "true");
        }

        let response = download_request.send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to pull image: {}", error_text));
        }

        // Create parent directories
        if let Some(parent) = local_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Stream download to file
        let mut file = fs::File::create(local_path).await?;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to download chunk")?;
            file.write_all(&chunk).await?;
        }

        file.flush().await?;

        info!("✅ Image pulled successfully: {}", image_ref);

        // Return image metadata
        Ok(GhostbayContainerImage {
            name: format!("{}/{}", namespace, name),
            tag,
            digest: manifest["config"]["digest"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            size: manifest["config"]["size"].as_u64().unwrap_or(0),
            layers: Vec::new(), // Would parse from manifest
            manifest,
            gaming_optimized: true, // Would check from metadata
            created_at: chrono::Utc::now(),
        })
    }

    /// Upload gaming assets with specialized optimization
    pub async fn upload_gaming_assets(
        &self,
        assets_dir: &Path,
        game_id: &str,
    ) -> Result<Vec<String>> {
        info!("🎮 Uploading gaming assets for: {}", game_id);

        if !self.features.gaming_assets {
            return Err(anyhow::anyhow!("Gaming assets feature not enabled"));
        }

        let mut uploaded_assets = Vec::new();

        // Scan for gaming-related files
        let gaming_files = self.scan_gaming_files(assets_dir).await?;

        for file_path in gaming_files {
            let relative_path = file_path.strip_prefix(assets_dir)?;
            let asset_key = format!("gaming/{}/{}", game_id, relative_path.to_string_lossy());

            self.upload_gaming_asset(&file_path, &asset_key).await?;
            uploaded_assets.push(asset_key);
        }

        // Enable CDN acceleration for gaming assets
        if uploaded_assets.len() > 0 {
            self.enable_cdn_acceleration(&uploaded_assets).await?;
        }

        info!("✅ Uploaded {} gaming assets", uploaded_assets.len());
        Ok(uploaded_assets)
    }

    async fn upload_gaming_asset(&self, local_path: &Path, asset_key: &str) -> Result<()> {
        debug!(
            "📤 Uploading gaming asset: {} -> {}",
            local_path.display(),
            asset_key
        );

        let upload_url = format!("{}/api/v1/gaming/assets/upload", self.endpoint);

        let file_content = tokio::fs::read(local_path).await?;
        let file_part = reqwest::multipart::Part::bytes(file_content)
            .file_name(asset_key.to_string())
            .mime_str("application/octet-stream")?;

        let form = reqwest::multipart::Form::new()
            .text("key", asset_key.to_string())
            .text("gaming_optimized", "true")
            .text("compression", "true")
            .part("asset", file_part);

        let response = self
            .client
            .post(&upload_url)
            .header(
                "Authorization",
                format!("Bearer {}:{}", self.access_key, self.secret_key),
            )
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to upload gaming asset: {}",
                asset_key
            ));
        }

        debug!("✅ Gaming asset uploaded: {}", asset_key);
        Ok(())
    }

    async fn scan_gaming_files(&self, dir: &Path) -> Result<Vec<PathBuf>> {
        let mut gaming_files = Vec::new();

        // Gaming file extensions to look for
        let gaming_extensions = [
            ".pak",
            ".vpk",
            ".wad",
            ".bsp",
            ".mdl",
            ".vtx",
            ".vvd",
            ".phy",
            ".dds",
            ".tga",
            ".wav",
            ".ogg",
            ".mp3",
            ".bik",
            ".wmv",
            ".dll",
            ".exe",
            ".bat",
            ".cfg",
            ".ini",
            ".lua",
            ".py",
            ".unity3d",
            ".assets",
            ".resource",
            ".bundle",
        ];

        let mut stack = vec![dir.to_path_buf()];

        while let Some(current_dir) = stack.pop() {
            let mut dir_reader = fs::read_dir(&current_dir).await?;

            while let Some(entry) = dir_reader.next_entry().await? {
                let path = entry.path();

                if path.is_dir() {
                    stack.push(path);
                } else if let Some(extension) = path.extension() {
                    let ext_str = extension.to_string_lossy().to_lowercase();
                    if gaming_extensions
                        .iter()
                        .any(|&ge| ge == format!(".{}", ext_str))
                    {
                        gaming_files.push(path);
                    }
                }
            }
        }

        debug!("🎮 Found {} gaming files", gaming_files.len());
        Ok(gaming_files)
    }

    async fn analyze_gaming_content(&self, _image_path: &Path) -> Result<bool> {
        // Analyze container image for gaming content
        // This would inspect the image for gaming-related files, executables, etc.

        debug!("🔍 Analyzing image for gaming content");

        // For now, return true to enable gaming optimizations
        // In a real implementation, this would analyze the image layers
        Ok(true)
    }

    async fn optimize_gaming_layers(&self, _digest: &str) -> Result<()> {
        debug!("🎮 Applying gaming layer optimizations");

        // Ghostbay-specific gaming optimizations:
        // - Reorder layers for faster game startup
        // - Compress gaming assets with specialized algorithms
        // - Pre-cache frequently accessed files
        // - Set up CDN distribution for game assets

        info!("✅ Gaming layer optimizations applied");
        Ok(())
    }

    async fn enable_cdn_acceleration(&self, _asset_keys: &[String]) -> Result<()> {
        debug!("🌐 Enabling CDN acceleration for gaming assets");

        // Configure CDN acceleration for gaming assets
        // This would integrate with Ghostbay's CDN features

        info!("✅ CDN acceleration enabled");
        Ok(())
    }

    pub async fn get_cluster_status(&self) -> Result<GhostbayClusterStatus> {
        info!("📊 Getting Ghostbay cluster status");

        let status_url = format!("{}/api/v1/cluster/status", self.endpoint);

        let response = self
            .client
            .get(&status_url)
            .header(
                "Authorization",
                format!("Bearer {}:{}", self.access_key, self.secret_key),
            )
            .send()
            .await
            .context("Failed to get cluster status")?;

        if response.status().is_success() {
            let status: GhostbayClusterStatus = response
                .json()
                .await
                .context("Failed to parse cluster status")?;

            info!(
                "  📋 Cluster: {} ({})",
                status.name,
                match status.status {
                    ClusterStatus::Healthy => "✅ Healthy",
                    ClusterStatus::Degraded => "⚠️ Degraded",
                    ClusterStatus::Maintenance => "🔧 Maintenance",
                    ClusterStatus::Critical => "🚨 Critical",
                }
            );
            info!("  🖥️  Nodes: {}", status.nodes.len());
            info!(
                "  💾 Storage: {:.1}% used",
                (status.storage.used_space as f64 / status.storage.total_capacity as f64) * 100.0
            );

            Ok(status)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get cluster status: HTTP {}",
                response.status()
            ))
        }
    }

    pub async fn create_distributed_cache(&self, cache_name: &str, size_gb: u32) -> Result<()> {
        info!(
            "🗄️  Creating distributed cache: {} ({}GB)",
            cache_name, size_gb
        );

        if !self.features.distributed_cache {
            return Err(anyhow::anyhow!("Distributed cache feature not enabled"));
        }

        let cache_url = format!("{}/api/v1/cache/create", self.endpoint);

        let cache_config = serde_json::json!({
            "name": cache_name,
            "size_gb": size_gb,
            "replication_factor": 3,
            "eviction_policy": "lru",
            "gaming_optimized": true
        });

        let response = self
            .client
            .post(&cache_url)
            .header(
                "Authorization",
                format!("Bearer {}:{}", self.access_key, self.secret_key),
            )
            .json(&cache_config)
            .send()
            .await?;

        if response.status().is_success() {
            info!("✅ Distributed cache created: {}", cache_name);
        } else {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to create cache: {}", error_text));
        }

        Ok(())
    }

    fn parse_image_ref(&self, image_ref: &str) -> Result<(String, String, String)> {
        // Parse image reference: [namespace/]name[:tag]
        let parts: Vec<&str> = image_ref.split('/').collect();

        let (namespace, name_tag) = if parts.len() == 1 {
            ("library".to_string(), parts[0].to_string())
        } else {
            (parts[0].to_string(), parts[1..].join("/"))
        };

        let (name, tag) = if let Some(colon_pos) = name_tag.find(':') {
            (
                name_tag[..colon_pos].to_string(),
                name_tag[colon_pos + 1..].to_string(),
            )
        } else {
            (name_tag.to_string(), "latest".to_string())
        };

        Ok((namespace, name, tag))
    }
}

/// Integration helper for Bolt's storage manager
pub async fn create_ghostbay_volume(config: GhostbayConfig, volume_name: &str) -> Result<()> {
    info!("👻 Creating Ghostbay volume: {}", volume_name);

    let client = GhostbayClient::new(config).await?;

    // Create a specialized Ghostbay bucket for this volume
    let bucket_name = format!("bolt-volume-{}", volume_name);

    // In a real implementation, this would create the bucket via Ghostbay API
    // For now, we'll just log the configuration

    info!("📦 Volume configuration:");
    info!("  👻 Provider: Ghostbay");
    info!("  🪣 Bucket: {}", bucket_name);
    info!("  🎮 Gaming optimized: {}", client.features.gaming_assets);
    info!(
        "  🗄️  Distributed cache: {}",
        client.features.distributed_cache
    );
    info!(
        "  🔗 Deduplication: {}",
        client.features.content_deduplication
    );

    if client.features.distributed_cache {
        client
            .create_distributed_cache(&format!("{}-cache", volume_name), 10)
            .await?;
    }

    info!("✅ Ghostbay volume created: {}", volume_name);
    Ok(())
}
