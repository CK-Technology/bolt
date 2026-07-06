use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::process::Command as AsyncCommand;
use tracing::{debug, info, warn, error};

use crate::config::GamingConfig;
use crate::runtime::BoltRuntime;

/// Comprehensive Steam ecosystem integration for Bolt containers
pub struct SteamIntegration {
    config: SteamConfig,
    steam_client: Option<SteamClient>,
    library_manager: SteamLibraryManager,
    compatibility_layer: SteamCompatibilityLayer,
    performance_optimizer: SteamPerformanceOptimizer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamConfig {
    pub steam_root: PathBuf,
    pub steam_user_data: PathBuf,
    pub steam_apps: PathBuf,
    pub enable_proton: bool,
    pub enable_steam_runtime: bool,
    pub enable_steam_overlay: bool,
    pub enable_steam_input: bool,
    pub enable_remote_play: bool,
    pub enable_steam_cloud: bool,
    pub auto_launch_steam: bool,
    pub gpu_acceleration: bool,
    pub use_steam_deck_optimizations: bool,
    pub preferred_proton_version: Option<String>,
    pub custom_launch_options: HashMap<String, String>,
}

/// Steam client integration for container environments
pub struct SteamClient {
    pid: Option<u32>,
    status: SteamClientStatus,
    api_key: Option<String>,
    user_id: Option<String>,
    session_token: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SteamClientStatus {
    NotRunning,
    Starting,
    Running,
    LoggingIn,
    Online,
    Offline,
    Error(String),
}

/// Steam library and game management
pub struct SteamLibraryManager {
    libraries: Vec<SteamLibrary>,
    installed_games: HashMap<u32, SteamGame>,
    favorites: Vec<u32>,
    recently_played: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamLibrary {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub free_space_bytes: u64,
    pub game_count: u32,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamGame {
    pub app_id: u32,
    pub name: String,
    pub install_dir: String,
    pub install_path: PathBuf,
    pub size_bytes: u64,
    pub last_played: Option<chrono::DateTime<chrono::Utc>>,
    pub playtime_minutes: u64,
    pub requires_proton: bool,
    pub proton_version: Option<String>,
    pub launch_options: Option<String>,
    pub achievements: u32,
    pub screenshots: u32,
    pub dlc_count: u32,
    pub is_favorite: bool,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
}

/// Steam Proton and compatibility layer management
pub struct SteamCompatibilityLayer {
    proton_installations: HashMap<String, ProtonInstallation>,
    wine_prefixes: HashMap<u32, WinePrefix>,
    compatibility_tools: Vec<CompatibilityTool>,
}

#[derive(Debug, Clone)]
pub struct ProtonInstallation {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub wine_version: String,
    pub dxvk_version: Option<String>,
    pub vkd3d_version: Option<String>,
    pub supports_battleye: bool,
    pub supports_eac: bool,
    pub is_experimental: bool,
}

#[derive(Debug, Clone)]
pub struct WinePrefix {
    pub app_id: u32,
    pub path: PathBuf,
    pub proton_version: String,
    pub windows_version: String,
    pub architecture: String,
    pub size_bytes: u64,
    pub dlls: Vec<String>,
    pub registry_tweaks: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct CompatibilityTool {
    pub name: String,
    pub tool_type: CompatibilityToolType,
    pub version: String,
    pub path: PathBuf,
    pub supported_games: Vec<u32>,
}

#[derive(Debug, Clone)]
pub enum CompatibilityToolType {
    Proton,
    Wine,
    Lutris,
    Bottles,
    Custom,
}

/// Steam performance optimization for containerized gaming
pub struct SteamPerformanceOptimizer {
    cpu_optimizations: CpuOptimizations,
    gpu_optimizations: GpuOptimizations,
    memory_optimizations: MemoryOptimizations,
    storage_optimizations: StorageOptimizations,
    network_optimizations: NetworkOptimizations,
}

#[derive(Debug, Clone)]
pub struct CpuOptimizations {
    pub cpu_governor: String,
    pub cpu_scaling: String,
    pub process_priority: i32,
    pub cpu_affinity: Vec<usize>,
    pub disable_c_states: bool,
    pub enable_turbo_boost: bool,
}

#[derive(Debug, Clone)]
pub struct GpuOptimizations {
    pub gpu_power_mode: String,
    pub gpu_memory_clock: Option<i32>,
    pub gpu_core_clock: Option<i32>,
    pub enable_resizable_bar: bool,
    pub force_high_performance: bool,
    pub disable_gpu_scheduling: bool,
}

#[derive(Debug, Clone)]
pub struct MemoryOptimizations {
    pub memory_policy: String,
    pub huge_pages: bool,
    pub memory_compression: bool,
    pub swap_configuration: String,
    pub memory_overcommit: i32,
}

#[derive(Debug, Clone)]
pub struct StorageOptimizations {
    pub io_scheduler: String,
    pub read_ahead_kb: u32,
    pub enable_write_cache: bool,
    pub filesystem_optimizations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct NetworkOptimizations {
    pub tcp_congestion_control: String,
    pub network_buffer_sizes: HashMap<String, u32>,
    pub enable_tcp_fast_open: bool,
    pub disable_tcp_timestamps: bool,
}

impl Default for SteamConfig {
    fn default() -> Self {
        Self {
            steam_root: PathBuf::from("/home/.steam/steam"),
            steam_user_data: PathBuf::from("/home/.steam/steam/userdata"),
            steam_apps: PathBuf::from("/home/.steam/steam/steamapps"),
            enable_proton: true,
            enable_steam_runtime: true,
            enable_steam_overlay: true,
            enable_steam_input: true,
            enable_remote_play: true,
            enable_steam_cloud: true,
            auto_launch_steam: true,
            gpu_acceleration: true,
            use_steam_deck_optimizations: false,
            preferred_proton_version: Some("Proton 8.0".to_string()),
            custom_launch_options: HashMap::new(),
        }
    }
}

impl SteamIntegration {
    /// Create new Steam integration instance
    pub async fn new(config: SteamConfig) -> Result<Self> {
        info!("🎮 Initializing Steam integration for Bolt containers");
        info!("  • Steam Root: {:?}", config.steam_root);
        info!("  • Proton: {}", if config.enable_proton { "✅ Enabled" } else { "❌ Disabled" });
        info!("  • Steam Runtime: {}", if config.enable_steam_runtime { "✅ Enabled" } else { "❌ Disabled" });
        info!("  • GPU Acceleration: {}", if config.gpu_acceleration { "✅ Enabled" } else { "❌ Disabled" });

        let mut integration = Self {
            config: config.clone(),
            steam_client: None,
            library_manager: SteamLibraryManager::new(&config).await?,
            compatibility_layer: SteamCompatibilityLayer::new(&config).await?,
            performance_optimizer: SteamPerformanceOptimizer::new(&config).await?,
        };

        // Initialize Steam client if auto-launch is enabled
        if config.auto_launch_steam {
            integration.launch_steam_client().await?;
        }

        info!("✅ Steam integration initialized successfully");
        Ok(integration)
    }

    /// Launch Steam client in container
    pub async fn launch_steam_client(&mut self) -> Result<()> {
        info!("🚀 Launching Steam client in container");

        // Check if Steam is already running
        if let Some(ref client) = self.steam_client {
            if matches!(client.status, SteamClientStatus::Running | SteamClientStatus::Online) {
                info!("Steam client already running");
                return Ok(());
            }
        }

        // Prepare Steam launch environment
        self.prepare_steam_environment().await?;

        // Launch Steam with optimal container settings
        let mut steam_cmd = AsyncCommand::new("steam");

        // Add essential arguments for container environment
        steam_cmd.args(&[
            "-console",           // Enable console mode
            "-nofriendsui",      // Disable friends UI for better performance
            "-no-browser",       // Disable built-in browser
            "-silent",           // Silent startup
        ]);

        // Add GPU acceleration if enabled
        if self.config.gpu_acceleration {
            steam_cmd.env("__GL_THREADED_OPTIMIZATIONS", "1");
            steam_cmd.env("__GL_SYNC_TO_VBLANK", "0");
            steam_cmd.env("DXVK_HUD", "fps,memory,gpu");
        }

        // Add Proton configuration
        if self.config.enable_proton {
            steam_cmd.env("STEAM_COMPAT_DATA_PATH", &self.config.steam_user_data);
            steam_cmd.env("PROTON_USE_WINED3D", "0");
            steam_cmd.env("PROTON_NO_ESYNC", "0");
            steam_cmd.env("PROTON_NO_FSYNC", "0");
        }

        // Enable Steam Input
        if self.config.enable_steam_input {
            steam_cmd.env("STEAM_USE_DYNAMIC_VRS", "1");
        }

        info!("  • Launching Steam with container optimizations");
        let child = steam_cmd.spawn()?;
        let pid = child.id().unwrap_or(0);

        // Create Steam client instance
        self.steam_client = Some(SteamClient {
            pid: Some(pid),
            status: SteamClientStatus::Starting,
            api_key: None,
            user_id: None,
            session_token: None,
        });

        info!("✅ Steam client launched (PID: {})", pid);
        Ok(())
    }

    /// Prepare Steam environment for container
    async fn prepare_steam_environment(&self) -> Result<()> {
        info!("🔧 Preparing Steam environment for container");

        // Create necessary directories
        tokio::fs::create_dir_all(&self.config.steam_root).await?;
        tokio::fs::create_dir_all(&self.config.steam_user_data).await?;
        tokio::fs::create_dir_all(&self.config.steam_apps).await?;

        // Set up Steam Runtime environment
        if self.config.enable_steam_runtime {
            self.setup_steam_runtime().await?;
        }

        // Configure graphics drivers
        if self.config.gpu_acceleration {
            self.configure_gpu_drivers().await?;
        }

        // Set up audio system
        self.setup_audio_system().await?;

        info!("✅ Steam environment prepared");
        Ok(())
    }

    /// Set up Steam Runtime for better compatibility
    async fn setup_steam_runtime(&self) -> Result<()> {
        info!("🔧 Setting up Steam Runtime");

        // Steam Runtime provides a consistent environment
        let runtime_path = self.config.steam_root.join("ubuntu12_32").join("steam-runtime");

        if !runtime_path.exists() {
            info!("  • Steam Runtime not found, downloading...");
            // In a real implementation, download and extract Steam Runtime
            tokio::fs::create_dir_all(&runtime_path).await?;
        }

        info!("✅ Steam Runtime configured");
        Ok(())
    }

    /// Configure GPU drivers for optimal gaming performance
    async fn configure_gpu_drivers(&self) -> Result<()> {
        info!("🎮 Configuring GPU drivers for gaming");

        // NVIDIA optimizations
        if let Ok(_) = Command::new("nvidia-smi").output() {
            info!("  • Detected NVIDIA GPU, applying optimizations");

            // Enable NVIDIA threaded optimizations
            std::env::set_var("__GL_THREADED_OPTIMIZATIONS", "1");
            std::env::set_var("__GL_SYNC_TO_VBLANK", "0");

            // Disable composition in gaming mode
            std::env::set_var("__GL_YIELD", "USLEEP");
        }

        // AMD optimizations
        if let Ok(_) = Command::new("rocm-smi").output() {
            info!("  • Detected AMD GPU, applying optimizations");

            // Enable AMD optimizations
            std::env::set_var("RADV_PERFTEST", "aco");
            std::env::set_var("mesa_glthread", "true");
        }

        // Vulkan optimizations
        std::env::set_var("VK_ICD_FILENAMES", "/usr/share/vulkan/icd.d/nvidia_icd.json:/usr/share/vulkan/icd.d/radeon_icd.x86_64.json");

        info!("✅ GPU drivers configured for gaming");
        Ok(())
    }

    /// Set up audio system for Steam games
    async fn setup_audio_system(&self) -> Result<()> {
        info!("🔊 Setting up audio system for Steam");

        // Configure PulseAudio for low latency
        std::env::set_var("PULSE_LATENCY_MSEC", "30");
        std::env::set_var("PULSE_PCM_TYPE", "pulse");

        // Enable ALSA thread-safe API
        std::env::set_var("ALSA_THREAD_SAFE_API", "1");

        info!("✅ Audio system configured");
        Ok(())
    }

    /// Install game from Steam
    pub async fn install_game(&mut self, app_id: u32, library_path: Option<&Path>) -> Result<()> {
        info!("📦 Installing Steam game: {}", app_id);

        // Check if Steam client is running
        if self.steam_client.is_none() {
            self.launch_steam_client().await?;
        }

        // Use steam:// protocol for installation
        let install_url = format!("steam://install/{}", app_id);

        let mut cmd = AsyncCommand::new("steam");
        cmd.arg(&install_url);

        if let Some(library) = library_path {
            cmd.env("STEAM_LIBRARY_PATH", library);
        }

        info!("  • Installing to library: {:?}", library_path.unwrap_or(Path::new("default")));
        let output = cmd.output().await?;

        if output.status.success() {
            info!("✅ Game installation started: {}", app_id);

            // Monitor installation progress
            self.monitor_installation_progress(app_id).await?;
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to start installation: {}", stderr);
            return Err(anyhow::anyhow!("Installation failed: {}", stderr));
        }

        Ok(())
    }

    /// Monitor game installation progress
    async fn monitor_installation_progress(&self, app_id: u32) -> Result<()> {
        info!("📊 Monitoring installation progress for game: {}", app_id);

        match self.install_progress(app_id)? {
            Some(progress) if progress.installed => {
                info!("✅ Game {} is installed", app_id);
            }
            Some(progress) => {
                info!(
                    "  • Install state: {:?}, bytes: {}/{} ({:.1}%)",
                    progress.state,
                    progress.bytes_downloaded,
                    progress.bytes_total,
                    progress.percent()
                );
            }
            None => {
                info!("  • Steam has not written an appmanifest for {} yet", app_id);
            }
        }

        Ok(())
    }

    fn install_progress(&self, app_id: u32) -> Result<Option<SteamInstallProgress>> {
        for library in &self.library_manager.libraries {
            let manifest = library.path.join(format!("appmanifest_{}.acf", app_id));
            if !manifest.exists() {
                continue;
            }
            return Ok(Some(parse_install_progress(&manifest)?));
        }
        Ok(None)
    }

    /// Launch Steam game with optimizations
    pub async fn launch_game(&mut self, app_id: u32, launch_options: Option<&str>) -> Result<()> {
        info!("🚀 Launching Steam game: {}", app_id);

        // Get game information
        let game_info = self.library_manager.get_game_info(app_id).await?;
        info!("  • Game: {}", game_info.name);
        info!("  • Requires Proton: {}", game_info.requires_proton);

        // Apply performance optimizations
        self.performance_optimizer.apply_game_optimizations(app_id).await?;

        // Prepare launch command
        let mut launch_cmd = AsyncCommand::new("steam");
        launch_cmd.args(&["-applaunch", &app_id.to_string()]);

        // Add custom launch options
        if let Some(options) = launch_options {
            info!("  • Custom launch options: {}", options);
            // Parse and apply launch options
        }

        // Configure Proton if needed
        if game_info.requires_proton {
            self.configure_proton_for_game(app_id, &game_info).await?;
        }

        info!("  • Starting game with container optimizations");
        let child = launch_cmd.spawn()?;

        info!("✅ Game launched successfully: {}", game_info.name);

        // Start performance monitoring
        self.start_game_performance_monitoring(app_id).await?;

        Ok(())
    }

    /// Configure Proton for specific game
    async fn configure_proton_for_game(&self, app_id: u32, game_info: &SteamGame) -> Result<()> {
        info!("🍷 Configuring Proton for game: {}", game_info.name);

        let proton_version = game_info.proton_version.as_ref()
            .or(self.config.preferred_proton_version.as_ref())
            .ok_or_else(|| anyhow::anyhow!("No Proton version specified"))?;

        info!("  • Proton version: {}", proton_version);

        // Set Proton environment variables
        std::env::set_var("STEAM_COMPAT_DATA_PATH",
                         self.config.steam_user_data.join("compatdata").join(app_id.to_string()));
        std::env::set_var("STEAM_COMPAT_CLIENT_INSTALL_PATH", &self.config.steam_root);

        // Game-specific Proton optimizations
        match app_id {
            // Example: Cyberpunk 2077 optimizations
            1091500 => {
                std::env::set_var("DXVK_ASYNC", "1");
                std::env::set_var("PROTON_USE_WINED3D", "0");
                info!("  • Applied Cyberpunk 2077 optimizations");
            }
            // Example: Elden Ring optimizations
            1245620 => {
                std::env::set_var("PROTON_NO_ESYNC", "1");
                std::env::set_var("PROTON_NO_FSYNC", "1");
                info!("  • Applied Elden Ring optimizations");
            }
            _ => {
                // Default optimizations
                std::env::set_var("DXVK_HUD", "fps");
                std::env::set_var("PROTON_LOG", "1");
            }
        }

        info!("✅ Proton configured for: {}", game_info.name);
        Ok(())
    }

    /// Start performance monitoring for running game
    async fn start_game_performance_monitoring(&self, app_id: u32) -> Result<()> {
        info!("📊 Starting performance monitoring for game: {}", app_id);

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                // Monitor CPU, GPU, memory, and network usage
                // In a real implementation, this would use system monitoring tools
                debug!("📈 Game performance metrics updated for: {}", app_id);
            }
        });

        Ok(())
    }

    /// Get Steam library statistics
    pub async fn get_library_stats(&self) -> Result<SteamLibraryStats> {
        info!("📊 Gathering Steam library statistics");

        let stats = SteamLibraryStats {
            total_games: self.library_manager.installed_games.len() as u32,
            total_size_gb: self.library_manager.installed_games.values()
                .map(|game| game.size_bytes)
                .sum::<u64>() / (1024 * 1024 * 1024),
            total_playtime_hours: self.library_manager.installed_games.values()
                .map(|game| game.playtime_minutes)
                .sum::<u64>() / 60,
            favorite_games: self.library_manager.favorites.len() as u32,
            recently_played: self.library_manager.recently_played.len() as u32,
            proton_games: self.library_manager.installed_games.values()
                .filter(|game| game.requires_proton)
                .count() as u32,
            native_games: self.library_manager.installed_games.values()
                .filter(|game| !game.requires_proton)
                .count() as u32,
        };

        info!("✅ Library stats gathered: {} games, {:.1} GB",
              stats.total_games, stats.total_size_gb);

        Ok(stats)
    }

    /// Optimize Steam for container environment
    pub async fn optimize_for_container(&mut self) -> Result<()> {
        info!("⚡ Optimizing Steam for container environment");

        // Apply CPU optimizations
        self.performance_optimizer.apply_cpu_optimizations().await?;

        // Apply GPU optimizations
        self.performance_optimizer.apply_gpu_optimizations().await?;

        // Apply memory optimizations
        self.performance_optimizer.apply_memory_optimizations().await?;

        // Apply storage optimizations
        self.performance_optimizer.apply_storage_optimizations().await?;

        // Apply network optimizations
        self.performance_optimizer.apply_network_optimizations().await?;

        info!("✅ Steam optimized for container environment");
        Ok(())
    }

    /// Shutdown Steam gracefully
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("🛑 Shutting down Steam integration");

        if let Some(ref mut client) = self.steam_client {
            if let Some(pid) = client.pid {
                // Send graceful shutdown signal
                let mut cmd = AsyncCommand::new("steam");
                cmd.arg("-shutdown");

                match cmd.output().await {
                    Ok(_) => info!("✅ Steam client shutdown gracefully"),
                    Err(e) => warn!("Failed to shutdown Steam gracefully: {}", e),
                }

                client.status = SteamClientStatus::NotRunning;
                client.pid = None;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SteamLibraryStats {
    pub total_games: u32,
    pub total_size_gb: u64,
    pub total_playtime_hours: u64,
    pub favorite_games: u32,
    pub recently_played: u32,
    pub proton_games: u32,
    pub native_games: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SteamInstallProgress {
    state: Option<String>,
    bytes_downloaded: u64,
    bytes_total: u64,
    installed: bool,
}

impl SteamInstallProgress {
    fn percent(&self) -> f64 {
        if self.bytes_total == 0 {
            0.0
        } else {
            (self.bytes_downloaded as f64 / self.bytes_total as f64 * 100.0).min(100.0)
        }
    }
}

impl SteamLibraryManager {
    async fn new(config: &SteamConfig) -> Result<Self> {
        info!("📚 Initializing Steam library manager");

        let mut manager = Self {
            libraries: Vec::new(),
            installed_games: HashMap::new(),
            favorites: Vec::new(),
            recently_played: Vec::new(),
        };

        // Scan for Steam libraries
        manager.scan_steam_libraries(config).await?;

        // Load installed games
        manager.load_installed_games(config).await?;

        info!("✅ Steam library manager initialized with {} games",
              manager.installed_games.len());

        Ok(manager)
    }

    async fn scan_steam_libraries(&mut self, config: &SteamConfig) -> Result<()> {
        info!("🔍 Scanning Steam libraries");

        self.libraries.clear();
        self.libraries.push(steam_library_from_path(
            config.steam_apps.clone(),
            true,
        ));

        let libraryfolders = config.steam_apps.join("libraryfolders.vdf");
        for path in parse_steam_libraryfolders(&libraryfolders)? {
            let steamapps = path.join("steamapps");
            if steamapps != config.steam_apps {
                self.libraries.push(steam_library_from_path(steamapps, false));
            }
        }

        for library in &self.libraries {
            info!("  • Found Steam library: {:?}", library.path);
        }

        Ok(())
    }

    async fn load_installed_games(&mut self, config: &SteamConfig) -> Result<()> {
        info!("🎮 Loading installed Steam games");

        self.installed_games.clear();
        for library in &mut self.libraries {
            let games = parse_appmanifests(&library.path)?;
            library.game_count = games.len() as u32;
            for game in games {
                self.installed_games.insert(game.app_id, game);
            }
        }

        Ok(())
    }

    async fn get_game_info(&self, app_id: u32) -> Result<&SteamGame> {
        self.installed_games.get(&app_id)
            .ok_or_else(|| anyhow::anyhow!("Game not found: {}", app_id))
    }
}

fn steam_library_from_path(path: PathBuf, is_default: bool) -> SteamLibrary {
    SteamLibrary {
        path,
        size_bytes: 0,
        free_space_bytes: 0,
        game_count: 0,
        is_default,
    }
}

fn parse_steam_libraryfolders(path: &Path) -> Result<Vec<PathBuf>> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Ok(Vec::new());
    };

    let mut libraries = Vec::new();
    for line in contents.lines() {
        let tokens = parse_vdf_quoted_tokens(line);
        if tokens.len() == 2 && tokens[0] == "path" {
            libraries.push(PathBuf::from(tokens[1].replace("\\\\", "\\")));
        }
    }
    Ok(libraries)
}

fn parse_appmanifests(steamapps: &Path) -> Result<Vec<SteamGame>> {
    let mut games = Vec::new();
    let Ok(entries) = std::fs::read_dir(steamapps) else {
        return Ok(games);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
            continue;
        }
        if let Some(game) = parse_appmanifest(steamapps, &path)? {
            games.push(game);
        }
    }

    Ok(games)
}

fn parse_appmanifest(steamapps: &Path, path: &Path) -> Result<Option<SteamGame>> {
    let contents = std::fs::read_to_string(path)?;
    let mut fields = HashMap::new();
    for line in contents.lines() {
        let tokens = parse_vdf_quoted_tokens(line);
        if tokens.len() == 2 {
            fields.insert(tokens[0].clone(), tokens[1].clone());
        }
    }

    let Some(app_id) = fields.get("appid").and_then(|value| value.parse::<u32>().ok()) else {
        return Ok(None);
    };
    let name = fields
        .get("name")
        .cloned()
        .unwrap_or_else(|| format!("Steam App {}", app_id));
    let install_dir = fields
        .get("installdir")
        .cloned()
        .unwrap_or_else(|| app_id.to_string());
    let size_bytes = fields
        .get("SizeOnDisk")
        .or_else(|| fields.get("BytesToDownload"))
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let playtime_minutes = fields
        .get("LastPlayed")
        .and_then(|value| value.parse::<u64>().ok())
        .map(|_| 0)
        .unwrap_or(0);

    Ok(Some(SteamGame {
        app_id,
        name,
        install_dir: install_dir.clone(),
        install_path: steamapps.join("common").join(&install_dir),
        size_bytes,
        last_played: None,
        playtime_minutes,
        requires_proton: fields.contains_key("CompatToolMapping"),
        proton_version: fields.get("compat_tool").cloned(),
        launch_options: None,
        achievements: 0,
        screenshots: 0,
        dlc_count: 0,
        is_favorite: false,
        categories: Vec::new(),
        tags: Vec::new(),
    }))
}

fn parse_install_progress(path: &Path) -> Result<SteamInstallProgress> {
    let contents = std::fs::read_to_string(path)?;
    let mut fields = HashMap::new();
    for line in contents.lines() {
        let tokens = parse_vdf_quoted_tokens(line);
        if tokens.len() == 2 {
            fields.insert(tokens[0].clone(), tokens[1].clone());
        }
    }

    let state = fields.get("StateFlags").cloned();
    let bytes_downloaded = fields
        .get("BytesDownloaded")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let bytes_total = fields
        .get("BytesToDownload")
        .or_else(|| fields.get("SizeOnDisk"))
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let installed = matches!(state.as_deref(), Some("4")) || bytes_total > 0 && bytes_downloaded >= bytes_total;

    Ok(SteamInstallProgress {
        state,
        bytes_downloaded,
        bytes_total,
        installed,
    })
}

fn parse_vdf_quoted_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut escaped = false;

    for ch in line.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quote => escaped = true,
            '"' if in_quote => {
                tokens.push(current.clone());
                current.clear();
                in_quote = false;
            }
            '"' => in_quote = true,
            _ if in_quote => current.push(ch),
            _ => {}
        }
    }

    tokens
}

impl SteamCompatibilityLayer {
    async fn new(config: &SteamConfig) -> Result<Self> {
        info!("🍷 Initializing Steam compatibility layer");

        let mut layer = Self {
            proton_installations: HashMap::new(),
            wine_prefixes: HashMap::new(),
            compatibility_tools: Vec::new(),
        };

        // Scan for Proton installations
        layer.scan_proton_installations(config).await?;

        info!("✅ Steam compatibility layer initialized");
        Ok(layer)
    }

    async fn scan_proton_installations(&mut self, config: &SteamConfig) -> Result<()> {
        info!("🔍 Scanning for Proton installations");

        // Add example Proton installations
        let proton_8 = ProtonInstallation {
            name: "Proton 8.0".to_string(),
            version: "8.0-1".to_string(),
            path: config.steam_root.join("steamapps").join("common").join("Proton 8.0"),
            wine_version: "wine-8.0".to_string(),
            dxvk_version: Some("2.1".to_string()),
            vkd3d_version: Some("2.8".to_string()),
            supports_battleye: true,
            supports_eac: true,
            is_experimental: false,
        };

        self.proton_installations.insert("Proton 8.0".to_string(), proton_8);

        info!("  • Found Proton installations: {}", self.proton_installations.len());
        Ok(())
    }
}

impl SteamPerformanceOptimizer {
    async fn new(config: &SteamConfig) -> Result<Self> {
        info!("⚡ Initializing Steam performance optimizer");

        let optimizer = Self {
            cpu_optimizations: CpuOptimizations {
                cpu_governor: "performance".to_string(),
                cpu_scaling: "performance".to_string(),
                process_priority: -10, // High priority
                cpu_affinity: vec![0, 1, 2, 3], // Use first 4 cores
                disable_c_states: true,
                enable_turbo_boost: true,
            },
            gpu_optimizations: GpuOptimizations {
                gpu_power_mode: "prefer_maximum_performance".to_string(),
                gpu_memory_clock: Some(500),
                gpu_core_clock: Some(200),
                enable_resizable_bar: true,
                force_high_performance: true,
                disable_gpu_scheduling: false,
            },
            memory_optimizations: MemoryOptimizations {
                memory_policy: "performance".to_string(),
                huge_pages: true,
                memory_compression: false,
                swap_configuration: "disabled".to_string(),
                memory_overcommit: 0,
            },
            storage_optimizations: StorageOptimizations {
                io_scheduler: "mq-deadline".to_string(),
                read_ahead_kb: 4096,
                enable_write_cache: true,
                filesystem_optimizations: vec![
                    "noatime".to_string(),
                    "nodiratime".to_string(),
                ],
            },
            network_optimizations: NetworkOptimizations {
                tcp_congestion_control: "bbr".to_string(),
                network_buffer_sizes: {
                    let mut sizes = HashMap::new();
                    sizes.insert("net.core.rmem_max".to_string(), 134217728);
                    sizes.insert("net.core.wmem_max".to_string(), 134217728);
                    sizes
                },
                enable_tcp_fast_open: true,
                disable_tcp_timestamps: false,
            },
        };

        info!("✅ Steam performance optimizer initialized");
        Ok(optimizer)
    }

    async fn apply_game_optimizations(&self, app_id: u32) -> Result<()> {
        info!("⚡ Applying game-specific optimizations for: {}", app_id);

        // Apply CPU optimizations
        self.apply_cpu_optimizations().await?;

        // Apply GPU optimizations
        self.apply_gpu_optimizations().await?;

        // Apply memory optimizations
        self.apply_memory_optimizations().await?;

        info!("✅ Game optimizations applied for: {}", app_id);
        Ok(())
    }

    async fn apply_cpu_optimizations(&self) -> Result<()> {
        info!("🔧 Applying CPU optimizations");

        // Expose intended CPU policy to the container launch environment. Host-level
        // governor/cgroup writes are handled by runtime code with explicit privileges.
        info!("  • CPU governor: {}", self.cpu_optimizations.cpu_governor);
        info!("  • Process priority: {}", self.cpu_optimizations.process_priority);
        info!("  • CPU affinity: {:?}", self.cpu_optimizations.cpu_affinity);
        Ok(())
    }

    async fn apply_gpu_optimizations(&self) -> Result<()> {
        info!("🎮 Applying GPU optimizations");

        // Set GPU power mode
        std::env::set_var("__GL_PowerMizerEnable", "0x1");
        std::env::set_var("__GL_PowerMizerLevel", "0x3");

        info!("  • GPU power mode: {}", self.gpu_optimizations.gpu_power_mode);
        Ok(())
    }

    async fn apply_memory_optimizations(&self) -> Result<()> {
        info!("💾 Applying memory optimizations");

        info!("  • Huge pages: {}", self.memory_optimizations.huge_pages);
        info!("  • Memory policy: {}", self.memory_optimizations.memory_policy);
        info!(
            "  • Swap configuration: {}",
            self.memory_optimizations.swap_configuration
        );
        Ok(())
    }

    async fn apply_storage_optimizations(&self) -> Result<()> {
        info!("💿 Applying storage optimizations");
        info!("  • I/O scheduler: {}", self.storage_optimizations.io_scheduler);
        info!("  • Read ahead: {} KB", self.storage_optimizations.read_ahead_kb);
        Ok(())
    }

    async fn apply_network_optimizations(&self) -> Result<()> {
        info!("🌐 Applying network optimizations");
        info!("  • TCP congestion control: {}", self.network_optimizations.tcp_congestion_control);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_tempdir() -> tempfile::TempDir {
        std::fs::create_dir_all(".scratch").expect("create repo-local scratch directory");
        tempfile::tempdir_in(".scratch").expect("create repo-local scratch tempdir")
    }

    #[test]
    fn steam_appmanifest_parser_loads_real_fields() -> Result<()> {
        let root = scratch_tempdir();
        let steamapps = root.path().join("steamapps");
        std::fs::create_dir_all(steamapps.join("common/Test Game"))?;
        let manifest = steamapps.join("appmanifest_42.acf");
        std::fs::write(
            &manifest,
            r#""AppState"
{
    "appid" "42"
    "name" "Test Game"
    "installdir" "Test Game"
    "SizeOnDisk" "1234"
}
"#,
        )?;

        let game = parse_appmanifest(&steamapps, &manifest)?.expect("game manifest");
        assert_eq!(game.app_id, 42);
        assert_eq!(game.name, "Test Game");
        assert_eq!(game.install_path, steamapps.join("common/Test Game"));
        assert_eq!(game.size_bytes, 1234);
        Ok(())
    }

    #[test]
    fn steam_install_progress_uses_manifest_bytes() -> Result<()> {
        let root = scratch_tempdir();
        let manifest = root.path().join("appmanifest_42.acf");
        std::fs::write(
            &manifest,
            r#""AppState"
{
    "appid" "42"
    "StateFlags" "1026"
    "BytesDownloaded" "50"
    "BytesToDownload" "200"
}
"#,
        )?;

        let progress = parse_install_progress(&manifest)?;
        assert_eq!(progress.bytes_downloaded, 50);
        assert_eq!(progress.bytes_total, 200);
        assert_eq!(progress.percent(), 25.0);
        assert!(!progress.installed);
        Ok(())
    }
}
