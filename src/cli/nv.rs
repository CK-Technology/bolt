//! NVIDIA GPU management commands (`bolt nv`)
//!
//! Provides native NVIDIA GPU management without requiring nvidia-container-toolkit.

use crate::Result;
use bolt::runtime::gpu::nvbind::{DriverType, GpuArchitecture, NvbindManager};
use bolt::runtime::gpu::profiles::{GpuProfile, GpuProfileManager};
use clap::Subcommand;
use std::path::Path;
use tracing::info;

#[derive(Subcommand)]
pub enum NvCommands {
    /// Show NVIDIA GPU information
    Info {
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
    },

    /// Run system diagnostics and check GPU health
    Doctor {
        /// Attempt to fix detected issues
        #[arg(long)]
        fix: bool,
    },

    /// Run GPU benchmark
    Benchmark {
        /// Number of iterations
        #[arg(short, long, default_value = "3")]
        iterations: u32,
    },

    /// CDI (Container Device Interface) management
    Cdi {
        #[command(subcommand)]
        command: CdiCommands,
    },

    /// GPU profile management
    Profile {
        #[command(subcommand)]
        command: ProfileCommands,
    },

    /// Show driver information
    Driver,

    /// Show GPU architecture details
    Arch {
        /// GPU index (default: 0)
        #[arg(short, long, default_value = "0")]
        gpu: u32,
    },
}

#[derive(Subcommand)]
pub enum ProfileCommands {
    /// List available GPU profiles
    List {
        /// Profile type (gaming, ai, all)
        #[arg(short, long, default_value = "all")]
        profile_type: String,
    },

    /// Show profile details
    Show {
        /// Profile name
        name: String,
    },

    /// Generate CDI spec with profile
    Apply {
        /// Profile name (gaming profile or AI profile name)
        name: String,

        /// Output CDI spec file
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum CdiCommands {
    /// Generate CDI specification
    Generate {
        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Profile type (general, gaming, aiml)
        #[arg(short, long, default_value = "general")]
        profile: String,

        /// GPU devices to include (all, 0, 1,2)
        #[arg(long, default_value = "all")]
        devices: String,
    },

    /// List existing CDI specifications
    List,

    /// Validate a CDI specification file
    Validate {
        /// Path to CDI spec file
        file: String,
    },

    /// Show CDI specification info without generating
    Info,
}

pub async fn execute(command: &NvCommands) -> Result<()> {
    match command {
        NvCommands::Info { format, detailed } => show_info(format, *detailed).await,
        NvCommands::Doctor { fix } => run_doctor(*fix).await,
        NvCommands::Benchmark { iterations } => run_benchmark(*iterations).await,
        NvCommands::Cdi { command } => execute_cdi(command).await,
        NvCommands::Profile { command } => execute_profile(command).await,
        NvCommands::Driver => show_driver_info().await,
        NvCommands::Arch { gpu } => show_architecture(*gpu).await,
    }
}

async fn show_info(format: &str, detailed: bool) -> Result<()> {
    info!("🔍 Detecting NVIDIA GPUs...");

    let manager = NvbindManager::detect()?;

    if !manager.is_available {
        println!("\n⚠️  No NVIDIA GPUs detected or driver not loaded\n");
        println!("Possible causes:");
        println!("  • NVIDIA driver not installed");
        println!("  • GPU not detected by system");
        println!("  • Nouveau driver blocking NVIDIA driver\n");
        return Ok(());
    }

    if format == "json" {
        let json = serde_json::to_string_pretty(&manager)?;
        println!("{}", json);
        return Ok(());
    }

    // Text format
    println!();
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                      NVIDIA GPU Information                          ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();

    // Driver info
    if let Some(ref driver) = manager.driver_info {
        println!("📦 Driver Information:");
        println!("   Version:     {}", driver.version);
        println!("   Type:        {}", driver.driver_type.name());
        if let Some(ref cuda) = driver.cuda_version {
            println!("   CUDA:        {}", cuda);
        }
        if detailed && !driver.libraries.is_empty() {
            println!("   Libraries:   {} found", driver.libraries.len());
        }
        println!();
    }

    // GPU info
    println!("🎮 Detected GPUs: {}", manager.detected_gpus.len());
    println!();

    for gpu in &manager.detected_gpus {
        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ GPU {}: {}", gpu.id, gpu.name);
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!("│  PCI Address:      {}", gpu.pci_address);
        println!("│  Device Path:      {}", gpu.device_path);
        println!("│  Architecture:     {:?}", gpu.architecture);

        if let Some((major, minor)) = gpu.compute_capability {
            println!("│  Compute Cap:      {}.{}", major, minor);
        }

        if let Some(mem) = gpu.memory_bytes {
            let mem_gb = mem as f64 / (1024.0 * 1024.0 * 1024.0);
            println!("│  Memory:           {:.1} GB", mem_gb);
        }

        if detailed {
            println!("│");
            println!("│  Features:");
            println!(
                "│    Tensor Cores:   {}",
                gpu.architecture
                    .tensor_core_generation()
                    .map(|g| format!("Gen {}", g))
                    .unwrap_or_else(|| "No".to_string())
            );
            println!(
                "│    MIG Support:    {}",
                if gpu.architecture.supports_mig() {
                    "Yes"
                } else {
                    "No"
                }
            );
            println!(
                "│    FP4 Support:    {}",
                if gpu.architecture.supports_fp4() {
                    "Yes"
                } else {
                    "No"
                }
            );
        }

        println!("└─────────────────────────────────────────────────────────────────────┘");
        println!();
    }

    Ok(())
}

async fn run_doctor(fix: bool) -> Result<()> {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                      NVIDIA GPU Diagnostics                          ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();

    let mut issues_found = 0;
    let issues_fixed = 0;

    // Check 1: Driver loaded
    print!("🔍 Checking NVIDIA driver... ");
    if Path::new("/proc/driver/nvidia/version").exists() {
        println!("✅ Loaded");
    } else if Path::new("/sys/module/nouveau").exists() {
        println!("⚠️  Nouveau driver active (blocks NVIDIA)");
        issues_found += 1;
        if fix {
            println!("   ℹ️  To fix: blacklist nouveau and install NVIDIA driver");
        }
    } else {
        println!("❌ Not loaded");
        issues_found += 1;
    }

    // Check 2: Device nodes
    print!("🔍 Checking device nodes... ");
    let nvidiactl = Path::new("/dev/nvidiactl").exists();
    let nvidia0 = Path::new("/dev/nvidia0").exists();
    let nvidia_uvm = Path::new("/dev/nvidia-uvm").exists();

    if nvidiactl && nvidia0 {
        println!("✅ Present");
        if !nvidia_uvm {
            println!("   ⚠️  /dev/nvidia-uvm missing (CUDA may not work)");
            issues_found += 1;
            if fix {
                println!("   ℹ️  Run: sudo modprobe nvidia-uvm");
            }
        }
    } else {
        println!("❌ Missing");
        issues_found += 1;
    }

    // Check 3: Driver type
    print!("🔍 Checking driver type... ");
    let manager = NvbindManager::detect()?;
    if let Some(ref driver) = manager.driver_info {
        match driver.driver_type {
            DriverType::NvidiaOpen => println!("✅ Open GPU Kernel Modules (recommended)"),
            DriverType::NvidiaProprietary => println!("✅ Proprietary driver"),
            DriverType::Nouveau => {
                println!("⚠️  Nouveau (limited features)");
                issues_found += 1;
            }
        }
    } else {
        println!("❌ Unable to detect");
        issues_found += 1;
    }

    // Check 4: nvidia-smi
    print!("🔍 Checking nvidia-smi... ");
    if std::process::Command::new("nvidia-smi")
        .arg("--version")
        .output()
        .is_ok()
    {
        println!("✅ Available");
    } else {
        println!("⚠️  Not found (optional but recommended)");
    }

    // Check 5: Container device access
    print!("🔍 Checking container device access... ");
    let required_devices = NvbindManager::get_required_devices();
    let mut missing_devices = Vec::new();
    for device in &required_devices {
        if !Path::new(device).exists() {
            missing_devices.push(device.clone());
        }
    }
    if missing_devices.is_empty() {
        println!("✅ All {} devices accessible", required_devices.len());
    } else {
        println!("⚠️  {} devices missing", missing_devices.len());
        issues_found += 1;
    }

    // Check 6: CUDA libraries
    print!("🔍 Checking CUDA libraries... ");
    if let Some(ref driver) = manager.driver_info {
        if !driver.libraries.is_empty() {
            println!("✅ {} libraries found", driver.libraries.len());
        } else {
            println!("⚠️  No CUDA libraries found");
        }
    } else {
        println!("❌ Unable to check");
    }

    // Summary
    println!();
    println!("─────────────────────────────────────────────────────────────────────────");
    if issues_found == 0 {
        println!("✅ All checks passed! GPU is ready for container workloads.");
    } else {
        println!("⚠️  {} issue(s) found", issues_found);
        if issues_fixed > 0 {
            println!("✅ {} issue(s) fixed", issues_fixed);
        }
        if !fix && issues_found > issues_fixed {
            println!("   Run with --fix to attempt automatic fixes");
        }
    }
    println!();

    Ok(())
}

async fn run_benchmark(iterations: u32) -> Result<()> {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                      NVIDIA GPU Benchmark                            ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();

    let manager = NvbindManager::detect()?;

    if !manager.is_available {
        println!("❌ No NVIDIA GPUs available for benchmarking");
        return Ok(());
    }

    println!("🏁 Running {} iteration(s)...", iterations);
    println!();

    // GPU detection benchmark
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = NvbindManager::detect()?;
    }
    let detection_time = start.elapsed();
    let avg_detection = detection_time.as_millis() as f64 / iterations as f64;

    println!("📊 Results:");
    println!("   GPU Detection:    {:.2} ms average", avg_detection);
    println!("   GPUs Detected:    {}", manager.detected_gpus.len());

    if let Some(ref driver) = manager.driver_info {
        println!("   Driver Version:   {}", driver.version);
    }

    println!();
    println!("💡 Note: For full GPU compute benchmarks, use nvidia-smi or dedicated tools");
    println!();

    Ok(())
}

async fn execute_cdi(command: &CdiCommands) -> Result<()> {
    match command {
        CdiCommands::Generate {
            output,
            profile,
            devices,
        } => generate_cdi(output.as_deref(), profile, devices).await,
        CdiCommands::List => list_cdi().await,
        CdiCommands::Validate { file } => validate_cdi(file).await,
        CdiCommands::Info => show_cdi_info().await,
    }
}

async fn generate_cdi(output: Option<&str>, profile: &str, _devices: &str) -> Result<()> {
    println!("🔧 Generating {} CDI specification (native)...", profile);

    let manager = NvbindManager::detect()?;

    if !manager.is_available {
        println!("❌ No NVIDIA GPUs detected");
        return Ok(());
    }

    // Generate CDI spec based on profile using native methods
    let cdi_spec = match profile.to_lowercase().as_str() {
        "gaming" => {
            println!("🎮 Using gaming profile (optimized for low-latency graphics)");
            manager.generate_gaming_cdi_spec().await?
        }
        "aiml" | "ai" | "ml" => {
            println!("🧠 Using AI/ML profile (optimized for compute workloads)");
            manager.generate_aiml_cdi_spec().await?
        }
        _ => {
            println!("📊 Using general profile (balanced for all workloads)");
            manager.generate_default_cdi_spec().await?
        }
    };

    let spec_json = serde_json::to_string_pretty(&cdi_spec)?;

    // Show summary
    println!();
    println!("📋 CDI Specification Summary:");
    println!("   • Version: {}", cdi_spec.cdi_version);
    println!("   • Kind: {}", cdi_spec.kind);
    println!("   • GPUs: {}", cdi_spec.devices.len());
    println!(
        "   • Device nodes: {}",
        cdi_spec.container_edits.device_nodes.len()
    );
    println!("   • Mounts: {}", cdi_spec.container_edits.mounts.len());
    println!(
        "   • Environment vars: {}",
        cdi_spec.container_edits.env.len()
    );
    println!();

    if let Some(path) = output {
        // Create parent directory if needed
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &spec_json)?;
        println!("✅ CDI spec written to: {}", path);
        println!();
        println!("💡 Usage:");
        println!("   • Container runtimes will auto-discover this spec");
        println!("   • Use 'bolt run --gpu all' to use the CDI spec");
        println!("   • Validate with: bolt nv cdi validate {}", path);
    } else {
        println!("{}", spec_json);
    }

    Ok(())
}

async fn list_cdi() -> Result<()> {
    println!("🔍 Searching for CDI specifications...");
    println!();

    let cdi_paths = ["/etc/cdi", "/var/run/cdi", "/usr/local/share/cdi"];

    let mut found = 0;
    for path in &cdi_paths {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.ends_with(".json") || name_str.ends_with(".yaml") {
                    println!("  📄 {}/{}", path, name_str);
                    found += 1;
                }
            }
        }
    }

    if found == 0 {
        println!("  No CDI specifications found");
        println!();
        println!("💡 Generate one with: bolt nv cdi generate --output /etc/cdi/nvidia.json");
    } else {
        println!();
        println!("Found {} CDI specification(s)", found);
    }

    Ok(())
}

async fn validate_cdi(file: &str) -> Result<()> {
    println!("🔍 Validating CDI specification: {}", file);

    let content = std::fs::read_to_string(file)?;
    let spec: serde_json::Value = serde_json::from_str(&content)?;

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Check required fields
    if spec.get("cdiVersion").is_none() {
        errors.push("Missing 'cdiVersion' field");
    }
    if spec.get("kind").is_none() {
        errors.push("Missing 'kind' field");
    }
    if spec.get("devices").is_none() && spec.get("containerEdits").is_none() {
        errors.push("Must have 'devices' or 'containerEdits'");
    }

    // Check version
    if let Some(version) = spec.get("cdiVersion").and_then(|v| v.as_str())
        && !version.starts_with("0.")
    {
        warnings.push(format!("CDI version {} may not be supported", version));
    }

    // Validate device nodes exist
    if let Some(edits) = spec.get("containerEdits")
        && let Some(nodes) = edits.get("deviceNodes").and_then(|n| n.as_array())
    {
        for node in nodes {
            if let Some(path) = node.get("path").and_then(|p| p.as_str())
                && !std::path::Path::new(path).exists()
            {
                warnings.push(format!("Device node does not exist: {}", path));
            }
        }
    }

    println!();
    if errors.is_empty() && warnings.is_empty() {
        println!("✅ CDI specification is valid");
        println!("   All device nodes verified");
    } else {
        for error in &errors {
            println!("❌ Error: {}", error);
        }
        for warning in &warnings {
            println!("⚠️  Warning: {}", warning);
        }
        if errors.is_empty() {
            println!();
            println!("✅ CDI specification structure is valid (with warnings)");
        }
    }

    Ok(())
}

async fn show_cdi_info() -> Result<()> {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                      CDI (Container Device Interface)                ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();
    println!("CDI is a specification for exposing devices to containers in a");
    println!("vendor-neutral way. Bolt generates native CDI specs without requiring");
    println!("nvidia-container-toolkit.");
    println!();
    println!("📋 Available Profiles:");
    println!("   • general  - Balanced for all workloads (default)");
    println!("   • gaming   - Optimized for low-latency graphics");
    println!("   • aiml     - Optimized for AI/ML compute workloads");
    println!();
    println!("📁 CDI Specification Locations:");
    println!("   • /etc/cdi/                  - System-wide specs (recommended)");
    println!("   • /var/run/cdi/              - Runtime specs");
    println!("   • ~/.config/cdi/             - User specs");
    println!();
    println!("💡 Usage Examples:");
    println!("   bolt nv cdi generate --output /etc/cdi/nvidia.json");
    println!("   bolt nv cdi generate --profile gaming -o /etc/cdi/nvidia-gaming.json");
    println!("   bolt nv cdi generate --profile aiml -o /etc/cdi/nvidia-aiml.json");
    println!("   bolt nv cdi list");
    println!("   bolt nv cdi validate /etc/cdi/nvidia.json");
    println!();
    println!("🔧 Bolt automatically uses CDI specs when running containers with --gpu");
    println!();

    Ok(())
}

async fn show_driver_info() -> Result<()> {
    let manager = NvbindManager::detect()?;

    println!();
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                      NVIDIA Driver Information                       ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();

    if let Some(ref driver) = manager.driver_info {
        println!("Version:        {}", driver.version);
        println!("Type:           {}", driver.driver_type.name());
        println!(
            "CUDA Support:   {}",
            if driver.driver_type.supports_cuda() {
                "Yes"
            } else {
                "No"
            }
        );

        if let Some(ref cuda) = driver.cuda_version {
            println!("CUDA Version:   {}", cuda);
        }

        println!();
        println!("Libraries Found: {}", driver.libraries.len());
        if !driver.libraries.is_empty() {
            for lib in driver.libraries.iter().take(10) {
                println!("  • {}", lib);
            }
            if driver.libraries.len() > 10 {
                println!("  ... and {} more", driver.libraries.len() - 10);
            }
        }
    } else {
        println!("❌ Unable to detect driver information");
    }

    println!();
    Ok(())
}

async fn show_architecture(gpu_index: u32) -> Result<()> {
    let manager = NvbindManager::detect()?;

    if !manager.is_available {
        println!("❌ No NVIDIA GPUs detected");
        return Ok(());
    }

    let gpu = manager.detected_gpus.get(gpu_index as usize);

    if let Some(gpu) = gpu {
        println!();
        println!("╔══════════════════════════════════════════════════════════════════════╗");
        println!("║                      GPU Architecture Details                        ║");
        println!("╚══════════════════════════════════════════════════════════════════════╝");
        println!();
        println!("GPU {}:          {}", gpu.id, gpu.name);
        println!("Architecture:   {:?}", gpu.architecture);

        if let Some((major, minor)) = gpu.compute_capability {
            println!("Compute Cap:    {}.{}", major, minor);
        }

        println!();
        println!("Feature Support:");
        println!(
            "  Tensor Cores:     {}",
            gpu.architecture
                .tensor_core_generation()
                .map(|g| format!("Yes (Gen {})", g))
                .unwrap_or_else(|| "No".to_string())
        );
        println!(
            "  MIG Support:      {}",
            if gpu.architecture.supports_mig() {
                "Yes"
            } else {
                "No"
            }
        );
        println!(
            "  FP4 (Blackwell):  {}",
            if gpu.architecture.supports_fp4() {
                "Yes"
            } else {
                "No"
            }
        );

        println!();
        println!("Architecture Capabilities:");
        match gpu.architecture {
            GpuArchitecture::Blackwell => {
                println!("  • 5th gen Tensor Cores with FP4 support");
                println!("  • MIG (Multi-Instance GPU) capable");
                println!("  • DLSS 4 with Frame Generation");
                println!("  • AV1 encode/decode");
            }
            GpuArchitecture::AdaLovelace => {
                println!("  • 4th gen Tensor Cores");
                println!("  • 3rd gen RT Cores");
                println!("  • DLSS 3 with Frame Generation");
                println!("  • AV1 encode/decode");
            }
            GpuArchitecture::Hopper => {
                println!("  • Enhanced 4th gen Tensor Cores");
                println!("  • MIG (Multi-Instance GPU) capable");
                println!("  • Transformer Engine with FP8");
                println!("  • HBM3 memory support");
            }
            GpuArchitecture::Ampere => {
                println!("  • 3rd gen Tensor Cores");
                println!("  • 2nd gen RT Cores");
                println!("  • DLSS 2.x support");
                println!("  • MIG support (A100 only)");
            }
            GpuArchitecture::Turing => {
                println!("  • 2nd gen Tensor Cores");
                println!("  • 1st gen RT Cores");
                println!("  • DLSS 1.x/2.x support");
            }
            GpuArchitecture::Volta => {
                println!("  • 1st gen Tensor Cores");
                println!("  • HBM2 memory");
            }
            GpuArchitecture::Pascal | GpuArchitecture::Maxwell => {
                println!("  • CUDA compute capable");
                println!("  • No Tensor/RT cores");
            }
            GpuArchitecture::Unknown => {
                println!("  • Architecture not recognized");
            }
        }
        println!();
    } else {
        println!("❌ GPU {} not found", gpu_index);
    }

    Ok(())
}

// ============= Profile Commands =============

async fn execute_profile(command: &ProfileCommands) -> Result<()> {
    match command {
        ProfileCommands::List { profile_type } => list_profiles(profile_type).await,
        ProfileCommands::Show { name } => show_profile(name).await,
        ProfileCommands::Apply { name, output } => apply_profile(name, output.as_deref()).await,
    }
}

async fn list_profiles(profile_type: &str) -> Result<()> {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                      GPU Profiles                                    ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();

    let profile_manager = GpuProfileManager::new();

    let show_gaming = profile_type == "all" || profile_type == "gaming";
    let show_ai = profile_type == "all" || profile_type == "ai";

    if show_gaming {
        println!("🎮 Gaming Profiles:");
        println!("─────────────────────────────────────────────────────────────────────────");
        for name in profile_manager.list_gaming_profiles() {
            if let Some(settings) = profile_manager.get_gaming_profile(&name) {
                println!("  • {}", settings.profile_name);
                println!(
                    "    DLSS: {} | RT: {} | Target FPS: {}",
                    if settings.dlss_enabled { "Yes" } else { "No" },
                    if settings.raytracing_enabled {
                        "Yes"
                    } else {
                        "No"
                    },
                    settings.target_fps
                );
            }
        }
        println!();
    }

    if show_ai {
        println!("🧠 AI/ML Profiles:");
        println!("─────────────────────────────────────────────────────────────────────────");
        for name in profile_manager.list_ai_profiles() {
            if let Some(settings) = profile_manager.get_ai_profile(&name) {
                let quant = settings
                    .quantization
                    .as_ref()
                    .map(|q| format!("{:?}", q))
                    .unwrap_or_else(|| "None".to_string());
                println!("  • {}", name);
                println!(
                    "    Model: {:?} | Quant: {} | Flash Attn: {}",
                    settings.model_size,
                    quant,
                    if settings.flash_attention {
                        "Yes"
                    } else {
                        "No"
                    }
                );
            }
        }
        println!();
    }

    println!("💡 Usage:");
    println!("   bolt nv profile show <name>     - Show profile details");
    println!("   bolt nv profile apply <name>    - Generate CDI spec with profile");
    println!("   bolt run --gpu all --gpu-profile <name> <image>  - Run with profile");
    println!();

    Ok(())
}

async fn show_profile(name: &str) -> Result<()> {
    let profile_manager = GpuProfileManager::new();

    // Try gaming profile first
    if let Some(settings) = profile_manager.get_gaming_profile(name) {
        println!();
        println!("╔══════════════════════════════════════════════════════════════════════╗");
        println!("║  Gaming Profile: {:<50} ║", settings.profile_name);
        println!("╚══════════════════════════════════════════════════════════════════════╝");
        println!();
        println!("Performance Settings:");
        println!("  Mode:             {:?}", settings.performance_mode);
        println!("  Target FPS:       {}", settings.target_fps);
        println!("  Expected VRAM:    {} MB", settings.expected_vram_mb);
        println!(
            "  Low Latency:      {}",
            if settings.low_latency_mode {
                "Yes"
            } else {
                "No"
            }
        );
        println!();
        println!("NVIDIA Features:");
        println!(
            "  DLSS:             {}",
            if settings.dlss_enabled {
                format!(
                    "Enabled ({:?})",
                    settings
                        .dlss_mode
                        .as_ref()
                        .unwrap_or(&bolt::gaming::profiles::DlssMode::Balanced)
                )
            } else {
                "Disabled".to_string()
            }
        );
        println!(
            "  Ray Tracing:      {}",
            if settings.raytracing_enabled {
                format!(
                    "Enabled ({:?})",
                    settings
                        .raytracing_quality
                        .as_ref()
                        .unwrap_or(&bolt::gaming::profiles::RaytracingQuality::Medium)
                )
            } else {
                "Disabled".to_string()
            }
        );
        println!(
            "  Reflex:           {}",
            if settings.reflex_enabled {
                "Enabled"
            } else {
                "Disabled"
            }
        );
        println!();

        // Show CDI environment preview
        let profile = GpuProfile::Gaming(settings);
        let env_vars = profile_manager.get_nvidia_cdi_env(&profile);
        println!("CDI Environment Variables:");
        for var in env_vars.iter().take(10) {
            println!("  {}", var);
        }
        if env_vars.len() > 10 {
            println!("  ... and {} more", env_vars.len() - 10);
        }
        println!();

        return Ok(());
    }

    // Try AI profile
    if let Some(settings) = profile_manager.get_ai_profile(name) {
        println!();
        println!("╔══════════════════════════════════════════════════════════════════════╗");
        println!("║  AI/ML Profile: {:<52} ║", name);
        println!("╚══════════════════════════════════════════════════════════════════════╝");
        println!();
        println!("Model Settings:");
        println!("  Name:             {}", settings.model_name);
        println!("  Size:             {:?}", settings.model_size);
        println!("  Quantization:     {:?}", settings.quantization);
        println!("  Context Length:   {:?}", settings.context_length);
        println!("  Batch Size:       {:?}", settings.batch_size);
        println!();
        println!("Performance Settings:");
        println!(
            "  Flash Attention:  {}",
            if settings.flash_attention {
                "Yes"
            } else {
                "No"
            }
        );
        println!(
            "  Tensor Parallel:  {}",
            if settings.tensor_parallelism {
                "Yes"
            } else {
                "No"
            }
        );
        println!(
            "  Mixed Precision:  {}",
            if settings.mixed_precision {
                "Yes"
            } else {
                "No"
            }
        );
        println!(
            "  Multi-GPU:        {}",
            if settings.multi_gpu { "Yes" } else { "No" }
        );
        println!();

        // Show CDI environment preview
        let profile = GpuProfile::AiInference(settings);
        let env_vars = profile_manager.get_nvidia_cdi_env(&profile);
        println!("CDI Environment Variables:");
        for var in env_vars.iter().take(10) {
            println!("  {}", var);
        }
        if env_vars.len() > 10 {
            println!("  ... and {} more", env_vars.len() - 10);
        }
        println!();

        return Ok(());
    }

    println!("❌ Profile '{}' not found", name);
    println!();
    println!("💡 Use 'bolt nv profile list' to see available profiles");

    Ok(())
}

async fn apply_profile(name: &str, output: Option<&str>) -> Result<()> {
    let profile_manager = GpuProfileManager::new();
    let nvbind_manager = NvbindManager::detect()?;

    if !nvbind_manager.is_available {
        println!("❌ No NVIDIA GPUs detected");
        return Ok(());
    }

    // Determine profile type and generate CDI
    let (profile_type, cdi_env) = if let Some(settings) = profile_manager.get_gaming_profile(name) {
        let profile = GpuProfile::Gaming(settings);
        ("gaming", profile_manager.get_nvidia_cdi_env(&profile))
    } else if let Some(settings) = profile_manager.get_ai_profile(name) {
        let profile = GpuProfile::AiInference(settings);
        ("aiml", profile_manager.get_nvidia_cdi_env(&profile))
    } else {
        println!("❌ Profile '{}' not found", name);
        return Ok(());
    };

    println!("🔧 Generating CDI specification with profile: {}", name);
    println!("   Type: {}", profile_type);

    // Generate base CDI spec
    let mut cdi_spec = match profile_type {
        "gaming" => nvbind_manager.generate_gaming_cdi_spec().await?,
        _ => nvbind_manager.generate_aiml_cdi_spec().await?,
    };

    // Add profile-specific environment
    cdi_spec.container_edits.env.extend(cdi_env);

    let spec_json = serde_json::to_string_pretty(&cdi_spec)?;

    // Show summary
    println!();
    println!("📋 CDI Specification Summary:");
    println!("   • Version: {}", cdi_spec.cdi_version);
    println!("   • Kind: {}", cdi_spec.kind);
    println!("   • GPUs: {}", cdi_spec.devices.len());
    println!(
        "   • Device nodes: {}",
        cdi_spec.container_edits.device_nodes.len()
    );
    println!(
        "   • Environment vars: {}",
        cdi_spec.container_edits.env.len()
    );
    println!();

    if let Some(path) = output {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &spec_json)?;
        println!("✅ CDI spec written to: {}", path);
        println!();
        println!("💡 Usage:");
        println!("   bolt run --gpu all <image>  - Container runtime will auto-discover spec");
    } else {
        println!("{}", spec_json);
    }

    Ok(())
}
