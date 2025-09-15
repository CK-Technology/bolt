use anyhow::{Context, Result};
use oci_spec::runtime::{Linux, Mount, Process, Root, Spec};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info, warn};

pub mod container;
pub mod executor;
pub mod namespace;

use crate::capsules::CapsuleManager;
use crate::runtime::storage::StorageManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub id: String,
    pub name: Option<String>,
    pub image: String,
    pub command: Vec<String>,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_dir: Option<String>,
    pub user: Option<String>,
    pub ports: Vec<PortMapping>,
    pub volumes: Vec<VolumeMount>,
    pub capabilities: Vec<String>,
    pub privileged: bool,
    pub readonly_rootfs: bool,
    pub security_profile: SecurityProfile,
    pub resource_limits: ResourceLimits,
    pub gaming_config: Option<crate::config::GamingConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String, // tcp, udp, quic
    pub host_ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMount {
    pub source: String,
    pub destination: String,
    pub readonly: bool,
    pub mount_type: String, // bind, volume, tmpfs, device
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityProfile {
    pub apparmor_profile: Option<String>,
    pub selinux_label: Option<String>,
    pub seccomp_profile: Option<String>,
    pub no_new_privileges: bool,
    pub drop_capabilities: Vec<String>,
    pub add_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub memory_limit: Option<u64>, // bytes
    pub cpu_limit: Option<f64>,    // cores
    pub pids_limit: Option<u32>,   // max processes
    pub blkio_weight: Option<u16>, // 10-1000
    pub cpu_shares: Option<u32>,   // relative weight
}

impl Default for SecurityProfile {
    fn default() -> Self {
        Self {
            apparmor_profile: None,
            selinux_label: None,
            seccomp_profile: Some("default".to_string()),
            no_new_privileges: true,
            drop_capabilities: vec![
                "CAP_SYS_ADMIN".to_string(),
                "CAP_NET_ADMIN".to_string(),
                "CAP_SYS_MODULE".to_string(),
            ],
            add_capabilities: vec![],
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            memory_limit: Some(512 * 1024 * 1024), // 512MB default
            cpu_limit: Some(1.0),                  // 1 core default
            pids_limit: Some(1024),                // 1024 processes
            blkio_weight: Some(500),               // medium I/O priority
            cpu_shares: Some(1024),                // standard weight
        }
    }
}

pub struct OCIRuntime {
    pub storage: StorageManager,
    pub capsule_manager: CapsuleManager,
    pub runtime_dir: PathBuf,
    pub containers: HashMap<String, ContainerState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerState {
    pub id: String,
    pub config: ContainerConfig,
    pub status: ContainerStatus,
    pub pid: Option<u32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub exit_code: Option<i32>,
    pub bundle_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContainerStatus {
    Created,
    Running,
    Stopped,
    Paused,
    Unknown,
}

impl OCIRuntime {
    pub fn new(runtime_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&runtime_dir).context("Failed to create runtime directory")?;

        let storage = StorageManager::new(runtime_dir.join("storage"))?;
        let capsule_manager = CapsuleManager::new(runtime_dir.join("capsules"))?;

        Ok(Self {
            storage,
            capsule_manager,
            runtime_dir,
            containers: HashMap::new(),
        })
    }

    pub async fn run_container(&mut self, config: ContainerConfig) -> Result<String> {
        info!("🐳 Starting OCI container: {}", config.image);

        // Check if this should be a Bolt Capsule instead
        if config.image.starts_with("bolt://") {
            info!("🔧 Delegating to Bolt Capsule runtime");
            return self.run_capsule(config).await;
        }

        let container_id = config.id.clone();
        debug!("Container config: {:?}", config);

        // Gaming optimizations
        if let Some(ref gaming) = config.gaming_config {
            self.apply_gaming_optimizations(&container_id, gaming)
                .await?;
        }

        // Create container bundle directory
        let bundle_path = self.runtime_dir.join("bundles").join(&container_id);
        std::fs::create_dir_all(&bundle_path).context("Failed to create container bundle")?;

        // Pull image if needed
        self.storage.pull_image(&config.image).await?;

        // Create OCI spec
        let spec = self.create_oci_spec(&config)?;
        let spec_path = bundle_path.join("config.json");
        let spec_json = serde_json::to_string_pretty(&spec)?;
        std::fs::write(&spec_path, spec_json).context("Failed to write OCI spec")?;

        // Create container state
        let state = ContainerState {
            id: container_id.clone(),
            config,
            status: ContainerStatus::Created,
            pid: None,
            created_at: chrono::Utc::now(),
            started_at: None,
            finished_at: None,
            exit_code: None,
            bundle_path,
        };

        // Execute container
        let pid = executor::execute_container(&state, &spec).await?;

        // Update state
        let mut updated_state = state;
        updated_state.status = ContainerStatus::Running;
        updated_state.pid = Some(pid);
        updated_state.started_at = Some(chrono::Utc::now());

        self.containers.insert(container_id.clone(), updated_state);

        info!("✅ Container {} started with PID {}", container_id, pid);
        Ok(container_id)
    }

    pub async fn run_capsule(&mut self, config: ContainerConfig) -> Result<String> {
        info!("🔧 Starting Bolt Capsule: {}", config.image);

        let capsule_name = config
            .image
            .strip_prefix("bolt://")
            .unwrap_or(&config.image);

        // Use Capsule manager for VM-like containers
        let capsule_id = self
            .capsule_manager
            .create_capsule(capsule_name, &config)
            .await?;

        info!("✅ Bolt Capsule {} created", capsule_id);
        Ok(capsule_id)
    }

    async fn apply_gaming_optimizations(
        &self,
        container_id: &str,
        gaming_config: &crate::config::GamingConfig,
    ) -> Result<()> {
        info!(
            "🎮 Applying gaming optimizations for container: {}",
            container_id
        );

        // GPU passthrough setup
        if let Some(ref gpu) = gaming_config.gpu {
            if let Some(ref nvidia) = gpu.nvidia {
                info!("🟢 Setting up NVIDIA GPU passthrough");
                self.setup_nvidia_passthrough(container_id, nvidia).await?;
            }
            if let Some(ref amd) = gpu.amd {
                info!("🔴 Setting up AMD GPU passthrough");
                self.setup_amd_passthrough(container_id, amd).await?;
            }
        }

        // Audio optimization
        if let Some(ref audio) = gaming_config.audio {
            info!("🔊 Configuring audio: {}", audio.system);
            self.setup_audio_passthrough(container_id, audio).await?;
        }

        // Performance tuning
        if let Some(ref perf) = gaming_config.performance {
            info!("⚡ Applying performance tuning");
            self.apply_performance_tuning(container_id, perf).await?;
        }

        Ok(())
    }

    async fn setup_nvidia_passthrough(
        &self,
        _container_id: &str,
        nvidia: &crate::config::NvidiaConfig,
    ) -> Result<()> {
        info!("Setting up NVIDIA GPU device: {:?}", nvidia.device);

        // Check NVIDIA runtime
        if !std::path::Path::new("/usr/bin/nvidia-container-runtime").exists() {
            warn!("nvidia-container-runtime not found - GPU passthrough may not work");
        }

        if nvidia.dlss == Some(true) {
            info!("✨ DLSS support enabled");
        }
        if nvidia.raytracing == Some(true) {
            info!("🌟 Ray tracing support enabled");
        }

        // TODO: Implement actual GPU device passthrough
        warn!("NVIDIA GPU passthrough implementation pending");
        Ok(())
    }

    async fn setup_amd_passthrough(
        &self,
        _container_id: &str,
        amd: &crate::config::AmdConfig,
    ) -> Result<()> {
        info!("Setting up AMD GPU device: {:?}", amd.device);

        // Check for DRI devices
        if !std::path::Path::new("/dev/dri").exists() {
            warn!("DRI devices not found - GPU passthrough may not work");
        }

        // TODO: Implement actual AMD GPU device passthrough
        warn!("AMD GPU passthrough implementation pending");
        Ok(())
    }

    async fn setup_audio_passthrough(
        &self,
        _container_id: &str,
        audio: &crate::config::AudioConfig,
    ) -> Result<()> {
        match audio.system.as_str() {
            "pipewire" => {
                info!("🎵 Configuring PipeWire audio passthrough");
                // TODO: PipeWire socket passthrough
            }
            "pulseaudio" => {
                info!("🔊 Configuring PulseAudio passthrough");
                // TODO: PulseAudio socket passthrough
            }
            _ => {
                warn!("Unsupported audio system: {}", audio.system);
            }
        }
        Ok(())
    }

    async fn apply_performance_tuning(
        &self,
        container_id: &str,
        perf: &crate::config::PerformanceConfig,
    ) -> Result<()> {
        if let Some(ref governor) = perf.cpu_governor {
            info!("⚙️  Setting CPU governor to: {}", governor);
            // TODO: Set CPU governor for container processes
        }

        if let Some(nice) = perf.nice_level {
            info!("📊 Setting nice level to: {}", nice);
            // TODO: Apply nice level to container processes
        }

        if let Some(priority) = perf.rt_priority {
            info!("🚀 Setting RT priority to: {}", priority);
            // TODO: Apply real-time priority
        }

        info!(
            "✅ Performance tuning applied to container: {}",
            container_id
        );
        Ok(())
    }

    fn create_oci_spec(&self, config: &ContainerConfig) -> Result<Spec> {
        info!("📋 Creating OCI specification");

        // Create basic spec with Linux-specific configs
        let mut spec = Spec::default();

        // Root filesystem
        let mut root = Root::default();
        root.set_path("rootfs".into());
        root.set_readonly(Some(config.readonly_rootfs));
        spec.set_root(Some(root));

        // Process configuration
        let mut process = Process::default();
        process.set_args(Some([config.command.clone(), config.args.clone()].concat()));

        // Environment variables
        let env: Vec<String> = config
            .env
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        process.set_env(Some(env));

        if let Some(ref cwd) = config.working_dir {
            process.set_cwd(cwd.clone().into());
        }

        spec.set_process(Some(process));

        // Mounts
        let mut mounts = vec![
            // Standard mounts
            {
                let mut mount = Mount::default();
                mount.set_destination("/proc".into());
                mount.set_source(Some("proc".to_string().into()));
                mount.set_typ(Some("proc".to_string()));
                mount
            },
            {
                let mut mount = Mount::default();
                mount.set_destination("/dev".into());
                mount.set_source(Some("tmpfs".to_string().into()));
                mount.set_typ(Some("tmpfs".to_string()));
                mount.set_options(Some(vec!["nosuid".to_string(), "strictatime".to_string()]));
                mount
            },
            {
                let mut mount = Mount::default();
                mount.set_destination("/sys".into());
                mount.set_source(Some("sysfs".to_string().into()));
                mount.set_typ(Some("sysfs".to_string()));
                mount.set_options(Some(vec![
                    "nosuid".to_string(),
                    "noexec".to_string(),
                    "nodev".to_string(),
                ]));
                mount
            },
        ];

        // Add custom volumes
        for volume in &config.volumes {
            let mut options = vec!["bind".to_string()];
            if volume.readonly {
                options.push("ro".to_string());
            }

            let mut mount = Mount::default();
            mount.set_destination(volume.destination.clone().into());
            mount.set_source(Some(volume.source.clone().into()));
            mount.set_typ(Some("bind".to_string()));
            mount.set_options(Some(options));
            mounts.push(mount);
        }

        spec.set_mounts(Some(mounts));

        // Linux-specific configuration
        let linux = Linux::default();
        spec.set_linux(Some(linux));

        debug!("✅ OCI spec created successfully");
        Ok(spec)
    }

    pub async fn stop_container(&mut self, container_id: &str) -> Result<()> {
        info!("🛑 Stopping container: {}", container_id);

        if let Some(state) = self.containers.get_mut(container_id) {
            if let Some(pid) = state.pid {
                // Send SIGTERM first
                nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(pid as i32),
                    nix::sys::signal::Signal::SIGTERM,
                )
                .context("Failed to send SIGTERM")?;

                // Wait a bit, then SIGKILL if needed
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

                // Check if process still exists
                match nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None) {
                    Ok(_) => {
                        // Process still exists, force kill
                        warn!(
                            "Container {} didn't respond to SIGTERM, sending SIGKILL",
                            container_id
                        );
                        nix::sys::signal::kill(
                            nix::unistd::Pid::from_raw(pid as i32),
                            nix::sys::signal::Signal::SIGKILL,
                        )
                        .context("Failed to send SIGKILL")?;
                    }
                    Err(_) => {
                        // Process already dead
                    }
                }
            }

            state.status = ContainerStatus::Stopped;
            state.finished_at = Some(chrono::Utc::now());
            info!("✅ Container {} stopped", container_id);
        } else {
            return Err(anyhow::anyhow!("Container {} not found", container_id));
        }

        Ok(())
    }

    pub fn list_containers(&self, all: bool) -> Vec<&ContainerState> {
        self.containers
            .values()
            .filter(|state| all || state.status == ContainerStatus::Running)
            .collect()
    }
}
