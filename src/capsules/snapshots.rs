// Snapshot/restore system for Bolt Capsules
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SnapshotType {
    Manual,
    Auto,
    Daily,
    Weekly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub name: String,
    pub created_at: String,
    pub snapshot_type: SnapshotType,
    pub description: Option<String>,
    pub path: PathBuf,
    #[serde(default)]
    pub metadata: SnapshotMetadata,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub root_path: PathBuf,
    pub containers_path: PathBuf,
    pub image_digests: Vec<String>,
    pub container_ids: Vec<String>,
    #[serde(default)]
    pub boltfile_path: Option<PathBuf>,
    #[serde(default)]
    pub boltfile_hash: Option<String>,
    #[serde(default)]
    pub boltfile_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generation {
    pub id: String,
    pub snapshot_name: String,
    pub created_at: String,
    pub snapshot_type: SnapshotType,
    pub container_ids: Vec<String>,
    pub image_digests: Vec<String>,
    pub boltfile_path: Option<PathBuf>,
    pub boltfile_hash: Option<String>,
    pub boltfile_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotPreflight {
    pub filesystem: String,
    pub snapshot_root: PathBuf,
    pub containers_path: PathBuf,
    pub btrfs_available: bool,
    pub zfs_available: bool,
    pub supported: bool,
    pub reason: Option<String>,
}

pub struct SnapshotManager {
    data_root: PathBuf,
    snapshot_root: PathBuf,
    containers_path: PathBuf,
}

impl SnapshotManager {
    pub async fn new() -> Result<Self> {
        let data_root = bolt_data_root();
        Self::new_with_root(data_root).await
    }

    pub async fn new_with_root(data_root: PathBuf) -> Result<Self> {
        let snapshot_root = data_root.join("snapshots");
        let containers_path = data_root.join("containers");
        tokio::fs::create_dir_all(&snapshot_root).await?;
        tokio::fs::create_dir_all(&containers_path).await?;
        Ok(Self {
            data_root,
            snapshot_root,
            containers_path,
        })
    }

    pub async fn preflight(&self) -> Result<SnapshotPreflight> {
        let filesystem = detect_filesystem(&self.data_root).unwrap_or_else(|err| {
            warn!("failed to detect snapshot filesystem: {}", err);
            "unknown".to_string()
        });
        let btrfs_available = command_available("btrfs");
        let zfs_available = command_available("zfs");
        let supported = match filesystem.as_str() {
            "btrfs" => btrfs_available,
            "zfs" => zfs_available,
            _ => false,
        };
        let reason = if supported {
            None
        } else if filesystem == "btrfs" {
            Some("btrfs command is not available".to_string())
        } else if filesystem == "zfs" {
            Some("zfs command is not available".to_string())
        } else {
            Some(format!(
                "filesystem '{}' is not snapshot-capable",
                filesystem
            ))
        };

        Ok(SnapshotPreflight {
            filesystem,
            snapshot_root: self.snapshot_root.clone(),
            containers_path: self.containers_path.clone(),
            btrfs_available,
            zfs_available,
            supported,
            reason,
        })
    }

    pub async fn create_snapshot(
        &self,
        name: &str,
        snapshot_type: SnapshotType,
        description: Option<&str>,
    ) -> Result<()> {
        info!("Creating {:?} snapshot: {}", snapshot_type, name);

        let snapshot_path = self.snapshot_root.join(name);
        if snapshot_path.exists() {
            anyhow::bail!("Snapshot '{}' already exists", name);
        }

        // Create snapshot using BTRFS subvolume snapshot
        let output = Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(&self.containers_path)
            .arg(&snapshot_path)
            .output()
            .context("Failed to create BTRFS snapshot")?;

        if !output.status.success() {
            anyhow::bail!(
                "BTRFS snapshot failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Save snapshot metadata
        let snapshot = Snapshot {
            name: name.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            snapshot_type,
            description: description.map(|s| s.to_string()),
            path: snapshot_path.clone(),
            metadata: self.collect_metadata().await?,
        };

        let metadata_path = snapshot_path.join("snapshot.json");
        let metadata = serde_json::to_string_pretty(&snapshot)?;
        tokio::fs::write(&metadata_path, metadata).await?;

        info!("✅ Snapshot '{}' created at {:?}", name, snapshot_path);
        Ok(())
    }

    pub async fn list_snapshots(&self) -> Result<Vec<Snapshot>> {
        let mut snapshots = Vec::new();

        let mut entries = tokio::fs::read_dir(&self.snapshot_root).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let metadata_path = entry.path().join("snapshot.json");
                if metadata_path.exists() {
                    let metadata = tokio::fs::read_to_string(&metadata_path).await?;
                    if let Ok(snapshot) = serde_json::from_str::<Snapshot>(&metadata) {
                        snapshots.push(snapshot);
                    }
                }
            }
        }

        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(snapshots)
    }

    pub async fn list_generations(&self) -> Result<Vec<Generation>> {
        let generations = self
            .list_snapshots()
            .await?
            .into_iter()
            .map(|snapshot| Generation {
                id: snapshot.name.clone(),
                snapshot_name: snapshot.name,
                created_at: snapshot.created_at,
                snapshot_type: snapshot.snapshot_type,
                container_ids: snapshot.metadata.container_ids,
                image_digests: snapshot.metadata.image_digests,
                boltfile_path: snapshot.metadata.boltfile_path,
                boltfile_hash: snapshot.metadata.boltfile_hash,
                boltfile_revision: snapshot.metadata.boltfile_revision,
            })
            .collect();
        Ok(generations)
    }

    pub async fn rollback_to_snapshot(&self, name: &str) -> Result<()> {
        self.rollback_to_snapshot_checked(name, false).await
    }

    pub async fn rollback_to_snapshot_checked(&self, name: &str, force: bool) -> Result<()> {
        if !force {
            anyhow::bail!("Rollback requires --force because it replaces container state");
        }

        info!("Rolling back to snapshot: {}", name);

        let snapshot_path = self.snapshot_root.join(name);
        if !snapshot_path.exists() {
            anyhow::bail!("Snapshot '{}' not found", name);
        }

        // Stop all containers first
        debug!("Stopping all containers before rollback");

        let rescue_name = format!(
            "pre-rollback-{}",
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        );
        let rescue_path = self.snapshot_root.join(&rescue_name);
        if self.containers_path.exists() {
            let rescue = Command::new("btrfs")
                .args(["subvolume", "snapshot", "-r"])
                .arg(&self.containers_path)
                .arg(&rescue_path)
                .output()
                .context("Failed to create pre-rollback rescue snapshot")?;
            if !rescue.status.success() {
                anyhow::bail!(
                    "BTRFS rescue snapshot failed: {}",
                    String::from_utf8_lossy(&rescue.stderr)
                );
            }
            self.write_metadata_file(
                &rescue_path,
                &rescue_name,
                SnapshotType::Auto,
                Some("Pre-rollback rescue snapshot"),
            )
            .await?;
        }

        // Delete current container data
        if self.containers_path.exists() {
            tokio::fs::remove_dir_all(&self.containers_path).await?;
        }

        // Restore from snapshot using BTRFS snapshot
        let output = Command::new("btrfs")
            .args(["subvolume", "snapshot"])
            .arg(&snapshot_path)
            .arg(&self.containers_path)
            .output()
            .context("Failed to restore from snapshot")?;

        if !output.status.success() {
            anyhow::bail!(
                "BTRFS restore failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        info!("✅ Rolled back to snapshot '{}'", name);
        Ok(())
    }

    pub async fn delete_snapshot(&self, name: &str) -> Result<()> {
        self.delete_snapshot_checked(name, false, false).await
    }

    pub async fn delete_snapshot_checked(
        &self,
        name: &str,
        force: bool,
        dry_run: bool,
    ) -> Result<()> {
        info!("Deleting snapshot: {}", name);

        let snapshot_path = self.snapshot_root.join(name);
        if !snapshot_path.exists() {
            anyhow::bail!("Snapshot '{}' not found", name);
        }
        if !force && !dry_run {
            anyhow::bail!("Deleting snapshot '{}' requires --force", name);
        }
        if dry_run {
            println!("Would delete snapshot: {}", name);
            return Ok(());
        }

        // Delete BTRFS subvolume
        let output = Command::new("btrfs")
            .args(["subvolume", "delete"])
            .arg(&snapshot_path)
            .output()
            .context("Failed to delete BTRFS snapshot")?;

        if !output.status.success() {
            anyhow::bail!(
                "BTRFS delete failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        info!("✅ Deleted snapshot '{}'", name);
        Ok(())
    }

    pub async fn cleanup_old_snapshots(
        &self,
        dry_run: bool,
        force: bool,
        keep: usize,
    ) -> Result<usize> {
        let snapshots = self.list_snapshots().await?;
        let mut automatic: Vec<_> = snapshots
            .into_iter()
            .filter(|snapshot| {
                matches!(
                    snapshot.snapshot_type,
                    SnapshotType::Auto | SnapshotType::Daily | SnapshotType::Weekly
                )
            })
            .collect();
        automatic.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let mut count = 0;
        for snapshot in automatic.into_iter().skip(keep) {
            self.delete_snapshot_checked(&snapshot.name, force, dry_run)
                .await?;
            count += 1;
        }
        Ok(count)
    }

    async fn collect_metadata(&self) -> Result<SnapshotMetadata> {
        Ok(SnapshotMetadata {
            root_path: self.data_root.clone(),
            containers_path: self.containers_path.clone(),
            image_digests: read_image_digests(self.data_root.join("images")).await?,
            container_ids: read_child_dirs(&self.containers_path).await?,
            boltfile_path: find_boltfile(),
            boltfile_hash: hash_boltfile().await?,
            boltfile_revision: git_revision(),
        })
    }

    async fn write_metadata_file(
        &self,
        snapshot_path: &PathBuf,
        name: &str,
        snapshot_type: SnapshotType,
        description: Option<&str>,
    ) -> Result<()> {
        let snapshot = Snapshot {
            name: name.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            snapshot_type,
            description: description.map(ToOwned::to_owned),
            path: snapshot_path.clone(),
            metadata: self.collect_metadata().await?,
        };
        let metadata_path = snapshot_path.join("snapshot.json");
        tokio::fs::write(&metadata_path, serde_json::to_string_pretty(&snapshot)?).await?;
        Ok(())
    }
}

fn find_boltfile() -> Option<PathBuf> {
    let path = PathBuf::from("Boltfile.toml");
    path.exists().then_some(path)
}

async fn hash_boltfile() -> Result<Option<String>> {
    let Some(path) = find_boltfile() else {
        return Ok(None);
    };
    let bytes = tokio::fs::read(path).await?;
    Ok(Some(format!("sha256:{:x}", Sha256::digest(&bytes))))
}

fn git_revision() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|revision| !revision.is_empty())
}

fn bolt_data_root() -> PathBuf {
    std::env::var("BOLT_STORAGE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("bolt")
        })
}

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn detect_filesystem(path: &PathBuf) -> Result<String> {
    let output = Command::new("findmnt")
        .arg("-n")
        .arg("-o")
        .arg("FSTYPE")
        .arg("--target")
        .arg(path)
        .output()
        .context("Failed to run findmnt")?;
    if !output.status.success() {
        anyhow::bail!(
            "findmnt failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_lowercase())
}

async fn read_child_dirs(path: &PathBuf) -> Result<Vec<String>> {
    let mut out = Vec::new();
    if !path.exists() {
        return Ok(out);
    }
    let mut entries = tokio::fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir()
            && let Some(name) = entry.file_name().to_str()
        {
            out.push(name.to_string());
        }
    }
    out.sort();
    Ok(out)
}

async fn read_image_digests(images_dir: PathBuf) -> Result<Vec<String>> {
    let mut out = Vec::new();
    if !images_dir.exists() {
        return Ok(out);
    }
    let mut entries = tokio::fs::read_dir(images_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.path().join("metadata.json");
        if !metadata.exists() {
            continue;
        }
        let Ok(contents) = tokio::fs::read_to_string(metadata).await else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
            continue;
        };
        if let Some(digest) = value.get("digest").and_then(|digest| digest.as_str()) {
            out.push(digest.to_string());
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn snapshot_metadata_collects_containers_and_image_digests() -> Result<()> {
        tokio::fs::create_dir_all(".scratch").await?;
        let root = tempfile::tempdir_in(".scratch")?;
        let data_root = root.path().to_path_buf();
        tokio::fs::create_dir_all(data_root.join("containers/c1")).await?;
        tokio::fs::create_dir_all(data_root.join("images/i1")).await?;
        tokio::fs::write(
            data_root.join("images/i1/metadata.json"),
            r#"{"digest":"sha256:abc"}"#,
        )
        .await?;

        let manager = SnapshotManager::new_with_root(data_root).await?;
        let metadata = manager.collect_metadata().await?;
        assert_eq!(metadata.container_ids, vec!["c1"]);
        assert_eq!(metadata.image_digests, vec!["sha256:abc"]);
        Ok(())
    }
}
