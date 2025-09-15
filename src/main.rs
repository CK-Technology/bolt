mod cli;

use clap::Parser;
use cli::{Cli, Commands, SurgeCommands, GamingCommands, NetworkCommands};
use tracing::info;
use anyhow::Result;
use bolt::{BoltConfig, BoltRuntime, surge, gaming, network};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive(level.parse()?))
        .init();

    info!("🚀 Bolt starting up...");

    // Create BoltConfig from CLI config path
    let mut bolt_config = BoltConfig::load()?;
    bolt_config.boltfile_path = std::path::PathBuf::from(&cli.config);
    bolt_config.verbose = cli.verbose;

    let runtime = BoltRuntime::new()?;

    match cli.command {
        Commands::Run { image, name, ports, env, volumes, detach } => {
            info!("Running container: {}", image);
            runtime.run_container(&image, name.as_deref(), &ports, &env, &volumes, detach).await?;
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
            for container in containers {
                println!("{}: {} ({})", container.name, container.image, container.status);
            }
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

        Commands::Surge { command } => {
            match command {
                SurgeCommands::Up { services, detach, force_recreate } => {
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
                        println!("  {}: {} ({})", service.name, service.status, service.replicas);
                    }
                }

                SurgeCommands::Logs { service, follow, tail } => {
                    surge::logs(&bolt_config, service.as_deref(), follow, tail).await?;
                }

                SurgeCommands::Scale { services } => {
                    surge::scale(&bolt_config, &services).await?;
                }
            }
        }

        Commands::Gaming { command } => {
            match command {
                GamingCommands::Gpu { command } => {
                    let gaming_command = match command {
                        cli::GpuCommands::List => gaming::GpuCommands::List,
                        cli::GpuCommands::Nvidia { device, dlss, raytracing } => gaming::GpuCommands::Nvidia { device, dlss, raytracing },
                        cli::GpuCommands::Amd { device } => gaming::GpuCommands::Amd { device },
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
            }
        }

        Commands::Network { command } => {
            match command {
                NetworkCommands::Create { name, driver, subnet } => {
                    network::create_network(&name, &driver, subnet.as_deref()).await?;
                }

                NetworkCommands::List => {
                    network::list_networks().await?;
                }

                NetworkCommands::Remove { name } => {
                    network::remove_network(&name).await?;
                }
            }
        }
    }

    Ok(())
}
