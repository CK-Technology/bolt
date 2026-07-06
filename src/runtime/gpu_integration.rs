use crate::Result;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::spawn_blocking;
use tracing::{debug, info, warn};

/// Bolt GPU Integration Manager with native GPU support
pub struct BoltGpuIntegration {
    nvidia_manager: Option<super::gpu::nvbind::NvbindManager>,
    containers: Arc<RwLock<HashMap<String, GpuContainerInfo>>>,
    fallback_mode: bool,
    amd_backend: Option<AmdGpuBackend>,
    amd_monitor: Option<super::amd_metrics::AmdGpuMonitor>,
}

impl std::fmt::Debug for BoltGpuIntegration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoltGpuIntegration")
            .field("fallback_mode", &self.fallback_mode)
            .field("containers_count", &self.containers)
            .field("amd_backend", &self.amd_backend)
            .field(
                "amd_monitor",
                &self.amd_monitor.as_ref().map(|_| "initialized"),
            )
            .finish()
    }
}

/// GPU container information
#[derive(Debug, Clone)]
pub struct GpuContainerInfo {
    pub container_id: String,
    pub workload_type: GpuWorkloadType,
    pub isolation_level: GpuIsolationLevel,
    pub device_nodes: Vec<CdiDeviceNode>,
    pub optimization_applied: bool,
    pub cdi_mounts: Vec<CdiMount>,
    pub cdi_hooks: Vec<CdiHook>,
    pub cdi_env: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AppliedCdiSpec {
    pub env: Vec<String>,
    pub device_nodes: Vec<CdiDeviceNode>,
    pub mounts: Vec<CdiMount>,
    pub hooks: Vec<CdiHook>,
}

impl AppliedCdiSpec {
    pub fn is_empty(&self) -> bool {
        self.device_nodes.is_empty() && self.mounts.is_empty() && self.hooks.is_empty()
    }

    pub fn device_paths(&self) -> Vec<String> {
        self.device_nodes
            .iter()
            .map(|node| node.path.clone())
            .collect()
    }

    pub fn dedup(&mut self) {
        self.dedup_env();
        self.dedup_mounts();
        self.dedup_hooks();
        self.dedup_device_nodes();
    }

    fn dedup_env(&mut self) {
        let mut seen = HashSet::new();
        let mut deduped: Vec<String> = Vec::with_capacity(self.env.len());
        for entry in self.env.drain(..).rev() {
            let key = entry
                .split_once('=')
                .map(|(k, _)| k.to_string())
                .unwrap_or_else(|| entry.clone());
            if seen.insert(key) {
                deduped.push(entry);
            }
        }
        deduped.reverse();
        self.env = deduped;
    }

    fn dedup_mounts(&mut self) {
        let mut seen = HashSet::new();
        self.mounts.retain(|mount| {
            let mut options = mount.options.clone();
            options.sort();
            let key = (
                mount.host_path.clone(),
                mount.container_path.clone(),
                options,
            );
            seen.insert(key)
        });
    }

    fn dedup_hooks(&mut self) {
        let mut seen = HashSet::new();
        self.hooks.retain(|hook| {
            let mut args = hook.args.clone();
            let mut env = hook.env.clone();
            args.shrink_to_fit();
            env.shrink_to_fit();
            let key = (
                hook.hook_name.clone(),
                hook.path.clone(),
                args,
                env,
                hook.timeout,
            );
            seen.insert(key)
        });
    }

    fn dedup_device_nodes(&mut self) {
        let mut seen = HashSet::new();
        self.device_nodes.retain(|node| {
            let key = (node.path.clone(), node.major, node.minor);
            seen.insert(key)
        });
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct CdiMount {
    pub host_path: String,
    pub container_path: String,
    pub options: Vec<String>,
}

impl CdiMount {
    fn new(
        host_path: impl Into<String>,
        container_path: impl Into<String>,
        options: Vec<String>,
    ) -> Self {
        Self {
            host_path: host_path.into(),
            container_path: container_path.into(),
            options,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct CdiHook {
    pub hook_name: String,
    pub path: String,
    pub args: Vec<String>,
    pub env: Vec<String>,
    pub timeout: Option<u32>,
}

impl CdiHook {
    #[allow(dead_code)]
    fn new(
        hook_name: impl Into<String>,
        path: impl Into<String>,
        args: Vec<String>,
        env: Vec<String>,
        timeout: Option<u32>,
    ) -> Self {
        Self {
            hook_name: hook_name.into(),
            path: path.into(),
            args,
            env,
            timeout,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct CdiDeviceNode {
    pub path: String,
    pub device_type: Option<String>,
    pub major: Option<i64>,
    pub minor: Option<i64>,
    pub file_mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
}

impl CdiDeviceNode {
    fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            device_type: None,
            major: None,
            minor: None,
            file_mode: None,
            uid: None,
            gid: None,
        }
    }
}

// NOTE: External nvbind CDI conversions removed - native GPU support integrated directly

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuWorkloadType {
    Gaming {
        dlss_enabled: bool,
        raytracing_enabled: bool,
        performance_profile: String,
        wine_proton_enabled: bool,
        vrs_enabled: bool,
    },
    AiMl {
        cuda_cache_mb: Option<u64>,
        tensor_cores_enabled: bool,
        mixed_precision_enabled: bool,
        memory_pool_size: Option<String>,
        mig_enabled: bool,
    },
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuIsolationLevel {
    Shared,
    Exclusive,
    Virtual,
}

/// GPU configuration for containers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuConfig {
    pub enabled: bool,
    pub devices: Option<String>,
    pub workload_type: GpuWorkloadType,
    pub isolation_level: GpuIsolationLevel,
    pub memory_limit: Option<String>,
    pub snapshot_support: bool,
    pub quick_sync: Option<super::quick_sync::QuickSyncConfig>, // Intel hardware video
}

impl BoltGpuIntegration {
    /// Initialize GPU integration with native GPU support
    pub async fn new() -> Result<Self> {
        info!("🎮 Initializing Bolt GPU Integration (native)");

        // Initialize native NVIDIA GPU manager
        let nvidia_manager = Self::initialize_nvidia_manager().await;
        let fallback_mode = nvidia_manager.is_none();

        let amd_backend = match AmdGpuBackend::detect().await {
            Ok(Some(backend)) => {
                info!(
                    "✅ AMD GPU backend detected ({} device{})",
                    backend.device_count(),
                    if backend.device_count() == 1 { "" } else { "s" }
                );
                for summary in backend.device_summaries() {
                    info!("   • {}", summary);
                }
                Some(backend)
            }
            Ok(None) => {
                info!("ℹ️  No AMD GPUs detected on host");
                None
            }
            Err(err) => {
                warn!("⚠️  AMD GPU backend initialization failed: {}", err);
                None
            }
        };

        // Initialize AMD metrics monitor if backend is available
        let amd_monitor = if amd_backend.is_some() {
            match super::amd_metrics::AmdGpuMonitor::new() {
                Ok(monitor) => {
                    info!("✅ AMD GPU metrics monitor initialized (rocm-smi)");
                    Some(monitor)
                }
                Err(err) => {
                    warn!("⚠️  AMD GPU metrics monitor initialization failed: {}", err);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            nvidia_manager,
            containers: Arc::new(RwLock::new(HashMap::new())),
            fallback_mode,
            amd_backend,
            amd_monitor,
        })
    }

    /// Initialize native NVIDIA GPU manager
    async fn initialize_nvidia_manager() -> Option<super::gpu::nvbind::NvbindManager> {
        info!("🔍 Auto-detecting NVIDIA GPU capabilities (native)...");

        // Use native NvbindManager detection
        match super::gpu::nvbind::NvbindManager::detect() {
            Ok(manager) => {
                if manager.is_available {
                    info!("✅ Native NVIDIA GPU manager initialized successfully");
                    info!("🎯 Native GPU features available:");
                    info!("   • Direct GPU device passthrough");
                    info!("   • CDI specification generation");
                    info!("   • Driver detection (Open/Proprietary/Nouveau)");
                    info!("   • No nvidia-container-toolkit required");

                    if let Some(ref driver_info) = manager.driver_info {
                        info!(
                            "   • Driver: {} ({})",
                            driver_info.version,
                            driver_info.driver_type.name()
                        );
                        if let Some(ref cuda) = driver_info.cuda_version {
                            info!("   • CUDA: {}", cuda);
                        }
                    }

                    info!("   • GPUs detected: {}", manager.detected_gpus.len());
                    for gpu in &manager.detected_gpus {
                        info!("     - {}: {} ({:?})", gpu.id, gpu.name, gpu.architecture);
                    }

                    Some(manager)
                } else {
                    info!("ℹ️  No NVIDIA GPUs detected");
                    None
                }
            }
            Err(e) => {
                warn!("⚠️  Native NVIDIA GPU detection failed: {}", e);
                None
            }
        }
    }

    /// Setup GPU for container with nvbind optimization
    pub async fn setup_gpu_for_container(
        &self,
        container_id: &str,
        gpu_config: &GpuConfig,
    ) -> Result<AppliedCdiSpec> {
        if !gpu_config.enabled {
            return Ok(AppliedCdiSpec::default());
        }

        info!("🔧 Setting up GPU for container: {}", container_id);

        // Use native NVIDIA manager if available
        if let Some(ref nvidia_manager) = self.nvidia_manager {
            return self
                .setup_with_native_nvidia(container_id, gpu_config, nvidia_manager)
                .await;
        }

        if let Some(ref backend) = self.amd_backend
            && backend.is_available()
        {
            return self
                .setup_with_amd_backend(container_id, gpu_config, backend)
                .await;
        }

        // Fallback to basic GPU setup
        self.setup_gpu_fallback(container_id, gpu_config).await
    }

    /// Retrieve the CDI artifacts that were applied to a container during GPU setup.
    pub async fn applied_cdi_spec(&self, container_id: &str) -> Option<AppliedCdiSpec> {
        let containers = self.containers.read().await;
        containers.get(container_id).map(|info| AppliedCdiSpec {
            env: info.cdi_env.clone(),
            device_nodes: info.device_nodes.clone(),
            mounts: info.cdi_mounts.clone(),
            hooks: info.cdi_hooks.clone(),
        })
    }

    /// Setup GPU with native NVIDIA manager
    async fn setup_with_native_nvidia(
        &self,
        container_id: &str,
        gpu_config: &GpuConfig,
        nvidia_manager: &super::gpu::nvbind::NvbindManager,
    ) -> Result<AppliedCdiSpec> {
        info!(
            "🚀 Setting up GPU with native NVIDIA manager for container: {}",
            container_id
        );

        // Generate CDI specification based on workload type
        let cdi_spec = match &gpu_config.workload_type {
            GpuWorkloadType::Gaming { .. } => {
                info!("🎮 Generating gaming-optimized CDI spec (native)");
                nvidia_manager
                    .generate_gaming_cdi_spec()
                    .await
                    .map_err(|e| anyhow!("Failed to generate gaming CDI spec: {}", e))?
            }
            GpuWorkloadType::AiMl { .. } => {
                info!("🧠 Generating AI/ML-optimized CDI spec (native)");
                nvidia_manager
                    .generate_aiml_cdi_spec()
                    .await
                    .map_err(|e| anyhow!("Failed to generate AI/ML CDI spec: {}", e))?
            }
            GpuWorkloadType::General => {
                info!("📊 Generating general-purpose CDI spec (native)");
                nvidia_manager
                    .generate_default_cdi_spec()
                    .await
                    .map_err(|e| anyhow!("Failed to generate CDI spec: {}", e))?
            }
        };

        // Apply CDI specification to container context
        let applied_cdi = self.apply_native_cdi_spec(container_id, &cdi_spec).await?;

        // Setup GPU isolation
        self.setup_gpu_isolation(container_id, &gpu_config.isolation_level)
            .await?;

        // Apply workload-specific optimizations
        self.apply_workload_optimizations(container_id, &gpu_config.workload_type)
            .await?;

        // Configure GPU environment variables
        self.setup_gpu_environment(container_id, gpu_config, &applied_cdi)
            .await?;

        // Store container GPU info
        let container_info = GpuContainerInfo {
            container_id: container_id.to_string(),
            workload_type: gpu_config.workload_type.clone(),
            isolation_level: gpu_config.isolation_level.clone(),
            device_nodes: applied_cdi.device_nodes.clone(),
            optimization_applied: true,
            cdi_mounts: applied_cdi.mounts.clone(),
            cdi_hooks: applied_cdi.hooks.clone(),
            cdi_env: applied_cdi.env.clone(),
        };

        let mut containers = self.containers.write().await;
        containers.insert(container_id.to_string(), container_info);

        info!(
            "✅ Native NVIDIA GPU setup completed for container: {}",
            container_id
        );
        Ok(applied_cdi)
    }

    async fn setup_with_amd_backend(
        &self,
        container_id: &str,
        gpu_config: &GpuConfig,
        backend: &AmdGpuBackend,
    ) -> Result<AppliedCdiSpec> {
        info!(
            "🔥 Setting up AMD GPU for container: {} ({} devices)",
            container_id,
            backend.device_count()
        );

        let cdi_spec = backend.build_cdi_spec();
        let applied_cdi = self.apply_legacy_cdi_spec(container_id, &cdi_spec).await?;

        self.setup_gpu_isolation(container_id, &gpu_config.isolation_level)
            .await?;

        self.apply_workload_optimizations(container_id, &gpu_config.workload_type)
            .await?;

        self.configure_amd_environment(container_id, backend, gpu_config)
            .await?;

        // Configure GPU environment variables
        self.setup_gpu_environment(container_id, gpu_config, &applied_cdi)
            .await?;

        let container_info = GpuContainerInfo {
            container_id: container_id.to_string(),
            workload_type: gpu_config.workload_type.clone(),
            isolation_level: gpu_config.isolation_level.clone(),
            device_nodes: applied_cdi.device_nodes.clone(),
            optimization_applied: true,
            cdi_mounts: applied_cdi.mounts.clone(),
            cdi_hooks: applied_cdi.hooks.clone(),
            cdi_env: applied_cdi.env.clone(),
        };

        let mut containers = self.containers.write().await;
        containers.insert(container_id.to_string(), container_info);

        info!("✅ AMD GPU setup completed for container: {}", container_id);
        Ok(applied_cdi)
    }

    /// Fallback GPU setup without nvbind
    async fn setup_gpu_fallback(
        &self,
        container_id: &str,
        gpu_config: &GpuConfig,
    ) -> Result<AppliedCdiSpec> {
        warn!(
            "⚠️  Using fallback GPU setup for container: {}",
            container_id
        );

        // Basic GPU device detection and setup
        let gpu_devices = self.detect_gpu_devices().await?;

        if gpu_devices.is_empty() {
            return Err(anyhow!("No GPU devices found for container: {}", container_id).into());
        }

        // Apply basic GPU environment variables
        self.setup_basic_gpu_environment(container_id, &gpu_devices)
            .await?;

        let device_nodes = gpu_devices
            .iter()
            .cloned()
            .map(CdiDeviceNode::new)
            .collect();
        let mut applied_cdi = AppliedCdiSpec {
            env: Vec::new(),
            device_nodes,
            mounts: Vec::new(),
            hooks: Vec::new(),
        };
        applied_cdi.dedup();

        // Configure GPU environment variables
        self.setup_gpu_environment(container_id, gpu_config, &applied_cdi)
            .await?;

        // Store container info
        let container_info = GpuContainerInfo {
            container_id: container_id.to_string(),
            workload_type: gpu_config.workload_type.clone(),
            isolation_level: gpu_config.isolation_level.clone(),
            device_nodes: applied_cdi.device_nodes.clone(),
            optimization_applied: false,
            cdi_mounts: Vec::new(),
            cdi_hooks: Vec::new(),
            cdi_env: applied_cdi.env.clone(),
        };

        let mut containers = self.containers.write().await;
        containers.insert(container_id.to_string(), container_info);

        info!(
            "✅ Fallback GPU setup completed for container: {}",
            container_id
        );
        Ok(applied_cdi)
    }

    /// Apply native CDI spec to container
    async fn apply_native_cdi_spec(
        &self,
        container_id: &str,
        cdi_spec: &super::gpu::nvbind::CdiSpec,
    ) -> Result<AppliedCdiSpec> {
        info!("🔌 Applying native CDI spec to container: {}", container_id);

        // Convert native CdiSpec to AppliedCdiSpec
        let mut applied = AppliedCdiSpec {
            env: cdi_spec.container_edits.env.clone(),
            device_nodes: cdi_spec
                .container_edits
                .device_nodes
                .iter()
                .map(|node| CdiDeviceNode {
                    path: node.path.clone(),
                    device_type: node.device_type.clone(),
                    major: node.major,
                    minor: node.minor,
                    file_mode: node.file_mode,
                    uid: node.uid,
                    gid: node.gid,
                })
                .collect(),
            mounts: cdi_spec
                .container_edits
                .mounts
                .iter()
                .map(|mount| CdiMount {
                    host_path: mount.host_path.clone(),
                    container_path: mount.container_path.clone(),
                    options: mount.options.clone(),
                })
                .collect(),
            hooks: cdi_spec
                .container_edits
                .hooks
                .iter()
                .map(|hook| CdiHook {
                    hook_name: hook.hook_name.clone(),
                    path: hook.path.clone(),
                    args: hook.args.clone(),
                    env: hook.env.clone(),
                    timeout: hook.timeout,
                })
                .collect(),
        };

        // Also include device edits from individual devices
        for device in &cdi_spec.devices {
            for node in &device.container_edits.device_nodes {
                applied.device_nodes.push(CdiDeviceNode {
                    path: node.path.clone(),
                    device_type: node.device_type.clone(),
                    major: node.major,
                    minor: node.minor,
                    file_mode: node.file_mode,
                    uid: node.uid,
                    gid: node.gid,
                });
            }
        }

        applied.dedup();
        Self::log_cdi_artifacts(container_id, &applied);
        Ok(applied)
    }

    async fn apply_legacy_cdi_spec(
        &self,
        container_id: &str,
        cdi_spec: &CdiSpec,
    ) -> Result<AppliedCdiSpec> {
        info!("🔌 Applying legacy CDI spec to container: {}", container_id);

        let mut applied = AppliedCdiSpec {
            env: Vec::new(),
            device_nodes: cdi_spec
                .devices
                .as_ref()
                .map(|devices| devices.iter().cloned().map(CdiDeviceNode::new).collect())
                .unwrap_or_default(),
            mounts: cdi_spec
                .mounts
                .as_ref()
                .map(|mounts| {
                    mounts
                        .iter()
                        .map(|path| {
                            CdiMount::new(
                                path.clone(),
                                path.clone(),
                                vec![
                                    "bind".to_string(),
                                    "ro".to_string(),
                                    "nosuid".to_string(),
                                    "nodev".to_string(),
                                    "relatime".to_string(),
                                ],
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            hooks: Vec::new(),
        };

        applied.dedup();
        Self::log_cdi_artifacts(container_id, &applied);
        Ok(applied)
    }

    fn log_cdi_artifacts(container_id: &str, applied: &AppliedCdiSpec) {
        if !applied.device_nodes.is_empty() {
            for node in &applied.device_nodes {
                debug!(
                    "CDI device for {} => {} (major={:?}, minor={:?})",
                    container_id, node.path, node.major, node.minor
                );
            }
        }

        if !applied.mounts.is_empty() {
            for mount in &applied.mounts {
                debug!(
                    "CDI mount for {} => {} -> {} (options={:?})",
                    container_id, mount.host_path, mount.container_path, mount.options
                );
            }
        }

        if !applied.hooks.is_empty() {
            for hook in &applied.hooks {
                debug!(
                    "CDI hook for {} => {} {} (timeout={:?})",
                    container_id, hook.hook_name, hook.path, hook.timeout
                );
            }
        }

        if !applied.env.is_empty() {
            for env in &applied.env {
                debug!("CDI env for {} => {}", container_id, env);
            }
        }
    }

    /// Setup GPU isolation based on level
    async fn setup_gpu_isolation(
        &self,
        container_id: &str,
        isolation_level: &GpuIsolationLevel,
    ) -> Result<()> {
        match isolation_level {
            GpuIsolationLevel::Shared => {
                info!(
                    "🤝 Setting up shared GPU access for container: {}",
                    container_id
                );
                // Allow multiple containers to share GPU
            }
            GpuIsolationLevel::Exclusive => {
                info!(
                    "🔒 Setting up exclusive GPU access for container: {}",
                    container_id
                );
                // Give container exclusive GPU access
            }
            GpuIsolationLevel::Virtual => {
                info!(
                    "💻 Setting up virtual GPU access for container: {}",
                    container_id
                );
                // Virtual GPU with resource limits
            }
        }

        Ok(())
    }

    /// Apply workload-specific optimizations
    async fn apply_workload_optimizations(
        &self,
        container_id: &str,
        workload_type: &GpuWorkloadType,
    ) -> Result<()> {
        match workload_type {
            GpuWorkloadType::Gaming { .. } => {
                self.apply_gaming_optimizations(container_id, workload_type)
                    .await?;
            }
            GpuWorkloadType::AiMl { .. } => {
                self.apply_aiml_optimizations(container_id, workload_type)
                    .await?;
            }
            GpuWorkloadType::General => {
                info!(
                    "📊 Applying general GPU optimizations for container: {}",
                    container_id
                );
            }
        }

        Ok(())
    }

    /// Apply gaming-specific optimizations
    async fn apply_gaming_optimizations(
        &self,
        container_id: &str,
        workload_type: &GpuWorkloadType,
    ) -> Result<()> {
        if let GpuWorkloadType::Gaming {
            dlss_enabled,
            raytracing_enabled,
            performance_profile,
            wine_proton_enabled,
            vrs_enabled,
        } = workload_type
        {
            info!(
                "🎮 Applying gaming optimizations for container: {}",
                container_id
            );

            // Set performance governor
            if performance_profile == "ultra-low-latency" {
                self.set_cpu_governor(container_id, "performance").await?;
            }

            // Enable DLSS/RT cores
            if *dlss_enabled || *raytracing_enabled {
                self.enable_gaming_features(container_id, dlss_enabled, raytracing_enabled)
                    .await?;
            }

            // Wine/Proton optimizations
            if *wine_proton_enabled {
                self.setup_wine_environment(container_id).await?;
            }

            // Variable Rate Shading
            if *vrs_enabled {
                self.enable_vrs(container_id).await?;
            }
        }

        Ok(())
    }

    /// Apply AI/ML-specific optimizations
    async fn apply_aiml_optimizations(
        &self,
        container_id: &str,
        workload_type: &GpuWorkloadType,
    ) -> Result<()> {
        if let GpuWorkloadType::AiMl {
            cuda_cache_mb,
            tensor_cores_enabled,
            mixed_precision_enabled,
            memory_pool_size,
            mig_enabled,
        } = workload_type
        {
            info!(
                "🧠 Applying AI/ML optimizations for container: {}",
                container_id
            );

            // Set CUDA cache size
            if let Some(cache_size) = cuda_cache_mb {
                self.set_container_env(
                    container_id,
                    "CUDA_CACHE_MAXSIZE",
                    &(cache_size * 1024 * 1024).to_string(),
                )
                .await?;
            }

            // Enable tensor cores
            if *tensor_cores_enabled {
                self.set_container_env(container_id, "NVIDIA_TF32_OVERRIDE", "1")
                    .await?;
            }

            // Mixed precision support
            if *mixed_precision_enabled {
                self.set_container_env(container_id, "NVBIND_MIXED_PRECISION", "1")
                    .await?;
            }

            // Memory pool configuration
            if let Some(pool_size) = memory_pool_size {
                self.configure_gpu_memory_pool(container_id, pool_size)
                    .await?;
            }

            // Multi-Instance GPU (MIG)
            if *mig_enabled {
                self.enable_mig(container_id).await?;
            }
        }

        Ok(())
    }

    /// Setup GPU environment variables
    async fn setup_gpu_environment(
        &self,
        container_id: &str,
        gpu_config: &GpuConfig,
        applied_cdi: &AppliedCdiSpec,
    ) -> Result<()> {
        let device_paths = applied_cdi.device_paths();
        let requested_devices = gpu_config.devices.as_deref().unwrap_or("all");
        let assigned_devices = if requested_devices != "all" {
            requested_devices.to_string()
        } else if device_paths.is_empty() {
            "all".to_string()
        } else {
            device_paths.join(",")
        };

        // Identify isolation level for telemetry/env wiring
        let isolation = match gpu_config.isolation_level {
            GpuIsolationLevel::Shared => "shared",
            GpuIsolationLevel::Exclusive => "exclusive",
            GpuIsolationLevel::Virtual => "virtual",
        };

        // Base NVIDIA runtime configuration
        self.set_container_env(container_id, "BOLT_GPU_VENDOR", "nvidia")
            .await?;
        self.set_container_env(container_id, "BOLT_GPU_DRIVER", "nvidia")
            .await?;
        self.set_container_env(container_id, "BOLT_GPU_ISOLATION", isolation)
            .await?;
        self.set_container_env(container_id, "BOLT_GPU_DEVICES", &assigned_devices)
            .await?;

        // Map CDI devices to NVIDIA runtime expectations
        let nvidia_visible = if requested_devices != "all" {
            requested_devices.to_string()
        } else if !device_paths.is_empty()
            && device_paths.iter().all(|device| !device.contains('/'))
        {
            assigned_devices.clone()
        } else {
            "all".to_string()
        };
        self.set_container_env(container_id, "NVIDIA_VISIBLE_DEVICES", &nvidia_visible)
            .await?;
        self.set_container_env(
            container_id,
            "NVIDIA_DRIVER_CAPABILITIES",
            "compute,video,graphics,utility,display",
        )
        .await?;

        if !applied_cdi.mounts.is_empty() {
            let mounts = applied_cdi
                .mounts
                .iter()
                .map(|mount| format!("{}->{}", mount.host_path, mount.container_path))
                .collect::<Vec<_>>()
                .join(",");
            self.set_container_env(container_id, "BOLT_GPU_CDI_MOUNTS", &mounts)
                .await?;
        }

        if !applied_cdi.hooks.is_empty() {
            let hooks = applied_cdi
                .hooks
                .iter()
                .map(|hook| hook.path.clone())
                .collect::<Vec<_>>()
                .join(",");
            self.set_container_env(container_id, "BOLT_GPU_CDI_HOOKS", &hooks)
                .await?;
        }

        if !device_paths.is_empty() {
            self.set_container_env(
                container_id,
                "NVBIND_ASSIGNED_CDI_DEVICES",
                &assigned_devices,
            )
            .await?;
        }

        for env_entry in &applied_cdi.env {
            if let Some((key, value)) = env_entry.split_once('=') {
                self.set_container_env(container_id, key, value).await?;
            } else {
                self.set_container_env(container_id, env_entry, "1").await?;
            }
        }

        // Derive workload indicator for nvbind
        let workload = match &gpu_config.workload_type {
            GpuWorkloadType::Gaming { .. } => "gaming",
            GpuWorkloadType::AiMl { .. } => "aiml",
            GpuWorkloadType::General => "general",
        };

        // Set nvbind-specific environment
        if !self.fallback_mode {
            self.set_container_env(container_id, "NVBIND_ENABLED", "1")
                .await?;
            self.set_container_env(container_id, "NVBIND_RUNTIME", "bolt")
                .await?;
            self.set_container_env(container_id, "NVBIND_WORKLOAD", workload)
                .await?;
            self.set_container_env(container_id, "NVBIND_ISOLATION", isolation)
                .await?;
        }

        Ok(())
    }

    async fn configure_amd_environment(
        &self,
        container_id: &str,
        backend: &AmdGpuBackend,
        gpu_config: &GpuConfig,
    ) -> Result<()> {
        let mut node_list = backend.device_nodes();
        node_list.sort();
        node_list.dedup();
        let device_list = node_list.join(",");
        self.set_container_env(container_id, "BOLT_GPU_VENDOR", "amd")
            .await?;
        self.set_container_env(container_id, "BOLT_GPU_DRIVER", "amdgpu")
            .await?;
        self.set_container_env(container_id, "BOLT_GPU_DEVICES", &device_list)
            .await?;

        if let Some(render_nodes) = backend.render_nodes_csv() {
            self.set_container_env(container_id, "BOLT_RENDER_DEVICES", &render_nodes)
                .await?;
        }

        if let Some(pci_bus) = backend.pci_bus_list() {
            self.set_container_env(container_id, "BOLT_GPU_PCI_BUS_IDS", &pci_bus)
                .await?;
        }

        let visible_devices = backend.visible_device_list();
        self.set_container_env(container_id, "HIP_VISIBLE_DEVICES", &visible_devices)
            .await?;
        self.set_container_env(container_id, "ROCR_VISIBLE_DEVICES", &visible_devices)
            .await?;

        if backend.supports_rocm() {
            self.set_container_env(container_id, "BOLT_ROCM_ENABLED", "1")
                .await?;
            if let Some(root) = backend.rocm_root_env() {
                self.set_container_env(container_id, "ROCM_PATH", &root)
                    .await?;
            }
            if let Some(ld_path) = backend.rocm_ld_library_path() {
                self.set_container_env(container_id, "BOLT_ROCM_LIBRARY_PATH", &ld_path)
                    .await?;
            }
            if let Some(smi_path) = backend.rocm_smi_path() {
                self.set_container_env(container_id, "BOLT_ROCM_SMI_PATH", &smi_path)
                    .await?;
            }
        } else {
            self.set_container_env(container_id, "BOLT_ROCM_ENABLED", "0")
                .await?;
        }

        if backend.kfd_available() {
            self.set_container_env(container_id, "BOLT_KFD_AVAILABLE", "1")
                .await?;
            self.set_container_env(container_id, "HSA_ENABLE_SDMA", "1")
                .await?;
        }

        if let Some(icd) = backend.vulkan_icd_path() {
            self.set_container_env(container_id, "VK_ICD_FILENAMES", &icd)
                .await?;
        }

        if let Some(memory) = backend.total_memory_mb() {
            self.set_container_env(container_id, "BOLT_GPU_MEMORY_MB", &memory.to_string())
                .await?;
        }

        match &gpu_config.workload_type {
            GpuWorkloadType::Gaming { .. } => {
                self.set_container_env(container_id, "RADV_PERFTEST", "aco")
                    .await?;
                self.set_container_env(container_id, "BOLT_GPU_LATENCY_MODE", "gaming")
                    .await?;
            }
            GpuWorkloadType::AiMl { .. } => {
                self.set_container_env(container_id, "GPU_FORCE_64BIT_PTR", "1")
                    .await?;
                self.set_container_env(container_id, "BOLT_GPU_LATENCY_MODE", "compute")
                    .await?;
            }
            GpuWorkloadType::General => {
                self.set_container_env(container_id, "BOLT_GPU_LATENCY_MODE", "balanced")
                    .await?;
            }
        }

        Ok(())
    }

    /// Basic GPU environment setup (fallback)
    async fn setup_basic_gpu_environment(
        &self,
        container_id: &str,
        gpu_devices: &[String],
    ) -> Result<()> {
        let devices_str = gpu_devices.join(",");
        let has_nvidia = gpu_devices.iter().any(|device| device.contains("nvidia"));

        if has_nvidia {
            self.set_container_env(container_id, "NVIDIA_VISIBLE_DEVICES", "all")
                .await?;
            self.set_container_env(container_id, "NVIDIA_DRIVER_CAPABILITIES", "all")
                .await?;
            self.set_container_env(container_id, "BOLT_GPU_VENDOR", "nvidia")
                .await?;
            self.set_container_env(container_id, "BOLT_GPU_DRIVER", "nvidia")
                .await?;
        } else {
            self.set_container_env(container_id, "BOLT_GPU_VENDOR", "generic")
                .await?;
            self.set_container_env(container_id, "BOLT_GPU_DRIVER", "generic")
                .await?;
        }

        self.set_container_env(container_id, "BOLT_GPU_DEVICES", &devices_str)
            .await?;

        Ok(())
    }

    /// Detect available GPU devices (fallback)
    async fn detect_gpu_devices(&self) -> Result<Vec<String>> {
        if let Some(ref backend) = self.amd_backend
            && backend.is_available()
        {
            return Ok(backend.device_nodes());
        }

        let mut devices = Vec::new();

        if Path::new("/dev/nvidia0").exists() {
            devices.push("/dev/nvidia0".to_string());
        }
        if Path::new("/dev/dri/card0").exists() {
            devices.push("/dev/dri/card0".to_string());
        }
        if Path::new("/dev/dri/renderD128").exists() {
            devices.push("/dev/dri/renderD128".to_string());
        }
        if Path::new("/dev/kfd").exists() {
            devices.push("/dev/kfd".to_string());
        }

        if devices.is_empty() {
            warn!("No GPU devices detected via fallback path");
        } else {
            info!("Detected GPU devices: {:?}", devices);
        }

        Ok(devices)
    }

    /// Record an environment variable in the per-container env map. The native
    /// runtime merges this map into the OCI process env when building the spec
    /// (see `create_oci_spec`), so values land in the container rather than the
    /// host process.
    async fn set_container_env(&self, container_id: &str, key: &str, value: &str) -> Result<()> {
        debug!(
            "Setting environment variable for container {}: {}={}",
            container_id, key, value
        );
        crate::runtime::environment::env_manager().set_container_env(container_id, key, value)?;
        Ok(())
    }

    async fn set_cpu_governor(&self, container_id: &str, governor: &str) -> Result<()> {
        debug!(
            "Setting CPU governor for container {}: {}",
            container_id, governor
        );
        // In real implementation, this would set the CPU governor
        Ok(())
    }

    async fn enable_gaming_features(
        &self,
        container_id: &str,
        dlss: &bool,
        rt: &bool,
    ) -> Result<()> {
        debug!(
            "Enabling gaming features for container {}: DLSS={}, RT={}",
            container_id, dlss, rt
        );
        // In real implementation, this would enable DLSS/RT cores
        Ok(())
    }

    async fn setup_wine_environment(&self, container_id: &str) -> Result<()> {
        debug!(
            "Setting up Wine environment for container: {}",
            container_id
        );
        // In real implementation, this would configure Wine/Proton optimizations
        Ok(())
    }

    async fn enable_vrs(&self, container_id: &str) -> Result<()> {
        debug!(
            "Enabling Variable Rate Shading for container: {}",
            container_id
        );
        // In real implementation, this would enable VRS
        Ok(())
    }

    async fn configure_gpu_memory_pool(&self, container_id: &str, pool_size: &str) -> Result<()> {
        debug!(
            "Configuring GPU memory pool for container {}: {}",
            container_id, pool_size
        );
        // In real implementation, this would configure GPU memory pools
        Ok(())
    }

    async fn enable_mig(&self, container_id: &str) -> Result<()> {
        debug!("Enabling MIG for container: {}", container_id);
        // In real implementation, this would enable Multi-Instance GPU
        Ok(())
    }

    /// Get GPU metrics for container
    pub async fn get_gpu_metrics(&self, container_id: &str) -> Result<GpuMetrics> {
        let containers = self.containers.read().await;
        let container_info = containers.get(container_id);

        // Try AMD metrics first if available
        if let (Some(monitor), Some(info)) = (&self.amd_monitor, container_info)
            && let Some(device_node) = info.device_nodes.first()
            && let Some(card_idx) = device_node.path.strip_prefix("/dev/dri/card")
            && let Ok(index) = card_idx.parse::<u32>()
        {
            match monitor.get_metrics(index).await {
                Ok(amd_metrics) => {
                    info!(
                        "🔥 AMD GPU metrics for container {}: {}% util, {:.1}°C",
                        container_id, amd_metrics.gpu_utilization, amd_metrics.temperature_c
                    );
                    return Ok(GpuMetrics {
                        utilization: amd_metrics.gpu_utilization as f64,
                        memory_used: amd_metrics.memory_used_mb,
                        memory_total: amd_metrics.memory_total_mb,
                        temperature: amd_metrics.temperature_c as f64,
                        power_draw: amd_metrics.power_draw_watts as f64,
                    });
                }
                Err(e) => {
                    warn!("Failed to get AMD GPU metrics: {}", e);
                }
            }
        }

        // nvbind doesn't expose per-container metrics directly yet
        if container_info.is_some() {
            debug!("Using fallback GPU metrics for container: {}", container_id);
        }

        // Fallback metrics
        Ok(GpuMetrics {
            utilization: 0.0,
            memory_used: 0,
            memory_total: 0,
            temperature: 0.0,
            power_draw: 0.0,
        })
    }

    /// Get all AMD GPU metrics (not container-specific)
    pub async fn get_all_amd_gpu_metrics(&self) -> Result<Vec<super::amd_metrics::AmdGpuMetrics>> {
        if let Some(ref monitor) = self.amd_monitor {
            let results = monitor.get_all_metrics().await;
            let mut metrics = Vec::new();
            for result in results {
                match result {
                    Ok(metric) => metrics.push(metric),
                    Err(e) => warn!("Failed to get AMD GPU metric: {}", e),
                }
            }
            Ok(metrics)
        } else {
            Err(anyhow!("AMD GPU monitor not available").into())
        }
    }

    /// List available AMD GPUs
    pub fn list_amd_gpus(&self) -> Option<Vec<String>> {
        self.amd_monitor.as_ref().map(|monitor| {
            monitor
                .list_devices()
                .iter()
                .map(|device| {
                    format!(
                        "AMD GPU {}: {} ({}MB VRAM, PCI: {})",
                        device.index, device.name, device.vram_mb, device.pci_bus
                    )
                })
                .collect()
        })
    }

    /// Check if native NVIDIA GPU support is available
    pub fn is_nvbind_available(&self) -> bool {
        self.nvidia_manager.is_some()
    }

    /// Get reference to native NVIDIA manager
    pub fn nvidia_manager(&self) -> Option<&super::gpu::nvbind::NvbindManager> {
        self.nvidia_manager.as_ref()
    }

    /// Check if AMD GPU monitoring is available
    pub fn is_amd_monitor_available(&self) -> bool {
        self.amd_monitor.is_some()
    }
}

#[derive(Debug, Clone)]
struct AmdGpuBackend {
    devices: Vec<AmdGpuDevice>,
    rocm_root: Option<PathBuf>,
    rocm_smi: Option<PathBuf>,
    mesa_icd: Option<PathBuf>,
    kfd_available: bool,
    rocm_capable: bool,
}

#[derive(Debug, Clone)]
struct AmdGpuDevice {
    index: u32,
    card_path: PathBuf,
    render_path: Option<PathBuf>,
    pci_bus: Option<String>,
    memory_mb: Option<u64>,
    marketing_name: Option<String>,
}

impl AmdGpuBackend {
    async fn detect() -> Result<Option<Self>> {
        spawn_blocking(Self::discover_sync)
            .await
            .map_err(|err| anyhow!("AMD detection task failed: {}", err))?
    }

    fn discover_sync() -> Result<Option<Self>> {
        let drm_root = Path::new("/sys/class/drm");
        if !drm_root.exists() {
            return Ok(None);
        }

        let mut devices = Vec::new();
        let read_dir = match fs::read_dir(drm_root) {
            Ok(dir) => dir,
            Err(_) => return Ok(None),
        };

        for entry in read_dir.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("card") {
                continue;
            }

            let index = match name.trim_start_matches("card").parse::<u32>() {
                Ok(idx) => idx,
                Err(_) => continue,
            };

            let card_path = entry.path();
            let vendor_path = card_path.join("device/vendor");
            let vendor = match fs::read_to_string(&vendor_path) {
                Ok(v) => v,
                Err(_) => continue,
            };

            if vendor.trim() != "0x1002" {
                continue;
            }

            let card_node = PathBuf::from(format!("/dev/dri/{}", name));
            if !card_node.exists() {
                continue;
            }

            let render_candidate = PathBuf::from(format!("/dev/dri/renderD{}", 128 + index));
            let render_path = if render_candidate.exists() {
                Some(render_candidate)
            } else {
                None
            };

            let pci_bus = fs::read_to_string(card_path.join("device/uevent"))
                .ok()
                .and_then(|content| {
                    content
                        .lines()
                        .find_map(|line| line.strip_prefix("PCI_SLOT_NAME="))
                        .map(|s| s.trim().to_string())
                });

            let marketing_name = fs::read_to_string(card_path.join("device/product_name"))
                .ok()
                .map(|s| s.trim().to_string())
                .or_else(|| {
                    fs::read_to_string(card_path.join("device/device"))
                        .ok()
                        .map(|s| format!("gfx: {}", s.trim()))
                });

            let memory_mb = fs::read_to_string(card_path.join("device/mem_info_vram_total"))
                .ok()
                .and_then(|raw| raw.trim().parse::<u64>().ok())
                .map(|bytes| bytes / (1024 * 1024));

            devices.push(AmdGpuDevice {
                index,
                card_path: card_node,
                render_path,
                pci_bus,
                memory_mb,
                marketing_name,
            });
        }

        if devices.is_empty() {
            return Ok(None);
        }

        devices.sort_by_key(|device| device.index);

        let rocm_root = ["/opt/rocm", "/usr/lib/rocm", "/usr/local/rocm"]
            .iter()
            .map(Path::new)
            .find(|path| path.exists())
            .map(|path| path.to_path_buf());

        let rocm_smi = rocm_root
            .as_ref()
            .map(|root| root.join("bin/rocm-smi"))
            .filter(|path| path.exists());

        let mesa_icd_path = Path::new("/usr/share/vulkan/icd.d/amd_icd64.json");
        let mesa_icd = if mesa_icd_path.exists() {
            Some(mesa_icd_path.to_path_buf())
        } else {
            None
        };

        let kfd_available = Path::new("/dev/kfd").exists();
        let rocm_capable = rocm_root.is_some() && kfd_available;

        Ok(Some(Self {
            devices,
            rocm_root,
            rocm_smi,
            mesa_icd,
            kfd_available,
            rocm_capable,
        }))
    }

    fn is_available(&self) -> bool {
        !self.devices.is_empty()
    }

    fn device_count(&self) -> usize {
        self.devices.len()
    }

    fn device_summaries(&self) -> Vec<String> {
        self.devices.iter().map(|device| device.summary()).collect()
    }

    fn build_cdi_spec(&self) -> CdiSpec {
        let mut devices = self.device_nodes();
        devices.sort();
        devices.dedup();

        let mounts: Vec<String> = self
            .rocm_mounts()
            .into_iter()
            .map(|path| path.display().to_string())
            .collect();

        CdiSpec {
            devices: if devices.is_empty() {
                None
            } else {
                Some(devices)
            },
            mounts: if mounts.is_empty() {
                None
            } else {
                Some(mounts)
            },
            hooks: None,
        }
    }

    fn device_nodes(&self) -> Vec<String> {
        let mut nodes: Vec<String> = self
            .devices
            .iter()
            .flat_map(|device| {
                let mut paths = vec![device.card_path.display().to_string()];
                if let Some(render) = &device.render_path {
                    paths.push(render.display().to_string());
                }
                paths
            })
            .collect();

        if self.kfd_available {
            nodes.push("/dev/kfd".to_string());
        }

        nodes
    }

    fn render_nodes_csv(&self) -> Option<String> {
        let nodes: Vec<String> = self
            .devices
            .iter()
            .filter_map(|device| device.render_path.as_ref())
            .map(|path| path.display().to_string())
            .collect();
        if nodes.is_empty() {
            None
        } else {
            Some(nodes.join(","))
        }
    }

    fn pci_bus_list(&self) -> Option<String> {
        let buses: Vec<String> = self
            .devices
            .iter()
            .filter_map(|device| device.pci_bus.clone())
            .collect();
        if buses.is_empty() {
            None
        } else {
            Some(buses.join(","))
        }
    }

    fn visible_device_list(&self) -> String {
        (0..self.devices.len())
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    fn supports_rocm(&self) -> bool {
        self.rocm_capable
    }

    fn rocm_root_env(&self) -> Option<String> {
        self.rocm_root
            .as_ref()
            .map(|path| path.display().to_string())
    }

    fn rocm_ld_library_path(&self) -> Option<String> {
        let root = self.rocm_root.as_ref()?;
        let mut entries = Vec::new();
        let lib = root.join("lib");
        if lib.exists() {
            entries.push(lib.display().to_string());
        }
        let lib64 = root.join("lib64");
        if lib64.exists() {
            entries.push(lib64.display().to_string());
        }
        if entries.is_empty() {
            None
        } else {
            Some(entries.join(":"))
        }
    }

    fn rocm_smi_path(&self) -> Option<String> {
        self.rocm_smi
            .as_ref()
            .map(|path| path.display().to_string())
    }

    fn vulkan_icd_path(&self) -> Option<String> {
        self.mesa_icd
            .as_ref()
            .map(|path| path.display().to_string())
    }

    fn total_memory_mb(&self) -> Option<u64> {
        let total: u64 = self
            .devices
            .iter()
            .filter_map(|device| device.memory_mb)
            .sum();
        if total == 0 { None } else { Some(total) }
    }

    fn kfd_available(&self) -> bool {
        self.kfd_available
    }

    fn rocm_mounts(&self) -> Vec<PathBuf> {
        let mut mounts = Vec::new();
        if let Some(root) = &self.rocm_root {
            let lib = root.join("lib");
            if lib.exists() {
                mounts.push(lib);
            }
            let lib64 = root.join("lib64");
            if lib64.exists() {
                mounts.push(lib64);
            }
            let bin = root.join("bin");
            if bin.exists() {
                mounts.push(bin);
            }
        }
        mounts
    }
}

impl AmdGpuDevice {
    fn summary(&self) -> String {
        let name = self.marketing_name.as_deref().unwrap_or("AMD GPU");
        let pci = self.pci_bus.as_deref().unwrap_or("PCI:unknown");
        let memory = self
            .memory_mb
            .map(|mb| format!("{} MiB", mb))
            .unwrap_or_else(|| "memory n/a".to_string());
        let render = self
            .render_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "render node unavailable".to_string());

        format!(
            "{} (index {} @ {} | {} | {})",
            name, self.index, pci, memory, render
        )
    }
}

/// CDI specification structure (simplified)
#[derive(Debug, Clone)]
pub struct CdiSpec {
    pub devices: Option<Vec<String>>,
    pub mounts: Option<Vec<String>>,
    pub hooks: Option<Vec<String>>,
}

/// GPU metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuMetrics {
    pub utilization: f64,
    pub memory_used: u64,
    pub memory_total: u64,
    pub temperature: f64,
    pub power_draw: f64,
}

// Native NVIDIA GPU support is now always available via src/runtime/gpu/nvbind.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amd_backend_builds_cdi_spec_with_expected_devices() {
        let backend = AmdGpuBackend {
            devices: vec![AmdGpuDevice {
                index: 0,
                card_path: PathBuf::from("/dev/dri/card0"),
                render_path: Some(PathBuf::from("/dev/dri/renderD128")),
                pci_bus: Some("0000:0a:00.0".to_string()),
                memory_mb: Some(8192),
                marketing_name: Some("Radeon RX 6800".to_string()),
            }],
            rocm_root: Some(PathBuf::from("/opt/rocm")),
            rocm_smi: None,
            mesa_icd: Some(PathBuf::from("/usr/share/vulkan/icd.d/amd_icd64.json")),
            kfd_available: true,
            rocm_capable: true,
        };

        let spec = backend.build_cdi_spec();
        let devices = spec.devices.expect("device list");
        assert!(devices.contains(&"/dev/dri/card0".to_string()));
        assert!(devices.contains(&"/dev/dri/renderD128".to_string()));
        assert!(devices.contains(&"/dev/kfd".to_string()));

        assert_eq!(backend.visible_device_list(), "0");
        assert_eq!(backend.device_count(), 1);
        assert!(backend.supports_rocm());
    }

    #[test]
    fn amd_device_summary_includes_basic_metadata() {
        let device = AmdGpuDevice {
            index: 1,
            card_path: PathBuf::from("/dev/dri/card1"),
            render_path: None,
            pci_bus: Some("0000:0b:00.0".to_string()),
            memory_mb: Some(4096),
            marketing_name: Some("Radeon Pro V620".to_string()),
        };

        let summary = device.summary();
        assert!(summary.contains("Radeon Pro V620"));
        assert!(summary.contains("index 1"));
        assert!(summary.contains("0000:0b:00.0"));
        assert!(summary.contains("4096 MiB"));
    }

    #[test]
    fn applied_cdi_spec_dedup_env_prefers_last_assignment() {
        let mut spec = AppliedCdiSpec {
            env: vec![
                "FOO=1".to_string(),
                "BAR=2".to_string(),
                "FOO=3".to_string(),
                "FLAG".to_string(),
                "FLAG".to_string(),
            ],
            device_nodes: Vec::new(),
            mounts: Vec::new(),
            hooks: Vec::new(),
        };

        spec.dedup();

        assert_eq!(
            spec.env,
            vec!["BAR=2".to_string(), "FOO=3".to_string(), "FLAG".to_string(),]
        );
    }

    #[test]
    fn applied_cdi_spec_dedup_mounts_collapses_duplicate_definitions() {
        let mut spec = AppliedCdiSpec {
            env: Vec::new(),
            device_nodes: Vec::new(),
            mounts: vec![
                CdiMount::new(
                    "/opt/driver",
                    "/opt/driver",
                    vec!["bind".to_string(), "ro".to_string()],
                ),
                CdiMount::new(
                    "/opt/driver",
                    "/opt/driver",
                    vec!["ro".to_string(), "bind".to_string()],
                ),
                CdiMount::new(
                    "/var/cache",
                    "/var/cache",
                    vec!["bind".to_string(), "rw".to_string()],
                ),
            ],
            hooks: Vec::new(),
        };

        spec.dedup();

        assert_eq!(spec.mounts.len(), 2);
        assert!(
            spec.mounts
                .iter()
                .any(|mount| mount.host_path == "/opt/driver"
                    && mount.container_path == "/opt/driver")
        );
        assert!(
            spec.mounts.iter().any(
                |mount| mount.host_path == "/var/cache" && mount.container_path == "/var/cache"
            )
        );
    }

    #[test]
    fn applied_cdi_spec_dedup_hooks_and_devices_keeps_unique_entries() {
        let duplicate_hook = CdiHook::new(
            "prestart",
            "/usr/bin/nv-init",
            vec!["--verbose".to_string()],
            vec!["FOO=bar".to_string()],
            Some(5),
        );

        let mut spec = AppliedCdiSpec {
            env: Vec::new(),
            mounts: Vec::new(),
            hooks: vec![duplicate_hook.clone(), duplicate_hook.clone()],
            device_nodes: vec![
                {
                    let mut node = CdiDeviceNode::new("/dev/nvidia0");
                    node.device_type = Some("c".to_string());
                    node.major = Some(195);
                    node.minor = Some(0);
                    node
                },
                {
                    let mut node = CdiDeviceNode::new("/dev/nvidia0");
                    node.device_type = Some("c".to_string());
                    node.major = Some(195);
                    node.minor = Some(0);
                    node
                },
                CdiDeviceNode::new("/dev/nvidiactl"),
            ],
        };

        spec.dedup();

        assert_eq!(spec.hooks.len(), 1);
        assert_eq!(spec.device_nodes.len(), 2);
        assert!(
            spec.device_nodes
                .iter()
                .any(|node| node.path == "/dev/nvidiactl")
        );
    }

    #[tokio::test]
    async fn gpu_environment_honors_requested_visible_devices() {
        let integration = BoltGpuIntegration {
            nvidia_manager: None,
            containers: Arc::new(RwLock::new(HashMap::new())),
            fallback_mode: true,
            amd_backend: None,
            amd_monitor: None,
        };
        let container_id = "gpu-visible-test";
        crate::runtime::environment::env_manager()
            .clear_container_env(container_id)
            .expect("clear test env");

        let config = GpuConfig {
            enabled: true,
            devices: Some("0,1".to_string()),
            workload_type: GpuWorkloadType::General,
            isolation_level: GpuIsolationLevel::Shared,
            memory_limit: None,
            snapshot_support: false,
            quick_sync: None,
        };
        let applied = AppliedCdiSpec {
            env: Vec::new(),
            device_nodes: vec![CdiDeviceNode::new("/dev/nvidia0")],
            mounts: Vec::new(),
            hooks: Vec::new(),
        };

        integration
            .setup_gpu_environment(container_id, &config, &applied)
            .await
            .expect("gpu env setup");

        let env = crate::runtime::environment::env_manager()
            .get_container_env(container_id)
            .expect("read test env");
        assert_eq!(env.get("BOLT_GPU_DEVICES").map(String::as_str), Some("0,1"));
        assert_eq!(
            env.get("NVIDIA_VISIBLE_DEVICES").map(String::as_str),
            Some("0,1")
        );
        crate::runtime::environment::env_manager()
            .clear_container_env(container_id)
            .expect("clear test env");
    }
}
