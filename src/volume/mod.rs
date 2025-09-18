use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Volume management for Bolt containers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeManager {
    pub volumes_dir: PathBuf,
    pub volumes: HashMap<String, Volume>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub name: String,
    pub driver: String,
    pub mount_point: PathBuf,
    pub size_bytes: Option<u64>,
    pub created_at: SystemTime,
    pub labels: HashMap<String, String>,
    pub options: HashMap<String, String>,
    pub in_use: bool,
    pub used_by: Vec<String>, // Container IDs using this volume
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeCreateOptions {
    pub driver: String,
    pub size: Option<String>,
    pub labels: HashMap<String, String>,
    pub options: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub name: String,
    pub driver: String,
    pub mount_point: String,
    pub size: String,
    pub created: String,
    pub in_use: bool,
    pub containers: Vec<String>,
}

impl VolumeManager {
    /// Create new volume manager
    pub fn new() -> Result<Self> {
        let volumes_dir = PathBuf::from("/var/lib/bolt/volumes");

        // Create volumes directory if it doesn't exist
        if !volumes_dir.exists() {
            fs::create_dir_all(&volumes_dir)?;
            info!("📁 Created volumes directory: {:?}", volumes_dir);
        }

        let mut manager = Self {
            volumes_dir,
            volumes: HashMap::new(),
        };

        // Load existing volumes
        manager.load_volumes()?;

        Ok(manager)
    }

    /// Create a new volume
    pub fn create_volume(&mut self, name: &str, options: VolumeCreateOptions) -> Result<Volume> {
        info!("📦 Creating volume: {}", name);

        // Check if volume already exists
        if self.volumes.contains_key(name) {
            return Err(anyhow::anyhow!("Volume '{}' already exists", name));
        }

        // Parse size if provided
        let size_bytes = if let Some(ref size_str) = options.size {
            Some(self.parse_size(size_str)?)
        } else {
            None
        };

        // Create volume directory
        let volume_path = self.volumes_dir.join(name);
        if !volume_path.exists() {
            fs::create_dir_all(&volume_path)?;
            info!("  ✓ Created volume directory: {:?}", volume_path);
        }

        // Set up volume based on driver
        match options.driver.as_str() {
            "local" => {
                self.setup_local_volume(&volume_path, size_bytes)?;
            }
            "nfs" => {
                self.setup_nfs_volume(&volume_path, &options.options)?;
            }
            "overlay" => {
                self.setup_overlay_volume(&volume_path)?;
            }
            "tmpfs" => {
                self.setup_tmpfs_volume(&volume_path, size_bytes)?;
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported volume driver: {}",
                    options.driver
                ));
            }
        }

        let volume = Volume {
            name: name.to_string(),
            driver: options.driver,
            mount_point: volume_path,
            size_bytes,
            created_at: SystemTime::now(),
            labels: options.labels,
            options: options.options,
            in_use: false,
            used_by: Vec::new(),
        };

        // Save volume metadata
        self.save_volume_metadata(&volume)?;

        // Store in memory
        self.volumes.insert(name.to_string(), volume.clone());

        info!("✅ Volume '{}' created successfully", name);
        Ok(volume)
    }

    /// List all volumes
    pub fn list_volumes(&self) -> Vec<VolumeInfo> {
        self.volumes
            .values()
            .map(|vol| VolumeInfo {
                name: vol.name.clone(),
                driver: vol.driver.clone(),
                mount_point: vol.mount_point.to_string_lossy().to_string(),
                size: self.format_size(vol.size_bytes),
                created: self.format_time(vol.created_at),
                in_use: vol.in_use,
                containers: vol.used_by.clone(),
            })
            .collect()
    }

    /// Remove a volume
    pub fn remove_volume(&mut self, name: &str, force: bool) -> Result<()> {
        info!("🗑️ Removing volume: {} (force: {})", name, force);

        let volume = self
            .volumes
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Volume '{}' not found", name))?;

        // Check if volume is in use
        if volume.in_use && !force {
            return Err(anyhow::anyhow!(
                "Volume '{}' is in use by containers: {:?}. Use --force to remove anyway.",
                name,
                volume.used_by
            ));
        }

        // Unmount if necessary
        self.unmount_volume(volume)?;

        // Remove volume directory
        let volume_path = &volume.mount_point;
        if volume_path.exists() {
            fs::remove_dir_all(volume_path)?;
            info!("  ✓ Removed volume directory: {:?}", volume_path);
        }

        // Remove metadata file
        let metadata_file = self.volumes_dir.join(format!("{}.json", name));
        if metadata_file.exists() {
            fs::remove_file(metadata_file)?;
            info!("  ✓ Removed volume metadata");
        }

        // Remove from memory
        self.volumes.remove(name);

        info!("✅ Volume '{}' removed successfully", name);
        Ok(())
    }

    /// Inspect a volume
    pub fn inspect_volume(&self, name: &str) -> Result<Volume> {
        info!("🔍 Inspecting volume: {}", name);

        self.volumes
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Volume '{}' not found", name))
    }

    /// Prune unused volumes
    pub fn prune_volumes(&mut self, force: bool) -> Result<Vec<String>> {
        info!("🧹 Pruning unused volumes (force: {})", force);

        let mut removed_volumes = Vec::new();
        let unused_volumes: Vec<String> = self
            .volumes
            .iter()
            .filter(|(_, vol)| !vol.in_use)
            .map(|(name, _)| name.clone())
            .collect();

        for volume_name in unused_volumes {
            match self.remove_volume(&volume_name, force) {
                Ok(_) => {
                    removed_volumes.push(volume_name);
                }
                Err(e) => {
                    warn!("Failed to remove volume '{}': {}", volume_name, e);
                }
            }
        }

        info!("✅ Pruned {} unused volumes", removed_volumes.len());
        Ok(removed_volumes)
    }

    /// Mount volume to container
    pub fn mount_volume(&mut self, volume_name: &str, container_id: &str) -> Result<PathBuf> {
        info!(
            "🔗 Mounting volume '{}' to container '{}'",
            volume_name, container_id
        );

        let volume = self
            .volumes
            .get_mut(volume_name)
            .ok_or_else(|| anyhow::anyhow!("Volume '{}' not found", volume_name))?;

        // Add container to usage list
        if !volume.used_by.contains(&container_id.to_string()) {
            volume.used_by.push(container_id.to_string());
            volume.in_use = true;
        }

        // Save updated metadata
        let mount_point = volume.mount_point.clone();
        let volume_clone = volume.clone();
        self.save_volume_metadata(&volume_clone)?;

        info!("  ✓ Volume mounted to: {:?}", mount_point);
        Ok(mount_point)
    }

    /// Unmount volume from container
    pub fn unmount_volume_from_container(
        &mut self,
        volume_name: &str,
        container_id: &str,
    ) -> Result<()> {
        info!(
            "🔗 Unmounting volume '{}' from container '{}'",
            volume_name, container_id
        );

        let volume = self
            .volumes
            .get_mut(volume_name)
            .ok_or_else(|| anyhow::anyhow!("Volume '{}' not found", volume_name))?;

        // Remove container from usage list
        volume.used_by.retain(|id| id != container_id);
        volume.in_use = !volume.used_by.is_empty();

        // Clone what we need before calling save_volume_metadata
        let volume_clone = volume.clone();

        // Save updated metadata
        self.save_volume_metadata(&volume_clone)?;

        info!("  ✓ Volume unmounted from container");
        Ok(())
    }

    /// Setup local volume
    fn setup_local_volume(&self, path: &Path, size_bytes: Option<u64>) -> Result<()> {
        info!("  🏠 Setting up local volume");

        // Set permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(path)?;
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions)?;
        }

        // Create quota file if size specified
        if let Some(size) = size_bytes {
            let quota_file = path.join(".bolt_quota");
            fs::write(quota_file, size.to_string())?;
            info!("    ✓ Set volume quota: {} bytes", size);
        }

        info!("    ✓ Local volume ready");
        Ok(())
    }

    /// Setup NFS volume
    fn setup_nfs_volume(&self, path: &Path, options: &HashMap<String, String>) -> Result<()> {
        info!("  🌐 Setting up NFS volume");

        let server = options
            .get("server")
            .ok_or_else(|| anyhow::anyhow!("NFS server not specified"))?;
        let export = options
            .get("export")
            .ok_or_else(|| anyhow::anyhow!("NFS export not specified"))?;

        // Mount NFS share
        let mount_cmd = std::process::Command::new("mount")
            .arg("-t")
            .arg("nfs")
            .arg(format!("{}:{}", server, export))
            .arg(path)
            .output();

        match mount_cmd {
            Ok(output) => {
                if output.status.success() {
                    info!("    ✓ NFS volume mounted from {}:{}", server, export);
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(anyhow::anyhow!("Failed to mount NFS: {}", stderr));
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to run mount command: {}", e));
            }
        }

        Ok(())
    }

    /// Setup overlay volume
    fn setup_overlay_volume(&self, path: &Path) -> Result<()> {
        info!("  📚 Setting up overlay volume");

        // Create overlay directories
        let lower_dir = path.join("lower");
        let upper_dir = path.join("upper");
        let work_dir = path.join("work");
        let merged_dir = path.join("merged");

        for dir in &[&lower_dir, &upper_dir, &work_dir, &merged_dir] {
            fs::create_dir_all(dir)?;
        }

        info!("    ✓ Overlay directories created");
        Ok(())
    }

    /// Setup tmpfs volume
    fn setup_tmpfs_volume(&self, path: &Path, size_bytes: Option<u64>) -> Result<()> {
        info!("  💾 Setting up tmpfs volume");

        let mut mount_args = vec!["-t".to_string(), "tmpfs".to_string()];

        if let Some(size) = size_bytes {
            mount_args.push("-o".to_string());
            mount_args.push(format!("size={}", size));
        }

        mount_args.push("tmpfs".to_string());
        mount_args.push(path.to_string_lossy().to_string());

        let mount_cmd = std::process::Command::new("mount")
            .args(&mount_args)
            .output();

        match mount_cmd {
            Ok(output) => {
                if output.status.success() {
                    info!("    ✓ tmpfs volume mounted");
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(anyhow::anyhow!("Failed to mount tmpfs: {}", stderr));
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to run mount command: {}", e));
            }
        }

        Ok(())
    }

    /// Unmount volume
    fn unmount_volume(&self, volume: &Volume) -> Result<()> {
        if volume.driver == "nfs" || volume.driver == "tmpfs" {
            info!("  🔌 Unmounting {} volume", volume.driver);

            let umount_cmd = std::process::Command::new("umount")
                .arg(&volume.mount_point)
                .output();

            match umount_cmd {
                Ok(output) => {
                    if output.status.success() {
                        info!("    ✓ Volume unmounted");
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        warn!("Failed to unmount volume: {}", stderr);
                    }
                }
                Err(e) => {
                    warn!("Failed to run umount command: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Parse size string (e.g., "10GB", "512MB")
    fn parse_size(&self, size_str: &str) -> Result<u64> {
        let size_str = size_str.to_uppercase();

        if let Some(pos) = size_str.find(|c: char| c.is_alphabetic()) {
            let (number_part, unit_part) = size_str.split_at(pos);
            let number: f64 = number_part
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid size number: {}", number_part))?;

            let multiplier = match unit_part {
                "B" => 1,
                "KB" => 1_024,
                "MB" => 1_024_u64.pow(2),
                "GB" => 1_024_u64.pow(3),
                "TB" => 1_024_u64.pow(4),
                _ => return Err(anyhow::anyhow!("Invalid size unit: {}", unit_part)),
            };

            Ok((number * multiplier as f64) as u64)
        } else {
            // Assume bytes if no unit
            size_str
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid size: {}", size_str))
        }
    }

    /// Format size in bytes to human-readable string
    fn format_size(&self, size_bytes: Option<u64>) -> String {
        match size_bytes {
            Some(bytes) => {
                if bytes >= 1_024_u64.pow(4) {
                    format!("{:.1}TB", bytes as f64 / 1_024_f64.powi(4))
                } else if bytes >= 1_024_u64.pow(3) {
                    format!("{:.1}GB", bytes as f64 / 1_024_f64.powi(3))
                } else if bytes >= 1_024_u64.pow(2) {
                    format!("{:.1}MB", bytes as f64 / 1_024_f64.powi(2))
                } else if bytes >= 1_024 {
                    format!("{:.1}KB", bytes as f64 / 1_024.0)
                } else {
                    format!("{}B", bytes)
                }
            }
            None => "N/A".to_string(),
        }
    }

    /// Format system time to human-readable string
    fn format_time(&self, time: SystemTime) -> String {
        match time.elapsed() {
            Ok(duration) => {
                let days = duration.as_secs() / 86400;
                let hours = (duration.as_secs() % 86400) / 3600;
                let minutes = (duration.as_secs() % 3600) / 60;

                if days > 0 {
                    format!("{} days ago", days)
                } else if hours > 0 {
                    format!("{} hours ago", hours)
                } else if minutes > 0 {
                    format!("{} minutes ago", minutes)
                } else {
                    "Just now".to_string()
                }
            }
            Err(_) => "Unknown".to_string(),
        }
    }

    /// Save volume metadata to disk
    fn save_volume_metadata(&self, volume: &Volume) -> Result<()> {
        let metadata_file = self.volumes_dir.join(format!("{}.json", volume.name));
        let metadata_json = serde_json::to_string_pretty(volume)?;
        fs::write(metadata_file, metadata_json)?;
        Ok(())
    }

    /// Load existing volumes from disk
    fn load_volumes(&mut self) -> Result<()> {
        if !self.volumes_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.volumes_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension() == Some(std::ffi::OsStr::new("json")) {
                match fs::read_to_string(&path) {
                    Ok(content) => match serde_json::from_str::<Volume>(&content) {
                        Ok(volume) => {
                            debug!("Loaded volume: {}", volume.name);
                            self.volumes.insert(volume.name.clone(), volume);
                        }
                        Err(e) => {
                            warn!("Failed to parse volume metadata {:?}: {}", path, e);
                        }
                    },
                    Err(e) => {
                        warn!("Failed to read volume metadata {:?}: {}", path, e);
                    }
                }
            }
        }

        info!("📦 Loaded {} volumes", self.volumes.len());
        Ok(())
    }
}

impl Default for VolumeManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            volumes_dir: PathBuf::from("/tmp/bolt-volumes"),
            volumes: HashMap::new(),
        })
    }
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
