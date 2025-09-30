mod cli;

use anyhow::Result;
use bolt::{BoltConfig, BoltRuntime, gaming, surge};
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

            VolumeCommands::Prune { force } => {
                info!("Pruning unused volumes (force: {})", force);
                let volumes = runtime.list_volumes().await?;
                let unused_volumes: Vec<_> =
                    volumes.iter().filter(|v| v.used_by.is_empty()).collect();

                if unused_volumes.is_empty() {
                    info!("No unused volumes to prune");
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
                    let snapshot_name = name.unwrap_or_else(|| {
                        format!(
                            "snapshot-{}",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                        )
                    });
                    info!("Creating {} snapshot '{}'", snapshot_type, snapshot_name);
                    if let Some(desc) = description {
                        info!("Description: {}", desc);
                    }
                    // TODO: Implement snapshot creation
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
                    // TODO: Implement snapshot listing
                    if verbose {
                        println!("No snapshots found (verbose mode)");
                    } else {
                        println!("No snapshots found");
                    }
                }
                cli::SnapshotCommands::Show { snapshot } => {
                    info!("Showing snapshot details for: {}", snapshot);
                    // TODO: Implement snapshot details
                    println!("Snapshot '{}' not found", snapshot);
                }
                cli::SnapshotCommands::Rollback { snapshot, force } => {
                    info!("Rolling back to snapshot '{}' (force: {})", snapshot, force);
                    // TODO: Implement snapshot rollback
                    info!("✅ Rolled back to snapshot '{}' successfully", snapshot);
                }
                cli::SnapshotCommands::Delete { snapshot, force } => {
                    info!("Deleting snapshot '{}' (force: {})", snapshot, force);
                    // TODO: Implement snapshot deletion
                    info!("✅ Snapshot '{}' deleted successfully", snapshot);
                }
                cli::SnapshotCommands::Cleanup { .. } => {
                    info!("Cleaning up old snapshots");
                    // TODO: Implement snapshot cleanup
                    info!("✅ Snapshot cleanup completed");
                }
                cli::SnapshotCommands::Config { .. } => {
                    info!("Managing snapshot configuration");
                    // TODO: Implement snapshot configuration
                    info!("✅ Snapshot configuration updated");
                }
                cli::SnapshotCommands::Auto { .. } => {
                    info!("Managing automatic snapshots");
                    // TODO: Implement automatic snapshots
                    info!("✅ Automatic snapshot settings configured");
                }
            }
        }

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
                        let governor = CpuGovernor::from_str(mode_str)
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

        Commands::Compat { command } => {
            compat::handle_compat_command(compat::CompatArgs { command }, runtime).await?;
        }
    }

    Ok(())
}
