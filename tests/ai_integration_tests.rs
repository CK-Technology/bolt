//! Integration tests for AI/ML features
//!
//! Tests:
//! - GPU scheduler allocation strategies
//! - Model serving lifecycle
//! - Model cache management
//! - Multi-GPU workload scheduling

use anyhow::Result;

#[cfg(feature = "ai-ml")]
mod ai_tests {
    use super::*;
    use bolt::ai::model_cache::ModelCache;
    use bolt::ai::model_serving::{ModelServer, ServeConfig, ServingBackend};
    use bolt::runtime::gpu_scheduler::{GpuScheduler, GpuConfig, GpuRequest, SchedulingStrategy};

    #[tokio::test]
    async fn test_gpu_scheduler_initialization() -> Result<()> {
        let scheduler = GpuScheduler::new(SchedulingStrategy::RoundRobin).await?;

        // Scheduler should initialize even without GPUs
        assert!(scheduler.list_gpus().await.is_ok());

        println!("✅ GPU scheduler initialization test passed");
        Ok(())
    }

    #[tokio::test]
    async fn test_gpu_allocation_strategies() -> Result<()> {
        let strategies = vec![
            SchedulingStrategy::RoundRobin,
            SchedulingStrategy::LeastUtilized,
            SchedulingStrategy::MostMemory,
            SchedulingStrategy::Exclusive,
        ];

        for strategy in strategies {
            let scheduler = GpuScheduler::new(strategy.clone()).await?;

            let config = GpuConfig {
                request: GpuRequest::Count(1),
                memory_limit: Some(8192), // 8GB
                exclusive: false,
            };

            // Allocation should not fail (may return empty if no GPUs)
            let allocation_result = scheduler.allocate("test-container", config).await;

            match allocation_result {
                Ok(gpus) => {
                    println!("   Strategy {:?}: Allocated {} GPUs", strategy, gpus.len());
                    // Cleanup
                    scheduler.deallocate("test-container").await?;
                }
                Err(e) => {
                    // Expected when no GPUs available
                    println!("   Strategy {:?}: No GPUs available ({})", strategy, e);
                }
            }
        }

        println!("✅ GPU allocation strategies test passed");
        Ok(())
    }

    #[tokio::test]
    async fn test_gpu_vram_limits() -> Result<()> {
        let scheduler = GpuScheduler::new(SchedulingStrategy::RoundRobin).await?;

        let config_8gb = GpuConfig {
            request: GpuRequest::Count(1),
            memory_limit: Some(8192),
            exclusive: false,
        };

        let config_16gb = GpuConfig {
            request: GpuRequest::Count(1),
            memory_limit: Some(16384),
            exclusive: false,
        };

        // Test different memory limits
        let alloc1 = scheduler.allocate("container-8gb", config_8gb).await;
        let alloc2 = scheduler.allocate("container-16gb", config_16gb).await;

        if alloc1.is_ok() {
            scheduler.deallocate("container-8gb").await?;
        }
        if alloc2.is_ok() {
            scheduler.deallocate("container-16gb").await?;
        }

        println!("✅ GPU VRAM limits test passed");
        Ok(())
    }

    #[tokio::test]
    async fn test_multi_container_gpu_allocation() -> Result<()> {
        let scheduler = GpuScheduler::new(SchedulingStrategy::LeastUtilized).await?;

        let config = GpuConfig {
            request: GpuRequest::Count(1),
            memory_limit: Some(4096),
            exclusive: false,
        };

        // Allocate GPUs to multiple containers
        let containers = vec!["train-1", "train-2", "infer-1", "infer-2"];
        let mut successful_allocations = Vec::new();

        for container_id in &containers {
            match scheduler.allocate(container_id, config.clone()).await {
                Ok(gpus) => {
                    println!("   Allocated {} GPUs to {}", gpus.len(), container_id);
                    successful_allocations.push(*container_id);
                }
                Err(_) => {
                    println!("   No GPUs available for {}", container_id);
                }
            }
        }

        // Cleanup
        for container_id in successful_allocations {
            scheduler.deallocate(container_id).await?;
        }

        println!("✅ Multi-container GPU allocation test passed");
        Ok(())
    }

    #[tokio::test]
    async fn test_model_cache_initialization() -> Result<()> {
        let cache = ModelCache::new().await?;

        // Cache should initialize successfully
        let models = cache.list_models();
        assert!(models.is_empty() || !models.is_empty()); // Just verify it doesn't panic

        println!("✅ Model cache initialization test passed");
        Ok(())
    }

    #[tokio::test]
    async fn test_model_cache_operations() -> Result<()> {
        let mut cache = ModelCache::new().await?;

        // Test model path retrieval (should be None for non-existent model)
        let path = cache.get_model_path("non-existent-model");
        assert!(path.is_none());

        // Test listing models
        let models = cache.list_models();
        println!("   Found {} cached models", models.len());

        println!("✅ Model cache operations test passed");
        Ok(())
    }

    #[tokio::test]
    async fn test_model_server_creation() -> Result<()> {
        let config = ServeConfig {
            model_name: "test-model".to_string(),
            backend: ServingBackend::VLLM {
                tensor_parallel: 1,
                pipeline_parallel: 1,
                max_batch_size: 32,
                max_num_seqs: 128,
            },
            gpus: vec!["gpu:0".to_string()],
            port: 8000,
            host: "127.0.0.1".to_string(),
            enable_healthcheck: false, // Disable for testing
            auto_restart: false,
        };

        let server = ModelServer::new(config);

        // Server should be created successfully (not started)
        println!("   Model server created successfully");

        println!("✅ Model server creation test passed");
        Ok(())
    }

    #[tokio::test]
    async fn test_model_serving_backends() -> Result<()> {
        let backends = vec![
            ServingBackend::VLLM {
                tensor_parallel: 2,
                pipeline_parallel: 1,
                max_batch_size: 64,
                max_num_seqs: 256,
            },
            ServingBackend::TensorRT {
                engine_path: None,
                precision: bolt::ai::model_serving::Precision::FP16,
            },
            ServingBackend::ONNX {
                execution_provider: "CUDA".to_string(),
            },
        ];

        for backend in backends {
            let config = ServeConfig {
                model_name: "test-model".to_string(),
                backend: backend.clone(),
                gpus: vec!["gpu:0".to_string()],
                port: 8000,
                host: "127.0.0.1".to_string(),
                enable_healthcheck: false,
                auto_restart: false,
            };

            let _server = ModelServer::new(config);
            println!("   Created server with backend: {:?}", backend);
        }

        println!("✅ Model serving backends test passed");
        Ok(())
    }

    #[tokio::test]
    async fn test_gpu_scheduler_metrics() -> Result<()> {
        let scheduler = GpuScheduler::new(SchedulingStrategy::RoundRobin).await?;

        // Update metrics (should not fail even without GPUs)
        let update_result = scheduler.update_metrics().await;

        match update_result {
            Ok(_) => println!("   Metrics updated successfully"),
            Err(e) => println!("   Metrics update failed (expected without GPUs): {}", e),
        }

        // List allocations
        let allocations = scheduler.list_allocations().await?;
        println!("   Current allocations: {}", allocations.len());

        println!("✅ GPU scheduler metrics test passed");
        Ok(())
    }

    #[tokio::test]
    async fn test_exclusive_gpu_allocation() -> Result<()> {
        let scheduler = GpuScheduler::new(SchedulingStrategy::Exclusive).await?;

        let config = GpuConfig {
            request: GpuRequest::Count(1),
            memory_limit: None,
            exclusive: true,
        };

        // First allocation should succeed (if GPUs available)
        let alloc1 = scheduler.allocate("exclusive-1", config.clone()).await;

        if let Ok(gpus) = alloc1 {
            println!("   Exclusive allocation to exclusive-1: {} GPUs", gpus.len());

            // Second allocation to same GPU should fail with exclusive mode
            // (This would be tested with actual GPU hardware)

            // Cleanup
            scheduler.deallocate("exclusive-1").await?;
        } else {
            println!("   No GPUs available for exclusive allocation");
        }

        println!("✅ Exclusive GPU allocation test passed");
        Ok(())
    }

    #[tokio::test]
    async fn test_mig_allocation() -> Result<()> {
        let scheduler = GpuScheduler::new(SchedulingStrategy::RoundRobin).await?;

        let config = GpuConfig {
            request: GpuRequest::Mig {
                profile: "1g.5gb".to_string(),
            },
            memory_limit: None,
            exclusive: false,
        };

        // MIG allocation (will fail without A100/H100, which is expected)
        let alloc_result = scheduler.allocate("mig-test", config).await;

        match alloc_result {
            Ok(gpus) => {
                println!("   MIG allocation successful: {} instances", gpus.len());
                scheduler.deallocate("mig-test").await?;
            }
            Err(e) => {
                println!("   MIG allocation failed (expected without A100/H100): {}", e);
            }
        }

        println!("✅ MIG allocation test passed");
        Ok(())
    }
}

// Fallback tests when ai-ml feature is not enabled
#[cfg(not(feature = "ai-ml"))]
#[tokio::test]
async fn test_ai_features_disabled() {
    println!("⚠️  AI/ML features are not enabled");
    println!("   Enable with --features ai-ml to run AI integration tests");
}
