use crate::config::{BoltConfig, BoltFile};
use crate::error::RuntimeError;
use crate::runtime;
use crate::runtime::unified::UnifiedRuntime;
use crate::{BoltError, Result};
use anyhow::anyhow;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub mod status_api;

/// Enhanced Surge with native runtime integration
pub async fn up_with_native_runtime(
    config: &BoltConfig,
    services: &[String],
    detach: bool,
    force_recreate: bool,
) -> Result<()> {
    info!("🚀 Surge orchestration starting with native runtime...");

    // Initialize unified runtime (native with fallback)
    let runtime = UnifiedRuntime::new().await?;

    let boltfile = config.load_boltfile().map_err(|e| {
        error!("Failed to load Boltfile: {}", e);
        BoltError::Other(anyhow!(
            "Cannot load Boltfile at {:?}: {}",
            config.boltfile_path,
            e
        ))
    })?;

    info!("📦 Project: {}", boltfile.project);

    if runtime.is_native() {
        info!("✅ Using Bolt native runtime");
    } else {
        info!("⚠️  Falling back to delegation runtime");
    }

    let target_services = if services.is_empty() {
        boltfile.services.keys().collect::<Vec<_>>()
    } else {
        services.iter().collect::<Vec<_>>()
    };

    info!("🎯 Target services: {:?}", target_services);
    debug!("Detached: {}, Force recreate: {}", detach, force_recreate);

    for service_name in target_services {
        if let Some(service) = boltfile.services.get(service_name.as_str()) {
            info!("🔧 Starting service: {}", service_name);

            if let Some(ref gaming) = service.gaming {
                info!("🎮 Gaming optimizations enabled for {}", service_name);
                setup_gaming_service(service_name, gaming).await?;
            }

            // Handle different service types
            if let Some(ref image) = service.image {
                info!("  📦 Image: {}", image);

                // Prepare container arguments
                let container_name = format!("{}_{}", boltfile.project, service_name);
                let ports = service.ports.as_ref().map(|p| p.as_slice()).unwrap_or(&[]);
                let env_vars = service
                    .env
                    .as_ref()
                    .map(|env| {
                        env.iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let volumes = service
                    .volumes
                    .as_ref()
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);

                // Stop existing container if force_recreate
                if force_recreate {
                    let _ = runtime.stop_container(&container_name).await;
                    let _ = runtime.remove_container(&container_name, true).await;
                }

                // Pull image if it doesn't exist locally
                if let Err(_) = runtime.pull_image(image).await {
                    warn!("Could not pull image {}, trying with local image", image);
                }

                // Check if this is a GPU-enabled service
                let container_id = if let Some(ref gaming) = service.gaming {
                    if gaming.gpu_passthrough || gaming.gpu.is_some() {
                        info!("🎮 Creating GPU-enabled gaming container for service: {}", service_name);

                        // Determine GPU configuration from service
                        let dlss_enabled = gaming.gpu.as_ref()
                            .and_then(|g| g.nvidia.as_ref())
                            .and_then(|n| n.dlss)
                            .unwrap_or(false);

                        let raytracing_enabled = gaming.gpu.as_ref()
                            .and_then(|g| g.nvidia.as_ref())
                            .and_then(|n| n.raytracing)
                            .unwrap_or(false);

                        // Use unified runtime to create GPU-optimized container
                        if runtime.is_native() {
                            // Create gaming container with nvbind if native runtime available
                            let native_runtime_arc = runtime.get_native_runtime();
                            let mut native_runtime = native_runtime_arc.write().await;
                            native_runtime.create_gaming_container(
                                image,
                                Some(&container_name),
                                dlss_enabled,
                                raytracing_enabled,
                            ).await?
                        } else {
                            // Fallback to standard container creation
                            runtime.run_container(
                                image,
                                Some(&container_name),
                                &ports.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                                &env_vars,
                                &volumes.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                                detach,
                            ).await?
                        }
                    } else {
                        // Standard container creation
                        runtime.run_container(
                            image,
                            Some(&container_name),
                            &ports.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                            &env_vars,
                            &volumes.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                            detach,
                        ).await?
                    }
                } else {
                    // Standard container creation for non-gaming services
                    runtime.run_container(
                        image,
                        Some(&container_name),
                        &ports.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                        &env_vars,
                        &volumes.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                        detach,
                    ).await?
                };

                if service.gaming.is_some() {
                    info!("✅ GPU-enabled service {} started successfully (container: {})", service_name, container_id);
                } else {
                    info!("✅ Service {} started successfully (container: {})", service_name, container_id);
                }
            } else if let Some(ref capsule) = service.capsule {
                info!("  🔧 Capsule: {}", capsule);

                let container_name = format!("{}_{}", boltfile.project, service_name);
                let bolt_image = format!("bolt://{}", capsule);

                let container_id = runtime.run_container(&bolt_image, Some(&container_name), &[], &[], &[], detach)
                    .await?;

                info!("✅ Capsule {} started successfully (container: {})", service_name, container_id);
            } else if let Some(ref build) = service.build {
                info!("  🔨 Build context: {}", build);

                let image_tag = format!("{}_{}", boltfile.project, service_name);
                let dockerfile = "Dockerfile"; // Default dockerfile name

                // Build the image using unified runtime
                runtime.build_image(build, Some(&image_tag), dockerfile).await?;

                // Run the built image
                let container_name = format!("{}_{}", boltfile.project, service_name);
                let ports = service.ports.as_ref().map(|p| p.as_slice()).unwrap_or(&[]);
                let env_vars = service
                    .env
                    .as_ref()
                    .map(|env| {
                        env.iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let volumes = service
                    .volumes
                    .as_ref()
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);

                let container_id = runtime.run_container(
                    &image_tag,
                    Some(&container_name),
                    &ports.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                    &env_vars,
                    &volumes.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                    detach,
                )
                .await?;

                info!("✅ Service {} built and started successfully (container: {})", service_name, container_id);
            } else {
                error!(
                    "Service {} has no image, capsule, or build configuration",
                    service_name
                );
            }
        } else {
            error!("Service '{}' not found in Boltfile", service_name);
        }
    }

    info!("🎉 Surge orchestration completed successfully!");

    Ok(())
}

pub async fn up(
    config: &BoltConfig,
    services: &[String],
    detach: bool,
    force_recreate: bool,
) -> Result<()> {
    info!("🚀 Surge orchestration starting up...");

    let boltfile = config.load_boltfile().map_err(|e| {
        error!("Failed to load Boltfile: {}", e);
        BoltError::Other(anyhow!(
            "Cannot load Boltfile at {:?}: {}",
            config.boltfile_path,
            e
        ))
    })?;

    info!("📦 Project: {}", boltfile.project);

    let target_services = if services.is_empty() {
        boltfile.services.keys().collect::<Vec<_>>()
    } else {
        services.iter().collect::<Vec<_>>()
    };

    info!("🎯 Target services: {:?}", target_services);
    debug!("Detached: {}, Force recreate: {}", detach, force_recreate);

    for service_name in target_services {
        if let Some(service) = boltfile.services.get(service_name.as_str()) {
            info!("🔧 Starting service: {}", service_name);

            if let Some(ref gaming) = service.gaming {
                info!("🎮 Gaming optimizations enabled for {}", service_name);
                setup_gaming_service(service_name, gaming).await?;
            }

            // Handle different service types
            if let Some(ref image) = service.image {
                info!("  📦 Image: {}", image);

                // Prepare container arguments
                let container_name = format!("{}_{}", boltfile.project, service_name);
                let ports = service.ports.as_ref().map(|p| p.as_slice()).unwrap_or(&[]);
                let env_vars = service
                    .env
                    .as_ref()
                    .map(|env| {
                        env.iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let volumes = service
                    .volumes
                    .as_ref()
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);

                // Stop existing container if force_recreate
                if force_recreate {
                    let _ = runtime::stop_container(&container_name).await;
                    let _ = runtime::remove_container(&container_name, true).await;
                }

                // Pull image if it doesn't exist locally
                if let Err(_) = runtime::pull_image(image).await {
                    warn!("Could not pull image {}, trying with local image", image);
                }

                // Start the container
                runtime::run_container(
                    image,
                    Some(&container_name),
                    ports,
                    &env_vars,
                    volumes,
                    detach,
                )
                .await?;

                info!("✅ Service {} started successfully", service_name);
            } else if let Some(ref capsule) = service.capsule {
                info!("  🔧 Capsule: {}", capsule);

                let container_name = format!("{}_{}", boltfile.project, service_name);
                let bolt_image = format!("bolt://{}", capsule);

                runtime::run_container(&bolt_image, Some(&container_name), &[], &[], &[], detach)
                    .await?;

                info!("✅ Capsule {} started successfully", service_name);
            } else if let Some(ref build) = service.build {
                info!("  🔨 Build context: {}", build);

                let image_tag = format!("{}_{}", boltfile.project, service_name);
                let dockerfile = "Dockerfile"; // Default dockerfile name

                // Build the image
                runtime::build_image(build, Some(&image_tag), dockerfile).await?;

                // Run the built image
                let container_name = format!("{}_{}", boltfile.project, service_name);
                let ports = service.ports.as_ref().map(|p| p.as_slice()).unwrap_or(&[]);
                let env_vars = service
                    .env
                    .as_ref()
                    .map(|env| {
                        env.iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let volumes = service
                    .volumes
                    .as_ref()
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);

                runtime::run_container(
                    &image_tag,
                    Some(&container_name),
                    ports,
                    &env_vars,
                    volumes,
                    detach,
                )
                .await?;

                info!("✅ Service {} built and started successfully", service_name);
            } else {
                error!(
                    "Service {} has no image, capsule, or build configuration",
                    service_name
                );
            }
        } else {
            error!("Service '{}' not found in Boltfile", service_name);
        }
    }

    Ok(())
}

pub async fn down(config: &BoltConfig, services: &[String], remove_volumes: bool) -> Result<()> {
    info!("🛑 Surge orchestration shutting down...");

    let boltfile = config.load_boltfile()?;
    info!("📦 Project: {}", boltfile.project);

    let target_services = if services.is_empty() {
        boltfile.services.keys().collect::<Vec<_>>()
    } else {
        services.iter().collect::<Vec<_>>()
    };

    debug!("Target services: {:?}", target_services);
    debug!("Remove volumes: {}", remove_volumes);

    for service_name in target_services {
        info!("🛑 Stopping service: {}", service_name);

        let container_name = format!("{}_{}", boltfile.project, service_name);

        // Stop the container
        if let Err(e) = runtime::stop_container(&container_name).await {
            warn!("Failed to stop container {}: {}", container_name, e);
        }

        // Remove the container
        if let Err(e) = runtime::remove_container(&container_name, false).await {
            warn!("Failed to remove container {}: {}", container_name, e);
        }

        // Remove volumes if requested
        if remove_volumes {
            info!("🗑️  Removing volumes for service: {}", service_name);
            // Volume removal logic would go here
        }

        info!("✅ Service {} stopped successfully", service_name);
    }

    Ok(())
}

pub async fn status(config: &BoltConfig) -> Result<()> {
    info!("📊 Checking surge status...");

    let boltfile = config.load_boltfile()?;
    let containers = runtime::list_containers_info(true).await?;

    println!("Project: {}", boltfile.project);
    println!();
    println!(
        "{:<15} {:<12} {:<15} {}",
        "SERVICE", "STATUS", "CONTAINER", "PORTS"
    );

    for (name, service) in &boltfile.services {
        let container_name = format!("{}_{}", boltfile.project, name);
        let container = containers.iter().find(|c| c.name == container_name);

        let (status, container_id) = match container {
            Some(c) => (c.status.clone(), c.id[..12].to_string()),
            None => ("not running".to_string(), "-".to_string()),
        };

        let ports = service
            .ports
            .as_ref()
            .map(|p| p.join(", "))
            .unwrap_or_else(|| "-".to_string());

        println!("{:<15} {:<12} {:<15} {}", name, status, container_id, ports);
    }

    Ok(())
}

pub async fn logs(
    config: &BoltConfig,
    service: Option<&str>,
    follow: bool,
    tail: Option<usize>,
) -> Result<()> {
    let boltfile = config.load_boltfile()?;
    let runtime = crate::runtime::detect_container_runtime().await?;

    match service {
        Some(service_name) => {
            info!("📜 Showing logs for service: {}", service_name);
            let container_name = format!("{}_{}", boltfile.project, service_name);

            let mut cmd = tokio::process::Command::new(&runtime);
            cmd.arg("logs");

            if follow {
                cmd.arg("-f");
            }

            if let Some(tail_count) = tail {
                cmd.arg("--tail").arg(tail_count.to_string());
            }

            cmd.arg(&container_name);

            let output = cmd.output().await?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(BoltError::Runtime(RuntimeError::StartFailed {
                    reason: format!("Failed to get logs: {}", stderr),
                }));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("{}", stdout);
        }
        None => {
            info!("📜 Showing logs for all services");
            for service_name in boltfile.services.keys() {
                println!("==> {} <==", service_name);
                let container_name = format!("{}_{}", boltfile.project, service_name);

                let mut cmd = tokio::process::Command::new(&runtime);
                cmd.arg("logs");

                if let Some(tail_count) = tail {
                    cmd.arg("--tail").arg(tail_count.to_string());
                }

                cmd.arg(&container_name);

                if let Ok(output) = cmd.output().await {
                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        println!("{}", stdout);
                    }
                }
                println!();
            }
        }
    }

    Ok(())
}

pub async fn scale(config: &BoltConfig, services: &[String]) -> Result<()> {
    info!("📈 Scaling services...");

    let boltfile = config.load_boltfile()?;

    for service_spec in services {
        let parts: Vec<&str> = service_spec.split('=').collect();
        if parts.len() != 2 {
            error!(
                "Invalid scale format: {} (expected service=count)",
                service_spec
            );
            continue;
        }

        let service_name = parts[0];
        let count: u32 = parts[1]
            .parse()
            .map_err(|_| anyhow!("Invalid count: {}", parts[1]))?;

        if !boltfile.services.contains_key(service_name) {
            error!("Service '{}' not found", service_name);
            continue;
        }

        info!("📈 Scaling {} to {} instances", service_name, count);

        // Get current running containers for this service
        let container_prefix = format!("{}_{}", boltfile.project, service_name);
        let containers = runtime::list_containers_info(true).await?;
        let current_containers: Vec<_> = containers
            .iter()
            .filter(|c| c.name.starts_with(&container_prefix))
            .collect();

        let current_count = current_containers.len() as u32;
        info!("Current instances: {}, Target: {}", current_count, count);

        if count > current_count {
            // Scale up - start new instances
            let service = boltfile.services.get(service_name).unwrap();
            for i in current_count..count {
                let instance_name = format!("{}_{}", container_prefix, i + 1);

                if let Some(ref image) = service.image {
                    let ports = service.ports.as_ref().map(|p| p.as_slice()).unwrap_or(&[]);
                    let env_vars = service
                        .env
                        .as_ref()
                        .map(|env| {
                            env.iter()
                                .map(|(k, v)| format!("{}={}", k, v))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let volumes = service
                        .volumes
                        .as_ref()
                        .map(|v| v.as_slice())
                        .unwrap_or(&[]);

                    runtime::run_container(
                        image,
                        Some(&instance_name),
                        ports,
                        &env_vars,
                        volumes,
                        true, // Always detached for scaling
                    )
                    .await?;

                    info!("✅ Started instance: {}", instance_name);
                }
            }
        } else if count < current_count {
            // Scale down - stop excess instances
            let containers_to_stop = current_count - count;
            for i in 0..containers_to_stop {
                if let Some(container) = current_containers.get(i as usize) {
                    let _ = runtime::stop_container(&container.name).await;
                    let _ = runtime::remove_container(&container.name, false).await;
                    info!("✅ Stopped instance: {}", container.name);
                }
            }
        }

        info!("✅ Service {} scaled to {} instances", service_name, count);
    }

    Ok(())
}

async fn setup_gaming_service(
    service_name: &str,
    gaming_config: &crate::config::GamingConfig,
) -> Result<()> {
    info!("🎮 Setting up gaming optimizations for {}", service_name);

    if let Some(ref gpu) = gaming_config.gpu {
        info!("  🖥️  GPU configuration detected");
        if let Some(ref nvidia) = gpu.nvidia {
            info!("    🟢 NVIDIA GPU (device: {:?})", nvidia.device);
            if nvidia.dlss == Some(true) {
                info!("    ✨ DLSS enabled");
            }
            if nvidia.raytracing == Some(true) {
                info!("    🌟 Ray tracing enabled");
            }
        }
        if let Some(ref amd) = gpu.amd {
            info!("    🔴 AMD GPU (device: {:?})", amd.device);
        }
    }

    if let Some(ref audio) = gaming_config.audio {
        info!("  🔊 Audio system: {}", audio.system);
    }

    if let Some(ref wine) = gaming_config.wine {
        info!("  🍷 Wine configuration");
        if let Some(ref proton) = wine.proton {
            info!("    Proton version: {}", proton);
        }
    }

    if let Some(ref perf) = gaming_config.performance {
        info!("  ⚡ Performance tuning enabled");
        if let Some(ref governor) = perf.cpu_governor {
            info!("    CPU governor: {}", governor);
        }
    }

    Ok(())
}

// API-only functions for library usage
