//! BTRFS/ZFS Snapshot Management for Bolt
//!
//! This module provides snapper-like functionality for automatic and manual
//! filesystem snapshots to ensure reproducibility and easy rollbacks.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;
use tracing::{debug, info, warn};

pub mod btrfs;
pub mod zfs;
pub mod retention;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotConfig {
    pub enabled: bool,
    pub filesystem_type: FilesystemType,
    pub root_path: PathBuf,
    pub snapshot_path: PathBuf,
    pub retention: RetentionPolicy,
    pub auto_snapshot: AutoSnapshotConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilesystemType {
    BTRFS,
    ZFS,
    Auto, // Auto-detect
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub keep_hourly: u32,
    pub keep_daily: u32,
    pub keep_weekly: u32,
    pub keep_monthly: u32,
    pub keep_yearly: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoSnapshotConfig {
    pub before_container_run: bool,
    pub before_build: bool,
    pub before_major_operations: bool,
    pub hourly: bool,
    pub daily: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub snapshot_type: SnapshotType,
    pub filesystem_type: FilesystemType,
    pub path: PathBuf,
    pub size_bytes: Option<u64>,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnapshotType {
    Manual,
    Auto,
    BeforeOperation(String),
    Named(String),
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug)]
pub struct SnapshotManager {
    config: SnapshotConfig,
    filesystem_type: FilesystemType,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            filesystem_type: FilesystemType::Auto,
            root_path: PathBuf::from("/"),
            snapshot_path: PathBuf::from("/.snapshots"),
            retention: RetentionPolicy::default(),
            auto_snapshot: AutoSnapshotConfig::default(),
        }
    }
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            keep_hourly: 24,  // Keep 24 hours
            keep_daily: 7,    // Keep 7 days
            keep_weekly: 4,   // Keep 4 weeks
            keep_monthly: 6,  // Keep 6 months
            keep_yearly: 2,   // Keep 2 years
        }
    }
}

impl Default for AutoSnapshotConfig {
    fn default() -> Self {
        Self {
            before_container_run: true,
            before_build: true,
            before_major_operations: true,
            hourly: false,
            daily: true,
        }
    }
}

impl SnapshotManager {
    pub fn new(config: SnapshotConfig) -> Result<Self> {
        let filesystem_type = match config.filesystem_type {
            FilesystemType::Auto => Self::detect_filesystem(&config.root_path)?,
            fs_type => fs_type,
        };

        info!("🗂️  Snapshot manager initialized");
        info!("  • Filesystem: {:?}", filesystem_type);
        info!("  • Root path: {}", config.root_path.display());
        info!("  • Snapshot path: {}", config.snapshot_path.display());

        Ok(Self {
            config,
            filesystem_type,
        })
    }

    /// Auto-detect filesystem type
    fn detect_filesystem(path: &Path) -> Result<FilesystemType> {
        let output = Command::new("findmnt")
            .arg("-n")
            .arg("-o")
            .arg("FSTYPE")
            .arg(path)
            .output()
            .context("Failed to detect filesystem type")?;

        let fstype = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_lowercase();

        match fstype.as_str() {
            "btrfs" => {
                info!("✅ Detected BTRFS filesystem");
                Ok(FilesystemType::BTRFS)
            }
            "zfs" => {
                info!("✅ Detected ZFS filesystem");
                Ok(FilesystemType::ZFS)
            }
            _ => {
                warn!("⚠️ Unsupported filesystem: {}, falling back to BTRFS", fstype);
                Ok(FilesystemType::BTRFS)
            }
        }
    }

    /// Create a snapshot
    pub async fn create_snapshot(
        &self,
        name: Option<String>,
        description: Option<String>,
        snapshot_type: SnapshotType,
    ) -> Result<Snapshot> {
        if !self.config.enabled {
            warn!("Snapshots disabled, skipping");
            return Err(anyhow::anyhow!("Snapshots are disabled"));
        }

        let timestamp = chrono::Utc::now();
        let snapshot_id = format!("bolt-{}", timestamp.format("%Y%m%d-%H%M%S"));

        info!("📸 Creating snapshot: {}", snapshot_id);
        if let Some(ref name) = name {
            info!("  • Name: {}", name);
        }
        if let Some(ref desc) = description {
            info!("  • Description: {}", desc);
        }

        let snapshot = match self.filesystem_type {
            FilesystemType::BTRFS => {
                btrfs::create_snapshot(&self.config, &snapshot_id, name.as_deref(), description.as_deref()).await?
            }
            FilesystemType::ZFS => {
                zfs::create_snapshot(&self.config, &snapshot_id, name.as_deref(), description.as_deref()).await?
            }
            FilesystemType::Auto => unreachable!("Auto should be resolved during initialization"),
        };

        info!("✅ Snapshot created: {}", snapshot_id);
        Ok(snapshot)
    }

    /// List all snapshots
    pub async fn list_snapshots(&self) -> Result<Vec<Snapshot>> {
        info!("📋 Listing snapshots...");

        let snapshots = match self.filesystem_type {
            FilesystemType::BTRFS => btrfs::list_snapshots(&self.config).await?,
            FilesystemType::ZFS => zfs::list_snapshots(&self.config).await?,
            FilesystemType::Auto => unreachable!(),
        };

        info!("  Found {} snapshots", snapshots.len());
        Ok(snapshots)
    }

    /// Rollback to a snapshot
    pub async fn rollback_to_snapshot(&self, snapshot_id: &str) -> Result<()> {
        info!("🔄 Rolling back to snapshot: {}", snapshot_id);

        match self.filesystem_type {
            FilesystemType::BTRFS => btrfs::rollback_snapshot(&self.config, snapshot_id).await?,
            FilesystemType::ZFS => zfs::rollback_snapshot(&self.config, snapshot_id).await?,
            FilesystemType::Auto => unreachable!(),
        }

        info!("✅ Rollback completed");
        Ok(())
    }

    /// Delete a snapshot
    pub async fn delete_snapshot(&self, snapshot_id: &str) -> Result<()> {
        info!("🗑️  Deleting snapshot: {}", snapshot_id);

        match self.filesystem_type {
            FilesystemType::BTRFS => btrfs::delete_snapshot(&self.config, snapshot_id).await?,
            FilesystemType::ZFS => zfs::delete_snapshot(&self.config, snapshot_id).await?,
            FilesystemType::Auto => unreachable!(),
        }

        info!("✅ Snapshot deleted");
        Ok(())
    }

    /// Apply retention policy
    pub async fn apply_retention_policy(&self) -> Result<()> {
        info!("🧹 Applying retention policy...");

        let snapshots = self.list_snapshots().await?;
        let to_delete = retention::calculate_snapshots_to_delete(&snapshots, &self.config.retention);

        for snapshot in to_delete {
            info!("  • Cleaning up old snapshot: {}", snapshot.id);
            self.delete_snapshot(&snapshot.id).await?;
        }

        info!("✅ Retention policy applied");
        Ok(())
    }

    /// Auto snapshot before major operations
    pub async fn auto_snapshot_before_operation(&self, operation: &str) -> Result<Option<Snapshot>> {
        if !self.config.auto_snapshot.before_major_operations {
            return Ok(None);
        }

        info!("📸 Auto snapshot before operation: {}", operation);

        let snapshot = self.create_snapshot(
            None,
            Some(format!("Before operation: {}", operation)),
            SnapshotType::BeforeOperation(operation.to_string()),
        ).await?;

        Ok(Some(snapshot))
    }

    /// Get snapshot configuration
    pub fn get_config(&self) -> &SnapshotConfig {
        &self.config
    }

    /// Update snapshot configuration
    pub async fn update_config(&mut self, new_config: SnapshotConfig) -> Result<()> {
        info!("⚙️  Updating snapshot configuration");
        self.config = new_config;
        Ok(())
    }
}

/// Initialize snapshot manager from configuration
pub async fn init_snapshot_manager() -> Result<SnapshotManager> {
    let config = SnapshotConfig::default(); // TODO: Load from file
    SnapshotManager::new(config)
}

/// Create a pre-operation snapshot
pub async fn create_pre_operation_snapshot(
    manager: &SnapshotManager,
    operation: &str,
) -> Result<Option<Snapshot>> {
    manager.auto_snapshot_before_operation(operation).await
}