//! GPU Integration Tests
//!
//! Tests for the Bolt GPU integration layer, including native NVIDIA,
//! AMD, and Intel GPU support.

use bolt::runtime::gpu::GPUManager;
use bolt::runtime::gpu::profiles::{GpuProfile, GpuProfileManager};
use bolt::runtime::gpu_integration::{
    BoltGpuIntegration, GpuConfig, GpuIsolationLevel, GpuWorkloadType,
};

#[tokio::test]
async fn test_gpu_integration_initialization() {
    let gpu_integration = BoltGpuIntegration::new().await;
    assert!(
        gpu_integration.is_ok(),
        "GPU integration should initialize successfully"
    );
}

#[tokio::test]
async fn test_gpu_manager_initialization() {
    let gpu_manager = GPUManager::new();
    assert!(
        gpu_manager.is_ok(),
        "GPU manager should initialize successfully"
    );
}

#[tokio::test]
async fn test_gpu_config_general_workload() {
    let config = GpuConfig {
        enabled: true,
        workload_type: GpuWorkloadType::General,
        isolation_level: GpuIsolationLevel::Shared,
        memory_limit: None,
        snapshot_support: false,
        quick_sync: None,
    };

    assert!(config.enabled);
    assert!(matches!(config.workload_type, GpuWorkloadType::General));
    assert!(matches!(config.isolation_level, GpuIsolationLevel::Shared));
}

#[tokio::test]
async fn test_gpu_config_gaming_workload() {
    let config = GpuConfig {
        enabled: true,
        workload_type: GpuWorkloadType::Gaming {
            dlss_enabled: true,
            raytracing_enabled: true,
            performance_profile: "ultra-low-latency".to_string(),
            wine_proton_enabled: true,
            vrs_enabled: false,
        },
        isolation_level: GpuIsolationLevel::Exclusive,
        memory_limit: Some("8G".to_string()),
        snapshot_support: false,
        quick_sync: None,
    };

    assert!(config.enabled);
    if let GpuWorkloadType::Gaming {
        dlss_enabled,
        raytracing_enabled,
        wine_proton_enabled,
        ..
    } = &config.workload_type
    {
        assert!(dlss_enabled);
        assert!(raytracing_enabled);
        assert!(wine_proton_enabled);
    } else {
        panic!("Expected Gaming workload type");
    }
}

#[tokio::test]
async fn test_gpu_config_aiml_workload() {
    let config = GpuConfig {
        enabled: true,
        workload_type: GpuWorkloadType::AiMl {
            cuda_cache_mb: Some(4096),
            tensor_cores_enabled: true,
            mixed_precision_enabled: true,
            memory_pool_size: Some("16G".to_string()),
            mig_enabled: false,
        },
        isolation_level: GpuIsolationLevel::Exclusive,
        memory_limit: None,
        snapshot_support: false,
        quick_sync: None,
    };

    assert!(config.enabled);
    if let GpuWorkloadType::AiMl {
        tensor_cores_enabled,
        mixed_precision_enabled,
        ..
    } = &config.workload_type
    {
        assert!(tensor_cores_enabled);
        assert!(mixed_precision_enabled);
    } else {
        panic!("Expected AiMl workload type");
    }
}

#[tokio::test]
async fn test_gpu_isolation_levels() {
    let levels = vec![
        GpuIsolationLevel::Shared,
        GpuIsolationLevel::Exclusive,
        GpuIsolationLevel::Virtual,
    ];

    for level in levels {
        let config = GpuConfig {
            enabled: true,
            workload_type: GpuWorkloadType::General,
            isolation_level: level.clone(),
            memory_limit: None,
            snapshot_support: false,
            quick_sync: None,
        };

        // Just verify the config can be created with each isolation level
        assert!(config.enabled);
    }
}

#[tokio::test]
async fn test_container_gpu_setup() {
    let gpu_integration = BoltGpuIntegration::new().await.unwrap();
    let container_id = "test_container_gpu_setup";

    let gpu_config = GpuConfig {
        enabled: true,
        workload_type: GpuWorkloadType::General,
        isolation_level: GpuIsolationLevel::Shared,
        memory_limit: None,
        snapshot_support: false,
        quick_sync: None,
    };

    // Should handle gracefully whether GPU hardware is available or not
    let result = gpu_integration
        .setup_gpu_for_container(container_id, &gpu_config)
        .await;

    // The result depends on hardware availability
    // It should either succeed or return an error about no GPU found
    match result {
        Ok(applied_cdi) => {
            // If it succeeds, verify we got a CDI spec
            // (may be empty if fallback mode and no devices found)
            println!(
                "GPU setup succeeded with {} devices",
                applied_cdi.device_nodes.len()
            );
        }
        Err(e) => {
            // Expected when no GPU hardware available
            println!("GPU setup failed (expected on systems without GPU): {}", e);
        }
    }
}

#[tokio::test]
async fn test_container_gaming_gpu_setup() {
    let gpu_integration = BoltGpuIntegration::new().await.unwrap();
    let container_id = "test_gaming_container";

    let gpu_config = GpuConfig {
        enabled: true,
        workload_type: GpuWorkloadType::Gaming {
            dlss_enabled: true,
            raytracing_enabled: true,
            performance_profile: "ultra-low-latency".to_string(),
            wine_proton_enabled: true,
            vrs_enabled: false,
        },
        isolation_level: GpuIsolationLevel::Exclusive,
        memory_limit: Some("8G".to_string()),
        snapshot_support: false,
        quick_sync: None,
    };

    let result = gpu_integration
        .setup_gpu_for_container(container_id, &gpu_config)
        .await;

    match result {
        Ok(applied_cdi) => {
            println!(
                "Gaming GPU setup succeeded with {} devices",
                applied_cdi.device_nodes.len()
            );
        }
        Err(e) => {
            println!(
                "Gaming GPU setup failed (expected on systems without GPU): {}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_container_aiml_gpu_setup() {
    let gpu_integration = BoltGpuIntegration::new().await.unwrap();
    let container_id = "test_aiml_container";

    let gpu_config = GpuConfig {
        enabled: true,
        workload_type: GpuWorkloadType::AiMl {
            cuda_cache_mb: Some(4096),
            tensor_cores_enabled: true,
            mixed_precision_enabled: true,
            memory_pool_size: Some("16G".to_string()),
            mig_enabled: false,
        },
        isolation_level: GpuIsolationLevel::Exclusive,
        memory_limit: None,
        snapshot_support: false,
        quick_sync: None,
    };

    let result = gpu_integration
        .setup_gpu_for_container(container_id, &gpu_config)
        .await;

    match result {
        Ok(applied_cdi) => {
            println!(
                "AI/ML GPU setup succeeded with {} devices",
                applied_cdi.device_nodes.len()
            );
        }
        Err(e) => {
            println!(
                "AI/ML GPU setup failed (expected on systems without GPU): {}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_nvbind_availability() {
    let gpu_integration = BoltGpuIntegration::new().await.unwrap();

    // Check nvbind availability
    let is_available = gpu_integration.is_nvbind_available();
    println!("Native NVIDIA support available: {}", is_available);

    // Either way should not panic
    if is_available {
        let manager = gpu_integration.nvidia_manager();
        assert!(manager.is_some());
    }
}

#[tokio::test]
async fn test_amd_monitor_availability() {
    let gpu_integration = BoltGpuIntegration::new().await.unwrap();

    // Check AMD monitor availability
    let is_available = gpu_integration.is_amd_monitor_available();
    println!("AMD GPU monitoring available: {}", is_available);

    // Should not panic regardless of hardware
    if is_available {
        let gpus = gpu_integration.list_amd_gpus();
        assert!(gpus.is_some());
    }
}

#[tokio::test]
async fn test_gpu_metrics_fallback() {
    let gpu_integration = BoltGpuIntegration::new().await.unwrap();
    let container_id = "test_metrics_container";

    // Should return fallback metrics when no GPU or container not set up
    let metrics = gpu_integration.get_gpu_metrics(container_id).await;
    assert!(metrics.is_ok());

    let m = metrics.unwrap();
    // Fallback metrics are zeros
    assert_eq!(m.utilization, 0.0);
    assert_eq!(m.memory_used, 0);
}

#[tokio::test]
async fn test_disabled_gpu_config() {
    let gpu_integration = BoltGpuIntegration::new().await.unwrap();
    let container_id = "test_disabled_gpu";

    let gpu_config = GpuConfig {
        enabled: false,
        workload_type: GpuWorkloadType::General,
        isolation_level: GpuIsolationLevel::Shared,
        memory_limit: None,
        snapshot_support: false,
        quick_sync: None,
    };

    let result = gpu_integration
        .setup_gpu_for_container(container_id, &gpu_config)
        .await;

    // Should return empty CDI spec when GPU is disabled
    assert!(result.is_ok());
    let applied_cdi = result.unwrap();
    assert!(applied_cdi.is_empty());
}

// ============= Profile Manager Tests =============

#[test]
fn test_profile_manager_creation() {
    let manager = GpuProfileManager::new();

    // Should have gaming and AI profiles
    let gaming_profiles = manager.list_gaming_profiles();
    let ai_profiles = manager.list_ai_profiles();

    assert!(!gaming_profiles.is_empty(), "Should have gaming profiles");
    assert!(!ai_profiles.is_empty(), "Should have AI profiles");
}

#[test]
fn test_gaming_profile_lookup() {
    let manager = GpuProfileManager::new();

    // Try to get a gaming profile
    let profile = manager.get_gaming_profile("cyberpunk 2077");
    assert!(profile.is_some(), "Should find Cyberpunk 2077 profile");

    let settings = profile.unwrap();
    assert!(settings.raytracing_enabled);
    assert!(settings.dlss_enabled);
}

#[test]
fn test_ai_profile_lookup() {
    let manager = GpuProfileManager::new();

    // Try to get an AI profile
    let profile = manager.get_ai_profile("ollama-medium");
    assert!(profile.is_some(), "Should find ollama-medium profile");

    let settings = profile.unwrap();
    assert!(settings.flash_attention);
    assert_eq!(settings.model_name, "llama3:8b");
}

#[test]
fn test_profile_cdi_env_generation() {
    let manager = GpuProfileManager::new();

    // Get a gaming profile and generate CDI env
    let settings = manager.get_gaming_profile("counter-strike 2").unwrap();
    let profile = GpuProfile::Gaming(settings);

    let env = manager.get_nvidia_cdi_env(&profile);

    // Should have NVIDIA visible devices
    assert!(env.iter().any(|e| e.contains("NVIDIA_VISIBLE_DEVICES")));
    // Should have reflex enabled for CS2
    assert!(
        env.iter()
            .any(|e| e.contains("REFLEX") || e.contains("LOW_LATENCY"))
    );
}

#[test]
fn test_ai_profile_cdi_env_generation() {
    let manager = GpuProfileManager::new();

    // Get an AI profile and generate CDI env
    let settings = manager.get_ai_profile("ollama-large").unwrap();
    let profile = GpuProfile::AiInference(settings);

    let env = manager.get_nvidia_cdi_env(&profile);

    // Should have NVIDIA visible devices
    assert!(env.iter().any(|e| e.contains("NVIDIA_VISIBLE_DEVICES")));
    // Should have flash attention for Ollama
    assert!(env.iter().any(|e| e.contains("FLASH_ATTENTION")));
}

#[test]
fn test_profile_env_vars() {
    let manager = GpuProfileManager::new();

    // Use a profile that exists
    let settings = manager.get_gaming_profile("cyberpunk 2077").unwrap();
    let profile = GpuProfile::Gaming(settings);

    let env_map = manager.get_profile_env_vars(&profile);

    assert_eq!(env_map.get("GPU_PROFILE"), Some(&"gaming".to_string()));
    assert!(env_map.contains_key("TARGET_FPS"));
}

#[tokio::test]
async fn test_gpu_manager_profile_integration() {
    let gpu_manager = GPUManager::new();
    if gpu_manager.is_err() {
        println!("GPU manager not available, skipping test");
        return;
    }

    let manager = gpu_manager.unwrap();

    // List profiles
    let gaming = manager.list_gaming_profiles();
    let ai = manager.list_ai_profiles();

    assert!(!gaming.is_empty());
    assert!(!ai.is_empty());

    // Try to get profile CDI env (may fail without GPU)
    let result = manager.get_gaming_profile_cdi_env("cyberpunk 2077");
    // Result depends on GPU availability
    println!("Gaming profile CDI env result: {}", result.is_ok());

    let result = manager.get_ai_profile_cdi_env("ollama-medium", false);
    println!("AI profile CDI env result: {}", result.is_ok());
}
