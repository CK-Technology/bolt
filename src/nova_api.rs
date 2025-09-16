//! Nova Integration API - Programmatic access to Bolt runtime for Nova VM Manager
//!
//! This module provides a clean, async API for Nova to integrate Bolt containers
//! alongside KVM/QEMU virtual machines.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::capsules::{CapsuleConfig, CapsuleManager, CapsuleStatus};
use crate::runtime;

/// Nova-compatible runtime handle for Bolt
///
/// This provides async operations that integrate with Nova's tokio runtime
#[derive(Clone)]
pub struct BoltNovaRuntime {
    inner: Arc<RwLock<BoltRuntimeInner>>,
}

struct BoltRuntimeInner {
    capsule_manager: CapsuleManager,
    active_capsules: HashMap<String, CapsuleHandle>,
}

/// Handle to a running capsule with lifecycle management
#[derive(Clone)]
pub struct CapsuleHandle {
    pub id: String,
    pub name: String,
    pub status: CapsuleStatus,
    pub pid: Option<u32>,
}

/// Configuration from Nova's TOML format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovaContainerConfig {
    pub capsule: String,  // Image name
    pub volumes: Vec<String>,
    pub network: String,
    pub env: HashMap<String, String>,
    pub gpu_passthrough: bool,
    pub memory_mb: Option<u64>,
    pub cpus: Option<u32>,
}

/// Unified status for Nova's GUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NovaStatus {
    Running,
    Stopped,
    Starting,
    Error(String),
}

/// Resource metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub memory_limit_mb: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
}

impl BoltNovaRuntime {
    /// Create a new Bolt runtime for Nova integration
    pub async fn new() -> Result<Self> {
        let capsule_manager = CapsuleManager {
            root_path: PathBuf::from("/var/lib/bolt/capsules"),
            capsules: HashMap::new(),
            templates: HashMap::new(),
        };

        Ok(Self {
            inner: Arc::new(RwLock::new(BoltRuntimeInner {
                capsule_manager,
                active_capsules: HashMap::new(),
            })),
        })
    }

    /// Start a capsule from Nova configuration
    pub async fn start_capsule(&self, name: &str, config: &NovaContainerConfig) -> Result<CapsuleHandle> {
        info!("Starting capsule '{}' from Nova config", name);

        // Convert Nova config to Bolt capsule config
        let _capsule_config = self.nova_to_bolt_config(config)?;

        // Create environment variables
        let env_vars: Vec<String> = config.env
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        // Create volume mappings
        let volumes = config.volumes.clone();

        // Determine if we need GPU support
        let _gpu_support = if config.gpu_passthrough {
            vec!["--gpus".to_string(), "all".to_string()]
        } else {
            vec![]
        };

        // Run the container
        runtime::run_container(
            &config.capsule,
            Some(name),
            &[],  // ports - will be configured via network
            &env_vars,
            &volumes,
            true,  // detach for background running
        ).await?;

        // Get container info
        let containers = runtime::list_containers_info(false).await?;
        let container = containers
            .iter()
            .find(|c| c.name == name)
            .ok_or_else(|| anyhow::anyhow!("Container not found after creation"))?;

        // Create handle
        let handle = CapsuleHandle {
            id: container.id.clone(),
            name: name.to_string(),
            status: CapsuleStatus::Running,
            pid: None,  // Will be populated by monitoring
        };

        // Store in active capsules
        let mut inner = self.inner.write().await;
        inner.active_capsules.insert(name.to_string(), handle.clone());

        Ok(handle)
    }

    /// Stop a running capsule
    pub async fn stop_capsule(&self, name: &str) -> Result<()> {
        info!("Stopping capsule '{}'", name);

        runtime::stop_container(name).await?;

        let mut inner = self.inner.write().await;
        if let Some(handle) = inner.active_capsules.get_mut(name) {
            handle.status = CapsuleStatus::Stopped;
        }

        Ok(())
    }

    /// List all capsules
    pub async fn list_capsules(&self) -> Result<Vec<CapsuleHandle>> {
        let containers = runtime::list_containers_info(true).await?;

        let mut capsules = Vec::new();
        for container in containers {
            let status = if container.status == "running" {
                CapsuleStatus::Running
            } else if container.status == "exited" {
                CapsuleStatus::Stopped
            } else {
                CapsuleStatus::Created
            };

            capsules.push(CapsuleHandle {
                id: container.id,
                name: container.name.clone(),
                status,
                pid: None,
            });
        }

        Ok(capsules)
    }

    /// Get capsule status
    pub async fn get_capsule_status(&self, name: &str) -> Result<NovaStatus> {
        let containers = runtime::list_containers_info(true).await?;

        let container = containers
            .iter()
            .find(|c| c.name == name);

        match container {
            Some(c) if c.status == "running" => Ok(NovaStatus::Running),
            Some(c) if c.status == "exited" => Ok(NovaStatus::Stopped),
            Some(c) if c.status == "created" => Ok(NovaStatus::Starting),
            Some(_) => Ok(NovaStatus::Error("Unknown state".to_string())),
            None => Ok(NovaStatus::Stopped),
        }
    }

    /// Get resource metrics for a capsule
    pub async fn get_capsule_metrics(&self, _name: &str) -> Result<CapsuleMetrics> {
        // This would integrate with runtime metrics collection
        // For now, return placeholder data
        Ok(CapsuleMetrics {
            cpu_usage_percent: 0.0,
            memory_usage_mb: 0,
            memory_limit_mb: 0,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
        })
    }

    /// Get logs for a capsule
    pub async fn get_capsule_logs(&self, _name: &str, _lines: usize) -> Result<Vec<String>> {
        // This would integrate with log collection
        // For now, return empty
        Ok(Vec::new())
    }

    /// Restart a capsule
    pub async fn restart_capsule(&self, name: &str) -> Result<()> {
        // Store the handle before stopping
        let _handle = {
            let inner = self.inner.read().await;
            inner.active_capsules.get(name).cloned()
        };

        self.stop_capsule(name).await?;

        // Wait a moment for clean shutdown
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Start the container again
        // This would ideally use the stored configuration
        // For now, we'll need Nova to provide the config again

        Ok(())
    }

    /// Remove a capsule
    pub async fn remove_capsule(&self, name: &str, force: bool) -> Result<()> {
        info!("Removing capsule '{}'", name);

        runtime::remove_container(name, force).await?;

        let mut inner = self.inner.write().await;
        inner.active_capsules.remove(name);

        Ok(())
    }

    /// Connect capsule to Nova network
    pub async fn connect_to_network(&self, capsule: &str, network: &str) -> Result<()> {
        // This will integrate with Nova's bridge networks
        info!("Connecting capsule '{}' to network '{}'", capsule, network);

        // Placeholder for network connection logic
        // This would use the bridge manager to connect to Nova's networks

        Ok(())
    }

    /// Configure GPU passthrough for gaming
    pub async fn configure_gpu_passthrough(&self, capsule: &str, _gpu_id: &str) -> Result<()> {
        info!("Configuring GPU passthrough for capsule '{}'", capsule);

        // This would configure GPU passthrough
        // Using NVIDIA or AMD runtime extensions

        Ok(())
    }

    /// Convert Nova config to Bolt config
    fn nova_to_bolt_config(&self, nova_config: &NovaContainerConfig) -> Result<CapsuleConfig> {
        let resources = crate::capsules::CapsuleResources {
            memory_mb: nova_config.memory_mb.unwrap_or(2048),
            vcpus: nova_config.cpus.unwrap_or(2),
            cpu_shares: 1024,
            memory_balloon: false,
            cpu_hotplug: false,
            numa_topology: None,
        };

        let networking = crate::capsules::CapsuleNetworking {
            network_type: crate::capsules::NetworkType::Bridge,
            interfaces: vec![],
            dns_config: crate::capsules::DnsConfig {
                servers: vec!["8.8.8.8".to_string()],
                search_domains: vec![],
                bolt_dns_enabled: true,
            },
            firewall_rules: vec![],
        };

        let storage = crate::capsules::CapsuleStorage {
            root_disk: crate::capsules::DiskConfig {
                name: "root".to_string(),
                size_gb: 20,
                disk_type: crate::capsules::DiskType::SSD,
                encryption: false,
                compression: false,
                cache_policy: crate::capsules::CachePolicy::WriteBack,
            },
            data_disks: vec![],
            shared_folders: nova_config.volumes
                .iter()
                .filter_map(|v| {
                    let parts: Vec<&str> = v.split(':').collect();
                    if parts.len() == 2 {
                        Some(crate::capsules::SharedFolder {
                            host_path: parts[0].to_string(),
                            capsule_path: parts[1].to_string(),
                            readonly: false,
                            auto_mount: true,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            snapshot_policy: crate::capsules::SnapshotPolicy {
                auto_snapshot: false,
                interval_minutes: 60,
                max_snapshots: 10,
                compress_snapshots: true,
            },
        };

        let security = crate::capsules::CapsuleSecurity {
            isolation_level: if nova_config.gpu_passthrough {
                crate::capsules::IsolationLevel::Gaming
            } else {
                crate::capsules::IsolationLevel::Container
            },
            privilege_mode: crate::capsules::PrivilegeMode::Unprivileged,
            allowed_syscalls: vec![],
            device_permissions: vec![],
            mandatory_access_control: false,
        };

        let gaming = if nova_config.gpu_passthrough {
            Some(crate::capsules::GamingCapsuleConfig {
                gpu_passthrough: true,
                audio_passthrough: true,
                input_devices: vec![],
                display_server: crate::capsules::DisplayServer::Wayland,
                performance_mode: crate::capsules::PerformanceMode::Gaming,
                anti_cheat_compat: false,
                steam_integration: false,
                wine_config: None,
            })
        } else {
            None
        };

        Ok(CapsuleConfig {
            template: None,
            image: nova_config.capsule.clone(),
            resources,
            networking,
            storage,
            security,
            gaming,
        })
    }
}

/// Error type for Nova integration
#[derive(Debug, thiserror::Error)]
pub enum NovaError {
    #[error("Bolt runtime error: {0}")]
    BoltError(#[from] anyhow::Error),

    #[error("Capsule not found: {0}")]
    CapsuleNotFound(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("GPU configuration error: {0}")]
    GpuError(String),
}

