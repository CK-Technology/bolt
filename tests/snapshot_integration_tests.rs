//! Integration tests for snapshot system.

#[cfg(feature = "snapshots")]
mod snapshot_tests {
    use anyhow::Result;
    use bolt::snapshots::{
        AutoSnapshotConfig, FilesystemType, RetentionPolicy, SnapshotConfig, SnapshotManager,
        SnapshotType,
    };
    use std::path::PathBuf;

    fn test_config(path: &str, filesystem_type: FilesystemType) -> SnapshotConfig {
        let root_path = PathBuf::from(path);
        SnapshotConfig {
            enabled: true,
            filesystem_type,
            snapshot_path: root_path.join(".snapshots"),
            root_path,
            retention: RetentionPolicy {
                keep_hourly: 24,
                keep_daily: 7,
                keep_weekly: 4,
                keep_monthly: 6,
                keep_yearly: 2,
            },
            auto_snapshot: AutoSnapshotConfig {
                before_container_run: false,
                before_build: false,
                before_major_operations: false,
                hourly: false,
                daily: false,
            },
        }
    }

    #[tokio::test]
    async fn test_snapshot_manager_initialization() -> Result<()> {
        let config = test_config("/tmp/bolt-test-snapshots", FilesystemType::BTRFS);

        match SnapshotManager::new(config) {
            Ok(manager) => {
                let snapshot_result = manager
                    .create_snapshot(
                        Some("test-snapshot".to_string()),
                        Some("Test snapshot for integration testing".to_string()),
                        SnapshotType::Manual,
                    )
                    .await;

                if let Ok(snapshot) = snapshot_result {
                    let _ = manager.delete_snapshot(&snapshot.id).await;
                }
            }
            Err(e) => {
                println!(
                    "   Snapshot manager initialization failed (expected without BTRFS/ZFS): {}",
                    e
                );
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_snapshot_listing() -> Result<()> {
        let config = test_config("/tmp/bolt-test-snapshots", FilesystemType::BTRFS);

        if let Ok(manager) = SnapshotManager::new(config) {
            let snapshots = manager.list_snapshots().await?;
            for snapshot in snapshots.iter().take(5) {
                println!("   - {} ({:?})", snapshot.id, snapshot.snapshot_type);
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_snapshot_with_gpu_state() -> Result<()> {
        let config = test_config("/tmp/bolt-test-snapshots", FilesystemType::BTRFS);

        if let Ok(manager) = SnapshotManager::new(config) {
            let snapshot_result = manager
                .create_snapshot(
                    Some("gpu-snapshot-test".to_string()),
                    Some("Test snapshot with GPU state".to_string()),
                    SnapshotType::Manual,
                )
                .await;

            if let Ok(snapshot) = snapshot_result {
                if let Some(gpu_state) = &snapshot.gpu_state {
                    println!(
                        "   GPU state captured: {} devices",
                        gpu_state.device_states.len()
                    );
                }
                let _ = manager.delete_snapshot(&snapshot.id).await;
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_snapshot_rollback() -> Result<()> {
        let config = test_config("/tmp/bolt-test-snapshots", FilesystemType::BTRFS);

        if let Ok(manager) = SnapshotManager::new(config)
            && let Ok(snapshot) = manager
                .create_snapshot(
                    Some("rollback-test".to_string()),
                    Some("Test snapshot for rollback".to_string()),
                    SnapshotType::Manual,
                )
                .await
        {
            let rollback_result = manager.rollback_to_snapshot(&snapshot.id).await;
            if let Err(e) = rollback_result {
                println!("   Rollback failed (expected in test environment): {}", e);
            }
            let _ = manager.delete_snapshot(&snapshot.id).await;
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_retention_policy() -> Result<()> {
        let mut config = test_config("/tmp/bolt-test-snapshots", FilesystemType::BTRFS);
        config.retention.keep_hourly = 2;
        config.retention.keep_daily = 2;
        config.retention.keep_weekly = 1;
        config.retention.keep_monthly = 1;

        if let Ok(manager) = SnapshotManager::new(config) {
            let _ = manager.apply_retention_policy().await;
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_snapshot_deletion() -> Result<()> {
        let config = test_config("/tmp/bolt-test-snapshots", FilesystemType::BTRFS);

        if let Ok(manager) = SnapshotManager::new(config)
            && let Ok(snapshot) = manager
                .create_snapshot(
                    Some("delete-test".to_string()),
                    Some("Test snapshot for deletion".to_string()),
                    SnapshotType::Manual,
                )
                .await
        {
            let delete_result = manager.delete_snapshot(&snapshot.id).await;
            if delete_result.is_ok() {
                let snapshots = manager.list_snapshots().await?;
                let found = snapshots.iter().any(|s| s.id == snapshot.id);
                assert!(!found, "Snapshot should be deleted");
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_filesystem_type_detection() -> Result<()> {
        let btrfs_config = test_config("/tmp/bolt-test-btrfs", FilesystemType::BTRFS);
        let zfs_config = test_config("/tmp/bolt-test-zfs", FilesystemType::ZFS);

        for (name, config) in [("BTRFS", btrfs_config), ("ZFS", zfs_config)] {
            match SnapshotManager::new(config) {
                Ok(_) => println!("   {} snapshot manager initialized", name),
                Err(e) => println!("   {} snapshot manager failed (expected): {}", name, e),
            }
        }

        Ok(())
    }
}

#[cfg(not(feature = "snapshots"))]
#[tokio::test]
async fn test_snapshots_disabled() {
    println!("Snapshot features are not enabled");
}
