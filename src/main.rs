mod cli;
mod config;
mod runtime;
mod surge;
mod network;
mod gaming;
mod capsules;
mod builds;

use clap::Parser;
use cli::{Cli, Commands, SurgeCommands, GamingCommands, NetworkCommands};
use tracing::{info, warn};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive(level.parse()?))
        .init();

    info!("🚀 Bolt starting up...");

    match cli.command {
        Commands::Run { image, name, ports, env, volumes, detach } => {
            info!("Running container: {}", image);
            runtime::run_container(&image, name.as_deref(), &ports, &env, &volumes, detach).await?;
        }

        Commands::Build { path, tag, file } => {
            info!("Building image from: {}", path);
            runtime::build_image(&path, tag.as_deref(), &file).await?;
        }

        Commands::Pull { image } => {
            info!("Pulling image: {}", image);
            runtime::pull_image(&image).await?;
        }

        Commands::Push { image } => {
            info!("Pushing image: {}", image);
            runtime::push_image(&image).await?;
        }

        Commands::Ps { all } => {
            runtime::list_containers(all).await?;
        }

        Commands::Stop { containers } => {
            for container in containers {
                info!("Stopping container: {}", container);
                runtime::stop_container(&container).await?;
            }
        }

        Commands::Rm { containers, force } => {
            for container in containers {
                info!("Removing container: {}", container);
                runtime::remove_container(&container, force).await?;
            }
        }

        Commands::Surge { command } => {
            match command {
                SurgeCommands::Up { services, detach, force_recreate } => {
                    info!("Starting surge orchestration...");
                    surge::up(&cli.config, &services, detach, force_recreate).await?;
                }

                SurgeCommands::Down { services, volumes } => {
                    info!("Stopping surge services...");
                    surge::down(&cli.config, &services, volumes).await?;
                }

                SurgeCommands::Status => {
                    surge::status(&cli.config).await?;
                }

                SurgeCommands::Logs { service, follow, tail } => {
                    surge::logs(&cli.config, service.as_deref(), follow, tail).await?;
                }

                SurgeCommands::Scale { services } => {
                    surge::scale(&cli.config, &services).await?;
                }
            }
        }

        Commands::Gaming { command } => {
            match command {
                GamingCommands::Gpu { command } => {
                    gaming::handle_gpu_command(command).await?;
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
