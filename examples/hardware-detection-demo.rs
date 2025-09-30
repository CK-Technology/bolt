//! Hardware Detection Demo
//!
//! Demonstrates Bolt's comprehensive CPU and GPU detection with vendor-specific optimizations.
//!
//! Features:
//! - AMD Ryzen Zen/Zen2/Zen3/Zen4 with 3D V-Cache detection
//! - Intel Alder Lake / Raptor Lake hybrid architecture (P-cores + E-cores)
//! - Intel GPU with Quick Sync video acceleration
//! - Automatic CPU affinity optimization per workload type

use bolt::runtime::hardware_detection::{HardwareProfile, WorkloadType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("🔍 Bolt Hardware Detection Demo\n");

    // Detect hardware
    let hw = HardwareProfile::detect().await?;

    // Display CPU information
    println!("🖥️  CPU Information:");
    println!("   Vendor: {:?}", hw.cpu.vendor);
    println!("   Model: {}", hw.cpu.model_name);
    println!("   Architecture: {:?}", hw.cpu.architecture);
    println!("   Physical Cores: {}", hw.cpu.cores_physical);
    println!("   Logical Cores: {}", hw.cpu.cores_logical);

    if let Some(ref zen_gen) = hw.cpu.zen_generation {
        println!("   ⚡ AMD Zen Generation: {:?}", zen_gen);
        if hw.cpu.has_3d_vcache {
            println!("   🎮 3D V-Cache: YES (gaming optimized!)");
        }
        if let Some(ccd_count) = hw.cpu.ccd_count {
            println!("   📦 CCD Count: {}", ccd_count);
        }
    }

    if let Some(ref hybrid) = hw.cpu.hybrid_architecture {
        println!("   🔀 Intel Hybrid Architecture:");
        println!(
            "      P-cores: {} @ {:.1} GHz",
            hybrid.p_cores, hybrid.p_core_base_freq
        );
        println!(
            "      E-cores: {} @ {:.1} GHz",
            hybrid.e_cores, hybrid.e_core_base_freq
        );
        println!(
            "      Thread Director: {}",
            if hybrid.thread_director { "✅" } else { "❌" }
        );
    }

    if let Some(cache_kb) = hw.cpu.cache_l3_kb {
        println!("   💾 L3 Cache: {} MB", cache_kb / 1024);
    }

    println!("   🌐 NUMA Nodes: {}", hw.cpu.numa_nodes);

    // Display GPU information
    println!("\n🎮 GPU Information:");
    if hw.gpu.is_empty() {
        println!("   No GPUs detected");
    } else {
        for (i, gpu) in hw.gpu.iter().enumerate() {
            println!("   GPU {}: {:?}", i, gpu.vendor);
            println!("      Device: {}", gpu.device_path.display());
            if let Some(ref render) = gpu.render_node {
                println!("      Render Node: {}", render.display());
            }
            if let Some(ref pci) = gpu.pci_id {
                println!("      PCI ID: {}", pci);
            }

            println!("      Capabilities:");
            if gpu.capabilities.cuda {
                println!("        ✅ CUDA (NVIDIA compute)");
            }
            if gpu.capabilities.rocm {
                println!("        ✅ ROCm (AMD compute)");
            }
            if gpu.capabilities.opencl {
                println!("        ✅ OpenCL");
            }
            if gpu.capabilities.vulkan {
                println!("        ✅ Vulkan");
            }
            if gpu.capabilities.vaapi {
                println!("        ✅ VA-API (video acceleration)");
            }
            if gpu.capabilities.quick_sync {
                println!("        ✅ Intel Quick Sync (hardware video)");
            }
            if gpu.capabilities.nvenc_nvdec {
                println!("        ✅ NVENC/NVDEC (NVIDIA video)");
            }
            if gpu.capabilities.vce_vcn {
                println!("        ✅ VCE/VCN (AMD video)");
            }
            if gpu.capabilities.ray_tracing {
                println!("        ✅ Ray Tracing");
            }
            if gpu.capabilities.dlss {
                println!("        ✅ DLSS (NVIDIA AI upscaling)");
            }
            if gpu.capabilities.fsr {
                println!("        ✅ FSR (AMD upscaling)");
            }
            if gpu.capabilities.xess {
                println!("        ✅ XeSS (Intel upscaling)");
            }
        }
    }

    // Display Memory information
    println!("\n💾 Memory Information:");
    println!("   Total: {} GB", hw.memory.total_mb / 1024);
    println!("   NUMA Nodes: {}", hw.memory.numa_nodes);
    println!("   2MB Hugepages: {}", hw.memory.hugepages_2mb);
    println!("   1GB Hugepages: {}", hw.memory.hugepages_1gb);

    // Show optimal CPU affinity for different workload types
    println!("\n🎯 Optimal CPU Affinity Recommendations:");

    let gaming_affinity = hw.optimal_cpu_affinity(WorkloadType::Gaming);
    println!("   Gaming: {:?}", gaming_affinity);

    let performance_affinity = hw.optimal_cpu_affinity(WorkloadType::HighPerformance);
    println!("   High Performance: {:?}", performance_affinity);

    let background_affinity = hw.optimal_cpu_affinity(WorkloadType::Background);
    println!("   Background Tasks: {:?}", background_affinity);

    // Practical recommendations
    println!("\n💡 Optimization Recommendations:");

    if hw.cpu.has_3d_vcache {
        println!("   🎮 Detected AMD 3D V-Cache CPU:");
        println!("      → Pin gaming containers to first CCD for maximum cache benefit");
        println!("      → Use cores 0-7 for best gaming performance");
    }

    if hw.cpu.hybrid_architecture.is_some() {
        println!("   🔀 Detected Intel Hybrid Architecture:");
        println!("      → Pin gaming/performance containers to P-cores");
        println!("      → Route background tasks to E-cores");
        println!("      → Let Thread Director handle scheduling when possible");
    }

    if hw.gpu.iter().any(|g| g.capabilities.quick_sync) {
        println!("   🎬 Intel Quick Sync available:");
        println!("      → Use for hardware video encoding/decoding");
        println!("      → Offload media transcoding to GPU");
    }

    if hw.gpu.iter().any(|g| g.capabilities.dlss) {
        println!("   🚀 NVIDIA DLSS available:");
        println!("      → Enable AI-powered upscaling for gaming");
    }

    if hw.gpu.iter().any(|g| g.capabilities.fsr) {
        println!("   🚀 AMD FSR available:");
        println!("      → Enable upscaling for better performance");
    }

    println!("\n✅ Hardware detection complete!");

    Ok(())
}
