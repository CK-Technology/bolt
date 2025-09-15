use super::{GPUInfo, GPUManager};
use anyhow::Result;
use nix::unistd;
use std::path::Path;
use tracing::{debug, info, warn};

/// Velocity: Bolt's native GPU runtime with support for:
/// - NVIDIA Open GPU Kernel Modules (primary)
/// - nvidia-container-runtime integration
/// - Proprietary and nouveau drivers
impl GPUManager {
    /// Check if nvidia-container-runtime is available on the system
    pub async fn has_nvidia_container_runtime(&self) -> bool {
        let runtime_paths = [
            "/usr/bin/nvidia-container-runtime",
            "/usr/bin/nvidia-container-toolkit",
            "/usr/bin/nvidia-docker",
        ];

        for path in &runtime_paths {
            if Path::new(path).exists() {
                debug!("Found nvidia-container-runtime at: {}", path);
                return true;
            }
        }

        false
    }

    /// Enhanced GPU setup with support for both nvidia-container-runtime and bolt native
    pub async fn setup_gpu_with_runtime_preference(
        &self,
        container_id: &str,
        gpu_config: &crate::config::GpuConfig,
        prefer_nvidia_runtime: bool,
    ) -> Result<()> {
        info!(
            "⚡ Setting up GPU access for container: {} (prefer nvidia-runtime: {})",
            container_id, prefer_nvidia_runtime
        );

        let nvidia_runtime_available = self.has_nvidia_container_runtime().await;
        let is_rootless = unistd::getuid().as_raw() != 0;

        // Handle NVIDIA GPU configuration
        if let Some(ref nvidia_config) = gpu_config.nvidia {
            if let Some(ref nvidia_manager) = self.nvidia {
                if nvidia_runtime_available && prefer_nvidia_runtime && !is_rootless {
                    info!("  🐳 Using nvidia-container-runtime integration");
                    self.configure_nvidia_runtime_integration(container_id, nvidia_config)
                        .await?;
                } else {
                    info!("  ⚡ Using bolt native GPU runtime");
                    nvidia_manager
                        .setup_container_access(container_id, nvidia_config)
                        .await?;

                    // Also setup open-source driver support if detected
                    if let Ok(_) = nvidia_manager
                        .setup_open_source_gpu_access(
                            container_id,
                            &[nvidia_config.device.unwrap_or(0)],
                        )
                        .await
                    {
                        info!("    🌍 Open-source driver support also configured");
                    }
                }
            } else {
                warn!("⚠️ NVIDIA GPU requested but not available");
            }
        }

        // Handle AMD GPU configuration (always uses bolt native)
        if let Some(ref amd_config) = gpu_config.amd {
            if let Some(ref amd_manager) = self.amd {
                info!("  🔥 Using bolt native AMD support");
                amd_manager
                    .setup_container_access(container_id, amd_config)
                    .await?;
            } else {
                warn!("⚠️ AMD GPU requested but not available");
            }
        }

        // Handle GPU passthrough
        if gpu_config.passthrough.unwrap_or(false) {
            info!("  🚀 GPU passthrough mode enabled");
            self.setup_gpu_passthrough(container_id, gpu_config).await?;
        }

        Ok(())
    }

    /// Configure nvidia-container-runtime integration
    async fn configure_nvidia_runtime_integration(
        &self,
        container_id: &str,
        nvidia_config: &crate::config::NvidiaConfig,
    ) -> Result<()> {
        info!(
            "    🔧 Configuring nvidia-container-runtime integration for {}",
            container_id
        );

        let device = nvidia_config.device.unwrap_or(0);

        // Set up environment variables that nvidia-container-runtime uses
        unsafe {
            std::env::set_var("NVIDIA_VISIBLE_DEVICES", device.to_string());

            let mut capabilities = vec!["utility"];

            if nvidia_config.cuda.unwrap_or(false) {
                capabilities.push("compute");
                info!("      🚀 CUDA compute enabled");
            }

            // Always include graphics capabilities for gaming
            capabilities.extend(&["graphics", "video", "display"]);

            std::env::set_var("NVIDIA_DRIVER_CAPABILITIES", capabilities.join(","));

            // Advanced features
            if nvidia_config.dlss.unwrap_or(false) {
                std::env::set_var("NVIDIA_ENABLE_DLSS", "1");
                info!("      ✨ DLSS enabled");
            }

            if nvidia_config.raytracing.unwrap_or(false) {
                std::env::set_var("NVIDIA_ENABLE_RTX", "1");
                info!("      🌟 Ray tracing enabled");
            }
        }

        info!("    ✅ nvidia-container-runtime configuration complete");
        Ok(())
    }

    /// Setup GPU passthrough for maximum performance
    async fn setup_gpu_passthrough(
        &self,
        container_id: &str,
        gpu_config: &crate::config::GpuConfig,
    ) -> Result<()> {
        info!(
            "  🚀 Setting up GPU passthrough for container: {}",
            container_id
        );

        // Passthrough requires direct device access
        let mut device_paths = Vec::new();

        if let Some(ref nvidia_config) = gpu_config.nvidia {
            let device_idx = nvidia_config.device.unwrap_or(0);

            // NVIDIA proprietary devices
            device_paths.extend(vec![
                "/dev/nvidiactl".to_string(),
                "/dev/nvidia-uvm".to_string(),
                "/dev/nvidia-uvm-tools".to_string(),
                format!("/dev/nvidia{}", device_idx),
            ]);

            // DRI devices for Vulkan/OpenGL
            device_paths.extend(vec![
                format!("/dev/dri/card{}", device_idx),
                format!("/dev/dri/renderD{}", 128 + device_idx),
            ]);
        }

        if let Some(ref amd_config) = gpu_config.amd {
            let device_idx = amd_config.device.unwrap_or(0);

            // AMD DRI devices
            device_paths.extend(vec![
                format!("/dev/dri/card{}", device_idx),
                format!("/dev/dri/renderD{}", 128 + device_idx),
            ]);
        }

        // Verify and log available devices
        let mut available_devices = 0;
        for device in &device_paths {
            if Path::new(device).exists() {
                available_devices += 1;
                debug!("      📱 Available for passthrough: {}", device);
            } else {
                debug!("      ❌ Not available: {}", device);
            }
        }

        info!(
            "      📊 GPU passthrough: {}/{} devices available",
            available_devices,
            device_paths.len()
        );

        Ok(())
    }

    /// Check for rootless container support
    pub async fn check_rootless_gpu_support(&self) -> Result<RootlessGpuSupport> {
        info!("🔍 Checking rootless GPU support");

        let is_rootless = unistd::getuid().as_raw() != 0;
        let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());

        let mut support = RootlessGpuSupport {
            is_rootless,
            user: user.clone(),
            dri_access: false,
            nvidia_access: false,
            suggestions: Vec::new(),
        };

        if !is_rootless {
            info!("  ✅ Running as root, full GPU access available");
            support.dri_access = true;
            support.nvidia_access = true;
            return Ok(support);
        }

        info!("  🧑 Running as user '{}', checking device access", user);

        // Check DRI device access
        let dri_devices = ["/dev/dri/card0", "/dev/dri/renderD128"];
        for device in &dri_devices {
            if Path::new(device).exists() {
                if let Ok(_) = std::fs::File::open(device) {
                    support.dri_access = true;
                    debug!("    ✅ Can access: {}", device);
                    break;
                }
            }
        }

        // Check NVIDIA device access
        let nvidia_devices = ["/dev/nvidiactl", "/dev/nvidia0"];
        for device in &nvidia_devices {
            if Path::new(device).exists() {
                if let Ok(_) = std::fs::File::open(device) {
                    support.nvidia_access = true;
                    debug!("    ✅ Can access: {}", device);
                    break;
                }
            }
        }

        // Generate suggestions if access is limited
        if !support.dri_access {
            support
                .suggestions
                .push("Add user to 'render' group: sudo usermod -a -G render $USER".to_string());
            support
                .suggestions
                .push("Add user to 'video' group: sudo usermod -a -G video $USER".to_string());
        }

        if !support.nvidia_access {
            support
                .suggestions
                .push("Create udev rule for NVIDIA device access".to_string());
            support
                .suggestions
                .push("Consider using --privileged if security allows".to_string());
        }

        info!(
            "  📊 Rootless support - DRI: {}, NVIDIA: {}",
            support.dri_access, support.nvidia_access
        );

        Ok(support)
    }
}

#[derive(Debug, Clone)]
pub struct RootlessGpuSupport {
    pub is_rootless: bool,
    pub user: String,
    pub dri_access: bool,
    pub nvidia_access: bool,
    pub suggestions: Vec<String>,
}

impl RootlessGpuSupport {
    pub fn print_suggestions(&self) {
        if !self.suggestions.is_empty() {
            warn!("💡 Rootless GPU Setup Suggestions:");
            for suggestion in &self.suggestions {
                warn!("  • {}", suggestion);
            }
        }
    }
}
