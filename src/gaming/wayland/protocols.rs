use anyhow::{Context, Result};
use tracing::{debug, info};

/// Gaming-specific Wayland protocol extensions
/// These protocols provide low-latency, high-performance interfaces for gaming

#[derive(Debug, Clone)]
pub enum GamingProtocol {
    /// Direct scanout protocol for bypass composition
    DirectScanout,
    /// Variable refresh rate control
    VariableRefreshRate,
    /// Presentation timing for frame synchronization
    PresentationTiming,
    /// Linux DRM syncobj for GPU synchronization
    LinuxDrmSyncobj,
    /// Gaming-specific input optimizations
    GamingInput,
    /// HDR metadata and color management
    HdrMetadata,
    /// GameScope integration
    GameScope,
}

pub struct ProtocolManager {
    enabled_protocols: Vec<GamingProtocol>,
}

impl ProtocolManager {
    pub fn new() -> Self {
        Self {
            enabled_protocols: Vec::new(),
        }
    }

    pub async fn enable_gaming_protocols(&mut self) -> Result<()> {
        info!("🎮 Enabling gaming Wayland protocols");

        // Enable core gaming protocols
        self.enable_direct_scanout().await?;
        self.enable_presentation_timing().await?;
        self.enable_variable_refresh_rate().await?;
        self.enable_linux_drm_syncobj().await?;
        self.enable_gaming_input().await?;
        self.enable_hdr_metadata().await?;

        info!("✅ Gaming protocols enabled");
        Ok(())
    }

    async fn enable_direct_scanout(&mut self) -> Result<()> {
        debug!("📺 Enabling direct scanout protocol");

        // Direct scanout allows bypassing the compositor for full-screen games
        // This reduces latency by eliminating composition overhead

        self.enabled_protocols.push(GamingProtocol::DirectScanout);
        info!("  ✓ Direct scanout protocol enabled");

        Ok(())
    }

    async fn enable_presentation_timing(&mut self) -> Result<()> {
        debug!("⏱️  Enabling presentation timing protocol");

        // Presentation timing provides precise frame timing information
        // Essential for smooth gameplay and frame pacing

        self.enabled_protocols
            .push(GamingProtocol::PresentationTiming);
        info!("  ✓ Presentation timing protocol enabled");

        Ok(())
    }

    async fn enable_variable_refresh_rate(&mut self) -> Result<()> {
        debug!("🔄 Enabling variable refresh rate protocol");

        // VRR protocol for G-Sync/FreeSync support
        // Eliminates screen tearing and stuttering

        self.enabled_protocols
            .push(GamingProtocol::VariableRefreshRate);
        info!("  ✓ Variable refresh rate protocol enabled");

        Ok(())
    }

    async fn enable_linux_drm_syncobj(&mut self) -> Result<()> {
        debug!("🔗 Enabling Linux DRM syncobj protocol");

        // DRM syncobj for explicit GPU synchronization
        // Provides more efficient GPU-CPU synchronization

        self.enabled_protocols.push(GamingProtocol::LinuxDrmSyncobj);
        info!("  ✓ Linux DRM syncobj protocol enabled");

        Ok(())
    }

    async fn enable_gaming_input(&mut self) -> Result<()> {
        debug!("🎮 Enabling gaming input protocol");

        // Gaming input protocol for low-latency input handling
        // Provides raw input access and optimized event delivery

        self.enabled_protocols.push(GamingProtocol::GamingInput);
        info!("  ✓ Gaming input protocol enabled");

        Ok(())
    }

    async fn enable_hdr_metadata(&mut self) -> Result<()> {
        debug!("🌈 Enabling HDR metadata protocol");

        // HDR metadata protocol for high dynamic range content
        // Enables proper HDR gaming

        self.enabled_protocols.push(GamingProtocol::HdrMetadata);
        info!("  ✓ HDR metadata protocol enabled");

        Ok(())
    }

    pub fn is_protocol_enabled(&self, protocol: &GamingProtocol) -> bool {
        self.enabled_protocols.contains(protocol)
    }

    pub fn get_enabled_protocols(&self) -> &Vec<GamingProtocol> {
        &self.enabled_protocols
    }
}

impl PartialEq for GamingProtocol {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// Wayland protocol interface definitions for gaming
pub mod interfaces {
    /// Direct scanout interface for bypassing composition
    pub const DIRECT_SCANOUT_INTERFACE: &str = "bolt_direct_scanout";

    /// Variable refresh rate interface
    pub const VRR_INTERFACE: &str = "bolt_variable_refresh_rate";

    /// Gaming input interface
    pub const GAMING_INPUT_INTERFACE: &str = "bolt_gaming_input";

    /// HDR metadata interface
    pub const HDR_METADATA_INTERFACE: &str = "bolt_hdr_metadata";
}

/// Protocol message definitions
pub mod messages {
    #[derive(Debug)]
    pub enum DirectScanoutMessage {
        Enable,
        Disable,
        SetSurface { surface_id: u32 },
    }

    #[derive(Debug)]
    pub enum VrrMessage {
        EnableVrr { min_fps: u32, max_fps: u32 },
        DisableVrr,
        SetRefreshRate { fps: u32 },
    }

    #[derive(Debug)]
    pub enum GamingInputMessage {
        EnableRawInput,
        DisableRawInput,
        SetPollingRate { rate_hz: u32 },
    }

    #[derive(Debug)]
    pub enum HdrMessage {
        EnableHdr,
        DisableHdr,
        SetMetadata {
            max_luminance: u32,
            max_frame_average: u32,
            min_luminance: u32,
        },
    }
}
