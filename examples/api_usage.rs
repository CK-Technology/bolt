use bolt::api::*;
use bolt::{BoltRuntime, BoltFileBuilder, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Example 1: Using the high-level BoltRuntime API
    let runtime = BoltRuntime::new()?;

    // Run a container
    runtime.run_container(
        "nginx:latest",
        Some("my-nginx"),
        &["8080:80".to_string()],
        &[],
        &[],
        false,
    ).await?;

    // List containers
    let containers = runtime.list_containers(false).await?;
    println!("Found {} containers", containers.len());

    // Example 2: Gaming container with NVIDIA GPU
    let gaming_config = GamingConfig {
        gpu: Some(bolt::config::GpuConfig {
            nvidia: Some(bolt::config::NvidiaConfig {
                device: Some(0),
                dlss: Some(true),
                raytracing: Some(true),
                cuda: Some(false),
            }),
            amd: None,
            passthrough: Some(true),
        }),
        audio: Some(bolt::config::AudioConfig {
            system: "pipewire".to_string(),
            latency: Some("low".to_string()),
        }),
        wine: Some(bolt::config::WineConfig {
            version: None,
            proton: Some("8.0".to_string()),
            winver: Some("win10".to_string()),
            prefix: Some("/games/wine-prefix".to_string()),
        }),
        performance: Some(bolt::config::PerformanceConfig {
            cpu_governor: Some("performance".to_string()),
            nice_level: Some(-10),
            rt_priority: Some(50),
        }),
    };

    // Example 3: Building Boltfiles programmatically
    let boltfile = BoltFileBuilder::new("my-gaming-project")
        .add_gaming_service("steam", "bolt://steam:latest", gaming_config)
        .build();

    // Save the Boltfile
    let config = BoltConfig::load()?;
    config.save_boltfile(&boltfile)?;

    // Example 4: Surge orchestration
    runtime.surge_up(&[], false, false).await?;
    let status = runtime.surge_status().await?;
    println!("Services: {}", status.services.len());

    // Example 5: Network management
    runtime.create_network("gaming-net", "bolt", Some("10.1.0.0/16")).await?;
    let networks = runtime.list_networks().await?;
    println!("Networks: {}", networks.len());

    println!("✅ Bolt API integration example completed!");
    Ok(())
}