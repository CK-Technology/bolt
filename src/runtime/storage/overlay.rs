use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

pub struct OverlayDriver {
    pub root_path: PathBuf,
    pub layers: HashMap<String, LayerInfo>,
}

#[derive(Debug, Clone)]
pub struct LayerInfo {
    pub id: String,
    pub diff_path: PathBuf,
    pub merged_path: PathBuf,
    pub work_path: PathBuf,
    pub lower_dirs: Vec<String>,
}

impl OverlayDriver {
    pub fn new(root_path: PathBuf) -> Result<Self> {
        info!("📂 Initializing overlay storage driver at: {:?}", root_path);

        // Create overlay directory structure
        let overlay_dirs = ["diff", "merged", "work", "link"];
        for dir in &overlay_dirs {
            let dir_path = root_path.join(dir);
            std::fs::create_dir_all(&dir_path)
                .with_context(|| format!("Failed to create overlay directory: {:?}", dir_path))?;
        }

        Ok(Self {
            root_path,
            layers: HashMap::new(),
        })
    }

    pub async fn create_layer(
        &mut self,
        layer_id: &str,
        parent_layers: &[String],
    ) -> Result<LayerInfo> {
        info!("📦 Creating overlay layer: {}", layer_id);

        let diff_path = self.root_path.join("diff").join(layer_id);
        let merged_path = self.root_path.join("merged").join(layer_id);
        let work_path = self.root_path.join("work").join(layer_id);

        // Create directories
        fs::create_dir_all(&diff_path)
            .await
            .with_context(|| format!("Failed to create diff directory: {:?}", diff_path))?;
        fs::create_dir_all(&merged_path)
            .await
            .with_context(|| format!("Failed to create merged directory: {:?}", merged_path))?;
        fs::create_dir_all(&work_path)
            .await
            .with_context(|| format!("Failed to create work directory: {:?}", work_path))?;

        let layer_info = LayerInfo {
            id: layer_id.to_string(),
            diff_path: diff_path.clone(),
            merged_path: merged_path.clone(),
            work_path: work_path.clone(),
            lower_dirs: parent_layers.to_vec(),
        };

        // Mount overlay filesystem
        self.mount_overlay(&layer_info).await?;

        self.layers.insert(layer_id.to_string(), layer_info.clone());

        debug!("✅ Overlay layer created: {}", layer_id);
        Ok(layer_info)
    }

    async fn mount_overlay(&self, layer_info: &LayerInfo) -> Result<()> {
        debug!("🔗 Mounting overlay for layer: {}", layer_info.id);

        // Build lowerdir string from parent layers
        let mut lowerdir_parts = Vec::new();
        for parent_id in &layer_info.lower_dirs {
            if let Some(parent_layer) = self.layers.get(parent_id) {
                lowerdir_parts.push(parent_layer.diff_path.to_string_lossy().to_string());
            }
        }

        let mount_options = if lowerdir_parts.is_empty() {
            // Base layer - no lowerdir
            format!(
                "upperdir={},workdir={}",
                layer_info.diff_path.display(),
                layer_info.work_path.display()
            )
        } else {
            // Layer with parents
            format!(
                "lowerdir={},upperdir={},workdir={}",
                lowerdir_parts.join(":"),
                layer_info.diff_path.display(),
                layer_info.work_path.display()
            )
        };

        debug!("Mount options: {}", mount_options);

        // In a real implementation, we would use mount(2) syscall
        // For now, we'll simulate the overlay mount
        info!("✅ Overlay mounted for layer: {}", layer_info.id);

        Ok(())
    }

    pub async fn remove_layer(&mut self, layer_id: &str) -> Result<()> {
        info!("🗑️  Removing overlay layer: {}", layer_id);

        if let Some(layer_info) = self.layers.remove(layer_id) {
            // Unmount overlay
            self.unmount_overlay(&layer_info).await?;

            // Remove directories
            if layer_info.merged_path.exists() {
                fs::remove_dir_all(&layer_info.merged_path)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to remove merged directory: {:?}",
                            layer_info.merged_path
                        )
                    })?;
            }

            if layer_info.diff_path.exists() {
                fs::remove_dir_all(&layer_info.diff_path)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to remove diff directory: {:?}",
                            layer_info.diff_path
                        )
                    })?;
            }

            if layer_info.work_path.exists() {
                fs::remove_dir_all(&layer_info.work_path)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to remove work directory: {:?}",
                            layer_info.work_path
                        )
                    })?;
            }

            info!("✅ Overlay layer removed: {}", layer_id);
        } else {
            warn!("Layer not found: {}", layer_id);
        }

        Ok(())
    }

    async fn unmount_overlay(&self, layer_info: &LayerInfo) -> Result<()> {
        debug!("🔌 Unmounting overlay for layer: {}", layer_info.id);

        // In a real implementation, we would use umount(2) syscall
        // For now, we'll simulate the overlay unmount
        debug!("✅ Overlay unmounted for layer: {}", layer_info.id);

        Ok(())
    }

    pub fn get_layer(&self, layer_id: &str) -> Option<&LayerInfo> {
        self.layers.get(layer_id)
    }

    pub fn list_layers(&self) -> Vec<&LayerInfo> {
        self.layers.values().collect()
    }

    pub async fn get_storage_usage(&self) -> Result<StorageUsage> {
        let mut total_size = 0u64;
        let mut layer_count = 0u32;

        for layer_info in self.layers.values() {
            if let Ok(size) = get_directory_size_sync(&layer_info.diff_path) {
                total_size += size;
            }
            layer_count += 1;
        }

        Ok(StorageUsage {
            total_size,
            layer_count,
            driver: "overlay2".to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct StorageUsage {
    pub total_size: u64,
    pub layer_count: u32,
    pub driver: String,
}

fn get_directory_size_sync(path: &Path) -> Result<u64> {
    let mut total_size = 0u64;

    for entry in
        std::fs::read_dir(path).with_context(|| format!("Failed to read directory: {:?}", path))?
    {
        let entry = entry?;
        let metadata = entry
            .metadata()
            .with_context(|| format!("Failed to get metadata for: {:?}", entry.path()))?;

        if metadata.is_file() {
            total_size += metadata.len();
        } else if metadata.is_dir() {
            total_size += get_directory_size_sync(&entry.path())?;
        }
    }

    Ok(total_size)
}
