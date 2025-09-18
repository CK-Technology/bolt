//! BTRFS snapshot operations

use super::{Snapshot, SnapshotConfig, SnapshotType, FilesystemType};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;
use tokio::process::Command as AsyncCommand;
use tracing::{debug, info, warn};

/// Create a BTRFS snapshot
pub async fn create_snapshot(
    config: &SnapshotConfig,
    snapshot_id: &str,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<Snapshot> {
    let snapshot_path = config.snapshot_path.join(snapshot_id);

    // Create snapshot directory if it doesn't exist
    tokio::fs::create_dir_all(&config.snapshot_path).await
        .context("Failed to create snapshot directory")?;

    // Create BTRFS snapshot
    let mut cmd = AsyncCommand::new("btrfs");
    cmd.arg("subvolume")
        .arg("snapshot")
        .arg("-r") // Read-only snapshot
        .arg(&config.root_path)
        .arg(&snapshot_path);

    let output = cmd.output().await
        .context("Failed to execute btrfs snapshot command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("BTRFS snapshot failed: {}", stderr));
    }

    // Get snapshot size
    let size_bytes = get_snapshot_size(&snapshot_path).await.ok();

    let snapshot = Snapshot {
        id: snapshot_id.to_string(),
        name: name.map(|s| s.to_string()),
        description: description.map(|s| s.to_string()),
        timestamp: chrono::Utc::now(),
        snapshot_type: SnapshotType::Manual,
        filesystem_type: FilesystemType::BTRFS,
        path: snapshot_path,
        size_bytes,
        parent: None,
    };

    // Store snapshot metadata
    store_snapshot_metadata(&snapshot).await?;

    Ok(snapshot)
}

/// List BTRFS snapshots
pub async fn list_snapshots(config: &SnapshotConfig) -> Result<Vec<Snapshot>> {
    let mut cmd = AsyncCommand::new("btrfs");
    cmd.arg("subvolume")
        .arg("list")
        .arg("-s") // Show snapshots only
        .arg(&config.snapshot_path);

    let output = cmd.output().await
        .context("Failed to list BTRFS snapshots")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("Failed to list snapshots: {}", stderr);
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut snapshots = Vec::new();

    for line in stdout.lines() {
        if let Some(snapshot) = parse_btrfs_snapshot_line(line, config).await {
            snapshots.push(snapshot);
        }
    }

    // Load metadata for each snapshot
    for snapshot in &mut snapshots {
        if let Ok(metadata) = load_snapshot_metadata(&snapshot.id).await {
            snapshot.name = metadata.name;
            snapshot.description = metadata.description;
            snapshot.snapshot_type = metadata.snapshot_type;
        }
    }

    snapshots.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(snapshots)
}

/// Rollback to a BTRFS snapshot
pub async fn rollback_snapshot(config: &SnapshotConfig, snapshot_id: &str) -> Result<()> {
    let snapshot_path = config.snapshot_path.join(snapshot_id);

    if !snapshot_path.exists() {
        return Err(anyhow::anyhow!("Snapshot not found: {}", snapshot_id));
    }

    info!("🔄 Rolling back to BTRFS snapshot: {}", snapshot_id);

    // Create a backup snapshot of current state
    let backup_id = format!("bolt-backup-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
    let backup_path = config.snapshot_path.join(&backup_id);

    let mut backup_cmd = AsyncCommand::new("btrfs");
    backup_cmd.arg("subvolume")
        .arg("snapshot")
        .arg(&config.root_path)
        .arg(&backup_path);

    let backup_output = backup_cmd.output().await?;
    if !backup_output.status.success() {
        let stderr = String::from_utf8_lossy(&backup_output.stderr);
        return Err(anyhow::anyhow!("Failed to create backup snapshot: {}", stderr));
    }

    info!("✅ Backup snapshot created: {}", backup_id);

    // Perform the rollback
    // Note: This is a simplified approach. In practice, you might want to use
    // btrfs send/receive or other mechanisms depending on your setup
    let mut rollback_cmd = AsyncCommand::new("btrfs");
    rollback_cmd.arg("subvolume")
        .arg("snapshot")
        .arg(&snapshot_path)
        .arg(format!("{}.new", config.root_path.display()));

    let rollback_output = rollback_cmd.output().await?;
    if !rollback_output.status.success() {
        let stderr = String::from_utf8_lossy(&rollback_output.stderr);
        return Err(anyhow::anyhow!("Failed to rollback snapshot: {}", stderr));
    }

    warn!("⚠️ Rollback completed. You may need to reboot or remount the filesystem.");
    Ok(())
}

/// Delete a BTRFS snapshot
pub async fn delete_snapshot(config: &SnapshotConfig, snapshot_id: &str) -> Result<()> {
    let snapshot_path = config.snapshot_path.join(snapshot_id);

    if !snapshot_path.exists() {
        return Err(anyhow::anyhow!("Snapshot not found: {}", snapshot_id));
    }

    let mut cmd = AsyncCommand::new("btrfs");
    cmd.arg("subvolume")
        .arg("delete")
        .arg(&snapshot_path);

    let output = cmd.output().await
        .context("Failed to delete BTRFS snapshot")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to delete snapshot: {}", stderr));
    }

    // Remove metadata
    let metadata_path = get_metadata_path(snapshot_id);
    if metadata_path.exists() {
        tokio::fs::remove_file(metadata_path).await
            .context("Failed to remove snapshot metadata")?;
    }

    Ok(())
}

/// Get snapshot size in bytes
async fn get_snapshot_size(snapshot_path: &PathBuf) -> Result<u64> {
    let mut cmd = AsyncCommand::new("btrfs");
    cmd.arg("filesystem")
        .arg("du")
        .arg("-s")
        .arg(snapshot_path);

    let output = cmd.output().await?;
    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to get snapshot size"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(line) = stdout.lines().next() {
        if let Some(size_str) = line.split_whitespace().next() {
            return size_str.parse::<u64>()
                .context("Failed to parse snapshot size");
        }
    }

    Err(anyhow::anyhow!("Could not determine snapshot size"))
}

/// Parse BTRFS snapshot list line
async fn parse_btrfs_snapshot_line(line: &str, config: &SnapshotConfig) -> Option<Snapshot> {
    // Example line: "ID 123 gen 456 top level 5 path @snapshots/bolt-20231201-120000"
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 8 || parts[0] != "ID" {
        return None;
    }

    let snapshot_name = parts.last()?.strip_prefix("@snapshots/")
        .or_else(|| parts.last()?.strip_prefix(&format!("{}/", config.snapshot_path.display())))?;

    if !snapshot_name.starts_with("bolt-") {
        return None;
    }

    // Parse timestamp from snapshot name
    let timestamp_str = snapshot_name.strip_prefix("bolt-")?;
    let timestamp = chrono::NaiveDateTime::parse_from_str(timestamp_str, "%Y%m%d-%H%M%S")
        .ok()
        .map(|dt| chrono::DateTime::from_utc(dt, chrono::Utc))?;

    Some(Snapshot {
        id: snapshot_name.to_string(),
        name: None,
        description: None,
        timestamp,
        snapshot_type: SnapshotType::Manual,
        filesystem_type: FilesystemType::BTRFS,
        path: config.snapshot_path.join(snapshot_name),
        size_bytes: None,
        parent: None,
    })
}

/// Store snapshot metadata
async fn store_snapshot_metadata(snapshot: &Snapshot) -> Result<()> {
    let metadata_path = get_metadata_path(&snapshot.id);
    let metadata_dir = metadata_path.parent().unwrap();

    tokio::fs::create_dir_all(metadata_dir).await
        .context("Failed to create metadata directory")?;

    let metadata_json = serde_json::to_string_pretty(snapshot)
        .context("Failed to serialize snapshot metadata")?;

    tokio::fs::write(metadata_path, metadata_json).await
        .context("Failed to write snapshot metadata")?;

    Ok(())
}

/// Load snapshot metadata
async fn load_snapshot_metadata(snapshot_id: &str) -> Result<Snapshot> {
    let metadata_path = get_metadata_path(snapshot_id);
    let metadata_json = tokio::fs::read_to_string(metadata_path).await
        .context("Failed to read snapshot metadata")?;

    let snapshot: Snapshot = serde_json::from_str(&metadata_json)
        .context("Failed to parse snapshot metadata")?;

    Ok(snapshot)
}

/// Get metadata file path for a snapshot
fn get_metadata_path(snapshot_id: &str) -> PathBuf {
    PathBuf::from("/var/lib/bolt/snapshots").join(format!("{}.json", snapshot_id))
}