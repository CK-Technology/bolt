use crate::{BoltError, Result};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command as AsyncCommand;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayTechConfig {
    /// Enable Variable Refresh Rate (G-Sync/FreeSync)
    pub vrr_enabled: bool,
    /// VRR frequency range (min, max)
    pub vrr_range: Option<(u32, u32)>,
    /// Enable HDR (High Dynamic Range)
    pub hdr_enabled: bool,
    /// HDR color space
    pub hdr_colorspace: HDRColorSpace,
    /// HDR peak brightness (nits)
    pub hdr_peak_brightness: u32,
    /// Display color depth
    pub color_depth: ColorDepth,
    /// Adaptive brightness for HDR
    pub adaptive_brightness: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HDRColorSpace {
    /// Rec. 709 (Standard Dynamic Range)
    Rec709,
    /// Rec. 2020 (Wide Color Gamut)
    Rec2020,
    /// DCI-P3 (Digital Cinema)
    DCIP3,
    /// Adobe RGB
    AdobeRGB,
    /// Automatic selection
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColorDepth {
    /// 8-bit per channel
    Depth8Bit,
    /// 10-bit per channel (HDR)
    Depth10Bit,
    /// 12-bit per channel (High-end HDR)
    Depth12Bit,
    /// Automatic selection
    Auto,
}

#[derive(Debug)]
pub struct DisplayTechManager {
    config: DisplayTechConfig,
    detected_displays: Arc<RwLock<Vec<DisplayDevice>>>,
    vrr_controller: Arc<VRRController>,
    hdr_controller: Arc<HDRController>,
}

#[derive(Debug, Clone)]
pub struct DisplayDevice {
    pub name: String,
    pub connector: String,
    pub resolution: (u32, u32),
    pub refresh_rate: u32,
    pub vrr_supported: bool,
    pub vrr_range: Option<(u32, u32)>,
    pub hdr_supported: bool,
    pub hdr_formats: Vec<String>,
    pub color_depth: u32,
    pub is_gaming_monitor: bool,
    pub manufacturer: String,
    pub model: String,
}

#[derive(Debug)]
pub struct VRRController {
    enabled: Arc<RwLock<bool>>,
    current_range: Arc<RwLock<Option<(u32, u32)>>>,
    supported_displays: Arc<RwLock<Vec<String>>>,
}

#[derive(Debug)]
pub struct HDRController {
    enabled: Arc<RwLock<bool>>,
    current_colorspace: Arc<RwLock<HDRColorSpace>>,
    peak_brightness: Arc<RwLock<u32>>,
    supported_displays: Arc<RwLock<Vec<String>>>,
}

impl Default for DisplayTechConfig {
    fn default() -> Self {
        Self {
            vrr_enabled: true,
            vrr_range: None, // Auto-detect
            hdr_enabled: true,
            hdr_colorspace: HDRColorSpace::Auto,
            hdr_peak_brightness: 1000, // 1000 nits typical gaming monitor
            color_depth: ColorDepth::Auto,
            adaptive_brightness: false, // Don't change brightness during gaming
        }
    }
}

impl DisplayTechManager {
    pub async fn new(config: DisplayTechConfig) -> Result<Self> {
        info!("🖥️ Initializing Display Technology Manager");
        info!("   VRR: {} ({:?})", config.vrr_enabled, config.vrr_range);
        info!("   HDR: {} ({:?}, {} nits)", config.hdr_enabled, config.hdr_colorspace, config.hdr_peak_brightness);
        info!("   Color Depth: {:?}", config.color_depth);

        // Detect connected displays
        let detected_displays = Self::detect_displays().await?;
        info!("   Detected {} display(s)", detected_displays.len());

        for display in &detected_displays {
            info!("   📺 {}: {}x{}@{}Hz", display.name, display.resolution.0, display.resolution.1, display.refresh_rate);
            if display.vrr_supported {
                info!("      VRR: {:?}Hz", display.vrr_range);
            }
            if display.hdr_supported {
                info!("      HDR: {} formats", display.hdr_formats.len());
            }
        }

        // Initialize VRR controller
        let vrr_displays: Vec<String> = detected_displays.iter()
            .filter(|d| d.vrr_supported)
            .map(|d| d.name.clone())
            .collect();

        let vrr_controller = Arc::new(VRRController {
            enabled: Arc::new(RwLock::new(config.vrr_enabled)),
            current_range: Arc::new(RwLock::new(config.vrr_range)),
            supported_displays: Arc::new(RwLock::new(vrr_displays)),
        });

        // Initialize HDR controller
        let hdr_displays: Vec<String> = detected_displays.iter()
            .filter(|d| d.hdr_supported)
            .map(|d| d.name.clone())
            .collect();

        let hdr_controller = Arc::new(HDRController {
            enabled: Arc::new(RwLock::new(config.hdr_enabled)),
            current_colorspace: Arc::new(RwLock::new(config.hdr_colorspace.clone())),
            peak_brightness: Arc::new(RwLock::new(config.hdr_peak_brightness)),
            supported_displays: Arc::new(RwLock::new(hdr_displays)),
        });

        let manager = Self {
            config,
            detected_displays: Arc::new(RwLock::new(detected_displays)),
            vrr_controller,
            hdr_controller,
        };

        // Apply initial configuration
        if manager.config.vrr_enabled {
            manager.enable_vrr().await?;
        }

        if manager.config.hdr_enabled {
            manager.enable_hdr().await?;
        }

        info!("✅ Display Technology Manager initialized");
        Ok(manager)
    }

    async fn detect_displays() -> Result<Vec<DisplayDevice>> {
        let mut displays = Vec::new();

        // Try different methods to detect displays
        if let Ok(wayland_displays) = Self::detect_wayland_displays().await {
            displays.extend(wayland_displays);
        }

        if displays.is_empty() {
            if let Ok(x11_displays) = Self::detect_x11_displays().await {
                displays.extend(x11_displays);
            }
        }

        if displays.is_empty() {
            if let Ok(drm_displays) = Self::detect_drm_displays().await {
                displays.extend(drm_displays);
            }
        }

        if displays.is_empty() {
            warn!("No displays detected, using fallback");
            displays.push(DisplayDevice {
                name: "Unknown Display".to_string(),
                connector: "Unknown".to_string(),
                resolution: (1920, 1080),
                refresh_rate: 60,
                vrr_supported: false,
                vrr_range: None,
                hdr_supported: false,
                hdr_formats: Vec::new(),
                color_depth: 8,
                is_gaming_monitor: false,
                manufacturer: "Unknown".to_string(),
                model: "Unknown".to_string(),
            });
        }

        Ok(displays)
    }

    async fn detect_wayland_displays() -> Result<Vec<DisplayDevice>> {
        let output = AsyncCommand::new("wlr-randr")
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("wlr-randr failed"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(Self::parse_wayland_displays(&stdout))
    }

    fn parse_wayland_displays(output: &str) -> Vec<DisplayDevice> {
        let mut displays = Vec::new();
        let mut current_display: Option<DisplayDevice> = None;

        for line in output.lines() {
            if !line.starts_with(' ') && line.contains("\"") {
                // New display
                if let Some(display) = current_display.take() {
                    displays.push(display);
                }

                let name = line.split('"').nth(1).unwrap_or("Unknown").to_string();
                current_display = Some(DisplayDevice {
                    name: name.clone(),
                    connector: name,
                    resolution: (1920, 1080),
                    refresh_rate: 60,
                    vrr_supported: false,
                    vrr_range: None,
                    hdr_supported: false,
                    hdr_formats: Vec::new(),
                    color_depth: 8,
                    is_gaming_monitor: false,
                    manufacturer: "Unknown".to_string(),
                    model: "Unknown".to_string(),
                });
            } else if let Some(ref mut display) = current_display {
                if line.contains("current") {
                    // Parse resolution and refresh rate
                    if let Some(mode_part) = line.split_whitespace().find(|s| s.contains("@")) {
                        Self::parse_display_mode(mode_part, display);
                    }
                } else if line.contains("VRR") || line.contains("Variable") {
                    display.vrr_supported = true;
                    if let Some(range) = Self::parse_vrr_range(line) {
                        display.vrr_range = Some(range);
                    }
                } else if line.contains("HDR") {
                    display.hdr_supported = true;
                    display.hdr_formats = Self::parse_hdr_formats(line);
                }
            }
        }

        if let Some(display) = current_display {
            displays.push(display);
        }

        displays
    }

    async fn detect_x11_displays() -> Result<Vec<DisplayDevice>> {
        let output = AsyncCommand::new("xrandr")
            .args(&["--verbose"])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("xrandr failed"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(Self::parse_x11_displays(&stdout))
    }

    fn parse_x11_displays(output: &str) -> Vec<DisplayDevice> {
        let mut displays = Vec::new();
        let mut current_display: Option<DisplayDevice> = None;

        for line in output.lines() {
            if line.contains(" connected") && !line.contains("disconnected") {
                // New connected display
                if let Some(display) = current_display.take() {
                    displays.push(display);
                }

                let name = line.split_whitespace().next().unwrap_or("Unknown").to_string();
                current_display = Some(DisplayDevice {
                    name: name.clone(),
                    connector: name,
                    resolution: (1920, 1080),
                    refresh_rate: 60,
                    vrr_supported: false,
                    vrr_range: None,
                    hdr_supported: false,
                    hdr_formats: Vec::new(),
                    color_depth: 8,
                    is_gaming_monitor: line.to_lowercase().contains("gaming") ||
                                      line.to_lowercase().contains("rog") ||
                                      line.to_lowercase().contains("predator"),
                    manufacturer: "Unknown".to_string(),
                    model: "Unknown".to_string(),
                });
            } else if let Some(ref mut display) = current_display {
                if line.contains("*") && line.contains("+") {
                    // Current mode
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(mode) = parts.first() {
                        Self::parse_display_mode(mode, display);
                    }
                } else if line.contains("Variable refresh rate") || line.contains("VRR") {
                    display.vrr_supported = true;
                } else if line.contains("HDR") {
                    display.hdr_supported = true;
                }
            }
        }

        if let Some(display) = current_display {
            displays.push(display);
        }

        displays
    }

    async fn detect_drm_displays() -> Result<Vec<DisplayDevice>> {
        let mut displays = Vec::new();

        // Check DRM subsystem for display information
        let drm_path = Path::new("/sys/class/drm");
        if !drm_path.exists() {
            return Err(anyhow::anyhow!("DRM subsystem not available"));
        }

        let entries = std::fs::read_dir(drm_path)?;

        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if name_str.starts_with("card") && name_str.contains("-") {
                // This is a connector
                let status_path = entry.path().join("status");
                let enabled_path = entry.path().join("enabled");

                if let (Ok(status), Ok(enabled)) = (
                    std::fs::read_to_string(&status_path),
                    std::fs::read_to_string(&enabled_path),
                ) {
                    if status.trim() == "connected" && enabled.trim() == "enabled" {
                        let mut display = DisplayDevice {
                            name: name_str.to_string(),
                            connector: name_str.to_string(),
                            resolution: (1920, 1080), // Default
                            refresh_rate: 60, // Default
                            vrr_supported: false,
                            vrr_range: None,
                            hdr_supported: false,
                            hdr_formats: Vec::new(),
                            color_depth: 8,
                            is_gaming_monitor: false,
                            manufacturer: "Unknown".to_string(),
                            model: "Unknown".to_string(),
                        };

                        // Check for VRR support
                        let vrr_capable_path = entry.path().join("vrr_capable");
                        if let Ok(vrr_capable) = std::fs::read_to_string(&vrr_capable_path) {
                            display.vrr_supported = vrr_capable.trim() == "1";
                        }

                        // Check for HDR support
                        let hdr_path = entry.path().join("hdr_output_metadata");
                        display.hdr_supported = hdr_path.exists();

                        displays.push(display);
                    }
                }
            }
        }

        Ok(displays)
    }

    fn parse_display_mode(mode_str: &str, display: &mut DisplayDevice) {
        // Parse "1920x1080@144.000000Hz" or similar
        if let Some((res_part, rate_part)) = mode_str.split_once('@') {
            // Parse resolution
            if let Some((width_str, height_str)) = res_part.split_once('x') {
                if let (Ok(width), Ok(height)) = (width_str.parse(), height_str.parse()) {
                    display.resolution = (width, height);
                }
            }
            // Parse refresh rate
            let rate_str = rate_part.trim_end_matches("Hz");
            if let Ok(rate) = rate_str.parse::<f64>() {
                display.refresh_rate = rate as u32;
            }
        }
    }

    fn parse_vrr_range(line: &str) -> Option<(u32, u32)> {
        // Try to extract VRR range from line like "VRR: 48-144Hz"
        if let Some(range_part) = line.split(':').nth(1) {
            if let Some((min_str, max_str)) = range_part.trim().split_once('-') {
                let min_str = min_str.trim();
                let max_str = max_str.trim_end_matches("Hz").trim();
                if let (Ok(min), Ok(max)) = (min_str.parse(), max_str.parse()) {
                    return Some((min, max));
                }
            }
        }
        None
    }

    fn parse_hdr_formats(line: &str) -> Vec<String> {
        // Extract HDR formats from the line
        let mut formats = Vec::new();
        let line_lower = line.to_lowercase();

        if line_lower.contains("hdr10") {
            formats.push("HDR10".to_string());
        }
        if line_lower.contains("dolby vision") {
            formats.push("Dolby Vision".to_string());
        }
        if line_lower.contains("hdr10+") {
            formats.push("HDR10+".to_string());
        }

        formats
    }

    pub async fn enable_vrr(&self) -> Result<()> {
        info!("📺 Enabling Variable Refresh Rate (VRR)");

        *self.vrr_controller.enabled.write().await = true;

        let displays = self.detected_displays.read().await;
        let vrr_displays: Vec<&DisplayDevice> = displays.iter()
            .filter(|d| d.vrr_supported)
            .collect();

        if vrr_displays.is_empty() {
            warn!("No VRR-capable displays found");
            return Ok(());
        }

        for display in vrr_displays {
            info!("   Enabling VRR on {}", display.name);

            // Try different methods to enable VRR
            if let Err(e) = self.enable_vrr_wayland(&display.name).await {
                debug!("Wayland VRR failed: {}, trying X11", e);
                if let Err(e) = self.enable_vrr_x11(&display.name).await {
                    debug!("X11 VRR failed: {}, trying DRM", e);
                    if let Err(e) = self.enable_vrr_drm(&display.connector).await {
                        warn!("Failed to enable VRR on {}: {}", display.name, e);
                    }
                }
            }
        }

        info!("✅ VRR configuration completed");
        Ok(())
    }

    async fn enable_vrr_wayland(&self, display_name: &str) -> Result<()> {
        // Enable VRR via wlr-randr
        let output = AsyncCommand::new("wlr-randr")
            .args(&["--output", display_name, "--adaptive-sync", "enabled"])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("wlr-randr VRR failed: {}", stderr));
        }

        Ok(())
    }

    async fn enable_vrr_x11(&self, display_name: &str) -> Result<()> {
        // Enable VRR via xrandr
        let output = AsyncCommand::new("xrandr")
            .args(&["--output", display_name, "--set", "vrr_capable", "1"])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("xrandr VRR failed: {}", stderr));
        }

        Ok(())
    }

    async fn enable_vrr_drm(&self, connector: &str) -> Result<()> {
        // Enable VRR via DRM properties
        let vrr_path = format!("/sys/class/drm/{}/vrr_capable", connector);
        if Path::new(&vrr_path).exists() {
            std::fs::write(&vrr_path, "1")?;
        }

        Ok(())
    }

    pub async fn enable_hdr(&self) -> Result<()> {
        info!("🌈 Enabling HDR (High Dynamic Range)");

        *self.hdr_controller.enabled.write().await = true;

        let displays = self.detected_displays.read().await;
        let hdr_displays: Vec<&DisplayDevice> = displays.iter()
            .filter(|d| d.hdr_supported)
            .collect();

        if hdr_displays.is_empty() {
            warn!("No HDR-capable displays found");
            return Ok(());
        }

        for display in hdr_displays {
            info!("   Enabling HDR on {}", display.name);

            // Configure HDR
            if let Err(e) = self.configure_hdr_display(display).await {
                warn!("Failed to configure HDR on {}: {}", display.name, e);
            }
        }

        // Set system-wide HDR environment variables
        self.set_hdr_environment().await?;

        info!("✅ HDR configuration completed");
        Ok(())
    }

    async fn configure_hdr_display(&self, display: &DisplayDevice) -> Result<()> {
        // Configure HDR for specific display
        let colorspace = match self.config.hdr_colorspace {
            HDRColorSpace::Rec709 => "Rec709",
            HDRColorSpace::Rec2020 => "Rec2020",
            HDRColorSpace::DCIP3 => "DCI-P3",
            HDRColorSpace::AdobeRGB => "Adobe RGB",
            HDRColorSpace::Auto => "Auto",
        };

        info!("      Colorspace: {}, Brightness: {} nits", colorspace, self.config.hdr_peak_brightness);

        // Try to set HDR mode via different methods
        if let Err(e) = self.set_hdr_wayland(&display.name, colorspace).await {
            debug!("Wayland HDR failed: {}, trying X11", e);
            if let Err(e) = self.set_hdr_x11(&display.name, colorspace).await {
                debug!("X11 HDR failed: {}, trying DRM", e);
                self.set_hdr_drm(&display.connector, colorspace).await?;
            }
        }

        Ok(())
    }

    async fn set_hdr_wayland(&self, display_name: &str, colorspace: &str) -> Result<()> {
        // Set HDR via wlr-randr (if supported)
        let output = AsyncCommand::new("wlr-randr")
            .args(&["--output", display_name, "--hdr", "on", "--colorspace", colorspace])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("wlr-randr HDR failed: {}", stderr));
        }

        Ok(())
    }

    async fn set_hdr_x11(&self, display_name: &str, colorspace: &str) -> Result<()> {
        // Set HDR via xrandr
        let output = AsyncCommand::new("xrandr")
            .args(&["--output", display_name, "--set", "Colorspace", colorspace])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("xrandr HDR failed: {}", stderr));
        }

        Ok(())
    }

    async fn set_hdr_drm(&self, connector: &str, _colorspace: &str) -> Result<()> {
        // Set HDR via DRM properties
        let hdr_path = format!("/sys/class/drm/{}/hdr_output_metadata", connector);
        if Path::new(&hdr_path).exists() {
            // HDR metadata would be more complex in reality
            std::fs::write(&hdr_path, "1")?;
        }

        Ok(())
    }

    async fn set_hdr_environment(&self) -> Result<()> {
        // Set HDR-related environment variables
        unsafe {
            std::env::set_var("KWIN_HDR", "1");
            std::env::set_var("MESA_VK_ENABLE_HDR", "1");
            std::env::set_var("VK_HDR_SURFACE_EXTENSION", "1");
            std::env::set_var("HDR_PEAK_BRIGHTNESS", &self.config.hdr_peak_brightness.to_string());

            match self.config.hdr_colorspace {
                HDRColorSpace::Rec2020 => {
                    std::env::set_var("COLORSPACE", "Rec2020");
                }
                HDRColorSpace::DCIP3 => {
                    std::env::set_var("COLORSPACE", "DCI-P3");
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub async fn configure_container_display_tech(&self, container_id: &str) -> Result<HashMap<String, String>> {
        info!("🖥️ Configuring display technology for container: {}", container_id);

        let mut env_vars = HashMap::new();

        // VRR configuration
        let vrr_enabled = *self.vrr_controller.enabled.read().await;
        env_vars.insert("VRR_ENABLED".to_string(), vrr_enabled.to_string());

        if vrr_enabled {
            env_vars.insert("__GL_GSYNC_ALLOWED".to_string(), "1".to_string());
            env_vars.insert("__GL_VRR_ALLOWED".to_string(), "1".to_string());
            env_vars.insert("KWIN_VRR".to_string(), "1".to_string());
        }

        // HDR configuration
        let hdr_enabled = *self.hdr_controller.enabled.read().await;
        env_vars.insert("HDR_ENABLED".to_string(), hdr_enabled.to_string());

        if hdr_enabled {
            env_vars.insert("KWIN_HDR".to_string(), "1".to_string());
            env_vars.insert("MESA_VK_ENABLE_HDR".to_string(), "1".to_string());
            env_vars.insert("HDR_PEAK_BRIGHTNESS".to_string(), self.config.hdr_peak_brightness.to_string());

            let colorspace = match self.config.hdr_colorspace {
                HDRColorSpace::Rec709 => "Rec709",
                HDRColorSpace::Rec2020 => "Rec2020",
                HDRColorSpace::DCIP3 => "DCI-P3",
                HDRColorSpace::AdobeRGB => "Adobe RGB",
                HDRColorSpace::Auto => "Auto",
            };
            env_vars.insert("HDR_COLORSPACE".to_string(), colorspace.to_string());
        }

        // Color depth configuration
        let color_depth_value = match self.config.color_depth {
            ColorDepth::Depth8Bit => "8",
            ColorDepth::Depth10Bit => "10",
            ColorDepth::Depth12Bit => "12",
            ColorDepth::Auto => "auto",
        };
        env_vars.insert("COLOR_DEPTH".to_string(), color_depth_value.to_string());

        // Display information for games
        let displays = self.detected_displays.read().await;
        if let Some(primary_display) = displays.first() {
            env_vars.insert("PRIMARY_DISPLAY_WIDTH".to_string(), primary_display.resolution.0.to_string());
            env_vars.insert("PRIMARY_DISPLAY_HEIGHT".to_string(), primary_display.resolution.1.to_string());
            env_vars.insert("PRIMARY_DISPLAY_REFRESH".to_string(), primary_display.refresh_rate.to_string());

            if let Some((min_vrr, max_vrr)) = primary_display.vrr_range {
                env_vars.insert("VRR_MIN_RATE".to_string(), min_vrr.to_string());
                env_vars.insert("VRR_MAX_RATE".to_string(), max_vrr.to_string());
            }
        }

        info!("✅ Display technology configured with {} parameters", env_vars.len());
        Ok(env_vars)
    }

    pub async fn get_display_capabilities(&self) -> Vec<DisplayDevice> {
        self.detected_displays.read().await.clone()
    }

    pub async fn verify_display_tech_performance(&self, container_id: &str) -> Result<DisplayTechReport> {
        info!("📊 Verifying display technology performance for container: {}", container_id);

        let vrr_enabled = *self.vrr_controller.enabled.read().await;
        let hdr_enabled = *self.hdr_controller.enabled.read().await;

        let displays = self.detected_displays.read().await;
        let gaming_displays = displays.iter().filter(|d| d.is_gaming_monitor).count();
        let vrr_displays = displays.iter().filter(|d| d.vrr_supported && vrr_enabled).count();
        let hdr_displays = displays.iter().filter(|d| d.hdr_supported && hdr_enabled).count();

        let report = DisplayTechReport {
            vrr_active: vrr_enabled,
            hdr_active: hdr_enabled,
            vrr_displays_active: vrr_displays,
            hdr_displays_active: hdr_displays,
            gaming_displays_detected: gaming_displays,
            total_displays: displays.len(),
            peak_brightness_nits: self.config.hdr_peak_brightness,
            colorspace: self.config.hdr_colorspace.clone(),
            performance_score: Self::calculate_performance_score(
                vrr_enabled, hdr_enabled, gaming_displays, vrr_displays, hdr_displays
            ),
        };

        info!("📈 Display Technology Report:");
        info!("   VRR: {} ({} displays)", report.vrr_active, report.vrr_displays_active);
        info!("   HDR: {} ({} displays, {} nits)", report.hdr_active, report.hdr_displays_active, report.peak_brightness_nits);
        info!("   Gaming Displays: {}/{}", report.gaming_displays_detected, report.total_displays);
        info!("   Performance Score: {}/100", report.performance_score);

        Ok(report)
    }

    fn calculate_performance_score(
        vrr_enabled: bool,
        hdr_enabled: bool,
        gaming_displays: usize,
        vrr_displays: usize,
        hdr_displays: usize,
    ) -> u32 {
        let mut score = 60; // Base score

        if vrr_enabled && vrr_displays > 0 {
            score += 20; // VRR adds significant gaming value
        }

        if hdr_enabled && hdr_displays > 0 {
            score += 15; // HDR adds visual quality
        }

        if gaming_displays > 0 {
            score += 5; // Gaming monitor detected
        }

        score.min(100)
    }
}

#[derive(Debug, Clone)]
pub struct DisplayTechReport {
    pub vrr_active: bool,
    pub hdr_active: bool,
    pub vrr_displays_active: usize,
    pub hdr_displays_active: usize,
    pub gaming_displays_detected: usize,
    pub total_displays: usize,
    pub peak_brightness_nits: u32,
    pub colorspace: HDRColorSpace,
    pub performance_score: u32,
}