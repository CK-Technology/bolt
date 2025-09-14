use anyhow::{Result, Context};
use tracing::{info, debug, warn, error};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod compositor;
pub mod display;
pub mod input;
pub mod output;
pub mod protocols;

use compositor::BoltCompositor;
use display::DisplayManager;
use input::InputManager;
use output::OutputManager;

#[derive(Debug, Clone)]
pub struct WaylandGamingConfig {
    pub display_name: String,
    pub socket_path: PathBuf,
    pub enable_vsync: bool,
    pub target_fps: Option<u32>,
    pub enable_vrr: bool, // Variable Refresh Rate
    pub enable_hdr: bool,
    pub enable_low_latency: bool,
    pub gpu_passthrough: bool,
    pub multi_monitor: bool,
    pub compositor_threads: u32,
}

impl Default for WaylandGamingConfig {
    fn default() -> Self {
        Self {
            display_name: "bolt-gaming".to_string(),
            socket_path: PathBuf::from("/tmp/bolt-wayland"),
            enable_vsync: false, // Disabled for gaming
            target_fps: Some(144),
            enable_vrr: true,
            enable_hdr: true,
            enable_low_latency: true,
            gpu_passthrough: true,
            multi_monitor: true,
            compositor_threads: 4,
        }
    }
}

#[derive(Debug)]
pub struct WaylandGamingSession {
    pub id: String,
    pub config: WaylandGamingConfig,
    pub compositor: Arc<Mutex<BoltCompositor>>,
    pub display_manager: Arc<Mutex<DisplayManager>>,
    pub input_manager: Arc<Mutex<InputManager>>,
    pub output_manager: Arc<Mutex<OutputManager>>,
    pub game_process_id: Option<u32>,
    pub active: bool,
}

impl WaylandGamingSession {
    pub async fn new(config: WaylandGamingConfig) -> Result<Self> {
        info!("🎮 Creating Wayland gaming session: {}", config.display_name);

        let compositor = Arc::new(Mutex::new(BoltCompositor::new(&config).await?));
        let display_manager = Arc::new(Mutex::new(DisplayManager::new(&config).await?));
        let input_manager = Arc::new(Mutex::new(InputManager::new(&config).await?));
        let output_manager = Arc::new(Mutex::new(OutputManager::new(&config).await?));

        let session = Self {
            id: uuid::Uuid::new_v4().to_string(),
            config,
            compositor,
            display_manager,
            input_manager,
            output_manager,
            game_process_id: None,
            active: false,
        };

        Ok(session)
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting Wayland gaming session: {}", self.id);

        // Initialize display server
        {
            let mut display = self.display_manager.lock().await;
            display.start().await?;
        }

        // Initialize compositor with gaming optimizations
        {
            let mut compositor = self.compositor.lock().await;
            compositor.start_with_gaming_optimizations().await?;
        }

        // Setup input handling
        {
            let mut input = self.input_manager.lock().await;
            input.setup_gaming_input().await?;
        }

        // Configure outputs for gaming
        {
            let mut output = self.output_manager.lock().await;
            output.setup_gaming_outputs().await?;
        }

        self.active = true;
        info!("✅ Wayland gaming session started successfully");
        Ok(())
    }

    pub async fn attach_game_process(&mut self, pid: u32) -> Result<()> {
        info!("🎯 Attaching game process {} to Wayland session", pid);

        self.game_process_id = Some(pid);

        // Apply gaming-specific optimizations
        self.apply_game_optimizations().await?;

        // Setup GPU passthrough for the game
        if self.config.gpu_passthrough {
            self.setup_gpu_passthrough_for_game(pid).await?;
        }

        Ok(())
    }

    async fn apply_game_optimizations(&self) -> Result<()> {
        debug!("⚡ Applying gaming optimizations to Wayland session");

        // Configure compositor for low latency
        if self.config.enable_low_latency {
            let mut compositor = self.compositor.lock().await;
            compositor.enable_low_latency_mode().await?;
        }

        // Setup VRR if enabled
        if self.config.enable_vrr {
            let mut output = self.output_manager.lock().await;
            output.enable_variable_refresh_rate().await?;
        }

        // Configure target FPS
        if let Some(fps) = self.config.target_fps {
            let mut compositor = self.compositor.lock().await;
            compositor.set_target_fps(fps).await?;
        }

        Ok(())
    }

    async fn setup_gpu_passthrough_for_game(&self, _pid: u32) -> Result<()> {
        debug!("🎮 Setting up GPU passthrough for game process");

        // This would integrate with our existing GPU managers
        // to ensure the game has direct GPU access through Wayland

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        info!("🛑 Stopping Wayland gaming session: {}", self.id);

        // Stop compositor
        {
            let mut compositor = self.compositor.lock().await;
            compositor.stop().await?;
        }

        // Stop display server
        {
            let mut display = self.display_manager.lock().await;
            display.stop().await?;
        }

        self.active = false;
        info!("✅ Wayland gaming session stopped");
        Ok(())
    }

    pub async fn get_performance_metrics(&self) -> Result<GamingPerformanceMetrics> {
        let compositor = self.compositor.lock().await;
        compositor.get_performance_metrics().await
    }
}

#[derive(Debug, Clone)]
pub struct GamingPerformanceMetrics {
    pub current_fps: f64,
    pub frame_time_ms: f64,
    pub input_latency_ms: f64,
    pub gpu_utilization: f64,
    pub memory_usage_mb: u64,
    pub dropped_frames: u64,
    pub vsync_enabled: bool,
}

pub struct WaylandGamingManager {
    sessions: HashMap<String, WaylandGamingSession>,
}

impl WaylandGamingManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub async fn create_gaming_session(&mut self, config: WaylandGamingConfig) -> Result<String> {
        let session = WaylandGamingSession::new(config).await?;
        let session_id = session.id.clone();

        self.sessions.insert(session_id.clone(), session);

        info!("🎮 Gaming session created: {}", session_id);
        Ok(session_id)
    }

    pub async fn start_session(&mut self, session_id: &str) -> Result<()> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.start().await?;
        } else {
            return Err(anyhow::anyhow!("Gaming session not found: {}", session_id));
        }
        Ok(())
    }

    pub async fn stop_session(&mut self, session_id: &str) -> Result<()> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.stop().await?;
        } else {
            return Err(anyhow::anyhow!("Gaming session not found: {}", session_id));
        }
        Ok(())
    }

    pub async fn attach_game_to_session(&mut self, session_id: &str, game_pid: u32) -> Result<()> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.attach_game_process(game_pid).await?;
        } else {
            return Err(anyhow::anyhow!("Gaming session not found: {}", session_id));
        }
        Ok(())
    }

    pub async fn get_session_metrics(&self, session_id: &str) -> Result<GamingPerformanceMetrics> {
        if let Some(session) = self.sessions.get(session_id) {
            session.get_performance_metrics().await
        } else {
            Err(anyhow::anyhow!("Gaming session not found: {}", session_id))
        }
    }

    pub fn list_active_sessions(&self) -> Vec<&str> {
        self.sessions.keys().map(|k| k.as_str()).collect()
    }
}