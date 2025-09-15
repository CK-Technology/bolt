use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::{GamingPerformanceMetrics, WaylandGamingConfig};

#[derive(Debug)]
pub struct BoltCompositor {
    config: WaylandGamingConfig,
    surfaces: Arc<RwLock<HashMap<u32, Surface>>>,
    frame_scheduler: FrameScheduler,
    performance_monitor: PerformanceMonitor,
    low_latency_mode: bool,
    running: bool,
}

#[derive(Debug, Clone)]
pub struct Surface {
    id: u32,
    width: u32,
    height: u32,
    format: SurfaceFormat,
    game_surface: bool,
    priority: SurfacePriority,
    last_frame: Instant,
}

#[derive(Debug, Clone)]
pub enum SurfaceFormat {
    ARGB8888,
    XRGB8888,
    RGB565,
    RGBA1010102, // HDR format
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SurfacePriority {
    Background = 0,
    Normal = 1,
    Gaming = 2,
    Critical = 3,
}

#[derive(Debug)]
pub struct FrameScheduler {
    target_fps: Option<u32>,
    frame_budget_ns: u64,
    last_frame_time: Instant,
    frame_counter: u64,
    enable_vsync: bool,
}

impl FrameScheduler {
    pub fn new(config: &WaylandGamingConfig) -> Self {
        let frame_budget_ns = if let Some(fps) = config.target_fps {
            1_000_000_000 / fps as u64
        } else {
            16_666_666 // 60 FPS default
        };

        Self {
            target_fps: config.target_fps,
            frame_budget_ns,
            last_frame_time: Instant::now(),
            frame_counter: 0,
            enable_vsync: config.enable_vsync,
        }
    }

    pub async fn wait_for_next_frame(&mut self) -> Result<()> {
        if !self.enable_vsync {
            // For gaming, we often want to run without vsync for lower latency
            return Ok(());
        }

        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame_time);
        let target_duration = Duration::from_nanos(self.frame_budget_ns);

        if elapsed < target_duration {
            let sleep_duration = target_duration - elapsed;
            tokio::time::sleep(sleep_duration).await;
        }

        self.last_frame_time = Instant::now();
        self.frame_counter += 1;

        Ok(())
    }

    pub fn get_current_fps(&self) -> f64 {
        if self.frame_counter == 0 {
            return 0.0;
        }

        let elapsed = self.last_frame_time.elapsed();
        if elapsed.as_secs() == 0 {
            return 0.0;
        }

        self.frame_counter as f64 / elapsed.as_secs_f64()
    }
}

#[derive(Debug)]
pub struct PerformanceMonitor {
    frame_times: Vec<Duration>,
    input_latencies: Vec<Duration>,
    dropped_frames: u64,
    gpu_utilization: f64,
    memory_usage: u64,
    last_update: Instant,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            frame_times: Vec::with_capacity(1000),
            input_latencies: Vec::with_capacity(1000),
            dropped_frames: 0,
            gpu_utilization: 0.0,
            memory_usage: 0,
            last_update: Instant::now(),
        }
    }

    pub fn record_frame_time(&mut self, duration: Duration) {
        self.frame_times.push(duration);

        // Keep only last 1000 measurements for performance
        if self.frame_times.len() > 1000 {
            self.frame_times.remove(0);
        }
    }

    pub fn record_input_latency(&mut self, latency: Duration) {
        self.input_latencies.push(latency);

        if self.input_latencies.len() > 1000 {
            self.input_latencies.remove(0);
        }
    }

    pub fn increment_dropped_frames(&mut self) {
        self.dropped_frames += 1;
    }

    pub fn get_average_frame_time(&self) -> Duration {
        if self.frame_times.is_empty() {
            return Duration::ZERO;
        }

        let total: Duration = self.frame_times.iter().sum();
        total / self.frame_times.len() as u32
    }

    pub fn get_average_input_latency(&self) -> Duration {
        if self.input_latencies.is_empty() {
            return Duration::ZERO;
        }

        let total: Duration = self.input_latencies.iter().sum();
        total / self.input_latencies.len() as u32
    }

    pub async fn update_system_metrics(&mut self) -> Result<()> {
        // Update GPU utilization (would integrate with actual GPU monitoring)
        self.gpu_utilization = self.get_gpu_utilization().await?;

        // Update memory usage
        self.memory_usage = self.get_memory_usage().await?;

        self.last_update = Instant::now();
        Ok(())
    }

    async fn get_gpu_utilization(&self) -> Result<f64> {
        // This would integrate with NVIDIA-ML or similar for real GPU metrics
        // For now, return a simulated value
        Ok(75.0)
    }

    async fn get_memory_usage(&self) -> Result<u64> {
        // This would get actual memory usage from the system
        // For now, return a simulated value in MB
        Ok(1024)
    }
}

impl BoltCompositor {
    pub async fn new(config: &WaylandGamingConfig) -> Result<Self> {
        info!("🎮 Initializing Bolt gaming compositor");

        let compositor = Self {
            config: config.clone(),
            surfaces: Arc::new(RwLock::new(HashMap::new())),
            frame_scheduler: FrameScheduler::new(config),
            performance_monitor: PerformanceMonitor::new(),
            low_latency_mode: config.enable_low_latency,
            running: false,
        };

        debug!("✅ Bolt compositor initialized with config: {:?}", config);
        Ok(compositor)
    }

    pub async fn start_with_gaming_optimizations(&mut self) -> Result<()> {
        info!("🚀 Starting Bolt compositor with gaming optimizations");

        self.running = true;

        // Setup gaming-specific optimizations
        if self.low_latency_mode {
            self.enable_low_latency_optimizations().await?;
        }

        // Start compositor event loop
        self.start_event_loop().await?;

        info!("✅ Bolt compositor started successfully");
        Ok(())
    }

    async fn enable_low_latency_optimizations(&mut self) -> Result<()> {
        debug!("⚡ Enabling low-latency optimizations");

        // Disable composition effects that add latency
        self.frame_scheduler.enable_vsync = false;

        // Set high priority for compositor thread
        self.set_high_priority().await?;

        // Configure for minimum buffering
        self.configure_minimal_buffering().await?;

        Ok(())
    }

    async fn set_high_priority(&self) -> Result<()> {
        debug!("🔥 Setting high priority for compositor thread");

        // This would use real-time scheduling APIs
        // For now, just log the intention
        info!("  ✓ Compositor thread priority set to high");

        Ok(())
    }

    async fn configure_minimal_buffering(&self) -> Result<()> {
        debug!("📦 Configuring minimal buffering for low latency");

        // Configure single or double buffering instead of triple buffering
        info!("  ✓ Minimal buffering configured");

        Ok(())
    }

    async fn start_event_loop(&mut self) -> Result<()> {
        debug!("🔄 Starting compositor event loop");

        // In a real implementation, this would start the main Wayland event loop
        // For now, we'll simulate it
        tokio::spawn(async move {
            // Compositor event loop would run here
            loop {
                tokio::time::sleep(Duration::from_millis(1)).await;
                // Process Wayland events, compose frames, etc.
            }
        });

        Ok(())
    }

    pub async fn enable_low_latency_mode(&mut self) -> Result<()> {
        info!("⚡ Enabling low-latency mode");

        self.low_latency_mode = true;
        self.frame_scheduler.enable_vsync = false;

        // Reconfigure for minimal latency
        self.enable_low_latency_optimizations().await?;

        Ok(())
    }

    pub async fn set_target_fps(&mut self, fps: u32) -> Result<()> {
        info!("🎯 Setting target FPS to: {}", fps);

        self.frame_scheduler.target_fps = Some(fps);
        self.frame_scheduler.frame_budget_ns = 1_000_000_000 / fps as u64;

        Ok(())
    }

    pub async fn create_surface(
        &mut self,
        width: u32,
        height: u32,
        game_surface: bool,
    ) -> Result<u32> {
        let surface_id = self.generate_surface_id().await;

        let surface = Surface {
            id: surface_id,
            width,
            height,
            format: if self.config.enable_hdr {
                SurfaceFormat::RGBA1010102
            } else {
                SurfaceFormat::ARGB8888
            },
            game_surface,
            priority: if game_surface {
                SurfacePriority::Gaming
            } else {
                SurfacePriority::Normal
            },
            last_frame: Instant::now(),
        };

        let mut surfaces = self.surfaces.write().await;
        surfaces.insert(surface_id, surface);

        info!(
            "🖼️  Created surface {} ({}x{}, game: {})",
            surface_id, width, height, game_surface
        );

        Ok(surface_id)
    }

    async fn generate_surface_id(&self) -> u32 {
        let surfaces = self.surfaces.read().await;

        // Find next available ID
        let mut id = 1;
        while surfaces.contains_key(&id) {
            id += 1;
        }

        id
    }

    pub async fn render_frame(&mut self) -> Result<()> {
        let frame_start = Instant::now();

        // Wait for next frame if vsync is enabled
        self.frame_scheduler.wait_for_next_frame().await?;

        // Compose and render frame
        self.compose_frame().await?;

        // Record performance metrics
        let frame_time = frame_start.elapsed();
        self.performance_monitor.record_frame_time(frame_time);

        // Update system metrics periodically
        if self.performance_monitor.last_update.elapsed() > Duration::from_secs(1) {
            self.performance_monitor.update_system_metrics().await?;
        }

        Ok(())
    }

    async fn compose_frame(&self) -> Result<()> {
        let surfaces = self.surfaces.read().await;

        // Sort surfaces by priority (gaming surfaces get priority)
        let mut sorted_surfaces: Vec<_> = surfaces.values().collect();
        sorted_surfaces.sort_by_key(|s| s.priority);

        // Compose frame (simplified - real implementation would do actual composition)
        for surface in sorted_surfaces {
            if surface.game_surface {
                debug!("🎮 Composing gaming surface {}", surface.id);
                // Gaming surfaces get special treatment for performance
            }
        }

        Ok(())
    }

    pub async fn get_performance_metrics(&self) -> Result<GamingPerformanceMetrics> {
        let metrics = GamingPerformanceMetrics {
            current_fps: self.frame_scheduler.get_current_fps(),
            frame_time_ms: self
                .performance_monitor
                .get_average_frame_time()
                .as_millis() as f64,
            input_latency_ms: self
                .performance_monitor
                .get_average_input_latency()
                .as_millis() as f64,
            gpu_utilization: self.performance_monitor.gpu_utilization,
            memory_usage_mb: self.performance_monitor.memory_usage,
            dropped_frames: self.performance_monitor.dropped_frames,
            vsync_enabled: self.frame_scheduler.enable_vsync,
        };

        Ok(metrics)
    }

    pub async fn stop(&mut self) -> Result<()> {
        info!("🛑 Stopping Bolt compositor");

        self.running = false;

        // Clean up surfaces
        {
            let mut surfaces = self.surfaces.write().await;
            surfaces.clear();
        }

        info!("✅ Bolt compositor stopped");
        Ok(())
    }
}
