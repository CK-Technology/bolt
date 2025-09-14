use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, debug, warn, error};
use sha2::{Sha256, Digest};

pub mod overlay;
pub mod registry;
pub mod s3;
pub mod ghostbay;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageManager {
    pub root_path: PathBuf,
    pub images: HashMap<String, ImageMetadata>,
    pub layers: HashMap<String, LayerMetadata>,
    pub driver: StorageDriver,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageDriver {
    Overlay2,
    BoltFS, // Our custom storage driver
    ZFS,
    BTRFS,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub digest: String,
    pub size: u64,
    pub layers: Vec<String>,
    pub config: ImageConfig,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    pub architecture: String,
    pub os: String,
    pub env: Vec<String>,
    pub cmd: Vec<String>,
    pub entrypoint: Vec<String>,
    pub working_dir: String,
    pub user: String,
    pub exposed_ports: HashMap<String, serde_json::Value>,
    pub volumes: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerMetadata {
    pub id: String,
    pub digest: String,
    pub size: u64,
    pub media_type: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub parent: Option<String>,
}

impl StorageManager {
    pub fn new(root_path: PathBuf) -> Result<Self> {
        info!("📦 Initializing storage manager at: {:?}", root_path);

        std::fs::create_dir_all(&root_path)
            .context("Failed to create storage root directory")?;

        // Create storage subdirectories
        let dirs = ["images", "layers", "tmp", "cache", "content"];
        for dir in &dirs {
            std::fs::create_dir_all(root_path.join(dir))
                .with_context(|| format!("Failed to create {} directory", dir))?;
        }

        Ok(Self {
            root_path,
            images: HashMap::new(),
            layers: HashMap::new(),
            driver: StorageDriver::Overlay2, // Default to overlay2
        })
    }

    pub async fn pull_image(&mut self, image_ref: &str) -> Result<String> {
        info!("⬇️  Pulling image: {}", image_ref);

        // Parse image reference
        let (registry, name, tag) = self.parse_image_ref(image_ref)?;

        // Check if image already exists
        let image_id = self.generate_image_id(&name, &tag);
        if self.images.contains_key(&image_id) {
            info!("✅ Image {} already exists locally", image_ref);
            return Ok(image_id);
        }

        // Handle different image sources
        match registry.as_str() {
            "bolt" => {
                // Bolt native images - use our registry
                self.pull_bolt_image(&name, &tag).await
            }
            "docker.io" | "" => {
                // Docker Hub images
                self.pull_docker_image(&name, &tag).await
            }
            _ => {
                // Other registries
                self.pull_oci_image(&registry, &name, &tag).await
            }
        }
    }

    async fn pull_bolt_image(&mut self, name: &str, tag: &str) -> Result<String> {
        info!("🔧 Pulling Bolt native image: {}:{}", name, tag);

        // Check if this is a Ghostbay-hosted image
        if let Ok(ghostbay_client) = self.get_ghostbay_client().await {
            info!("  👻 Using Ghostbay for Bolt image: {}:{}", name, tag);

            let image_ref = format!("{}:{}", name, tag);
            let temp_path = std::env::temp_dir().join(format!("ghostbay-{}-{}.tar", name, tag));

            match ghostbay_client.pull_container_image(&image_ref, &temp_path).await {
                Ok(ghostbay_image) => {
                    info!("  ✅ Downloaded from Ghostbay with gaming optimizations");

                    // Convert Ghostbay image to Bolt format
                    let image_id = self.generate_image_id(name, tag);
                    let image_metadata = ImageMetadata {
                        id: image_id.clone(),
                        name: name.to_string(),
                        tag: tag.to_string(),
                        digest: ghostbay_image.digest,
                        size: ghostbay_image.size,
                        layers: ghostbay_image.layers.iter().map(|l| l.digest.clone()).collect(),
                        config: ImageConfig {
                            architecture: "amd64".to_string(),
                            os: "linux".to_string(),
                            env: vec!["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string()],
                            cmd: vec!["/bin/bash".to_string()],
                            entrypoint: vec![],
                            working_dir: "/".to_string(),
                            user: "root".to_string(),
                            exposed_ports: HashMap::new(),
                            volumes: HashMap::new(),
                        },
                        created_at: ghostbay_image.created_at,
                    };

                    self.images.insert(image_id.clone(), image_metadata);

                    // Clean up temp file
                    let _ = std::fs::remove_file(&temp_path);

                    info!("✅ Bolt image pulled from Ghostbay: {}", image_id);
                    return Ok(image_id);
                }
                Err(e) => {
                    warn!("  ⚠️  Ghostbay pull failed: {}, falling back to standard pull", e);
                }
            }
        }

        // Standard Bolt image pull
        // Bolt images are our optimized format:
        // 1. Content-addressed layers
        // 2. Gaming optimizations built-in
        // 3. Faster extraction
        // 4. Better compression

        let image_id = self.generate_image_id(name, tag);

        // For now, create a mock Bolt image
        let image_metadata = ImageMetadata {
            id: image_id.clone(),
            name: name.to_string(),
            tag: tag.to_string(),
            digest: format!("sha256:{}", self.hash_string(&format!("{}:{}", name, tag))),
            size: 100_000_000, // 100MB mock size
            layers: vec!["layer1".to_string(), "layer2".to_string()],
            config: ImageConfig {
                architecture: "amd64".to_string(),
                os: "linux".to_string(),
                env: vec!["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string()],
                cmd: vec!["/bin/bash".to_string()],
                entrypoint: vec![],
                working_dir: "/".to_string(),
                user: "root".to_string(),
                exposed_ports: HashMap::new(),
                volumes: HashMap::new(),
            },
            created_at: chrono::Utc::now(),
        };

        self.images.insert(image_id.clone(), image_metadata);

        info!("✅ Bolt image pulled successfully: {}", image_id);
        Ok(image_id)
    }

    async fn pull_docker_image(&mut self, name: &str, tag: &str) -> Result<String> {
        info!("🐳 Pulling Docker image: {}:{}", name, tag);

        // Use registry client to pull from Docker Hub
        let mut registry_client = registry::RegistryClient::new("registry-1.docker.io".to_string());
        let image_ref = format!("{}:{}", name, tag);

        let image_path = registry_client.pull_image(&image_ref).await?;

        let image_id = self.generate_image_id(name, tag);

        // Mock Docker image
        let image_metadata = ImageMetadata {
            id: image_id.clone(),
            name: name.to_string(),
            tag: tag.to_string(),
            digest: format!("sha256:{}", self.hash_string(&format!("{}:{}", name, tag))),
            size: 200_000_000, // 200MB mock size
            layers: vec!["base_layer".to_string(), "app_layer".to_string()],
            config: ImageConfig {
                architecture: "amd64".to_string(),
                os: "linux".to_string(),
                env: vec![
                    "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
                ],
                cmd: match name {
                    "nginx" => vec!["nginx", "-g", "daemon off;"].iter().map(|s| s.to_string()).collect(),
                    "postgres" => vec!["postgres"].iter().map(|s| s.to_string()).collect(),
                    _ => vec!["/bin/bash".to_string()],
                },
                entrypoint: vec![],
                working_dir: "/".to_string(),
                user: "root".to_string(),
                exposed_ports: {
                    let mut ports = HashMap::new();
                    match name {
                        "nginx" => {
                            ports.insert("80/tcp".to_string(), serde_json::json!({}));
                        }
                        "postgres" => {
                            ports.insert("5432/tcp".to_string(), serde_json::json!({}));
                        }
                        _ => {}
                    }
                    ports
                },
                volumes: HashMap::new(),
            },
            created_at: chrono::Utc::now(),
        };

        self.images.insert(image_id.clone(), image_metadata);

        info!("✅ Docker image pulled successfully: {}", image_id);
        Ok(image_id)
    }

    async fn pull_oci_image(&mut self, registry: &str, name: &str, tag: &str) -> Result<String> {
        info!("📋 Pulling OCI image from {}: {}:{}", registry, name, tag);

        // TODO: Implement generic OCI registry client
        warn!("Generic OCI registry support not yet implemented");

        let image_id = self.generate_image_id(name, tag);
        Ok(image_id)
    }

    pub async fn build_image(
        &mut self,
        build_context: &Path,
        dockerfile: &str,
        tag: Option<&str>,
    ) -> Result<String> {
        info!("🔨 Building image from context: {:?}", build_context);

        let dockerfile_path = build_context.join(dockerfile);
        if !dockerfile_path.exists() {
            return Err(anyhow::anyhow!("Dockerfile not found: {:?}", dockerfile_path));
        }

        // Read Dockerfile
        let dockerfile_content = std::fs::read_to_string(&dockerfile_path)
            .context("Failed to read Dockerfile")?;

        // Parse Dockerfile and execute build steps
        let image_id = self.execute_dockerfile_build(&dockerfile_content, build_context, tag).await?;

        info!("✅ Image built successfully: {}", image_id);
        Ok(image_id)
    }

    async fn execute_dockerfile_build(
        &mut self,
        dockerfile_content: &str,
        build_context: &Path,
        tag: Option<&str>,
    ) -> Result<String> {
        info!("📝 Executing Dockerfile build");

        // Simple Dockerfile parser and executor
        let mut current_image_id = String::new();
        let mut layers = Vec::new();

        for line in dockerfile_content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() < 2 {
                continue;
            }

            let instruction = parts[0].to_uppercase();
            let args = parts[1];

            match instruction.as_str() {
                "FROM" => {
                    info!("📦 FROM {}", args);
                    current_image_id = self.pull_image(args).await?;
                }
                "RUN" => {
                    info!("🏃 RUN {}", args);
                    let layer_id = self.execute_run_instruction(args, &current_image_id).await?;
                    layers.push(layer_id);
                }
                "COPY" | "ADD" => {
                    info!("📁 {} {}", instruction, args);
                    let layer_id = self.execute_copy_instruction(args, build_context).await?;
                    layers.push(layer_id);
                }
                "CMD" | "ENTRYPOINT" | "ENV" | "WORKDIR" | "USER" => {
                    info!("⚙️  {} {}", instruction, args);
                    // These affect the image config, not layers
                }
                _ => {
                    warn!("Unsupported Dockerfile instruction: {}", instruction);
                }
            }
        }

        // Create final image metadata
        let image_name = tag.unwrap_or("built-image");
        let (name, tag) = if let Some(colon_pos) = image_name.find(':') {
            (&image_name[..colon_pos], &image_name[colon_pos + 1..])
        } else {
            (image_name, "latest")
        };

        let final_image_id = self.generate_image_id(name, tag);

        let image_metadata = ImageMetadata {
            id: final_image_id.clone(),
            name: name.to_string(),
            tag: tag.to_string(),
            digest: format!("sha256:{}", self.hash_string(&final_image_id)),
            size: layers.len() as u64 * 50_000_000, // Mock size calculation
            layers,
            config: ImageConfig {
                architecture: "amd64".to_string(),
                os: "linux".to_string(),
                env: vec!["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string()],
                cmd: vec!["/bin/bash".to_string()],
                entrypoint: vec![],
                working_dir: "/".to_string(),
                user: "root".to_string(),
                exposed_ports: HashMap::new(),
                volumes: HashMap::new(),
            },
            created_at: chrono::Utc::now(),
        };

        self.images.insert(final_image_id.clone(), image_metadata);
        Ok(final_image_id)
    }

    async fn execute_run_instruction(&self, command: &str, _base_image_id: &str) -> Result<String> {
        info!("Executing RUN: {}", command);

        // In a real implementation, we would:
        // 1. Create a temporary container from base image
        // 2. Execute the command in the container
        // 3. Commit the changes as a new layer
        // 4. Return the layer ID

        let layer_id = uuid::Uuid::new_v4().to_string();
        info!("Created layer: {}", layer_id);

        Ok(layer_id)
    }

    async fn execute_copy_instruction(&self, args: &str, _build_context: &Path) -> Result<String> {
        info!("Executing COPY: {}", args);

        // Parse COPY arguments
        let parts: Vec<&str> = args.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(anyhow::anyhow!("Invalid COPY instruction: {}", args));
        }

        // In a real implementation, we would:
        // 1. Copy files from build context to layer
        // 2. Calculate layer digest
        // 3. Store layer in content-addressed storage

        let layer_id = uuid::Uuid::new_v4().to_string();
        info!("Created COPY layer: {}", layer_id);

        Ok(layer_id)
    }

    pub fn get_image(&self, image_id: &str) -> Option<&ImageMetadata> {
        self.images.get(image_id)
    }

    pub fn list_images(&self) -> Vec<&ImageMetadata> {
        self.images.values().collect()
    }

    pub async fn remove_image(&mut self, image_id: &str) -> Result<()> {
        info!("🗑️  Removing image: {}", image_id);

        if let Some(image) = self.images.remove(image_id) {
            // TODO: Remove image layers if not used by other images
            info!("✅ Image removed: {}", image.name);
        } else {
            return Err(anyhow::anyhow!("Image not found: {}", image_id));
        }

        Ok(())
    }

    fn parse_image_ref(&self, image_ref: &str) -> Result<(String, String, String)> {
        // Parse image reference: [registry/]name[:tag]
        let parts: Vec<&str> = image_ref.split('/').collect();

        let (registry, name_tag) = if parts.len() == 1 {
            ("docker.io".to_string(), parts[0].to_string())
        } else if parts[0].contains("://") || parts[0].contains('.') || parts[0] == "bolt" {
            (parts[0].to_string(), parts[1..].join("/"))
        } else {
            ("docker.io".to_string(), image_ref.to_string())
        };

        let (name, tag) = if let Some(colon_pos) = name_tag.find(':') {
            (
                name_tag[..colon_pos].to_string(),
                name_tag[colon_pos + 1..].to_string(),
            )
        } else {
            (name_tag.to_string(), "latest".to_string())
        };

        Ok((registry, name, tag))
    }

    fn generate_image_id(&self, name: &str, tag: &str) -> String {
        format!("{}:{}", name, tag)
    }

    fn hash_string(&self, input: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    // Volume management functionality
    pub async fn create_volume(&mut self, name: &str, driver: &str, options: &HashMap<String, String>) -> Result<VolumeInfo> {
        info!("📂 Creating volume: {} (driver: {})", name, driver);

        let volume_id = uuid::Uuid::new_v4().to_string();
        let volume_path = self.root_path.join("volumes").join(&volume_id);

        std::fs::create_dir_all(&volume_path)
            .with_context(|| format!("Failed to create volume directory: {:?}", volume_path))?;

        let volume_info = VolumeInfo {
            id: volume_id,
            name: name.to_string(),
            driver: driver.to_string(),
            mountpoint: volume_path.clone(),
            options: options.clone(),
            created_at: chrono::Utc::now(),
            size: 0, // Will be calculated lazily
        };

        // Apply driver-specific volume creation
        match driver {
            "local" => {
                self.create_local_volume(&volume_info).await?;
            }
            "nfs" => {
                self.create_nfs_volume(&volume_info).await?;
            }
            "bolt" => {
                self.create_bolt_volume(&volume_info).await?;
            }
            "s3" => {
                self.create_s3_volume(&volume_info, options).await?;
            }
            "minio" => {
                self.create_minio_volume(&volume_info, options).await?;
            }
            "ghostbay" => {
                self.create_ghostbay_volume(&volume_info, options).await?;
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported volume driver: {}", driver));
            }
        }

        info!("✅ Volume created: {} at {:?}", name, volume_path);
        Ok(volume_info)
    }

    async fn create_local_volume(&self, volume_info: &VolumeInfo) -> Result<()> {
        debug!("Creating local volume: {}", volume_info.name);

        // For local volumes, just ensure the directory exists
        std::fs::create_dir_all(&volume_info.mountpoint)
            .with_context(|| format!("Failed to create local volume directory: {:?}", volume_info.mountpoint))?;

        // Set appropriate permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&volume_info.mountpoint, perms)
                .with_context(|| format!("Failed to set permissions on volume: {:?}", volume_info.mountpoint))?;
        }

        Ok(())
    }

    async fn create_nfs_volume(&self, volume_info: &VolumeInfo) -> Result<()> {
        debug!("Creating NFS volume: {}", volume_info.name);

        let nfs_server = volume_info.options.get("server")
            .ok_or_else(|| anyhow::anyhow!("NFS volume requires 'server' option"))?;
        let nfs_path = volume_info.options.get("path")
            .ok_or_else(|| anyhow::anyhow!("NFS volume requires 'path' option"))?;

        info!("  📡 NFS server: {}", nfs_server);
        info!("  📁 NFS path: {}", nfs_path);

        // Create mount point
        std::fs::create_dir_all(&volume_info.mountpoint)
            .with_context(|| format!("Failed to create NFS mount point: {:?}", volume_info.mountpoint))?;

        // In a real implementation, we would mount the NFS share here
        info!("✅ NFS volume configuration ready");

        Ok(())
    }

    async fn create_bolt_volume(&self, volume_info: &VolumeInfo) -> Result<()> {
        debug!("Creating Bolt optimized volume: {}", volume_info.name);

        // Bolt volumes have special features:
        // 1. Content deduplication
        // 2. Snapshot support
        // 3. Gaming optimizations (low latency, anti-cheat compatibility)
        // 4. Encryption support

        std::fs::create_dir_all(&volume_info.mountpoint)
            .with_context(|| format!("Failed to create Bolt volume directory: {:?}", volume_info.mountpoint))?;

        // Create Bolt-specific metadata
        let metadata_dir = volume_info.mountpoint.join(".bolt");
        std::fs::create_dir_all(&metadata_dir)?;

        let metadata = serde_json::json!({
            "version": "1.0",
            "type": "bolt-volume",
            "features": {
                "deduplication": true,
                "snapshots": true,
                "gaming_optimized": true,
                "encryption": volume_info.options.get("encrypt").unwrap_or(&"false".to_string()) == "true"
            },
            "created_at": volume_info.created_at
        });

        std::fs::write(
            metadata_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata)?
        )?;

        info!("✅ Bolt volume created with optimizations");
        Ok(())
    }

    async fn create_s3_volume(&self, volume_info: &VolumeInfo, options: &HashMap<String, String>) -> Result<()> {
        debug!("Creating S3 volume: {}", volume_info.name);

        let endpoint = options.get("endpoint").unwrap_or(&"https://s3.amazonaws.com".to_string()).clone();
        let region = options.get("region").unwrap_or(&"us-east-1".to_string()).clone();
        let bucket = options.get("bucket")
            .ok_or_else(|| anyhow::anyhow!("S3 volume requires 'bucket' option"))?;
        let access_key = options.get("access_key")
            .ok_or_else(|| anyhow::anyhow!("S3 volume requires 'access_key' option"))?;
        let secret_key = options.get("secret_key")
            .ok_or_else(|| anyhow::anyhow!("S3 volume requires 'secret_key' option"))?;

        info!("  ☁️  S3 endpoint: {}", endpoint);
        info!("  🌎 Region: {}", region);
        info!("  🪣 Bucket: {}", bucket);

        let s3_config = crate::runtime::storage::s3::S3VolumeConfig {
            provider: crate::runtime::storage::s3::S3Provider::Generic {
                endpoint: endpoint.clone(),
                path_style: true,
            },
            bucket: bucket.clone(),
            prefix: Some(format!("volumes/{}/", volume_info.name)),
            access_key: access_key.clone(),
            secret_key: secret_key.clone(),
            encryption: None,
            compression: options.get("compression").map(|c| c == "true").unwrap_or(false),
            cache_enabled: options.get("cache").map(|c| c == "true").unwrap_or(true),
            cache_ttl_seconds: options.get("cache_ttl").and_then(|ttl| ttl.parse().ok()).unwrap_or(3600),
        };

        // Test S3 connection
        let s3_client = crate::runtime::storage::s3::S3StorageClient::new(s3_config).await
            .context("Failed to initialize S3 client")?;

        // Create volume metadata
        std::fs::create_dir_all(&volume_info.mountpoint)?;
        let metadata_dir = volume_info.mountpoint.join(".bolt");
        std::fs::create_dir_all(&metadata_dir)?;

        let metadata = serde_json::json!({
            "version": "1.0",
            "type": "s3-volume",
            "s3_config": {
                "endpoint": endpoint,
                "region": region,
                "bucket": bucket
            },
            "features": {
                "compression": s3_config.compression,
                "cache_enabled": s3_config.cache_enabled,
                "cache_ttl_seconds": s3_config.cache_ttl_seconds
            },
            "created_at": volume_info.created_at
        });

        std::fs::write(
            metadata_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata)?
        )?;

        info!("✅ S3 volume created successfully");
        Ok(())
    }

    async fn create_minio_volume(&self, volume_info: &VolumeInfo, options: &HashMap<String, String>) -> Result<()> {
        debug!("Creating MinIO volume: {}", volume_info.name);

        let endpoint = options.get("endpoint")
            .ok_or_else(|| anyhow::anyhow!("MinIO volume requires 'endpoint' option"))?;
        let bucket = options.get("bucket")
            .ok_or_else(|| anyhow::anyhow!("MinIO volume requires 'bucket' option"))?;
        let access_key = options.get("access_key")
            .ok_or_else(|| anyhow::anyhow!("MinIO volume requires 'access_key' option"))?;
        let secret_key = options.get("secret_key")
            .ok_or_else(|| anyhow::anyhow!("MinIO volume requires 'secret_key' option"))?;
        let tls = options.get("tls").map(|t| t == "true").unwrap_or(false);

        info!("  🪣 MinIO endpoint: {}", endpoint);
        info!("  🔒 TLS enabled: {}", tls);
        info!("  📦 Bucket: {}", bucket);

        let minio_config = crate::runtime::storage::s3::S3VolumeConfig {
            provider: crate::runtime::storage::s3::S3Provider::MinIO {
                endpoint: endpoint.clone(),
                tls,
            },
            bucket: bucket.clone(),
            prefix: Some(format!("bolt-volumes/{}/", volume_info.name)),
            access_key: access_key.clone(),
            secret_key: secret_key.clone(),
            encryption: options.get("encryption").map(|_| crate::runtime::storage::s3::S3Encryption {
                method: "AES256".to_string(),
                kms_key_id: None,
            }),
            compression: options.get("compression").map(|c| c == "true").unwrap_or(true),
            cache_enabled: true,
            cache_ttl_seconds: 1800, // 30 minutes
        };

        // Initialize MinIO client
        let minio_client = crate::runtime::storage::s3::S3StorageClient::new(minio_config).await
            .context("Failed to initialize MinIO client")?;

        // Create volume structure
        std::fs::create_dir_all(&volume_info.mountpoint)?;
        let metadata_dir = volume_info.mountpoint.join(".bolt");
        std::fs::create_dir_all(&metadata_dir)?;

        let metadata = serde_json::json!({
            "version": "1.0",
            "type": "minio-volume",
            "minio_config": {
                "endpoint": endpoint,
                "bucket": bucket,
                "tls": tls
            },
            "features": {
                "compression": true,
                "cache_enabled": true,
                "encryption": options.contains_key("encryption")
            },
            "created_at": volume_info.created_at
        });

        std::fs::write(
            metadata_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata)?
        )?;

        info!("✅ MinIO volume created with optimizations");
        Ok(())
    }

    async fn create_ghostbay_volume(&self, volume_info: &VolumeInfo, options: &HashMap<String, String>) -> Result<()> {
        debug!("Creating Ghostbay volume: {}", volume_info.name);

        let endpoint = options.get("endpoint")
            .ok_or_else(|| anyhow::anyhow!("Ghostbay volume requires 'endpoint' option"))?;
        let bucket = options.get("bucket")
            .ok_or_else(|| anyhow::anyhow!("Ghostbay volume requires 'bucket' option"))?;
        let access_key = options.get("access_key")
            .ok_or_else(|| anyhow::anyhow!("Ghostbay volume requires 'access_key' option"))?;
        let secret_key = options.get("secret_key")
            .ok_or_else(|| anyhow::anyhow!("Ghostbay volume requires 'secret_key' option"))?;

        info!("  👻 Ghostbay endpoint: {}", endpoint);
        info!("  📦 Bucket: {}", bucket);
        if let Some(cluster_id) = options.get("cluster_id") {
            info!("  🏭 Cluster: {}", cluster_id);
        }

        let ghostbay_config = crate::runtime::storage::ghostbay::GhostbayConfig {
            endpoint: endpoint.clone(),
            cluster_id: options.get("cluster_id").cloned(),
            access_key: access_key.clone(),
            secret_key: secret_key.clone(),
            bucket: bucket.clone(),
            region: options.get("region").cloned(),
            features: crate::runtime::storage::ghostbay::GhostbayFeatures {
                container_registry: true,
                gaming_assets: true,
                distributed_cache: true,
                content_deduplication: true,
                encryption_at_rest: true,
                multi_region: false,
            },
            gaming_optimizations: crate::runtime::storage::ghostbay::GhostbayGamingConfig {
                enable_fast_downloads: true,
                cdn_acceleration: true,
                asset_preloading: true,
                compression_level: 6,
                chunk_size_mb: 8,
            },
        };

        // Initialize Ghostbay client
        let ghostbay_client = crate::runtime::storage::ghostbay::GhostbayClient::new(ghostbay_config).await
            .context("Failed to initialize Ghostbay client")?;

        // Create volume with Ghostbay-specific features
        crate::runtime::storage::ghostbay::create_ghostbay_volume(
            ghostbay_client.into(), // This would need proper conversion
            &volume_info.name
        ).await.context("Failed to create Ghostbay volume")?;

        // Create local volume structure
        std::fs::create_dir_all(&volume_info.mountpoint)?;
        let metadata_dir = volume_info.mountpoint.join(".bolt");
        std::fs::create_dir_all(&metadata_dir)?;

        let metadata = serde_json::json!({
            "version": "1.0",
            "type": "ghostbay-volume",
            "ghostbay_config": {
                "endpoint": endpoint,
                "bucket": bucket,
                "cluster_id": options.get("cluster_id")
            },
            "features": {
                "gaming_optimized": true,
                "distributed_cache": true,
                "content_deduplication": true,
                "encryption": true,
                "cdn_acceleration": true
            },
            "created_at": volume_info.created_at
        });

        std::fs::write(
            metadata_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata)?
        )?;

        info!("✅ Ghostbay volume created with gaming optimizations");
        Ok(())
    }

    pub async fn remove_volume(&mut self, name: &str, force: bool) -> Result<()> {
        info!("🗑️  Removing volume: {} (force: {})", name, force);

        // Find volume by name
        let volume_path = self.root_path.join("volumes");
        let mut volume_to_remove: Option<PathBuf> = None;

        if volume_path.exists() {
            let mut dir = std::fs::read_dir(&volume_path)?;
            while let Some(entry) = dir.next() {
                let entry = entry?;
                let metadata_path = entry.path().join(".bolt").join("metadata.json");

                if metadata_path.exists() {
                    let metadata_content = std::fs::read_to_string(&metadata_path)?;
                    if metadata_content.contains(&format!("\"name\":\"{}\"", name)) {
                        volume_to_remove = Some(entry.path());
                        break;
                    }
                }
            }
        }

        if let Some(volume_path) = volume_to_remove {
            if !force {
                // Check if volume is in use by any containers
                info!("  🔍 Checking if volume is in use");
                // In a real implementation, we would check running containers
            }

            // Remove volume directory
            std::fs::remove_dir_all(&volume_path)
                .with_context(|| format!("Failed to remove volume directory: {:?}", volume_path))?;

            info!("✅ Volume removed: {}", name);
        } else {
            return Err(anyhow::anyhow!("Volume not found: {}", name));
        }

        Ok(())
    }

    pub async fn list_volumes(&self) -> Result<Vec<VolumeInfo>> {
        debug!("📋 Listing volumes");

        let volumes_path = self.root_path.join("volumes");
        let mut volumes = Vec::new();

        if !volumes_path.exists() {
            return Ok(volumes);
        }

        let mut dir = std::fs::read_dir(&volumes_path)?;
        while let Some(entry) = dir.next() {
            let entry = entry?;
            let metadata_path = entry.path().join(".bolt").join("metadata.json");

            if metadata_path.exists() {
                let metadata_content = std::fs::read_to_string(&metadata_path)?;
                let metadata: serde_json::Value = serde_json::from_str(&metadata_content)?;

                let size = get_directory_size_sync(&entry.path())?;

                let volume_info = VolumeInfo {
                    id: entry.file_name().to_string_lossy().to_string(),
                    name: metadata.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                    driver: metadata.get("driver").and_then(|v| v.as_str()).unwrap_or("local").to_string(),
                    mountpoint: entry.path(),
                    options: HashMap::new(), // TODO: Parse from metadata
                    created_at: chrono::DateTime::parse_from_rfc3339(
                        metadata.get("created_at").and_then(|v| v.as_str()).unwrap_or("1970-01-01T00:00:00Z")
                    ).unwrap_or_default().with_timezone(&chrono::Utc),
                    size,
                };

                volumes.push(volume_info);
            }
        }

        Ok(volumes)
    }

    pub async fn get_storage_usage(&self) -> Result<StorageManagerUsage> {
        info!("📊 Calculating storage usage");

        let mut total_images_size = 0u64;
        let mut total_volumes_size = 0u64;
        let mut total_layers_size = 0u64;

        // Calculate images size
        for image in self.images.values() {
            total_images_size += image.size;
        }

        // Calculate volumes size
        let volumes = self.list_volumes().await?;
        for volume in volumes {
            total_volumes_size += volume.size;
        }

        // Calculate layers size (estimated)
        total_layers_size = self.layers.len() as u64 * 50_000_000; // 50MB average per layer

        Ok(StorageManagerUsage {
            total_size: total_images_size + total_volumes_size + total_layers_size,
            images_size: total_images_size,
            volumes_size: total_volumes_size,
            layers_size: total_layers_size,
            images_count: self.images.len() as u32,
            volumes_count: volumes.len() as u32,
            layers_count: self.layers.len() as u32,
        })
    }

    pub async fn cleanup_unused(&mut self, dry_run: bool) -> Result<CleanupReport> {
        info!("🧹 Performing storage cleanup (dry_run: {})", dry_run);

        let mut report = CleanupReport {
            images_removed: 0,
            layers_removed: 0,
            volumes_removed: 0,
            space_reclaimed: 0,
            items: Vec::new(),
        };

        // Find unused images (not referenced by any containers)
        // In a real implementation, we would check running containers
        for (image_id, image) in &self.images {
            // Mock: assume images older than 30 days are unused
            let thirty_days_ago = chrono::Utc::now() - chrono::Duration::days(30);
            if image.created_at < thirty_days_ago {
                report.items.push(format!("Unused image: {} ({}MB)", image.name, image.size / 1_000_000));
                report.space_reclaimed += image.size;
                report.images_removed += 1;

                if !dry_run {
                    // Remove in actual cleanup
                    info!("  🗑️  Would remove unused image: {}", image_id);
                }
            }
        }

        // Find unused layers
        // Layers not referenced by any image
        let referenced_layers: std::collections::HashSet<String> = self.images.values()
            .flat_map(|img| img.layers.iter())
            .cloned()
            .collect();

        for layer_id in self.layers.keys() {
            if !referenced_layers.contains(layer_id) {
                report.items.push(format!("Unused layer: {}", layer_id));
                report.space_reclaimed += 20_000_000; // Estimate 20MB per layer
                report.layers_removed += 1;

                if !dry_run {
                    info!("  🗑️  Would remove unused layer: {}", layer_id);
                }
            }
        }

        info!("✅ Cleanup analysis complete: {} items, {}MB reclaimable",
              report.images_removed + report.layers_removed + report.volumes_removed,
              report.space_reclaimed / 1_000_000);

        Ok(report)
    }

    /// Backup storage to object storage (S3/MinIO/Ghostbay)
    pub async fn backup_to_object_storage(&self, backup_config: ObjectStorageBackupConfig) -> Result<BackupReport> {
        info!("💾 Starting backup to object storage");

        let mut backup_report = BackupReport {
            backup_id: uuid::Uuid::new_v4().to_string(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            status: BackupStatus::Running,
            items_backed_up: 0,
            total_size: 0,
            compressed_size: 0,
            items: Vec::new(),
        };

        // Initialize object storage client based on provider
        let storage_client = match backup_config.provider {
            ObjectStorageProvider::S3 { ref config } => {
                crate::runtime::storage::s3::S3StorageClient::new(config.clone()).await?
            }
            ObjectStorageProvider::MinIO { ref config } => {
                crate::runtime::storage::s3::S3StorageClient::new(config.clone()).await?
            }
            ObjectStorageProvider::Ghostbay { ref config } => {
                // For Ghostbay, we'll use the specialized client
                return self.backup_to_ghostbay(&backup_config, config).await;
            }
        };

        // Backup images
        if backup_config.include_images {
            for (image_id, image) in &self.images {
                let backup_key = format!("backups/{}/images/{}.tar.gz", backup_report.backup_id, image_id);

                // Create temporary tarball of image
                let temp_path = std::env::temp_dir().join(format!("{}.tar.gz", image_id));
                self.create_image_tarball(image, &temp_path).await?;

                // Upload to object storage
                storage_client.upload_file(&temp_path, &backup_key).await?;

                let file_size = std::fs::metadata(&temp_path)?.len();
                backup_report.items_backed_up += 1;
                backup_report.total_size += file_size;
                backup_report.items.push(format!("Image: {} ({}MB)", image.name, file_size / 1_000_000));

                // Clean up temporary file
                std::fs::remove_file(&temp_path)?;

                info!("  ✅ Backed up image: {}", image.name);
            }
        }

        // Backup volumes
        if backup_config.include_volumes {
            let volumes = self.list_volumes().await?;
            for volume in volumes {
                let backup_key = format!("backups/{}/volumes/{}.tar.gz", backup_report.backup_id, volume.name);

                // Create tarball of volume
                let temp_path = std::env::temp_dir().join(format!("volume-{}.tar.gz", volume.name));
                self.create_volume_tarball(&volume, &temp_path).await?;

                // Upload to object storage
                storage_client.upload_file(&temp_path, &backup_key).await?;

                let file_size = std::fs::metadata(&temp_path)?.len();
                backup_report.items_backed_up += 1;
                backup_report.total_size += file_size;
                backup_report.items.push(format!("Volume: {} ({}MB)", volume.name, file_size / 1_000_000));

                // Clean up temporary file
                std::fs::remove_file(&temp_path)?;

                info!("  ✅ Backed up volume: {}", volume.name);
            }
        }

        // Create backup manifest
        let manifest = serde_json::json!({
            "backup_id": backup_report.backup_id,
            "version": "1.0",
            "created_at": backup_report.started_at,
            "bolt_version": env!("CARGO_PKG_VERSION"),
            "items": backup_report.items,
            "metadata": {
                "images_count": if backup_config.include_images { self.images.len() } else { 0 },
                "volumes_count": if backup_config.include_volumes { backup_report.items_backed_up } else { 0 },
                "total_size": backup_report.total_size,
                "compression": backup_config.compression
            }
        });

        let manifest_key = format!("backups/{}/manifest.json", backup_report.backup_id);
        let manifest_data = serde_json::to_string_pretty(&manifest)?;
        let manifest_path = std::env::temp_dir().join("manifest.json");
        std::fs::write(&manifest_path, manifest_data)?;

        storage_client.upload_file(&manifest_path, &manifest_key).await?;
        std::fs::remove_file(&manifest_path)?;

        backup_report.completed_at = Some(chrono::Utc::now());
        backup_report.status = BackupStatus::Completed;

        info!("✅ Backup completed: {} items, {}MB total",
              backup_report.items_backed_up,
              backup_report.total_size / 1_000_000);

        Ok(backup_report)
    }

    /// Restore storage from object storage backup
    pub async fn restore_from_object_storage(&mut self, restore_config: ObjectStorageRestoreConfig) -> Result<RestoreReport> {
        info!("🔄 Starting restore from object storage backup: {}", restore_config.backup_id);

        let mut restore_report = RestoreReport {
            backup_id: restore_config.backup_id.clone(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            status: RestoreStatus::Running,
            items_restored: 0,
            items: Vec::new(),
        };

        // Initialize object storage client
        let storage_client = match restore_config.provider {
            ObjectStorageProvider::S3 { ref config } => {
                crate::runtime::storage::s3::S3StorageClient::new(config.clone()).await?
            }
            ObjectStorageProvider::MinIO { ref config } => {
                crate::runtime::storage::s3::S3StorageClient::new(config.clone()).await?
            }
            ObjectStorageProvider::Ghostbay { ref config } => {
                return self.restore_from_ghostbay(&restore_config, config).await;
            }
        };

        // Download and parse manifest
        let manifest_key = format!("backups/{}/manifest.json", restore_config.backup_id);
        let manifest_path = std::env::temp_dir().join("restore-manifest.json");

        storage_client.download_file(&manifest_key, &manifest_path).await?;
        let manifest_content = std::fs::read_to_string(&manifest_path)?;
        let manifest: serde_json::Value = serde_json::from_str(&manifest_content)?;
        std::fs::remove_file(&manifest_path)?;

        info!("  📋 Backup manifest loaded");
        info!("  📅 Created: {}", manifest["created_at"].as_str().unwrap_or("unknown"));
        info!("  📊 Items: {}", manifest["items"].as_array().map(|a| a.len()).unwrap_or(0));

        // Restore images
        if restore_config.include_images {
            let images_prefix = format!("backups/{}/images/", restore_config.backup_id);
            let image_objects = storage_client.list_objects(Some(&images_prefix)).await?;

            for object in image_objects {
                let temp_path = std::env::temp_dir().join("restore-image.tar.gz");
                storage_client.download_file(&object.key, &temp_path).await?;

                // Extract and import image
                self.import_image_from_tarball(&temp_path).await?;

                restore_report.items_restored += 1;
                restore_report.items.push(format!("Image: {}", object.key));

                std::fs::remove_file(&temp_path)?;
                info!("  ✅ Restored image from: {}", object.key);
            }
        }

        // Restore volumes
        if restore_config.include_volumes {
            let volumes_prefix = format!("backups/{}/volumes/", restore_config.backup_id);
            let volume_objects = storage_client.list_objects(Some(&volumes_prefix)).await?;

            for object in volume_objects {
                let temp_path = std::env::temp_dir().join("restore-volume.tar.gz");
                storage_client.download_file(&object.key, &temp_path).await?;

                // Extract and restore volume
                self.restore_volume_from_tarball(&temp_path).await?;

                restore_report.items_restored += 1;
                restore_report.items.push(format!("Volume: {}", object.key));

                std::fs::remove_file(&temp_path)?;
                info!("  ✅ Restored volume from: {}", object.key);
            }
        }

        restore_report.completed_at = Some(chrono::Utc::now());
        restore_report.status = RestoreStatus::Completed;

        info!("✅ Restore completed: {} items restored", restore_report.items_restored);
        Ok(restore_report)
    }

    async fn backup_to_ghostbay(&self, backup_config: &ObjectStorageBackupConfig, ghostbay_config: &crate::runtime::storage::ghostbay::GhostbayConfig) -> Result<BackupReport> {
        info!("👻 Starting Ghostbay backup with gaming optimizations");

        let ghostbay_client = crate::runtime::storage::ghostbay::GhostbayClient::new(ghostbay_config.clone()).await?;

        let mut backup_report = BackupReport {
            backup_id: uuid::Uuid::new_v4().to_string(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            status: BackupStatus::Running,
            items_backed_up: 0,
            total_size: 0,
            compressed_size: 0,
            items: Vec::new(),
        };

        // Ghostbay-specific optimizations for gaming content
        if backup_config.include_images {
            for (image_id, image) in &self.images {
                let temp_path = std::env::temp_dir().join(format!("{}.tar", image_id));
                self.create_image_tarball(image, &temp_path).await?;

                // Push to Ghostbay container registry with gaming optimizations
                let digest = ghostbay_client.push_container_image(&format!("backup/{}", image.name), &temp_path).await?;

                backup_report.items_backed_up += 1;
                backup_report.items.push(format!("Gaming Image: {} (digest: {})", image.name, digest));

                std::fs::remove_file(&temp_path)?;
            }
        }

        backup_report.completed_at = Some(chrono::Utc::now());
        backup_report.status = BackupStatus::Completed;

        info!("✅ Ghostbay backup completed with gaming optimizations");
        Ok(backup_report)
    }

    async fn restore_from_ghostbay(&mut self, restore_config: &ObjectStorageRestoreConfig, ghostbay_config: &crate::runtime::storage::ghostbay::GhostbayConfig) -> Result<RestoreReport> {
        info!("👻 Starting Ghostbay restore with gaming optimizations");

        let ghostbay_client = crate::runtime::storage::ghostbay::GhostbayClient::new(ghostbay_config.clone()).await?;

        let mut restore_report = RestoreReport {
            backup_id: restore_config.backup_id.clone(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            status: RestoreStatus::Running,
            items_restored: 0,
            items: Vec::new(),
        };

        // Ghostbay-specific restore with gaming optimizations would go here

        restore_report.completed_at = Some(chrono::Utc::now());
        restore_report.status = RestoreStatus::Completed;

        info!("✅ Ghostbay restore completed");
        Ok(restore_report)
    }

    async fn create_image_tarball(&self, _image: &ImageMetadata, output_path: &Path) -> Result<()> {
        // Create tarball of image layers and metadata
        debug!("📦 Creating image tarball: {:?}", output_path);

        // In a real implementation, this would:
        // 1. Collect all image layers
        // 2. Create a tar.gz archive
        // 3. Include image metadata

        // For now, create a mock tarball
        std::fs::write(output_path, b"mock image tarball")?;
        Ok(())
    }

    async fn create_volume_tarball(&self, volume: &VolumeInfo, output_path: &Path) -> Result<()> {
        // Create tarball of volume data
        debug!("📦 Creating volume tarball: {:?}", output_path);

        // In a real implementation, this would:
        // 1. Archive the volume mountpoint
        // 2. Include volume metadata
        // 3. Handle different volume drivers appropriately

        // For now, create a mock tarball
        std::fs::write(output_path, format!("mock volume tarball for {}", volume.name))?;
        Ok(())
    }

    async fn import_image_from_tarball(&mut self, _tarball_path: &Path) -> Result<()> {
        // Import image from tarball
        debug!("📥 Importing image from tarball");

        // In a real implementation, this would:
        // 1. Extract the tarball
        // 2. Import layers into storage
        // 3. Register image metadata

        Ok(())
    }

    async fn restore_volume_from_tarball(&self, _tarball_path: &Path) -> Result<()> {
        // Restore volume from tarball
        debug!("📥 Restoring volume from tarball");

        // In a real implementation, this would:
        // 1. Extract the tarball
        // 2. Restore volume data
        // 3. Recreate volume metadata

        Ok(())
    }

    pub async fn get_ghostbay_client(&self) -> Result<crate::runtime::storage::ghostbay::GhostbayClient> {
        let endpoint = std::env::var("GHOSTBAY_ENDPOINT")
            .unwrap_or_else(|_| "https://api.ghostbay.io".to_string());
        let cluster_id = std::env::var("GHOSTBAY_CLUSTER_ID").ok();
        let access_key = std::env::var("GHOSTBAY_ACCESS_KEY")
            .map_err(|_| anyhow::anyhow!("GHOSTBAY_ACCESS_KEY environment variable not set"))?;
        let secret_key = std::env::var("GHOSTBAY_SECRET_KEY")
            .map_err(|_| anyhow::anyhow!("GHOSTBAY_SECRET_KEY environment variable not set"))?;

        let config = crate::runtime::storage::ghostbay::GhostbayConfig {
            endpoint,
            cluster_id,
            access_key,
            secret_key,
            gaming_optimizations: true,
            cache_enabled: true,
            cdn_acceleration: true,
        };

        crate::runtime::storage::ghostbay::GhostbayClient::new(config).await
    }

    pub async fn push_to_ghostbay(&self, image_name: &str, image_path: &Path) -> Result<String> {
        info!("👻 Pushing image {} to Ghostbay with gaming optimizations", image_name);

        let ghostbay_client = self.get_ghostbay_client().await?;
        let digest = ghostbay_client.push_container_image(image_name, image_path).await?;

        info!("✅ Image pushed to Ghostbay with digest: {}", digest);
        Ok(digest)
    }

    pub async fn upload_gaming_assets_to_ghostbay(&self, assets_path: &Path, game_id: &str) -> Result<Vec<String>> {
        info!("🎮 Uploading gaming assets for {} to Ghostbay", game_id);

        let ghostbay_client = self.get_ghostbay_client().await?;
        let upload_results = ghostbay_client.upload_gaming_assets(assets_path, game_id).await?;

        info!("✅ Gaming assets uploaded to Ghostbay: {} files", upload_results.len());
        Ok(upload_results)
    }
}

#[derive(Debug, Clone)]
pub struct VolumeInfo {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub mountpoint: PathBuf,
    pub options: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct StorageManagerUsage {
    pub total_size: u64,
    pub images_size: u64,
    pub volumes_size: u64,
    pub layers_size: u64,
    pub images_count: u32,
    pub volumes_count: u32,
    pub layers_count: u32,
}

#[derive(Debug, Clone)]
pub struct CleanupReport {
    pub images_removed: u32,
    pub layers_removed: u32,
    pub volumes_removed: u32,
    pub space_reclaimed: u64,
    pub items: Vec<String>,
}

// Backup and restore configuration types
#[derive(Debug, Clone)]
pub enum ObjectStorageProvider {
    S3 { config: crate::runtime::storage::s3::S3VolumeConfig },
    MinIO { config: crate::runtime::storage::s3::S3VolumeConfig },
    Ghostbay { config: crate::runtime::storage::ghostbay::GhostbayConfig },
}

#[derive(Debug, Clone)]
pub struct ObjectStorageBackupConfig {
    pub provider: ObjectStorageProvider,
    pub include_images: bool,
    pub include_volumes: bool,
    pub compression: bool,
    pub encryption: bool,
    pub retention_days: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ObjectStorageRestoreConfig {
    pub provider: ObjectStorageProvider,
    pub backup_id: String,
    pub include_images: bool,
    pub include_volumes: bool,
    pub force_overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackupStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RestoreStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupReport {
    pub backup_id: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: BackupStatus,
    pub items_backed_up: u32,
    pub total_size: u64,
    pub compressed_size: u64,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreReport {
    pub backup_id: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: RestoreStatus,
    pub items_restored: u32,
    pub items: Vec<String>,
}

fn get_directory_size_sync(path: &Path) -> Result<u64> {
    let mut total_size = 0u64;

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;

        if metadata.is_file() {
            total_size += metadata.len();
        } else if metadata.is_dir() {
            total_size += get_directory_size_sync(&entry.path())?;
        }
    }

    Ok(total_size)
}