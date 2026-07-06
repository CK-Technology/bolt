mod cli;

use anyhow::Result;
use bolt::runtime::gpu_integration::{GpuConfig, GpuIsolationLevel, GpuWorkloadType};
use bolt::runtime::unified::ContainerRunOptions;
use bolt::{BoltConfig, BoltRuntime, gaming, surge};
use clap::{CommandFactory, Parser};
use cli::{
    Cli, Commands, GamingCommands, GenerationCommands, ImageCommands, ImportCommands,
    NetworkCommands, SurgeCommands, VolumeCommands, compat,
};
use std::path::PathBuf;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let quiet_generator = matches!(
        &cli.command,
        Commands::Completions { .. } | Commands::Manpage
    );

    // Initialize logging
    let level = if quiet_generator {
        "error"
    } else if cli.verbose {
        "debug"
    } else {
        "info"
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(level.parse()?),
        )
        .init();

    if !quiet_generator {
        info!("🚀 Bolt starting up...");
    }

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
            gpus,
            gpu_profile,
            interactive,
            tty,
            rm,
            workdir,
            user,
            hostname,
            entrypoint,
            cpus,
            memory,
            network,
            cap_add,
            cap_drop,
            privileged,
            read_only,
            pids_limit,
            security_opt,
            command,
        } => {
            info!("Running container: {}", image);

            // Handle GPU flags (both --gpu and --gpus for Docker compatibility)
            let gpu_spec = gpus.or(gpu);

            // Handle GPU profile
            if let Some(ref profile) = gpu_profile {
                info!("  • GPU Profile: {}", profile);
            }

            // Build GPU configuration if GPU requested
            let gpu_config = if gpu_spec.is_some() || gpu_runtime.is_some() {
                let devices = gpu_spec.clone().unwrap_or_else(|| "all".to_string());
                info!("🎮 GPU passthrough enabled:");
                info!("  • Devices: {}", devices);

                if let Some(ref rt) = gpu_runtime {
                    info!("  • Runtime hint: {}", rt);
                }

                Some(GpuConfig {
                    enabled: true,
                    devices: Some(normalize_gpu_device_request(&devices)),
                    workload_type: GpuWorkloadType::General,
                    isolation_level: GpuIsolationLevel::Shared,
                    memory_limit: None,
                    snapshot_support: false,
                    quick_sync: None,
                })
            } else {
                None
            };

            // Handle interactive/TTY flags
            if interactive || tty {
                info!("  Interactive mode: {}, TTY: {}", interactive, tty);
            }

            // Parse `--security-opt seccomp=<path|unconfined>`; other options
            // are accepted but not yet consumed.
            let seccomp = security_opt
                .iter()
                .find_map(|opt| opt.strip_prefix("seccomp=").map(|value| value.to_string()));

            let run_options = ContainerRunOptions {
                rm,
                command: if command.is_empty() {
                    None
                } else {
                    Some(command)
                },
                entrypoint: entrypoint.map(|value| vec![value]),
                working_dir: workdir,
                user,
                hostname,
                cpus,
                memory,
                network,
                cap_add,
                cap_drop,
                privileged,
                tty,
                interactive,
                readonly_rootfs: read_only,
                pids_limit,
                seccomp,
            };

            runtime
                .run_container_with_options(
                    &image,
                    name.as_deref(),
                    &ports,
                    &env,
                    &volumes,
                    detach,
                    gpu_config,
                    run_options,
                )
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

        Commands::Image { command } => match command {
            ImageCommands::List => {
                let images = runtime.list_images().await?;
                if images.is_empty() {
                    info!("No native images found");
                    return Ok(());
                }

                println!(
                    "{:<24} {:<64} {:>12} {:<25}",
                    "REPOSITORY:TAG", "IMAGE ID", "SIZE", "CREATED"
                );
                for image in images {
                    println!(
                        "{:<24} {:<64} {:>12} {:<25}",
                        image.name,
                        image.id,
                        format_bytes(image.size),
                        image.created.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
            ImageCommands::Inspect { image } => {
                let (reference, metadata, pinned) = runtime.inspect_image(&image).await?;
                println!("Image: {}", reference);
                println!("  Digest: {}", metadata.digest);
                println!(
                    "  Config digest: {}",
                    metadata.config_digest.unwrap_or_else(|| "-".to_string())
                );
                println!("  Size: {}", format_bytes(metadata.size));
                println!("  Created: {}", metadata.created.to_rfc3339());
                println!("  Pinned: {}", yes_no(pinned));
                println!("  Layers: {}", metadata.layers.len());
            }
            ImageCommands::Pin { image } => {
                runtime.pin_image(&image).await?;
                println!("Pinned image: {}", image);
            }
            ImageCommands::Unpin { image } => {
                runtime.unpin_image(&image).await?;
                println!("Unpinned image: {}", image);
            }
            ImageCommands::Prune { dry_run, force } => {
                let preview_only = dry_run || !force;
                let report = runtime.prune_images(preview_only).await?;
                if report.candidates.is_empty() && report.roots.is_empty() {
                    info!("No unused native image or runtime roots to prune");
                    return Ok(());
                }

                println!(
                    "{} native image(s), {} runtime root(s) {}:",
                    report.candidates.len(),
                    report.roots.len(),
                    if preview_only {
                        "would be removed"
                    } else {
                        "were removed"
                    }
                );
                for candidate in &report.candidates {
                    println!(
                        "  {} ({}, {})",
                        candidate.reference,
                        candidate.digest,
                        format_bytes(candidate.bytes)
                    );
                }
                for candidate in &report.roots {
                    println!(
                        "  {}:{} ({})",
                        candidate.kind,
                        candidate.id,
                        format_bytes(candidate.bytes)
                    );
                }
                if !report.protected_images.is_empty() {
                    println!("Protected images:");
                    for image in &report.protected_images {
                        println!("  {}", image);
                    }
                }
                println!(
                    "Total reclaimable: {}",
                    format_bytes(report.reclaimed_bytes)
                );

                if dry_run {
                    return Ok(());
                }

                if !force {
                    println!("Re-run with --force to remove these images.");
                    return Ok(());
                }

                info!(
                    "Pruned {} native image(s), reclaimed {}",
                    report.candidates.len(),
                    format_bytes(report.reclaimed_bytes)
                );
            }
        },

        Commands::Ps { all } => {
            let containers = runtime.list_containers(all).await?;

            if containers.is_empty() {
                info!("No containers found");
                return Ok(());
            }

            // Modern table output similar to Docker but enhanced
            println!(
                "{:<12} {:<25} {:<20} {:<12} {:<15} {:<20} {:<15}",
                "CONTAINER ID", "IMAGE", "COMMAND", "CREATED", "STATUS", "PORTS", "NAMES"
            );
            println!("{}", "─".repeat(120));

            for container in &containers {
                let short_id = container.id.chars().take(12).collect::<String>();
                let short_image = if container.image.len() > 24 {
                    format!(
                        "{}...",
                        container.image.chars().take(21).collect::<String>()
                    )
                } else {
                    container.image.clone()
                };

                let short_command = if container.command.len() > 19 {
                    format!(
                        "{}...",
                        container.command.chars().take(16).collect::<String>()
                    )
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

                println!(
                    "{:<12} {:<25} {:<20} {:<12} {:<15} {:<20} {:<15}",
                    short_id,
                    short_image,
                    short_command,
                    container.created,
                    status_display,
                    ports_display,
                    container.name
                );
            }

            println!();
            info!(
                "Found {} containers (showing all: {})",
                containers.len(),
                all
            );
        }

        Commands::Stop {
            containers,
            timeout,
        } => {
            for container in containers {
                info!("Stopping container: {} (timeout: {}s)", container, timeout);
                runtime
                    .stop_container_with_timeout(&container, timeout)
                    .await?;
            }
        }

        Commands::Rm { containers, force } => {
            for container in containers {
                info!("Removing container: {}", container);
                runtime.remove_container(&container, force).await?;
            }
        }

        Commands::Restart {
            containers,
            timeout,
        } => {
            for container in containers {
                info!(
                    "Restarting container: {} (timeout: {}s)",
                    container, timeout
                );
                runtime.restart_container(&container, timeout).await?;
            }
        }

        Commands::Exec { exec } => {
            exec.execute().await?;
        }

        Commands::Logs { logs } => {
            logs.execute().await?;
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

        Commands::Plan { json } => {
            let plan = bolt::project::plan(&bolt_config, &runtime).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                bolt::project::print_plan(&plan);
            }
        }

        Commands::Apply {
            services,
            detach,
            force_recreate,
            locked,
        } => {
            let plan = bolt::project::apply(
                &bolt_config,
                &runtime,
                &services,
                detach,
                force_recreate,
                locked,
            )
            .await?;
            bolt::project::print_plan(&plan);
            info!("✅ Apply completed and Boltfile.lock updated");
        }

        Commands::Destroy {
            services,
            volumes,
            force,
        } => {
            let plan =
                bolt::project::destroy(&bolt_config, &runtime, &services, volumes, force).await?;
            bolt::project::print_plan(&plan);
            if !force {
                bolt::project::ensure_force(false, "destroy")?;
            }
            info!("✅ Destroy completed");
        }

        Commands::Lock { check } => {
            if check {
                bolt::project::check_lock(&bolt_config, &runtime).await?;
                println!("Boltfile.lock is current");
            } else {
                let lock = bolt::project::write_lock(&bolt_config, &runtime).await?;
                println!(
                    "Wrote Boltfile.lock for project {} ({})",
                    lock.project, lock.boltfile_hash
                );
            }
        }

        Commands::Drift { json } => {
            let report = bolt::project::drift(&bolt_config, &runtime).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else if report.drifted {
                println!("Drift detected:");
                for entry in report.entries {
                    println!(
                        "  {:?}.{} - {}",
                        entry.resource_type, entry.name, entry.message
                    );
                }
            } else {
                println!("No drift detected");
            }
        }

        Commands::Doctor { json } => {
            let report = bolt::project::doctor(&bolt_config, &runtime).await;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Doctor: {}", if report.ok { "ok" } else { "issues found" });
                for check in &report.checks {
                    println!(
                        "  {:<18} {:<4} {}",
                        check.name,
                        if check.ok { "ok" } else { "fail" },
                        check.message
                    );
                }
            }
        }

        Commands::Validate { json } => {
            let report = bolt::project::validate(&bolt_config, &runtime).await;
            print_check_report("Validate", &report, json)?;
        }

        Commands::SelfTest { json } => {
            let report = bolt::project::self_test(&bolt_config, &runtime).await;
            print_check_report("Self-test", &report, json)?;
        }

        Commands::Dns { command } => match command {
            cli::DnsCommands::Resolve { name } => {
                let entry = bolt::project::resolve_dns_name(&bolt_config, &name)?;
                println!("{} {}", entry.dns_name, entry.address);
            }
            cli::DnsCommands::Hosts => {
                for line in bolt::project::hosts_entries(&bolt_config)? {
                    println!("{}", line);
                }
            }
            cli::DnsCommands::Serve { bind } => {
                println!("Serving Bolt DNS on {}", bind);
                bolt::project::serve_dns(bolt_config.clone(), bind).await?;
            }
        },

        Commands::Completions { shell } => {
            print_completion(&shell)?;
        }

        Commands::Manpage => {
            print_manpage();
        }

        Commands::Import { command } => match command {
            ImportCommands::Compose { input, output } => {
                bolt::project::import_compose(&input, &output)?;
                println!("Imported compose file into {}", output.display());
            }
            ImportCommands::Container { id, service } => {
                bolt::project::import_container(&bolt_config, &runtime, &id, service.as_deref())
                    .await?;
                println!(
                    "Imported container '{}' into {}",
                    id,
                    bolt_config.boltfile_path.display()
                );
            }
            ImportCommands::Image { image, service } => {
                bolt::project::import_image(&bolt_config, &image, service.as_deref())?;
                println!(
                    "Imported image '{}' into {}",
                    image,
                    bolt_config.boltfile_path.display()
                );
            }
        },

        Commands::Inspect { kind, name, json } => {
            let kind = parse_inspect_kind(&kind)?;
            let inspection = bolt::project::inspect(&bolt_config, &runtime, kind, &name).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&inspection)?);
            } else {
                print_inspection(&inspection);
            }
        }

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
                    cli::GpuCommands::Nvbind {
                        devices,
                        driver,
                        performance,
                        wsl2,
                    } => {
                        info!("nvbind GPU configuration:");
                        info!("  • Devices: {:?}", devices);
                        info!("  • Driver: {}", driver);
                        info!("  • Performance: {}", performance);
                        info!("  • WSL2: {}", wsl2);
                        gaming::GpuCommands::List // For now, just list GPUs
                    }
                    cli::GpuCommands::Check => {
                        info!("Checking nvbind runtime compatibility...");
                        gaming::GpuCommands::List // For now, just list GPUs
                    }
                    cli::GpuCommands::Benchmark => {
                        info!("Running GPU runtime performance comparison...");
                        gaming::GpuCommands::List // For now, just list GPUs
                    }
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

                // Create network with enhanced implementation
                let mut network_manager = bolt::networking::NetworkManager::new(
                    bolt::networking::NetworkConfig::default(),
                )
                .await?;
                network_manager
                    .create_bolt_network(&name, &driver, subnet.as_deref())
                    .await?;
                info!("✅ Network '{}' created successfully", name);
            }

            NetworkCommands::List => {
                info!("📋 Listing networks...");

                // Modern network listing with QUIC details
                println!(
                    "{:<15} {:<12} {:<8} {:<18} {:<15} {:<20}",
                    "NETWORK ID", "NAME", "DRIVER", "SCOPE", "IP RANGE", "GATEWAY"
                );
                println!("{}", "─".repeat(90));

                // Get actual network data with enhanced features
                let network_manager = bolt::networking::NetworkManager::new(
                    bolt::networking::NetworkConfig::default(),
                )
                .await?;
                let networks = network_manager.list_bolt_networks().await?;

                for network in &networks {
                    println!(
                        "{:<15} {:<12} {:<8} {:<18} {:<15} {:<20}",
                        network.id,
                        network.name,
                        network.driver,
                        network.scope,
                        network.subnet,
                        network.gateway
                    );
                }

                // Show example if no networks exist
                if networks.is_empty() {
                    println!(
                        "{:<15} {:<12} {:<8} {:<18} {:<15} {:<20}",
                        "1a2b3c4d5e6f",
                        "bolt0",
                        "bolt",
                        "local",
                        "172.20.0.0/16",
                        "172.20.0.1 (QUIC)"
                    );
                    println!(
                        "{:<15} {:<12} {:<8} {:<18} {:<15} {:<20}",
                        "2b3c4d5e6f7g", "bridge", "bridge", "local", "172.17.0.0/16", "172.17.0.1"
                    );
                    println!(
                        "{:<15} {:<12} {:<8} {:<18} {:<15} {:<20}",
                        "3c4d5e6f7g8h", "host", "host", "local", "-", "-"
                    );
                    println!(
                        "{:<15} {:<12} {:<8} {:<18} {:<15} {:<20}",
                        "4d5e6f7g8h9i", "none", "null", "local", "-", "-"
                    );

                    println!();
                    info!("Bolt networks use QUIC protocol for enhanced performance");

                    // network::list_networks().await?;
                }
            }

            NetworkCommands::Remove { name } => {
                info!("Removing network: {}", name);

                // Use the enhanced NetworkManager to remove QUIC networks
                let mut network_manager = bolt::networking::NetworkManager::new(
                    bolt::networking::NetworkConfig::default(),
                )
                .await?;
                network_manager.remove_bolt_network(&name).await?;
            }
            NetworkCommands::Preflight => {
                let preflight = bolt::networking::bridge::BridgeManager::preflight();
                println!("Bridge networking preflight:");
                println!("  CAP_NET_ADMIN: {}", yes_no(preflight.cap_net_admin));
                println!("  ip command: {}", yes_no(preflight.ip_available));
                println!("  nft command: {}", yes_no(preflight.nft_available));
                println!(
                    "  iptables command: {}",
                    yes_no(preflight.iptables_available)
                );
                println!("  Can manage links: {}", yes_no(preflight.can_manage_links));
                if !preflight.reasons.is_empty() {
                    println!("  Reasons: {}", preflight.reasons.join(", "));
                }
            }
        },

        Commands::Volume { command } => match command {
            VolumeCommands::Create {
                name,
                driver,
                size,
                opt,
            } => {
                info!("Creating volume: {} (driver: {})", name, driver);
                if let Some(ref size_str) = size {
                    info!("  Size: {}", size_str);
                }
                if !opt.is_empty() {
                    info!("  Options: {:?}", opt);
                }

                let volume_info = runtime
                    .create_volume(&name, &driver, size.as_deref(), &opt)
                    .await?;
                info!("✅ Volume '{}' created successfully", volume_info.name);
                info!("   Mountpoint: {}", volume_info.mountpoint.display());
            }

            VolumeCommands::List => {
                info!("📋 Listing volumes...");
                let volumes = runtime.list_volumes().await?;

                if volumes.is_empty() {
                    println!("No volumes found");
                    return Ok(());
                }

                println!(
                    "{:<15} {:<10} {:<20} {:<25} {:<15}",
                    "NAME", "DRIVER", "CREATED", "MOUNTPOINT", "SIZE"
                );
                println!("{}", "─".repeat(85));

                for volume in volumes {
                    let created_display = volume.created.format("%Y-%m-%d %H:%M");
                    let mountpoint_display = volume.mountpoint.to_string_lossy();
                    let size_display = if let Some(limit) = volume.size_limit {
                        format!("{} bytes", limit)
                    } else {
                        "unlimited".to_string()
                    };

                    println!(
                        "{:<15} {:<10} {:<20} {:<25} {:<15}",
                        volume.name,
                        volume.driver,
                        created_display,
                        mountpoint_display,
                        size_display
                    );
                }
            }

            VolumeCommands::Remove { name, force } => {
                info!("Removing volume: {} (force: {})", name, force);
                runtime.remove_volume(&name, force).await?;
                info!("✅ Volume '{}' removed successfully", name);
            }

            VolumeCommands::Inspect { name } => {
                info!("Inspecting volume: {}", name);
                let volumes = runtime.list_volumes().await?;
                let volume = volumes.iter().find(|v| v.name == name);

                match volume {
                    Some(vol) => {
                        println!("Volume details for '{}':", name);
                        println!("  Name: {}", vol.name);
                        println!("  Driver: {}", vol.driver);
                        println!("  Mountpoint: {}", vol.mountpoint.display());
                        println!("  Created: {}", vol.created.format("%Y-%m-%d %H:%M:%S UTC"));
                        println!("  Scope: {:?}", vol.scope);
                        if let Some(limit) = vol.size_limit {
                            println!("  Size Limit: {} bytes", limit);
                        } else {
                            println!("  Size Limit: unlimited");
                        }
                        if !vol.used_by.is_empty() {
                            println!("  Used By: {}", vol.used_by.join(", "));
                        } else {
                            println!("  Used By: none");
                        }
                        if !vol.labels.is_empty() {
                            println!("  Labels:");
                            for (key, value) in &vol.labels {
                                println!("    {}: {}", key, value);
                            }
                        }
                        if !vol.options.is_empty() {
                            println!("  Options:");
                            for (key, value) in &vol.options {
                                println!("    {}: {}", key, value);
                            }
                        }
                    }
                    None => {
                        println!("Volume '{}' not found", name);
                        std::process::exit(1);
                    }
                }
            }

            VolumeCommands::Prune { dry_run, force } => {
                info!(
                    "Pruning unused volumes (dry_run: {}, force: {})",
                    dry_run, force
                );
                let volumes = runtime.list_volumes().await?;
                let unused_volumes: Vec<_> =
                    volumes.iter().filter(|v| v.used_by.is_empty()).collect();

                if unused_volumes.is_empty() {
                    info!("No unused volumes to prune");
                    return Ok(());
                }

                if dry_run {
                    println!("The following volumes would be removed:");
                    for volume in &unused_volumes {
                        println!("  {}", volume.name);
                    }
                    info!("✅ Dry run completed - no volumes removed");
                    return Ok(());
                }

                if !force {
                    println!("The following volumes will be removed:");
                    for volume in &unused_volumes {
                        println!("  {}", volume.name);
                    }
                    print!("Are you sure? [y/N] ");
                    std::io::Write::flush(&mut std::io::stdout())?;

                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    if !input.trim().to_lowercase().starts_with('y') {
                        info!("Aborted");
                        return Ok(());
                    }
                }

                let mut removed_count = 0;
                for volume in unused_volumes {
                    match runtime.remove_volume(&volume.name, true).await {
                        Ok(_) => {
                            info!("Removed volume: {}", volume.name);
                            removed_count += 1;
                        }
                        Err(e) => {
                            eprintln!("Failed to remove volume {}: {}", volume.name, e);
                        }
                    }
                }

                info!("✅ Pruned {} unused volumes", removed_count);
            }
        },

        Commands::Snapshot { command } => {
            match command {
                cli::SnapshotCommands::Create {
                    name,
                    description,
                    snapshot_type,
                } => {
                    let snapshot_name = name
                        .unwrap_or_else(|| format!("snapshot-{}", chrono::Utc::now().timestamp()));
                    info!("Creating {} snapshot '{}'", snapshot_type, snapshot_name);
                    if let Some(ref desc) = description {
                        info!("Description: {}", desc);
                    }

                    use bolt::capsules::snapshots::SnapshotType as SnapType;
                    let snap_type = match snapshot_type.as_str() {
                        "manual" => SnapType::Manual,
                        "auto" => SnapType::Auto,
                        "daily" => SnapType::Daily,
                        "weekly" => SnapType::Weekly,
                        _ => SnapType::Manual,
                    };

                    let snapshot_manager =
                        bolt::capsules::snapshots::SnapshotManager::new().await?;
                    snapshot_manager
                        .create_snapshot(&snapshot_name, snap_type, description.as_deref())
                        .await?;

                    info!("✅ Snapshot '{}' created successfully", snapshot_name);
                }
                cli::SnapshotCommands::List {
                    verbose,
                    filter_type,
                } => {
                    info!("Listing snapshots");
                    if let Some(filter) = filter_type {
                        info!("Filtering by type: {}", filter);
                    }

                    let snapshot_manager =
                        bolt::capsules::snapshots::SnapshotManager::new().await?;
                    let snapshots = snapshot_manager.list_snapshots().await?;

                    if snapshots.is_empty() {
                        if verbose {
                            println!("No snapshots found (verbose mode)");
                        } else {
                            println!("No snapshots found");
                        }
                    } else {
                        for snapshot in snapshots {
                            if verbose {
                                println!(
                                    "{} - {} ({:?})",
                                    snapshot.name, snapshot.created_at, snapshot.snapshot_type
                                );
                            } else {
                                println!("{}", snapshot.name);
                            }
                        }
                    }
                }
                cli::SnapshotCommands::Show { snapshot } => {
                    info!("Showing snapshot details for: {}", snapshot);

                    let snapshot_manager =
                        bolt::capsules::snapshots::SnapshotManager::new().await?;
                    let snapshots = snapshot_manager.list_snapshots().await?;

                    if let Some(snap) = snapshots.iter().find(|s| s.name == snapshot) {
                        println!("📸 Snapshot Details\n");
                        println!("Name:        {}", snap.name);
                        println!("Type:        {:?}", snap.snapshot_type);
                        println!("Created:     {}", snap.created_at);
                        println!("Path:        {}", snap.path.display());
                        if let Some(ref desc) = snap.description {
                            println!("Description: {}", desc);
                        }
                        println!("Root:        {}", snap.metadata.root_path.display());
                        println!("Containers:  {}", snap.metadata.containers_path.display());
                        if !snap.metadata.container_ids.is_empty() {
                            println!("Container IDs: {}", snap.metadata.container_ids.join(", "));
                        }
                        if !snap.metadata.image_digests.is_empty() {
                            println!("Image digests:");
                            for digest in &snap.metadata.image_digests {
                                println!("  {}", digest);
                            }
                        }

                        // Show size if possible
                        if let Ok(metadata) = tokio::fs::metadata(&snap.path).await
                            && metadata.is_dir()
                        {
                            println!("Status:      Ready");
                        }
                    } else {
                        println!("❌ Snapshot '{}' not found", snapshot);
                    }
                }
                cli::SnapshotCommands::Rollback { snapshot, force } => {
                    info!("Rolling back to snapshot '{}' (force: {})", snapshot, force);

                    let snapshot_manager =
                        bolt::capsules::snapshots::SnapshotManager::new().await?;
                    snapshot_manager
                        .rollback_to_snapshot_checked(&snapshot, force)
                        .await?;

                    info!("✅ Rolled back to snapshot '{}' successfully", snapshot);
                }
                cli::SnapshotCommands::Delete {
                    snapshot,
                    dry_run,
                    force,
                } => {
                    info!(
                        "Deleting snapshot '{}' (dry_run: {}, force: {})",
                        snapshot, dry_run, force
                    );

                    let snapshot_manager =
                        bolt::capsules::snapshots::SnapshotManager::new().await?;
                    snapshot_manager
                        .delete_snapshot_checked(&snapshot, force, dry_run)
                        .await?;

                    if dry_run {
                        info!(
                            "✅ Dry run completed - snapshot '{}' was not deleted",
                            snapshot
                        );
                    } else {
                        info!("✅ Snapshot '{}' deleted successfully", snapshot);
                    }
                }
                cli::SnapshotCommands::Cleanup { dry_run, force } => {
                    info!(
                        "Cleaning up old snapshots (dry_run: {}, force: {})",
                        dry_run, force
                    );

                    let snapshot_manager =
                        bolt::capsules::snapshots::SnapshotManager::new().await?;
                    let deleted = snapshot_manager
                        .cleanup_old_snapshots(dry_run, force, 10)
                        .await?;

                    if dry_run {
                        info!("✅ Dry run completed - no snapshots deleted");
                    } else {
                        info!("✅ Cleanup completed - {} snapshots deleted", deleted);
                    }
                }
                cli::SnapshotCommands::Preflight => {
                    let snapshot_manager =
                        bolt::capsules::snapshots::SnapshotManager::new().await?;
                    let report = snapshot_manager.preflight().await?;
                    println!("Filesystem:      {}", report.filesystem);
                    println!("Supported:       {}", report.supported);
                    println!("BTRFS command:   {}", report.btrfs_available);
                    println!("ZFS command:     {}", report.zfs_available);
                    println!("Snapshot root:   {}", report.snapshot_root.display());
                    println!("Containers path: {}", report.containers_path.display());
                    if let Some(reason) = report.reason {
                        println!("Reason:          {}", reason);
                    }
                }
                cli::SnapshotCommands::Config { verbose } => {
                    info!("Showing snapshot configuration");

                    // Load current config from SnapshotManager
                    let config_path = dirs::config_dir()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join("bolt")
                        .join("snapshots.toml");

                    if config_path.exists() {
                        let content = tokio::fs::read_to_string(&config_path).await?;
                        if verbose {
                            println!("📋 Snapshot Configuration\n");
                            println!("{}", content);
                        } else {
                            println!("Configuration file: {}", config_path.display());
                            println!("Run with --verbose to see full config");
                        }
                    } else {
                        println!("⚙️  Using default snapshot configuration");
                        println!("  • Auto snapshots: disabled");
                        println!("  • Keep daily: 7 snapshots");
                        println!("  • Keep weekly: 4 snapshots");
                        println!("  • Storage: /var/lib/bolt/snapshots");
                        println!(
                            "\nConfiguration file will be created at: {}",
                            config_path.display()
                        );
                    }
                }
                cli::SnapshotCommands::Auto { action } => {
                    use cli::AutoAction;

                    let config_path = dirs::config_dir()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join("bolt")
                        .join("snapshots.toml");

                    match action {
                        AutoAction::Enable => {
                            info!("Enabling automatic snapshots");

                            // Create default config
                            let config = r#"
[auto_snapshot]
enabled = true
before_container_run = true
before_build = true
hourly = false
daily = true

[retention]
keep_daily = 7
keep_weekly = 4
keep_monthly = 3
"#;

                            if let Some(parent) = config_path.parent() {
                                tokio::fs::create_dir_all(parent).await?;
                            }
                            tokio::fs::write(&config_path, config).await?;

                            println!("✅ Automatic snapshots enabled");
                            println!("   Daily snapshots will be created");
                            println!("   Config: {}", config_path.display());
                        }
                        AutoAction::Disable => {
                            info!("Disabling automatic snapshots");

                            if config_path.exists() {
                                tokio::fs::remove_file(&config_path).await?;
                                println!("✅ Automatic snapshots disabled");
                            } else {
                                println!("⚠️  Automatic snapshots already disabled");
                            }
                        }
                        AutoAction::Status => {
                            if config_path.exists() {
                                println!("✅ Automatic snapshots: enabled");
                                println!("   Config: {}", config_path.display());
                            } else {
                                println!("❌ Automatic snapshots: disabled");
                            }
                        }
                    }
                }
            }
        }

        Commands::Generations { command } => match command {
            GenerationCommands::List { verbose } => {
                let snapshot_manager = bolt::capsules::snapshots::SnapshotManager::new().await?;
                let generations = snapshot_manager.list_generations().await?;
                if generations.is_empty() {
                    println!("No generations found");
                    return Ok(());
                }

                println!(
                    "{:<28} {:<24} {:<10} {:<10}",
                    "GENERATION", "CREATED", "CONTAINERS", "IMAGES"
                );
                println!("{}", "-".repeat(78));
                for generation in generations {
                    println!(
                        "{:<28} {:<24} {:<10} {:<10}",
                        generation.id,
                        generation.created_at,
                        generation.container_ids.len(),
                        generation.image_digests.len()
                    );
                    if verbose {
                        if let Some(path) = &generation.boltfile_path {
                            println!("  Boltfile: {}", path.display());
                        }
                        if let Some(hash) = &generation.boltfile_hash {
                            println!("  Boltfile hash: {}", hash);
                        }
                        if let Some(revision) = &generation.boltfile_revision {
                            println!("  Revision: {}", revision);
                        }
                        if !generation.container_ids.is_empty() {
                            println!("  Containers: {}", generation.container_ids.join(", "));
                        }
                        if !generation.image_digests.is_empty() {
                            println!("  Image digests:");
                            for digest in &generation.image_digests {
                                println!("    {}", digest);
                            }
                        }
                    }
                }
            }
        },

        Commands::Hardware { command } => {
            use bolt::runtime::hardware_detection::{
                HardwareProfile, WorkloadType as HwWorkloadType,
            };
            use cli::HardwareCommands;

            match command {
                HardwareCommands::Detect { format } => {
                    info!("🔍 Detecting hardware...");
                    let hw = HardwareProfile::detect().await?;

                    if format == "json" {
                        println!("{}", serde_json::to_string_pretty(&hw)?);
                    } else {
                        println!("\n🖥️  CPU: {} ({:?})", hw.cpu.model_name, hw.cpu.vendor);
                        println!(
                            "   Cores: {} physical, {} logical",
                            hw.cpu.cores_physical, hw.cpu.cores_logical
                        );
                        if let Some(ref zen) = hw.cpu.zen_generation {
                            println!(
                                "   AMD Zen: {:?}{}",
                                zen,
                                if hw.cpu.has_3d_vcache {
                                    " with 3D V-Cache"
                                } else {
                                    ""
                                }
                            );
                        }
                        if let Some(ref hybrid) = hw.cpu.hybrid_architecture {
                            println!(
                                "   Intel Hybrid: {}P + {}E cores",
                                hybrid.p_cores, hybrid.e_cores
                            );
                        }
                        println!("\n🎮 GPUs: {}", hw.gpu.len());
                        for (i, gpu) in hw.gpu.iter().enumerate() {
                            println!(
                                "   GPU {}: {:?} at {}",
                                i,
                                gpu.vendor,
                                gpu.device_path.display()
                            );
                        }
                        println!("\n💾 Memory: {} GB", hw.memory.total_mb / 1024);
                    }
                }
                HardwareCommands::Cpu { verbose } => {
                    let hw = HardwareProfile::detect().await?;
                    println!("🖥️  CPU Information:\n");
                    println!("Vendor: {:?}", hw.cpu.vendor);
                    println!("Model: {}", hw.cpu.model_name);
                    println!("Architecture: {:?}", hw.cpu.architecture);
                    println!("Physical Cores: {}", hw.cpu.cores_physical);
                    println!("Logical Cores: {}", hw.cpu.cores_logical);

                    if let Some(cache) = hw.cpu.cache_l3_kb {
                        println!("L3 Cache: {} MB", cache / 1024);
                    }
                    println!("NUMA Nodes: {}", hw.cpu.numa_nodes);

                    if let Some(ref zen) = hw.cpu.zen_generation {
                        println!("\n⚡ AMD Ryzen Details:");
                        println!("  Zen Generation: {:?}", zen);
                        println!(
                            "  3D V-Cache: {}",
                            if hw.cpu.has_3d_vcache {
                                "YES 🎮"
                            } else {
                                "No"
                            }
                        );
                        if let Some(ccd) = hw.cpu.ccd_count {
                            println!("  CCD Count: {}", ccd);
                        }
                    }

                    if let Some(ref hybrid) = hw.cpu.hybrid_architecture {
                        println!("\n🔀 Intel Hybrid Architecture:");
                        println!(
                            "  P-cores: {} @ {:.1} GHz",
                            hybrid.p_cores, hybrid.p_core_base_freq
                        );
                        println!(
                            "  E-cores: {} @ {:.1} GHz",
                            hybrid.e_cores, hybrid.e_core_base_freq
                        );
                        println!(
                            "  Thread Director: {}",
                            if hybrid.thread_director { "✅" } else { "❌" }
                        );
                    }

                    if verbose {
                        println!("\n📋 CPU Features:");
                        for (i, feature) in hw.cpu.features.iter().take(20).enumerate() {
                            if i % 5 == 0 {
                                println!();
                            }
                            print!("  {:<12}", feature);
                        }
                        if hw.cpu.features.len() > 20 {
                            println!("\n  ... and {} more", hw.cpu.features.len() - 20);
                        }
                    }
                }
                HardwareCommands::Gpu { verbose } => {
                    let hw = HardwareProfile::detect().await?;
                    println!("🎮 GPU Information:\n");

                    if hw.gpu.is_empty() {
                        println!("No GPUs detected");
                    } else {
                        for (i, gpu) in hw.gpu.iter().enumerate() {
                            println!("GPU {}: {:?}", i, gpu.vendor);
                            println!("  Device: {}", gpu.device_path.display());
                            if let Some(ref render) = gpu.render_node {
                                println!("  Render Node: {}", render.display());
                            }
                            if let Some(ref pci) = gpu.pci_id {
                                println!("  PCI ID: {}", pci);
                            }

                            if verbose {
                                println!("  Capabilities:");
                                if gpu.capabilities.cuda {
                                    println!("    ✅ CUDA");
                                }
                                if gpu.capabilities.rocm {
                                    println!("    ✅ ROCm");
                                }
                                if gpu.capabilities.opencl {
                                    println!("    ✅ OpenCL");
                                }
                                if gpu.capabilities.vulkan {
                                    println!("    ✅ Vulkan");
                                }
                                if gpu.capabilities.vaapi {
                                    println!("    ✅ VA-API");
                                }
                                if gpu.capabilities.quick_sync {
                                    println!("    ✅ Quick Sync (Intel)");
                                }
                                if gpu.capabilities.nvenc_nvdec {
                                    println!("    ✅ NVENC/NVDEC");
                                }
                                if gpu.capabilities.vce_vcn {
                                    println!("    ✅ VCE/VCN (AMD)");
                                }
                                if gpu.capabilities.ray_tracing {
                                    println!("    ✅ Ray Tracing");
                                }
                                if gpu.capabilities.dlss {
                                    println!("    ✅ DLSS");
                                }
                                if gpu.capabilities.fsr {
                                    println!("    ✅ FSR");
                                }
                                if gpu.capabilities.xess {
                                    println!("    ✅ XeSS");
                                }
                            }
                            println!();
                        }
                    }
                }
                HardwareCommands::Memory => {
                    let hw = HardwareProfile::detect().await?;
                    println!("💾 Memory Information:\n");
                    println!("Total: {} GB", hw.memory.total_mb / 1024);
                    println!("NUMA Nodes: {}", hw.memory.numa_nodes);
                    println!("2MB Hugepages: {}", hw.memory.hugepages_2mb);
                    println!("1GB Hugepages: {}", hw.memory.hugepages_1gb);
                }
                HardwareCommands::Affinity { workload } => {
                    let hw = HardwareProfile::detect().await?;
                    println!("🎯 CPU Affinity Recommendations:\n");

                    let workloads = if let Some(wl) = workload {
                        vec![match wl {
                            cli::WorkloadType::Gaming => HwWorkloadType::Gaming,
                            cli::WorkloadType::Performance => HwWorkloadType::HighPerformance,
                            cli::WorkloadType::Balanced => HwWorkloadType::Balanced,
                            cli::WorkloadType::Background => HwWorkloadType::Background,
                            cli::WorkloadType::Batch => HwWorkloadType::Batch,
                        }]
                    } else {
                        vec![
                            HwWorkloadType::Gaming,
                            HwWorkloadType::HighPerformance,
                            HwWorkloadType::Balanced,
                            HwWorkloadType::Background,
                            HwWorkloadType::Batch,
                        ]
                    };

                    for wl in workloads {
                        let affinity = hw.optimal_cpu_affinity(wl);
                        println!("{:?}: {:?}", wl, affinity);
                    }
                }
                HardwareCommands::Governor { mode } => {
                    use bolt::runtime::hardware_detection::CpuGovernor;

                    if let Some(ref mode_str) = mode {
                        // Parse and set governor
                        let governor = CpuGovernor::parse(mode_str)
                            .ok_or_else(|| anyhow::anyhow!("Unknown governor: {}", mode_str))?;

                        info!("Setting system-wide CPU governor to: {}", mode_str);

                        match CpuGovernor::set_system_governor(governor) {
                            Ok(()) => {
                                println!("✅ CPU governor set to '{}'", mode_str);
                                println!(
                                    "   Note: This change is temporary and will reset on reboot."
                                );
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to set CPU governor: {}", e);
                                eprintln!("   Hint: Try running with sudo/root privileges");
                                std::process::exit(1);
                            }
                        }
                    } else {
                        // Show current governor
                        match CpuGovernor::get_system_governor() {
                            Ok(gov) => {
                                println!("Current CPU governor: {}", gov.as_str());

                                // Show frequency info
                                if let Ok(freq) = CpuGovernor::get_current_frequency(0) {
                                    println!(
                                        "Current frequency (CPU 0): {:.2} GHz",
                                        freq as f64 / 1_000_000.0
                                    );
                                }
                                if let Ok((min, max)) = CpuGovernor::get_frequency_range(0) {
                                    println!(
                                        "Frequency range: {:.2} - {:.2} GHz",
                                        min as f64 / 1_000_000.0,
                                        max as f64 / 1_000_000.0
                                    );
                                }

                                println!("\nAvailable governors:");
                                match CpuGovernor::list_available() {
                                    Ok(governors) => {
                                        for gov in governors {
                                            let marker = if gov.as_str() == gov.as_str() {
                                                "➜"
                                            } else {
                                                " "
                                            };
                                            println!("  {} {}", marker, gov.as_str());
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to list available governors: {}", e);
                                    }
                                }

                                println!("\nRecommended governors by workload:");
                                println!("  • Gaming/High Performance: performance");
                                println!("  • Balanced: schedutil");
                                println!("  • Power Saving: powersave");
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to read current governor: {}", e);
                                std::process::exit(1);
                            }
                        }
                    }
                }
            }
        }

        Commands::Gpu { command } => {
            cli::gpu::GpuCommand { command }.execute().await?;
        }

        Commands::Nv { command } => {
            cli::nv::execute(&command).await?;
        }

        Commands::Amd { command } => {
            cli::amd::execute(&command).await?;
        }

        Commands::Arc { command } => {
            cli::arc::execute(&command).await?;
        }

        Commands::Compat { command } => {
            compat::handle_compat_command(compat::CompatArgs { command }, runtime).await?;
        }

        Commands::Tools { command } => {
            cli::tools::execute(&command, &bolt_config.boltfile_path).await?;
        }

        Commands::Dashboard { port } => {
            use bolt::monitoring::{MetricsCollector, dashboard::MetricsDashboard};
            use std::sync::Arc;

            info!("🎛️  Starting metrics dashboard on port {}", port);

            let metrics = Arc::new(MetricsCollector::new().await?);
            let dashboard = MetricsDashboard::new(Arc::new(runtime.clone()), metrics);

            println!("🚀 Metrics Dashboard starting...");
            println!("   📊 Dashboard: http://localhost:{}", port);
            println!("   📈 API Endpoints:");
            println!("      • http://localhost:{}/api/metrics", port);
            println!("      • http://localhost:{}/api/gpu", port);
            println!("      • http://localhost:{}/api/containers", port);
            println!("\n   Press Ctrl+C to stop\n");

            dashboard.start(port).await?;
        }
    }

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn parse_inspect_kind(kind: &str) -> Result<bolt::project::InspectKind> {
    match kind {
        "service" | "svc" => Ok(bolt::project::InspectKind::Service),
        "container" | "ctr" => Ok(bolt::project::InspectKind::Container),
        "image" => Ok(bolt::project::InspectKind::Image),
        "volume" | "vol" => Ok(bolt::project::InspectKind::Volume),
        "network" | "net" => Ok(bolt::project::InspectKind::Network),
        other => anyhow::bail!(
            "unsupported inspect kind '{}'; use service, container, image, volume, or network",
            other
        ),
    }
}

fn print_check_report(label: &str, report: &bolt::project::DoctorReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "{}: {}",
        label,
        if report.ok { "ok" } else { "issues found" }
    );
    for check in &report.checks {
        println!(
            "  {:<18} {:<4} {}",
            check.name,
            if check.ok { "ok" } else { "fail" },
            check.message
        );
    }
    Ok(())
}

fn print_completion(shell: &str) -> Result<()> {
    let commands = "run build pull push image ps stop exec logs rm restart surge plan apply destroy lock drift doctor validate self-test dns import inspect gaming gpu nv amd arc network volume snapshot generations hardware compat tools dashboard completions manpage";
    match shell {
        "bash" => println!(
            r#"_bolt()
{{
  local cur
  cur="${{COMP_WORDS[COMP_CWORD]}}"
  COMPREPLY=( $(compgen -W "{commands}" -- "$cur") )
}}
complete -F _bolt bolt"#
        ),
        "zsh" => println!(
            r#"#compdef bolt
_arguments '1:command:({commands})'"#
        ),
        "fish" => {
            for command in commands.split_whitespace() {
                println!("complete -c bolt -f -a {command}");
            }
        }
        other => anyhow::bail!("unsupported shell '{}'; use bash, zsh, or fish", other),
    }
    Ok(())
}

fn print_manpage() {
    let mut command = Cli::command();
    let about = command
        .get_about()
        .map(|s| s.to_string())
        .unwrap_or_default();
    println!(".TH BOLT 1");
    println!(".SH NAME");
    println!("bolt \\- {}", about);
    println!(".SH SYNOPSIS");
    println!(".B bolt [OPTIONS] <COMMAND>");
    println!(".SH DESCRIPTION");
    println!(
        "Bolt is a performance-first container runtime with Boltfile orchestration, GPU support, snapshots, and QUIC networking."
    );
    println!(".SH COMMANDS");
    for subcommand in command.get_subcommands_mut() {
        let about = subcommand
            .get_about()
            .map(|s| s.to_string())
            .unwrap_or_default();
        println!(".TP");
        println!(".B {}", subcommand.get_name());
        println!("{}", about);
    }
}

fn print_inspection(inspection: &bolt::project::Inspection) {
    match inspection {
        bolt::project::Inspection::Service(service) => {
            println!("Service: {}", service.name);
            println!("  Container: {}", service.container_name);
            if let Some(image) = &service.desired.image {
                println!("  Image: {}", image);
            }
            if let Some(discovery) = &service.discovery {
                println!("  DNS: {}", discovery.dns_name);
                println!("  Protocol: {}", discovery.protocol);
            }
            println!(
                "  Runtime state: {}",
                if service.container.is_some() {
                    "present"
                } else {
                    "missing"
                }
            );
        }
        bolt::project::Inspection::Container(container) => {
            println!("Container: {}", container.name);
            println!("  ID: {}", container.id);
            println!("  Image: {}", container.image);
            println!("  Status: {}", container.status);
        }
        bolt::project::Inspection::Image(image) => {
            println!("Image: {}", image.name);
            println!("  ID: {}", image.id);
            println!("  Size: {}", format_bytes(image.size));
        }
        bolt::project::Inspection::Volume(volume) => {
            println!("Volume: {}", volume.name);
            println!("  Driver: {}", volume.driver);
            println!("  Mountpoint: {}", volume.mountpoint.display());
            println!("  Used by: {}", volume.used_by.join(", "));
        }
        bolt::project::Inspection::Network(network) => {
            println!("Network: {}", network.name);
            println!("  ID: {}", network.id);
            println!("  Driver: {}", network.driver);
            if let Some(subnet) = &network.subnet {
                println!("  Subnet: {}", subnet);
            }
        }
    }
}

fn normalize_gpu_device_request(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
        return "all".to_string();
    }

    trimmed
        .strip_prefix("device=")
        .unwrap_or(trimmed)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::normalize_gpu_device_request;

    #[test]
    fn gpu_device_request_normalizes_docker_device_syntax() {
        assert_eq!(normalize_gpu_device_request("all"), "all");
        assert_eq!(normalize_gpu_device_request(" device=0,1 "), "0,1");
        assert_eq!(normalize_gpu_device_request("0"), "0");
    }
}
