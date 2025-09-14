use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, debug, warn, error};
use sha2::{Sha256, Digest};

pub mod overlay;
pub mod registry;

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
}