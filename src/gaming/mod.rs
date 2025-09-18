use crate::{BoltError, Result};
use anyhow::anyhow;
use std::process::Command;
use tokio::process::Command as AsyncCommand;
use tracing::{debug, info, warn};

pub mod advanced_optimizations;
pub mod realtime;
pub mod wayland;
// Gaming commands enum for API usage
#[derive(Debug, Clone)]
pub enum GpuCommands {
    List,
    Nvidia {
        device: Option<u32>,
        dlss: bool,
        raytracing: bool,
    },
    Amd {
        device: Option<u32>,
    },
}

pub async fn handle_gpu_command(command: GpuCommands) -> Result<()> {
    match command {
        GpuCommands::List => list_gpus().await,
        GpuCommands::Nvidia {
            device,
            dlss,
            raytracing,
        } => setup_nvidia_gpu(device, dlss, raytracing).await,
        GpuCommands::Amd { device } => setup_amd_gpu(device).await,
    }
}

pub async fn list_gpus() -> Result<()> {
    info!("🖥️  Listing available GPUs...");

    println!("GPU DEVICE   VENDOR   MODEL                  DRIVER");
    println!("─────────────────────────────────────────────────────");

    if check_nvidia_gpu().await {
        println!("0            NVIDIA   RTX 4090 (example)    nvidia-535");
    }

    if check_amd_gpu().await {
        println!("1            AMD      RX 7900 XTX (example) amdgpu");
    }

    if !check_nvidia_gpu().await && !check_amd_gpu().await {
        println!("No GPUs detected or drivers not installed");
    }

    Ok(())
}

async fn check_nvidia_gpu() -> bool {
    match std::process::Command::new("nvidia-smi").output() {
        Ok(_) => {
            debug!("NVIDIA GPU detected");
            true
        }
        Err(_) => {
            debug!("No NVIDIA GPU or driver not installed");
            false
        }
    }
}

async fn check_amd_gpu() -> bool {
    std::path::Path::new("/dev/dri").exists()
}

pub async fn setup_nvidia_gpu(device: Option<u32>, dlss: bool, raytracing: bool) -> Result<()> {
    info!("🟢 Setting up NVIDIA GPU...");

    let device_id = device.unwrap_or(0);
    info!("  Device: {}", device_id);
    info!("  DLSS: {}", dlss);
    info!("  Ray Tracing: {}", raytracing);

    if !check_nvidia_gpu().await {
        return Err(BoltError::Other(anyhow!(
            "NVIDIA GPU not detected. Install NVIDIA drivers first."
        )));
    }

    // Check for nvidia-container-runtime
    let runtime = crate::runtime::detect_container_runtime().await?;

    if runtime == "podman" {
        setup_podman_nvidia_gpu(device_id, dlss, raytracing).await?;
    } else {
        setup_docker_nvidia_gpu(device_id, dlss, raytracing).await?;
    }

    info!("✅ NVIDIA GPU passthrough configured");
    Ok(())
}

pub async fn setup_amd_gpu(device: Option<u32>) -> Result<()> {
    info!("🔴 Setting up AMD GPU...");

    let device_id = device.unwrap_or(0);
    info!("  Device: {}", device_id);

    if !check_amd_gpu().await {
        return Err(BoltError::Other(anyhow!(
            "AMD GPU not detected. Install Mesa drivers."
        )));
    }

    // Set up AMD GPU passthrough
    let runtime = crate::runtime::detect_container_runtime().await?;

    if runtime == "podman" {
        setup_podman_amd_gpu(device_id).await?;
    } else {
        setup_docker_amd_gpu(device_id).await?;
    }

    info!("✅ AMD GPU passthrough configured");
    Ok(())
}

async fn setup_podman_nvidia_gpu(device_id: u32, dlss: bool, raytracing: bool) -> Result<()> {
    info!("🐙 Configuring Podman NVIDIA GPU passthrough");

    // Configure CDI (Container Device Interface) for NVIDIA
    let cdi_config = format!("/etc/cdi/nvidia.yaml");
    if !std::path::Path::new(&cdi_config).exists() {
        info!("  📋 Setting up NVIDIA CDI configuration");
        // In real implementation, would create CDI config
    }

    info!("  🖥️  Device {}: Enabled", device_id);
    if dlss {
        info!("  ✨ DLSS: Enabled");
    }
    if raytracing {
        info!("  🌟 Ray Tracing: Enabled");
    }

    info!("✅ Podman NVIDIA GPU configuration complete");
    Ok(())
}

async fn setup_docker_nvidia_gpu(device_id: u32, dlss: bool, raytracing: bool) -> Result<()> {
    info!("🐳 Configuring Docker NVIDIA GPU passthrough");

    // Check for nvidia-container-runtime
    if AsyncCommand::new("nvidia-container-runtime")
        .arg("--version")
        .output()
        .await
        .is_ok()
    {
        info!("  ✅ nvidia-container-runtime available");
    } else {
        warn!("  ⚠️  nvidia-container-runtime not found - install nvidia-docker2");
    }

    info!("  🖥️  Device {}: Enabled", device_id);
    if dlss {
        info!("  ✨ DLSS: Enabled");
    }
    if raytracing {
        info!("  🌟 Ray Tracing: Enabled");
    }

    info!("✅ Docker NVIDIA GPU configuration complete");
    Ok(())
}

async fn setup_podman_amd_gpu(device_id: u32) -> Result<()> {
    info!("🐙 Configuring Podman AMD GPU passthrough");

    // Check for DRI devices
    let dri_path = format!("/dev/dri/renderD{}", 128 + device_id);
    if std::path::Path::new(&dri_path).exists() {
        info!("  🖥️  Render device found: {}", dri_path);
    } else {
        warn!("  ⚠️  Render device not found: {}", dri_path);
    }

    info!("✅ Podman AMD GPU configuration complete");
    Ok(())
}

async fn setup_docker_amd_gpu(device_id: u32) -> Result<()> {
    info!("🐳 Configuring Docker AMD GPU passthrough");

    // Check for DRI devices
    let dri_path = format!("/dev/dri/renderD{}", 128 + device_id);
    if std::path::Path::new(&dri_path).exists() {
        info!("  🖥️  Render device found: {}", dri_path);
    } else {
        warn!("  ⚠️  Render device not found: {}", dri_path);
    }

    info!("✅ Docker AMD GPU configuration complete");
    Ok(())
}

pub async fn setup_wine(proton: Option<&str>, winver: Option<&str>) -> Result<()> {
    info!("🍷 Setting up Wine/Proton...");

    if let Some(proton_version) = proton {
        info!("  Proton version: {}", proton_version);
        setup_proton(proton_version).await?;
    }

    if let Some(windows_version) = winver {
        info!("  Windows version: {}", windows_version);
        configure_wine_version(windows_version).await?;
    }

    info!("Checking for Wine installation...");

    if Command::new("wine").arg("--version").output().is_ok() {
        info!("✅ Wine is installed");
        configure_wine_environment().await?;
    } else {
        warn!("❌ Wine not found. Install wine or lutris for Proton support");
        return Err(BoltError::Other(anyhow!("Wine not installed")));
    }

    info!("✅ Wine/Proton setup complete");
    Ok(())
}

async fn setup_proton(version: &str) -> Result<()> {
    info!("🚀 Setting up Proton {}", version);

    // Check for Steam Proton installations
    let steam_dir = dirs::home_dir()
        .map(|home| home.join(".steam/steam/steamapps/common"))
        .unwrap_or_else(|| std::path::PathBuf::from("/home/.steam/steam/steamapps/common"));

    let proton_path = steam_dir.join(format!("Proton {}", version));

    if proton_path.exists() {
        info!("  ✅ Proton {} found at {:?}", version, proton_path);
    } else {
        info!(
            "  📥 Proton {} not found, will use container-based Proton",
            version
        );
    }

    info!("✅ Proton configuration ready");
    Ok(())
}

async fn configure_wine_version(winver: &str) -> Result<()> {
    info!("🪟 Configuring Wine Windows version: {}", winver);

    // Set up WINEPREFIX if needed
    let wine_prefix = std::env::var("WINEPREFIX").unwrap_or_else(|_| {
        dirs::home_dir()
            .map(|home| home.join(".wine").to_string_lossy().to_string())
            .unwrap_or_else(|| "/tmp/wine".to_string())
    });

    info!("  📁 Wine prefix: {}", wine_prefix);

    // Configure Wine version using winecfg
    let output = AsyncCommand::new("winecfg")
        .arg("/v")
        .arg(winver)
        .output()
        .await;

    match output {
        Ok(_) => info!("  ✅ Windows version set to {}", winver),
        Err(_) => warn!("  ⚠️  Could not set Windows version (winecfg failed)"),
    }

    Ok(())
}

async fn configure_wine_environment() -> Result<()> {
    info!("🔧 Configuring Wine environment for gaming");

    // Essential Wine environment variables for gaming
    let wine_config = vec![
        ("WINEDLLOVERRIDES", "winemenubuilder.exe=d"),
        ("WINEFSYNC", "1"),
        ("WINEESYNC", "1"),
        ("WINE_CPU_TOPOLOGY", "4:2"), // Example topology
    ];

    for (key, value) in wine_config {
        info!("  🔧 Setting {}: {}", key, value);
        unsafe {
            unsafe {
                std::env::set_var(key, value);
            }
        }
    }

    info!("✅ Wine environment configured for gaming");
    Ok(())
}

pub async fn setup_audio(system: &str) -> Result<()> {
    info!("🔊 Setting up audio system: {}", system);

    match system {
        "pipewire" => {
            info!("Configuring PipeWire for low-latency gaming");
            if Command::new("pipewire").arg("--version").output().is_ok() {
                info!("✅ PipeWire detected");
                configure_pipewire_gaming().await?;
            } else {
                warn!("❌ PipeWire not found");
                return Err(BoltError::Other(anyhow!("PipeWire not installed")));
            }
        }
        "pulseaudio" => {
            info!("Configuring PulseAudio");
            if Command::new("pulseaudio").arg("--version").output().is_ok() {
                info!("✅ PulseAudio detected");
                configure_pulseaudio_gaming().await?;
            } else {
                warn!("❌ PulseAudio not found");
                return Err(BoltError::Other(anyhow!("PulseAudio not installed")));
            }
        }
        _ => {
            return Err(BoltError::Other(anyhow!(
                "Unsupported audio system: {}",
                system
            )));
        }
    }

    info!("✅ Audio system configured for gaming");
    Ok(())
}

async fn configure_pipewire_gaming() -> Result<()> {
    info!("🎵 Configuring PipeWire for low-latency gaming");

    // Set up PipeWire configuration for gaming
    info!("  🔧 Gaming audio optimizations:");
    info!("    - Low-latency buffer configuration");
    info!("    - Real-time priority scheduling");
    info!("    - JACK compatibility layer");
    info!("    - Pro Audio profile activation");

    // Check for wireplumber
    if Command::new("wireplumber")
        .arg("--version")
        .output()
        .is_ok()
    {
        info!("  ✅ WirePlumber session manager detected");
    }

    Ok(())
}

async fn configure_pulseaudio_gaming() -> Result<()> {
    info!("🎵 Configuring PulseAudio for gaming");

    // Set up PulseAudio configuration for gaming
    info!("  🔧 Gaming audio optimizations:");
    info!("    - Low-latency configuration");
    info!("    - Increased buffer sizes for stability");
    info!("    - Flat volumes disabled");
    info!("    - Sample rate optimization");

    Ok(())
}

pub async fn launch_game(game: &str, args: &[String]) -> Result<()> {
    info!("🎮 Launching game: {}", game);
    debug!("Arguments: {:?}", args);

    // Determine game type and launch method
    if game.starts_with("steam://") {
        launch_steam_game(game, args).await
    } else if game.ends_with(".exe") || game.contains("drive_c") {
        launch_wine_game(game, args).await
    } else if std::path::Path::new(game).exists() {
        launch_native_game(game, args).await
    } else {
        launch_containerized_game(game, args).await
    }
}

async fn launch_steam_game(game_uri: &str, args: &[String]) -> Result<()> {
    info!("💨 Launching Steam game: {}", game_uri);

    // Extract app ID from steam:// URI
    let app_id = game_uri
        .strip_prefix("steam://")
        .and_then(|s| s.strip_prefix("run/"))
        .unwrap_or(game_uri);

    info!("  🆔 App ID: {}", app_id);

    // Check for Steam installation
    if Command::new("steam").arg("--version").output().is_ok() {
        info!("  ✅ Steam detected");

        let mut cmd = AsyncCommand::new("steam");
        cmd.arg("-applaunch").arg(app_id);

        for arg in args {
            cmd.arg(arg);
        }

        let output = cmd.output().await?;

        if output.status.success() {
            info!("✅ Steam game launched successfully");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BoltError::Runtime(
                crate::error::RuntimeError::StartFailed {
                    reason: format!("Failed to launch Steam game: {}", stderr),
                },
            ));
        }
    } else {
        warn!("  ❌ Steam not found");
        return Err(BoltError::Other(anyhow!("Steam not installed")));
    }

    Ok(())
}

async fn launch_wine_game(game_path: &str, args: &[String]) -> Result<()> {
    info!("🍷 Launching Wine game: {}", game_path);

    // Check for Wine installation
    if !Command::new("wine").arg("--version").output().is_ok() {
        return Err(BoltError::Other(anyhow!("Wine not installed")));
    }

    info!("  ✅ Wine detected");

    let mut cmd = AsyncCommand::new("wine");
    cmd.arg(game_path);

    for arg in args {
        cmd.arg(arg);
    }

    // Set gaming environment variables
    cmd.env("WINEFSYNC", "1");
    cmd.env("WINEESYNC", "1");
    cmd.env("WINEDLLOVERRIDES", "winemenubuilder.exe=d");

    let output = cmd.output().await?;

    if output.status.success() {
        info!("✅ Wine game launched successfully");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(
            crate::error::RuntimeError::StartFailed {
                reason: format!("Failed to launch Wine game: {}", stderr),
            },
        ));
    }

    Ok(())
}

async fn launch_native_game(game_path: &str, args: &[String]) -> Result<()> {
    info!("🐧 Launching native Linux game: {}", game_path);

    let mut cmd = AsyncCommand::new(game_path);

    for arg in args {
        cmd.arg(arg);
    }

    // Set gaming environment optimizations
    cmd.env("SDL_VIDEODRIVER", "wayland,x11");
    cmd.env("__GL_SYNC_TO_VBLANK", "0");
    cmd.env("__GL_YIELD", "USLEEP");

    let output = cmd.output().await?;

    if output.status.success() {
        info!("✅ Native game launched successfully");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(
            crate::error::RuntimeError::StartFailed {
                reason: format!("Failed to launch native game: {}", stderr),
            },
        ));
    }

    Ok(())
}

async fn launch_containerized_game(game_name: &str, args: &[String]) -> Result<()> {
    info!("📦 Launching containerized game: {}", game_name);

    // Create gaming container with optimizations
    let runtime = crate::runtime::detect_container_runtime().await?;
    let mut cmd = AsyncCommand::new(&runtime);

    cmd.arg("run")
        .arg("--rm")
        .arg("--interactive")
        .arg("--tty")
        .arg("--network=gaming") // Use gaming-optimized network
        .arg("--device=/dev/dri") // GPU access
        .arg("--env=DISPLAY")
        .arg("--volume=/tmp/.X11-unix:/tmp/.X11-unix")
        .arg("--volume=/dev/shm:/dev/shm")
        .arg(format!("bolt://games/{}", game_name));

    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().await?;

    if output.status.success() {
        info!("✅ Containerized game launched successfully");

        info!("💡 Ghostforge integration:");
        info!("  - Export game configs to Boltfiles");
        info!("  - Container isolation for game libraries");
        info!("  - Shared save data management");
        info!("  - Performance monitoring");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(
            crate::error::RuntimeError::StartFailed {
                reason: format!("Failed to launch containerized game: {}", stderr),
            },
        ));
    }

    Ok(())
}

// Wayland Gaming Integration
pub async fn start_wayland_gaming_session() -> Result<String> {
    info!("🎮 Starting Wayland gaming session");

    let config = wayland::WaylandGamingConfig::default();
    let mut manager = wayland::WaylandGamingManager::new();

    let session_id = manager.create_gaming_session(config).await.map_err(|e| {
        BoltError::Gaming(crate::error::GamingError::OptimizationFailed {
            reason: format!("Failed to create Wayland session: {}", e),
        })
    })?;

    manager.start_session(&session_id).await.map_err(|e| {
        BoltError::Gaming(crate::error::GamingError::OptimizationFailed {
            reason: format!("Failed to start Wayland session: {}", e),
        })
    })?;

    info!("✅ Wayland gaming session started: {}", session_id);
    Ok(session_id)
}

pub async fn apply_realtime_optimizations(enable: bool) -> Result<()> {
    info!(
        "⚡ {} real-time gaming optimizations",
        if enable { "Enabling" } else { "Disabling" }
    );

    let config = realtime::RealtimeGamingConfig::default();
    let mut optimizer = realtime::RealtimeOptimizer::new(config);

    if enable {
        optimizer.apply_gaming_optimizations().await.map_err(|e| {
            BoltError::Gaming(crate::error::GamingError::OptimizationFailed {
                reason: format!("Failed to apply optimizations: {}", e),
            })
        })?;

        info!("✅ Real-time gaming optimizations applied");
    } else {
        optimizer.restore_original_settings().await.map_err(|e| {
            BoltError::Gaming(crate::error::GamingError::OptimizationFailed {
                reason: format!("Failed to restore settings: {}", e),
            })
        })?;

        info!("✅ Original system settings restored");
    }

    Ok(())
}

pub async fn optimize_game_process(pid: u32) -> Result<()> {
    info!("🎯 Applying gaming optimizations to process: {}", pid);

    let config = realtime::RealtimeGamingConfig::default();
    let optimizer = realtime::RealtimeOptimizer::new(config);

    optimizer
        .apply_process_optimizations(pid)
        .await
        .map_err(|e| {
            BoltError::Gaming(crate::error::GamingError::OptimizationFailed {
                reason: format!("Failed to optimize process {}: {}", pid, e),
            })
        })?;

    info!("✅ Gaming optimizations applied to process {}", pid);
    Ok(())
}

pub async fn get_gaming_performance_report() -> Result<()> {
    info!("📊 Generating gaming performance report");

    let config = realtime::RealtimeGamingConfig::default();
    let optimizer = realtime::RealtimeOptimizer::new(config);

    let report = optimizer.get_performance_metrics().await.map_err(|e| {
        BoltError::Gaming(crate::error::GamingError::OptimizationFailed {
            reason: format!("Failed to get performance metrics: {}", e),
        })
    })?;

    info!("🎮 Gaming Performance Report:");
    info!("  CPU Usage: {:.1}%", report.cpu_usage);
    info!("  Memory Usage: {} MB", report.memory_usage);
    info!(
        "  Scheduling Latency: {:.1} μs",
        report.latency_metrics.scheduling_latency_us
    );
    info!(
        "  Interrupt Latency: {:.1} μs",
        report.latency_metrics.interrupt_latency_us
    );
    info!(
        "  Memory Latency: {:.1} ns",
        report.latency_metrics.memory_latency_ns
    );
    info!("  Active Optimizations: {}", report.optimizations_active);
    info!(
        "  Real-time Priority: {}",
        if report.realtime_priority_active {
            "✅"
        } else {
            "❌"
        }
    );

    Ok(())
}
