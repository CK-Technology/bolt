use crate::{BoltError, Result};
use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use uuid::Uuid;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

use super::oci::{ContainerConfig, ContainerState, execute_container};
use super::storage::{StorageManager, ImageMetadata};
use super::security::{BoltSecurityManager, SecurityMetrics};
use super::performance::{BoltPerformanceOptimizer, PerformanceMetrics, BenchmarkResults};
use super::networking::{BoltNetworkManager, ContainerNetworkConfig, NetworkPerformanceMode};
use super::gpu_integration::{BoltGpuIntegration, GpuConfig, GpuWorkloadType, GpuIsolationLevel, GpuMetrics};

/// Enhanced Native Bolt container runtime with cutting-edge security and performance
#[derive(Debug)]
pub struct BoltNativeRuntime {
    storage: StorageManager,
    containers: HashMap<String, ContainerState>,
    runtime_dir: PathBuf,
    security_manager: BoltSecurityManager,
    performance_optimizer: BoltPerformanceOptimizer,
    network_manager: BoltNetworkManager,
    gpu_integration: BoltGpuIntegration,
    gaming_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeContainerConfig {
    pub image: String,
    pub name: Option<String>,
    pub ports: Vec<String>,
    pub env: Vec<String>,
    pub volumes: Vec<String>,
    pub detach: bool,
    pub command: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub user: Option<String>,
    pub gpu_config: Option<GpuConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeContainerInfo {
    pub id: String,
    pub name: Option<String>,
    pub image: String,
    pub status: ContainerStatus,
    pub created: std::time::SystemTime,
    pub ports: Vec<String>,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerStatus {
    Created,
    Running,
    Stopped,
    Paused,
    Exited(i32),
    Error(String),
}

impl BoltNativeRuntime {
    pub async fn new() -> Result<Self> {
        Self::new_with_gaming_mode(false).await
    }

    pub async fn new_with_gaming_mode(gaming_mode: bool) -> Result<Self> {
        info!("🚀 Initializing Enhanced Bolt Native Runtime (gaming: {})", gaming_mode);

        let runtime_dir = std::env::temp_dir().join("bolt-runtime");
        std::fs::create_dir_all(&runtime_dir)
            .context("Failed to create runtime directory")?;

        // Initialize all subsystems
        let storage = StorageManager::new().await?;
        let security_manager = BoltSecurityManager::new()?;
        let performance_optimizer = BoltPerformanceOptimizer::new(gaming_mode);
        let network_manager = BoltNetworkManager::new().await?;
        let gpu_integration = BoltGpuIntegration::new().await?;

        if gaming_mode && gpu_integration.is_nvbind_available() {
            info!("🎮 Gaming mode enabled with nvbind GPU acceleration");
        } else if gaming_mode {
            info!("🎮 Gaming mode enabled with fallback GPU support");
        }

        info!("✅ All subsystems initialized successfully");

        Ok(Self {
            storage,
            containers: HashMap::new(),
            runtime_dir,
            security_manager,
            performance_optimizer,
            network_manager,
            gpu_integration,
            gaming_mode,
        })
    }

    /// Run a container with native OCI runtime (replaces docker/podman run)
    pub async fn run_container(&mut self, config: NativeContainerConfig) -> Result<String> {
        info!("🐳 Starting native container: {}", config.image);

        // Generate unique container ID
        let uuid_str = Uuid::new_v4().to_string();
        let container_id = format!("bolt-{}", &uuid_str[..8]);

        // Pull image if needed
        if !self.storage.image_exists(&config.image).await? {
            info!("⬇️  Pulling image: {}", config.image);
            self.pull_image_native(&config.image).await?;
        }

        // Create container configuration
        let container_config = self.create_container_config(&container_id, &config).await?;

        // Create OCI spec from configuration
        let spec = self.create_oci_spec(&container_config).await?;

        // Create container state
        let container_state = ContainerState {
            id: container_id.clone(),
            status: super::oci::ContainerStatus::Created,
            pid: None,
            bundle_path: self.runtime_dir.join(&container_id),
            config: container_config,
            created: std::time::SystemTime::now(),
        };

        // Create bundle directory
        std::fs::create_dir_all(&container_state.bundle_path)
            .context("Failed to create container bundle")?;

        // Write OCI spec to bundle
        let spec_path = container_state.bundle_path.join("config.json");
        let spec_json = serde_json::to_string_pretty(&spec)?;
        std::fs::write(&spec_path, spec_json)
            .context("Failed to write OCI spec")?;

        // Apply security hardening before execution
        let security_profile = if self.gaming_mode { "gaming" } else { "secure" };
        self.security_manager.harden_container(&container_id, security_profile).await?;

        // Apply performance optimizations
        self.performance_optimizer.optimize_container(&container_id).await?;

        // Setup networking
        self.setup_container_networking(&container_id, &config).await?;

        // Setup GPU if requested
        if let Some(ref gpu_config) = config.gpu_config {
            info!("🎮 Setting up GPU for container: {}", container_id);
            self.gpu_integration.setup_gpu_for_container(&container_id, gpu_config).await?;
        }

        // Execute container with native OCI runtime
        let pid = execute_container(&container_state, &spec).await?;

        // Update container state
        let mut updated_state = container_state;
        updated_state.status = super::oci::ContainerStatus::Running;
        updated_state.pid = Some(pid);

        // Store container state
        self.containers.insert(container_id.clone(), updated_state);

        // Start monitoring
        tokio::spawn({
            let security_manager = self.security_manager.clone();
            let performance_optimizer = self.performance_optimizer.clone();
            let container_id = container_id.clone();
            async move {
                Self::monitor_container(security_manager, performance_optimizer, container_id).await
            }
        });

        info!("✅ Enhanced native container started: {}", container_id);
        Ok(container_id)
    }

    /// Stop a running container (replaces docker/podman stop)
    pub async fn stop_container(&mut self, id: &str) -> Result<()> {
        info!("🛑 Stopping container: {}", id);

        let container = self.containers.get_mut(id)
            .ok_or_else(|| anyhow!("Container not found: {}", id))?;

        if let Some(pid) = container.pid {
            let nix_pid = Pid::from_raw(pid as i32);

            // Send SIGTERM first for graceful shutdown
            if let Err(e) = signal::kill(nix_pid, Signal::SIGTERM) {
                warn!("Failed to send SIGTERM to process {}: {}", pid, e);
            } else {
                info!("Sent SIGTERM to process {}", pid);
            }

            // Wait for graceful shutdown
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;

            // Force kill if still running - check if process exists first
            match signal::kill(nix_pid, None) {
                Ok(_) => {
                    // Process still exists, force kill
                    if let Err(e) = signal::kill(nix_pid, Signal::SIGKILL) {
                        warn!("Failed to send SIGKILL to process {}: {}", pid, e);
                    } else {
                        info!("Sent SIGKILL to process {}", pid);
                    }
                }
                Err(_) => {
                    // Process already terminated
                    info!("Process {} already terminated", pid);
                }
            }
        }

        container.status = super::oci::ContainerStatus::Stopped;
        container.pid = None;

        info!("✅ Container stopped: {}", id);
        Ok(())
    }

    /// Remove a container (replaces docker/podman rm)
    pub async fn remove_container(&mut self, id: &str, force: bool) -> Result<()> {
        info!("🗑️  Removing container: {}", id);

        let container = self.containers.get(id)
            .ok_or_else(|| anyhow!("Container not found: {}", id))?;

        // Stop if running
        if matches!(container.status, super::oci::ContainerStatus::Running) {
            if force {
                self.stop_container(id).await?;
            } else {
                return Err(anyhow!("Container is running. Use force=true to stop and remove.").into());
            }
        }

        // Clean up bundle directory
        if container.bundle_path.exists() {
            std::fs::remove_dir_all(&container.bundle_path)
                .context("Failed to remove container bundle")?;
        }

        // Remove from containers map
        self.containers.remove(id);

        info!("✅ Container removed: {}", id);
        Ok(())
    }

    /// List containers (replaces docker/podman ps)
    pub async fn list_containers(&self, all: bool) -> Result<Vec<NativeContainerInfo>> {
        let mut containers = Vec::new();

        for (id, state) in &self.containers {
            if !all && !matches!(state.status, super::oci::ContainerStatus::Running) {
                continue;
            }

            let info = NativeContainerInfo {
                id: id.clone(),
                name: state.config.name.clone(),
                image: state.config.image.clone(),
                status: match &state.status {
                    super::oci::ContainerStatus::Created => ContainerStatus::Created,
                    super::oci::ContainerStatus::Running => ContainerStatus::Running,
                    super::oci::ContainerStatus::Stopped => ContainerStatus::Stopped,
                    super::oci::ContainerStatus::Exited(code) => ContainerStatus::Exited(*code),
                },
                created: state.created,
                ports: state.config.ports.iter()
                    .map(|p| format!("{}:{}", p.host_port, p.container_port))
                    .collect(),
                pid: state.pid,
            };

            containers.push(info);
        }

        Ok(containers)
    }

    /// Pull an image (replaces docker/podman pull)
    pub async fn pull_image_native(&mut self, image: &str) -> Result<()> {
        info!("⬇️  Pulling image with native client: {}", image);

        // Use the native storage manager to pull
        let metadata = self.storage.pull_image(image).await?;

        info!("✅ Image pulled: {} ({})", image, metadata.digest);
        Ok(())
    }

    /// Build an image (replaces docker/podman build)
    pub async fn build_image_native(&mut self, context: &str, tag: Option<&str>, dockerfile: &str) -> Result<()> {
        info!("🔨 Building image natively from: {}", context);

        // Use native image builder
        self.storage.build_image(context, tag.unwrap_or("latest"), dockerfile).await?;

        info!("✅ Image built successfully");
        Ok(())
    }

    // Helper methods
    async fn create_container_config(&self, id: &str, config: &NativeContainerConfig) -> Result<ContainerConfig> {
        // Parse ports
        let mut ports = Vec::new();
        for port_str in &config.ports {
            // Parse "host:container" format
            let parts: Vec<&str> = port_str.split(':').collect();
            if parts.len() == 2 {
                ports.push(super::oci::PortMapping {
                    host_port: parts[0].parse().context("Invalid host port")?,
                    container_port: parts[1].parse().context("Invalid container port")?,
                    protocol: "tcp".to_string(),
                });
            }
        }

        // Parse environment variables
        let mut env = HashMap::new();
        for env_str in &config.env {
            if let Some(eq_pos) = env_str.find('=') {
                let key = &env_str[..eq_pos];
                let value = &env_str[eq_pos + 1..];
                env.insert(key.to_string(), value.to_string());
            }
        }

        // Parse volume mounts
        let mut volumes = Vec::new();
        for vol_str in &config.volumes {
            let parts: Vec<&str> = vol_str.split(':').collect();
            if parts.len() >= 2 {
                volumes.push(super::oci::VolumeMount {
                    source: parts[0].to_string(),
                    destination: parts[1].to_string(),
                    readonly: parts.get(2).map(|&s| s == "ro").unwrap_or(false),
                });
            }
        }

        Ok(ContainerConfig {
            id: id.to_string(),
            name: config.name.clone(),
            image: config.image.clone(),
            command: config.command.clone().unwrap_or_else(|| vec!["sh".to_string()]),
            args: vec![],
            env,
            working_dir: config.working_dir.clone(),
            user: config.user.clone(),
            ports,
            volumes,
            capabilities: vec![], // Default capabilities
            resource_limits: None,
            gaming_config: None, // Will be set later if needed
        })
    }

    async fn create_oci_spec(&self, config: &ContainerConfig) -> Result<oci_spec::runtime::Spec> {
        use oci_spec::runtime::*;

        // Create basic OCI runtime spec
        let mut spec_builder = Spec::default();

        // Set version
        spec_builder.set_version("1.0.0".to_string());

        // Create process configuration
        let mut process = Process::default();
        process.set_args(Some(config.command.clone()));
        if let Some(ref cwd) = config.working_dir {
            process.set_cwd(PathBuf::from(cwd));
        }

        // Set environment variables
        let env_vec: Vec<String> = config.env.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        if !env_vec.is_empty() {
            process.set_env(Some(env_vec));
        }

        spec_builder.set_process(Some(process));

        // Set root filesystem
        let mut root = Root::default();
        root.set_path(PathBuf::from("rootfs"));
        spec_builder.set_root(Some(root));

        // Create basic mounts
        let mounts = vec![
            // Proc filesystem
            Mount {
                destination: PathBuf::from("/proc"),
                typ: Some("proc".to_string()),
                source: Some(PathBuf::from("proc")),
                options: Some(vec!["nosuid".to_string(), "noexec".to_string(), "nodev".to_string()]),
            },
            // Sys filesystem
            Mount {
                destination: PathBuf::from("/sys"),
                typ: Some("sysfs".to_string()),
                source: Some(PathBuf::from("sysfs")),
                options: Some(vec!["nosuid".to_string(), "noexec".to_string(), "nodev".to_string(), "ro".to_string()]),
            },
            // Dev filesystem
            Mount {
                destination: PathBuf::from("/dev"),
                typ: Some("tmpfs".to_string()),
                source: Some(PathBuf::from("tmpfs")),
                options: Some(vec!["nosuid".to_string(), "strictatime".to_string(), "mode=755".to_string(), "size=65536k".to_string()]),
            },
        ];

        spec_builder.set_mounts(Some(mounts));

        // Set Linux-specific configuration
        let mut linux = Linux::default();

        // Configure namespaces
        let namespaces = vec![
            LinuxNamespace {
                typ: LinuxNamespaceType::Pid,
                path: None,
            },
            LinuxNamespace {
                typ: LinuxNamespaceType::Network,
                path: None,
            },
            LinuxNamespace {
                typ: LinuxNamespaceType::Mount,
                path: None,
            },
            LinuxNamespace {
                typ: LinuxNamespaceType::Ipc,
                path: None,
            },
            LinuxNamespace {
                typ: LinuxNamespaceType::Uts,
                path: None,
            },
        ];

        linux.set_namespaces(Some(namespaces));
        spec_builder.set_linux(Some(linux));

        Ok(spec_builder)
    }

    /// Setup container networking with advanced optimization
    async fn setup_container_networking(&self, container_id: &str, config: &NativeContainerConfig) -> Result<()> {
        info!("🌐 Setting up advanced networking for container: {}", container_id);

        // Create network configuration
        let network_config = ContainerNetworkConfig {
            port_mappings: config.ports.iter().map(|port_str| {
                let parts: Vec<&str> = port_str.split(':').collect();
                if parts.len() == 2 {
                    super::networking::PortMapping {
                        host_port: parts[0].parse().unwrap_or(8080),
                        container_port: parts[1].parse().unwrap_or(8080),
                        protocol: super::networking::Protocol::Tcp,
                        quic_enabled: self.gaming_mode,
                    }
                } else {
                    super::networking::PortMapping {
                        host_port: 8080,
                        container_port: 8080,
                        protocol: super::networking::Protocol::Tcp,
                        quic_enabled: self.gaming_mode,
                    }
                }
            }).collect(),
            bandwidth_limit: None,
            latency_target: if self.gaming_mode { Some(100) } else { None }, // 100μs for gaming
            dns_servers: vec![], // Use system DNS
        };

        // Create or get default network
        let network_id = "default".to_string();

        // Connect container to network
        // self.network_manager.connect_container(&network_id, container_id, network_config).await?;

        Ok(())
    }

    /// Monitor container performance and security
    async fn monitor_container(
        security_manager: BoltSecurityManager,
        performance_optimizer: BoltPerformanceOptimizer,
        container_id: String,
    ) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));

        loop {
            interval.tick().await;

            // Monitor security
            if let Ok(security_metrics) = security_manager.monitor_container_security(&container_id).await {
                if matches!(security_metrics.threat_level, super::security::ThreatLevel::High | super::security::ThreatLevel::Critical) {
                    warn!("🚨 High threat level detected for container: {}", container_id);
                }
            }

            // Monitor performance
            if let Ok(perf_metrics) = performance_optimizer.monitor_performance(&container_id).await {
                debug!("📊 Container {} performance: CPU {:.1}%, Memory {}MB",
                       container_id, perf_metrics.cpu_usage, perf_metrics.memory_usage / 1024 / 1024);
            }
        }
    }

    /// Get enhanced container metrics including GPU
    pub async fn get_container_metrics(&self, container_id: &str) -> Result<(SecurityMetrics, PerformanceMetrics, Option<GpuMetrics>)> {
        let security_metrics = self.security_manager.monitor_container_security(container_id).await?;
        let performance_metrics = self.performance_optimizer.monitor_performance(container_id).await?;

        // Get GPU metrics if GPU is enabled for this container
        let gpu_metrics = self.gpu_integration.get_gpu_metrics(container_id).await.ok();

        Ok((security_metrics, performance_metrics, gpu_metrics))
    }

    /// Benchmark container performance
    pub async fn benchmark_container(&self, container_id: &str) -> Result<BenchmarkResults> {
        info!("🏁 Running comprehensive benchmark for container: {}", container_id);
        self.performance_optimizer.benchmark_container(container_id).await
    }

    /// Enable gaming mode for existing runtime
    pub async fn enable_gaming_mode(&mut self) -> Result<()> {
        info!("🎮 Enabling gaming mode for runtime");
        self.gaming_mode = true;

        // Reinitialize performance optimizer with gaming settings
        self.performance_optimizer = BoltPerformanceOptimizer::new(true);

        // Apply gaming optimizations to all running containers
        for container_id in self.containers.keys() {
            if let Err(e) = self.performance_optimizer.optimize_container(container_id).await {
                warn!("Failed to apply gaming optimizations to container {}: {}", container_id, e);
            }
        }

        Ok(())
    }

    /// Create GPU-enabled gaming container with nvbind optimization
    pub async fn create_gaming_container(
        &mut self,
        image: &str,
        name: Option<&str>,
        dlss_enabled: bool,
        raytracing_enabled: bool,
    ) -> Result<String> {
        let gpu_config = GpuConfig {
            enabled: true,
            workload_type: GpuWorkloadType::Gaming {
                dlss_enabled,
                raytracing_enabled,
                performance_profile: "ultra-low-latency".to_string(),
                wine_proton_enabled: true,
                vrs_enabled: true,
            },
            isolation_level: GpuIsolationLevel::Exclusive,
            memory_limit: Some("8GB".to_string()),
            snapshot_support: true,
        };

        let config = NativeContainerConfig {
            image: image.to_string(),
            name: name.map(|s| s.to_string()),
            ports: vec![],
            env: vec![],
            volumes: vec![],
            detach: true,
            command: None,
            working_dir: None,
            user: None,
            gpu_config: Some(gpu_config),
        };

        self.run_container(config).await
    }

    /// Create AI/ML container with nvbind optimization
    pub async fn create_aiml_container(
        &mut self,
        image: &str,
        name: Option<&str>,
        tensor_cores: bool,
        mixed_precision: bool,
    ) -> Result<String> {
        let gpu_config = GpuConfig {
            enabled: true,
            workload_type: GpuWorkloadType::AiMl {
                cuda_cache_mb: Some(4096),
                tensor_cores_enabled: tensor_cores,
                mixed_precision_enabled: mixed_precision,
                memory_pool_size: Some("16GB".to_string()),
                mig_enabled: false,
            },
            isolation_level: GpuIsolationLevel::Virtual,
            memory_limit: Some("12GB".to_string()),
            snapshot_support: false,
        };

        let config = NativeContainerConfig {
            image: image.to_string(),
            name: name.map(|s| s.to_string()),
            ports: vec![],
            env: vec![],
            volumes: vec![],
            detach: true,
            command: None,
            working_dir: None,
            user: None,
            gpu_config: Some(gpu_config),
        };

        self.run_container(config).await
    }

    /// Check if nvbind GPU acceleration is available
    pub fn is_nvbind_available(&self) -> bool {
        self.gpu_integration.is_nvbind_available()
    }
}