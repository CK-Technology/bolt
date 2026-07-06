use crate::{
    BoltError, Result,
    registry::drift_integration::{DriftRegistryClient, PackageManifest},
};
use anyhow::{Context, anyhow};
use chrono::{DateTime, Utc};
use dirs::data_dir;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt;
use std::fs as stdfs;
use std::io::Read;
#[cfg(test)]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tar::Archive;
use tokio::fs;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::task;
use tracing::{debug, info, warn};
#[cfg(test)]
use {
    flate2::{Compression, write::GzEncoder},
    tar::{Builder, EntryType, Header},
};

#[cfg(unix)]
#[cfg(test)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

// Boltfile TOML Configuration Structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltfileBuildConfig {
    /// Base image configuration
    pub base: Option<BoltBaseConfig>,
    /// Package dependencies
    pub dependencies: Option<BoltDependencies>,
    /// File operations (copy, add)
    pub files: Option<Vec<BoltFileOperation>>,
    /// Commands to run during build
    pub run: Option<Vec<String>>,
    /// Runtime configuration
    pub runtime: Option<BoltRuntimeConfig>,
    /// Bolt-specific optimizations
    pub optimization: Option<BoltOptimizationConfig>,
    /// Image metadata
    pub metadata: Option<BoltMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltBaseConfig {
    /// Base image (e.g., "ubuntu:22.04")
    pub image: String,
    /// Platform (e.g., "linux/amd64")
    pub platform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltDependencies {
    /// System packages to install
    pub packages: Vec<String>,
    /// Package manager to use
    pub manager: Option<String>, // apt, yum, apk, etc.
    /// Gaming libraries
    pub gaming: Option<Vec<String>>,
    /// Development tools
    pub dev: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BoltFileOperation {
    #[serde(rename = "copy")]
    Copy { from: String, to: String },
    #[serde(rename = "add")]
    Add { url: String, to: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltRuntimeConfig {
    /// Default command
    pub cmd: Option<Vec<String>>,
    /// Entrypoint
    pub entrypoint: Option<Vec<String>>,
    /// Working directory
    pub workdir: Option<String>,
    /// Environment variables
    pub env: Option<Vec<String>>,
    /// Exposed ports
    pub expose: Option<Vec<u16>>,
    /// User to run as
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltOptimizationConfig {
    /// Enable gaming optimizations
    pub gaming: Option<bool>,
    /// Performance tier: "maximum", "balanced", "efficiency"
    pub performance_tier: Option<String>,
    /// Enable QUIC networking
    pub quic_networking: Option<bool>,
    /// GPU acceleration
    pub gpu: Option<BoltGpuConfig>,
    /// Memory optimizations
    pub memory: Option<BoltMemoryConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltGpuConfig {
    /// Enable GPU passthrough
    pub enabled: Option<bool>,
    /// NVIDIA runtime
    pub nvidia: Option<bool>,
    /// AMD GPU support
    pub amd: Option<bool>,
    /// Exclusive GPU mode
    pub exclusive: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltMemoryConfig {
    /// Use huge pages
    pub huge_pages: Option<bool>,
    /// Memory prefaulting
    pub prefault: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltMetadata {
    /// Image labels
    pub labels: Option<HashMap<String, String>>,
    /// Author information
    pub author: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Version
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BoltfileBuildContext {
    pub context_path: PathBuf,
    pub build_config: BoltfileBuildConfig,
    pub tag: String,
}

// Legacy Dockerfile support structures
#[derive(Debug, Clone)]
pub struct DockerfileBuildContext {
    pub context_path: PathBuf,
    pub dockerfile_content: String,
    pub tag: String,
}

#[derive(Debug, Clone)]
pub enum DockerfileInstruction {
    From { image: String },
    Run { command: String },
    Copy { src: String, dest: String },
    Add { src: String, dest: String },
    Workdir { path: String },
    Cmd { args: Vec<String> },
    Entrypoint { args: Vec<String> },
    Env { key: String, value: String },
    Expose { port: String },
    User { user: String },
    Label { key: String, value: String },
}

pub mod object_store {
    use super::*;
    use aws_config::BehaviorVersion;
    use aws_sdk_s3::Client as S3Client;
    use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Credentials, Region};
    use aws_smithy_types::byte_stream::ByteStream;

    use async_trait::async_trait;

    #[async_trait]
    pub trait ObjectStore: Send + Sync {
        async fn blob_exists(&self, repository: &str, digest: &str) -> Result<bool>;
        async fn download_cached_layer(
            &self,
            repository: &str,
            digest: &str,
            destination: &Path,
        ) -> Result<bool>;
        async fn upload_layer(&self, repository: &str, digest: &str, source: &Path) -> Result<()>;
        async fn download_cached_config(
            &self,
            repository: &str,
            digest: &str,
            destination: &Path,
        ) -> Result<bool>;
        async fn download_config(
            &self,
            repository: &str,
            digest: &str,
            destination: &Path,
        ) -> Result<()>;
        async fn upload_config(&self, repository: &str, digest: &str, source: &Path) -> Result<()>;
        async fn fetch_manifest(
            &self,
            repository: &str,
            reference: &str,
        ) -> Result<Option<Vec<u8>>>;
        async fn store_manifest(
            &self,
            repository: &str,
            reference: &str,
            data: &[u8],
        ) -> Result<()>;
    }

    #[derive(Debug, Clone)]
    pub struct ObjectStoreConfig {
        pub endpoint: Option<String>,
        pub bucket: String,
        pub region: Option<String>,
        pub access_key: Option<String>,
        pub secret_key: Option<String>,
        pub session_token: Option<String>,
        pub prefix: Option<String>,
        pub path_style: bool,
        pub provider_hint: Option<String>,
    }

    impl ObjectStoreConfig {
        pub fn detect_from_env() -> Result<Option<Self>> {
            let backend = env::var("BOLT_STORAGE_BACKEND")
                .ok()
                .map(|v| v.to_ascii_lowercase());
            let force_s3 = matches!(backend.as_deref(), Some("s3"));
            let force_local = matches!(backend.as_deref(), Some("local"));

            if force_local {
                return Ok(None);
            }

            let endpoint = env::var("BOLT_S3_ENDPOINT")
                .or_else(|_| env::var("GHOSTBAY_ENDPOINT"))
                .ok();
            let bucket = env::var("BOLT_S3_BUCKET")
                .or_else(|_| env::var("GHOSTBAY_BUCKET"))
                .or_else(|_| env::var("AWS_S3_BUCKET"))
                .ok();

            if !force_s3 && endpoint.is_none() && bucket.is_none() {
                return Ok(None);
            }

            let bucket = bucket.ok_or_else(|| {
                BoltError::Config(crate::error::ConfigError::MissingField {
                    field: "BOLT_S3_BUCKET or GHOSTBAY_BUCKET".to_string(),
                })
            })?;

            let region = env::var("BOLT_S3_REGION")
                .or_else(|_| env::var("GHOSTBAY_REGION"))
                .or_else(|_| env::var("AWS_REGION"))
                .ok();

            let access_key = env::var("BOLT_S3_ACCESS_KEY")
                .or_else(|_| env::var("GHOSTBAY_ACCESS_KEY"))
                .or_else(|_| env::var("AWS_ACCESS_KEY_ID"))
                .ok();

            let secret_key = env::var("BOLT_S3_SECRET_KEY")
                .or_else(|_| env::var("GHOSTBAY_SECRET_KEY"))
                .or_else(|_| env::var("AWS_SECRET_ACCESS_KEY"))
                .ok();

            let session_token = env::var("BOLT_S3_SESSION_TOKEN")
                .or_else(|_| env::var("AWS_SESSION_TOKEN"))
                .ok();

            let prefix = env::var("BOLT_S3_PREFIX")
                .or_else(|_| env::var("GHOSTBAY_PREFIX"))
                .ok();

            let path_style = env::var("BOLT_S3_PATH_STYLE")
                .or_else(|_| env::var("GHOSTBAY_PATH_STYLE"))
                .ok()
                .map(|v| super::parse_bool(&v))
                .unwrap_or_else(|| {
                    endpoint
                        .as_ref()
                        .map(|ep| ep.contains("localhost") || ep.contains("127.0.0.1"))
                        .unwrap_or(false)
                });

            let provider_hint = env::var("BOLT_S3_PROVIDER")
                .or_else(|_| env::var("GHOSTBAY_PROVIDER"))
                .ok()
                .or_else(|| {
                    endpoint.as_ref().map(|ep| {
                        if ep.contains("amazonaws.com") {
                            "aws".to_string()
                        } else if ep.contains("wasabi") {
                            "wasabi".to_string()
                        } else if ep.contains("backblaze") {
                            "backblaze".to_string()
                        } else if ep.contains("minio") {
                            "minio".to_string()
                        } else {
                            "custom".to_string()
                        }
                    })
                });

            Ok(Some(Self {
                endpoint,
                bucket,
                region,
                access_key,
                secret_key,
                session_token,
                prefix,
                path_style,
                provider_hint,
            }))
        }
    }

    #[derive(Debug, Clone)]
    pub struct ObjectStoreClient {
        bucket: String,
        prefix: Option<String>,
        client: S3Client,
        endpoint: Option<String>,
        provider_hint: Option<String>,
    }

    impl ObjectStoreClient {
        pub async fn new(config: ObjectStoreConfig) -> Result<Self> {
            let mut loader = aws_config::defaults(BehaviorVersion::latest());

            if let Some(region) = &config.region {
                loader = loader.region(Region::new(region.clone()));
            }

            if let Some(endpoint) = &config.endpoint {
                loader = loader.endpoint_url(endpoint.clone());
            }

            // Pass explicit credentials through the SDK's credential provider
            // rather than the process environment. Mutating AWS_* via
            // std::env::set_var is process-global and races across concurrent
            // clients; a per-config provider keeps credentials scoped to this
            // client. Falls back to the default credential chain when unset.
            if let (Some(access_key), Some(secret_key)) = (&config.access_key, &config.secret_key) {
                let credentials = Credentials::new(
                    access_key.clone(),
                    secret_key.clone(),
                    config.session_token.clone(),
                    None,
                    "bolt-object-store",
                );
                loader = loader.credentials_provider(credentials);
            }

            let shared_config = loader.load().await;
            let mut builder = S3ConfigBuilder::from(&shared_config);

            if config.path_style {
                builder = builder.force_path_style(true);
            }

            if let Some(region) = &config.region {
                builder = builder.region(Region::new(region.clone()));
            }

            let client = S3Client::from_conf(builder.build());

            if let Some(endpoint) = &config.endpoint {
                info!(
                    "🪣 Object store enabled (bucket: {}) via {}",
                    config.bucket, endpoint
                );
            } else {
                info!(
                    "🪣 Object store enabled (bucket: {}) via AWS global endpoint",
                    config.bucket
                );
            }

            Ok(Self {
                bucket: config.bucket,
                prefix: config.prefix,
                client,
                endpoint: config.endpoint,
                provider_hint: config.provider_hint,
            })
        }

        pub fn provider(&self) -> Option<&str> {
            self.provider_hint.as_deref()
        }

        pub fn endpoint(&self) -> Option<&str> {
            self.endpoint.as_deref()
        }

        fn object_key(&self, repository: &str, digest: &str, suffix: &str) -> String {
            let repo_segment = repository.replace('/', "_");
            let digest_segment = digest.replace(':', "_");
            let base = format!("{repo_segment}/{digest_segment}{suffix}");
            if let Some(prefix) = &self.prefix {
                format!("{}/{}", prefix.trim_end_matches('/'), base)
            } else {
                base
            }
        }

        fn config_key(&self, repository: &str, digest: &str) -> String {
            self.object_key(repository, digest, ".config")
        }

        fn manifest_key(&self, repository: &str, reference: &str) -> String {
            let repo_segment = repository.replace('/', "_");
            let reference_segment = reference.replace(':', "_");
            let base = format!("{repo_segment}/manifests/{reference_segment}.json");
            if let Some(prefix) = &self.prefix {
                format!("{}/{}", prefix.trim_end_matches('/'), base)
            } else {
                base
            }
        }

        async fn object_exists(&self, key: &str) -> Result<bool> {
            match self
                .client
                .head_object()
                .bucket(&self.bucket)
                .key(key)
                .send()
                .await
            {
                Ok(_) => Ok(true),
                Err(err) => {
                    let err_str = err.to_string();
                    if err_str.contains("NoSuchKey") || err_str.contains("404") {
                        Ok(false)
                    } else {
                        Err(anyhow!("object store head_object failed for {key}: {err}").into())
                    }
                }
            }
        }

        async fn download_key_to_path(&self, key: &str, destination: &Path) -> Result<()> {
            let response = self
                .client
                .get_object()
                .bucket(&self.bucket)
                .key(key)
                .send()
                .await
                .with_context(|| format!("object store get_object failed for {}", key))?;

            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)
                    .await
                    .with_context(|| format!("Failed to create {}", parent.display()))?;
            }

            let mut reader = response.body.into_async_read();
            let mut file = fs::File::create(destination)
                .await
                .with_context(|| format!("Failed to create {}", destination.display()))?;
            io::copy(&mut reader, &mut file)
                .await
                .with_context(|| format!("Failed to write {}", destination.display()))?;
            file.flush()
                .await
                .with_context(|| format!("Failed to flush {}", destination.display()))?;
            Ok(())
        }

        async fn read_object_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
            match self
                .client
                .get_object()
                .bucket(&self.bucket)
                .key(key)
                .send()
                .await
            {
                Ok(response) => {
                    let mut reader = response.body.into_async_read();
                    let mut data = Vec::new();
                    reader
                        .read_to_end(&mut data)
                        .await
                        .with_context(|| format!("Failed to read object {}", key))?;
                    Ok(Some(data))
                }
                Err(err) => {
                    let err_str = err.to_string();
                    if err_str.contains("NoSuchKey") || err_str.contains("404") {
                        Ok(None)
                    } else {
                        Err(anyhow!("object store get_object failed for {key}: {err}").into())
                    }
                }
            }
        }

        async fn put_bytes(&self, key: &str, data: Vec<u8>) -> Result<()> {
            let body = ByteStream::from(data);
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .body(body)
                .send()
                .await
                .with_context(|| format!("object store put_object failed for {}", key))?;
            Ok(())
        }

        pub async fn blob_exists(&self, repository: &str, digest: &str) -> Result<bool> {
            let key = self.object_key(repository, digest, ".layer");
            self.object_exists(&key).await
        }

        pub async fn download_cached_layer(
            &self,
            repository: &str,
            digest: &str,
            destination: &Path,
        ) -> Result<bool> {
            if !self.blob_exists(repository, digest).await? {
                return Ok(false);
            }

            if let Err(err) = self.download_layer(repository, digest, destination).await {
                warn!(
                    "Object store download failed for {} ({}): {err}",
                    digest, repository
                );
                Ok(false)
            } else {
                Ok(true)
            }
        }

        pub async fn download_layer(
            &self,
            repository: &str,
            digest: &str,
            destination: &Path,
        ) -> Result<()> {
            let key = self.object_key(repository, digest, ".layer");
            self.download_key_to_path(&key, destination).await
        }

        pub async fn upload_layer(
            &self,
            repository: &str,
            digest: &str,
            source: &Path,
        ) -> Result<()> {
            let key = self.object_key(repository, digest, ".layer");

            let body = ByteStream::from_path(source.to_path_buf())
                .await
                .with_context(|| format!("Failed to open {}", source.display()))?;

            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(&key)
                .body(body)
                .send()
                .await
                .with_context(|| format!("object store put_object failed for {}", key))?;
            Ok(())
        }

        pub async fn download_cached_config(
            &self,
            repository: &str,
            digest: &str,
            destination: &Path,
        ) -> Result<bool> {
            let key = self.config_key(repository, digest);
            if !self.object_exists(&key).await? {
                return Ok(false);
            }

            if let Err(err) = self.download_key_to_path(&key, destination).await {
                warn!(
                    "Object store config download failed for {} ({}): {}",
                    digest, repository, err
                );
                Ok(false)
            } else {
                Ok(true)
            }
        }

        pub async fn download_config(
            &self,
            repository: &str,
            digest: &str,
            destination: &Path,
        ) -> Result<()> {
            let key = self.config_key(repository, digest);
            self.download_key_to_path(&key, destination).await
        }

        pub async fn upload_config(
            &self,
            repository: &str,
            digest: &str,
            source: &Path,
        ) -> Result<()> {
            let key = self.config_key(repository, digest);
            let data = fs::read(source)
                .await
                .with_context(|| format!("Failed to read {}", source.display()))?;
            self.put_bytes(&key, data).await
        }

        pub async fn fetch_manifest(
            &self,
            repository: &str,
            reference: &str,
        ) -> Result<Option<Vec<u8>>> {
            let key = self.manifest_key(repository, reference);
            self.read_object_bytes(&key).await
        }

        pub async fn store_manifest(
            &self,
            repository: &str,
            reference: &str,
            data: &[u8],
        ) -> Result<()> {
            let key = self.manifest_key(repository, reference);
            self.put_bytes(&key, data.to_vec()).await
        }
    }

    #[async_trait]
    impl ObjectStore for ObjectStoreClient {
        async fn blob_exists(&self, repository: &str, digest: &str) -> Result<bool> {
            ObjectStoreClient::blob_exists(self, repository, digest).await
        }

        async fn download_cached_layer(
            &self,
            repository: &str,
            digest: &str,
            destination: &Path,
        ) -> Result<bool> {
            ObjectStoreClient::download_cached_layer(self, repository, digest, destination).await
        }

        async fn upload_layer(&self, repository: &str, digest: &str, source: &Path) -> Result<()> {
            ObjectStoreClient::upload_layer(self, repository, digest, source).await
        }

        async fn download_cached_config(
            &self,
            repository: &str,
            digest: &str,
            destination: &Path,
        ) -> Result<bool> {
            ObjectStoreClient::download_cached_config(self, repository, digest, destination).await
        }

        async fn download_config(
            &self,
            repository: &str,
            digest: &str,
            destination: &Path,
        ) -> Result<()> {
            ObjectStoreClient::download_config(self, repository, digest, destination).await
        }

        async fn upload_config(&self, repository: &str, digest: &str, source: &Path) -> Result<()> {
            ObjectStoreClient::upload_config(self, repository, digest, source).await
        }

        async fn fetch_manifest(
            &self,
            repository: &str,
            reference: &str,
        ) -> Result<Option<Vec<u8>>> {
            ObjectStoreClient::fetch_manifest(self, repository, reference).await
        }

        async fn store_manifest(
            &self,
            repository: &str,
            reference: &str,
            data: &[u8],
        ) -> Result<()> {
            ObjectStoreClient::store_manifest(self, repository, reference, data).await
        }
    }
}

use object_store::{ObjectStore, ObjectStoreClient, ObjectStoreConfig};

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

pub struct StorageManager {
    storage_root: PathBuf,
    images: HashMap<String, ImageMetadata>,
    registry: DriftRegistryClient,
    object_store: Option<Arc<dyn ObjectStore>>,
}

impl fmt::Debug for StorageManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cached: Vec<&String> = self.images.keys().collect();
        f.debug_struct("StorageManager")
            .field("storage_root", &self.storage_root)
            .field("cached_images", &cached)
            .field("has_object_store", &self.object_store.is_some())
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub name: String,
    pub tag: String,
    #[serde(default)]
    pub reference: Option<String>,
    pub digest: String,
    pub size: u64,
    pub created: DateTime<Utc>,
    pub layers: Vec<LayerMetadata>,
    pub config: ImageConfig,
    #[serde(default)]
    pub config_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerMetadata {
    pub digest: String,
    pub size: u64,
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    pub env: Vec<String>,
    pub cmd: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub user: Option<String>,
    pub exposed_ports: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGcCandidate {
    pub reference: String,
    pub digest: String,
    pub bytes: u64,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootGcCandidate {
    pub kind: String,
    pub id: String,
    pub bytes: u64,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImageGcReport {
    pub dry_run: bool,
    pub candidates: Vec<ImageGcCandidate>,
    pub roots: Vec<RootGcCandidate>,
    pub reclaimed_bytes: u64,
    #[serde(default)]
    pub protected_images: Vec<String>,
}

struct StorageBootstrap {
    root: PathBuf,
    registry_endpoint: String,
    registry_credentials: Option<(String, String)>,
    object_store: Option<ObjectStoreConfig>,
}

/// Resolve the Bolt storage root, honoring `BOLT_STORAGE_ROOT` and otherwise
/// falling back to the platform data directory. Shared so the runtime and the
/// CLI (which read persisted state directly) agree on the same location.
pub fn storage_root() -> PathBuf {
    env::var("BOLT_STORAGE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("bolt")
        })
}

impl StorageBootstrap {
    fn detect() -> Result<Self> {
        let root = storage_root();

        let registry_endpoint = env::var("BOLT_REGISTRY_ENDPOINT")
            .or_else(|_| env::var("DRIFT_REGISTRY_ENDPOINT"))
            .unwrap_or_else(|_| "https://registry-1.docker.io".to_string());

        let registry_credentials = match (
            env::var("BOLT_REGISTRY_USERNAME").ok(),
            env::var("BOLT_REGISTRY_PASSWORD").ok(),
        ) {
            (Some(user), Some(pass)) if !user.is_empty() && !pass.is_empty() => Some((user, pass)),
            _ => None,
        };

        let object_store = ObjectStoreConfig::detect_from_env()?;

        Ok(Self {
            root,
            registry_endpoint,
            registry_credentials,
            object_store,
        })
    }
}

impl StorageManager {
    pub async fn new() -> Result<Self> {
        let bootstrap = StorageBootstrap::detect()?;

        fs::create_dir_all(bootstrap.root.join("images"))
            .await
            .context("Failed to initialize image storage directory")?;
        fs::create_dir_all(bootstrap.root.join("containers"))
            .await
            .context("Failed to initialize container storage directory")?;

        let object_store: Option<Arc<dyn ObjectStore>> =
            if let Some(cfg) = bootstrap.object_store.clone() {
                let client = ObjectStoreClient::new(cfg.clone()).await?;
                if let Some(provider) = client.provider() {
                    info!("🔗 Object store provider: {}", provider);
                }
                if let Some(endpoint) = client.endpoint() {
                    debug!("Object store endpoint set to {}", endpoint);
                }
                let client: Arc<dyn ObjectStore> = Arc::new(client);
                Some(client)
            } else {
                info!("📦 Using local filesystem storage backend");
                None
            };

        let registry = DriftRegistryClient::new(
            bootstrap.registry_endpoint,
            object_store.clone(),
            bootstrap.registry_credentials,
        )
        .await?;

        let mut manager = Self {
            storage_root: bootstrap.root,
            images: HashMap::new(),
            registry,
            object_store,
        };

        manager.load_existing_images().await?;
        Ok(manager)
    }

    pub fn get_cached_image_metadata(&self, image: &str) -> Option<ImageMetadata> {
        let reference = normalize_reference(image);
        self.images.get(&reference).cloned()
    }

    pub async fn image_exists(&self, image: &str) -> Result<bool> {
        let reference = normalize_reference(image);
        if self.images.contains_key(&reference) {
            return Ok(true);
        }

        let image_path = self.get_image_path(&reference);
        Ok(image_path.exists())
    }

    pub fn list_cached_images(&self) -> Vec<(String, ImageMetadata)> {
        let mut images: Vec<_> = self
            .images
            .iter()
            .map(|(reference, metadata)| (reference.clone(), metadata.clone()))
            .collect();
        images.sort_by(|a, b| a.0.cmp(&b.0));
        images
    }

    pub async fn inspect_image(&self, image: &str) -> Result<(String, ImageMetadata, bool)> {
        let reference = normalize_reference(image);
        let metadata = self
            .images
            .get(&reference)
            .cloned()
            .ok_or_else(|| anyhow!("image '{}' not found", image))?;
        let pins = self.load_image_pins().await?;
        Ok((reference.clone(), metadata, pins.contains(&reference)))
    }

    pub async fn pin_image(&self, image: &str) -> Result<()> {
        let reference = normalize_reference(image);
        if !self.images.contains_key(&reference) {
            return Err(anyhow!("image '{}' not found", image).into());
        }
        let mut pins = self.load_image_pins().await?;
        pins.insert(reference);
        self.save_image_pins(&pins).await
    }

    pub async fn unpin_image(&self, image: &str) -> Result<()> {
        let reference = normalize_reference(image);
        let mut pins = self.load_image_pins().await?;
        pins.remove(&reference);
        self.save_image_pins(&pins).await
    }

    async fn load_image_pins(&self) -> Result<HashSet<String>> {
        let path = self.image_pins_path();
        if !path.exists() {
            return Ok(HashSet::new());
        }
        let data = fs::read_to_string(&path).await?;
        Ok(serde_json::from_str(&data)?)
    }

    async fn save_image_pins(&self, pins: &HashSet<String>) -> Result<()> {
        let path = self.image_pins_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, serde_json::to_string_pretty(pins)?).await?;
        Ok(())
    }

    fn image_pins_path(&self) -> PathBuf {
        self.storage_root.join("image_pins.json")
    }

    pub async fn prune_images(
        &mut self,
        protected_references: &HashSet<String>,
        protected_digests: &HashSet<String>,
        protected_container_ids: &HashSet<String>,
        dry_run: bool,
    ) -> Result<ImageGcReport> {
        let mut report = ImageGcReport {
            dry_run,
            ..ImageGcReport::default()
        };

        let mut candidates = Vec::new();
        let mut candidate_paths = HashSet::new();
        let pinned_images = self.load_image_pins().await.unwrap_or_default();
        for (reference, metadata) in &self.images {
            if protected_references.contains(reference) {
                report
                    .protected_images
                    .push(format!("{reference} (container reference)"));
                continue;
            }
            if pinned_images.contains(reference) {
                report
                    .protected_images
                    .push(format!("{reference} (pinned)"));
                continue;
            }
            if image_metadata_is_protected(metadata, protected_digests) {
                report
                    .protected_images
                    .push(format!("{reference} (protected digest)"));
                continue;
            }
            let path = self.get_image_path(reference);
            candidate_paths.insert(path.clone());
            candidates.push(ImageGcCandidate {
                reference: reference.clone(),
                digest: metadata.digest.clone(),
                bytes: self
                    .image_disk_usage(reference)
                    .await
                    .unwrap_or(metadata.size),
                path,
            });
        }

        let images_dir = self.storage_root.join("images");
        if images_dir.exists() {
            let mut entries = fs::read_dir(&images_dir)
                .await
                .context("Failed to read images directory for GC")?;
            while let Some(entry) = entries
                .next_entry()
                .await
                .context("Failed to iterate images directory for GC")?
            {
                if !entry.file_type().await?.is_dir() {
                    continue;
                }
                let path = entry.path();
                if candidate_paths.contains(&path) {
                    continue;
                }
                let metadata_path = path.join("metadata.json");
                if metadata_path.exists() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                candidates.push(ImageGcCandidate {
                    reference: format!("stale-image-dir:{name}"),
                    digest: "unknown".to_string(),
                    bytes: disk_usage(&path).await.unwrap_or(0),
                    path,
                });
            }
        }

        let containers_dir = self.storage_root.join("containers");
        if containers_dir.exists() {
            let mut entries = fs::read_dir(&containers_dir)
                .await
                .context("Failed to read containers directory for GC")?;
            while let Some(entry) = entries
                .next_entry()
                .await
                .context("Failed to iterate containers directory for GC")?
            {
                if !entry.file_type().await?.is_dir() {
                    continue;
                }
                let path = entry.path();
                let Some(id) = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(ToOwned::to_owned)
                else {
                    continue;
                };
                if protected_container_ids.contains(&id) {
                    continue;
                }
                report.roots.push(RootGcCandidate {
                    kind: "stale-container-bundle".to_string(),
                    id,
                    bytes: disk_usage(&path).await.unwrap_or(0),
                    path,
                });
            }
        }

        candidates.sort_by(|a, b| a.reference.cmp(&b.reference));
        report.roots.sort_by(|a, b| a.id.cmp(&b.id));
        report.reclaimed_bytes = candidates
            .iter()
            .map(|candidate| candidate.bytes)
            .sum::<u64>()
            + report
                .roots
                .iter()
                .map(|candidate| candidate.bytes)
                .sum::<u64>();

        if !dry_run {
            for candidate in &candidates {
                if candidate.path.exists() {
                    fs::remove_dir_all(&candidate.path).await.with_context(|| {
                        format!("Failed to remove image at {}", candidate.path.display())
                    })?;
                }
                self.images.remove(&candidate.reference);
            }
            for candidate in &report.roots {
                if candidate.path.exists() {
                    fs::remove_dir_all(&candidate.path).await.with_context(|| {
                        format!("Failed to remove GC root at {}", candidate.path.display())
                    })?;
                }
            }
        }

        report.candidates = candidates;
        Ok(report)
    }

    async fn image_disk_usage(&self, reference: &str) -> Result<u64> {
        let path = self.get_image_path(reference);
        disk_usage(&path).await
    }

    pub async fn pull_image(&mut self, image: &str) -> Result<ImageMetadata> {
        info!("⬇️  Pulling image: {}", image);

        let reference = normalize_reference(image);
        if let Some(metadata) = self.get_cached_image_metadata(&reference) {
            debug!("Using cached image metadata for {}", reference);
            return Ok(metadata);
        }

        let resolved = self.registry.resolve_manifest(&reference).await?;

        let image_path = self.get_image_path(&reference);
        fs::create_dir_all(&image_path).await.with_context(|| {
            format!("Failed to create image directory {}", image_path.display())
        })?;

        let layers_dir = image_path.join("layers");
        fs::create_dir_all(&layers_dir).await.with_context(|| {
            format!(
                "Failed to prepare layers directory {}",
                layers_dir.display()
            )
        })?;

        let mut layer_metadata = Vec::new();
        for layer in &resolved.manifest.layers {
            let layer_meta = LayerMetadata {
                digest: layer.digest.clone(),
                size: layer.size,
                media_type: layer.media_type.clone(),
            };

            let filename = Self::layer_filename(&layer_meta);
            let destination = layers_dir.join(&filename);

            let mut fetched_from_cache = false;
            if destination.exists() {
                match Self::verify_blob_digest(&destination, &layer.digest).await {
                    Ok(()) => {
                        debug!("Using verified local layer {}", layer.digest);
                        layer_metadata.push(layer_meta);
                        continue;
                    }
                    Err(err) => {
                        warn!(
                            "Discarding invalid local layer {} at {}: {}",
                            layer.digest,
                            destination.display(),
                            err
                        );
                        fs::remove_file(&destination).await.ok();
                    }
                }
            }

            if let Some(store) = &self.object_store {
                match store
                    .download_cached_layer(&resolved.repository, &layer.digest, &destination)
                    .await
                {
                    Ok(true) => match Self::verify_blob_digest(&destination, &layer.digest).await {
                        Ok(()) => {
                            debug!("Layer {} fetched from object store cache", layer.digest);
                            fetched_from_cache = true;
                        }
                        Err(err) => {
                            warn!(
                                "Discarding invalid cached layer {} at {}: {}",
                                layer.digest,
                                destination.display(),
                                err
                            );
                            fs::remove_file(&destination).await.ok();
                        }
                    },
                    Ok(false) => {}
                    Err(err) => {
                        warn!(
                            "Object store cache check failed for {}: {}",
                            layer.digest, err
                        );
                    }
                }
            }

            if !fetched_from_cache {
                self.registry
                    .download_blob_to(&resolved.repository, &layer.digest, &destination)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to download layer {} for image {}",
                            layer.digest, reference
                        )
                    })?;

                if let Some(store) = &self.object_store
                    && let Err(err) = store
                        .upload_layer(&resolved.repository, &layer.digest, &destination)
                        .await
                {
                    warn!(
                        "Failed to upload layer {} to object store: {}",
                        layer.digest, err
                    );
                }
            }

            Self::verify_blob_digest(&destination, &layer.digest).await?;
            layer_metadata.push(layer_meta);
        }

        let config_path = image_path.join("config.json");
        self.registry
            .download_config_to(
                &resolved.repository,
                &resolved.manifest.config.digest,
                &config_path,
            )
            .await
            .with_context(|| format!("Failed to download config blob for {}", reference))?;
        if let Err(err) =
            Self::verify_blob_digest(&config_path, &resolved.manifest.config.digest).await
        {
            warn!(
                "Discarding invalid cached config {} at {}: {}",
                resolved.manifest.config.digest,
                config_path.display(),
                err
            );
            fs::remove_file(&config_path).await.ok();
            self.registry
                .download_blob_to(
                    &resolved.repository,
                    &resolved.manifest.config.digest,
                    &config_path,
                )
                .await
                .with_context(|| {
                    format!(
                        "Failed to redownload config blob {} for {}",
                        resolved.manifest.config.digest, reference
                    )
                })?;
            Self::verify_blob_digest(&config_path, &resolved.manifest.config.digest).await?;
        }

        let config_bytes = fs::read(&config_path)
            .await
            .context("Failed to read downloaded image config")?;
        let (image_config, created_at) = Self::parse_image_config(&config_bytes)?;

        let manifest_path = image_path.join("manifest.json");
        let manifest_json = serde_json::to_vec_pretty(&resolved.manifest)
            .context("Failed to serialize manifest for persistence")?;
        fs::write(&manifest_path, manifest_json)
            .await
            .context("Failed to persist manifest metadata")?;

        let total_size: u64 = layer_metadata.iter().map(|layer| layer.size).sum();
        let created = created_at.unwrap_or_else(Utc::now);

        let metadata = ImageMetadata {
            name: resolved.repository.clone(),
            tag: resolved.reference.clone(),
            reference: Some(reference.clone()),
            digest: resolved
                .registry_digest
                .clone()
                .unwrap_or_else(|| resolved.manifest.config.digest.clone()),
            size: total_size,
            created,
            layers: layer_metadata.clone(),
            config: image_config,
            config_digest: Some(resolved.manifest.config.digest.clone()),
        };

        self.persist_image_metadata(&reference, &metadata).await?;
        self.images.insert(reference.clone(), metadata.clone());

        info!("✅ Image pulled successfully: {}", reference);
        Ok(metadata)
    }

    pub async fn push_image(&self, image: &str) -> Result<()> {
        let (source_reference, metadata, target_repository, target_reference) =
            self.resolve_push_source(image)?;
        let image_path = self.get_image_path(&source_reference);

        let manifest_path = image_path.join("manifest.json");
        let manifest_bytes = fs::read(&manifest_path).await.with_context(|| {
            format!(
                "Cannot push {}; missing persisted manifest at {}",
                source_reference,
                manifest_path.display()
            )
        })?;
        let manifest: PackageManifest = serde_json::from_slice(&manifest_bytes)
            .context("Failed to parse persisted manifest")?;

        if let Some(metadata_config_digest) = metadata.config_digest.as_deref()
            && metadata_config_digest != manifest.config.digest
        {
            return Err(anyhow!(
                "Cannot push {}; metadata config digest {} does not match manifest config digest {}",
                source_reference,
                metadata_config_digest,
                manifest.config.digest
            )
            .into());
        }

        let config_digest = manifest.config.digest.as_str();
        let config_path = image_path.join("config.json");
        Self::verify_blob_digest(&config_path, config_digest)
            .await
            .with_context(|| {
                format!(
                    "Cannot push {}; config digest verification failed",
                    source_reference
                )
            })?;

        info!(
            "📤 Pushing image {} as {}:{}",
            source_reference, target_repository, target_reference
        );

        for layer in &manifest.layers {
            let layer_metadata = LayerMetadata {
                digest: layer.digest.clone(),
                size: layer.size,
                media_type: layer.media_type.clone(),
            };
            let layer_path = image_path
                .join("layers")
                .join(Self::layer_filename(&layer_metadata));
            Self::verify_blob_digest(&layer_path, &layer.digest)
                .await
                .with_context(|| {
                    format!(
                        "Cannot push {}; layer {} failed digest verification",
                        source_reference, layer.digest
                    )
                })?;
            self.registry
                .upload_blob_from_path(&target_repository, &layer.digest, &layer_path)
                .await
                .with_context(|| format!("Failed to push layer {}", layer.digest))?;
        }

        self.registry
            .upload_blob_from_path(&target_repository, config_digest, &config_path)
            .await
            .with_context(|| format!("Failed to push config blob {}", config_digest))?;

        self.registry
            .upload_manifest_bytes(
                &target_repository,
                &target_reference,
                &manifest_bytes,
                &manifest.media_type,
            )
            .await
            .with_context(|| {
                format!(
                    "Failed to push manifest for {}:{}",
                    target_repository, target_reference
                )
            })?;

        info!(
            "✅ Image pushed successfully: {}:{}",
            target_repository, target_reference
        );
        Ok(())
    }

    pub async fn build_image(&mut self, context: &str, tag: &str, build_file: &str) -> Result<()> {
        info!("🔨 Building image: {} from {}", tag, context);

        let context_path = Path::new(context);
        let build_file_path = context_path.join(build_file);

        if !build_file_path.exists() {
            return Err(anyhow!("Build file not found: {}", build_file_path.display()).into());
        }

        Err(anyhow!(
            "native image build is not implemented yet; use prebuilt OCI images or delegate builds to an external builder"
        )
        .into())
    }

    #[allow(dead_code)]
    async fn build_image_unimplemented_placeholder(
        &mut self,
        context: &str,
        tag: &str,
        build_file: &str,
    ) -> Result<()> {
        let context_path = Path::new(context);
        let build_file_path = context_path.join(build_file);
        let image_metadata = if build_file.to_lowercase().contains("boltfile")
            || build_file.ends_with(".toml")
            || build_file == "Boltfile"
        {
            // Native Boltfile (TOML) format
            info!("📜 Using native Boltfile format");

            let boltfile_content = fs::read_to_string(&build_file_path)
                .await
                .context("Failed to read Boltfile")?;

            let build_config: BoltfileBuildConfig =
                toml::from_str(&boltfile_content).context("Failed to parse Boltfile TOML")?;

            let build_context = BoltfileBuildContext {
                context_path: context_path.to_path_buf(),
                build_config,
                tag: tag.to_string(),
            };

            self.build_image_from_boltfile(build_context).await?
        } else {
            // Legacy Dockerfile format
            info!("📜 Using legacy Dockerfile format");

            let dockerfile_content = fs::read_to_string(&build_file_path)
                .await
                .context("Failed to read Dockerfile")?;

            let build_context = DockerfileBuildContext {
                context_path: context_path.to_path_buf(),
                dockerfile_content,
                tag: tag.to_string(),
            };

            self.build_image_from_dockerfile(build_context).await?
        };

        let reference = normalize_reference(tag);
        self.images.insert(reference.clone(), image_metadata);

        info!("✅ Image built successfully: {}", tag);
        Ok(())
    }

    /// Build an image from a Boltfile using native implementation
    async fn build_image_from_boltfile(
        &mut self,
        context: BoltfileBuildContext,
    ) -> Result<ImageMetadata> {
        info!("📜 Processing Boltfile...");

        let config = &context.build_config;
        let mut layers = Vec::new();
        let mut current_image_metadata = None;

        // Process base image
        if let Some(base) = &config.base {
            info!("📍 Base image: {}", base.image);
            current_image_metadata = Some(self.ensure_base_image(&base.image).await?);
        }

        // Process dependencies (package installations)
        if let Some(deps) = &config.dependencies {
            info!("📦 Installing {} dependencies", deps.packages.len());
            for package in &deps.packages {
                info!("  • Installing: {}", package);
                let layer = self.create_package_layer(&context, package).await?;
                layers.push(layer);
            }
        }

        // Process file operations
        if let Some(files) = &config.files {
            for file_op in files {
                match file_op {
                    BoltFileOperation::Copy { from, to } => {
                        info!("📎 Copy: {} -> {}", from, to);
                        let layer = self.create_copy_layer(&context, from, to).await?;
                        layers.push(layer);
                    }
                    BoltFileOperation::Add { url, to } => {
                        info!("⬇️ Download: {} -> {}", url, to);
                        let layer = self.create_download_layer(&context, url, to).await?;
                        layers.push(layer);
                    }
                }
            }
        }

        // Process run commands
        if let Some(commands) = &config.run {
            for command in commands {
                info!("🏃 Run: {}", command);
                let layer = self.create_run_layer(&context, command).await?;
                layers.push(layer);
            }
        }

        // Create final image metadata with Bolt-specific config
        let mut exposed_ports = std::collections::HashMap::new();
        if let Some(runtime) = &config.runtime
            && let Some(ports) = &runtime.expose
        {
            for port in ports {
                exposed_ports.insert(port.to_string(), serde_json::json!({}));
            }
        }

        // Use base image metadata if available for parent reference
        if let Some(ref base_metadata) = current_image_metadata {
            debug!(
                "Building on base image: {} ({})",
                base_metadata.name, base_metadata.digest
            );
        }

        let metadata = ImageMetadata {
            name: context.tag.clone(),
            tag: "latest".to_string(),
            reference: Some(format!("{}:latest", context.tag)),
            digest: format!("sha256:{}", hex::encode(rand::random::<[u8; 32]>())),
            size: layers.iter().map(|l| l.size).sum(),
            created: chrono::Utc::now(),
            layers,
            config: ImageConfig {
                env: config
                    .runtime
                    .as_ref()
                    .and_then(|r| r.env.as_ref())
                    .cloned()
                    .unwrap_or_default(),
                cmd: config
                    .runtime
                    .as_ref()
                    .and_then(|r| r.cmd.as_ref())
                    .cloned(),
                entrypoint: config
                    .runtime
                    .as_ref()
                    .and_then(|r| r.entrypoint.as_ref())
                    .cloned(),
                working_dir: config
                    .runtime
                    .as_ref()
                    .and_then(|r| r.workdir.as_ref())
                    .cloned(),
                user: config
                    .runtime
                    .as_ref()
                    .and_then(|r| r.user.as_ref())
                    .cloned(),
                exposed_ports: exposed_ports.keys().cloned().collect(),
            },
            config_digest: None,
        };

        Ok(metadata)
    }

    // Helper methods for Boltfile layer creation
    async fn create_package_layer(
        &self,
        _context: &BoltfileBuildContext,
        package: &str,
    ) -> Result<LayerMetadata> {
        info!("Creating package layer for: {}", package);
        Ok(LayerMetadata {
            digest: format!("sha256:{}", hex::encode(rand::random::<[u8; 32]>())),
            size: 50 * 1024 * 1024, // 50MB estimate
            media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
        })
    }

    async fn create_download_layer(
        &self,
        _context: &BoltfileBuildContext,
        url: &str,
        _dest: &str,
    ) -> Result<LayerMetadata> {
        info!("Creating download layer for: {}", url);
        Ok(LayerMetadata {
            digest: format!("sha256:{}", hex::encode(rand::random::<[u8; 32]>())),
            size: 10 * 1024 * 1024, // 10MB estimate
            media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
        })
    }

    async fn create_copy_layer(
        &self,
        context: &BoltfileBuildContext,
        src: &str,
        _dest: &str,
    ) -> Result<LayerMetadata> {
        let src_path = context.context_path.join(src);
        let size = if src_path.is_file() {
            fs::metadata(&src_path).await?.len()
        } else if src_path.is_dir() {
            self.calculate_directory_size(&src_path).await?
        } else {
            0
        };

        Ok(LayerMetadata {
            digest: format!("sha256:{}", hex::encode(rand::random::<[u8; 32]>())),
            size,
            media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
        })
    }

    async fn create_run_layer(
        &self,
        _context: &BoltfileBuildContext,
        command: &str,
    ) -> Result<LayerMetadata> {
        info!("Creating run layer for: {}", command);
        Ok(LayerMetadata {
            digest: format!("sha256:{}", hex::encode(rand::random::<[u8; 32]>())),
            size: 1024 * 1024, // 1MB estimate
            media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
        })
    }

    async fn calculate_directory_size(&self, dir: &Path) -> Result<u64> {
        let dir = dir.to_path_buf();
        let result = tokio::task::spawn_blocking(move || -> Result<u64> {
            Self::calculate_directory_size_sync(&dir)
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))??;
        Ok(result)
    }

    fn calculate_directory_size_sync(dir: &Path) -> Result<u64> {
        let mut total_size = 0;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_file() {
                total_size += metadata.len();
            } else if metadata.is_dir() {
                total_size += Self::calculate_directory_size_sync(&entry.path())?;
            }
        }
        Ok(total_size)
    }

    async fn ensure_base_image(&mut self, image: &str) -> Result<ImageMetadata> {
        // Try to get from local cache first
        if let Some(metadata) = self.get_cached_image_metadata(image) {
            return Ok(metadata);
        }

        // Pull the image if not found locally
        info!("Pulling base image: {}", image);
        self.pull_image(image).await
    }

    // Legacy Dockerfile support
    async fn build_image_from_dockerfile(
        &mut self,
        context: DockerfileBuildContext,
    ) -> Result<ImageMetadata> {
        info!("📜 Processing Dockerfile (legacy compatibility)...");

        let instructions = self.parse_dockerfile(&context.dockerfile_content)?;
        let mut layers = Vec::new();

        for instruction in instructions {
            match instruction {
                DockerfileInstruction::From { image } => {
                    info!("📍 FROM {}", image);
                    let _ = self.ensure_base_image(&image).await?;
                }
                DockerfileInstruction::Run { command } => {
                    info!("🏃 RUN {}", command);
                    let layer = LayerMetadata {
                        digest: format!("sha256:{}", hex::encode(rand::random::<[u8; 32]>())),
                        size: 1024 * 1024,
                        media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
                    };
                    layers.push(layer);
                }
                _ => {
                    debug!("Processed Dockerfile instruction: {:?}", instruction);
                }
            }
        }

        Ok(ImageMetadata {
            name: context.tag.clone(),
            tag: "latest".to_string(),
            reference: Some(format!("{}:latest", context.tag)),
            digest: format!("sha256:{}", hex::encode(rand::random::<[u8; 32]>())),
            size: layers.iter().map(|l| l.size).sum(),
            created: chrono::Utc::now(),
            layers,
            config: ImageConfig {
                env: vec![],
                cmd: Some(vec!["/bin/sh".to_string()]),
                entrypoint: None,
                working_dir: Some("/".to_string()),
                user: Some("root".to_string()),
                exposed_ports: vec![],
            },
            config_digest: None,
        })
    }

    fn parse_dockerfile(&self, content: &str) -> Result<Vec<DockerfileInstruction>> {
        let mut instructions = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let instruction = self.parse_dockerfile_line(line)?;
            instructions.push(instruction);
        }
        Ok(instructions)
    }

    fn parse_dockerfile_line(&self, line: &str) -> Result<DockerfileInstruction> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return Err(anyhow!("Empty instruction line").into());
        }

        let instruction = parts[0].to_uppercase();
        let args = &parts[1..];

        match instruction.as_str() {
            "FROM" => Ok(DockerfileInstruction::From {
                image: args[0].to_string(),
            }),
            "RUN" => Ok(DockerfileInstruction::Run {
                command: args.join(" "),
            }),
            "COPY" => Ok(DockerfileInstruction::Copy {
                src: args[0].to_string(),
                dest: args[1].to_string(),
            }),
            _ => Ok(DockerfileInstruction::Run {
                command: line.to_string(),
            }),
        }
    }

    pub fn get_image_path(&self, image: &str) -> PathBuf {
        let reference = normalize_reference(image);
        let safe_name = reference.replace(['/', ':'], "_");
        self.storage_root.join("images").join(safe_name)
    }

    pub fn get_container_path(&self, container_id: &str) -> PathBuf {
        self.storage_root.join("containers").join(container_id)
    }

    fn resolve_push_source(&self, image: &str) -> Result<(String, ImageMetadata, String, String)> {
        let (target_repository, target_reference) = parse_image_reference(image);
        let target_cache_reference = format!("{target_repository}:{target_reference}");

        if let Some(metadata) = self.images.get(&target_cache_reference).cloned() {
            return Ok((
                target_cache_reference,
                metadata,
                target_repository,
                target_reference,
            ));
        }

        if let Some((registry, source_repository)) = target_repository.split_once('/')
            && is_registry_prefix(registry)
        {
            let source_reference = normalize_reference(source_repository);
            if let Some(metadata) = self.images.get(&source_reference).cloned() {
                return Ok((
                    source_reference,
                    metadata,
                    target_repository,
                    target_reference,
                ));
            }
        }

        Err(anyhow!(
            "No local image metadata for {}; pull or build it before pushing",
            image
        )
        .into())
    }

    pub async fn create_container_rootfs(
        &self,
        container_id: &str,
        image: &str,
    ) -> Result<PathBuf> {
        info!("📁 Creating container rootfs: {}", container_id);

        let reference = normalize_reference(image);
        let container_path = self.get_container_path(container_id);
        let rootfs_path = container_path.join("rootfs");

        if rootfs_path.exists() {
            fs::remove_dir_all(&rootfs_path).await.with_context(|| {
                format!(
                    "Failed to clean existing rootfs at {}",
                    rootfs_path.display()
                )
            })?;
        }

        fs::create_dir_all(&rootfs_path)
            .await
            .context("Failed to create container rootfs directory")?;

        self.unpack_image_layers(&reference, &rootfs_path).await?;
        self.ensure_minimal_rootfs(&rootfs_path).await?;

        info!("✅ Container rootfs created: {}", rootfs_path.display());
        Ok(rootfs_path)
    }

    pub async fn remove_container(&self, container_id: &str) -> Result<()> {
        let container_path = self.get_container_path(container_id);

        if container_path.exists() {
            fs::remove_dir_all(&container_path)
                .await
                .context("Failed to remove container data")?;
        }

        Ok(())
    }

    async fn unpack_image_layers(&self, image: &str, rootfs_path: &Path) -> Result<()> {
        let metadata = self
            .get_cached_image_metadata(image)
            .ok_or_else(|| anyhow!("No cached metadata for image: {}", image))?;

        if metadata.layers.is_empty() {
            debug!("No layers present for {image}, skipping extraction");
            return Ok(());
        }

        let image_path = self.get_image_path(image).join("layers");
        let layer_specs: Vec<(LayerMetadata, PathBuf)> = metadata
            .layers
            .iter()
            .cloned()
            .map(|layer| {
                let path = image_path.join(Self::layer_filename(&layer));
                (layer, path)
            })
            .collect();

        let rootfs = rootfs_path.to_path_buf();
        task::spawn_blocking(move || -> Result<()> {
            for (layer, layer_path) in layer_specs {
                if !layer_path.exists() {
                    return Err(anyhow!(
                        "Layer file missing for {} (expected at {})",
                        layer.digest,
                        layer_path.display()
                    )
                    .into());
                }

                let file = stdfs::File::open(&layer_path).with_context(|| {
                    format!(
                        "Failed to open layer {} at {}",
                        layer.digest,
                        layer_path.display()
                    )
                })?;

                if layer.media_type.contains("gzip")
                    || layer_path.extension().and_then(|s| s.to_str()) == Some("gz")
                {
                    let decoder = GzDecoder::new(file);
                    Self::unpack_layer_archive(decoder, &rootfs)
                        .with_context(|| format!("Failed to extract layer {}", layer.digest))?;
                } else {
                    Self::unpack_layer_archive(file, &rootfs)
                        .with_context(|| format!("Failed to extract layer {}", layer.digest))?;
                }
            }
            Ok(())
        })
        .await
        .map_err(|e| anyhow!("Layer extraction task failed: {e}"))??;

        Ok(())
    }

    fn unpack_layer_archive<R: Read>(reader: R, rootfs: &Path) -> Result<()> {
        let mut archive = Archive::new(reader);
        archive.set_unpack_xattrs(true);
        archive.set_preserve_ownerships(Self::should_preserve_layer_ownerships());
        for entry in archive.entries().context("Failed to read layer entries")? {
            let mut entry = entry.context("Failed to read layer entry")?;
            let path = entry
                .path()
                .context("Failed to read layer entry path")?
                .into_owned();

            if Self::apply_whiteout(rootfs, &path)? {
                continue;
            }

            entry
                .unpack_in(rootfs)
                .with_context(|| format!("Failed to unpack {}", path.display()))?;
        }

        Ok(())
    }

    fn apply_whiteout(rootfs: &Path, entry_path: &Path) -> Result<bool> {
        let Some(file_name) = entry_path.file_name().and_then(|name| name.to_str()) else {
            return Ok(false);
        };

        if file_name == ".wh..wh..opq" {
            let parent = entry_path.parent().unwrap_or_else(|| Path::new(""));
            let target_dir = rootfs.join(parent);
            if target_dir.exists() {
                for entry in stdfs::read_dir(&target_dir).with_context(|| {
                    format!("Failed to read opaque directory {}", target_dir.display())
                })? {
                    let path = entry
                        .with_context(|| {
                            format!(
                                "Failed to inspect opaque directory {}",
                                target_dir.display()
                            )
                        })?
                        .path();
                    if path.is_dir() {
                        stdfs::remove_dir_all(&path).with_context(|| {
                            format!("Failed to remove opaque child {}", path.display())
                        })?;
                    } else {
                        stdfs::remove_file(&path).with_context(|| {
                            format!("Failed to remove opaque child {}", path.display())
                        })?;
                    }
                }
            }
            return Ok(true);
        }

        if let Some(removed_name) = file_name.strip_prefix(".wh.") {
            let target = entry_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(removed_name);
            let target_path = rootfs.join(target);
            if target_path.is_dir() {
                stdfs::remove_dir_all(&target_path).with_context(|| {
                    format!(
                        "Failed to remove whiteout directory {}",
                        target_path.display()
                    )
                })?;
            } else if target_path.exists() {
                stdfs::remove_file(&target_path).with_context(|| {
                    format!("Failed to remove whiteout file {}", target_path.display())
                })?;
            }
            return Ok(true);
        }

        Ok(false)
    }

    #[cfg(all(unix, feature = "oci-runtime"))]
    fn should_preserve_layer_ownerships() -> bool {
        nix::unistd::Uid::effective().is_root()
    }

    #[cfg(not(all(unix, feature = "oci-runtime")))]
    fn should_preserve_layer_ownerships() -> bool {
        false
    }

    async fn ensure_minimal_rootfs(&self, rootfs_path: &Path) -> Result<()> {
        for dir in [
            "bin", "etc", "lib", "tmp", "var", "usr", "dev", "proc", "sys",
        ] {
            fs::create_dir_all(rootfs_path.join(dir))
                .await
                .with_context(|| format!("Failed to create rootfs directory '{}'", dir))?;
        }
        Ok(())
    }

    fn layer_filename(layer: &LayerMetadata) -> String {
        let mut name = layer.digest.replace(':', "_");
        if layer.media_type.contains("gzip") {
            name.push_str(".tar.gz");
        } else {
            name.push_str(".tar");
        }
        name
    }

    async fn verify_blob_digest(path: &Path, digest: &str) -> Result<()> {
        let Some((algorithm, expected_hex)) = digest.split_once(':') else {
            return Err(anyhow!("invalid digest '{}'", digest).into());
        };

        if !algorithm.eq_ignore_ascii_case("sha256") {
            return Err(anyhow!("unsupported digest algorithm '{}'", algorithm).into());
        }

        let mut file = fs::File::open(path).await.with_context(|| {
            format!(
                "Failed to open blob for digest verification: {}",
                path.display()
            )
        })?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];

        loop {
            let read = file.read(&mut buffer).await.with_context(|| {
                format!(
                    "Failed to read blob for digest verification: {}",
                    path.display()
                )
            })?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }

        let computed = format!("{:x}", hasher.finalize());
        if computed != expected_hex {
            return Err(anyhow!(
                "digest mismatch for {} (expected {}, got sha256:{})",
                path.display(),
                digest,
                computed
            )
            .into());
        }

        Ok(())
    }

    fn parse_image_config(config_bytes: &[u8]) -> Result<(ImageConfig, Option<DateTime<Utc>>)> {
        let value: Value =
            serde_json::from_slice(config_bytes).context("Failed to parse image config JSON")?;
        let config_section = value
            .get("config")
            .or_else(|| value.get("Config"))
            .or_else(|| value.get("container_config"))
            .or_else(|| value.get("ContainerConfig"));

        let env = Self::extract_string_list(config_section, &["Env", "env"]);
        let cmd = Self::extract_optional_string_list(config_section, &["Cmd", "cmd"]);
        let entrypoint =
            Self::extract_optional_string_list(config_section, &["Entrypoint", "entrypoint"]);
        let working_dir = Self::extract_string_field(config_section, &["WorkingDir", "workingDir"]);
        let user = Self::extract_string_field(config_section, &["User", "user"]);

        let exposed_ports = config_section
            .and_then(|cfg| cfg.as_object())
            .and_then(|map| map.get("ExposedPorts").or_else(|| map.get("exposedPorts")))
            .and_then(|ports| ports.as_object())
            .map(|ports| ports.keys().cloned().collect::<Vec<String>>())
            .unwrap_or_default();

        let created = value
            .get("created")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let image_config = ImageConfig {
            env,
            cmd,
            entrypoint,
            working_dir,
            user,
            exposed_ports,
        };

        Ok((image_config, created))
    }

    fn extract_string_list(section: Option<&Value>, keys: &[&str]) -> Vec<String> {
        section
            .and_then(|cfg| cfg.as_object())
            .and_then(|map| {
                keys.iter().find_map(|key| {
                    map.get(*key).and_then(|v| v.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect::<Vec<String>>()
                    })
                })
            })
            .unwrap_or_default()
    }

    fn extract_optional_string_list(section: Option<&Value>, keys: &[&str]) -> Option<Vec<String>> {
        let values = Self::extract_string_list(section, keys);
        if values.is_empty() {
            None
        } else {
            Some(values)
        }
    }

    fn extract_string_field(section: Option<&Value>, keys: &[&str]) -> Option<String> {
        section.and_then(|cfg| cfg.as_object()).and_then(|map| {
            keys.iter().find_map(|key| {
                map.get(*key)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
        })
    }

    #[cfg(test)]
    async fn create_mock_image(&self, image: &str) -> Result<ImageMetadata> {
        let reference = normalize_reference(image);
        let image_path = self.get_image_path(&reference);
        fs::create_dir_all(&image_path).await?;

        let parts: Vec<&str> = reference.split(':').collect();
        let (name, tag) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            (reference.as_str(), "latest")
        };

        let mut layer_metadata = LayerMetadata {
            digest: "sha256:mock_layer".to_string(),
            size: 0,
            media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
        };

        let layers_dir = image_path.join("layers");
        let layer_clone = layer_metadata.clone();
        let layer_size = task::spawn_blocking(move || -> Result<u64> {
            stdfs::create_dir_all(&layers_dir).context("Failed to create mock layer directory")?;
            StorageManager::create_mock_layer(&layers_dir, &layer_clone)
        })
        .await
        .map_err(|e| anyhow!("Mock layer creation task failed: {e}"))??;

        layer_metadata.size = layer_size;

        let metadata = ImageMetadata {
            name: name.to_string(),
            tag: tag.to_string(),
            reference: Some(reference.clone()),
            digest: format!("sha256:{}", "mock_digest".repeat(8)),
            size: layer_size,
            created: Utc::now(),
            layers: vec![layer_metadata.clone()],
            config: ImageConfig {
                env: vec![
                    "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
                ],
                cmd: Some(vec!["/bin/sh".to_string()]),
                entrypoint: None,
                working_dir: Some("/".to_string()),
                user: None,
                exposed_ports: vec![],
            },
            config_digest: None,
        };

        self.persist_image_metadata(&reference, &metadata).await?;

        Ok(metadata)
    }

    #[cfg(test)]
    fn create_mock_layer(layer_dir: &Path, layer: &LayerMetadata) -> Result<u64> {
        let temp_root = Self::storage_scratch_tempdir()
            .context("Failed to allocate mock layer temp directory")?;
        let temp_path = temp_root.path();

        stdfs::create_dir_all(temp_path.join("etc"))
            .context("Failed to scaffold mock layer etc directory")?;
        stdfs::write(temp_path.join("etc/bolt-release"), b"Bolt Mock Image\n")
            .context("Failed to write bolt-release file")?;

        stdfs::create_dir_all(temp_path.join("bin"))
            .context("Failed to scaffold mock layer bin directory")?;
        let hello_path = temp_path.join("bin/hello");
        stdfs::write(&hello_path, b"#!/bin/sh\necho Bolt Mock Layer\n")
            .context("Failed to write mock hello script")?;

        #[cfg(unix)]
        {
            let mut perms = stdfs::metadata(&hello_path)
                .context("Failed to read mock script permissions")?
                .permissions();
            perms.set_mode(0o755);
            stdfs::set_permissions(&hello_path, perms)
                .context("Failed to set mock script permissions")?;
        }

        let tar_path = layer_dir.join(Self::layer_filename(layer));
        let tar_file = stdfs::File::create(&tar_path).with_context(|| {
            format!("Failed to create mock layer tar at {}", tar_path.display())
        })?;
        let encoder = GzEncoder::new(tar_file, Compression::default());
        let mut builder = Builder::new(encoder);
        builder
            .append_dir_all(".", temp_path)
            .context("Failed to append mock layer contents")?;
        builder
            .finish()
            .context("Failed to finalize mock layer tar")?;
        let encoder = builder
            .into_inner()
            .context("Failed to recover encoder for mock layer")?;
        let mut file = encoder
            .finish()
            .context("Failed to finish compressing mock layer")?;
        file.flush().context("Failed to flush mock layer to disk")?;
        let size = file
            .metadata()
            .context("Failed to stat mock layer tar")?
            .len();

        Ok(size)
    }

    #[cfg(test)]
    fn storage_scratch_tempdir() -> Result<tempfile::TempDir> {
        stdfs::create_dir_all(".scratch")
            .context("Failed to create repo-local scratch directory")?;
        Ok(tempfile::tempdir_in(".scratch")
            .context("Failed to create repo-local scratch temp directory")?)
    }

    async fn persist_image_metadata(&self, image: &str, metadata: &ImageMetadata) -> Result<()> {
        let image_path = self.get_image_path(image);
        fs::create_dir_all(&image_path)
            .await
            .context("Failed to ensure image metadata directory exists")?;

        let metadata_path = image_path.join("metadata.json");
        let metadata_json = serde_json::to_string_pretty(metadata)?;
        fs::write(metadata_path, metadata_json)
            .await
            .context("Failed to write image metadata")?;

        Ok(())
    }

    async fn hydrate_image_artifacts(
        &self,
        reference: &str,
        metadata: &mut ImageMetadata,
    ) -> Result<bool> {
        let mut updated = false;
        let image_path = self.get_image_path(reference);
        fs::create_dir_all(&image_path)
            .await
            .context("Failed to ensure image directory exists for hydration")?;

        let manifest_path = image_path.join("manifest.json");
        let mut manifest_bytes = if manifest_path.exists() {
            Some(fs::read(&manifest_path).await.with_context(|| {
                format!("Failed to read manifest at {}", manifest_path.display())
            })?)
        } else {
            None
        };

        if manifest_bytes.is_none()
            && let Some(store) = &self.object_store
        {
            match store.fetch_manifest(&metadata.name, &metadata.tag).await {
                Ok(Some(bytes)) => {
                    fs::write(&manifest_path, &bytes).await.with_context(|| {
                        format!("Failed to write manifest at {}", manifest_path.display())
                    })?;
                    manifest_bytes = Some(bytes);
                }
                Ok(None) => {}
                Err(err) => {
                    warn!(
                        "Object store manifest fetch failed for {}@{}: {}",
                        metadata.name, metadata.tag, err
                    );
                }
            }
        }

        if manifest_bytes.is_none() {
            let resolved = self
                .registry
                .resolve_manifest(reference)
                .await
                .with_context(|| format!("Failed to recover manifest for {}", reference))?;
            let bytes = serde_json::to_vec_pretty(&resolved.manifest)
                .context("Failed to serialise recovered manifest")?;
            fs::write(&manifest_path, &bytes).await.with_context(|| {
                format!("Failed to write manifest at {}", manifest_path.display())
            })?;
            manifest_bytes = Some(bytes);
        }

        let manifest_bytes = manifest_bytes
            .ok_or_else(|| anyhow!("Unable to hydrate manifest metadata for {}", reference))?;

        let manifest: PackageManifest = serde_json::from_slice(&manifest_bytes)
            .context("Failed to parse manifest while hydrating metadata")?;

        if metadata.config_digest.is_none() {
            metadata.config_digest = Some(manifest.config.digest.clone());
            updated = true;
        }

        if metadata.layers.is_empty() {
            metadata.layers = manifest
                .layers
                .iter()
                .map(|layer| LayerMetadata {
                    digest: layer.digest.clone(),
                    size: layer.size,
                    media_type: layer.media_type.clone(),
                })
                .collect();
            metadata.size = metadata.layers.iter().map(|layer| layer.size).sum();
            updated = true;
        }

        let config_digest = metadata
            .config_digest
            .clone()
            .unwrap_or_else(|| manifest.config.digest.clone());

        let config_path = image_path.join("config.json");
        if !config_path.exists() {
            let mut restored = false;
            if let Some(store) = &self.object_store {
                match store
                    .download_cached_config(&metadata.name, &config_digest, &config_path)
                    .await
                {
                    Ok(true) => {
                        match Self::verify_blob_digest(&config_path, &config_digest).await {
                            Ok(()) => {
                                restored = true;
                            }
                            Err(err) => {
                                warn!(
                                    "Discarding invalid cached config {} at {}: {}",
                                    config_digest,
                                    config_path.display(),
                                    err
                                );
                                fs::remove_file(&config_path).await.ok();
                            }
                        }
                    }
                    Ok(false) => {}
                    Err(err) => {
                        warn!(
                            "Object store config fetch failed for {}@{}: {}",
                            metadata.name, config_digest, err
                        );
                    }
                }
            }

            if !restored {
                self.registry
                    .download_blob_to(&metadata.name, &config_digest, &config_path)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to download config {} for {}",
                            config_digest, reference
                        )
                    })?;
            }

            Self::verify_blob_digest(&config_path, &config_digest)
                .await
                .with_context(|| {
                    format!(
                        "Hydrated config digest verification failed for {}",
                        reference
                    )
                })?;
        }

        if metadata.reference.is_none() {
            metadata.reference = Some(reference.to_string());
            updated = true;
        }

        Ok(updated)
    }

    async fn load_existing_images(&mut self) -> Result<()> {
        let images_dir = self.storage_root.join("images");
        if !images_dir.exists() {
            return Ok(());
        }

        let mut entries = fs::read_dir(&images_dir)
            .await
            .context("Failed to read images directory")?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .context("Failed to iterate images directory")?
        {
            if !entry.file_type().await?.is_dir() {
                continue;
            }

            let metadata_path = entry.path().join("metadata.json");
            if !metadata_path.exists() {
                continue;
            }

            match fs::read_to_string(&metadata_path).await {
                Ok(contents) => match serde_json::from_str::<ImageMetadata>(&contents) {
                    Ok(mut metadata) => {
                        let reference = metadata
                            .reference
                            .clone()
                            .unwrap_or_else(|| format!("{}:{}", metadata.name, metadata.tag));

                        debug!(
                            "Discovered cached image metadata: {} (path: {})",
                            reference,
                            metadata_path.display()
                        );

                        let mut needs_persist = metadata.reference.is_none();
                        match self
                            .hydrate_image_artifacts(&reference, &mut metadata)
                            .await
                        {
                            Ok(updated) => {
                                if updated {
                                    needs_persist = true;
                                }
                            }
                            Err(err) => {
                                warn!(
                                    "Failed to hydrate image metadata for {}: {}",
                                    reference, err
                                );
                                continue;
                            }
                        }

                        if metadata.reference.is_none() {
                            metadata.reference = Some(reference.clone());
                            needs_persist = true;
                        }

                        if needs_persist
                            && let Err(err) =
                                self.persist_image_metadata(&reference, &metadata).await
                        {
                            warn!("Failed to refresh metadata file for {}: {}", reference, err);
                        }

                        self.images.insert(reference, metadata);
                    }
                    Err(err) => {
                        warn!(
                            "Failed to parse cached image metadata at {}: {}",
                            metadata_path.display(),
                            err
                        );
                    }
                },
                Err(err) => {
                    warn!(
                        "Failed to read cached image metadata at {}: {}",
                        metadata_path.display(),
                        err
                    );
                }
            }
        }

        Ok(())
    }
}

fn image_metadata_is_protected(
    metadata: &ImageMetadata,
    protected_digests: &HashSet<String>,
) -> bool {
    protected_digests.contains(&metadata.digest)
        || metadata
            .config_digest
            .as_ref()
            .is_some_and(|digest| protected_digests.contains(digest))
        || metadata
            .layers
            .iter()
            .any(|layer| protected_digests.contains(&layer.digest))
}

pub(crate) fn normalize_reference(image: &str) -> String {
    if image.contains('@') {
        return image.to_string();
    }

    let (repository, reference) = parse_image_reference(image);
    format!("{repository}:{reference}")
}

async fn disk_usage(path: &Path) -> Result<u64> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<u64> {
        fn walk(path: &Path) -> Result<u64> {
            if !path.exists() {
                return Ok(0);
            }
            let metadata = stdfs::symlink_metadata(path)?;
            if metadata.is_file() {
                return Ok(metadata.len());
            }
            if metadata.is_dir() {
                let mut total = 0;
                for entry in stdfs::read_dir(path)? {
                    total += walk(&entry?.path())?;
                }
                return Ok(total);
            }
            Ok(0)
        }

        walk(&path)
    })
    .await
    .map_err(|err| anyhow!("Disk usage task failed: {err}"))?
}

fn parse_image_reference(image: &str) -> (String, String) {
    let last_slash = image.rfind('/');
    let last_colon = image.rfind(':');
    if let Some(colon) = last_colon
        && last_slash.is_none_or(|slash| colon > slash)
    {
        let repository = normalize_repository(&image[..colon]);
        let reference = image[colon + 1..].to_string();
        return (repository, reference);
    }

    (normalize_repository(image), "latest".to_string())
}

fn normalize_repository(repository: &str) -> String {
    if let Some((registry, name)) = repository.split_once('/')
        && is_registry_prefix(registry)
    {
        if is_docker_hub_registry(registry) && !name.contains('/') {
            return format!("{registry}/library/{name}");
        }
        return repository.to_string();
    }

    if !repository.contains('/') {
        format!("library/{repository}")
    } else {
        repository.to_string()
    }
}

fn is_registry_prefix(component: &str) -> bool {
    component == "localhost" || component.contains('.') || component.contains(':')
}

fn is_docker_hub_registry(registry: &str) -> bool {
    matches!(
        registry,
        "docker.io" | "index.docker.io" | "registry-1.docker.io"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::drift_integration::{
        BlobDescriptor, DriftRegistryClient, LayerDescriptor, PackageManifest,
    };
    use async_trait::async_trait;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    type DataStore = Arc<Mutex<HashMap<(String, String), Vec<u8>>>>;

    fn scratch_tempdir() -> tempfile::TempDir {
        stdfs::create_dir_all(".scratch").expect("create repo-local scratch directory");
        tempfile::tempdir_in(".scratch").expect("create repo-local scratch tempdir")
    }

    #[derive(Clone, Default)]
    struct TestObjectStore {
        manifests: DataStore,
        configs: DataStore,
    }

    #[async_trait]
    impl ObjectStore for TestObjectStore {
        async fn blob_exists(&self, _repository: &str, _digest: &str) -> Result<bool> {
            Ok(false)
        }

        async fn download_cached_layer(
            &self,
            _repository: &str,
            _digest: &str,
            _destination: &Path,
        ) -> Result<bool> {
            Ok(false)
        }

        async fn upload_layer(
            &self,
            _repository: &str,
            _digest: &str,
            _source: &Path,
        ) -> Result<()> {
            Ok(())
        }

        async fn download_cached_config(
            &self,
            repository: &str,
            digest: &str,
            destination: &Path,
        ) -> Result<bool> {
            let cached = {
                let guard = self.configs.lock().expect("config store poisoned");
                guard
                    .get(&(repository.to_string(), digest.to_string()))
                    .cloned()
            };

            if let Some(bytes) = cached {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).await?;
                }
                fs::write(destination, bytes).await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }

        async fn download_config(
            &self,
            repository: &str,
            digest: &str,
            destination: &Path,
        ) -> Result<()> {
            if !self
                .download_cached_config(repository, digest, destination)
                .await?
            {
                return Err(anyhow!(
                    "Test object store missing config {} for {}",
                    digest,
                    repository
                )
                .into());
            }
            Ok(())
        }

        async fn upload_config(&self, repository: &str, digest: &str, source: &Path) -> Result<()> {
            let bytes = fs::read(source).await?;
            self.configs
                .lock()
                .expect("config store poisoned")
                .insert((repository.to_string(), digest.to_string()), bytes);
            Ok(())
        }

        async fn fetch_manifest(
            &self,
            repository: &str,
            reference: &str,
        ) -> Result<Option<Vec<u8>>> {
            Ok(self
                .manifests
                .lock()
                .expect("manifest store poisoned")
                .get(&(repository.to_string(), reference.to_string()))
                .cloned())
        }

        async fn store_manifest(
            &self,
            repository: &str,
            reference: &str,
            data: &[u8],
        ) -> Result<()> {
            self.manifests
                .lock()
                .expect("manifest store poisoned")
                .insert(
                    (repository.to_string(), reference.to_string()),
                    data.to_vec(),
                );
            Ok(())
        }
    }

    #[tokio::test]
    async fn load_existing_images_hydrates_metadata_from_object_store() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let storage_root = temp_root.path().to_path_buf();
        fs::create_dir_all(storage_root.join("images")).await?;

        let object_store = Arc::new(TestObjectStore::default());
        let registry = DriftRegistryClient::new_test(Some(object_store.clone()));

        let mut manager = StorageManager {
            storage_root: storage_root.clone(),
            images: HashMap::new(),
            registry,
            object_store: Some(object_store.clone()),
        };

        let reference = "library/bolt:latest".to_string();
        let repository = "library/bolt".to_string();
        let tag = "latest".to_string();
        let config_bytes = br#"{"bolt":"config"}"#.to_vec();
        let config_digest = format!("sha256:{:x}", Sha256::digest(&config_bytes));
        let manifest = PackageManifest {
            schema_version: 2,
            media_type: "application/vnd.oci.image.manifest.v1+json".to_string(),
            config: BlobDescriptor {
                media_type: "application/vnd.oci.image.config.v1+json".to_string(),
                size: config_bytes.len() as u64,
                digest: config_digest,
            },
            layers: vec![LayerDescriptor {
                media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
                size: 2048,
                digest: "sha256:testlayer".to_string(),
                urls: None,
                annotations: None,
                gaming_assets: false,
                system_libraries: false,
                user_data: false,
                cacheable: true,
            }],
            annotations: HashMap::new(),
        };

        let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
        object_store
            .manifests
            .lock()
            .expect("manifest store poisoned")
            .insert((repository.clone(), tag.clone()), manifest_bytes.clone());

        object_store
            .configs
            .lock()
            .expect("config store poisoned")
            .insert(
                (repository.clone(), manifest.config.digest.clone()),
                config_bytes.clone(),
            );

        let metadata = ImageMetadata {
            name: repository.clone(),
            tag: tag.clone(),
            reference: None,
            digest: "sha256:imagedigest".to_string(),
            size: 0,
            created: Utc::now(),
            layers: Vec::new(),
            config: ImageConfig {
                env: vec![],
                cmd: None,
                entrypoint: None,
                working_dir: None,
                user: None,
                exposed_ports: vec![],
            },
            config_digest: None,
        };

        manager
            .persist_image_metadata(&reference, &metadata)
            .await?;

        manager.load_existing_images().await?;

        let cached = manager
            .images
            .get(&reference)
            .expect("metadata should be loaded");
        assert_eq!(cached.reference.as_deref(), Some(reference.as_str()));
        assert_eq!(
            cached.config_digest.as_deref(),
            Some(manifest.config.digest.as_str())
        );
        assert!(!cached.layers.is_empty());

        let image_path = manager.get_image_path(&reference);
        let manifest_path = image_path.join("manifest.json");
        let config_path = image_path.join("config.json");
        assert!(manifest_path.exists(), "manifest should be restored");
        assert!(config_path.exists(), "config should be restored");

        let stored_manifest = fs::read(&manifest_path).await?;
        assert_eq!(stored_manifest, manifest_bytes);

        let stored_config = fs::read(&config_path).await?;
        assert_eq!(stored_config, config_bytes);

        let metadata_path = image_path.join("metadata.json");
        let persisted: ImageMetadata =
            serde_json::from_str(&fs::read_to_string(&metadata_path).await?)?;
        assert_eq!(persisted.reference.as_deref(), Some(reference.as_str()));
        assert_eq!(
            persisted.config_digest.as_deref(),
            Some(manifest.config.digest.as_str())
        );

        Ok(())
    }

    #[test]
    fn layer_unpack_applies_file_and_opaque_whiteouts() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let rootfs = temp_root.path().join("rootfs");
        stdfs::create_dir_all(rootfs.join("etc/app"))?;
        stdfs::write(rootfs.join("etc/app/old.conf"), b"old")?;
        stdfs::write(rootfs.join("etc/app/keep.conf"), b"keep")?;
        stdfs::create_dir_all(rootfs.join("var/cache"))?;
        stdfs::write(rootfs.join("var/cache/stale"), b"stale")?;

        let mut layer_bytes = Vec::new();
        {
            let mut builder = Builder::new(&mut layer_bytes);

            let mut remove_old = Header::new_gnu();
            remove_old.set_entry_type(EntryType::Regular);
            remove_old.set_path("etc/app/.wh.old.conf")?;
            remove_old.set_mode(0o644);
            remove_old.set_size(0);
            remove_old.set_cksum();
            builder.append(&remove_old, std::io::empty())?;

            let mut opaque = Header::new_gnu();
            opaque.set_entry_type(EntryType::Regular);
            opaque.set_path("var/cache/.wh..wh..opq")?;
            opaque.set_mode(0o644);
            opaque.set_size(0);
            opaque.set_cksum();
            builder.append(&opaque, std::io::empty())?;

            let fresh = b"fresh";
            let mut fresh_header = Header::new_gnu();
            fresh_header.set_entry_type(EntryType::Regular);
            fresh_header.set_path("var/cache/fresh")?;
            fresh_header.set_mode(0o644);
            fresh_header.set_size(fresh.len() as u64);
            fresh_header.set_cksum();
            builder.append(&fresh_header, fresh.as_slice())?;

            builder.finish()?;
        }

        StorageManager::unpack_layer_archive(layer_bytes.as_slice(), &rootfs)?;

        assert!(!rootfs.join("etc/app/old.conf").exists());
        assert!(rootfs.join("etc/app/keep.conf").exists());
        assert!(!rootfs.join("var/cache/stale").exists());
        assert!(rootfs.join("var/cache/fresh").exists());

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn layer_unpack_preserves_links_and_modes() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let rootfs = temp_root.path().join("rootfs");
        stdfs::create_dir_all(&rootfs)?;

        let mut layer_bytes = Vec::new();
        {
            let mut builder = Builder::new(&mut layer_bytes);

            let mut opt_header = Header::new_gnu();
            opt_header.set_entry_type(EntryType::Directory);
            opt_header.set_path("opt")?;
            opt_header.set_mode(0o755);
            opt_header.set_size(0);
            opt_header.set_cksum();
            builder.append(&opt_header, std::io::empty())?;

            let mut bin_header = Header::new_gnu();
            bin_header.set_entry_type(EntryType::Directory);
            bin_header.set_path("opt/bin")?;
            bin_header.set_mode(0o750);
            bin_header.set_size(0);
            bin_header.set_cksum();
            builder.append(&bin_header, std::io::empty())?;

            let script = b"#!/bin/sh\nexit 0\n";
            let mut script_header = Header::new_gnu();
            script_header.set_entry_type(EntryType::Regular);
            script_header.set_path("opt/bin/run")?;
            script_header.set_mode(0o755);
            script_header.set_size(script.len() as u64);
            script_header.set_cksum();
            builder.append(&script_header, script.as_slice())?;

            let mut usr_header = Header::new_gnu();
            usr_header.set_entry_type(EntryType::Directory);
            usr_header.set_path("usr/bin")?;
            usr_header.set_mode(0o755);
            usr_header.set_size(0);
            usr_header.set_cksum();
            builder.append(&usr_header, std::io::empty())?;

            let mut symlink_header = Header::new_gnu();
            symlink_header.set_entry_type(EntryType::Symlink);
            symlink_header.set_path("usr/bin/run-link")?;
            symlink_header.set_link_name("../../opt/bin/run")?;
            symlink_header.set_mode(0o777);
            symlink_header.set_size(0);
            symlink_header.set_cksum();
            builder.append(&symlink_header, std::io::empty())?;

            let mut hardlink_header = Header::new_gnu();
            hardlink_header.set_entry_type(EntryType::Link);
            hardlink_header.set_path("opt/bin/run-hard")?;
            hardlink_header.set_link_name("opt/bin/run")?;
            hardlink_header.set_mode(0o755);
            hardlink_header.set_size(0);
            hardlink_header.set_cksum();
            builder.append(&hardlink_header, std::io::empty())?;

            builder.finish()?;
        }

        StorageManager::unpack_layer_archive(layer_bytes.as_slice(), &rootfs)?;

        let bin_mode = stdfs::metadata(rootfs.join("opt/bin"))?
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(bin_mode, 0o750);

        let run_mode = stdfs::metadata(rootfs.join("opt/bin/run"))?
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(run_mode, 0o755);

        assert_eq!(
            stdfs::read_link(rootfs.join("usr/bin/run-link"))?,
            PathBuf::from("../../opt/bin/run")
        );

        let run_meta = stdfs::metadata(rootfs.join("opt/bin/run"))?;
        let hard_meta = stdfs::metadata(rootfs.join("opt/bin/run-hard"))?;
        assert_eq!(run_meta.ino(), hard_meta.ino());
        assert!(run_meta.nlink() >= 2);

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn layer_unpack_preserves_pax_xattrs_when_supported() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let probe = temp_root.path().join("xattr-probe");
        stdfs::write(&probe, b"probe")?;
        if xattr::set(&probe, "user.bolt.probe", b"ok").is_err() {
            return Ok(());
        }

        let rootfs = temp_root.path().join("rootfs");
        stdfs::create_dir_all(&rootfs)?;

        let mut layer_bytes = Vec::new();
        {
            let mut builder = Builder::new(&mut layer_bytes);

            let xattr_record = pax_record("SCHILY.xattr.user.bolt.phase", "phase-b");
            let mut pax_header = Header::new_ustar();
            pax_header.set_entry_type(EntryType::XHeader);
            pax_header.set_path("PaxHeaders.0/opt/data")?;
            pax_header.set_mode(0o644);
            pax_header.set_size(xattr_record.len() as u64);
            pax_header.set_cksum();
            builder.append(&pax_header, xattr_record.as_slice())?;

            let data = b"with-xattr";
            let mut data_header = Header::new_ustar();
            data_header.set_entry_type(EntryType::Regular);
            data_header.set_path("opt/data")?;
            data_header.set_mode(0o644);
            data_header.set_size(data.len() as u64);
            data_header.set_cksum();
            builder.append(&data_header, data.as_slice())?;

            builder.finish()?;
        }

        StorageManager::unpack_layer_archive(layer_bytes.as_slice(), &rootfs)?;

        assert_eq!(
            xattr::get(rootfs.join("opt/data"), "user.bolt.phase")?,
            Some(b"phase-b".to_vec())
        );

        Ok(())
    }

    #[cfg(all(unix, feature = "oci-runtime"))]
    #[test]
    fn layer_unpack_preserves_owners_only_when_effective_root() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let rootfs = temp_root.path().join("rootfs");
        stdfs::create_dir_all(&rootfs)?;

        let current_uid = nix::unistd::getuid().as_raw();
        let current_gid = nix::unistd::getgid().as_raw();
        let preserve_owners = StorageManager::should_preserve_layer_ownerships();
        let archive_uid = if preserve_owners {
            123
        } else {
            current_uid + 1
        };
        let archive_gid = if preserve_owners {
            124
        } else {
            current_gid + 1
        };

        let mut layer_bytes = Vec::new();
        {
            let mut builder = Builder::new(&mut layer_bytes);
            let data = b"owned";
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Regular);
            header.set_path("owned-file")?;
            header.set_mode(0o644);
            header.set_uid(archive_uid.into());
            header.set_gid(archive_gid.into());
            header.set_size(data.len() as u64);
            header.set_cksum();
            builder.append(&header, data.as_slice())?;
            builder.finish()?;
        }

        StorageManager::unpack_layer_archive(layer_bytes.as_slice(), &rootfs)?;

        let metadata = stdfs::metadata(rootfs.join("owned-file"))?;
        if preserve_owners {
            assert_eq!(metadata.uid(), archive_uid);
            assert_eq!(metadata.gid(), archive_gid);
        } else {
            assert_eq!(metadata.uid(), current_uid);
            assert_eq!(metadata.gid(), current_gid);
        }

        Ok(())
    }

    #[cfg(unix)]
    fn pax_record(key: &str, value: &str) -> Vec<u8> {
        let body = format!("{key}={value}\n");
        let mut len = body.len() + 3;
        loop {
            let record = format!("{len} {body}");
            if record.len() == len {
                return record.into_bytes();
            }
            len = record.len();
        }
    }

    #[tokio::test]
    async fn verify_blob_digest_accepts_matching_sha256() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let blob_path = temp_root.path().join("blob");
        fs::write(&blob_path, b"verified").await?;

        let digest = format!("sha256:{:x}", Sha256::digest(b"verified"));
        StorageManager::verify_blob_digest(&blob_path, &digest).await?;

        Ok(())
    }

    #[tokio::test]
    async fn verify_blob_digest_rejects_mismatch() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let blob_path = temp_root.path().join("blob");
        fs::write(&blob_path, b"corrupt").await?;

        let digest = format!("sha256:{:x}", Sha256::digest(b"expected"));
        let err = StorageManager::verify_blob_digest(&blob_path, &digest)
            .await
            .expect_err("mismatched digest should fail");

        assert!(err.to_string().contains("digest mismatch"));
        Ok(())
    }

    #[test]
    fn image_reference_parser_handles_local_registry_ports() {
        assert_eq!(
            parse_image_reference("localhost:5000/bolt/app"),
            ("localhost:5000/bolt/app".to_string(), "latest".to_string())
        );
        assert_eq!(
            normalize_reference("localhost:5000/bolt/app"),
            "localhost:5000/bolt/app:latest"
        );
    }

    #[test]
    fn resolve_push_source_can_target_registry_from_unqualified_local_image() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let mut manager = StorageManager {
            storage_root: temp_root.path().to_path_buf(),
            images: HashMap::new(),
            registry: DriftRegistryClient::new_test(None),
            object_store: None,
        };
        let metadata = ImageMetadata {
            name: "library/alpine".to_string(),
            tag: "latest".to_string(),
            reference: Some("library/alpine:latest".to_string()),
            digest: "sha256:metadata".to_string(),
            size: 0,
            created: Utc::now(),
            layers: vec![],
            config: ImageConfig {
                env: vec![],
                cmd: None,
                entrypoint: None,
                working_dir: None,
                user: None,
                exposed_ports: vec![],
            },
            config_digest: Some(format!("sha256:{:x}", Sha256::digest(b"config"))),
        };
        manager
            .images
            .insert("library/alpine:latest".to_string(), metadata);

        let (source, _, target_repo, target_ref) =
            manager.resolve_push_source("localhost:5000/alpine:latest")?;

        assert_eq!(source, "library/alpine:latest");
        assert_eq!(target_repo, "localhost:5000/alpine");
        assert_eq!(target_ref, "latest");
        Ok(())
    }

    #[tokio::test]
    async fn push_rejects_metadata_manifest_config_digest_mismatch() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let mut manager = StorageManager {
            storage_root: temp_root.path().to_path_buf(),
            images: HashMap::new(),
            registry: DriftRegistryClient::new_test(None),
            object_store: None,
        };

        let metadata_config_digest = format!("sha256:{:x}", Sha256::digest(b"metadata-config"));
        let manifest_config_digest = format!("sha256:{:x}", Sha256::digest(b"manifest-config"));
        let reference = "library/alpine:latest";
        let metadata = ImageMetadata {
            name: "library/alpine".to_string(),
            tag: "latest".to_string(),
            reference: Some(reference.to_string()),
            digest: "sha256:metadata".to_string(),
            size: 0,
            created: Utc::now(),
            layers: vec![],
            config: ImageConfig {
                env: vec![],
                cmd: None,
                entrypoint: None,
                working_dir: None,
                user: None,
                exposed_ports: vec![],
            },
            config_digest: Some(metadata_config_digest.clone()),
        };
        manager.images.insert(reference.to_string(), metadata);

        let image_path = manager.get_image_path(reference);
        fs::create_dir_all(&image_path).await?;
        let manifest = PackageManifest {
            schema_version: 2,
            media_type: "application/vnd.oci.image.manifest.v1+json".to_string(),
            config: BlobDescriptor {
                media_type: "application/vnd.oci.image.config.v1+json".to_string(),
                size: 15,
                digest: manifest_config_digest.clone(),
            },
            layers: vec![],
            annotations: HashMap::new(),
        };
        fs::write(
            image_path.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )
        .await?;

        let err = manager
            .push_image("alpine:latest")
            .await
            .expect_err("mismatched config digest should fail");

        assert!(err.to_string().contains("metadata config digest"));
        assert!(err.to_string().contains(&metadata_config_digest));
        assert!(err.to_string().contains(&manifest_config_digest));
        Ok(())
    }

    #[tokio::test]
    async fn prune_images_respects_protected_references_and_dry_run() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let mut manager = StorageManager {
            storage_root: temp_root.path().to_path_buf(),
            images: HashMap::new(),
            registry: DriftRegistryClient::new_test(None),
            object_store: None,
        };

        let protected = "library/protected:latest".to_string();
        let unused = "library/unused:latest".to_string();
        for reference in [&protected, &unused] {
            let image_path = manager.get_image_path(reference);
            fs::create_dir_all(&image_path).await?;
            fs::write(image_path.join("metadata.json"), b"metadata").await?;
            manager.images.insert(
                reference.clone(),
                ImageMetadata {
                    name: reference.trim_end_matches(":latest").to_string(),
                    tag: "latest".to_string(),
                    reference: Some(reference.clone()),
                    digest: format!("sha256:{:x}", Sha256::digest(reference.as_bytes())),
                    size: 8,
                    created: Utc::now(),
                    layers: vec![],
                    config: ImageConfig {
                        env: vec![],
                        cmd: None,
                        entrypoint: None,
                        working_dir: None,
                        user: None,
                        exposed_ports: vec![],
                    },
                    config_digest: None,
                },
            );
        }

        let protected_refs = HashSet::from([protected.clone()]);
        let dry_run = manager
            .prune_images(&protected_refs, &HashSet::new(), &HashSet::new(), true)
            .await?;
        assert_eq!(dry_run.candidates.len(), 1);
        assert_eq!(dry_run.candidates[0].reference, unused);
        assert!(manager.get_image_path(&unused).exists());

        let removed = manager
            .prune_images(&protected_refs, &HashSet::new(), &HashSet::new(), false)
            .await?;
        assert_eq!(removed.candidates.len(), 1);
        assert!(manager.get_image_path(&protected).exists());
        assert!(!manager.get_image_path(&unused).exists());
        assert!(manager.images.contains_key(&protected));
        assert!(!manager.images.contains_key(&unused));

        Ok(())
    }

    #[tokio::test]
    async fn prune_images_removes_stale_image_directories_without_metadata() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let mut manager = StorageManager {
            storage_root: temp_root.path().to_path_buf(),
            images: HashMap::new(),
            registry: DriftRegistryClient::new_test(None),
            object_store: None,
        };

        let stale = temp_root.path().join("images").join("orphaned_bundle");
        fs::create_dir_all(&stale).await?;
        fs::write(stale.join("layer.blob"), b"orphaned").await?;

        let dry_run = manager
            .prune_images(&HashSet::new(), &HashSet::new(), &HashSet::new(), true)
            .await?;
        assert_eq!(dry_run.candidates.len(), 1);
        assert_eq!(
            dry_run.candidates[0].reference,
            "stale-image-dir:orphaned_bundle"
        );
        assert!(stale.exists());

        let removed = manager
            .prune_images(&HashSet::new(), &HashSet::new(), &HashSet::new(), false)
            .await?;
        assert_eq!(removed.candidates.len(), 1);
        assert!(!stale.exists());
        Ok(())
    }

    #[tokio::test]
    async fn prune_images_reports_stale_container_bundles_but_protects_live_ids()
    -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let mut manager = StorageManager {
            storage_root: temp_root.path().to_path_buf(),
            images: HashMap::new(),
            registry: DriftRegistryClient::new_test(None),
            object_store: None,
        };

        let stale = temp_root.path().join("containers").join("stale-container");
        let live = temp_root.path().join("containers").join("live-container");
        fs::create_dir_all(&stale).await?;
        fs::create_dir_all(&live).await?;
        fs::write(stale.join("state.json"), b"stale").await?;
        fs::write(live.join("state.json"), b"live").await?;

        let protected = HashSet::from(["live-container".to_string()]);
        let dry_run = manager
            .prune_images(&HashSet::new(), &HashSet::new(), &protected, true)
            .await?;
        assert_eq!(dry_run.roots.len(), 1);
        assert_eq!(dry_run.roots[0].id, "stale-container");
        assert!(stale.exists());
        assert!(live.exists());

        let removed = manager
            .prune_images(&HashSet::new(), &HashSet::new(), &protected, false)
            .await?;
        assert_eq!(removed.roots.len(), 1);
        assert!(!stale.exists());
        assert!(live.exists());
        Ok(())
    }

    #[tokio::test]
    async fn native_build_rejects_placeholder_builds() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let context = temp_root.path().join("context");
        let storage = temp_root.path().join("storage");
        fs::create_dir_all(&context).await?;
        fs::write(context.join("Dockerfile"), b"FROM scratch\n").await?;

        let mut manager = StorageManager {
            storage_root: storage,
            images: HashMap::new(),
            registry: DriftRegistryClient::new_test(None),
            object_store: None,
        };

        let err = manager
            .build_image(
                context.to_str().expect("utf8 context path"),
                "example:latest",
                "Dockerfile",
            )
            .await
            .expect_err("native build should be explicitly unsupported");

        assert!(
            err.to_string()
                .contains("native image build is not implemented")
        );
        assert!(manager.images.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn missing_layer_file_fails_rootfs_creation() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let storage_root = temp_root.path().to_path_buf();
        let mut manager = StorageManager {
            storage_root,
            images: HashMap::new(),
            registry: DriftRegistryClient::new_test(None),
            object_store: None,
        };

        let reference = "library/missing:latest".to_string();
        let metadata = ImageMetadata {
            name: "library/missing".to_string(),
            tag: "latest".to_string(),
            reference: Some(reference.clone()),
            digest: "sha256:missing".to_string(),
            size: 1,
            created: Utc::now(),
            layers: vec![LayerMetadata {
                digest: "sha256:missinglayer".to_string(),
                size: 1,
                media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
            }],
            config: ImageConfig {
                env: vec![],
                cmd: Some(vec!["/bin/sh".to_string()]),
                entrypoint: None,
                working_dir: Some("/".to_string()),
                user: None,
                exposed_ports: vec![],
            },
            config_digest: None,
        };
        manager.images.insert(reference.clone(), metadata);

        let result = manager
            .create_container_rootfs("bolt-missing", &reference)
            .await;

        assert!(result.is_err());
        assert!(
            result
                .expect_err("missing layer should fail")
                .to_string()
                .contains("Layer file missing")
        );

        Ok(())
    }

    #[tokio::test]
    async fn mock_image_fixture_is_test_scoped_and_unpackable() -> crate::Result<()> {
        let temp_root = scratch_tempdir();
        let mut manager = StorageManager {
            storage_root: temp_root.path().to_path_buf(),
            images: HashMap::new(),
            registry: DriftRegistryClient::new_test(None),
            object_store: None,
        };

        let metadata = manager.create_mock_image("fixture:latest").await?;
        manager
            .images
            .insert("library/fixture:latest".to_string(), metadata);

        let rootfs = manager
            .create_container_rootfs("bolt-fixture", "fixture:latest")
            .await?;

        assert!(rootfs.join("etc/bolt-release").exists());
        assert!(rootfs.join("bin/hello").exists());

        Ok(())
    }
}
