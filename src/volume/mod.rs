use crate::Result;
use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
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
        manager.reconcile_usage_from_container_state_sync()?;
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
        manager.reconcile_usage_from_container_state().await?;
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
        apply_volume_options_sync(&volume_path, &options.options)?;

        let volume_info = VolumeInfo {
            name: name.to_string(),
            driver: options.driver,
            mountpoint: volume_path,
            created: chrono::Utc::now(),
            labels: options.labels,
            options: options.options,
            scope: VolumeScope::Local,
            size_limit: parse_size_limit(options.size.as_deref())?,
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
        let options = request.options.unwrap_or_default();
        apply_volume_options_async(&volume_path, &options).await?;

        let volume_info = VolumeInfo {
            name: request.name.clone(),
            driver: request.driver,
            mountpoint: volume_path,
            created: chrono::Utc::now(),
            labels: request.labels.unwrap_or_default(),
            options,
            scope: VolumeScope::Local,
            size_limit: parse_size_limit(request.size.as_deref())?,
            used_by: Vec::new(),
        };

        // Save volume metadata
        self.save_volume_metadata(&volume_info).await?;
        self.volumes
            .insert(request.name.clone(), volume_info.clone());

        info!("✅ Volume created: {}", request.name);
        Ok(volume_info)
    }

    pub async fn attach_volume_async(
        &mut self,
        volume_name: &str,
        container_id: &str,
    ) -> Result<()> {
        let updated = {
            let volume = self
                .volumes
                .get_mut(volume_name)
                .ok_or_else(|| anyhow!("Volume '{}' not found", volume_name))?;
            if !volume.used_by.iter().any(|id| id == container_id) {
                volume.used_by.push(container_id.to_string());
                volume.used_by.sort();
            }
            volume.clone()
        };
        self.save_volume_metadata(&updated).await?;
        Ok(())
    }

    pub async fn detach_volume_async(
        &mut self,
        volume_name: &str,
        container_id: &str,
    ) -> Result<()> {
        let updated = {
            let volume = self
                .volumes
                .get_mut(volume_name)
                .ok_or_else(|| anyhow!("Volume '{}' not found", volume_name))?;
            volume.used_by.retain(|id| id != container_id);
            volume.clone()
        };
        self.save_volume_metadata(&updated).await?;
        Ok(())
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

    fn reconcile_usage_from_container_state_sync(&mut self) -> Result<()> {
        let containers_dir = self.storage_root.join("containers");
        if !containers_dir.exists() {
            return Ok(());
        }

        for volume in self.volumes.values_mut() {
            volume.used_by.clear();
        }

        for entry in std::fs::read_dir(containers_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let state_path = entry.path().join("state.json");
            let Ok(contents) = std::fs::read_to_string(state_path) else {
                continue;
            };
            let Ok(state) = serde_json::from_str::<crate::runtime::oci::ContainerState>(&contents)
            else {
                continue;
            };
            self.mark_container_volume_usage(&state.id, &state.config.volumes);
        }

        let volumes: Vec<_> = self.volumes.values().cloned().collect();
        for volume in volumes {
            self.save_volume_metadata_sync(&volume)?;
        }
        Ok(())
    }

    async fn reconcile_usage_from_container_state(&mut self) -> Result<()> {
        let containers_dir = self.storage_root.join("containers");
        if !containers_dir.exists() {
            return Ok(());
        }

        for volume in self.volumes.values_mut() {
            volume.used_by.clear();
        }

        let mut entries = fs::read_dir(containers_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }
            let state_path = entry.path().join("state.json");
            let Ok(contents) = fs::read_to_string(state_path).await else {
                continue;
            };
            let Ok(state) = serde_json::from_str::<crate::runtime::oci::ContainerState>(&contents)
            else {
                continue;
            };
            self.mark_container_volume_usage(&state.id, &state.config.volumes);
        }

        let volumes: Vec<_> = self.volumes.values().cloned().collect();
        for volume in volumes {
            self.save_volume_metadata(&volume).await?;
        }
        Ok(())
    }

    fn mark_container_volume_usage(
        &mut self,
        container_id: &str,
        mounts: &[crate::runtime::oci::VolumeMount],
    ) {
        for mount in mounts {
            for volume in self.volumes.values_mut() {
                if mount.source == volume.name
                    || PathBuf::from(&mount.source) == volume.mountpoint
                    || PathBuf::from(&mount.source)
                        == volume
                            .mountpoint
                            .parent()
                            .map(PathBuf::from)
                            .unwrap_or_else(|| volume.mountpoint.clone())
                {
                    if !volume.used_by.iter().any(|id| id == container_id) {
                        volume.used_by.push(container_id.to_string());
                    }
                }
            }
        }
        for volume in self.volumes.values_mut() {
            volume.used_by.sort();
            volume.used_by.dedup();
        }
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

fn parse_size_limit(size: Option<&str>) -> Result<Option<u64>> {
    let Some(raw) = size else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let split_at = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    let (number, suffix) = trimmed.split_at(split_at);
    let value: u64 = number
        .parse()
        .with_context(|| format!("Invalid volume size '{}'", raw))?;
    let multiplier = match suffix.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" | "ki" | "kib" => 1024,
        "m" | "mb" | "mi" | "mib" => 1024 * 1024,
        "g" | "gb" | "gi" | "gib" => 1024 * 1024 * 1024,
        "t" | "tb" | "ti" | "tib" => 1024_u64.pow(4),
        other => return Err(anyhow!("Unsupported volume size suffix '{}'", other).into()),
    };
    Ok(Some(value.saturating_mul(multiplier)))
}

fn apply_volume_options_sync(path: &PathBuf, options: &HashMap<String, String>) -> Result<()> {
    if let Some(mode) = parse_mode(options)? {
        #[cfg(unix)]
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    }
    apply_owner(path, options)?;
    Ok(())
}

async fn apply_volume_options_async(
    path: &PathBuf,
    options: &HashMap<String, String>,
) -> Result<()> {
    if let Some(mode) = parse_mode(options)? {
        #[cfg(unix)]
        fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).await?;
    }
    apply_owner(path, options)?;
    Ok(())
}

fn parse_mode(options: &HashMap<String, String>) -> Result<Option<u32>> {
    let Some(mode) = options.get("mode") else {
        return Ok(None);
    };
    let trimmed = mode.trim_start_matches("0o");
    Ok(Some(u32::from_str_radix(trimmed, 8).with_context(
        || format!("Invalid volume mode '{}'", mode),
    )?))
}

fn apply_owner(path: &PathBuf, options: &HashMap<String, String>) -> Result<()> {
    let uid = options.get("uid");
    let gid = options.get("gid");
    if uid.is_none() && gid.is_none() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        let owner = match (uid, gid) {
            (Some(uid), Some(gid)) => format!("{uid}:{gid}"),
            (Some(uid), None) => uid.to_string(),
            (None, Some(gid)) => format!(":{gid}"),
            (None, None) => unreachable!(),
        };
        let output = Command::new("chown").arg(owner).arg(path).output()?;
        if !output.status.success() {
            return Err(anyhow!(
                "Failed to set volume ownership on {}: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )
            .into());
        }
    }
    #[cfg(not(unix))]
    {
        anyhow::bail!("volume uid/gid options are only supported on Unix hosts");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::oci::{ContainerConfig, ContainerState, ContainerStatus, VolumeMount};

    fn scratch_tempdir() -> tempfile::TempDir {
        std::fs::create_dir_all(".scratch").expect("create repo-local scratch directory");
        tempfile::tempdir_in(".scratch").expect("create repo-local scratch tempdir")
    }

    #[tokio::test]
    async fn volume_usage_is_reconciled_from_container_state() -> Result<()> {
        let root = scratch_tempdir();
        let mut manager = VolumeManager::new_async(root.path().to_path_buf()).await?;
        manager
            .create_volume_async(VolumeCreateRequest {
                name: "data".to_string(),
                driver: "local".to_string(),
                labels: None,
                options: None,
                size: None,
            })
            .await?;

        let state_dir = root.path().join("containers/c1");
        fs::create_dir_all(&state_dir).await?;
        let state = ContainerState {
            id: "c1".to_string(),
            status: ContainerStatus::Created,
            pid: None,
            bundle_path: root.path().join("bundles/c1"),
            config: ContainerConfig {
                id: "c1".to_string(),
                name: None,
                image: "alpine:latest".to_string(),
                command: vec![],
                args: vec![],
                env: HashMap::new(),
                working_dir: None,
                user: None,
                hostname: None,
                network_mode: "bridge".to_string(),
                ports: vec![],
                volumes: vec![VolumeMount {
                    source: "data".to_string(),
                    destination: "/data".to_string(),
                    readonly: false,
                }],
                capabilities: vec![],
                resource_limits: None,
                gaming_config: None,
                detach: true,
                privileged: false,
                tty: false,
                readonly_rootfs: false,
                seccomp: None,
            },
            created: std::time::SystemTime::now(),
            started: None,
            finished: None,
            exit_code: None,
            image_digest: None,
            log_path: None,
            gpu_allocation: None,
        };
        fs::write(
            state_dir.join("state.json"),
            serde_json::to_string_pretty(&state)?,
        )
        .await?;

        let manager = VolumeManager::new_async(root.path().to_path_buf()).await?;
        let volumes = manager.list_volumes_async().await?;
        assert_eq!(volumes[0].used_by, vec!["c1"]);
        Ok(())
    }

    #[tokio::test]
    async fn volume_attach_detach_updates_metadata_immediately() -> Result<()> {
        let root = scratch_tempdir();
        let mut options = HashMap::new();
        options.insert("mode".to_string(), "0750".to_string());
        let mut manager = VolumeManager::new_async(root.path().to_path_buf()).await?;
        let created = manager
            .create_volume_async(VolumeCreateRequest {
                name: "data".to_string(),
                driver: "local".to_string(),
                labels: None,
                options: Some(options),
                size: Some("2MiB".to_string()),
            })
            .await?;
        assert_eq!(created.size_limit, Some(2 * 1024 * 1024));

        manager.attach_volume_async("data", "c1").await?;
        let manager = VolumeManager::new_async(root.path().to_path_buf()).await?;
        assert_eq!(manager.inspect_volume("data")?.used_by, vec!["c1"]);

        let mut manager = manager;
        manager.detach_volume_async("data", "c1").await?;
        let manager = VolumeManager::new_async(root.path().to_path_buf()).await?;
        assert!(manager.inspect_volume("data")?.used_by.is_empty());

        #[cfg(unix)]
        {
            let mode = std::fs::metadata(root.path().join("volumes/data/_data"))?
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o750);
        }
        Ok(())
    }
}
