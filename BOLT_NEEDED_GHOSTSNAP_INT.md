# Bolt Requirements for Ghostsnap Integration

## Overview

This document outlines the changes and additions needed in Bolt to integrate with ghostsnap (and restic) for comprehensive backup capabilities. The integration should provide seamless backup functionality that complements Bolt's existing BTRFS/ZFS snapshot system.

## Required Bolt Changes

### 1. Cargo.toml Dependencies

Add backup tool dependencies with feature flags:

```toml
[features]
default = ["backup-support"]
backup-support = ["ghostsnap", "restic"]
ghostsnap-support = ["ghostsnap"]
restic-support = ["restic"]

[dependencies]
# Ghostsnap integration
ghostsnap = { git = "https://github.com/ghostkellz/ghostsnap", optional = true }

# Restic integration (via subprocess)
tokio-process = "0.2"

# Additional backup utilities
walkdir = "2.4"          # For file tree traversal
globset = "0.4"          # For exclude patterns
async-trait = "0.1"      # For backup trait abstraction
serde_yaml = "0.9"       # For backup configuration
tempfile = "3.8"         # For temporary backup operations
```

### 2. Project Structure Changes

Create new modules for backup functionality:

```
src/
├── backup/
│   ├── mod.rs                 # Main backup module
│   ├── manager.rs             # BackupManager
│   ├── ghostsnap.rs           # Ghostsnap backend
│   ├── restic.rs              # Restic backend
│   ├── config.rs              # Backup configuration
│   ├── storage.rs             # Storage backend abstraction
│   └── retention.rs           # Retention policy management
├── integration/
│   ├── mod.rs                 # Integration coordination
│   └── snapshot_backup.rs     # Snapshot + backup workflows
└── cli/
    └── backup.rs              # Backup CLI commands
```

### 3. Configuration Schema Updates

Extend `src/config/mod.rs` to include backup configuration:

```rust
// Add to BoltConfig struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltConfig {
    // ... existing fields

    #[serde(default)]
    pub backups: Option<BackupConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub enabled: Option<bool>,
    pub tool: Option<BackupTool>,
    pub repository: Option<String>,
    pub password_file: Option<String>,
    pub key_file: Option<String>,

    #[serde(default)]
    pub triggers: Option<BackupTriggers>,

    #[serde(default)]
    pub retention: Option<BackupRetention>,

    #[serde(default)]
    pub include: Option<BackupInclude>,

    #[serde(default)]
    pub exclude: Option<BackupExclude>,

    #[serde(default)]
    pub storage: Option<BackupStorage>,

    #[serde(default)]
    pub performance: Option<BackupPerformance>,

    #[serde(default)]
    pub security: Option<BackupSecurity>,

    #[serde(default)]
    pub ghostsnap: Option<GhostSnapConfig>,

    #[serde(default)]
    pub restic: Option<ResticConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackupTool {
    #[serde(rename = "ghostsnap")]
    GhostSnap,
    #[serde(rename = "restic")]
    Restic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupTriggers {
    pub after_snapshot: Option<bool>,
    pub daily: Option<String>,
    pub weekly: Option<String>,
    pub monthly: Option<String>,
    pub before_surge_operations: Option<bool>,
    pub on_container_changes: Option<bool>,
    pub on_file_changes: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRetention {
    pub keep_daily: Option<u32>,
    pub keep_weekly: Option<u32>,
    pub keep_monthly: Option<u32>,
    pub keep_yearly: Option<u32>,
    pub keep_last: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInclude {
    pub container_volumes: Option<bool>,
    pub bolt_config: Option<bool>,
    pub system_config: Option<Vec<String>>,
    pub custom_paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupExclude {
    pub patterns: Option<Vec<String>>,
    pub paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupStorage {
    pub primary: Option<String>,
    pub secondary: Option<String>,
    pub s3: Option<S3Config>,
    pub azure: Option<AzureConfig>,
    pub minio: Option<MinIOConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPerformance {
    pub parallel_uploads: Option<u32>,
    pub chunk_size: Option<String>,
    pub compression: Option<String>,
    pub compression_level: Option<u32>,
    pub bandwidth_limit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSecurity {
    pub encryption: Option<String>,
    pub key_derivation: Option<String>,
    pub verify_certificates: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostSnapConfig {
    pub exclude_patterns: Option<Vec<String>>,
    pub chunk_size: Option<String>,
    pub compression: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResticConfig {
    pub cache_dir: Option<String>,
    pub parallel_connections: Option<u32>,
}
```

### 4. CLI Command Extensions

Add backup commands to `src/cli/mod.rs`:

```rust
// Add to Commands enum
#[derive(Subcommand)]
pub enum Commands {
    // ... existing commands

    /// Backup management
    Backup {
        #[command(subcommand)]
        command: BackupCommands,
    },

    /// Combined snapshot and backup operations
    SnapshotBackup {
        #[command(subcommand)]
        command: SnapshotBackupCommands,
    },
}

#[derive(Subcommand)]
pub enum BackupCommands {
    /// Initialize backup repository
    Init {
        /// Repository URL (e.g., s3:bucket/path)
        repository: String,

        /// Password file path
        #[arg(long)]
        password_file: Option<String>,
    },

    /// Create backup
    Create {
        /// Backup name
        #[arg(short, long)]
        name: Option<String>,

        /// Include specific paths
        #[arg(long)]
        include: Vec<String>,

        /// Exclude patterns
        #[arg(long)]
        exclude: Vec<String>,

        /// Show progress
        #[arg(long)]
        progress: bool,
    },

    /// List backups
    #[command(alias = "ls")]
    List {
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,

        /// Output format
        #[arg(long, default_value = "table")]
        format: String,
    },

    /// Show backup details
    Show {
        /// Backup ID or name
        backup: String,
    },

    /// Restore from backup
    Restore {
        /// Backup ID or name
        backup: String,

        /// Target directory
        #[arg(short, long)]
        target: Option<String>,

        /// Include patterns
        #[arg(long)]
        include: Vec<String>,

        /// Verify only (don't restore)
        #[arg(long)]
        verify_only: bool,
    },

    /// Check backup integrity
    Check {
        /// Read all data
        #[arg(long)]
        read_data: bool,
    },

    /// Remove old backups (apply retention)
    Forget {
        /// Show what would be deleted
        #[arg(long)]
        dry_run: bool,

        /// Apply retention policy
        #[arg(long)]
        apply: bool,
    },

    /// Show backup statistics
    Stats,

    /// Show backup configuration
    Config {
        /// Show detailed configuration
        #[arg(short, long)]
        verbose: bool,
    },

    /// Test backup connectivity
    TestConnection {
        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Control automatic backups
    Auto {
        #[arg(value_enum)]
        action: AutoBackupAction,
    },

    /// Show backup health
    Health,

    /// Export backup metrics
    Metrics {
        /// Output format
        #[arg(long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
pub enum SnapshotBackupCommands {
    /// Create snapshot and backup
    Create {
        /// Name for both snapshot and backup
        #[arg(short, long)]
        name: Option<String>,

        /// Description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Emergency restore (snapshot + backup)
    EmergencyRestore {
        /// Backup ID to restore from
        backup_id: String,

        /// Target directory
        #[arg(short, long)]
        target: Option<String>,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum AutoBackupAction {
    Enable,
    Disable,
    Status,
}
```

### 5. Core Backup Implementation

Create `src/backup/mod.rs`:

```rust
pub mod manager;
pub mod ghostsnap;
pub mod restic;
pub mod config;
pub mod storage;
pub mod retention;

pub use manager::BackupManager;
pub use config::*;

use async_trait::async_trait;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backup {
    pub id: String,
    pub name: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub size_bytes: Option<u64>,
    pub paths: Vec<String>,
    pub tags: Vec<String>,
    pub tool: BackupTool,
}

#[async_trait]
pub trait BackupBackend: Send + Sync {
    async fn init_repository(&self, repository: &str, password: &str) -> Result<()>;
    async fn create_backup(&self, paths: &[&Path], name: Option<&str>) -> Result<Backup>;
    async fn list_backups(&self) -> Result<Vec<Backup>>;
    async fn restore_backup(&self, backup_id: &str, target: &Path, include: &[String]) -> Result<()>;
    async fn check_integrity(&self, read_data: bool) -> Result<()>;
    async fn forget_backups(&self, retention: &BackupRetention, dry_run: bool) -> Result<Vec<String>>;
    async fn get_stats(&self) -> Result<BackupStats>;
    async fn test_connection(&self) -> Result<()>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupStats {
    pub total_size: u64,
    pub total_backups: u64,
    pub unique_data: u64,
    pub compression_ratio: f64,
    pub deduplication_ratio: f64,
}
```

### 6. Integration with Existing Systems

Extend `src/snapshots/mod.rs` to trigger backups:

```rust
// Add to SnapshotManager
impl SnapshotManager {
    pub async fn create_snapshot_with_backup(&self, name: Option<&str>) -> Result<(Snapshot, Option<Backup>)> {
        // Create snapshot first
        let snapshot = self.create_snapshot(name, None).await?;

        // Trigger backup if configured
        let backup = if let Some(backup_config) = &self.config.backup_integration {
            if backup_config.after_snapshot.unwrap_or(false) {
                let backup_manager = BackupManager::new(backup_config.clone()).await?;
                Some(backup_manager.create_backup(None).await?)
            } else {
                None
            }
        } else {
            None
        };

        Ok((snapshot, backup))
    }
}
```

Extend Surge commands to trigger backups:

```rust
// In surge implementation
pub async fn surge_up_with_backup(&self, services: &[String]) -> Result<()> {
    // Create backup before surge operations if configured
    if let Some(backup_config) = &self.config.backups {
        if backup_config.triggers.as_ref()
            .and_then(|t| t.before_surge_operations)
            .unwrap_or(false) {
            let backup_manager = BackupManager::new(backup_config.clone()).await?;
            backup_manager.create_backup(Some("before-surge-up")).await?;
        }
    }

    // Perform surge up
    self.surge_up(services).await?;

    Ok(())
}
```

### 7. Environment Variable Support

Add environment variable handling in `src/config/mod.rs`:

```rust
impl BackupConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: Some(true),
            tool: std::env::var("BOLT_BACKUP_TOOL")
                .ok()
                .and_then(|s| match s.as_str() {
                    "ghostsnap" => Some(BackupTool::GhostSnap),
                    "restic" => Some(BackupTool::Restic),
                    _ => None,
                }),
            repository: std::env::var("BOLT_BACKUP_REPOSITORY").ok(),
            password_file: std::env::var("BOLT_BACKUP_PASSWORD_FILE").ok(),
            // ... other fields
        }
    }
}
```

### 8. Error Handling

Create backup-specific errors in `src/backup/mod.rs`:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackupError {
    #[error("Backup repository not found: {0}")]
    RepositoryNotFound(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Storage backend error: {0}")]
    StorageError(String),

    #[error("Backup tool not available: {0}")]
    ToolNotAvailable(String),

    #[error("Invalid backup configuration: {0}")]
    InvalidConfig(String),

    #[error("Backup operation failed: {0}")]
    OperationFailed(String),
}
```

### 9. Testing Requirements

Add integration tests in `tests/backup_integration.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_ghostsnap_backup_creation() {
        // Test ghostsnap backup creation
    }

    #[tokio::test]
    async fn test_restic_backup_creation() {
        // Test restic backup creation
    }

    #[tokio::test]
    async fn test_snapshot_backup_integration() {
        // Test snapshot + backup workflow
    }

    #[tokio::test]
    async fn test_backup_retention() {
        // Test retention policy application
    }
}
```

### 10. Documentation Updates

Update existing documentation:

1. **README.md** - Add backup capabilities to overview and features
2. **docs/CLI.md** - Document new backup commands
3. **examples/** - Add Boltfile examples with backup configuration

### 11. Feature Flags

Ensure backup functionality is properly gated behind feature flags:

```rust
// In main.rs and relevant modules
#[cfg(feature = "backup-support")]
use crate::backup::BackupManager;

#[cfg(feature = "ghostsnap-support")]
use crate::backup::ghostsnap::GhostSnapBackend;

#[cfg(feature = "restic-support")]
use crate::backup::restic::ResticBackend;
```

## Implementation Priority

### Phase 1: Core Infrastructure
1. ✅ Configuration schema updates
2. ✅ CLI command structure
3. ✅ Basic backup trait and manager
4. ✅ Error handling

### Phase 2: Ghostsnap Integration
1. ✅ Ghostsnap backend implementation
2. ✅ Repository initialization
3. ✅ Basic backup/restore operations
4. ✅ Testing

### Phase 3: Restic Integration
1. ✅ Restic backend implementation (subprocess-based)
2. ✅ Command translation layer
3. ✅ Error handling and parsing
4. ✅ Testing

### Phase 4: Advanced Features
1. ✅ Snapshot + backup integration
2. ✅ Retention policies
3. ✅ Performance optimization
4. ✅ Monitoring and metrics

### Phase 5: Polish & Documentation
1. ✅ Comprehensive testing
2. ✅ Documentation updates
3. ✅ Example configurations
4. ✅ Migration tools

## Dependencies on Ghostsnap

### Required Ghostsnap Features
1. **Library API** - Ghostsnap should expose a Rust library API (not just CLI)
2. **Bolt Integration** - Add Bolt-specific features to ghostsnap
3. **Repository Format** - Ensure compatibility with Bolt's data structures
4. **Configuration** - Support Bolt's configuration format

### Recommended Ghostsnap Enhancements
1. **Progress Callbacks** - For backup progress reporting
2. **Metadata Hooks** - For storing Bolt-specific metadata
3. **Plugin System** - For Bolt-specific extensions
4. **Performance Metrics** - For monitoring integration

These requirements ensure seamless integration between Bolt and ghostsnap while maintaining the flexibility to support restic as an alternative backend.