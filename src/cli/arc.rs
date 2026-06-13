//! Intel Arc GPU management commands (`bolt arc`)
//!
//! Native Intel Arc GPU management with oneAPI/Level Zero integration.

use crate::Result;
use bolt::runtime::gpu::intel::{IntelArchitecture, IntelManager};
use clap::Subcommand;
use tracing::info;

#[derive(Subcommand)]
pub enum ArcCommands {
    /// Show Intel Arc GPU information
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

    /// Show oneAPI/Level Zero status
    Oneapi {
        #[command(subcommand)]
        command: OneapiCommands,
    },

    /// CDI (Container Device Interface) management
    Cdi {
        #[command(subcommand)]
        command: CdiCommands,
    },

    /// Show GPU architecture details
    Arch {
        /// GPU index (default: 0)
        #[arg(short, long, default_value = "0")]
        gpu: u32,
    },
}

#[derive(Subcommand)]
pub enum OneapiCommands {
    /// Check oneAPI installation status
    Status,

    /// Show Level Zero information
    LevelZero,
}

#[derive(Subcommand)]
pub enum CdiCommands {
    /// Generate CDI specification
    Generate {
        /// Output file path
        #[arg(short, long)]
        output: Option<String>,

        /// Profile type (general, gaming, aiml)
        #[arg(short, long, default_value = "general")]
        profile: String,
    },

    /// List existing CDI specifications
    List,

    /// Validate a CDI specification file
    Validate {
        /// Path to CDI spec file
        file: String,
    },

    /// Show CDI information
    Info,
}

pub async fn execute(command: &ArcCommands) -> Result<()> {
    match command {
        ArcCommands::Info { format, detailed } => show_info(format, *detailed).await,
        ArcCommands::Doctor { fix } => run_doctor(*fix).await,
        ArcCommands::Oneapi { command } => execute_oneapi(command).await,
        ArcCommands::Cdi { command } => execute_cdi(command).await,
        ArcCommands::Arch { gpu } => show_architecture(*gpu).await,
    }
}

async fn show_info(format: &str, detailed: bool) -> Result<()> {
    info!("🔍 Detecting Intel GPUs...");

    let manager = IntelManager::detect()?;

    if !manager.is_available {
        println!("\n⚠️  No Intel GPUs detected or driver not loaded\n");
        println!("Possible causes:");
        println!("  • Intel GPU driver not installed");
        println!("  • GPU not detected by system");
        println!("  • Missing /dev/dri devices\n");
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
    println!("║                      Intel GPU Information                           ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();

    println!("📦 Driver Information:");
    println!("   Version:     {}", manager.driver_version);
    println!("   Type:        {}", manager.driver_type.name());

    if let Some(ref oneapi) = manager.oneapi_version {
        println!("   oneAPI:      {}", oneapi);
    }
    if let Some(ref l0) = manager.level_zero_version {
        println!("   Level Zero:  {}", l0);
    }

    println!();
    println!("🎮 Detected GPUs: {}", manager.gpus.len());
    println!();

    for gpu in &manager.gpus {
        let gpu_type = if gpu.is_discrete {
            "Discrete"
        } else {
            "Integrated"
        };
        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ GPU {}: {}", gpu.index, gpu.name);
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!("│  Type:           {}", gpu_type);
        println!("│  PCI Address:    {}", gpu.pci_bus_id);
        println!("│  Device Path:    {}", gpu.device_path);
        println!("│  Render Path:    {}", gpu.render_path);
        println!("│  Architecture:   {}", gpu.architecture.name());
        if gpu.memory_mb > 0 {
            println!("│  VRAM:           {} MB", gpu.memory_mb);
        } else {
            println!("│  Memory:         Shared (System RAM)");
        }

        if detailed {
            println!(
                "│  Ray Tracing:    {}",
                if gpu.architecture.supports_raytracing() {
                    "Yes"
                } else {
                    "No"
                }
            );
            println!(
                "│  XeSS Support:   {}",
                if gpu.architecture.supports_xess() {
                    "Yes"
                } else {
                    "No"
                }
            );
            println!(
                "│  oneAPI:         {}",
                if gpu.architecture.supports_oneapi() {
                    "Yes"
                } else {
                    "No"
                }
            );
        }

        println!("└─────────────────────────────────────────────────────────────────────┘");
    }

    Ok(())
}

async fn run_doctor(fix: bool) -> Result<()> {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                      Intel GPU Diagnostics                           ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();

    let mut issues = Vec::new();

    // Check kernel driver
    print!("🔍 Checking Intel driver... ");
    if let Ok(modules) = std::fs::read_to_string("/proc/modules") {
        if modules.contains("xe ") {
            println!("✅ xe driver loaded (modern)");
        } else if modules.contains("i915 ") {
            println!("✅ i915 driver loaded");
        } else {
            println!("❌ Not loaded");
            issues.push("Intel graphics driver not loaded");
        }
    } else {
        println!("❌ Could not check");
    }

    // Check DRI devices
    print!("🔍 Checking DRI devices... ");
    if std::path::Path::new("/dev/dri").exists() {
        let mut count = 0;
        if let Ok(entries) = std::fs::read_dir("/dev/dri") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("card") || name.starts_with("renderD") {
                    count += 1;
                }
            }
        }
        if count > 0 {
            println!("✅ {} devices found", count);
        } else {
            println!("❌ No devices found");
            issues.push("No DRI devices found");
        }
    } else {
        println!("❌ /dev/dri not found");
        issues.push("/dev/dri directory not found");
    }

    // Check oneAPI
    print!("🔍 Checking oneAPI... ");
    if std::path::Path::new("/opt/intel/oneapi").exists() {
        println!("✅ Installed");
    } else {
        println!("ℹ️  Not installed");
    }

    // Check Level Zero
    print!("🔍 Checking Level Zero... ");
    let l0_paths = [
        "/usr/lib/x86_64-linux-gnu/libze_loader.so",
        "/usr/lib64/libze_loader.so",
    ];
    let mut l0_found = false;
    for path in &l0_paths {
        if std::path::Path::new(path).exists() {
            println!("✅ Found");
            l0_found = true;
            break;
        }
    }
    if !l0_found {
        println!("ℹ️  Not found");
    }

    // Check Vulkan
    print!("🔍 Checking Vulkan ICD... ");
    let vulkan_paths = [
        "/usr/share/vulkan/icd.d/intel_icd.x86_64.json",
        "/etc/vulkan/icd.d/intel_icd.x86_64.json",
    ];
    let mut vulkan_found = false;
    for path in &vulkan_paths {
        if std::path::Path::new(path).exists() {
            println!("✅ Found");
            vulkan_found = true;
            break;
        }
    }
    if !vulkan_found {
        println!("⚠️  Not found");
        issues.push("Intel Vulkan ICD not found - install mesa-vulkan-intel");
    }

    println!();
    println!("─────────────────────────────────────────────────────────────────────────");

    if issues.is_empty() {
        println!("✅ All checks passed! GPU is ready for container workloads.");
    } else {
        println!("⚠️  {} issue(s) found:", issues.len());
        for issue in &issues {
            println!("   • {}", issue);
        }

        if fix {
            println!();
            println!("🔧 Attempting fixes...");
            println!("   (Auto-fix not yet implemented for Intel)");
        }
    }

    Ok(())
}

async fn execute_oneapi(command: &OneapiCommands) -> Result<()> {
    match command {
        OneapiCommands::Status => {
            println!();
            println!("🔍 oneAPI Status");
            println!("─────────────────────────────────────────────────────────────────────────");

            if std::path::Path::new("/opt/intel/oneapi").exists() {
                println!("✅ oneAPI installed at /opt/intel/oneapi");

                // Check components
                let components = [
                    ("compiler", "DPC++ Compiler"),
                    ("mkl", "Math Kernel Library"),
                    ("dnn", "Deep Neural Networks"),
                    ("tbb", "Threading Building Blocks"),
                ];

                println!();
                println!("Components:");
                for (dir, name) in &components {
                    let path = format!("/opt/intel/oneapi/{}", dir);
                    if std::path::Path::new(&path).exists() {
                        println!("  ✅ {}", name);
                    }
                }
            } else {
                println!("❌ oneAPI not installed");
                println!();
                println!("💡 Install oneAPI:");
                println!(
                    "   https://www.intel.com/content/www/us/en/developer/tools/oneapi/base-toolkit.html"
                );
            }

            Ok(())
        }
        OneapiCommands::LevelZero => {
            let manager = IntelManager::detect()?;

            println!();
            println!("🔍 Level Zero Information");
            println!("─────────────────────────────────────────────────────────────────────────");

            if let Some(ref version) = manager.level_zero_version {
                println!("Version: {}", version);
            } else {
                println!("Level Zero not installed");
            }

            println!();
            println!("oneAPI-compatible GPUs:");
            for gpu in &manager.gpus {
                let status = if gpu.architecture.supports_oneapi() {
                    "✅"
                } else {
                    "❌"
                };
                println!(
                    "  {} GPU {}: {} ({})",
                    status,
                    gpu.index,
                    gpu.name,
                    gpu.architecture.name()
                );
            }

            Ok(())
        }
    }
}

async fn execute_cdi(command: &CdiCommands) -> Result<()> {
    match command {
        CdiCommands::Generate { output, profile } => generate_cdi(output.as_deref(), profile).await,
        CdiCommands::List => list_cdi().await,
        CdiCommands::Validate { file } => validate_cdi(file).await,
        CdiCommands::Info => show_cdi_info().await,
    }
}

async fn generate_cdi(output: Option<&str>, profile: &str) -> Result<()> {
    println!(
        "🔧 Generating {} CDI specification for Intel (native)...",
        profile
    );

    let manager = IntelManager::detect()?;

    if !manager.is_available {
        println!("❌ No Intel GPUs detected");
        return Ok(());
    }

    // Generate CDI spec based on profile
    let cdi_spec = match profile.to_lowercase().as_str() {
        "gaming" => {
            println!("🎮 Using gaming profile (optimized for Vulkan/XeSS)");
            manager.generate_gaming_cdi_spec().await?
        }
        "aiml" | "ai" | "ml" => {
            println!("🧠 Using AI/ML profile (optimized for oneAPI/Level Zero)");
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
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &spec_json)?;
        println!("✅ CDI spec written to: {}", path);
    } else {
        println!("{}", spec_json);
    }

    Ok(())
}

async fn list_cdi() -> Result<()> {
    println!("🔍 Searching for Intel CDI specifications...");
    println!();

    let cdi_paths = ["/etc/cdi", "/var/run/cdi"];

    let mut found = 0;
    for path in &cdi_paths {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains("intel") && (name.ends_with(".json") || name.ends_with(".yaml")) {
                    println!("  📄 {}/{}", path, name);
                    found += 1;
                }
            }
        }
    }

    if found == 0 {
        println!("  No Intel CDI specifications found");
        println!();
        println!("💡 Generate one with: bolt arc cdi generate --output /etc/cdi/intel.json");
    }

    Ok(())
}

async fn validate_cdi(file: &str) -> Result<()> {
    println!("🔍 Validating CDI specification: {}", file);

    let content = std::fs::read_to_string(file)?;
    let spec: serde_json::Value = serde_json::from_str(&content)?;

    let mut errors = Vec::new();

    if spec.get("cdiVersion").is_none() {
        errors.push("Missing 'cdiVersion' field");
    }
    if spec.get("kind").is_none() {
        errors.push("Missing 'kind' field");
    }

    println!();
    if errors.is_empty() {
        println!("✅ CDI specification is valid");
    } else {
        for error in &errors {
            println!("❌ Error: {}", error);
        }
    }

    Ok(())
}

async fn show_cdi_info() -> Result<()> {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║               Intel CDI (Container Device Interface)                 ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();
    println!("CDI provides vendor-neutral device exposure to containers.");
    println!("Bolt generates native Intel CDI specs for oneAPI and Vulkan workloads.");
    println!();
    println!("📋 Available Profiles:");
    println!("   • general  - Balanced for all workloads");
    println!("   • gaming   - Optimized for Vulkan, XeSS, gaming");
    println!("   • aiml     - Optimized for oneAPI/Level Zero compute");
    println!();
    println!("💡 Usage Examples:");
    println!("   bolt arc cdi generate --output /etc/cdi/intel.json");
    println!("   bolt arc cdi generate --profile gaming -o /etc/cdi/intel-gaming.json");
    println!("   bolt arc cdi validate /etc/cdi/intel.json");
    println!();

    Ok(())
}

async fn show_architecture(gpu_index: u32) -> Result<()> {
    let manager = IntelManager::detect()?;

    if !manager.is_available {
        println!("❌ No Intel GPUs detected");
        return Ok(());
    }

    let gpu = manager.gpus.iter().find(|g| g.index == gpu_index);

    if let Some(gpu) = gpu {
        println!();
        println!("╔══════════════════════════════════════════════════════════════════════╗");
        println!("║                  Intel GPU Architecture Details                      ║");
        println!("╚══════════════════════════════════════════════════════════════════════╝");
        println!();
        println!("GPU {}:             {}", gpu.index, gpu.name);
        println!("Architecture:      {}", gpu.architecture.name());
        println!(
            "Type:              {}",
            if gpu.is_discrete {
                "Discrete (Arc)"
            } else {
                "Integrated"
            }
        );
        println!();
        println!("Capabilities:");
        println!(
            "  • Ray Tracing:     {}",
            if gpu.architecture.supports_raytracing() {
                "Yes"
            } else {
                "No"
            }
        );
        println!(
            "  • XeSS Upscaling:  {}",
            if gpu.architecture.supports_xess() {
                "Yes"
            } else {
                "No"
            }
        );
        println!(
            "  • oneAPI/SYCL:     {}",
            if gpu.architecture.supports_oneapi() {
                "Yes"
            } else {
                "No"
            }
        );
        println!();

        match gpu.architecture {
            IntelArchitecture::XeHPG => {
                println!("Features (Arc Alchemist):");
                println!("  • Xe-cores with Ray Tracing Units");
                println!("  • XMX AI acceleration");
                println!("  • AV1 hardware encode/decode");
                println!("  • DisplayPort 2.0");
                println!("  • PCIe 4.0");
            }
            IntelArchitecture::Xe2HPG | IntelArchitecture::Xe2LPG => {
                println!("Features (Xe2 / Battlemage):");
                println!("  • Next-gen Xe-cores");
                println!("  • Enhanced Ray Tracing");
                println!("  • Improved XeSS 2.0");
                println!("  • DisplayPort 2.1");
            }
            IntelArchitecture::XeHPC => {
                println!("Features (Ponte Vecchio):");
                println!("  • High Bandwidth Memory (HBM2e)");
                println!("  • Xe-HPC compute tiles");
                println!("  • FP64 acceleration");
                println!("  • Xe Link interconnect");
            }
            _ => {}
        }
    } else {
        println!("❌ GPU {} not found", gpu_index);
    }

    Ok(())
}
