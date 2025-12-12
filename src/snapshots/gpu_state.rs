//! GPU State Snapshot Support
//!
//! Integrates nvbind's GPU snapshot functionality with Bolt's snapshot system
//! to capture and restore GPU configuration state across snapshots.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info, warn};

/// GPU state captured in a snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuSnapshotState {
    /// Timestamp when GPU state was captured
    pub captured_at: chrono::DateTime<chrono::Utc>,
    /// GPU device states (clocks, temps, power)
    pub device_states: Vec<GpuDeviceState>,
    /// Driver information
    pub driver_info: Option<DriverInfo>,
    /// Performance settings
    pub performance_settings: Option<PerformanceSettings>,
    /// Memory allocations summary
    pub memory_allocations: Option<MemoryAllocations>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuDeviceState {
    pub device_id: String,
    pub device_name: String,
    pub gpu_clock_mhz: u32,
    pub memory_clock_mhz: u32,
    pub temperature_c: u32,
    pub power_draw_w: u32,
    pub power_limit_w: u32,
    pub fan_speed_percent: u32,
    pub performance_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverInfo {
    pub driver_version: String,
    pub cuda_version: Option<String>,
    pub driver_type: String, // "nvidia-proprietary", "nvidia-open", etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    pub power_profile: String,
    pub clock_offset_mhz: i32,
    pub memory_offset_mhz: i32,
    pub persistence_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAllocations {
    pub total_vram_mb: u64,
    pub allocated_vram_mb: u64,
    pub free_vram_mb: u64,
}

/// GPU Snapshot Manager - integrates with nvbind
#[derive(Debug)]
pub struct GpuSnapshotManager {
    snapshot_root: std::path::PathBuf,
    #[cfg(feature = "nvbind-support")]
    nvbind_manager: Option<nvbind::snapshot::GpuSnapshotManager>,
}

impl GpuSnapshotManager {
    /// Create a new GPU snapshot manager
    pub fn new(snapshot_root: impl AsRef<Path>) -> Result<Self> {
        let snapshot_root = snapshot_root.as_ref().to_path_buf();

        #[cfg(feature = "nvbind-support")]
        let nvbind_manager = {
            match nvbind::snapshot::GpuSnapshotManager::new(&snapshot_root) {
                Ok(manager) => {
                    info!("✅ nvbind GPU snapshot manager initialized");
                    Some(manager)
                }
                Err(e) => {
                    warn!("⚠️ Failed to initialize nvbind snapshot manager: {}", e);
                    warn!("   GPU state snapshots will use fallback mode");
                    None
                }
            }
        };

        Ok(Self {
            snapshot_root,
            #[cfg(feature = "nvbind-support")]
            nvbind_manager,
        })
    }

    /// Capture current GPU state for snapshot
    pub async fn capture_gpu_state(&self, snapshot_id: &str) -> Result<GpuSnapshotState> {
        info!("📸 Capturing GPU state for snapshot: {}", snapshot_id);

        #[cfg(feature = "nvbind-support")]
        {
            if let Some(ref manager) = self.nvbind_manager {
                return self.capture_with_nvbind(manager, snapshot_id).await;
            }
        }

        // Fallback: capture basic GPU state via nvidia-smi
        self.capture_fallback(snapshot_id).await
    }

    #[cfg(feature = "nvbind-support")]
    async fn capture_with_nvbind(
        &self,
        manager: &nvbind::snapshot::GpuSnapshotManager,
        snapshot_id: &str,
    ) -> Result<GpuSnapshotState> {
        debug!("Using nvbind to capture GPU state");

        // Create nvbind snapshot
        let nvbind_snapshot = manager
            .create_snapshot(snapshot_id)
            .await
            .context("Failed to create nvbind GPU snapshot")?;

        // Convert nvbind snapshot to our format
        let device_states = nvbind_snapshot
            .device_states
            .iter()
            .map(|device| GpuDeviceState {
                device_id: device.device_id.clone(),
                device_name: device.device_name.clone(),
                gpu_clock_mhz: device.gpu_clock_mhz,
                memory_clock_mhz: device.memory_clock_mhz,
                temperature_c: device.temperature_c,
                power_draw_w: device.power_draw_w,
                power_limit_w: device.power_limit_w,
                fan_speed_percent: device.fan_speed_percent,
                performance_mode: device.performance_mode.clone(),
            })
            .collect();

        let driver_info = nvbind_snapshot
            .driver_state
            .as_ref()
            .map(|driver| DriverInfo {
                driver_version: driver.version.clone(),
                cuda_version: driver.cuda_version.clone(),
                driver_type: driver.driver_type.clone(),
            });

        let performance_settings =
            nvbind_snapshot
                .performance_settings
                .as_ref()
                .map(|perf| PerformanceSettings {
                    power_profile: perf.power_profile.clone(),
                    clock_offset_mhz: perf.clock_offset_mhz,
                    memory_offset_mhz: perf.memory_offset_mhz,
                    persistence_mode: perf.persistence_mode,
                });

        let memory_allocations =
            nvbind_snapshot
                .memory_state
                .as_ref()
                .map(|mem| MemoryAllocations {
                    total_vram_mb: mem.total_vram_mb,
                    allocated_vram_mb: mem.allocated_vram_mb,
                    free_vram_mb: mem.free_vram_mb,
                });

        info!(
            "✅ Captured GPU state with {} device(s)",
            device_states.len()
        );

        Ok(GpuSnapshotState {
            captured_at: chrono::Utc::now(),
            device_states,
            driver_info,
            performance_settings,
            memory_allocations,
        })
    }

    async fn capture_fallback(&self, _snapshot_id: &str) -> Result<GpuSnapshotState> {
        debug!("Using fallback GPU state capture (nvidia-smi)");

        use tokio::process::Command;

        // Query GPU state via nvidia-smi
        let output = Command::new("nvidia-smi")
            .args([
                "--query-gpu=index,name,clocks.gr,clocks.mem,temperature.gpu,power.draw,power.limit,fan.speed",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .await
            .context("Failed to query GPU state")?;

        if !output.status.success() {
            warn!("nvidia-smi query failed, creating empty GPU state");
            return Ok(GpuSnapshotState {
                captured_at: chrono::Utc::now(),
                device_states: Vec::new(),
                driver_info: None,
                performance_settings: None,
                memory_allocations: None,
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut device_states = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 8 {
                device_states.push(GpuDeviceState {
                    device_id: format!("gpu:{}", parts[0]),
                    device_name: parts[1].to_string(),
                    gpu_clock_mhz: parts[2].parse().unwrap_or(0),
                    memory_clock_mhz: parts[3].parse().unwrap_or(0),
                    temperature_c: parts[4].parse().unwrap_or(0),
                    power_draw_w: parts[5].parse().unwrap_or(0),
                    power_limit_w: parts[6].parse().unwrap_or(0),
                    fan_speed_percent: parts[7].parse().unwrap_or(0),
                    performance_mode: "unknown".to_string(),
                });
            }
        }

        // Get driver info
        let driver_output = Command::new("nvidia-smi")
            .args(["--query-gpu=driver_version", "--format=csv,noheader"])
            .output()
            .await;

        let driver_info = if let Ok(output) = driver_output {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !version.is_empty() {
                Some(DriverInfo {
                    driver_version: version,
                    cuda_version: None,
                    driver_type: "unknown".to_string(),
                })
            } else {
                None
            }
        } else {
            None
        };

        info!(
            "✅ Captured GPU state (fallback) with {} device(s)",
            device_states.len()
        );

        Ok(GpuSnapshotState {
            captured_at: chrono::Utc::now(),
            device_states,
            driver_info,
            performance_settings: None,
            memory_allocations: None,
        })
    }

    /// Restore GPU state from snapshot
    pub async fn restore_gpu_state(
        &self,
        snapshot_id: &str,
        gpu_state: &GpuSnapshotState,
    ) -> Result<()> {
        info!("🔄 Restoring GPU state from snapshot: {}", snapshot_id);

        #[cfg(feature = "nvbind-support")]
        {
            if let Some(ref manager) = self.nvbind_manager {
                return self
                    .restore_with_nvbind(manager, snapshot_id)
                    .await
                    .context("Failed to restore GPU state with nvbind");
            }
        }

        // Fallback: restore what we can via nvidia-smi
        self.restore_fallback(gpu_state).await
    }

    #[cfg(feature = "nvbind-support")]
    async fn restore_with_nvbind(
        &self,
        manager: &nvbind::snapshot::GpuSnapshotManager,
        snapshot_id: &str,
    ) -> Result<()> {
        debug!("Using nvbind to restore GPU state");

        manager
            .restore_snapshot(snapshot_id)
            .await
            .context("nvbind GPU state restore failed")?;

        info!("✅ GPU state restored successfully via nvbind");
        Ok(())
    }

    async fn restore_fallback(&self, gpu_state: &GpuSnapshotState) -> Result<()> {
        debug!("Using fallback GPU state restore");

        // Fallback mode: log what would be restored
        // In a full implementation, this would use nvidia-smi to set clocks, power limits, etc.
        for device in &gpu_state.device_states {
            info!(
                "  • {} ({}): {}MHz GPU, {}MHz mem, {}W power",
                device.device_name,
                device.device_id,
                device.gpu_clock_mhz,
                device.memory_clock_mhz,
                device.power_limit_w
            );
        }

        warn!("⚠️  Fallback mode: GPU state restore is informational only");
        warn!("   For full GPU state restoration, enable nvbind-support feature");

        Ok(())
    }

    /// Save GPU state to disk
    pub async fn save_gpu_state(
        &self,
        snapshot_id: &str,
        gpu_state: &GpuSnapshotState,
    ) -> Result<()> {
        let state_path = self
            .snapshot_root
            .join(format!("{}.gpu-state.json", snapshot_id));
        let json =
            serde_json::to_string_pretty(gpu_state).context("Failed to serialize GPU state")?;

        tokio::fs::write(&state_path, json)
            .await
            .context("Failed to write GPU state file")?;

        debug!("GPU state saved to: {}", state_path.display());
        Ok(())
    }

    /// Load GPU state from disk
    pub async fn load_gpu_state(&self, snapshot_id: &str) -> Result<Option<GpuSnapshotState>> {
        let state_path = self
            .snapshot_root
            .join(format!("{}.gpu-state.json", snapshot_id));

        if !state_path.exists() {
            debug!("No GPU state file found for snapshot: {}", snapshot_id);
            return Ok(None);
        }

        let json = tokio::fs::read_to_string(&state_path)
            .await
            .context("Failed to read GPU state file")?;

        let gpu_state: GpuSnapshotState =
            serde_json::from_str(&json).context("Failed to deserialize GPU state")?;

        debug!("GPU state loaded from: {}", state_path.display());
        Ok(Some(gpu_state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gpu_state_serialization() {
        let state = GpuSnapshotState {
            captured_at: chrono::Utc::now(),
            device_states: vec![GpuDeviceState {
                device_id: "gpu:0".to_string(),
                device_name: "NVIDIA RTX 4090".to_string(),
                gpu_clock_mhz: 2520,
                memory_clock_mhz: 10501,
                temperature_c: 65,
                power_draw_w: 350,
                power_limit_w: 450,
                fan_speed_percent: 75,
                performance_mode: "performance".to_string(),
            }],
            driver_info: Some(DriverInfo {
                driver_version: "550.54.14".to_string(),
                cuda_version: Some("12.4".to_string()),
                driver_type: "nvidia-proprietary".to_string(),
            }),
            performance_settings: None,
            memory_allocations: None,
        };

        let json = serde_json::to_string_pretty(&state).unwrap();
        let deserialized: GpuSnapshotState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.device_states.len(), 1);
        assert_eq!(deserialized.device_states[0].device_name, "NVIDIA RTX 4090");
    }
}
