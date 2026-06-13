//! AMD GPU management commands (`bolt amd`)
//!
//! Native AMD GPU management with ROCm integration.

use crate::Result;
use bolt::runtime::gpu::amd::{AmdArchitecture, AmdManager};
use clap::Subcommand;
use tracing::info;

#[derive(Subcommand)]
pub enum AmdCommands {
    /// Show AMD GPU information
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

    /// Show ROCm status and management
    Rocm {
        #[command(subcommand)]
        command: RocmCommands,
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
pub enum RocmCommands {
    /// Check ROCm installation status
    Status,

    /// Show ROCm version and capabilities
    Info,
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

pub async fn execute(command: &AmdCommands) -> Result<()> {
    match command {
        AmdCommands::Info { format, detailed } => show_info(format, *detailed).await,
        AmdCommands::Doctor { fix } => run_doctor(*fix).await,
        AmdCommands::Rocm { command } => execute_rocm(command).await,
        AmdCommands::Cdi { command } => execute_cdi(command).await,
        AmdCommands::Arch { gpu } => show_architecture(*gpu).await,
    }
}

async fn show_info(format: &str, detailed: bool) -> Result<()> {
    info!("🔍 Detecting AMD GPUs...");

    let manager = AmdManager::detect()?;

    if !manager.is_available {
        println!("\n⚠️  No AMD GPUs detected or driver not loaded\n");
        println!("Possible causes:");
        println!("  • AMD driver not installed");
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
    println!("║                       AMD GPU Information                            ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();

    println!("📦 Driver Information:");
    println!("   Version:     {}", manager.driver_version);
    println!("   Type:        {}", manager.driver_type.name());

    if let Some(ref rocm) = manager.rocm_version {
        println!("   ROCm:        {}", rocm);
    }

    println!();
    println!("🎮 Detected GPUs: {}", manager.gpus.len());
    println!();

    for gpu in &manager.gpus {
        println!("┌─────────────────────────────────────────────────────────────────────┐");
        println!("│ GPU {}: {}", gpu.index, gpu.name);
        println!("├─────────────────────────────────────────────────────────────────────┤");
        println!("│  PCI Address:      {}", gpu.pci_bus_id);
        println!("│  Device Path:      {}", gpu.device_path);
        println!("│  Render Path:      {}", gpu.render_path);
        println!("│  Architecture:     {}", gpu.architecture.name());
        println!("│  Memory:           {} MB", gpu.memory_mb);

        if detailed {
            println!(
                "│  ROCm Support:     {}",
                if gpu.architecture.supports_rocm() {
                    "Yes"
                } else {
                    "No"
                }
            );
            println!(
                "│  Ray Tracing:      {}",
                if gpu.architecture.supports_raytracing() {
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
    println!("║                       AMD GPU Diagnostics                            ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();

    let mut issues = Vec::new();

    // Check AMDGPU driver
    print!("🔍 Checking AMD driver... ");
    if let Ok(modules) = std::fs::read_to_string("/proc/modules") {
        if modules.contains("amdgpu ") {
            println!("✅ amdgpu loaded");
        } else if modules.contains("radeon ") {
            println!("⚠️  radeon driver (legacy)");
            issues.push("Consider using amdgpu driver for newer GPUs");
        } else {
            println!("❌ Not loaded");
            issues.push("AMD driver not loaded");
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

    // Check KFD for ROCm
    print!("🔍 Checking KFD (ROCm)... ");
    if std::path::Path::new("/dev/kfd").exists() {
        println!("✅ Present");
    } else {
        println!("⚠️  Not present (ROCm compute not available)");
    }

    // Check ROCm installation
    print!("🔍 Checking ROCm... ");
    if std::path::Path::new("/opt/rocm").exists() {
        if let Ok(version) = std::fs::read_to_string("/opt/rocm/.info/version") {
            println!("✅ Version {}", version.trim());
        } else {
            println!("✅ Installed (version unknown)");
        }
    } else {
        println!("ℹ️  Not installed");
    }

    // Check Vulkan
    print!("🔍 Checking Vulkan ICD... ");
    let vulkan_paths = [
        "/usr/share/vulkan/icd.d/radeon_icd.x86_64.json",
        "/etc/vulkan/icd.d/radeon_icd.x86_64.json",
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
        issues.push("AMD Vulkan ICD not found - install mesa-vulkan-radeon");
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
            println!("   (Auto-fix not yet implemented for AMD)");
        }
    }

    Ok(())
}

async fn execute_rocm(command: &RocmCommands) -> Result<()> {
    match command {
        RocmCommands::Status => {
            println!();
            println!("🔍 ROCm Status");
            println!("─────────────────────────────────────────────────────────────────────────");

            // Check ROCm installation
            if std::path::Path::new("/opt/rocm").exists() {
                println!("✅ ROCm installed at /opt/rocm");

                if let Ok(version) = std::fs::read_to_string("/opt/rocm/.info/version") {
                    println!("   Version: {}", version.trim());
                }

                // Check KFD
                if std::path::Path::new("/dev/kfd").exists() {
                    println!("✅ KFD device available (/dev/kfd)");
                } else {
                    println!("❌ KFD device not available");
                    println!("   ROCm compute will not work without /dev/kfd");
                }

                // Check rocm-smi
                if std::process::Command::new("rocm-smi")
                    .arg("--version")
                    .output()
                    .is_ok()
                {
                    println!("✅ rocm-smi available");
                } else {
                    println!("⚠️  rocm-smi not in PATH");
                }
            } else {
                println!("❌ ROCm not installed");
                println!();
                println!("💡 Install ROCm:");
                println!("   https://rocm.docs.amd.com/en/latest/deploy/linux/quick_start.html");
            }

            Ok(())
        }
        RocmCommands::Info => {
            let manager = AmdManager::detect()?;

            println!();
            println!("🔍 ROCm Information");
            println!("─────────────────────────────────────────────────────────────────────────");

            if let Some(ref version) = manager.rocm_version {
                println!("Version:      {}", version);
            } else {
                println!("Version:      Not installed");
            }

            println!();
            println!("ROCm-compatible GPUs:");
            for gpu in &manager.gpus {
                let status = if gpu.architecture.supports_rocm() {
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
        "🔧 Generating {} CDI specification for AMD (native)...",
        profile
    );

    let manager = AmdManager::detect()?;

    if !manager.is_available {
        println!("❌ No AMD GPUs detected");
        return Ok(());
    }

    // Generate CDI spec based on profile
    let cdi_spec = match profile.to_lowercase().as_str() {
        "gaming" => {
            println!("🎮 Using gaming profile (optimized for Vulkan/gaming)");
            manager.generate_gaming_cdi_spec().await?
        }
        "aiml" | "ai" | "ml" => {
            println!("🧠 Using AI/ML profile (optimized for ROCm compute)");
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
    println!("🔍 Searching for AMD CDI specifications...");
    println!();

    let cdi_paths = ["/etc/cdi", "/var/run/cdi"];

    let mut found = 0;
    for path in &cdi_paths {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains("amd") && (name.ends_with(".json") || name.ends_with(".yaml")) {
                    println!("  📄 {}/{}", path, name);
                    found += 1;
                }
            }
        }
    }

    if found == 0 {
        println!("  No AMD CDI specifications found");
        println!();
        println!("💡 Generate one with: bolt amd cdi generate --output /etc/cdi/amd.json");
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
    println!("║                 AMD CDI (Container Device Interface)                 ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();
    println!("CDI provides vendor-neutral device exposure to containers.");
    println!("Bolt generates native AMD CDI specs for ROCm and Vulkan workloads.");
    println!();
    println!("📋 Available Profiles:");
    println!("   • general  - Balanced for all workloads");
    println!("   • gaming   - Optimized for Vulkan, RADV, gaming");
    println!("   • aiml     - Optimized for ROCm compute workloads");
    println!();
    println!("💡 Usage Examples:");
    println!("   bolt amd cdi generate --output /etc/cdi/amd.json");
    println!("   bolt amd cdi generate --profile gaming -o /etc/cdi/amd-gaming.json");
    println!("   bolt amd cdi validate /etc/cdi/amd.json");
    println!();

    Ok(())
}

async fn show_architecture(gpu_index: u32) -> Result<()> {
    let manager = AmdManager::detect()?;

    if !manager.is_available {
        println!("❌ No AMD GPUs detected");
        return Ok(());
    }

    let gpu = manager.gpus.iter().find(|g| g.index == gpu_index);

    if let Some(gpu) = gpu {
        println!();
        println!("╔══════════════════════════════════════════════════════════════════════╗");
        println!("║                   AMD GPU Architecture Details                       ║");
        println!("╚══════════════════════════════════════════════════════════════════════╝");
        println!();
        println!("GPU {}:             {}", gpu.index, gpu.name);
        println!("Architecture:      {}", gpu.architecture.name());
        println!();
        println!("Capabilities:");
        println!(
            "  • ROCm Support:    {}",
            if gpu.architecture.supports_rocm() {
                "Yes"
            } else {
                "No"
            }
        );
        println!(
            "  • Ray Tracing:     {}",
            if gpu.architecture.supports_raytracing() {
                "Yes"
            } else {
                "No"
            }
        );
        println!();

        match gpu.architecture {
            AmdArchitecture::RDNA3 | AmdArchitecture::RDNA4 => {
                println!("Features:");
                println!("  • Unified Compute Units");
                println!("  • Hardware Ray Tracing");
                println!("  • AI Accelerators");
                println!("  • DisplayPort 2.1");
            }
            AmdArchitecture::RDNA2 => {
                println!("Features:");
                println!("  • Infinity Cache");
                println!("  • Hardware Ray Tracing");
                println!("  • Smart Access Memory");
            }
            AmdArchitecture::CDNA2 | AmdArchitecture::CDNA3 => {
                println!("Features:");
                println!("  • Matrix Cores");
                println!("  • High Bandwidth Memory (HBM)");
                println!("  • Infinity Fabric");
                println!("  • Multi-Instance GPU");
            }
            _ => {}
        }
    } else {
        println!("❌ GPU {} not found", gpu_index);
    }

    Ok(())
}
