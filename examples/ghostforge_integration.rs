use bolt::config::{
    AudioConfig, GamingConfig, GpuConfig, GpuGamingConfig, NvidiaConfig, PerformanceConfig,
    Service, WineConfig,
};
use bolt::{BoltFileBuilder, BoltRuntime, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let runtime = BoltRuntime::new()?;

    let gaming_config = GamingConfig {
        enabled: true,
        gpu_passthrough: true,
        nvidia_runtime: true,
        audio_passthrough: true,
        real_time_priority: true,
        gpu: Some(GpuConfig {
            runtime: Some("nvbind".to_string()),
            nvidia: Some(NvidiaConfig {
                device: Some(0),
                dlss: Some(true),
                reflex: Some(true),
                raytracing: Some(true),
                cuda: Some(false),
                ..Default::default()
            }),
            passthrough: Some(true),
            gaming: Some(GpuGamingConfig {
                profile: Some("gaming-ultra".to_string()),
                dlss_enabled: Some(true),
                rt_cores_enabled: Some(true),
                wine_optimizations: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        }),
        audio: Some(AudioConfig {
            system: "pipewire".to_string(),
            latency: Some("low".to_string()),
        }),
        wine: Some(WineConfig {
            proton: Some("8.0".to_string()),
            winver: Some("win10".to_string()),
            prefix: Some("/games/wine-prefix".to_string()),
            ..Default::default()
        }),
        performance: Some(PerformanceConfig {
            cpu_governor: Some("performance".to_string()),
            nice_level: Some(-10),
            rt_priority: Some(50),
        }),
        ..Default::default()
    };

    let boltfile = BoltFileBuilder::new("ghostforge-demo")
        .add_gaming_service(
            "steam",
            "ghcr.io/games-on-whales/steam:latest",
            gaming_config,
        )
        .add_service(
            "ollama",
            Service {
                image: Some("ollama/ollama".to_string()),
                ports: Some(vec!["11434:11434".to_string()]),
                volumes: Some(vec!["ollama_models:/root/.ollama".to_string()]),
                ..Default::default()
            },
        )
        .build();

    let config = bolt::BoltConfig::load()?;
    config.save_boltfile(&boltfile)?;

    let services = runtime.surge_status().await?;
    println!(
        "Ghostforge-style Boltfile written. Known services: {}",
        services.services.len()
    );

    Ok(())
}
