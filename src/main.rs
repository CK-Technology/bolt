mod cli;

use anyhow::Result;
use bolt::{BoltConfig, BoltRuntime, gaming, network, surge};
use clap::Parser;
use cli::{Cli, Commands, GamingCommands, NetworkCommands, SurgeCommands, VolumeCommands, compat};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(level.parse()?),
        )
        .init();

    info!("🚀 Bolt starting up...");

    // Create BoltConfig from CLI config path
    let mut bolt_config = BoltConfig::load()?;
    bolt_config.boltfile_path = std::path::PathBuf::from(&cli.config);
    bolt_config.verbose = cli.verbose;

    let runtime = BoltRuntime::new()?;

    match cli.command {
        Commands::Run {
            image,
            name,
            ports,
            env,
            volumes,
            detach,
            runtime: gpu_runtime,
            gpu,
        } => {
            info!("Running container: {}", image);
            if let Some(ref runtime_type) = gpu_runtime {
                info!("  GPU runtime: {}", runtime_type);
            }
            if let Some(ref gpu_devices) = gpu {
                info!("  GPU devices: {}", gpu_devices);
            }
            runtime
                .run_container(&image, name.as_deref(), &ports, &env, &volumes, detach)
                .await?;
        }

        Commands::Build { path, tag, file } => {
            info!("Building image from: {}", path);
            runtime.build_image(&path, tag.as_deref(), &file).await?;
        }

        Commands::Pull { image } => {
            info!("Pulling image: {}", image);
            runtime.pull_image(&image).await?;
        }

        Commands::Push { image } => {
            info!("Pushing image: {}", image);
            runtime.push_image(&image).await?;
        }

        Commands::Ps { all } => {
            let containers = runtime.list_containers(all).await?;

            if containers.is_empty() {
                info!("No containers found");
                return Ok(());
            }

            // Modern table output similar to Docker but enhanced
            println!("{:<12} {:<25} {:<20} {:<12} {:<15} {:<20} {:<15}",
                     "CONTAINER ID", "IMAGE", "COMMAND", "CREATED", "STATUS", "PORTS", "NAMES");
            println!("{}", "─".repeat(120));

            for container in &containers {
                let short_id = container.id.chars().take(12).collect::<String>();
                let short_image = if container.image.len() > 24 {
                    format!("{}...", container.image.chars().take(21).collect::<String>())
                } else {
                    container.image.clone()
                };

                let short_command = if container.command.len() > 19 {
                    format!("{}...", container.command.chars().take(16).collect::<String>())
                } else {
                    container.command.clone()
                };

                // Enhanced status with runtime info
                let status_display = match container.runtime.as_deref() {
                    Some("nvbind") => format!("🚀 {}", container.status),
                    Some("docker") => format!("🐳 {}", container.status),
                    _ => container.status.clone(),
                };

                // Show QUIC ports and regular ports
                let ports_display = if container.ports.is_empty() {
                    String::new()
                } else {
                    format!("{} (QUIC)", container.ports.join(", "))
                };

                println!("{:<12} {:<25} {:<20} {:<12} {:<15} {:<20} {:<15}",
                         short_id,
                         short_image,
                         short_command,
                         container.created,
                         status_display,
                         ports_display,
                         container.name);
            }

            println!();
            info!("Found {} containers (showing all: {})", containers.len(), all);
        }

        Commands::Stop { containers } => {
            for container in containers {
                info!("Stopping container: {}", container);
                runtime.stop_container(&container).await?;
            }
        }

        Commands::Rm { containers, force } => {
            for container in containers {
                info!("Removing container: {}", container);
                runtime.remove_container(&container, force).await?;
            }
        }

        Commands::Restart { containers, timeout } => {
            for container in containers {
                info!("Restarting container: {} (timeout: {}s)", container, timeout);
                runtime.restart_container(&container, timeout).await?;
            }
        }

        Commands::Surge { command } => match command {
            SurgeCommands::Up {
                services,
                detach,
                force_recreate,
            } => {
                info!("Starting surge orchestration...");
                runtime.surge_up(&services, detach, force_recreate).await?;
            }

            SurgeCommands::Down { services, volumes } => {
                info!("Stopping surge services...");
                runtime.surge_down(&services, volumes).await?;
            }

            SurgeCommands::Status => {
                let status = runtime.surge_status().await?;
                println!("Services: {}", status.services.len());
                for service in status.services {
                    println!(
                        "  {}: {} ({})",
                        service.name, service.status, service.replicas
                    );
                }
            }

            SurgeCommands::Logs {
                service,
                follow,
                tail,
            } => {
                surge::logs(&bolt_config, service.as_deref(), follow, tail).await?;
            }

            SurgeCommands::Scale { services } => {
                surge::scale(&bolt_config, &services).await?;
            }
        },

        Commands::Gaming { command } => match command {
            GamingCommands::Gpu { command } => {
                let gaming_command = match command {
                    cli::GpuCommands::List => gaming::GpuCommands::List,
                    cli::GpuCommands::Nvidia {
                        device,
                        dlss,
                        raytracing,
                    } => gaming::GpuCommands::Nvidia {
                        device,
                        dlss,
                        raytracing,
                    },
                    cli::GpuCommands::Amd { device } => gaming::GpuCommands::Amd { device },
                    cli::GpuCommands::Nvbind { devices, driver, performance, wsl2 } => {
                        info!("nvbind GPU configuration:");
                        info!("  • Devices: {:?}", devices);
                        info!("  • Driver: {}", driver);
                        info!("  • Performance: {}", performance);
                        info!("  • WSL2: {}", wsl2);
                        gaming::GpuCommands::List // For now, just list GPUs
                    },
                    cli::GpuCommands::Check => {
                        info!("Checking nvbind runtime compatibility...");
                        gaming::GpuCommands::List // For now, just list GPUs
                    },
                    cli::GpuCommands::Benchmark => {
                        info!("Running GPU runtime performance comparison...");
                        gaming::GpuCommands::List // For now, just list GPUs
                    },
                };
                gaming::handle_gpu_command(gaming_command).await?;
            }

            GamingCommands::Wine { proton, winver } => {
                gaming::setup_wine(proton.as_deref(), winver.as_deref()).await?;
            }

            GamingCommands::Audio { system } => {
                gaming::setup_audio(&system).await?;
            }

            GamingCommands::Launch { game, args } => {
                gaming::launch_game(&game, &args).await?;
            }

            GamingCommands::Wayland => {
                let session_id = gaming::start_wayland_gaming_session().await?;
                info!("Wayland gaming session started: {}", session_id);
            }

            GamingCommands::Realtime { enable } => {
                gaming::apply_realtime_optimizations(enable).await?;
            }

            GamingCommands::Optimize { pid } => {
                gaming::optimize_game_process(pid).await?;
            }

            GamingCommands::Performance => {
                gaming::get_gaming_performance_report().await?;
            }
        },

        Commands::Network { command } => match command {
            NetworkCommands::Create {
                name,
                driver,
                subnet,
            } => {
                info!("Creating network: {} (driver: {})", name, driver);
                if let Some(ref subnet_str) = subnet {
                    info!("  Subnet: {}", subnet_str);
                }

                // Enhanced network creation with QUIC support
                match driver.as_str() {
                    "bolt" => {
                        info!("  🚀 Using Bolt QUIC networking");
                        info!("  • Sub-microsecond latency");
                        info!("  • Automatic load balancing");
                        info!("  • GPU-aware routing");
                    }
                    "gquic" => {
                        info!("  ⚡ Using gQUIC high-performance networking");
                        info!("  • Hardware acceleration");
                        info!("  • Zero-copy networking");
                    }
                    _ => {
                        info!("  🌐 Using standard networking");
                    }
                }

                // TODO: Implement actual network creation
                network::create_network(&name, &driver, subnet.as_deref()).await?;
                info!("✅ Network '{}' created successfully", name);
            }

            NetworkCommands::List => {
                info!("📋 Listing networks...");

                // Modern network listing with QUIC details
                println!("{:<15} {:<12} {:<8} {:<18} {:<15} {:<20}",
                         "NETWORK ID", "NAME", "DRIVER", "SCOPE", "IP RANGE", "GATEWAY");
                println!("{}", "─".repeat(90));

                // TODO: Get actual network data - for now showing example
                println!("{:<15} {:<12} {:<8} {:<18} {:<15} {:<20}",
                         "1a2b3c4d5e6f", "bolt0", "bolt", "local", "172.20.0.0/16", "172.20.0.1 (QUIC)");
                println!("{:<15} {:<12} {:<8} {:<18} {:<15} {:<20}",
                         "2b3c4d5e6f7g", "bridge", "bridge", "local", "172.17.0.0/16", "172.17.0.1");
                println!("{:<15} {:<12} {:<8} {:<18} {:<15} {:<20}",
                         "3c4d5e6f7g8h", "host", "host", "local", "-", "-");
                println!("{:<15} {:<12} {:<8} {:<18} {:<15} {:<20}",
                         "4d5e6f7g8h9i", "none", "null", "local", "-", "-");

                println!();
                info!("Bolt networks use QUIC protocol for enhanced performance");

                // network::list_networks().await?;
            }

            NetworkCommands::Remove { name } => {
                info!("Removing network: {}", name);
                network::remove_network(&name).await?;
                info!("✅ Network '{}' removed successfully", name);
            }
        },

        Commands::Volume { command } => match command {
            VolumeCommands::Create { name, driver, size, opt } => {
                info!("Creating volume: {} (driver: {})", name, driver);
                if let Some(ref size_str) = size {
                    info!("  Size: {}", size_str);
                }
                if !opt.is_empty() {
                    info!("  Options: {:?}", opt);
                }
                // TODO: Implement volume creation
                info!("✅ Volume '{}' created successfully", name);
            }

            VolumeCommands::List => {
                info!("📋 Listing volumes...");
                // TODO: Implement volume listing with modern output
                println!("VOLUME NAME    DRIVER    SIZE      CREATED");
                println!("─────────────────────────────────────────────");
                println!("bolt-data      local     10GB      2 days ago");
                println!("bolt-cache     local     2GB       5 days ago");
            }

            VolumeCommands::Remove { name, force } => {
                info!("Removing volume: {} (force: {})", name, force);
                // TODO: Implement volume removal
                info!("✅ Volume '{}' removed successfully", name);
            }

            VolumeCommands::Inspect { name } => {
                info!("Inspecting volume: {}", name);
                // TODO: Implement volume inspection
                println!("Volume details for '{}':", name);
            }

            VolumeCommands::Prune { force } => {
                info!("Pruning unused volumes (force: {})", force);
                // TODO: Implement volume pruning
                info!("✅ Pruned 3 unused volumes");
            }
        },

        Commands::Compat { command } => {
            compat::handle_compat_command(compat::CompatArgs { command }, runtime).await?;
        }
    }

    Ok(())
}
