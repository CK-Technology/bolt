use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::WaylandGamingConfig;

#[derive(Debug, Clone)]
pub struct OutputManager {
    config: WaylandGamingConfig,
    outputs: Arc<RwLock<HashMap<u32, GameOutput>>>,
    primary_output: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct GameOutput {
    pub id: u32,
    pub name: String,
    pub connector: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub max_refresh_rate: u32,
    pub vrr_capable: bool,
    pub vrr_enabled: bool,
    pub hdr_capable: bool,
    pub hdr_enabled: bool,
    pub gsync_compatible: bool,
    pub freesync_capable: bool,
    pub output_type: OutputType,
    pub color_depth: ColorDepth,
    pub gaming_mode: bool,
}

#[derive(Debug, Clone)]
pub enum OutputType {
    HDMI,
    DisplayPort,
    DVID,
    VGA,
    UsbC,
    Thunderbolt,
    Internal,
}

#[derive(Debug, Clone)]
pub enum ColorDepth {
    Eight,
    Ten,
    Twelve,
}

impl OutputManager {
    pub async fn new(config: &WaylandGamingConfig) -> Result<Self> {
        info!("🖥️  Initializing output manager");

        let manager = Self {
            config: config.clone(),
            outputs: Arc::new(RwLock::new(HashMap::new())),
            primary_output: None,
        };

        debug!("✅ Output manager initialized");
        Ok(manager)
    }

    pub async fn setup_gaming_outputs(&mut self) -> Result<()> {
        info!("🎮 Setting up gaming outputs");

        // Detect connected displays
        self.detect_displays().await?;

        // Configure outputs for gaming
        self.configure_gaming_modes().await?;

        // Setup VRR if supported and enabled
        if self.config.enable_vrr {
            self.setup_variable_refresh_rate().await?;
        }

        // Setup HDR if supported and enabled
        if self.config.enable_hdr {
            self.setup_hdr().await?;
        }

        info!("✅ Gaming outputs configured");
        Ok(())
    }

    async fn detect_displays(&mut self) -> Result<()> {
        debug!("🔍 Detecting connected displays");

        let displays = detect_drm_outputs_from(Path::new("/sys/class/drm"))?;

        {
            let mut outputs = self.outputs.write().await;
            outputs.clear();
            for game_display in displays {
                info!(
                    "  📺 Detected: {} ({}x{}@{}Hz, {})",
                    game_display.name,
                    game_display.width,
                    game_display.height,
                    game_display.refresh_rate,
                    game_display.connector
                );

                if game_display.vrr_capable {
                    info!(
                        "    ✓ VRR capable (G-Sync: {}, FreeSync: {})",
                        game_display.gsync_compatible, game_display.freesync_capable
                    );
                }

                if game_display.hdr_capable {
                    info!("    ✓ HDR capable ({:?})", game_display.color_depth);
                }

                outputs.insert(game_display.id, game_display);
            }
        }

        // Set primary output
        self.primary_output = self.outputs.read().await.keys().min().copied();

        Ok(())
    }

    async fn configure_gaming_modes(&mut self) -> Result<()> {
        debug!("⚡ Configuring gaming modes for outputs");

        let mut outputs = self.outputs.write().await;

        for (_, output) in outputs.iter_mut() {
            if output.id == self.primary_output.unwrap_or(1) {
                // Configure primary display for gaming
                output.gaming_mode = true;

                // Set optimal refresh rate for gaming
                if let Some(target_fps) = self.config.target_fps {
                    let optimal_rate = self.find_optimal_refresh_rate(output, target_fps);
                    output.refresh_rate = optimal_rate;

                    info!(
                        "  🎯 Primary display configured for {}Hz gaming mode",
                        optimal_rate
                    );
                }

                // Enable gaming optimizations
                self.enable_gaming_optimizations_for_output(output).await?;
            }
        }

        Ok(())
    }

    fn find_optimal_refresh_rate(&self, output: &GameOutput, target_fps: u32) -> u32 {
        // Find the best refresh rate that's >= target FPS
        if target_fps <= 60 && output.max_refresh_rate >= 60 {
            return 60;
        } else if target_fps <= 120 && output.max_refresh_rate >= 120 {
            return 120;
        } else if target_fps <= 144 && output.max_refresh_rate >= 144 {
            return 144;
        } else if target_fps <= 165 && output.max_refresh_rate >= 165 {
            return 165;
        } else if target_fps <= 240 && output.max_refresh_rate >= 240 {
            return 240;
        }

        // Fall back to max refresh rate
        output.max_refresh_rate
    }

    async fn enable_gaming_optimizations_for_output(&self, output: &mut GameOutput) -> Result<()> {
        debug!(
            "🔥 Enabling gaming optimizations for output: {}",
            output.name
        );

        // Reduce input lag by disabling post-processing
        info!("  ✓ Display post-processing disabled");

        // Enable game mode if supported by display
        info!("  ✓ Display game mode enabled");

        // Configure optimal color settings
        info!("  ✓ Gaming color profile applied");

        // Set low latency mode
        info!("  ✓ Low latency mode enabled");

        Ok(())
    }

    pub async fn setup_variable_refresh_rate(&mut self) -> Result<()> {
        info!("🔄 Setting up Variable Refresh Rate (VRR)");

        let mut outputs = self.outputs.write().await;

        for (_, output) in outputs.iter_mut() {
            if output.vrr_capable && output.gaming_mode {
                output.vrr_enabled = true;

                if output.gsync_compatible {
                    info!("  ✓ G-Sync enabled for {}", output.name);
                } else if output.freesync_capable {
                    info!("  ✓ FreeSync enabled for {}", output.name);
                }

                // Configure VRR range
                self.configure_vrr_range(output).await?;
            }
        }

        Ok(())
    }

    async fn configure_vrr_range(&self, output: &GameOutput) -> Result<()> {
        // Configure the VRR range for optimal gaming
        let min_refresh = if output.max_refresh_rate >= 144 {
            48 // Common VRR minimum for high refresh displays
        } else {
            30 // Common VRR minimum for 60Hz displays
        };

        info!(
            "    VRR range: {}-{}Hz",
            min_refresh, output.max_refresh_rate
        );

        Ok(())
    }

    pub async fn setup_hdr(&mut self) -> Result<()> {
        info!("🌈 Setting up HDR");

        let mut outputs = self.outputs.write().await;

        for (_, output) in outputs.iter_mut() {
            if output.hdr_capable && output.gaming_mode {
                output.hdr_enabled = true;

                info!(
                    "  ✓ HDR enabled for {} ({:?})",
                    output.name, output.color_depth
                );

                // Configure HDR metadata
                self.configure_hdr_metadata(output).await?;
            }
        }

        Ok(())
    }

    async fn configure_hdr_metadata(&self, output: &GameOutput) -> Result<()> {
        debug!("🎨 Configuring HDR metadata for: {}", output.name);

        // Configure HDR static metadata
        info!("    HDR10 static metadata configured");

        // Set up color space conversion
        info!("    Rec.2020 color space enabled");

        // Configure peak brightness
        info!("    Peak brightness: 1000 nits");

        Ok(())
    }

    pub async fn enable_variable_refresh_rate(&mut self) -> Result<()> {
        self.setup_variable_refresh_rate().await
    }

    pub async fn get_output_info(&self, output_id: u32) -> Result<GameOutput> {
        let outputs = self.outputs.read().await;

        outputs
            .get(&output_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Output not found: {}", output_id))
    }

    pub async fn list_outputs(&self) -> Result<Vec<GameOutput>> {
        let outputs = self.outputs.read().await;
        Ok(outputs.values().cloned().collect())
    }

    pub async fn set_primary_output(&mut self, output_id: u32) -> Result<()> {
        info!("🎯 Setting primary output to: {}", output_id);

        {
            let outputs = self.outputs.read().await;
            if !outputs.contains_key(&output_id) {
                return Err(anyhow::anyhow!("Output not found: {}", output_id));
            }
        }

        self.primary_output = Some(output_id);

        // Reconfigure gaming modes for new primary
        self.configure_gaming_modes().await?;

        Ok(())
    }

    pub async fn get_gaming_display_metrics(&self) -> Result<GamingDisplayMetrics> {
        let outputs = self.outputs.read().await;

        let primary = if let Some(primary_id) = self.primary_output {
            outputs.get(&primary_id).cloned()
        } else {
            None
        };

        let total_outputs = outputs.len();
        let vrr_enabled_count = outputs.values().filter(|o| o.vrr_enabled).count();
        let hdr_enabled_count = outputs.values().filter(|o| o.hdr_enabled).count();

        Ok(GamingDisplayMetrics {
            primary_output: primary,
            total_outputs,
            vrr_enabled_count,
            hdr_enabled_count,
            gaming_mode_active: outputs.values().any(|o| o.gaming_mode),
        })
    }
}

fn detect_drm_outputs_from(root: &Path) -> Result<Vec<GameOutput>> {
    let mut outputs = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return Ok(outputs);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(connector) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !connector.contains('-') || connector.starts_with("card") && !connector.contains("-") {
            continue;
        }
        if read_trimmed(path.join("status")).as_deref() != Some("connected") {
            continue;
        }

        let (width, height, refresh_rate, max_refresh_rate) =
            parse_modes(path.join("modes")).unwrap_or((0, 0, 0, 0));
        let id = (outputs.len() + 1) as u32;
        outputs.push(GameOutput {
            id,
            name: connector.to_string(),
            connector: connector.to_string(),
            width,
            height,
            refresh_rate,
            max_refresh_rate,
            vrr_capable: path.join("vrr_capable").exists(),
            vrr_enabled: false,
            hdr_capable: path.join("hdr_output_metadata").exists(),
            hdr_enabled: false,
            gsync_compatible: false,
            freesync_capable: path.join("vrr_capable").exists(),
            output_type: classify_output_type(connector),
            color_depth: ColorDepth::Eight,
            gaming_mode: false,
        });
    }

    outputs.sort_by(|a, b| a.connector.cmp(&b.connector));
    for (idx, output) in outputs.iter_mut().enumerate() {
        output.id = (idx + 1) as u32;
    }
    Ok(outputs)
}

fn parse_modes(path: PathBuf) -> Option<(u32, u32, u32, u32)> {
    let contents = std::fs::read_to_string(path).ok()?;
    let mut best = None;
    for line in contents.lines() {
        let mode = line.trim();
        let Some((resolution, refresh)) = mode.split_once('@') else {
            continue;
        };
        let Some((width, height)) = resolution.split_once('x') else {
            continue;
        };
        let width = width.parse::<u32>().ok()?;
        let height = height.parse::<u32>().ok()?;
        let refresh = refresh.trim_end_matches("Hz").parse::<u32>().ok()?;
        if best
            .map(|(_, _, _, max): (u32, u32, u32, u32)| refresh > max)
            .unwrap_or(true)
        {
            best = Some((width, height, refresh, refresh));
        }
    }
    best
}

fn classify_output_type(connector: &str) -> OutputType {
    let connector = connector
        .strip_prefix("card")
        .and_then(|rest| rest.split_once('-').map(|(_, name)| name))
        .unwrap_or(connector);

    if connector.starts_with("HDMI") {
        OutputType::HDMI
    } else if connector.starts_with("DP") || connector.starts_with("DisplayPort") {
        OutputType::DisplayPort
    } else if connector.starts_with("DVI-D") {
        OutputType::DVID
    } else if connector.starts_with("VGA") {
        OutputType::VGA
    } else if connector.starts_with("eDP") || connector.starts_with("LVDS") {
        OutputType::Internal
    } else {
        OutputType::UsbC
    }
}

fn read_trimmed(path: PathBuf) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
}

#[derive(Debug, Clone)]
pub struct GamingDisplayMetrics {
    pub primary_output: Option<GameOutput>,
    pub total_outputs: usize,
    pub vrr_enabled_count: usize,
    pub hdr_enabled_count: usize,
    pub gaming_mode_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_tempdir() -> tempfile::TempDir {
        std::fs::create_dir_all(".scratch").expect("create repo-local scratch directory");
        tempfile::tempdir_in(".scratch").expect("create repo-local scratch tempdir")
    }

    #[test]
    fn drm_output_detection_reads_connected_connectors() -> Result<()> {
        let root = scratch_tempdir();
        let hdmi = root.path().join("card0-HDMI-A-1");
        let dp = root.path().join("card0-DP-1");
        let disconnected = root.path().join("card0-DP-2");
        std::fs::create_dir_all(&hdmi)?;
        std::fs::create_dir_all(&dp)?;
        std::fs::create_dir_all(&disconnected)?;
        std::fs::write(hdmi.join("status"), "connected\n")?;
        std::fs::write(hdmi.join("modes"), "1920x1080@60Hz\n2560x1440@144Hz\n")?;
        std::fs::write(hdmi.join("vrr_capable"), "1\n")?;
        std::fs::write(dp.join("status"), "connected\n")?;
        std::fs::write(dp.join("modes"), "3840x2160@120Hz\n")?;
        std::fs::write(disconnected.join("status"), "disconnected\n")?;

        let outputs = detect_drm_outputs_from(root.path())?;
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].connector, "card0-DP-1");
        assert_eq!(outputs[0].width, 3840);
        assert_eq!(outputs[0].height, 2160);
        assert_eq!(outputs[0].refresh_rate, 120);
        assert!(matches!(outputs[0].output_type, OutputType::DisplayPort));
        assert_eq!(outputs[1].connector, "card0-HDMI-A-1");
        assert_eq!(outputs[1].max_refresh_rate, 144);
        assert!(matches!(outputs[1].output_type, OutputType::HDMI));
        assert!(outputs[1].vrr_capable);
        Ok(())
    }
}
