use crate::Result;
use anyhow::{Context, anyhow};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
#[cfg(unix)]
use nix::unistd::Uid;
use oci_spec::runtime::{Capabilities, Capability};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(unix)]
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::gpu_integration::{
    AppliedCdiSpec, BoltGpuIntegration, GpuConfig, GpuIsolationLevel, GpuMetrics, GpuWorkloadType,
};
use super::hardware_detection::{HardwareProfile, WorkloadType as HwWorkloadType};
use super::networking::{
    BoltNetworkManager, ContainerNetworkConfig, NetworkDriver, NetworkPerformanceMode,
    PortMapping as NetworkPortMapping, Protocol as NetworkProtocol,
};
use super::oci::{self, ContainerConfig, ContainerState, ResourceLimits};
use super::performance::{BenchmarkResults, BoltPerformanceOptimizer, PerformanceMetrics};
use super::security::{BoltSecurityManager, SecurityMetrics};
use super::state;
use super::storage::{ImageGcReport, ImageMetadata, StorageManager, normalize_reference};
use std::{env, fs};

#[allow(dead_code)]
const DEFAULT_ROOTFS_DIRS: &[&str] = &[
    "bin", "etc", "lib", "tmp", "var", "usr", "dev", "proc", "sys",
];

/// Enhanced Native Bolt container runtime with cutting-edge security and performance
#[derive(Debug)]
pub struct BoltNativeRuntime {
    storage: StorageManager,
    containers: HashMap<String, ContainerState>,
    container_networks: HashMap<String, String>,
    container_network_modes: HashMap<String, String>,
    runtime_dir: PathBuf,
    security_manager: BoltSecurityManager,
    performance_optimizer: BoltPerformanceOptimizer,
    network_manager: BoltNetworkManager,
    gpu_integration: BoltGpuIntegration,
    hardware_profile: Option<HardwareProfile>, // Cached hardware detection
    gaming_mode: bool,
    rootless: bool,
    default_network_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeContainerConfig {
    pub image: String,
    pub name: Option<String>,
    pub ports: Vec<String>,
    pub env: Vec<String>,
    pub volumes: Vec<String>,
    pub detach: bool,
    pub rm: bool,
    pub command: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub user: Option<String>,
    pub hostname: Option<String>,
    pub cpus: Option<f32>,
    pub memory: Option<String>,
    pub network: Option<String>,
    pub cap_add: Vec<String>,
    pub cap_drop: Vec<String>,
    pub privileged: bool,
    pub tty: bool,
    pub interactive: bool,
    pub readonly_rootfs: bool,
    pub pids_limit: Option<i64>,
    pub seccomp: Option<String>, // OCI seccomp profile path, or "unconfined"
    pub gpu_config: Option<GpuConfig>,
    pub cpu_affinity: Option<Vec<usize>>, // CPU cores to pin to
    pub workload_hint: Option<WorkloadHint>, // Hint for auto-optimization
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkloadHint {
    Gaming,
    HighPerformance,
    Balanced,
    Background,
    Batch,
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
        info!(
            "🚀 Initializing Enhanced Bolt Native Runtime (gaming: {})",
            gaming_mode
        );

        let rootless = Self::detect_rootless_mode();
        let runtime_dir = Self::resolve_runtime_dir(rootless)?;
        fs::create_dir_all(&runtime_dir).with_context(|| {
            format!(
                "Failed to create runtime directory at {}",
                runtime_dir.display()
            )
        })?;
        info!(
            "🧭 Runtime initialized in {} mode (dir: {})",
            if rootless { "rootless" } else { "rootful" },
            runtime_dir.display()
        );

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

        // Detect hardware profile for CPU affinity optimization
        let hardware_profile = match HardwareProfile::detect().await {
            Ok(profile) => {
                info!("🔍 Hardware profile detected:");
                if profile.cpu.has_3d_vcache {
                    info!("   ⚡ AMD 3D V-Cache detected - gaming optimizations enabled");
                }
                if profile.cpu.hybrid_architecture.is_some() {
                    info!(
                        "   🔀 Intel hybrid architecture detected - P/E core optimization enabled"
                    );
                }
                Some(profile)
            }
            Err(e) => {
                warn!("⚠️  Failed to detect hardware profile: {}", e);
                None
            }
        };

        // Hydrate persisted container state so lifecycle commands (ps/stop/
        // restart/rm/logs/exec) survive a restart of the Bolt process.
        let mut containers = HashMap::new();
        match state::load_all() {
            Ok(states) => {
                for mut st in states {
                    state::reconcile_liveness(&mut st);
                    containers.insert(st.id.clone(), st);
                }
                if !containers.is_empty() {
                    info!("📦 Hydrated {} persisted container(s)", containers.len());
                }
            }
            Err(e) => warn!("⚠️  Failed to load persisted container state: {}", e),
        }

        Ok(Self {
            storage,
            containers,
            container_networks: HashMap::new(),
            container_network_modes: HashMap::new(),
            runtime_dir,
            security_manager,
            performance_optimizer,
            network_manager,
            gpu_integration,
            hardware_profile,
            gaming_mode,
            rootless,
            default_network_id: None,
        })
    }

    /// Run a container with native OCI runtime (replaces docker/podman run)
    pub async fn run_container(&mut self, config: NativeContainerConfig) -> Result<String> {
        info!("🐳 Starting native container: {}", config.image);
        Self::validate_run_options(&config)?;

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

        // Resolve the image digest for provenance, best-effort.
        let image_digest = self
            .storage
            .get_cached_image_metadata(&config.image)
            .map(|meta| meta.digest);

        // Create container state (spec will be generated after rootfs preparation)
        let container_state = ContainerState {
            id: container_id.clone(),
            status: super::oci::ContainerStatus::Created,
            pid: None,
            bundle_path: self.runtime_dir.join(&container_id),
            config: container_config,
            created: std::time::SystemTime::now(),
            started: None,
            finished: None,
            exit_code: None,
            image_digest,
            log_path: Some(container_log_path(&container_id)),
            gpu_allocation: config.gpu_config.as_ref().map(describe_gpu_allocation),
        };

        // Create bundle directory
        fs::create_dir_all(&container_state.bundle_path)
            .context("Failed to create container bundle")?;

        // Prepare persistent root filesystem storage
        let persistent_rootfs = self
            .storage
            .create_container_rootfs(&container_id, &config.image)
            .await?;

        // Prepare bundle root filesystem (until layers are applied, create minimal structure)
        let bundle_rootfs = container_state.bundle_path.join("rootfs");
        if bundle_rootfs.exists() {
            let metadata = fs::symlink_metadata(&bundle_rootfs)?;
            if metadata.file_type().is_symlink() {
                fs::remove_file(&bundle_rootfs).with_context(|| {
                    format!(
                        "Failed to remove existing bundle rootfs symlink for {}",
                        container_id
                    )
                })?;
            } else {
                fs::remove_dir_all(&bundle_rootfs).with_context(|| {
                    format!("Failed to reset bundle rootfs for {}", container_id)
                })?;
            }
        }
        #[cfg(unix)]
        {
            unix_fs::symlink(&persistent_rootfs, &bundle_rootfs).with_context(|| {
                format!(
                    "Failed to symlink bundle rootfs to persistent storage for {}",
                    container_id
                )
            })?;
        }
        #[cfg(not(unix))]
        {
            fs::create_dir_all(&bundle_rootfs)
                .with_context(|| format!("Failed to create bundle rootfs for {}", container_id))?;
            for dir in DEFAULT_ROOTFS_DIRS {
                fs::create_dir_all(bundle_rootfs.join(dir))
                    .with_context(|| format!("Failed to create rootfs directory '{}'", dir))?;
            }
        }

        let mut applied_cdi: Option<AppliedCdiSpec> = None;

        // Determine optimal CPU affinity
        let cpu_affinity = if let Some(explicit_affinity) = &config.cpu_affinity {
            // User explicitly specified CPU cores
            Some(explicit_affinity.clone())
        } else if let Some(ref hw_profile) = self.hardware_profile {
            // Auto-determine based on workload hint
            let workload = match config.workload_hint {
                Some(WorkloadHint::Gaming) => HwWorkloadType::Gaming,
                Some(WorkloadHint::HighPerformance) => HwWorkloadType::HighPerformance,
                Some(WorkloadHint::Balanced) => HwWorkloadType::Balanced,
                Some(WorkloadHint::Background) => HwWorkloadType::Background,
                Some(WorkloadHint::Batch) => HwWorkloadType::Batch,
                None if self.gaming_mode => HwWorkloadType::Gaming,
                None => HwWorkloadType::Balanced,
            };

            match hw_profile.optimal_cpu_affinity(workload) {
                super::hardware_detection::CpuAffinity::Specific(cores) => {
                    info!("🎯 Auto CPU affinity ({:?}): {:?}", workload, cores);
                    Some(cores)
                }
                _ => None,
            }
        } else {
            None
        };

        // Apply security hardening before execution
        let security_profile = if self.gaming_mode { "gaming" } else { "secure" };
        self.security_manager
            .harden_container(&container_id, security_profile)
            .await?;

        // Apply performance optimizations
        self.performance_optimizer
            .optimize_container(&container_id)
            .await?;

        // Setup networking
        self.setup_container_networking(&container_id, &config)
            .await?;
        let network_attached = self.container_networks.contains_key(&container_id);

        // Setup GPU if requested
        if let Some(ref gpu_config) = config.gpu_config {
            info!("🎮 Setting up GPU for container: {}", container_id);
            match self
                .gpu_integration
                .setup_gpu_for_container(&container_id, gpu_config)
                .await
            {
                Ok(cdi_artifacts) => {
                    applied_cdi = Some(cdi_artifacts);
                }
                Err(err) => {
                    if network_attached
                        && let Err(clean_err) =
                            self.teardown_container_networking(&container_id).await
                    {
                        warn!(
                            "Failed to clean up networking after GPU setup error for {}: {}",
                            container_id, clean_err
                        );
                    }
                    return Err(err);
                }
            }
        }

        // Create OCI spec from configuration
        let spec = self
            .create_oci_spec(
                &container_id,
                &container_state.config,
                applied_cdi.as_ref(),
                cpu_affinity.as_ref(),
            )
            .await?;

        if let Some(ref cdi) = applied_cdi {
            debug!(
                devices = ?cdi.device_nodes,
                mounts = ?cdi.mounts,
                hooks = ?cdi.hooks,
                "CDI artifacts prepared for container {}",
                container_id
            );
        }

        // Write OCI spec to bundle
        let spec_path = container_state.bundle_path.join("config.json");
        let spec_json = serde_json::to_string_pretty(&spec)?;
        fs::write(&spec_path, spec_json).context("Failed to write OCI spec")?;

        // Execute container with native OCI runtime
        let execution = match oci::execute_container(&container_state, &spec).await {
            Ok(result) => result,
            Err(err) => {
                if network_attached
                    && let Err(clean_err) = self.teardown_container_networking(&container_id).await
                {
                    warn!(
                        "Failed to clean up networking after execution error for {}: {}",
                        container_id, clean_err
                    );
                }
                return Err(err);
            }
        };

        if let Some(pid) = execution.pid
            && let Err(err) = self
                .finalize_container_networking(&container_id, pid as i32)
                .await
        {
            warn!(
                "Failed to finalize networking for container {}: {}",
                container_id, err
            );
        }

        // Update container state
        let mut updated_state = container_state;
        updated_state.started = Some(std::time::SystemTime::now());
        updated_state.pid = execution.pid;
        updated_state.exit_code = execution.exit_code;
        if let Some(code) = execution.exit_code {
            updated_state.status = super::oci::ContainerStatus::Exited(code);
            updated_state.finished = Some(std::time::SystemTime::now());
        } else {
            updated_state.status = super::oci::ContainerStatus::Running;
        }

        if config.rm && !config.detach {
            if network_attached
                && let Err(err) = self.teardown_container_networking(&container_id).await
            {
                warn!(
                    "Failed to clean up networking for --rm container {}: {}",
                    container_id, err
                );
            }
            if let Err(err) = self
                .cleanup_container_artifacts(&container_id, &updated_state)
                .await
            {
                warn!(
                    "Failed to clean up --rm container {} after exit: {}",
                    container_id, err
                );
            }
            return Ok(container_id);
        }

        if let Err(err) = self.mark_container_volumes_attached(&updated_state).await {
            warn!(
                "Failed to update volume usage for container {}: {}",
                container_id, err
            );
        }

        // Persist state so lifecycle commands survive a process restart.
        if let Err(err) = state::save(&updated_state) {
            warn!(
                "Failed to persist state for container {}: {}",
                container_id, err
            );
        }

        // Store container state
        self.containers.insert(container_id.clone(), updated_state);

        // Start monitoring
        if config.detach {
            tokio::spawn({
                let security_manager = self.security_manager.clone();
                let performance_optimizer = self.performance_optimizer.clone();
                let container_id = container_id.clone();
                async move {
                    Self::monitor_container(security_manager, performance_optimizer, container_id)
                        .await
                }
            });
        }

        // info!("✅ Enhanced native container started: {}", container_id);
        Ok(container_id)
    }

    /// Stop a running container (replaces docker/podman stop)
    pub async fn stop_container(&mut self, id: &str) -> Result<()> {
        self.stop_container_with_timeout(id, 10).await
    }

    /// Stop a running container with a graceful timeout before force kill.
    pub async fn stop_container_with_timeout(&mut self, id: &str, timeout: u64) -> Result<()> {
        info!("🛑 Stopping container: {}", id);

        let id = self
            .resolve_id(id)
            .ok_or_else(|| anyhow!("Container not found: {}", id))?;
        let id = id.as_str();

        let container = self
            .containers
            .get_mut(id)
            .ok_or_else(|| anyhow!("Container not found: {}", id))?;

        if let Some(pid) = container.pid {
            let nix_pid = Pid::from_raw(pid as i32);

            // Send SIGTERM first for graceful shutdown
            if let Err(e) = signal::kill(nix_pid, Signal::SIGTERM) {
                warn!("Failed to send SIGTERM to process {}: {}", pid, e);
            } else {
                info!("Sent SIGTERM to process {}", pid);
            }

            // Wait for graceful shutdown.
            tokio::time::sleep(std::time::Duration::from_secs(timeout)).await;

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
        container.finished = Some(std::time::SystemTime::now());

        if let Err(err) = oci::delete_runtime_container(id).await {
            warn!(
                "Failed to delete OCI runtime state for stopped container {}: {}",
                id, err
            );
        }

        if let Err(err) = state::save(container) {
            warn!(
                "Failed to persist stopped state for container {}: {}",
                id, err
            );
        }

        info!("✅ Container stopped: {}", id);
        Ok(())
    }

    /// Remove a container (replaces docker/podman rm)
    pub async fn remove_container(&mut self, id: &str, force: bool) -> Result<()> {
        info!("🗑️  Removing container: {}", id);

        let id = self
            .resolve_id(id)
            .ok_or_else(|| anyhow!("Container not found: {}", id))?;
        let id = id.as_str();

        // Check if container exists and get its status and bundle path
        let (is_running, bundle_path, volumes) = {
            let container = self
                .containers
                .get(id)
                .ok_or_else(|| anyhow!("Container not found: {}", id))?;

            (
                matches!(container.status, super::oci::ContainerStatus::Running),
                container.bundle_path.clone(),
                container.config.volumes.clone(),
            )
        };

        // Stop if running
        if is_running {
            if force {
                self.stop_container(id).await?;
            } else {
                return Err(
                    anyhow!("Container is running. Use force=true to stop and remove.").into(),
                );
            }
        }

        if let Err(err) = oci::delete_runtime_container(id).await {
            warn!(
                "Failed to delete OCI runtime state for removed container {}: {}",
                id, err
            );
        }

        // Clean up bundle directory
        if bundle_path.exists() {
            fs::remove_dir_all(&bundle_path).context("Failed to remove container bundle")?;
        }

        if let Err(err) = self.teardown_container_networking(id).await {
            warn!(
                "Networking teardown encountered an issue for container {}: {}",
                id, err
            );
        }

        // Remove from containers map and persistent storage
        self.containers.remove(id);

        self.storage.remove_container(id).await?;
        if let Err(err) = self.mark_container_volumes_detached(id, &volumes).await {
            warn!(
                "Failed to update volume usage for removed container {}: {}",
                id, err
            );
        }
        if let Err(err) = state::remove(id) {
            warn!(
                "Failed to remove persisted state for container {}: {}",
                id, err
            );
        }

        // Drop any per-container environment recorded during GPU/AI/gaming setup.
        if let Err(err) = crate::runtime::environment::env_manager().clear_container_env(id) {
            warn!("Failed to clear environment for container {}: {}", id, err);
        }

        info!("✅ Container removed: {}", id);
        Ok(())
    }

    /// Restart a persisted native container.
    pub async fn restart_container(&mut self, id: &str, timeout: u64) -> Result<()> {
        info!(
            "🔄 Restarting native container: {} (timeout: {}s)",
            id, timeout
        );

        let container_id = self
            .resolve_id(id)
            .or_else(|| state::resolve_ref(id).ok().flatten().map(|state| state.id))
            .ok_or_else(|| anyhow!("Container not found: {}", id))?;

        let mut existing = self
            .containers
            .get(&container_id)
            .cloned()
            .or_else(|| state::load(&container_id).ok().flatten())
            .ok_or_else(|| anyhow!("Container not found: {}", id))?;

        if existing.config.gaming_config.is_some() || existing.gpu_allocation.is_some() {
            return Err(anyhow!(
                "native restart for GPU/gaming containers is not supported yet; remove and recreate {}",
                id
            )
            .into());
        }

        state::reconcile_liveness(&mut existing);
        if matches!(existing.status, super::oci::ContainerStatus::Running) {
            self.stop_container_with_timeout(&container_id, timeout)
                .await?;
        }
        if let Err(err) = oci::delete_runtime_container(&container_id).await {
            warn!(
                "Failed to delete OCI runtime state before restart for {}: {}",
                container_id, err
            );
        }

        if existing.bundle_path.exists() {
            fs::remove_dir_all(&existing.bundle_path).with_context(|| {
                format!(
                    "Failed to reset container bundle {}",
                    existing.bundle_path.display()
                )
            })?;
        }
        fs::create_dir_all(&existing.bundle_path).context("Failed to recreate container bundle")?;

        let persistent_rootfs = self
            .storage
            .create_container_rootfs(&container_id, &existing.config.image)
            .await?;
        let bundle_rootfs = existing.bundle_path.join("rootfs");
        if bundle_rootfs.exists() {
            let metadata = fs::symlink_metadata(&bundle_rootfs)?;
            if metadata.file_type().is_symlink() {
                fs::remove_file(&bundle_rootfs).with_context(|| {
                    format!(
                        "Failed to remove existing bundle rootfs symlink for {}",
                        container_id
                    )
                })?;
            } else {
                fs::remove_dir_all(&bundle_rootfs).with_context(|| {
                    format!("Failed to reset bundle rootfs for {}", container_id)
                })?;
            }
        }
        #[cfg(unix)]
        {
            unix_fs::symlink(&persistent_rootfs, &bundle_rootfs).with_context(|| {
                format!(
                    "Failed to symlink bundle rootfs to persistent storage for {}",
                    container_id
                )
            })?;
        }
        #[cfg(not(unix))]
        {
            fs::create_dir_all(&bundle_rootfs)
                .with_context(|| format!("Failed to create bundle rootfs for {}", container_id))?;
        }

        let restart_config = Self::native_config_from_persisted_state(&existing);

        let network_attached = if existing.config.detach {
            self.setup_container_networking(&container_id, &restart_config)
                .await?;
            self.container_networks.contains_key(&container_id)
        } else {
            false
        };

        let spec = self
            .create_oci_spec(&container_id, &existing.config, None, None)
            .await?;

        let spec_path = existing.bundle_path.join("config.json");
        let spec_json = serde_json::to_string_pretty(&spec)?;
        fs::write(&spec_path, spec_json).context("Failed to write OCI spec")?;

        let execution = match oci::execute_container(&existing, &spec).await {
            Ok(result) => result,
            Err(err) => {
                if network_attached
                    && let Err(clean_err) = self.teardown_container_networking(&container_id).await
                {
                    warn!(
                        "Failed to clean up networking after restart error for {}: {}",
                        container_id, clean_err
                    );
                }
                return Err(err);
            }
        };

        if let Some(pid) = execution.pid
            && let Err(err) = self
                .finalize_container_networking(&container_id, pid as i32)
                .await
        {
            warn!(
                "Failed to finalize networking for restarted container {}: {}",
                container_id, err
            );
        }

        existing.status = if let Some(code) = execution.exit_code {
            super::oci::ContainerStatus::Exited(code)
        } else {
            super::oci::ContainerStatus::Running
        };
        existing.pid = execution.pid;
        existing.started = Some(std::time::SystemTime::now());
        existing.finished = execution.exit_code.map(|_| std::time::SystemTime::now());
        existing.exit_code = execution.exit_code;

        if let Err(err) = state::save(&existing) {
            warn!(
                "Failed to persist restarted state for container {}: {}",
                container_id, err
            );
        }
        self.containers.insert(container_id.clone(), existing);

        tokio::spawn({
            let security_manager = self.security_manager.clone();
            let performance_optimizer = self.performance_optimizer.clone();
            let container_id = container_id.clone();
            async move {
                Self::monitor_container(security_manager, performance_optimizer, container_id).await
            }
        });

        info!("✅ Container restarted: {}", container_id);
        Ok(())
    }

    async fn cleanup_container_artifacts(
        &mut self,
        container_id: &str,
        state: &ContainerState,
    ) -> Result<()> {
        if state.bundle_path.exists() {
            fs::remove_dir_all(&state.bundle_path).with_context(|| {
                format!(
                    "Failed to remove container bundle {}",
                    state.bundle_path.display()
                )
            })?;
        }

        if let Err(err) = oci::delete_runtime_container(container_id).await {
            warn!(
                "Failed to delete OCI runtime state for cleaned-up container {}: {}",
                container_id, err
            );
        }

        self.containers.remove(container_id);
        self.storage.remove_container(container_id).await?;
        state::remove(container_id)?;

        if let Err(err) =
            crate::runtime::environment::env_manager().clear_container_env(container_id)
        {
            warn!(
                "Failed to clear environment for container {}: {}",
                container_id, err
            );
        }

        Ok(())
    }

    fn native_config_from_persisted_state(state: &ContainerState) -> NativeContainerConfig {
        NativeContainerConfig {
            image: state.config.image.clone(),
            name: state.config.name.clone(),
            ports: state
                .config
                .ports
                .iter()
                .map(|port| format!("{}:{}", port.host_port, port.container_port))
                .collect(),
            env: state
                .config
                .env
                .iter()
                .map(|(key, value)| format!("{}={}", key, value))
                .collect(),
            volumes: state
                .config
                .volumes
                .iter()
                .map(|volume| {
                    if volume.readonly {
                        format!("{}:{}:ro", volume.source, volume.destination)
                    } else {
                        format!("{}:{}", volume.source, volume.destination)
                    }
                })
                .collect(),
            detach: state.config.detach,
            rm: false,
            command: Some(state.config.command.clone()),
            entrypoint: None,
            working_dir: state.config.working_dir.clone(),
            user: state.config.user.clone(),
            hostname: state.config.hostname.clone(),
            cpus: None,
            memory: None,
            network: Some("bridge".to_string()),
            cap_add: vec![],
            cap_drop: vec![],
            privileged: state.config.privileged,
            tty: state.config.tty,
            interactive: false,
            readonly_rootfs: state.config.readonly_rootfs,
            pids_limit: state
                .config
                .resource_limits
                .as_ref()
                .and_then(|limits| limits.pids_limit),
            seccomp: state.config.seccomp.clone(),
            gpu_config: None,
            cpu_affinity: None,
            workload_hint: None,
        }
    }

    /// Resolve a container reference (id or `--name`) to its stored id.
    fn resolve_id(&self, name_or_id: &str) -> Option<String> {
        if self.containers.contains_key(name_or_id) {
            return Some(name_or_id.to_string());
        }
        self.containers
            .iter()
            .find(|(_, st)| st.config.name.as_deref() == Some(name_or_id))
            .map(|(id, _)| id.clone())
    }

    /// List containers (replaces docker/podman ps)
    pub async fn list_containers(&self, all: bool) -> Result<Vec<NativeContainerInfo>> {
        let mut containers = Vec::new();

        for (id, stored) in &self.containers {
            // Report the reconciled status so a container whose process died
            // while we were not running is not shown as Running.
            let mut st = stored.clone();
            state::reconcile_liveness(&mut st);

            if !all && !matches!(st.status, super::oci::ContainerStatus::Running) {
                continue;
            }

            let info = NativeContainerInfo {
                id: id.clone(),
                name: st.config.name.clone(),
                image: st.config.image.clone(),
                status: match &st.status {
                    super::oci::ContainerStatus::Created => ContainerStatus::Created,
                    super::oci::ContainerStatus::Running => ContainerStatus::Running,
                    super::oci::ContainerStatus::Stopped => ContainerStatus::Stopped,
                    super::oci::ContainerStatus::Exited(code) => ContainerStatus::Exited(*code),
                },
                created: st.created,
                ports: st
                    .config
                    .ports
                    .iter()
                    .map(|p| format!("{}:{}", p.host_port, p.container_port))
                    .collect(),
                pid: st.pid,
            };

            containers.push(info);
        }

        Ok(containers)
    }

    /// Pull an image (replaces docker/podman pull)
    pub async fn pull_image_native(&mut self, image: &str) -> Result<()> {
        info!("⬇️  Pulling image with native client: {}", image);

        let metadata = self.storage.pull_image(image).await?;

        info!("✅ Image pulled: {} ({})", image, metadata.digest);
        Ok(())
    }

    /// Build an image (replaces docker/podman build)
    pub async fn build_image_native(
        &mut self,
        context: &str,
        tag: Option<&str>,
        dockerfile: &str,
    ) -> Result<()> {
        info!("🔨 Building image natively from: {}", context);

        // Use native image builder
        self.storage
            .build_image(context, tag.unwrap_or("latest"), dockerfile)
            .await?;

        info!("✅ Image built successfully");
        Ok(())
    }

    /// Push an image with the native registry client.
    pub async fn push_image_native(&self, image: &str) -> Result<()> {
        info!("⬆️  Pushing image with native client: {}", image);
        self.storage.push_image(image).await?;
        Ok(())
    }

    pub fn list_images_native(&self) -> Vec<(String, ImageMetadata)> {
        self.storage.list_cached_images()
    }

    pub async fn prune_images_native(&mut self, dry_run: bool) -> Result<ImageGcReport> {
        let protected = self
            .containers
            .values()
            .map(|container| normalize_reference(&container.config.image))
            .collect();
        let mut protected_container_ids: std::collections::HashSet<String> =
            self.containers.keys().cloned().collect();
        let mut protected_digests = self
            .containers
            .values()
            .filter_map(|container| container.image_digest.clone())
            .collect::<std::collections::HashSet<_>>();
        if let Ok(snapshot_manager) = crate::capsules::snapshots::SnapshotManager::new().await
            && let Ok(generations) = snapshot_manager.list_generations().await
        {
            for generation in generations {
                protected_digests.extend(generation.image_digests);
                protected_container_ids.extend(generation.container_ids);
            }
        }
        self.storage
            .prune_images(
                &protected,
                &protected_digests,
                &protected_container_ids,
                dry_run,
            )
            .await
    }

    // Helper methods
    async fn create_container_config(
        &self,
        id: &str,
        config: &NativeContainerConfig,
    ) -> Result<ContainerConfig> {
        // Pull image metadata for defaults
        let image_metadata = self.storage.get_cached_image_metadata(&config.image);

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
        if let Some(ref metadata) = image_metadata {
            for env_str in &metadata.config.env {
                if let Some(eq_pos) = env_str.find('=') {
                    let key = &env_str[..eq_pos];
                    let value = &env_str[eq_pos + 1..];
                    env.insert(key.to_string(), value.to_string());
                }
            }
        }
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

        // Determine command/entrypoint behavior
        let mut command = Vec::new();
        if let Some(ref entrypoint) = config.entrypoint {
            command.extend(entrypoint.clone());
        } else if let Some(ref metadata) = image_metadata
            && let Some(ref entrypoint) = metadata.config.entrypoint
        {
            command.extend(entrypoint.clone());
        }

        if let Some(custom_cmd) = config.command.clone() {
            command.extend(custom_cmd);
        } else if let Some(ref metadata) = image_metadata
            && let Some(ref default_cmd) = metadata.config.cmd
        {
            command.extend(default_cmd.clone());
        }

        if command.is_empty() {
            command.push("/bin/sh".to_string());
        }

        let resource_limits = Self::resource_limits_for(config)?;
        let capabilities = Self::capabilities_for(config)?;

        let working_dir = config.working_dir.clone().or_else(|| {
            image_metadata
                .as_ref()
                .and_then(|m| m.config.working_dir.clone())
        });

        let user = config
            .user
            .clone()
            .or_else(|| image_metadata.as_ref().and_then(|m| m.config.user.clone()));

        Ok(ContainerConfig {
            id: id.to_string(),
            name: config.name.clone(),
            image: config.image.clone(),
            command,
            args: vec![],
            env,
            working_dir,
            user,
            ports,
            volumes,
            capabilities,
            resource_limits,
            gaming_config: None, // Will be set later if needed
            detach: config.detach,
            hostname: config.hostname.clone(),
            network_mode: config
                .network
                .clone()
                .unwrap_or_else(|| "bridge".to_string()),
            privileged: config.privileged,
            tty: config.tty,
            readonly_rootfs: config.readonly_rootfs,
            seccomp: config.seccomp.clone(),
        })
    }

    async fn create_oci_spec(
        &self,
        container_id: &str,
        config: &ContainerConfig,
        applied_cdi: Option<&AppliedCdiSpec>,
        cpu_affinity: Option<&Vec<usize>>,
    ) -> Result<oci_spec::runtime::Spec> {
        use oci_spec::runtime::*;

        // Create basic OCI runtime spec
        let mut spec_builder = Spec::default();

        // Set version
        spec_builder.set_version("1.0.0".to_string());

        // Create process configuration
        let mut process = Process::default();
        process.set_args(Some(config.command.clone()));
        process.set_terminal(Some(config.tty));
        if let Some(ref cwd) = config.working_dir {
            process.set_cwd(PathBuf::from(cwd));
        }

        if let Some(user) = self.process_user_for(config) {
            process.set_user(user);
        }

        // Set environment variables. Precedence (later wins): image/user
        // defaults from the config, per-container vars recorded by GPU/AI/gaming
        // setup (EnvironmentManager), then CDI-provided device env on top.
        let mut combined_env: std::collections::HashMap<String, String> = config.env.clone();

        for entry in crate::runtime::environment::env_manager().get_container_env(container_id)? {
            combined_env.insert(entry.0, entry.1);
        }

        if let Some(cdi) = applied_cdi {
            for entry in &cdi.env {
                if let Some((key, value)) = entry.split_once('=') {
                    combined_env.insert(key.to_string(), value.to_string());
                } else {
                    combined_env.insert(entry.clone(), "1".to_string());
                }
            }
        }

        let mut env_vec: Vec<String> = combined_env
            .into_iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        env_vec.sort();
        if !env_vec.is_empty() {
            process.set_env(Some(env_vec));
        }

        if !config.capabilities.is_empty() || config.privileged {
            let capabilities = Self::linux_capabilities_for(config)?;
            process.set_capabilities(Some(capabilities));
            process.set_no_new_privileges(Some(!config.privileged));
        }

        spec_builder.set_process(Some(process));
        if let Some(ref hostname) = config.hostname {
            spec_builder.set_hostname(Some(hostname.clone()));
        }

        // Set root filesystem
        let mut root = Root::default();
        let bundle_rootfs = self.runtime_dir.join(container_id).join("rootfs");
        let rootfs_path = fs::canonicalize(&bundle_rootfs).unwrap_or(bundle_rootfs);
        root.set_path(rootfs_path);
        root.set_readonly(Some(config.readonly_rootfs));
        spec_builder.set_root(Some(root));

        // Create basic mounts
        let mut mounts = Vec::new();

        // Proc filesystem
        let mut proc_mount = Mount::default();
        proc_mount.set_destination(PathBuf::from("/proc"));
        proc_mount.set_typ(Some("proc".to_string()));
        proc_mount.set_source(Some(PathBuf::from("proc")));
        proc_mount.set_options(Some(vec![
            "nosuid".to_string(),
            "noexec".to_string(),
            "nodev".to_string(),
        ]));
        mounts.push(proc_mount);

        // Sys filesystem
        let mut sys_mount = Mount::default();
        sys_mount.set_destination(PathBuf::from("/sys"));
        sys_mount.set_typ(Some("sysfs".to_string()));
        sys_mount.set_source(Some(PathBuf::from("sysfs")));
        sys_mount.set_options(Some(vec![
            "nosuid".to_string(),
            "noexec".to_string(),
            "nodev".to_string(),
            "ro".to_string(),
        ]));
        mounts.push(sys_mount);

        // Dev filesystem
        let mut dev_mount = Mount::default();
        dev_mount.set_destination(PathBuf::from("/dev"));
        dev_mount.set_typ(Some("tmpfs".to_string()));
        dev_mount.set_source(Some(PathBuf::from("tmpfs")));
        dev_mount.set_options(Some(vec![
            "nosuid".to_string(),
            "strictatime".to_string(),
            "mode=755".to_string(),
            "size=65536k".to_string(),
        ]));
        mounts.push(dev_mount);

        // Add container volume mounts (OCI 7.0 compliant)
        for volume in &config.volumes {
            info!(
                "📁 Adding volume mount: {} -> {}",
                volume.source, volume.destination
            );

            // Resolve volume name to actual path
            let source_path = self.resolve_volume_source(&volume.source).await?;

            let mut volume_mount = Mount::default();
            volume_mount.set_destination(PathBuf::from(&volume.destination));
            volume_mount.set_typ(Some("bind".to_string()));
            volume_mount.set_source(Some(PathBuf::from(&source_path)));

            // Set mount options according to OCI 7.0 spec
            let mut mount_options = vec!["bind".to_string()];
            if volume.readonly {
                mount_options.push("ro".to_string());
            } else {
                mount_options.push("rw".to_string());
            }

            // Add security and reliability options
            mount_options.extend(vec!["relatime".to_string()]);

            volume_mount.set_options(Some(mount_options));
            mounts.push(volume_mount);

            info!(
                "✅ Volume mount configured: {} -> {} ({})",
                source_path,
                volume.destination,
                if volume.readonly {
                    "readonly"
                } else {
                    "readwrite"
                }
            );
        }

        // Inject CDI-provided mounts (bind-mount host paths to same container path)
        if let Some(cdi) = applied_cdi {
            for mount in &cdi.mounts {
                let host = PathBuf::from(&mount.host_path);
                if !host.exists() {
                    warn!("Skipping CDI mount for missing path: {}", mount.host_path);
                    continue;
                }

                let mut options = mount.options.clone();
                if options.is_empty() {
                    options.push("bind".to_string());
                } else if !options.iter().any(|opt| {
                    opt.eq_ignore_ascii_case("bind") || opt.eq_ignore_ascii_case("rbind")
                }) {
                    options.insert(0, "bind".to_string());
                }

                let mut cdi_mount = Mount::default();
                cdi_mount.set_destination(PathBuf::from(&mount.container_path));
                cdi_mount.set_typ(Some("bind".to_string()));
                cdi_mount.set_source(Some(host));
                cdi_mount.set_options(Some(options));
                mounts.push(cdi_mount);
            }
        }

        spec_builder.set_mounts(Some(mounts));

        let mut hook_section = Hooks::default();
        let mut hooks_added = false;
        if let Some(cdi) = applied_cdi {
            for hook in &cdi.hooks {
                let mut oci_hook = Hook::default();
                oci_hook.set_path(PathBuf::from(&hook.path));
                if !hook.args.is_empty() {
                    oci_hook.set_args(Some(hook.args.clone()));
                }
                if !hook.env.is_empty() {
                    oci_hook.set_env(Some(hook.env.clone()));
                }
                if let Some(timeout) = hook.timeout {
                    oci_hook.set_timeout(Some(timeout as i64));
                }

                let target = hook.hook_name.replace(['-', '_'], "").to_lowercase();

                match target.as_str() {
                    "prestart" => {
                        hook_section
                            .prestart_mut()
                            .get_or_insert_with(Vec::new)
                            .push(oci_hook);
                        hooks_added = true;
                    }
                    "createruntime" => {
                        hook_section
                            .create_runtime_mut()
                            .get_or_insert_with(Vec::new)
                            .push(oci_hook);
                        hooks_added = true;
                    }
                    "createcontainer" => {
                        hook_section
                            .create_container_mut()
                            .get_or_insert_with(Vec::new)
                            .push(oci_hook);
                        hooks_added = true;
                    }
                    "startcontainer" => {
                        hook_section
                            .start_container_mut()
                            .get_or_insert_with(Vec::new)
                            .push(oci_hook);
                        hooks_added = true;
                    }
                    "poststart" => {
                        hook_section
                            .poststart_mut()
                            .get_or_insert_with(Vec::new)
                            .push(oci_hook);
                        hooks_added = true;
                    }
                    "poststop" => {
                        hook_section
                            .poststop_mut()
                            .get_or_insert_with(Vec::new)
                            .push(oci_hook);
                        hooks_added = true;
                    }
                    _ => {
                        warn!(
                            "Ignoring CDI hook '{}' (unsupported lifecycle event)",
                            hook.hook_name
                        );
                    }
                }
            }
        }

        if hooks_added {
            spec_builder.set_hooks(Some(hook_section));
        }

        // Set Linux-specific configuration
        let mut linux = Linux::default();

        // Configure namespaces
        let mut namespaces = Vec::new();

        let mut pid_ns = LinuxNamespace::default();
        pid_ns.set_typ(LinuxNamespaceType::Pid);
        namespaces.push(pid_ns);

        if config.network_mode != "host" {
            let mut net_ns = LinuxNamespace::default();
            net_ns.set_typ(LinuxNamespaceType::Network);
            namespaces.push(net_ns);
        }

        let mut mount_ns = LinuxNamespace::default();
        mount_ns.set_typ(LinuxNamespaceType::Mount);
        namespaces.push(mount_ns);

        let mut ipc_ns = LinuxNamespace::default();
        ipc_ns.set_typ(LinuxNamespaceType::Ipc);
        namespaces.push(ipc_ns);

        let mut uts_ns = LinuxNamespace::default();
        uts_ns.set_typ(LinuxNamespaceType::Uts);
        namespaces.push(uts_ns);

        if self.rootless {
            let mut user_ns = LinuxNamespace::default();
            user_ns.set_typ(LinuxNamespaceType::User);
            namespaces.push(user_ns);

            // Add uid/gid mappings for rootless containers
            // Map current user to root inside container
            let current_uid = nix::unistd::getuid().as_raw();
            let current_gid = nix::unistd::getgid().as_raw();

            let uid_mappings = vec![
                LinuxIdMappingBuilder::default()
                    .container_id(0u32) // Root in container
                    .host_id(current_uid)
                    .size(1u32)
                    .build()
                    .context("Failed to build uid mapping")?,
            ];

            let gid_mappings = vec![
                LinuxIdMappingBuilder::default()
                    .container_id(0u32) // Root in container
                    .host_id(current_gid)
                    .size(1u32)
                    .build()
                    .context("Failed to build gid mapping")?,
            ];

            linux.set_uid_mappings(Some(uid_mappings));
            linux.set_gid_mappings(Some(gid_mappings));

            info!(
                "✅ Rootless mode: mapping host uid/gid {}:{} to container root",
                current_uid, current_gid
            );
        }

        linux.set_namespaces(Some(namespaces));

        // Add CDI device nodes when available
        if let Some(cdi) = applied_cdi {
            fn parse_device_type(value: Option<&str>) -> Option<LinuxDeviceType> {
                value.and_then(|raw| match raw.to_ascii_lowercase().as_str() {
                    "a" | "all" => Some(LinuxDeviceType::A),
                    "b" | "block" => Some(LinuxDeviceType::B),
                    "c" | "char" | "character" => Some(LinuxDeviceType::C),
                    "u" | "unbuffered" => Some(LinuxDeviceType::U),
                    "p" | "fifo" | "pipe" => Some(LinuxDeviceType::P),
                    _ => None,
                })
            }

            let mut linux_devices = Vec::new();
            for node in &cdi.device_nodes {
                let path_buf = PathBuf::from(&node.path);
                if !path_buf.exists() {
                    warn!("Skipping CDI device for missing path: {}", node.path);
                    continue;
                }

                let needs_stat = node.major.is_none()
                    || node.minor.is_none()
                    || node.device_type.is_none()
                    || node.file_mode.is_none()
                    || node.uid.is_none()
                    || node.gid.is_none();

                let stat_info = if needs_stat {
                    match nix::sys::stat::stat(&path_buf) {
                        Ok(stat) => Some(stat),
                        Err(err) => {
                            warn!("Failed to stat CDI device {}: {}", node.path, err);
                            None
                        }
                    }
                } else {
                    None
                };

                let device_type = parse_device_type(node.device_type.as_deref()).or_else(|| {
                    stat_info.as_ref().and_then(|stat| {
                        let file_type = nix::sys::stat::SFlag::from_bits_truncate(stat.st_mode);
                        if file_type.contains(nix::sys::stat::SFlag::S_IFCHR) {
                            Some(LinuxDeviceType::C)
                        } else if file_type.contains(nix::sys::stat::SFlag::S_IFBLK) {
                            Some(LinuxDeviceType::B)
                        } else {
                            None
                        }
                    })
                });

                let Some(device_type) = device_type else {
                    warn!(
                        "Skipping CDI device {} (unable to determine device type)",
                        node.path
                    );
                    continue;
                };

                let major = node.major.or_else(|| {
                    stat_info
                        .as_ref()
                        .map(|stat| nix::sys::stat::major(stat.st_rdev) as i64)
                });
                let minor = node.minor.or_else(|| {
                    stat_info
                        .as_ref()
                        .map(|stat| nix::sys::stat::minor(stat.st_rdev) as i64)
                });

                let (Some(major), Some(minor)) = (major, minor) else {
                    warn!(
                        "Skipping CDI device {} (missing major/minor numbers)",
                        node.path
                    );
                    continue;
                };

                let mut device = LinuxDevice::default();
                device.set_path(path_buf);
                device.set_typ(device_type);
                device.set_major(major);
                device.set_minor(minor);

                let file_mode = node
                    .file_mode
                    .or_else(|| stat_info.as_ref().map(|stat| stat.st_mode & 0o7777));
                if let Some(mode) = file_mode {
                    device.set_file_mode(Some(mode));
                } else {
                    device.set_file_mode(Some(0o666));
                }

                if let Some(uid) = node
                    .uid
                    .or_else(|| stat_info.as_ref().map(|stat| stat.st_uid))
                {
                    device.set_uid(Some(uid));
                }
                if let Some(gid) = node
                    .gid
                    .or_else(|| stat_info.as_ref().map(|stat| stat.st_gid))
                {
                    device.set_gid(Some(gid));
                }

                linux_devices.push(device);
            }

            if !linux_devices.is_empty() {
                let mut resources = match linux.resources() {
                    Some(r) => r.clone(),
                    None => oci_spec::runtime::LinuxResources::default(),
                };
                let mut device_rules = resources.devices().clone().unwrap_or_default();
                device_rules.extend(linux_devices.iter().map(LinuxDeviceCgroup::from));
                resources.set_devices(Some(device_rules));
                linux.set_devices(Some(linux_devices));
                linux.set_resources(Some(resources));
            }
        }

        // Apply CPU affinity if specified
        if let Some(cpu_cores) = cpu_affinity {
            let mut resources = match linux.resources() {
                Some(r) => r.clone(),
                None => oci_spec::runtime::LinuxResources::default(),
            };
            let mut cpu = match resources.cpu() {
                Some(c) => c.clone(),
                None => oci_spec::runtime::LinuxCpu::default(),
            };

            // Convert core list to cpuset format (e.g., "0-3,6-7")
            let cpuset = if cpu_cores.is_empty() {
                None
            } else {
                let mut sorted_cores = cpu_cores.clone();
                sorted_cores.sort_unstable();
                Some(Self::format_cpuset(&sorted_cores))
            };

            if let Some(ref cpuset_str) = cpuset {
                cpu.set_cpus(Some(cpuset_str.clone()));
                info!("🎯 CPU affinity applied: {}", cpuset_str);
            }

            resources.set_cpu(Some(cpu));
            linux.set_resources(Some(resources));
        }

        if let Some(ref limits) = config.resource_limits {
            let mut resources = match linux.resources() {
                Some(r) => r.clone(),
                None => oci_spec::runtime::LinuxResources::default(),
            };

            if let Some(memory_limit) = limits.memory {
                let mut memory_builder =
                    oci_spec::runtime::LinuxMemoryBuilder::default().limit(memory_limit as i64);
                if let Some(existing) = resources.memory() {
                    if let Some(reservation) = existing.reservation() {
                        memory_builder = memory_builder.reservation(reservation);
                    }
                    if let Some(swap) = existing.swap() {
                        memory_builder = memory_builder.swap(swap);
                    }
                }
                let memory = memory_builder
                    .build()
                    .context("Failed to build memory limits")?;
                resources.set_memory(Some(memory));
            }

            if limits.cpu_quota.is_some()
                || limits.cpu_period.is_some()
                || limits.cpu_shares.is_some()
            {
                let mut cpu = match resources.cpu() {
                    Some(c) => c.clone(),
                    None => oci_spec::runtime::LinuxCpu::default(),
                };
                if let Some(quota) = limits.cpu_quota {
                    cpu.set_quota(Some(quota));
                }
                if let Some(period) = limits.cpu_period {
                    cpu.set_period(Some(period));
                }
                if let Some(shares) = limits.cpu_shares {
                    cpu.set_shares(Some(shares));
                }
                resources.set_cpu(Some(cpu));
            }

            if let Some(pids_limit) = limits.pids_limit {
                let pids = oci_spec::runtime::LinuxPidsBuilder::default()
                    .limit(pids_limit)
                    .build()
                    .context("Failed to build pids limit")?;
                resources.set_pids(Some(pids));
            }

            linux.set_resources(Some(resources));
        }

        // Attach a seccomp profile when one is supplied via
        // `--security-opt seccomp=<path>`. "unconfined" or a privileged
        // container disables seccomp filtering entirely.
        if !config.privileged
            && let Some(ref seccomp) = config.seccomp
            && seccomp != "unconfined"
        {
            let profile = Self::load_seccomp_profile(seccomp)?;
            linux.set_seccomp(Some(profile));
        }

        spec_builder.set_linux(Some(linux));

        Ok(spec_builder)
    }

    /// Helper to format CPU cores into cpuset format
    fn format_cpuset(cores: &[usize]) -> String {
        if cores.is_empty() {
            return String::new();
        }

        let mut ranges = Vec::new();
        let mut start = cores[0];
        let mut end = cores[0];

        for &core in &cores[1..] {
            if core == end + 1 {
                end = core;
            } else {
                if start == end {
                    ranges.push(format!("{}", start));
                } else {
                    ranges.push(format!("{}-{}", start, end));
                }
                start = core;
                end = core;
            }
        }

        // Add final range
        if start == end {
            ranges.push(format!("{}", start));
        } else {
            ranges.push(format!("{}-{}", start, end));
        }

        ranges.join(",")
    }

    fn validate_run_options(config: &NativeContainerConfig) -> Result<()> {
        if config.rm && config.detach {
            return Err(anyhow!(
                "`bolt run --rm --detach` is not supported by the native runtime yet; run attached with `--rm` or remove the detached container explicitly with `bolt rm`"
            )
            .into());
        }

        if let Some(ref network) = config.network {
            match network.as_str() {
                "bridge" | "bolt" | "host" | "none" => {}
                _ => {
                    return Err(anyhow!(
                        "`--network {}` is not supported by the native runtime yet (supported: bridge, bolt, host, none)",
                        network
                    )
                    .into());
                }
            }
        }

        if let Some(cpus) = config.cpus
            && (!cpus.is_finite() || cpus <= 0.0)
        {
            return Err(anyhow!("`--cpus` must be a positive number").into());
        }

        if let Some(ref memory) = config.memory {
            Self::parse_memory_limit(memory)?;
        }

        for cap in config.cap_add.iter().chain(config.cap_drop.iter()) {
            Self::parse_capability(cap)?;
        }

        Ok(())
    }

    /// Load an OCI seccomp profile from a JSON file. The file must match the
    /// standard runtime-spec `LinuxSeccomp` schema (as produced by Docker's
    /// default profile, `containerd`, etc.).
    fn load_seccomp_profile(path: &str) -> Result<oci_spec::runtime::LinuxSeccomp> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read seccomp profile: {}", path))?;
        let profile: oci_spec::runtime::LinuxSeccomp = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse seccomp profile: {}", path))?;
        Ok(profile)
    }

    fn resource_limits_for(config: &NativeContainerConfig) -> Result<Option<ResourceLimits>> {
        let memory = config
            .memory
            .as_deref()
            .map(Self::parse_memory_limit)
            .transpose()?;

        let (cpu_period, cpu_quota, cpu_shares) = if let Some(cpus) = config.cpus {
            let period = 100_000_u64;
            let quota = (cpus as f64 * period as f64).round() as i64;
            let shares = (cpus as f64 * 1024.0).round().clamp(2.0, 262_144.0) as u64;
            (Some(period), Some(quota.max(1)), Some(shares))
        } else {
            (None, None, None)
        };

        let pids_limit = config.pids_limit;

        if memory.is_none()
            && cpu_quota.is_none()
            && cpu_period.is_none()
            && cpu_shares.is_none()
            && pids_limit.is_none()
        {
            return Ok(None);
        }

        Ok(Some(ResourceLimits {
            memory,
            cpu_shares,
            cpu_quota,
            cpu_period,
            pids_limit,
        }))
    }

    fn parse_memory_limit(value: &str) -> Result<u64> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("memory limit cannot be empty").into());
        }

        let split_at = trimmed
            .find(|ch: char| !ch.is_ascii_digit())
            .unwrap_or(trimmed.len());
        let (number, suffix) = trimmed.split_at(split_at);
        if number.is_empty() {
            return Err(anyhow!("invalid memory limit '{}'", value).into());
        }

        let amount: u64 = number
            .parse()
            .with_context(|| format!("invalid memory limit '{}'", value))?;
        let multiplier = match suffix.trim().to_ascii_lowercase().as_str() {
            "" | "b" => 1,
            "k" | "kb" | "kib" => 1024,
            "m" | "mb" | "mib" => 1024 * 1024,
            "g" | "gb" | "gib" => 1024 * 1024 * 1024,
            "t" | "tb" | "tib" => 1024_u64.pow(4),
            other => return Err(anyhow!("unsupported memory suffix '{}'", other).into()),
        };

        amount
            .checked_mul(multiplier)
            .ok_or_else(|| anyhow!("memory limit '{}' is too large", value).into())
    }

    fn capabilities_for(config: &NativeContainerConfig) -> Result<Vec<String>> {
        if config.privileged {
            return Ok(Self::all_capability_names());
        }

        let mut capabilities = Self::default_capability_names();

        for cap in &config.cap_add {
            let normalized = Self::normalize_capability_name(cap)?;
            if normalized == "ALL" {
                capabilities = Self::all_capability_names();
            } else if !capabilities.contains(&normalized) {
                capabilities.push(normalized);
            }
        }

        for cap in &config.cap_drop {
            let normalized = Self::normalize_capability_name(cap)?;
            if normalized == "ALL" {
                capabilities.clear();
            } else {
                capabilities.retain(|existing| existing != &normalized);
            }
        }

        capabilities.sort();
        Ok(capabilities)
    }

    fn linux_capabilities_for(
        config: &ContainerConfig,
    ) -> Result<oci_spec::runtime::LinuxCapabilities> {
        let parsed = config
            .capabilities
            .iter()
            .map(|cap| Self::parse_capability(cap))
            .collect::<Result<Vec<_>>>()?;
        let set = parsed.into_iter().collect::<Capabilities>();

        let mut capabilities = oci_spec::runtime::LinuxCapabilities::default();
        capabilities.set_bounding(Some(set.clone()));
        capabilities.set_effective(Some(set.clone()));
        capabilities.set_inheritable(Some(set.clone()));
        capabilities.set_permitted(Some(set.clone()));
        capabilities.set_ambient(Some(set));
        Ok(capabilities)
    }

    fn default_capability_names() -> Vec<String> {
        ["CAP_AUDIT_WRITE", "CAP_KILL", "CAP_NET_BIND_SERVICE"]
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    fn all_capability_names() -> Vec<String> {
        [
            "CAP_AUDIT_CONTROL",
            "CAP_AUDIT_READ",
            "CAP_AUDIT_WRITE",
            "CAP_BLOCK_SUSPEND",
            "CAP_BPF",
            "CAP_CHECKPOINT_RESTORE",
            "CAP_CHOWN",
            "CAP_DAC_OVERRIDE",
            "CAP_DAC_READ_SEARCH",
            "CAP_FOWNER",
            "CAP_FSETID",
            "CAP_IPC_LOCK",
            "CAP_IPC_OWNER",
            "CAP_KILL",
            "CAP_LEASE",
            "CAP_LINUX_IMMUTABLE",
            "CAP_MAC_ADMIN",
            "CAP_MAC_OVERRIDE",
            "CAP_MKNOD",
            "CAP_NET_ADMIN",
            "CAP_NET_BIND_SERVICE",
            "CAP_NET_BROADCAST",
            "CAP_NET_RAW",
            "CAP_PERFMON",
            "CAP_SETFCAP",
            "CAP_SETGID",
            "CAP_SETPCAP",
            "CAP_SETUID",
            "CAP_SYS_ADMIN",
            "CAP_SYS_BOOT",
            "CAP_SYS_CHROOT",
            "CAP_SYS_MODULE",
            "CAP_SYS_NICE",
            "CAP_SYS_PACCT",
            "CAP_SYS_PTRACE",
            "CAP_SYS_RAWIO",
            "CAP_SYS_RESOURCE",
            "CAP_SYS_TIME",
            "CAP_SYS_TTY_CONFIG",
            "CAP_SYSLOG",
            "CAP_WAKE_ALARM",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    }

    fn normalize_capability_name(value: &str) -> Result<String> {
        let trimmed = value.trim();
        if trimmed.eq_ignore_ascii_case("all") {
            return Ok("ALL".to_string());
        }

        let upper = trimmed.to_ascii_uppercase().replace('-', "_");
        let normalized = if upper.starts_with("CAP_") {
            upper
        } else {
            format!("CAP_{}", upper)
        };
        Self::parse_capability(&normalized)?;
        Ok(normalized)
    }

    fn parse_capability(value: &str) -> Result<Capability> {
        if value.eq_ignore_ascii_case("all") {
            return Err(
                anyhow!("ALL is a capability set keyword, not an individual capability").into(),
            );
        }

        let parse_value = value
            .strip_prefix("CAP_")
            .or_else(|| value.strip_prefix("cap_"))
            .unwrap_or(value);

        parse_value
            .parse::<Capability>()
            .with_context(|| format!("invalid Linux capability '{}'", value))
            .map_err(Into::into)
    }

    /// Resolve volume source path (handle named volumes and host paths)
    async fn resolve_volume_source(&self, source: &str) -> Result<String> {
        // Check if it's an absolute path (host mount)
        if source.starts_with('/') {
            info!("🗂️ Using host path mount: {}", source);

            // Ensure the host path exists
            if !std::path::Path::new(source).exists() {
                return Err(anyhow!("Host path does not exist: {}", source).into());
            }

            return Ok(source.to_string());
        }

        // Handle named volumes from our volume management system
        info!("📦 Resolving named volume: {}", source);

        // Use our volume manager to get the volume path
        let volume_manager =
            crate::volume::VolumeManager::new_async(self.volume_storage_root()).await?;

        if let Some(mount_path) = volume_manager.get_volume_mount_path(source) {
            let path_str = mount_path.to_string_lossy().to_string();
            info!("✅ Named volume resolved: {} -> {}", source, path_str);

            // Ensure volume directory exists
            tokio::fs::create_dir_all(&mount_path)
                .await
                .with_context(|| format!("Failed to create volume directory: {}", path_str))?;

            Ok(path_str)
        } else {
            Err(anyhow!("Named volume '{}' not found", source).into())
        }
    }

    fn volume_storage_root(&self) -> PathBuf {
        self.runtime_dir
            .parent()
            .unwrap_or(&self.runtime_dir)
            .to_path_buf()
    }

    async fn mark_container_volumes_attached(&self, state: &ContainerState) -> Result<()> {
        let mut volume_manager =
            crate::volume::VolumeManager::new_async(self.volume_storage_root()).await?;
        for volume in &state.config.volumes {
            if volume.source.starts_with('/') {
                continue;
            }
            volume_manager
                .attach_volume_async(&volume.source, &state.id)
                .await?;
        }
        Ok(())
    }

    async fn mark_container_volumes_detached(
        &self,
        container_id: &str,
        volumes: &[crate::runtime::oci::VolumeMount],
    ) -> Result<()> {
        let mut volume_manager =
            crate::volume::VolumeManager::new_async(self.volume_storage_root()).await?;
        for volume in volumes {
            if volume.source.starts_with('/') {
                continue;
            }
            volume_manager
                .detach_volume_async(&volume.source, container_id)
                .await?;
        }
        Ok(())
    }

    /// Setup container networking with advanced optimization
    async fn setup_container_networking(
        &mut self,
        container_id: &str,
        config: &NativeContainerConfig,
    ) -> Result<()> {
        if self.rootless {
            warn!(
                "Rootless mode detected; skipping privileged network setup for container {}",
                container_id
            );
            return Ok(());
        }

        match config.network.as_deref().unwrap_or("bridge") {
            "host" => {
                info!(
                    "🌐 Using host networking for container {}; skipping private namespace setup",
                    container_id
                );
                self.container_network_modes
                    .insert(container_id.to_string(), "host".to_string());
                return Ok(());
            }
            "none" => {
                info!(
                    "🌐 Network disabled for container {}; no interfaces will be attached",
                    container_id
                );
                self.container_network_modes
                    .insert(container_id.to_string(), "none".to_string());
                return Ok(());
            }
            "bridge" | "bolt" => {}
            other => {
                return Err(anyhow!("unsupported network mode '{}'", other).into());
            }
        }

        info!(
            "🌐 Setting up advanced networking for container: {}",
            container_id
        );

        let port_mappings = config
            .ports
            .iter()
            .map(|port| self.parse_network_port_mapping(port))
            .collect::<Result<Vec<_>>>()?;

        let network_config = ContainerNetworkConfig {
            port_mappings,
            bandwidth_limit: None,
            latency_target: if self.gaming_mode { Some(100) } else { None },
            dns_servers: vec![],
        };

        let network_id = match &self.default_network_id {
            Some(id) => id.clone(),
            None => {
                let performance = if self.gaming_mode {
                    NetworkPerformanceMode::Gaming
                } else {
                    NetworkPerformanceMode::Balanced
                };
                let created = self
                    .network_manager
                    .create_network(
                        "bolt-native",
                        NetworkDriver::BoltBridge,
                        "172.18.0.0/16",
                        performance,
                    )
                    .await?;
                self.default_network_id = Some(created.clone());
                created
            }
        };

        self.network_manager
            .connect_container(&network_id, container_id, network_config)
            .await?;

        self.container_networks
            .insert(container_id.to_string(), network_id);
        self.container_network_modes
            .insert(container_id.to_string(), "bridge".to_string());

        Ok(())
    }

    fn parse_network_port_mapping(&self, value: &str) -> Result<NetworkPortMapping> {
        let (host, container_proto) = value.split_once(':').ok_or_else(|| {
            anyhow!(
                "invalid port mapping '{}'; expected host:container[/proto]",
                value
            )
        })?;
        let (container, proto) = container_proto
            .split_once('/')
            .map_or((container_proto, "tcp"), |(port, proto)| (port, proto));
        let protocol = match proto.to_ascii_lowercase().as_str() {
            "tcp" => NetworkProtocol::Tcp,
            "udp" => NetworkProtocol::Udp,
            "quic" => NetworkProtocol::Quic,
            other => return Err(anyhow!("unsupported port protocol '{}'", other).into()),
        };

        Ok(NetworkPortMapping {
            host_port: host
                .parse()
                .with_context(|| format!("invalid host port in '{}'", value))?,
            container_port: container
                .parse()
                .with_context(|| format!("invalid container port in '{}'", value))?,
            protocol,
            quic_enabled: self.gaming_mode || proto.eq_ignore_ascii_case("quic"),
        })
    }

    async fn teardown_container_networking(&mut self, container_id: &str) -> Result<()> {
        if self.rootless {
            return Ok(());
        }

        if let Some(network_id) = self.container_networks.get(container_id).cloned()
            && let Err(err) = self
                .network_manager
                .disconnect_container(&network_id, container_id)
                .await
        {
            warn!(
                "Failed to disconnect container {} from network {}: {}",
                container_id, network_id, err
            );
        }

        if let Err(err) = oci::cleanup_network_namespace(container_id).await {
            warn!(
                "Failed to clean up network namespace for container {}: {}",
                container_id, err
            );
        }

        self.container_networks.remove(container_id);
        self.container_network_modes.remove(container_id);

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

            // Detect process exit and persist the terminal state so that
            // `ps -a`/`logs`/`rm` report the truth after the process is gone.
            match state::load(&container_id) {
                Ok(Some(mut st)) => {
                    if state::reconcile_liveness(&mut st) {
                        if let Err(err) = state::save(&st) {
                            warn!(
                                "Failed to persist exit state for container {}: {}",
                                container_id, err
                            );
                        }
                        info!("📦 Container {} exited; monitoring stopped", container_id);
                        break;
                    }
                }
                Ok(None) => break, // state removed (rm) — stop monitoring
                Err(err) => warn!(
                    "Failed to read state while monitoring container {}: {}",
                    container_id, err
                ),
            }

            // Monitor security
            if let Ok(security_metrics) = security_manager
                .monitor_container_security(&container_id)
                .await
                && matches!(
                    security_metrics.threat_level,
                    super::security::ThreatLevel::High | super::security::ThreatLevel::Critical
                )
            {
                warn!(
                    "🚨 High threat level detected for container: {}",
                    container_id
                );
            }

            // Monitor performance
            if let Ok(perf_metrics) = performance_optimizer
                .monitor_performance(&container_id)
                .await
            {
                debug!(
                    "📊 Container {} performance: CPU {:.1}%, Memory {}MB",
                    container_id,
                    perf_metrics.cpu_usage,
                    perf_metrics.memory_usage / 1024 / 1024
                );
            }
        }
    }

    /// Get enhanced container metrics including GPU
    pub async fn get_container_metrics(
        &self,
        container_id: &str,
    ) -> Result<(SecurityMetrics, PerformanceMetrics, Option<GpuMetrics>)> {
        let security_metrics = self
            .security_manager
            .monitor_container_security(container_id)
            .await?;
        let performance_metrics = self
            .performance_optimizer
            .monitor_performance(container_id)
            .await?;

        // Get GPU metrics if GPU is enabled for this container
        let gpu_metrics = self
            .gpu_integration
            .get_gpu_metrics(container_id)
            .await
            .ok();

        Ok((security_metrics, performance_metrics, gpu_metrics))
    }

    /// Benchmark container performance
    pub async fn benchmark_container(&self, container_id: &str) -> Result<BenchmarkResults> {
        info!(
            "🏁 Running comprehensive benchmark for container: {}",
            container_id
        );
        self.performance_optimizer
            .benchmark_container(container_id)
            .await
    }

    /// Enable gaming mode for existing runtime
    pub async fn enable_gaming_mode(&mut self) -> Result<()> {
        info!("🎮 Enabling gaming mode for runtime");
        self.gaming_mode = true;

        // Reinitialize performance optimizer with gaming settings
        self.performance_optimizer = BoltPerformanceOptimizer::new(true);

        // Apply gaming optimizations to all running containers
        for container_id in self.containers.keys() {
            if let Err(e) = self
                .performance_optimizer
                .optimize_container(container_id)
                .await
            {
                warn!(
                    "Failed to apply gaming optimizations to container {}: {}",
                    container_id, e
                );
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
            devices: Some("all".to_string()),
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
            quick_sync: None, // Intel Quick Sync not used for gaming
        };

        let config = NativeContainerConfig {
            image: image.to_string(),
            name: name.map(|s| s.to_string()),
            ports: vec![],
            env: vec![],
            volumes: vec![],
            detach: true,
            rm: false,
            command: None,
            entrypoint: None,
            working_dir: None,
            user: None,
            hostname: None,
            cpus: None,
            memory: None,
            network: None,
            cap_add: vec![],
            cap_drop: vec![],
            privileged: false,
            tty: false,
            interactive: false,
            readonly_rootfs: false,
            pids_limit: None,
            seccomp: None,
            gpu_config: Some(gpu_config),
            cpu_affinity: None,
            workload_hint: Some(WorkloadHint::Gaming),
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
            devices: Some("all".to_string()),
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
            quick_sync: None, // Intel Quick Sync not used for AI/ML
        };

        let config = NativeContainerConfig {
            image: image.to_string(),
            name: name.map(|s| s.to_string()),
            ports: vec![],
            env: vec![],
            volumes: vec![],
            detach: true,
            rm: false,
            command: None,
            entrypoint: None,
            working_dir: None,
            user: None,
            hostname: None,
            cpus: None,
            memory: None,
            network: None,
            cap_add: vec![],
            cap_drop: vec![],
            privileged: false,
            tty: false,
            interactive: false,
            readonly_rootfs: false,
            pids_limit: None,
            seccomp: None,
            gpu_config: Some(gpu_config),
            cpu_affinity: None,
            workload_hint: Some(WorkloadHint::Gaming),
        };

        self.run_container(config).await
    }

    /// Check if nvbind GPU acceleration is available
    pub fn is_nvbind_available(&self) -> bool {
        self.gpu_integration.is_nvbind_available()
    }

    async fn finalize_container_networking(&self, container_id: &str, pid: i32) -> Result<()> {
        if self.rootless {
            return Ok(());
        }

        match self
            .container_network_modes
            .get(container_id)
            .map(String::as_str)
            .unwrap_or("bridge")
        {
            "host" => return Ok(()),
            "none" => {
                self.network_manager
                    .configure_loopback_only_namespace(pid)
                    .await?;
                return Ok(());
            }
            _ => {}
        }

        let network_id = match self.container_networks.get(container_id) {
            Some(id) => id.clone(),
            None => return Ok(()),
        };

        self.network_manager
            .configure_container_namespace(&network_id, container_id, pid)
            .await?;

        Ok(())
    }

    fn process_user_for(&self, config: &ContainerConfig) -> Option<oci_spec::runtime::User> {
        if let Some(ref user_str) = config.user {
            return Self::parse_user_spec(user_str);
        }

        if self.rootless {
            #[cfg(unix)]
            {
                let mut user = oci_spec::runtime::User::default();
                user.set_uid(0);
                user.set_gid(0);
                return Some(user);
            }
        }

        None
    }

    fn parse_user_spec(value: &str) -> Option<oci_spec::runtime::User> {
        use oci_spec::runtime::User;

        if value.is_empty() {
            return None;
        }

        let mut user = User::default();

        if let Some((uid_str, gid_str)) = value.split_once(':') {
            if let Ok(uid) = uid_str.parse::<u32>() {
                user.set_uid(uid);
                if let Ok(gid) = gid_str.parse::<u32>() {
                    user.set_gid(gid);
                }
                return Some(user);
            }
        } else if let Ok(uid) = value.parse::<u32>() {
            user.set_uid(uid);
            return Some(user);
        }

        None
    }

    fn detect_rootless_mode() -> bool {
        if env::var_os("BOLT_FORCE_ROOTFUL").is_some() {
            return false;
        }
        if env::var_os("BOLT_FORCE_ROOTLESS").is_some() {
            return true;
        }

        #[cfg(unix)]
        {
            !Uid::current().is_root()
        }

        #[cfg(not(unix))]
        {
            false
        }
    }

    fn resolve_runtime_dir(rootless: bool) -> Result<PathBuf> {
        if let Some(dir) = env::var_os("BOLT_RUNTIME_DIR") {
            let path = PathBuf::from(dir);
            Self::ensure_writable_runtime_dir(&path)?;
            return Ok(path);
        }

        if rootless {
            let candidates = [dirs::runtime_dir(), dirs::data_dir()];

            for base in candidates.into_iter().flatten() {
                let path = base.join("bolt").join("runtime");
                if Self::ensure_writable_runtime_dir(&path).is_ok() {
                    return Ok(path);
                }
            }

            Err(anyhow!("no writable rootless runtime directory found").into())
        } else {
            Ok(PathBuf::from("/run/bolt"))
        }
    }

    fn ensure_writable_runtime_dir(path: &Path) -> Result<()> {
        fs::create_dir_all(path)
            .with_context(|| format!("Failed to create runtime directory at {}", path.display()))?;
        let probe = path.join(".bolt-write-test");
        fs::write(&probe, b"ok")
            .with_context(|| format!("Runtime directory {} is not writable", path.display()))?;
        let _ = fs::remove_file(probe);
        Ok(())
    }
}

/// Directory where container logs are captured. Honors `BOLT_LOG_DIR`; must
/// match the resolution used by `crate::cli::logs`.
pub fn container_log_dir() -> PathBuf {
    env::var_os("BOLT_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/log/bolt/containers"))
}

/// Path to a container's captured stdout/stderr log file.
pub fn container_log_path(container_id: &str) -> PathBuf {
    container_log_dir().join(format!("{}.log", container_id))
}

/// Short human-readable summary of a GPU allocation for persisted state.
fn describe_gpu_allocation(gpu: &GpuConfig) -> String {
    let mut summary = format!("{:?}/{:?}", gpu.workload_type, gpu.isolation_level);
    if let Some(ref mem) = gpu.memory_limit {
        summary.push_str(&format!(" mem={}", mem));
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::gpu_integration::CdiDeviceNode;
    use oci_spec::runtime::{Capability, LinuxNamespaceType};

    fn test_config() -> NativeContainerConfig {
        NativeContainerConfig {
            image: "alpine:latest".to_string(),
            name: Some("web".to_string()),
            ports: vec!["8080:80".to_string()],
            env: vec!["APP_ENV=prod".to_string()],
            volumes: vec![],
            detach: true,
            rm: false,
            command: Some(vec!["echo".to_string(), "ok".to_string()]),
            entrypoint: Some(vec!["/bin/custom".to_string()]),
            working_dir: Some("/srv/app".to_string()),
            user: Some("1000:1001".to_string()),
            hostname: Some("bolt-web".to_string()),
            cpus: Some(1.5),
            memory: Some("512m".to_string()),
            network: Some("bridge".to_string()),
            cap_add: vec!["NET_ADMIN".to_string()],
            cap_drop: vec!["KILL".to_string()],
            privileged: false,
            tty: true,
            interactive: true,
            readonly_rootfs: false,
            pids_limit: None,
            seccomp: None,
            gpu_config: None,
            cpu_affinity: None,
            workload_hint: None,
        }
    }

    #[tokio::test]
    async fn run_options_are_reflected_in_oci_spec() {
        let runtime = BoltNativeRuntime::new()
            .await
            .expect("native runtime should initialize for spec-only test");
        let native_config = test_config();
        let container_config = runtime
            .create_container_config("bolt-test", &native_config)
            .await
            .expect("container config should build");
        let spec = runtime
            .create_oci_spec("bolt-test", &container_config, None, None)
            .await
            .expect("oci spec should build");

        assert_eq!(spec.hostname(), &Some("bolt-web".to_string()));

        let process = spec.process().as_ref().expect("process should be set");
        assert_eq!(
            process.args().as_ref().expect("args should be set"),
            &vec![
                "/bin/custom".to_string(),
                "echo".to_string(),
                "ok".to_string()
            ]
        );
        assert_eq!(process.cwd(), &PathBuf::from("/srv/app"));
        assert_eq!(process.terminal(), Some(true));
        assert_eq!(process.user().uid(), 1000);
        assert_eq!(process.user().gid(), 1001);
        assert!(
            process
                .env()
                .as_ref()
                .expect("env should be set")
                .contains(&"APP_ENV=prod".to_string())
        );

        let capabilities = process
            .capabilities()
            .as_ref()
            .expect("capabilities should be set");
        let effective = capabilities
            .effective()
            .as_ref()
            .expect("effective capabilities should be set");
        assert!(effective.contains(&Capability::NetAdmin));
        assert!(!effective.contains(&Capability::Kill));

        let linux = spec.linux().as_ref().expect("linux config should be set");
        let resources = linux.resources().as_ref().expect("resources should be set");
        assert_eq!(
            resources
                .memory()
                .as_ref()
                .expect("memory limit should be set")
                .limit(),
            Some(512 * 1024 * 1024)
        );
        let cpu = resources.cpu().as_ref().expect("cpu limits should be set");
        assert_eq!(cpu.period(), Some(100_000));
        assert_eq!(cpu.quota(), Some(150_000));
    }

    #[tokio::test]
    async fn host_networking_omits_oci_network_namespace() {
        let mut config = test_config();
        config.network = Some("bridge".to_string());
        let bridge = spec_for(&config).await;
        assert!(
            bridge
                .linux()
                .as_ref()
                .and_then(|linux| linux.namespaces().as_ref())
                .expect("bridge namespaces")
                .iter()
                .any(|ns| ns.typ() == LinuxNamespaceType::Network)
        );

        config.network = Some("host".to_string());
        let host = spec_for(&config).await;
        assert!(
            !host
                .linux()
                .as_ref()
                .and_then(|linux| linux.namespaces().as_ref())
                .expect("host namespaces")
                .iter()
                .any(|ns| ns.typ() == LinuxNamespaceType::Network)
        );
    }

    #[tokio::test]
    async fn native_port_mapping_parses_protocols_and_rejects_bad_input() {
        let runtime = BoltNativeRuntime::new()
            .await
            .expect("native runtime should initialize for parser test");

        let tcp = runtime
            .parse_network_port_mapping("8080:80")
            .expect("tcp port mapping");
        assert_eq!(tcp.host_port, 8080);
        assert_eq!(tcp.container_port, 80);
        assert!(matches!(tcp.protocol, NetworkProtocol::Tcp));
        assert!(!tcp.quic_enabled);

        let quic = runtime
            .parse_network_port_mapping("4433:4433/quic")
            .expect("quic port mapping");
        assert!(matches!(quic.protocol, NetworkProtocol::Quic));
        assert!(quic.quic_enabled);

        assert!(runtime.parse_network_port_mapping("bad").is_err());
        assert!(runtime.parse_network_port_mapping("8080:80/sctp").is_err());
    }

    async fn spec_for(config: &NativeContainerConfig) -> oci_spec::runtime::Spec {
        let runtime = BoltNativeRuntime::new()
            .await
            .expect("native runtime should initialize for spec-only test");
        let container_config = runtime
            .create_container_config("bolt-test", config)
            .await
            .expect("container config should build");
        runtime
            .create_oci_spec("bolt-test", &container_config, None, None)
            .await
            .expect("oci spec should build")
    }

    #[tokio::test]
    async fn readonly_rootfs_flag_sets_root_readonly() {
        let mut config = test_config();
        config.readonly_rootfs = true;
        let spec = spec_for(&config).await;
        let root = spec.root().as_ref().expect("root should be set");
        assert_eq!(root.readonly(), Some(true));

        config.readonly_rootfs = false;
        let spec = spec_for(&config).await;
        let root = spec.root().as_ref().expect("root should be set");
        assert_eq!(root.readonly(), Some(false));
    }

    #[tokio::test]
    async fn pids_limit_is_set_on_resources() {
        let mut config = test_config();
        config.pids_limit = Some(128);
        let spec = spec_for(&config).await;
        let linux = spec.linux().as_ref().expect("linux config should be set");
        let resources = linux.resources().as_ref().expect("resources should be set");
        let pids = resources.pids().as_ref().expect("pids limit should be set");
        assert_eq!(pids.limit(), 128);
    }

    #[tokio::test]
    async fn cdi_devices_are_added_to_spec_and_cgroup_allowlist() {
        let runtime = BoltNativeRuntime::new()
            .await
            .expect("native runtime should initialize for spec-only test");
        let native_config = test_config();
        let container_config = runtime
            .create_container_config("bolt-test", &native_config)
            .await
            .expect("container config should build");
        let device = CdiDeviceNode {
            path: "/dev/null".to_string(),
            device_type: Some("c".to_string()),
            major: Some(1),
            minor: Some(3),
            file_mode: Some(0o666),
            uid: Some(0),
            gid: Some(0),
        };
        let applied = AppliedCdiSpec {
            env: vec![],
            device_nodes: vec![device],
            mounts: vec![],
            hooks: vec![],
        };

        let spec = runtime
            .create_oci_spec("bolt-test", &container_config, Some(&applied), None)
            .await
            .expect("oci spec should build");

        let linux = spec.linux().as_ref().expect("linux config should be set");
        let devices = linux.devices().as_ref().expect("devices should be set");
        assert!(devices.iter().any(|device| {
            device.path() == &PathBuf::from("/dev/null")
                && device.major() == 1
                && device.minor() == 3
        }));

        let resources = linux.resources().as_ref().expect("resources should be set");
        let cgroup_devices = resources
            .devices()
            .as_ref()
            .expect("device cgroup rules should be set");
        assert!(cgroup_devices.iter().any(|rule| {
            rule.allow()
                && rule.major() == Some(1)
                && rule.minor() == Some(3)
                && rule.access().as_deref() == Some("rwm")
        }));
    }

    #[tokio::test]
    async fn seccomp_profile_attached_from_file() {
        let dir = scratch_tempdir();
        let profile_path = dir.path().join("seccomp.json");
        std::fs::write(&profile_path, r#"{"defaultAction":"SCMP_ACT_ALLOW"}"#)
            .expect("write profile");

        let mut config = test_config();
        config.seccomp = Some(profile_path.to_string_lossy().into_owned());
        let spec = spec_for(&config).await;
        let linux = spec.linux().as_ref().expect("linux config should be set");
        let seccomp = linux
            .seccomp()
            .as_ref()
            .expect("seccomp should be attached");
        assert_eq!(
            seccomp.default_action(),
            oci_spec::runtime::LinuxSeccompAction::ScmpActAllow
        );
    }

    #[tokio::test]
    async fn seccomp_unconfined_leaves_no_profile() {
        let mut config = test_config();
        config.seccomp = Some("unconfined".to_string());
        let spec = spec_for(&config).await;
        let linux = spec.linux().as_ref().expect("linux config should be set");
        assert!(linux.seccomp().is_none());
    }

    #[tokio::test]
    async fn privileged_container_skips_seccomp() {
        let dir = scratch_tempdir();
        let profile_path = dir.path().join("seccomp.json");
        std::fs::write(&profile_path, r#"{"defaultAction":"SCMP_ACT_ALLOW"}"#)
            .expect("write profile");

        let mut config = test_config();
        config.privileged = true;
        config.seccomp = Some(profile_path.to_string_lossy().into_owned());
        let spec = spec_for(&config).await;
        let linux = spec.linux().as_ref().expect("linux config should be set");
        assert!(linux.seccomp().is_none());
    }

    fn scratch_tempdir() -> tempfile::TempDir {
        std::fs::create_dir_all(".scratch").expect("create repo-local scratch directory");
        tempfile::tempdir_in(".scratch").expect("create repo-local scratch tempdir")
    }

    #[tokio::test]
    async fn privileged_container_gets_full_capability_set() {
        let mut config = test_config();
        config.privileged = true;
        let spec = spec_for(&config).await;
        let process = spec.process().as_ref().expect("process should be set");
        let capabilities = process
            .capabilities()
            .as_ref()
            .expect("capabilities should be set");
        let effective = capabilities
            .effective()
            .as_ref()
            .expect("effective capabilities should be set");
        // Privileged grants the full capability set, including ones not in
        // the default allow-list such as SysAdmin.
        assert!(effective.contains(&Capability::SysAdmin));
        assert!(effective.contains(&Capability::NetAdmin));
    }

    #[test]
    fn unsupported_native_run_flags_fail_clearly() {
        let mut config = test_config();

        config.rm = true;
        let err = BoltNativeRuntime::validate_run_options(&config)
            .expect_err("--rm should be rejected until native cleanup is implemented");
        assert!(err.to_string().contains("--rm"));

        config.rm = false;
        config.network = Some("container:abc".to_string());
        let err = BoltNativeRuntime::validate_run_options(&config)
            .expect_err("unsupported network mode should be rejected");
        assert!(err.to_string().contains("--network container:abc"));
    }
}
