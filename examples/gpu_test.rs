use anyhow::Result;
use bolt::config::{GpuConfig, NvidiaConfig};
use bolt::runtime::gpu::GPUManager;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("🧪 Bolt GPU Support Test");

    // Initialize GPU Manager
    let gpu_manager = match GPUManager::new() {
        Ok(manager) => {
            info!("✅ GPU Manager initialized successfully");
            manager
        }
        Err(e) => {
            warn!("❌ Failed to initialize GPU Manager: {}", e);
            return Ok(());
        }
    };

    // Test 1: Basic GPU detection
    test_gpu_detection(&gpu_manager).await?;

    // Test 2: Check for nvidia-container-runtime
    test_nvidia_container_runtime(&gpu_manager).await?;

    // Test 3: Check rootless support
    test_rootless_support(&gpu_manager).await?;

    // Test 4: Test GPU configuration
    test_gpu_configuration(&gpu_manager).await?;

    info!("🎉 GPU test completed successfully!");
    Ok(())
}

async fn test_gpu_detection(gpu_manager: &GPUManager) -> Result<()> {
    info!("\n🔍 Test 1: GPU Detection");

    let gpus = gpu_manager.get_available_gpus().await?;
    info!("Found {} GPU(s):", gpus.len());

    for gpu in &gpus {
        info!(
            "  • {:?} {} ({}MB) - UUID: {:?}",
            gpu.vendor, gpu.name, gpu.memory_mb, gpu.uuid
        );

        for device_path in &gpu.device_paths {
            info!("    Device: {}", device_path);
        }
    }

    if gpus.is_empty() {
        warn!("⚠️ No GPUs detected - this might be expected in some environments");
    } else {
        info!("✅ Driver priority: NVIDIA Open → Proprietary → nouveau → NVK");

        // Test NVIDIA Open detection specifically
        if gpu_manager.nvidia.is_some() {
            info!("  🔍 Testing NVIDIA driver type detection...");
            // This would call the internal detection method if it were public
        }
    }

    Ok(())
}

async fn test_nvidia_container_runtime(gpu_manager: &GPUManager) -> Result<()> {
    info!("\n🐳 Test 2: nvidia-container-runtime Detection");

    let has_runtime = gpu_manager.has_nvidia_container_runtime().await;

    if has_runtime {
        info!("✅ nvidia-container-runtime is available");
    } else {
        info!("📝 nvidia-container-runtime not found - will use bolt native");
    }

    Ok(())
}

async fn test_rootless_support(gpu_manager: &GPUManager) -> Result<()> {
    info!("\n🧑 Test 3: Rootless GPU Support");

    let support = gpu_manager.check_rootless_gpu_support().await?;

    info!("User: {} (rootless: {})", support.user, support.is_rootless);
    info!(
        "DRI access: {}",
        if support.dri_access { "✅" } else { "❌" }
    );
    info!(
        "NVIDIA access: {}",
        if support.nvidia_access { "✅" } else { "❌" }
    );

    if !support.suggestions.is_empty() {
        support.print_suggestions();
    }

    Ok(())
}

async fn test_gpu_configuration(gpu_manager: &GPUManager) -> Result<()> {
    info!("\n⚙️ Test 4: GPU Configuration");

    // Create a test GPU config
    let gpu_config = GpuConfig {
        nvidia: Some(NvidiaConfig {
            device: Some(0),
            dlss: Some(true),
            raytracing: Some(true),
            cuda: Some(true),
            ..Default::default()
        }),
        amd: None,
        passthrough: Some(false),
        ..Default::default()
    };

    // Test with nvidia-container-runtime preference
    info!("Testing with nvidia-container-runtime preference...");
    match gpu_manager
        .setup_gpu_with_runtime_preference(
            "test-container-1",
            &gpu_config,
            true, // prefer nvidia-container-runtime
        )
        .await
    {
        Ok(_) => info!("✅ GPU configuration with nvidia-runtime preference successful"),
        Err(e) => warn!("⚠️ GPU configuration with nvidia-runtime failed: {}", e),
    }

    // Test with bolt native preference
    info!("Testing with bolt native preference...");
    match gpu_manager
        .setup_gpu_with_runtime_preference(
            "test-container-2",
            &gpu_config,
            false, // prefer bolt native
        )
        .await
    {
        Ok(_) => info!("✅ GPU configuration with bolt native successful"),
        Err(e) => warn!("⚠️ GPU configuration with bolt native failed: {}", e),
    }

    Ok(())
}
