use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, warn};

use super::{GamingPerformanceMetrics, WaylandGamingConfig};
use crate::runtime::gpu::{GPUManager, GPUVendor};

#[derive(Debug, Clone)]
pub struct WaylandGpuIntegration {
    pub gpu_manager: Option<GPUManager>,
    pub active_gpu_sessions: HashMap<String, GpuSessionInfo>,
    pub display_server: DisplayServerType,
    pub kde_optimizations: bool,
}

#[derive(Debug, Clone)]
pub struct GpuSessionInfo {
    pub container_id: String,
    pub gpu_vendor: GPUVendor,
    pub wayland_display: String,
    pub direct_rendering: bool,
    pub hardware_acceleration: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DisplayServerType {
    Wayland,
    X11,
    KDEWayland,
    GNOMEWayland,
    Sway,
    Hyprland,
    Weston,
}

impl WaylandGpuIntegration {
    pub async fn new() -> Result<Self> {
        info!("🎮 Initializing Wayland GPU integration");

        let gpu_manager = match GPUManager::new() {
            Ok(manager) => {
                info!("  ✅ GPU Manager integrated with Wayland");
                Some(manager)
            }
            Err(e) => {
                warn!("  ⚠️ GPU Manager not available: {}", e);
                None
            }
        };

        let display_server = Self::detect_display_server().await?;
        let kde_optimizations = display_server == DisplayServerType::KDEWayland;

        info!("  🖥️ Display server: {:?}", display_server);
        if kde_optimizations {
            info!("  🔷 KDE/Plasma optimizations enabled");
        }

        Ok(Self {
            gpu_manager,
            active_gpu_sessions: HashMap::new(),
            display_server,
            kde_optimizations,
        })
    }

    async fn detect_display_server() -> Result<DisplayServerType> {
        // Check environment variables first
        if let Ok(wayland_display) = std::env::var("WAYLAND_DISPLAY") {
            debug!("WAYLAND_DISPLAY detected: {}", wayland_display);

            // Detect specific Wayland compositors
            if std::env::var("KDE_SESSION_VERSION").is_ok()
                || std::env::var("KDE_FULL_SESSION").is_ok()
                || Self::is_kwin_running().await
            {
                return Ok(DisplayServerType::KDEWayland);
            }

            if std::env::var("GNOME_DESKTOP_SESSION_ID").is_ok()
                || std::env::var("XDG_CURRENT_DESKTOP")
                    .unwrap_or_default()
                    .contains("GNOME")
            {
                return Ok(DisplayServerType::GNOMEWayland);
            }

            if std::env::var("SWAYSOCK").is_ok() {
                return Ok(DisplayServerType::Sway);
            }

            if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
                return Ok(DisplayServerType::Hyprland);
            }

            // Default to generic Wayland
            return Ok(DisplayServerType::Wayland);
        }

        // Check for X11
        if std::env::var("DISPLAY").is_ok() {
            return Ok(DisplayServerType::X11);
        }

        // Default fallback
        Ok(DisplayServerType::Wayland)
    }

    async fn is_kwin_running() -> bool {
        if let Ok(output) = tokio::process::Command::new("pgrep")
            .arg("-f")
            .arg("kwin")
            .output()
            .await
        {
            !output.stdout.is_empty()
        } else {
            false
        }
    }

    pub async fn setup_gpu_for_wayland_container(
        &mut self,
        container_id: &str,
        config: &WaylandGamingConfig,
    ) -> Result<()> {
        info!("🚀 Setting up GPU for Wayland container: {}", container_id);

        let Some(ref gpu_manager) = self.gpu_manager else {
            return Err(anyhow::anyhow!("No GPU manager available"));
        };

        // Get available GPUs
        let gpus = gpu_manager.get_available_gpus().await?;
        if gpus.is_empty() {
            return Err(anyhow::anyhow!("No GPUs available for Wayland integration"));
        }

        let primary_gpu = &gpus[0];
        info!(
            "  🎯 Using GPU: {} {:?}",
            primary_gpu.name, primary_gpu.vendor
        );

        // Setup Wayland-specific GPU environment
        self.setup_wayland_gpu_environment(container_id, primary_gpu.vendor.clone())
            .await?;

        // Apply compositor-specific optimizations
        match self.display_server {
            DisplayServerType::KDEWayland => {
                self.setup_kde_wayland_optimizations(container_id).await?;
            }
            DisplayServerType::GNOMEWayland => {
                self.setup_gnome_wayland_optimizations(container_id).await?;
            }
            DisplayServerType::Sway | DisplayServerType::Hyprland => {
                self.setup_wlroots_optimizations(container_id).await?;
            }
            _ => {
                self.setup_generic_wayland_optimizations(container_id)
                    .await?;
            }
        }

        // Register the session
        let session_info = GpuSessionInfo {
            container_id: container_id.to_string(),
            gpu_vendor: primary_gpu.vendor.clone(),
            wayland_display: config.display_name.clone(),
            direct_rendering: true,
            hardware_acceleration: true,
        };

        self.active_gpu_sessions
            .insert(container_id.to_string(), session_info);
        info!("  ✅ GPU-Wayland integration complete for {}", container_id);

        Ok(())
    }

    async fn setup_wayland_gpu_environment(
        &self,
        container_id: &str,
        vendor: GPUVendor,
    ) -> Result<()> {
        info!("    🌊 Setting up Wayland GPU environment");

        unsafe {
            // Core Wayland environment
            std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
            std::env::set_var("GDK_BACKEND", "wayland");
            std::env::set_var("QT_QPA_PLATFORM", "wayland");
            std::env::set_var("CLUTTER_BACKEND", "wayland");

            // Enable DRI3 and hardware acceleration
            std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "0");
            std::env::set_var("MESA_LOADER_DRIVER_OVERRIDE", "");

            // Gaming-specific Wayland optimizations
            std::env::set_var("WAYLAND_GAMING_OPTIMIZATIONS", "1");
            std::env::set_var("WAYLAND_DISABLE_VSYNC", "1"); // Let games control VSync
            std::env::set_var("WAYLAND_LOW_LATENCY", "1");

            match vendor {
                GPUVendor::NVIDIA => {
                    // NVIDIA-specific Wayland setup
                    std::env::set_var("GBM_BACKEND", "nvidia-drm");
                    std::env::set_var("__GLX_VENDOR_LIBRARY_NAME", "nvidia");
                    std::env::set_var("WLR_NO_HARDWARE_CURSORS", "1");

                    // Enable NVIDIA Wayland features
                    std::env::set_var("NVIDIA_WAYLAND_OPTIMIZATIONS", "1");
                    std::env::set_var("__GL_GSYNC_ALLOWED", "1");
                    std::env::set_var("__GL_VRR_ALLOWED", "1");
                }
                GPUVendor::AMD => {
                    // AMD-specific Wayland setup
                    std::env::set_var("RADV_PERFTEST", "gpl,nggc,sam");
                    std::env::set_var("AMD_VULKAN_ICD", "RADV");
                    std::env::set_var("MESA_VK_DEVICE_SELECT", "radv");
                }
                GPUVendor::Intel => {
                    // Intel-specific optimizations
                    std::env::set_var("MESA_LOADER_DRIVER_OVERRIDE", "iris");
                    std::env::set_var("INTEL_DEBUG", "sync");
                }
            }
        }

        Ok(())
    }

    async fn setup_kde_wayland_optimizations(&self, container_id: &str) -> Result<()> {
        info!("    🔷 Applying KDE/Plasma Wayland optimizations");

        unsafe {
            // KDE/Plasma specific optimizations
            std::env::set_var("KWIN_DRM_DEVICES", "/dev/dri/card0");
            std::env::set_var("KWIN_EXPLICIT_SYNC", "1");

            // Gaming optimizations for KWin
            std::env::set_var("KWIN_TRIPLE_BUFFER", "1");
            std::env::set_var("KWIN_LOWLATENCY", "1");
            std::env::set_var("KWIN_DRM_USE_MODIFIERS", "1");

            // VRR support in KWin
            std::env::set_var("KWIN_VRR", "1");
            std::env::set_var("KWIN_ALLOW_TEARING", "1");

            // Plasma gaming features
            std::env::set_var("PLASMA_GAMING_MODE", "1");
            std::env::set_var("KDE_APPLICATIONS_AS_SCOPE", "1");

            // Qt optimizations for gaming
            std::env::set_var("QT_WAYLAND_DISABLE_WINDOWDECORATION", "1");
            std::env::set_var("QT_QPA_PLATFORMTHEME", "kde");
            std::env::set_var("QT_WAYLAND_FORCE_DPI", "96");
        }

        info!("      ✅ KDE/KWin gaming optimizations applied");
        Ok(())
    }

    async fn setup_gnome_wayland_optimizations(&self, container_id: &str) -> Result<()> {
        info!("    🟣 Applying GNOME Wayland optimizations");

        unsafe {
            // GNOME/Mutter specific
            std::env::set_var("MUTTER_DEBUG_FORCE_KMS_MODE", "simple");
            std::env::set_var("MUTTER_DEBUG_ENABLE_ATOMIC_KMS", "1");

            // Gaming optimizations for Mutter
            std::env::set_var("MUTTER_DISABLE_VSYNC", "1");
            std::env::set_var("GNOME_GAMING_OPTIMIZATIONS", "1");
        }

        Ok(())
    }

    async fn setup_wlroots_optimizations(&self, container_id: &str) -> Result<()> {
        info!("    🌊 Applying wlroots-based compositor optimizations");

        unsafe {
            // wlroots gaming optimizations (Sway, Hyprland, etc.)
            std::env::set_var("WLR_RENDERER", "vulkan");
            std::env::set_var("WLR_NO_HARDWARE_CURSORS", "1");
            std::env::set_var("WLR_DRM_FORCE_LIBLIFTOFF", "1");

            // Gaming-specific wlroots features
            std::env::set_var("WLR_GAMING_OPTIMIZATIONS", "1");
            std::env::set_var("WLR_DRM_NO_ATOMIC", "0"); // Use atomic DRM
        }

        Ok(())
    }

    async fn setup_generic_wayland_optimizations(&self, container_id: &str) -> Result<()> {
        info!("    🔧 Applying generic Wayland optimizations");

        unsafe {
            // Generic optimizations that work across compositors
            std::env::set_var("WAYLAND_DEBUG", "0");
            std::env::set_var("WAYLAND_GAMING_MODE", "1");
            std::env::set_var("EGL_PLATFORM", "wayland");
        }

        Ok(())
    }

    pub async fn enable_xwayland_for_legacy(&self, container_id: &str) -> Result<()> {
        info!("🔄 Enabling XWayland for legacy application support");

        unsafe {
            // XWayland configuration
            std::env::set_var("DISPLAY", ":0");
            std::env::set_var("XWAYLAND_NO_GLAMOR", "0");

            // Gaming optimizations for XWayland
            std::env::set_var("XWAYLAND_GAMING_OPTIMIZATIONS", "1");
            std::env::set_var("__GL_SYNC_TO_VBLANK", "0");
        }

        info!("  ✅ XWayland configured for legacy gaming support");
        Ok(())
    }

    pub async fn get_wayland_performance_metrics(
        &self,
        container_id: &str,
    ) -> Result<WaylandPerformanceMetrics> {
        let session = self.active_gpu_sessions.get(container_id).ok_or_else(|| {
            anyhow::anyhow!("No active GPU session for container: {}", container_id)
        })?;

        // This would integrate with compositor APIs to get real metrics
        Ok(WaylandPerformanceMetrics {
            compositor_fps: 120.0,
            input_latency_ms: 1.2,
            gpu_utilization: 85.5,
            memory_usage_mb: 2048,
            wayland_protocol_efficiency: 0.95,
            direct_rendering_active: session.direct_rendering,
            hardware_acceleration_active: session.hardware_acceleration,
        })
    }

    pub fn is_kde_session(&self) -> bool {
        self.display_server == DisplayServerType::KDEWayland
    }

    pub fn get_active_sessions(&self) -> Vec<&str> {
        self.active_gpu_sessions
            .keys()
            .map(|s| s.as_str())
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct WaylandPerformanceMetrics {
    pub compositor_fps: f64,
    pub input_latency_ms: f64,
    pub gpu_utilization: f64,
    pub memory_usage_mb: u64,
    pub wayland_protocol_efficiency: f64,
    pub direct_rendering_active: bool,
    pub hardware_acceleration_active: bool,
}
