# GhostForge Integration with Bolt

Integration guide for [GhostForge](https://github.com/ghostforge) - A pure Rust alternative to Lutris that leverages Bolt container runtime for Wine/Proton gaming environments.

## Overview

GhostForge replaces Lutris as a gaming platform manager, using Bolt's advanced container runtime instead of Podman for superior performance, GPU management, and declarative gaming environment configuration.

## Architecture

```
ghostforge
├── Core Gaming Engine (Rust)
├── Game Library Management
├── Wine/Proton Integration
└── Bolt Runtime
    ├── Gaming Capsules
    ├── Wine Containers
    ├── GPU Passthrough
    └── Storage Management
```

## Integration Setup

### 1. GhostForge Boltfile Configuration

Create `Boltfile.toml` in your ghostforge project:

```toml
project = "ghostforge"

# Base Wine/Proton runtime environment
[services.proton-runtime]
build = "./containers/proton"
gpu.nvidia = true
gpu.amd = true
privileged = true
volumes = [
    "./games:/games",
    "./wine-prefixes:/prefixes",
    "/tmp/.X11-unix:/tmp/.X11-unix",
    "/dev/dri:/dev/dri",
    "/run/user/1000/pulse:/run/user/1000/pulse"
]
devices = ["/dev/input", "/dev/uinput"]
env.DISPLAY = ":0"
env.PULSE_RUNTIME_PATH = "/run/user/1000/pulse"
env.WINE_PREFIX = "/prefixes/default"

# Steam compatibility layer
[services.steam-runtime]
build = "./containers/steam"
gpu.nvidia = true
volumes = [
    "./steam:/steam",
    "./games:/games",
    "/tmp/.X11-unix:/tmp/.X11-unix"
]
env.STEAM_COMPAT_DATA_PATH = "/steam/steamapps/compatdata"
env.STEAM_COMPAT_CLIENT_INSTALL_PATH = "/steam"

# Game-specific runtime (example)
[services.cyberpunk-2077]
capsule = "gaming/proton-ge"
gpu.nvidia = true
memory_limit = "16Gi"
cpu_limit = "8"
volumes = [
    "./games/cyberpunk2077:/game",
    "./saves/cyberpunk2077:/saves",
    "/tmp/.X11-unix:/tmp/.X11-unix"
]
env.PROTON_VERSION = "GE-Proton8-26"
env.WINE_PREFIX = "/prefixes/cyberpunk2077"
env.DXVK_ENABLE = "1"
env.MANGOHUD = "1"

# Gaming service manager
[services.game-manager]
build = "./src"
ports = ["8090:8090"]
volumes = ["./config:/config"]
env.BOLT_ENDPOINT = "unix:///var/run/bolt.sock"

# Performance monitoring
[services.performance-monitor]
build = "./containers/monitor"
gpu.nvidia = true
volumes = [
    "/sys/class/drm:/sys/class/drm:ro",
    "/proc:/host/proc:ro"
]
env.MONITORING_INTERVAL = "1000"

[network]
driver = "quic"
encryption = true

[storage.games]
type = "zfs"
size = "2Ti"
compression = "lz4"

[storage.prefixes]
type = "overlay"
size = "500Gi"
snapshot_enabled = true
```

### 2. Container Definitions

#### Proton Runtime Container (`containers/proton/Dockerfile.bolt`)

```dockerfile
FROM bolt://ubuntu:22.04

# Install Wine dependencies
RUN apt-get update && apt-get install -y \
    wine \
    winetricks \
    xvfb \
    pulseaudio \
    mesa-utils \
    vulkan-tools \
    dxvk \
    && rm -rf /var/lib/apt/lists/*

# Install Proton-GE
RUN mkdir -p /opt/proton-ge && \
    wget -O- https://github.com/GloriousEggroll/proton-ge-custom/releases/latest/download/GE-Proton*.tar.gz | \
    tar -xz -C /opt/proton-ge --strip-components=1

# Setup gaming environment
COPY gaming-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/gaming-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/gaming-entrypoint.sh"]
```

#### Steam Runtime Container (`containers/steam/Dockerfile.bolt`)

```dockerfile
FROM bolt://steamcmd:latest

# Install Steam Runtime
RUN apt-get update && apt-get install -y \
    steam-runtime \
    steam-devices \
    && rm -rf /var/lib/apt/lists/*

COPY steam-launcher.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/steam-launcher.sh

CMD ["/usr/local/bin/steam-launcher.sh"]
```

### 3. GhostForge Core Integration

```rust
// src/runtime/bolt_integration.rs
use bolt::runtime::oci::Container;
use bolt::surge::Orchestrator;
use bolt::capsules::snapshots::Snapshot;

pub struct BoltGameRuntime {
    orchestrator: Orchestrator,
}

impl BoltGameRuntime {
    pub async fn new() -> anyhow::Result<Self> {
        let boltfile = bolt::config::Boltfile::from_path("Boltfile.toml")?;
        let orchestrator = Orchestrator::new(boltfile);

        Ok(Self { orchestrator })
    }

    pub async fn launch_game(&self, game_id: &str) -> anyhow::Result<()> {
        // Create pre-launch snapshot
        let snapshot = Snapshot::create(&format!("pre-launch-{}", game_id)).await?;

        // Start gaming runtime
        self.orchestrator.start_service("proton-runtime").await?;

        // Launch specific game container
        let game_service = format!("game-{}", game_id);
        self.orchestrator.start_service(&game_service).await?;

        println!("🎮 Game {} launched in Bolt runtime", game_id);
        Ok(())
    }

    pub async fn install_game(&self, game_path: &str, proton_version: &str) -> anyhow::Result<()> {
        let install_config = format!(
            r#"
            [services.game-installer]
            capsule = "gaming/proton-{}"
            volumes = ["{}:/install"]
            env.WINE_PREFIX = "/prefixes/new-game"
            "#,
            proton_version, game_path
        );

        // Dynamic service creation
        self.orchestrator.add_service_from_config(&install_config).await?;
        self.orchestrator.start_service("game-installer").await?;

        Ok(())
    }
}
```

### 4. Game Management System

```rust
// src/games/manager.rs
use bolt::capsules::templates::Template;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GameConfig {
    pub name: String,
    pub executable: String,
    pub proton_version: String,
    pub wine_prefix: String,
    pub gpu_requirements: GpuRequirements,
    pub performance_profile: PerformanceProfile,
}

#[derive(Serialize, Deserialize)]
pub struct GpuRequirements {
    pub nvidia: bool,
    pub amd: bool,
    pub vram_min: String,
    pub dx_version: String,
}

pub struct GameManager {
    bolt_runtime: BoltGameRuntime,
    games_db: GamesDatabase,
}

impl GameManager {
    pub async fn create_game_template(&self, config: GameConfig) -> anyhow::Result<()> {
        let template = Template::new(&config.name);

        // Configure gaming capsule template
        template.set_base_image("gaming/proton-ge");
        template.set_gpu_config(config.gpu_requirements);
        template.set_wine_prefix(&config.wine_prefix);

        // Create performance-optimized configuration
        let bolt_config = format!(
            r#"
            [services.{}]
            capsule = "gaming/proton-ge"
            gpu.nvidia = {}
            gpu.amd = {}
            memory_limit = "{}"
            cpu_limit = "{}"
            volumes = [
                "./games/{}:/game",
                "./prefixes/{}:/prefix"
            ]
            env.WINE_PREFIX = "/prefix"
            env.PROTON_VERSION = "{}"
            env.DXVK_ENABLE = "1"
            "#,
            config.name.to_lowercase().replace(" ", "-"),
            config.gpu_requirements.nvidia,
            config.gpu_requirements.amd,
            config.performance_profile.memory_limit,
            config.performance_profile.cpu_limit,
            config.name,
            config.name,
            config.proton_version
        );

        template.save_config(&bolt_config).await?;
        println!("✅ Game template created for {}", config.name);

        Ok(())
    }

    pub async fn launch_with_mods(&self, game_id: &str, mods: Vec<String>) -> anyhow::Result<()> {
        // Create modded game snapshot
        let snapshot = Snapshot::create(&format!("{}-with-mods", game_id)).await?;

        // Install mods in isolated environment
        for mod_name in mods {
            self.install_mod(game_id, &mod_name).await?;
        }

        // Launch modded game
        self.bolt_runtime.launch_game(game_id).await?;

        Ok(())
    }
}
```

### 5. Performance Optimization

```rust
// src/performance/optimizer.rs
use bolt::runtime::gpu::GpuMetrics;

pub struct PerformanceOptimizer {
    bolt_runtime: BoltGameRuntime,
}

impl PerformanceOptimizer {
    pub async fn optimize_for_game(&self, game_id: &str) -> anyhow::Result<()> {
        let gpu_metrics = GpuMetrics::collect().await?;

        // Dynamic CPU/Memory allocation based on system resources
        let cpu_cores = num_cpus::get();
        let memory_gb = self.get_available_memory().await?;

        let optimized_config = format!(
            r#"
            [services.{}-optimized]
            cpu_limit = "{}"
            memory_limit = "{}Gi"
            cpu_scheduler = "performance"
            gpu.power_limit = "max"
            "#,
            game_id,
            (cpu_cores * 3 / 4), // Use 75% of cores
            (memory_gb * 2 / 3)  // Use 66% of memory
        );

        self.bolt_runtime.apply_config(&optimized_config).await?;

        println!("🚀 Performance optimized for {}", game_id);
        Ok(())
    }
}
```

## CLI Integration

GhostForge CLI commands using Bolt:

```bash
# Launch a game
ghostforge launch "Cyberpunk 2077"

# Install new game
ghostforge install --proton GE-8.26 --path ./games/newgame

# Create game snapshot
ghostforge snapshot create stable-config

# List running games
ghostforge ps

# Performance monitoring
ghostforge monitor --game "Cyberpunk 2077"

# Update Proton version
ghostforge proton update GE-8.30

# Mod management
ghostforge mod install nexusmods://1234 --game "Skyrim"
```

## Advanced Features

### 1. Multi-GPU Gaming

```toml
[services.dual-gpu-game]
gpu.nvidia = true
gpu.amd = true
env.GPU_PRIMARY = "nvidia"
env.GPU_SECONDARY = "amd"
env.PRIME_OFFLOAD = "1"
```

### 2. VR Gaming Support

```toml
[services.vr-runtime]
build = "./containers/openvr"
privileged = true
devices = ["/dev/hidraw*", "/dev/usb"]
volumes = ["/sys/devices:/sys/devices"]
env.OPENVR_ROOT = "/opt/openvr"
```

### 3. Wine Prefix Management

```rust
pub async fn create_wine_prefix(&self, name: &str, windows_version: &str) -> anyhow::Result<()> {
    let prefix_config = format!(
        r#"
        [services.prefix-{}]
        capsule = "wine/base"
        volumes = ["./prefixes/{}:/prefix"]
        env.WINE_PREFIX = "/prefix"
        env.WINEARCH = "win64"
        env.WINDOWS_VERSION = "{}"
        "#,
        name, name, windows_version
    );

    self.bolt_runtime.create_service_from_config(&prefix_config).await?;
    Ok(())
}
```

## Migration from Lutris/Bottles

### Configuration Migration

```rust
pub async fn migrate_lutris_games(&self, lutris_config_dir: &Path) -> anyhow::Result<()> {
    let lutris_games = self.parse_lutris_configs(lutris_config_dir)?;

    for game in lutris_games {
        let bolt_config = GameConfig {
            name: game.name,
            executable: game.exe,
            proton_version: "GE-Proton8-26".to_string(),
            wine_prefix: format!("/prefixes/{}", game.name.to_lowercase()),
            gpu_requirements: GpuRequirements {
                nvidia: true,
                amd: false,
                vram_min: "4Gi".to_string(),
                dx_version: "11".to_string(),
            },
            performance_profile: PerformanceProfile::default(),
        };

        self.create_game_template(bolt_config).await?;
    }

    println!("✅ Migrated {} games from Lutris", lutris_games.len());
    Ok(())
}
```

## Benefits of Bolt Integration

1. **Superior Performance**: Native container runtime optimized for gaming
2. **GPU Management**: Advanced GPU resource allocation and monitoring
3. **Snapshot System**: Safe game state management and mod testing
4. **Declarative Configuration**: Version-controlled gaming setups
5. **QUIC Networking**: Low-latency multiplayer gaming
6. **Wine/Proton Isolation**: Multiple Wine versions without conflicts
7. **Automatic Optimization**: Dynamic resource allocation based on game requirements

## Troubleshooting

- **Audio Issues**: Ensure PulseAudio socket is properly mounted
- **GPU Access**: Check device permissions and driver compatibility
- **Wine Prefix Corruption**: Use Bolt snapshots for quick recovery
- **Performance**: Monitor resource usage with built-in performance tools
- **Controller Support**: Verify uinput device access for game controllers

## Gaming Performance Monitoring

```rust
use bolt::runtime::metrics::GameMetrics;

pub async fn monitor_game_performance(&self, game_id: &str) -> anyhow::Result<()> {
    let metrics = GameMetrics::new(game_id);

    tokio::spawn(async move {
        loop {
            let stats = metrics.collect().await.unwrap();
            println!("🎮 {} - FPS: {}, GPU: {}%, CPU: {}%",
                game_id, stats.fps, stats.gpu_usage, stats.cpu_usage);

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    Ok(())
}
```