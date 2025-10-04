//! ZFS Snapshot Support (stub)

use anyhow::Result;
use super::{Snapshot, SnapshotConfig, SnapshotType, FilesystemType};
use std::path::PathBuf;
use tracing::{info, warn};

pub async fn create_snapshot(
    config: &SnapshotConfig,
    snapshot_id: &str,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<Snapshot> {
    warn!("ZFS snapshot support is not yet implemented");

    Ok(Snapshot {
        id: snapshot_id.to_string(),
        name: name.map(String::from),
        description: description.map(String::from),
        timestamp: chrono::Utc::now(),
        snapshot_type: SnapshotType::Manual,
        filesystem_type: FilesystemType::ZFS,
        path: config.snapshot_path.join(snapshot_id),
        size_bytes: None,
        parent: None,
        gpu_state: None,
    })
}

pub async fn list_snapshots(_config: &SnapshotConfig) -> Result<Vec<Snapshot>> {
    warn!("ZFS snapshot listing is not yet implemented");
    Ok(Vec::new())
}

pub async fn rollback_snapshot(_config: &SnapshotConfig, snapshot_id: &str) -> Result<()> {
    warn!("ZFS snapshot rollback is not yet implemented: {}", snapshot_id);
    Ok(())
}

pub async fn delete_snapshot(_config: &SnapshotConfig, snapshot_id: &str) -> Result<()> {
    warn!("ZFS snapshot deletion is not yet implemented: {}", snapshot_id);
    Ok(())
}
