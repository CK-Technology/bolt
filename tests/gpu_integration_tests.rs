use bolt::{BoltError, Result};
use bolt::runtime::gpu_integration::{BoltGpuIntegration, GpuConfig, GpuDriver, PerformanceLevel};
use bolt::runtime::native::BoltNativeRuntime;
use bolt::runtime::unified::UnifiedRuntime;
use std::collections::HashMap;
use tokio;

#[tokio::test]
async fn test_gpu_integration_initialization() {
    let gpu_integration = BoltGpuIntegration::new().await;
    assert!(gpu_integration.is_ok(), "GPU integration should initialize successfully");
}

#[tokio::test]
async fn test_gpu_detection() {
    let gpu_integration = BoltGpuIntegration::new().await.unwrap();
    let detected_gpus = gpu_integration.detect_gpus().await;

    // Should not fail even if no GPUs are present
    assert!(detected_gpus.is_ok(), "GPU detection should not fail");
}

#[tokio::test]
async fn test_gpu_config_validation() {
    let valid_config = GpuConfig {
        devices: vec!["0".to_string()],
        driver: GpuDriver::Auto,
        performance_level: PerformanceLevel::Ultra,
        enable_ray_tracing: false,
        enable_dlss: false,
        memory_limit: None,
        compute_capability: None,
        wsl2_optimizations: false,
        environment_vars: HashMap::new(),
    };

    let gpu_integration = BoltGpuIntegration::new().await.unwrap();
    let validation_result = gpu_integration.validate_config(&valid_config).await;
    assert!(validation_result.is_ok(), "Valid GPU config should pass validation");
}

#[cfg(feature = "nvbind-support")]
#[tokio::test]
async fn test_nvbind_integration() {
    use nvbind::GpuManager;

    let gpu_manager_result = GpuManager::new().await;
    if gpu_manager_result.is_ok() {
        let gpu_integration = BoltGpuIntegration::new().await.unwrap();
        let container_id = "test_container_123";

        let gpu_config = GpuConfig {
            devices: vec!["all".to_string()],
            driver: GpuDriver::NvidiaOpen,
            performance_level: PerformanceLevel::Ultra,
            enable_ray_tracing: true,
            enable_dlss: true,
            memory_limit: Some(8192),
            compute_capability: None,
            wsl2_optimizations: false,
            environment_vars: HashMap::new(),
        };

        // Test setup (should not fail even if no GPU hardware)
        let setup_result = gpu_integration.setup_container_gpu(container_id, &gpu_config).await;
        assert!(setup_result.is_ok() || matches!(setup_result, Err(BoltError::Runtime(_))));

        // Test cleanup
        let cleanup_result = gpu_integration.cleanup_container_gpu(container_id).await;
        assert!(cleanup_result.is_ok());
    }
}

#[tokio::test]
async fn test_gaming_container_setup() {
    let gpu_integration = BoltGpuIntegration::new().await.unwrap();
    let container_id = "gaming_test_container";

    let result = gpu_integration.setup_gaming_container(container_id).await;
    // Should succeed or gracefully handle missing hardware
    assert!(result.is_ok() || matches!(result, Err(BoltError::Runtime(_))));
}

#[tokio::test]
async fn test_ai_ml_container_setup() {
    let gpu_integration = BoltGpuIntegration::new().await.unwrap();
    let container_id = "ai_ml_test_container";

    let result = gpu_integration.setup_ai_ml_container(container_id).await;
    // Should succeed or gracefully handle missing hardware
    assert!(result.is_ok() || matches!(result, Err(BoltError::Runtime(_))));
}

#[tokio::test]
async fn test_unified_runtime_gpu_access() {
    let unified_runtime = UnifiedRuntime::new().await;
    assert!(unified_runtime.is_ok(), "Unified runtime should initialize");

    let runtime = unified_runtime.unwrap();
    let native_runtime = runtime.get_native_runtime();

    // Test that we can access the native runtime for GPU operations
    let native = native_runtime.read().await;
    let gpu_support = native.supports_gpu().await;
    // Should not fail regardless of hardware availability
    assert!(gpu_support.is_ok());
}

#[tokio::test]
async fn test_performance_levels() {
    let gpu_integration = BoltGpuIntegration::new().await.unwrap();

    // Test different performance levels
    let performance_levels = vec![
        PerformanceLevel::Efficient,
        PerformanceLevel::Balanced,
        PerformanceLevel::High,
        PerformanceLevel::Ultra,
    ];

    for level in performance_levels {
        let config = GpuConfig {
            devices: vec!["0".to_string()],
            driver: GpuDriver::Auto,
            performance_level: level,
            enable_ray_tracing: false,
            enable_dlss: false,
            memory_limit: None,
            compute_capability: None,
            wsl2_optimizations: false,
            environment_vars: HashMap::new(),
        };

        let validation_result = gpu_integration.validate_config(&config).await;
        assert!(validation_result.is_ok(), "Performance level {:?} should be valid", level);
    }
}

#[tokio::test]
async fn test_gpu_driver_types() {
    let gpu_integration = BoltGpuIntegration::new().await.unwrap();

    let drivers = vec![
        GpuDriver::Auto,
        GpuDriver::NvidiaOpen,
        GpuDriver::NvidiaProprietary,
        GpuDriver::Nouveau,
        GpuDriver::AMDGPU,
        GpuDriver::Intel,
    ];

    for driver in drivers {
        let config = GpuConfig {
            devices: vec!["0".to_string()],
            driver: driver.clone(),
            performance_level: PerformanceLevel::Balanced,
            enable_ray_tracing: false,
            enable_dlss: false,
            memory_limit: None,
            compute_capability: None,
            wsl2_optimizations: false,
            environment_vars: HashMap::new(),
        };

        let validation_result = gpu_integration.validate_config(&config).await;
        assert!(validation_result.is_ok(), "Driver {:?} should be valid", driver);
    }
}

#[tokio::test]
async fn test_container_runtime_fallback() {
    // Test that GPU integration gracefully falls back when nvbind is unavailable
    let gpu_integration = BoltGpuIntegration::new().await.unwrap();
    let container_id = "fallback_test_container";

    let gpu_config = GpuConfig {
        devices: vec!["0".to_string()],
        driver: GpuDriver::Auto,
        performance_level: PerformanceLevel::Balanced,
        enable_ray_tracing: false,
        enable_dlss: false,
        memory_limit: None,
        compute_capability: None,
        wsl2_optimizations: false,
        environment_vars: HashMap::new(),
    };

    // Should handle gracefully whether nvbind is available or not
    let setup_result = gpu_integration.setup_container_gpu(container_id, &gpu_config).await;

    // Either succeeds with nvbind or falls back gracefully
    match setup_result {
        Ok(_) => {
            // Success - cleanup
            let cleanup_result = gpu_integration.cleanup_container_gpu(container_id).await;
            assert!(cleanup_result.is_ok());
        }
        Err(BoltError::Runtime(_)) => {
            // Expected when nvbind is not available or no GPU hardware
        }
        Err(e) => {
            panic!("Unexpected error during GPU setup: {:?}", e);
        }
    }
}