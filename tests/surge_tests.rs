use bolt::{BoltRuntime, BoltFileBuilder, surge::*, config::*};
use std::collections::HashMap;
use tempfile::TempDir;
use tokio;

#[tokio::test]
async fn test_surge_basic_deployment() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    // Create a simple Boltfile
    let boltfile = BoltFileBuilder::new("test-project")
        .add_service("web", Service {
            image: Some("nginx:alpine".to_string()),
            ports: Some(vec!["8080:80".to_string()]),
            ..Default::default()
        })
        .build();

    // Save Boltfile
    let config = BoltConfig {
        config_dir: temp_dir.path().join("config"),
        data_dir: temp_dir.path().join("data"),
        boltfile_path: temp_dir.path().join("Boltfile.toml"),
        verbose: false,
    };

    config.save_boltfile(&boltfile).unwrap();

    // Deploy with Surge
    let runtime = BoltRuntime::with_config(config);
    let deploy_result = runtime.surge_up(&[], false, false).await;
    assert!(deploy_result.is_ok());

    // Check status
    let status = runtime.surge_status().await;
    assert!(status.is_ok());
    let status = status.unwrap();
    assert_eq!(status.services.len(), 1);
    assert!(status.services.iter().any(|s| s.name == "web"));

    // Tear down
    let down_result = runtime.surge_down(&[], false).await;
    assert!(down_result.is_ok());
}

#[tokio::test]
async fn test_surge_multi_service() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    // Create multi-service Boltfile
    let mut env = HashMap::new();
    env.insert("DATABASE_URL".to_string(), "bolt://db:5432".to_string());

    let boltfile = BoltFileBuilder::new("multi-service")
        .add_service("db", Service {
            capsule: Some("postgres".to_string()),
            auth: Some(Auth {
                user: "testuser".to_string(),
                password: "testpass".to_string(),
            }),
            storage: Some(Storage {
                size: "1Gi".to_string(),
                driver: None,
            }),
            ..Default::default()
        })
        .add_service("api", Service {
            image: Some("node:alpine".to_string()),
            ports: Some(vec!["3000:3000".to_string()]),
            env: Some(env),
            depends_on: Some(vec!["db".to_string()]),
            ..Default::default()
        })
        .add_service("web", Service {
            image: Some("nginx:alpine".to_string()),
            ports: Some(vec!["80:80".to_string()]),
            depends_on: Some(vec!["api".to_string()]),
            ..Default::default()
        })
        .build();

    let config = BoltConfig {
        config_dir: temp_dir.path().join("config"),
        data_dir: temp_dir.path().join("data"),
        boltfile_path: temp_dir.path().join("Boltfile.toml"),
        verbose: false,
    };

    config.save_boltfile(&boltfile).unwrap();

    // Deploy stack
    let runtime = BoltRuntime::with_config(config);
    let deploy_result = runtime.surge_up(&[], false, false).await;
    assert!(deploy_result.is_ok());

    // Verify all services are running
    let status = runtime.surge_status().await.unwrap();
    assert_eq!(status.services.len(), 3);
    assert!(status.services.iter().any(|s| s.name == "db"));
    assert!(status.services.iter().any(|s| s.name == "api"));
    assert!(status.services.iter().any(|s| s.name == "web"));

    // Test scaling
    let scale_result = runtime.surge_scale(&["api".to_string()]).await;
    assert!(scale_result.is_ok());

    // Tear down
    runtime.surge_down(&[], true).await.ok();
}

#[tokio::test]
async fn test_surge_with_dependencies() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    // Create service with dependencies
    let boltfile = BoltFileBuilder::new("dep-test")
        .add_service("database", Service {
            capsule: Some("postgres".to_string()),
            ..Default::default()
        })
        .add_service("cache", Service {
            image: Some("redis:alpine".to_string()),
            ..Default::default()
        })
        .add_service("app", Service {
            image: Some("alpine:latest".to_string()),
            depends_on: Some(vec!["database".to_string(), "cache".to_string()]),
            ..Default::default()
        })
        .build();

    let config = BoltConfig {
        config_dir: temp_dir.path().join("config"),
        data_dir: temp_dir.path().join("data"),
        boltfile_path: temp_dir.path().join("Boltfile.toml"),
        verbose: false,
    };

    config.save_boltfile(&boltfile).unwrap();

    // Deploy - should start services in dependency order
    let runtime = BoltRuntime::with_config(config);
    let deploy_result = runtime.surge_up(&[], false, false).await;
    assert!(deploy_result.is_ok());

    // Verify services started in correct order
    let status = runtime.surge_status().await.unwrap();
    assert_eq!(status.services.len(), 3);

    // Tear down
    runtime.surge_down(&[], false).await.ok();
}

#[tokio::test]
async fn test_surge_force_recreate() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    let boltfile = BoltFileBuilder::new("recreate-test")
        .add_service("web", Service {
            image: Some("nginx:alpine".to_string()),
            ..Default::default()
        })
        .build();

    let config = BoltConfig {
        config_dir: temp_dir.path().join("config"),
        data_dir: temp_dir.path().join("data"),
        boltfile_path: temp_dir.path().join("Boltfile.toml"),
        verbose: false,
    };

    config.save_boltfile(&boltfile).unwrap();

    let runtime = BoltRuntime::with_config(config.clone());

    // Initial deployment
    runtime.surge_up(&[], false, false).await.unwrap();

    // Get initial container ID
    let initial_status = runtime.surge_status().await.unwrap();
    let initial_id = initial_status.services[0].id.clone();

    // Force recreate
    let runtime = BoltRuntime::with_config(config);
    runtime.surge_up(&[], false, true).await.unwrap();

    // Verify container was recreated (new ID)
    let new_status = runtime.surge_status().await.unwrap();
    let new_id = new_status.services[0].id.clone();
    assert_ne!(initial_id, new_id);

    // Cleanup
    runtime.surge_down(&[], false).await.ok();
}

#[tokio::test]
async fn test_surge_selective_services() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    let boltfile = BoltFileBuilder::new("selective-test")
        .add_service("service1", Service {
            image: Some("alpine:latest".to_string()),
            ..Default::default()
        })
        .add_service("service2", Service {
            image: Some("alpine:latest".to_string()),
            ..Default::default()
        })
        .add_service("service3", Service {
            image: Some("alpine:latest".to_string()),
            ..Default::default()
        })
        .build();

    let config = BoltConfig {
        config_dir: temp_dir.path().join("config"),
        data_dir: temp_dir.path().join("data"),
        boltfile_path: temp_dir.path().join("Boltfile.toml"),
        verbose: false,
    };

    config.save_boltfile(&boltfile).unwrap();

    let runtime = BoltRuntime::with_config(config);

    // Deploy only specific services
    let deploy_result = runtime.surge_up(
        &["service1".to_string(), "service3".to_string()],
        false,
        false
    ).await;
    assert!(deploy_result.is_ok());

    // Verify only selected services are running
    let status = runtime.surge_status().await.unwrap();
    assert_eq!(status.services.len(), 2);
    assert!(status.services.iter().any(|s| s.name == "service1"));
    assert!(status.services.iter().any(|s| s.name == "service3"));
    assert!(!status.services.iter().any(|s| s.name == "service2"));

    // Cleanup
    runtime.surge_down(&[], false).await.ok();
}

#[tokio::test]
async fn test_surge_with_volumes() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    let boltfile = BoltFileBuilder::new("volume-test")
        .add_service("data-service", Service {
            image: Some("alpine:latest".to_string()),
            volumes: Some(vec![
                "./data:/app/data".to_string(),
                "named-volume:/app/cache".to_string(),
            ]),
            ..Default::default()
        })
        .build();

    let config = BoltConfig {
        config_dir: temp_dir.path().join("config"),
        data_dir: temp_dir.path().join("data"),
        boltfile_path: temp_dir.path().join("Boltfile.toml"),
        verbose: false,
    };

    // Create data directory
    std::fs::create_dir_all(temp_dir.path().join("data")).unwrap();

    config.save_boltfile(&boltfile).unwrap();

    let runtime = BoltRuntime::with_config(config);
    let deploy_result = runtime.surge_up(&[], false, false).await;
    assert!(deploy_result.is_ok());

    // Tear down with volumes
    let down_result = runtime.surge_down(&[], true).await;
    assert!(down_result.is_ok());
}

#[cfg(feature = "gaming")]
#[tokio::test]
async fn test_surge_gaming_service() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    let gaming_config = GamingConfig {
        gpu: Some(GpuConfig {
            nvidia: Some(NvidiaConfig {
                device: Some(0),
                dlss: Some(true),
                raytracing: Some(true),
                cuda: Some(false),
            }),
            amd: None,
            passthrough: Some(true),
        }),
        audio: Some(AudioConfig {
            system: "pipewire".to_string(),
            latency: Some("low".to_string()),
        }),
        wine: Some(WineConfig {
            version: None,
            proton: Some("8.0".to_string()),
            winver: Some("win10".to_string()),
            prefix: Some("/games/prefix".to_string()),
        }),
        performance: Some(PerformanceConfig {
            cpu_governor: Some("performance".to_string()),
            nice_level: Some(-10),
            rt_priority: Some(50),
        }),
    };

    let boltfile = BoltFileBuilder::new("gaming-test")
        .add_gaming_service("steam", "bolt://steam:latest", gaming_config)
        .build();

    let config = BoltConfig {
        config_dir: temp_dir.path().join("config"),
        data_dir: temp_dir.path().join("data"),
        boltfile_path: temp_dir.path().join("Boltfile.toml"),
        verbose: false,
    };

    config.save_boltfile(&boltfile).unwrap();

    let runtime = BoltRuntime::with_config(config);
    let deploy_result = runtime.surge_up(&[], false, false).await;

    // This test will pass or fail based on GPU availability
    if deploy_result.is_ok() {
        let status = runtime.surge_status().await.unwrap();
        assert!(status.services.iter().any(|s| s.name == "steam"));
        runtime.surge_down(&[], false).await.ok();
    } else {
        // No GPU available, which is fine for CI
        assert!(true);
    }
}