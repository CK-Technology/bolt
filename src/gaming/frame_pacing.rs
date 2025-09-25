use crate::{BoltError, Result};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command as AsyncCommand;
use tokio::sync::{RwLock, Mutex};
use tokio::time::interval;
use tracing::{debug, info, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FramePacingConfig {
    /// Target frame rate (0 = unlimited)
    pub target_fps: u32,
    /// VSync mode
    pub vsync_mode: VSyncMode,
    /// Frame pacing strategy
    pub pacing_strategy: FramePacingStrategy,
    /// Enable adaptive sync (G-Sync/FreeSync)
    pub adaptive_sync: bool,
    /// Frame rate limiting method
    pub rate_limit_method: RateLimitMethod,
    /// Enable frame interpolation
    pub frame_interpolation: bool,
    /// Low latency mode priority
    pub low_latency_priority: LatencyPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VSyncMode {
    /// VSync disabled (may cause tearing)
    Off,
    /// Traditional VSync (may cause stuttering)
    On,
    /// Adaptive VSync (VSync only when FPS > refresh rate)
    Adaptive,
    /// Enhanced VSync with reduced input lag
    Enhanced,
    /// Fast VSync (Immediate mode)
    Fast,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FramePacingStrategy {
    /// No frame pacing
    None,
    /// Consistent frame timing
    Consistent,
    /// Predictive frame pacing
    Predictive,
    /// Motion-adaptive pacing
    MotionAdaptive,
    /// AI-assisted frame pacing
    AIAssisted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitMethod {
    /// GPU-based frame limiting
    GPU,
    /// CPU-based frame limiting
    CPU,
    /// Driver-level limiting
    Driver,
    /// Hybrid approach
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LatencyPriority {
    /// Prioritize visual quality
    Quality,
    /// Balance quality and latency
    Balanced,
    /// Minimize latency at all costs
    UltraLowLatency,
}

#[derive(Debug)]
pub struct FramePacingManager {
    config: FramePacingConfig,
    display_info: Arc<RwLock<DisplayInfo>>,
    frame_metrics: Arc<RwLock<FrameMetrics>>,
    pacing_controller: Arc<PacingController>,
    vsync_controller: Arc<VSyncController>,
}

#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub refresh_rate_hz: u32,
    pub resolution: (u32, u32),
    pub adaptive_sync_supported: bool,
    pub hdr_supported: bool,
    pub variable_refresh_rate: Option<(u32, u32)>, // Min/Max VRR range
    pub display_name: String,
}

#[derive(Debug, Clone)]
pub struct FrameMetrics {
    pub current_fps: f64,
    pub target_fps: u32,
    pub frame_time_ms: f64,
    pub frame_time_variance: f64,
    pub dropped_frames: u64,
    pub stutters_per_second: f64,
    pub input_lag_ms: f64,
    pub vsync_active: bool,
    pub adaptive_sync_active: bool,
    pub last_updated: Instant,
}

#[derive(Debug)]
pub struct PacingController {
    target_frame_time: Arc<Mutex<Duration>>,
    frame_history: Arc<Mutex<Vec<Instant>>>,
    prediction_buffer: Arc<Mutex<Vec<Duration>>>,
}

#[derive(Debug)]
pub struct VSyncController {
    current_mode: Arc<RwLock<VSyncMode>>,
    display_refresh_rate: u32,
    adaptive_threshold_fps: f64,
}

impl Default for FramePacingConfig {
    fn default() -> Self {
        Self {
            target_fps: 144, // High refresh gaming target
            vsync_mode: VSyncMode::Adaptive,
            pacing_strategy: FramePacingStrategy::Predictive,
            adaptive_sync: true,
            rate_limit_method: RateLimitMethod::Hybrid,
            frame_interpolation: false, // Adds latency
            low_latency_priority: LatencyPriority::Balanced,
        }
    }
}

impl FramePacingManager {
    pub async fn new(config: FramePacingConfig) -> Result<Self> {
        info!("🎬 Initializing Frame Pacing Manager");
        info!("   Target FPS: {}", config.target_fps);
        info!("   VSync Mode: {:?}", config.vsync_mode);
        info!("   Pacing Strategy: {:?}", config.pacing_strategy);
        info!("   Adaptive Sync: {}", config.adaptive_sync);

        // Detect display information
        let display_info = Self::detect_display_info().await?;
        info!("   Display: {} ({}Hz, {}x{})",
              display_info.display_name,
              display_info.refresh_rate_hz,
              display_info.resolution.0,
              display_info.resolution.1);

        if display_info.adaptive_sync_supported {
            info!("   ✅ Adaptive Sync (G-Sync/FreeSync) supported");
        }

        // Initialize frame metrics
        let frame_metrics = FrameMetrics {
            current_fps: 0.0,
            target_fps: config.target_fps,
            frame_time_ms: 0.0,
            frame_time_variance: 0.0,
            dropped_frames: 0,
            stutters_per_second: 0.0,
            input_lag_ms: 0.0,
            vsync_active: matches!(config.vsync_mode, VSyncMode::On | VSyncMode::Adaptive | VSyncMode::Enhanced),
            adaptive_sync_active: config.adaptive_sync && display_info.adaptive_sync_supported,
            last_updated: Instant::now(),
        };

        // Initialize pacing controller
        let target_frame_time = if config.target_fps > 0 {
            Duration::from_nanos(1_000_000_000 / config.target_fps as u64)
        } else {
            Duration::from_nanos(1_000_000) // 1ms for unlimited
        };

        let pacing_controller = Arc::new(PacingController {
            target_frame_time: Arc::new(Mutex::new(target_frame_time)),
            frame_history: Arc::new(Mutex::new(Vec::with_capacity(120))), // 2 seconds at 60fps
            prediction_buffer: Arc::new(Mutex::new(Vec::with_capacity(10))),
        });

        // Initialize VSync controller
        let vsync_controller = Arc::new(VSyncController {
            current_mode: Arc::new(RwLock::new(config.vsync_mode.clone())),
            display_refresh_rate: display_info.refresh_rate_hz,
            adaptive_threshold_fps: display_info.refresh_rate_hz as f64 * 0.95, // 95% of refresh rate
        });

        let manager = Self {
            config,
            display_info: Arc::new(RwLock::new(display_info)),
            frame_metrics: Arc::new(RwLock::new(frame_metrics)),
            pacing_controller,
            vsync_controller,
        };

        // Start frame monitoring
        manager.start_frame_monitoring().await?;

        info!("✅ Frame Pacing Manager initialized");
        Ok(manager)
    }

    async fn detect_display_info() -> Result<DisplayInfo> {
        // Try to get display info from X11/Wayland
        let display_info = if Self::is_wayland_session().await {
            Self::get_wayland_display_info().await
        } else {
            Self::get_x11_display_info().await
        };

        match display_info {
            Ok(info) => Ok(info),
            Err(_) => {
                warn!("Could not detect display info, using defaults");
                Ok(DisplayInfo {
                    refresh_rate_hz: 60,
                    resolution: (1920, 1080),
                    adaptive_sync_supported: false,
                    hdr_supported: false,
                    variable_refresh_rate: None,
                    display_name: "Unknown Display".to_string(),
                })
            }
        }
    }

    async fn is_wayland_session() -> bool {
        std::env::var("WAYLAND_DISPLAY").is_ok() ||
        std::env::var("XDG_SESSION_TYPE").map_or(false, |t| t == "wayland")
    }

    async fn get_wayland_display_info() -> Result<DisplayInfo> {
        // Use wlr-randr or similar Wayland tool
        let output = AsyncCommand::new("wlr-randr")
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                Self::parse_wayland_display_info(&stdout)
            }
            _ => Err(anyhow::anyhow!("Failed to get Wayland display info")),
        }
    }

    fn parse_wayland_display_info(output: &str) -> Result<DisplayInfo> {
        let mut refresh_rate = 60u32;
        let mut resolution = (1920u32, 1080u32);
        let mut display_name = "Wayland Display".to_string();

        for line in output.lines() {
            if line.contains("current") {
                // Parse resolution and refresh rate
                // Format: "1920x1080@144.000000Hz"
                if let Some(mode_part) = line.split_whitespace().find(|s| s.contains("@")) {
                    let parts: Vec<&str> = mode_part.split('@').collect();
                    if parts.len() == 2 {
                        // Parse resolution
                        if let Some(res_part) = parts[0].split('x').collect::<Vec<&str>>().get(0..2) {
                            if let (Ok(w), Ok(h)) = (res_part[0].parse(), res_part[1].parse()) {
                                resolution = (w, h);
                            }
                        }
                        // Parse refresh rate
                        if let Ok(rate) = parts[1].trim_end_matches("Hz").parse::<f64>() {
                            refresh_rate = rate as u32;
                        }
                    }
                }
            } else if line.starts_with(' ') && !line.trim().is_empty() {
                display_name = line.trim().to_string();
            }
        }

        Ok(DisplayInfo {
            refresh_rate_hz: refresh_rate,
            resolution,
            adaptive_sync_supported: true, // Assume modern Wayland supports it
            hdr_supported: false, // Conservative default
            variable_refresh_rate: Some((48, refresh_rate)), // Common VRR range
            display_name,
        })
    }

    async fn get_x11_display_info() -> Result<DisplayInfo> {
        // Use xrandr for X11 display info
        let output = AsyncCommand::new("xrandr")
            .args(&["--verbose"])
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                Self::parse_x11_display_info(&stdout)
            }
            _ => Err(anyhow::anyhow!("Failed to get X11 display info")),
        }
    }

    fn parse_x11_display_info(output: &str) -> Result<DisplayInfo> {
        let mut refresh_rate = 60u32;
        let mut resolution = (1920u32, 1080u32);
        let mut display_name = "X11 Display".to_string();
        let mut adaptive_sync_supported = false;

        let mut current_display: Option<String> = None;

        for line in output.lines() {
            if line.contains(" connected") && !line.contains("disconnected") {
                current_display = line.split_whitespace().next().map(|s| s.to_string());
                if let Some(ref name) = current_display {
                    display_name = name.clone();
                }
            }

            if line.contains("*") && line.contains("+") {
                // Parse current mode
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(mode) = parts.first() {
                    // Parse resolution
                    if let Some(res_parts) = mode.split('x').collect::<Vec<&str>>().get(0..2) {
                        if let (Ok(w), Ok(h)) = (res_parts[0].parse(), res_parts[1].parse()) {
                            resolution = (w, h);
                        }
                    }
                }

                // Parse refresh rate from the line
                for part in parts {
                    if part.contains("*") {
                        let rate_str = part.trim_matches('*').trim_matches('+');
                        if let Ok(rate) = rate_str.parse::<f64>() {
                            refresh_rate = rate as u32;
                        }
                    }
                }
            }

            // Check for VRR/Adaptive sync
            if line.contains("Variable refresh rate") || line.contains("VRR") ||
               line.contains("FreeSync") || line.contains("G-SYNC") {
                adaptive_sync_supported = true;
            }
        }

        Ok(DisplayInfo {
            refresh_rate_hz: refresh_rate,
            resolution,
            adaptive_sync_supported,
            hdr_supported: false, // Would need additional detection
            variable_refresh_rate: if adaptive_sync_supported { Some((48, refresh_rate)) } else { None },
            display_name,
        })
    }

    async fn start_frame_monitoring(&self) -> Result<()> {
        info!("📊 Starting frame monitoring");

        let frame_metrics = self.frame_metrics.clone();
        let pacing_controller = self.pacing_controller.clone();
        let vsync_controller = self.vsync_controller.clone();
        let strategy = self.config.pacing_strategy.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(100)); // 10Hz monitoring

            loop {
                interval.tick().await;

                // Update frame metrics (simulated - in real implementation would hook into graphics API)
                if let Err(e) = Self::update_frame_metrics(&frame_metrics, &pacing_controller).await {
                    debug!("Error updating frame metrics: {}", e);
                }

                // Apply adaptive strategies
                match strategy {
                    FramePacingStrategy::Predictive => {
                        if let Err(e) = Self::apply_predictive_pacing(&pacing_controller).await {
                            debug!("Error applying predictive pacing: {}", e);
                        }
                    }
                    FramePacingStrategy::MotionAdaptive => {
                        if let Err(e) = Self::apply_motion_adaptive_pacing(&pacing_controller).await {
                            debug!("Error applying motion adaptive pacing: {}", e);
                        }
                    }
                    _ => {}
                }

                // Adjust VSync if needed
                if let Err(e) = Self::adjust_adaptive_vsync(&vsync_controller, &frame_metrics).await {
                    debug!("Error adjusting adaptive VSync: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn update_frame_metrics(
        frame_metrics: &Arc<RwLock<FrameMetrics>>,
        pacing_controller: &Arc<PacingController>,
    ) -> Result<()> {
        let now = Instant::now();

        // Update frame history
        {
            let mut history = pacing_controller.frame_history.lock().await;
            history.push(now);

            // Keep only last 2 seconds
            history.retain(|&timestamp| now.duration_since(timestamp) < Duration::from_secs(2));
        }

        // Calculate current FPS
        let frame_count = {
            let history = pacing_controller.frame_history.lock().await;
            history.len()
        };

        let current_fps = if frame_count > 10 {
            frame_count as f64 / 2.0 // 2 second window
        } else {
            0.0
        };

        // Update metrics
        {
            let mut metrics = frame_metrics.write().await;
            metrics.current_fps = current_fps;
            metrics.frame_time_ms = if current_fps > 0.0 { 1000.0 / current_fps } else { 0.0 };
            metrics.last_updated = now;

            // Simulate frame time variance (would be measured in real implementation)
            metrics.frame_time_variance = if current_fps > 60.0 { 0.5 } else { 2.0 };
        }

        Ok(())
    }

    async fn apply_predictive_pacing(pacing_controller: &Arc<PacingController>) -> Result<()> {
        // Predictive frame pacing based on frame history
        let history = pacing_controller.frame_history.lock().await;

        if history.len() >= 3 {
            let recent_times: Vec<Duration> = history.windows(2)
                .map(|window| window[1].duration_since(window[0]))
                .collect();

            if recent_times.len() >= 2 {
                // Predict next frame time based on trend
                let avg_frame_time = recent_times.iter().sum::<Duration>() / recent_times.len() as u32;

                let mut prediction_buffer = pacing_controller.prediction_buffer.lock().await;
                prediction_buffer.push(avg_frame_time);

                if prediction_buffer.len() > 10 {
                    prediction_buffer.remove(0);
                }

                // Adjust target frame time based on prediction
                let predicted_frame_time = prediction_buffer.iter().sum::<Duration>() / prediction_buffer.len() as u32;
                *pacing_controller.target_frame_time.lock().await = predicted_frame_time;
            }
        }

        Ok(())
    }

    async fn apply_motion_adaptive_pacing(_pacing_controller: &Arc<PacingController>) -> Result<()> {
        // Motion-adaptive pacing would analyze game motion and adjust accordingly
        // This is a placeholder for more advanced motion detection
        Ok(())
    }

    async fn adjust_adaptive_vsync(
        vsync_controller: &Arc<VSyncController>,
        frame_metrics: &Arc<RwLock<FrameMetrics>>,
    ) -> Result<()> {
        let current_fps = {
            let metrics = frame_metrics.read().await;
            metrics.current_fps
        };

        let mut current_mode = vsync_controller.current_mode.write().await;

        match &*current_mode {
            VSyncMode::Adaptive => {
                // Switch VSync on/off based on performance
                if current_fps > vsync_controller.adaptive_threshold_fps {
                    // High FPS, enable VSync to prevent tearing
                    debug!("Adaptive VSync: Enabling (FPS: {:.1})", current_fps);
                } else {
                    // Low FPS, disable VSync to reduce stuttering
                    debug!("Adaptive VSync: Disabling (FPS: {:.1})", current_fps);
                }
            }
            _ => {} // Other modes are static
        }

        Ok(())
    }

    pub async fn configure_container_frame_pacing(&self, container_id: &str) -> Result<HashMap<String, String>> {
        info!("🎬 Configuring frame pacing for container: {}", container_id);

        let mut env_vars = HashMap::new();

        // Basic frame pacing configuration
        env_vars.insert("FRAME_PACING_ENABLED".to_string(), "1".to_string());
        env_vars.insert("TARGET_FPS".to_string(), self.config.target_fps.to_string());

        // VSync configuration
        let vsync_value = match self.config.vsync_mode {
            VSyncMode::Off => "0",
            VSyncMode::On => "1",
            VSyncMode::Adaptive => "2",
            VSyncMode::Enhanced => "3",
            VSyncMode::Fast => "4",
        };
        env_vars.insert("VSYNC_MODE".to_string(), vsync_value.to_string());

        // Frame pacing strategy
        let strategy_value = match self.config.pacing_strategy {
            FramePacingStrategy::None => "0",
            FramePacingStrategy::Consistent => "1",
            FramePacingStrategy::Predictive => "2",
            FramePacingStrategy::MotionAdaptive => "3",
            FramePacingStrategy::AIAssisted => "4",
        };
        env_vars.insert("FRAME_PACING_STRATEGY".to_string(), strategy_value.to_string());

        // Display information
        let display_info = self.display_info.read().await;
        env_vars.insert("DISPLAY_REFRESH_RATE".to_string(), display_info.refresh_rate_hz.to_string());
        env_vars.insert("DISPLAY_RESOLUTION_X".to_string(), display_info.resolution.0.to_string());
        env_vars.insert("DISPLAY_RESOLUTION_Y".to_string(), display_info.resolution.1.to_string());

        // Adaptive sync configuration
        if self.config.adaptive_sync && display_info.adaptive_sync_supported {
            env_vars.insert("ADAPTIVE_SYNC_ENABLED".to_string(), "1".to_string());
            if let Some((min_rate, max_rate)) = display_info.variable_refresh_rate {
                env_vars.insert("VRR_MIN_RATE".to_string(), min_rate.to_string());
                env_vars.insert("VRR_MAX_RATE".to_string(), max_rate.to_string());
            }
        }

        // Low latency optimizations
        match self.config.low_latency_priority {
            LatencyPriority::UltraLowLatency => {
                env_vars.insert("LOW_LATENCY_MODE".to_string(), "2".to_string());
                env_vars.insert("PREEMPTION_PRIORITY".to_string(), "HIGH".to_string());
            }
            LatencyPriority::Balanced => {
                env_vars.insert("LOW_LATENCY_MODE".to_string(), "1".to_string());
            }
            LatencyPriority::Quality => {
                env_vars.insert("QUALITY_PRIORITY".to_string(), "1".to_string());
            }
        }

        // Rate limiting method
        let rate_limit_value = match self.config.rate_limit_method {
            RateLimitMethod::GPU => "gpu",
            RateLimitMethod::CPU => "cpu",
            RateLimitMethod::Driver => "driver",
            RateLimitMethod::Hybrid => "hybrid",
        };
        env_vars.insert("RATE_LIMIT_METHOD".to_string(), rate_limit_value.to_string());

        // Frame interpolation
        if self.config.frame_interpolation {
            env_vars.insert("FRAME_INTERPOLATION".to_string(), "1".to_string());
            warn!("Frame interpolation enabled - may increase input latency");
        }

        info!("✅ Frame pacing configured with {} parameters", env_vars.len());
        Ok(env_vars)
    }

    pub async fn get_frame_metrics(&self) -> FrameMetrics {
        self.frame_metrics.read().await.clone()
    }

    pub async fn get_display_info(&self) -> DisplayInfo {
        self.display_info.read().await.clone()
    }

    pub async fn optimize_for_competitive_gaming(&mut self) -> Result<()> {
        info!("🏆 Optimizing frame pacing for competitive gaming");

        // Ultra-low latency settings
        self.config.vsync_mode = VSyncMode::Off; // Disable VSync for minimum latency
        self.config.pacing_strategy = FramePacingStrategy::Consistent; // Predictable frame times
        self.config.low_latency_priority = LatencyPriority::UltraLowLatency;
        self.config.frame_interpolation = false; // Disable to reduce latency
        self.config.rate_limit_method = RateLimitMethod::GPU; // GPU limiting is faster

        // Set high target FPS if display supports it
        let display_info = self.display_info.read().await;
        if display_info.refresh_rate_hz >= 144 {
            self.config.target_fps = display_info.refresh_rate_hz;
        } else {
            self.config.target_fps = 144; // High fps even on lower refresh displays
        }

        // Update VSync controller
        *self.vsync_controller.current_mode.write().await = self.config.vsync_mode.clone();

        info!("✅ Competitive gaming optimizations applied");
        info!("   Target FPS: {}", self.config.target_fps);
        info!("   VSync: Disabled");
        info!("   Latency Priority: Ultra Low");

        Ok(())
    }

    pub async fn verify_frame_pacing_performance(&self, container_id: &str) -> Result<FramePacingPerformanceReport> {
        info!("📊 Verifying frame pacing performance for container: {}", container_id);

        let metrics = self.get_frame_metrics().await;
        let display_info = self.get_display_info().await;

        let target_achieved = if self.config.target_fps > 0 {
            metrics.current_fps >= self.config.target_fps as f64 * 0.95 // 95% of target
        } else {
            true // Unlimited FPS
        };

        let stuttering_acceptable = metrics.stutters_per_second < 1.0; // Less than 1 stutter per second
        let frame_time_consistent = metrics.frame_time_variance < 2.0; // Less than 2ms variance

        let overall_performance = if target_achieved && stuttering_acceptable && frame_time_consistent {
            "Excellent"
        } else if target_achieved && stuttering_acceptable {
            "Good"
        } else if target_achieved {
            "Fair"
        } else {
            "Poor"
        };

        let report = FramePacingPerformanceReport {
            current_fps: metrics.current_fps,
            target_fps: self.config.target_fps,
            frame_time_ms: metrics.frame_time_ms,
            frame_time_variance: metrics.frame_time_variance,
            stutters_per_second: metrics.stutters_per_second,
            input_lag_ms: metrics.input_lag_ms,
            vsync_active: metrics.vsync_active,
            adaptive_sync_active: metrics.adaptive_sync_active,
            target_achieved,
            overall_performance: overall_performance.to_string(),
        };

        info!("📈 Frame Pacing Performance Report:");
        info!("   Current FPS: {:.1} (target: {})", report.current_fps, report.target_fps);
        info!("   Frame Time: {:.2}ms (variance: {:.2}ms)", report.frame_time_ms, report.frame_time_variance);
        info!("   Stutters: {:.2}/sec", report.stutters_per_second);
        info!("   Input Lag: {:.2}ms", report.input_lag_ms);
        info!("   Overall Performance: {}", report.overall_performance);

        Ok(report)
    }
}

#[derive(Debug, Clone)]
pub struct FramePacingPerformanceReport {
    pub current_fps: f64,
    pub target_fps: u32,
    pub frame_time_ms: f64,
    pub frame_time_variance: f64,
    pub stutters_per_second: f64,
    pub input_lag_ms: f64,
    pub vsync_active: bool,
    pub adaptive_sync_active: bool,
    pub target_achieved: bool,
    pub overall_performance: String,
}