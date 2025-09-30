use crate::Result;
use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tracing::{info, warn};

/// Volume management for Bolt containers
#[derive(Debug, Clone)]
pub struct VolumeManager {
    storage_root: PathBuf,
    volumes: HashMap<String, VolumeInfo>,
}

/// Volume information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub name: String,
    pub driver: String,
    pub mountpoint: PathBuf,
    pub created: chrono::DateTime<chrono::Utc>,
    pub labels: HashMap<String, String>,
    pub options: HashMap<String, String>,
    pub scope: VolumeScope,
    pub size_limit: Option<u64>,
    pub used_by: Vec<String>, // Container IDs using this volume
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VolumeScope {
    Local,
    Global,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeCreateRequest {
    pub name: String,
    pub driver: String,
    pub labels: Option<HashMap<String, String>>,
    pub options: Option<HashMap<String, String>>,
    pub size: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeCreateOptions {
    pub driver: String,
    pub size: Option<String>,
    pub labels: HashMap<String, String>,
    pub options: HashMap<String, String>,
}

impl Default for VolumeCreateOptions {
    fn default() -> Self {
        Self {
            driver: "local".to_string(),
            size: None,
            labels: HashMap::new(),
            options: HashMap::new(),
        }
    }
}

impl VolumeManager {
    /// Create a new volume manager (sync version for compatibility)
    pub fn new() -> Result<Self> {
        let storage_root = std::env::var("BOLT_STORAGE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/var/lib/bolt"));

        let volumes_dir = storage_root.join("volumes");
        std::fs::create_dir_all(&volumes_dir).context("Failed to create volumes directory")?;

        let mut manager = VolumeManager {
            storage_root,
            volumes: HashMap::new(),
        };

        // Load existing volumes synchronously
        manager.load_volumes_sync()?;
        Ok(manager)
    }

    /// Create a new volume manager (async version)
    pub async fn new_async(storage_root: PathBuf) -> Result<Self> {
        let volumes_dir = storage_root.join("volumes");
        fs::create_dir_all(&volumes_dir)
            .await
            .context("Failed to create volumes directory")?;

        let mut manager = VolumeManager {
            storage_root,
            volumes: HashMap::new(),
        };

        // Load existing volumes
        manager.load_volumes().await?;
        Ok(manager)
    }

    /// Create a volume (sync version for compatibility)
    pub fn create_volume(&mut self, name: &str, options: VolumeCreateOptions) -> Result<()> {
        info!("📦 Creating volume: {}", name);

        // Check if volume already exists
        if self.volumes.contains_key(name) {
            return Err(anyhow!("Volume '{}' already exists", name).into());
        }

        // Create volume directory
        let volume_path = self.get_volume_path(name);
        std::fs::create_dir_all(&volume_path).with_context(|| {
            format!(
                "Failed to create volume directory: {}",
                volume_path.display()
            )
        })?;

        let volume_info = VolumeInfo {
            name: name.to_string(),
            driver: options.driver,
            mountpoint: volume_path,
            created: chrono::Utc::now(),
            labels: options.labels,
            options: options.options,
            scope: VolumeScope::Local,
            size_limit: None,
            used_by: Vec::new(),
        };

        // Save volume metadata
        self.save_volume_metadata_sync(&volume_info)?;
        self.volumes.insert(name.to_string(), volume_info);

        info!("✅ Volume created: {}", name);
        Ok(())
    }

    /// List volumes (sync version for compatibility)
    pub fn list_volumes(&self) -> Vec<VolumeInfo> {
        self.volumes.values().cloned().collect()
    }

    /// Remove volume (sync version for compatibility)
    pub fn remove_volume(&mut self, name: &str, force: bool) -> Result<()> {
        info!("🗑️ Removing volume: {}", name);

        let volume = self
            .volumes
            .get(name)
            .ok_or_else(|| anyhow!("Volume '{}' not found", name))?
            .clone();

        // Check if volume is in use
        if !volume.used_by.is_empty() && !force {
            return Err(anyhow!(
                "Volume '{}' is in use by containers: {}. Use --force to remove anyway.",
                name,
                volume.used_by.join(", ")
            )
            .into());
        }

        // Remove volume directory
        std::fs::remove_dir_all(&volume.mountpoint).with_context(|| {
            format!(
                "Failed to remove volume directory: {}",
                volume.mountpoint.display()
            )
        })?;

        // Remove metadata file
        let metadata_path = self.get_volume_metadata_path(name);
        if metadata_path.exists() {
            std::fs::remove_file(&metadata_path).with_context(|| {
                format!(
                    "Failed to remove volume metadata: {}",
                    metadata_path.display()
                )
            })?;
        }

        self.volumes.remove(name);
        info!("✅ Volume removed: {}", name);
        Ok(())
    }

    /// Inspect volume (sync version for compatibility)
    pub fn inspect_volume(&self, name: &str) -> Result<VolumeInfo> {
        self.volumes
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow!("Volume '{}' not found", name).into())
    }

    /// Prune unused volumes (sync version for compatibility)
    pub fn prune_volumes(&mut self, force: bool) -> Result<Vec<String>> {
        let unused_volumes: Vec<String> = self
            .volumes
            .iter()
            .filter(|(_, v)| v.used_by.is_empty())
            .map(|(name, _)| name.clone())
            .collect();

        let mut removed = Vec::new();
        for name in unused_volumes {
            if let Err(e) = self.remove_volume(&name, force) {
                warn!("Failed to remove volume {}: {}", name, e);
            } else {
                removed.push(name);
            }
        }

        Ok(removed)
    }

    /// Create a new volume (async version with request object)
    pub async fn create_volume_async(
        &mut self,
        request: VolumeCreateRequest,
    ) -> Result<VolumeInfo> {
        info!("📦 Creating volume: {}", request.name);

        // Check if volume already exists
        if self.volumes.contains_key(&request.name) {
            return Err(anyhow!("Volume '{}' already exists", request.name).into());
        }

        // Create volume directory
        let volume_path = self.get_volume_path(&request.name);
        fs::create_dir_all(&volume_path).await.with_context(|| {
            format!(
                "Failed to create volume directory: {}",
                volume_path.display()
            )
        })?;

        let volume_info = VolumeInfo {
            name: request.name.clone(),
            driver: request.driver,
            mountpoint: volume_path,
            created: chrono::Utc::now(),
            labels: request.labels.unwrap_or_default(),
            options: request.options.unwrap_or_default(),
            scope: VolumeScope::Local,
            size_limit: None,
            used_by: Vec::new(),
        };

        // Save volume metadata
        self.save_volume_metadata(&volume_info).await?;
        self.volumes
            .insert(request.name.clone(), volume_info.clone());

        info!("✅ Volume created: {}", request.name);
        Ok(volume_info)
    }

    /// List all volumes (async version)
    pub async fn list_volumes_async(&self) -> Result<Vec<VolumeInfo>> {
        let volumes: Vec<VolumeInfo> = self.volumes.values().cloned().collect();
        Ok(volumes)
    }

    /// Remove a volume (async version)
    pub async fn remove_volume_async(&mut self, name: &str, force: bool) -> Result<()> {
        info!("🗑️ Removing volume: {}", name);

        let volume = self
            .volumes
            .get(name)
            .ok_or_else(|| anyhow!("Volume '{}' not found", name))?
            .clone();

        // Check if volume is in use
        if !volume.used_by.is_empty() && !force {
            return Err(anyhow!(
                "Volume '{}' is in use by containers: {}. Use --force to remove anyway.",
                name,
                volume.used_by.join(", ")
            )
            .into());
        }

        // Remove volume directory
        fs::remove_dir_all(&volume.mountpoint)
            .await
            .with_context(|| {
                format!(
                    "Failed to remove volume directory: {}",
                    volume.mountpoint.display()
                )
            })?;

        // Remove metadata file
        let metadata_path = self.get_volume_metadata_path(name);
        if metadata_path.exists() {
            fs::remove_file(&metadata_path).await.with_context(|| {
                format!(
                    "Failed to remove volume metadata: {}",
                    metadata_path.display()
                )
            })?;
        }

        self.volumes.remove(name);
        info!("✅ Volume removed: {}", name);
        Ok(())
    }

    /// Get volume mount path
    pub fn get_volume_mount_path(&self, volume_name: &str) -> Option<PathBuf> {
        self.volumes.get(volume_name).map(|v| v.mountpoint.clone())
    }

    // Helper methods
    fn load_volumes_sync(&mut self) -> Result<()> {
        let volumes_dir = self.storage_root.join("volumes");
        if !volumes_dir.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(&volumes_dir)?;
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let volume_name = entry.file_name().to_string_lossy().to_string();
                if let Ok(volume_info) = self.load_volume_metadata_sync(&volume_name) {
                    self.volumes.insert(volume_name, volume_info);
                }
            }
        }

        info!("📦 Loaded {} volumes", self.volumes.len());
        Ok(())
    }

    fn load_volume_metadata_sync(&self, name: &str) -> Result<VolumeInfo> {
        let metadata_path = self.get_volume_metadata_path(name);
        let content = std::fs::read_to_string(&metadata_path)?;
        let volume_info: VolumeInfo = serde_json::from_str(&content)?;
        Ok(volume_info)
    }

    fn save_volume_metadata_sync(&self, volume: &VolumeInfo) -> Result<()> {
        let metadata_path = self.get_volume_metadata_path(&volume.name);
        let content = serde_json::to_string_pretty(volume)?;
        std::fs::write(&metadata_path, content)?;
        Ok(())
    }

    async fn load_volumes(&mut self) -> Result<()> {
        let volumes_dir = self.storage_root.join("volumes");
        if !volumes_dir.exists() {
            return Ok(());
        }

        let mut dir_reader = fs::read_dir(&volumes_dir).await?;
        while let Some(entry) = dir_reader.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let volume_name = entry.file_name().to_string_lossy().to_string();
                if let Ok(volume_info) = self.load_volume_metadata(&volume_name).await {
                    self.volumes.insert(volume_name, volume_info);
                }
            }
        }

        info!("📦 Loaded {} volumes", self.volumes.len());
        Ok(())
    }

    async fn load_volume_metadata(&self, name: &str) -> Result<VolumeInfo> {
        let metadata_path = self.get_volume_metadata_path(name);
        let content = fs::read_to_string(&metadata_path).await?;
        let volume_info: VolumeInfo = serde_json::from_str(&content)?;
        Ok(volume_info)
    }

    async fn save_volume_metadata(&self, volume: &VolumeInfo) -> Result<()> {
        let metadata_path = self.get_volume_metadata_path(&volume.name);
        let content = serde_json::to_string_pretty(volume)?;
        fs::write(&metadata_path, content).await?;
        Ok(())
    }

    fn get_volume_path(&self, name: &str) -> PathBuf {
        self.storage_root.join("volumes").join(name).join("_data")
    }

    fn get_volume_metadata_path(&self, name: &str) -> PathBuf {
        self.storage_root
            .join("volumes")
            .join(name)
            .join("metadata.json")
    }
}
