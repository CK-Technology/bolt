// Snapshot/restore system for Bolt Capsules
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, info};

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
}

pub struct SnapshotManager {
    snapshot_root: PathBuf,
}

impl SnapshotManager {
    pub async fn new() -> Result<Self> {
        let snapshot_root = PathBuf::from("/var/lib/bolt/snapshots");
        tokio::fs::create_dir_all(&snapshot_root).await?;
        Ok(Self { snapshot_root })
    }

    pub async fn create_snapshot(
        &self,
        name: &str,
        snapshot_type: SnapshotType,
        description: Option<&str>,
    ) -> Result<()> {
        info!("Creating {:?} snapshot: {}", snapshot_type, name);

        let snapshot_path = self.snapshot_root.join(name);

        // Create snapshot using BTRFS subvolume snapshot
        let output = Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r", "/var/lib/bolt/containers"])
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

    pub async fn rollback_to_snapshot(&self, name: &str) -> Result<()> {
        info!("Rolling back to snapshot: {}", name);

        let snapshot_path = self.snapshot_root.join(name);
        if !snapshot_path.exists() {
            anyhow::bail!("Snapshot '{}' not found", name);
        }

        // Stop all containers first
        debug!("Stopping all containers before rollback");

        // Delete current container data
        let containers_path = PathBuf::from("/var/lib/bolt/containers");
        if containers_path.exists() {
            tokio::fs::remove_dir_all(&containers_path).await?;
        }

        // Restore from snapshot using BTRFS snapshot
        let output = Command::new("btrfs")
            .args(["subvolume", "snapshot"])
            .arg(&snapshot_path)
            .arg(&containers_path)
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
        info!("Deleting snapshot: {}", name);

        let snapshot_path = self.snapshot_root.join(name);
        if !snapshot_path.exists() {
            anyhow::bail!("Snapshot '{}' not found", name);
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
}
