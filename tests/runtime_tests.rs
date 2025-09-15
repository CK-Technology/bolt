use bolt::runtime::{BoltRuntime, oci::*, storage::*, gpu::*};
use bolt::config::{BoltConfig, GamingConfig, GpuConfig, NvidiaConfig, AmdConfig};
use bolt::error::{BoltError, Result};
use std::path::PathBuf;
use tempfile::TempDir;
use tokio;

#[tokio::test]
async fn test_runtime_initialization() {
    let runtime = BoltRuntime::new();
    assert!(runtime.is_ok(), "Runtime should initialize successfully");

    let runtime = runtime.unwrap();
    assert!(runtime.config().is_some(), "Runtime should have config");
}

#[tokio::test]
async fn test_runtime_with_custom_config() {
    let temp_dir = TempDir::new().unwrap();
    let config = BoltConfig {
        config_dir: temp_dir.path().join("config"),
        data_dir: temp_dir.path().join("data"),
        boltfile_path: temp_dir.path().join("Boltfile.toml"),
        verbose: true,
    };

    let runtime = BoltRuntime::with_config(config);
    assert!(runtime.config().is_some());
    assert_eq!(runtime.config().unwrap().verbose, true);
}

#[tokio::test]
async fn test_container_lifecycle() {
    let runtime = BoltRuntime::new().unwrap();

    // Test running a container
    let result = runtime.run_container(
        "alpine:latest",
        Some("test-container"),
        &[],
        &["TEST_VAR=value"],
        &[],
        true
    ).await;

    assert!(result.is_ok(), "Should run container successfully");

    // Test listing containers
    let containers = runtime.list_containers(false).await;
    assert!(containers.is_ok());
    let containers = containers.unwrap();
    assert!(containers.iter().any(|c| c.name == "test-container"));

    // Test stopping container
    let stop_result = runtime.stop_container("test-container").await;
    assert!(stop_result.is_ok());

    // Test removing container
    let remove_result = runtime.remove_container("test-container", false).await;
    assert!(remove_result.is_ok());
}

#[tokio::test]
async fn test_container_with_ports() {
    let runtime = BoltRuntime::new().unwrap();

    let result = runtime.run_container(
        "nginx:alpine",
        Some("test-nginx"),
        &["8080:80".to_string()],
        &[],
        &[],
        true
    ).await;

    assert!(result.is_ok());

    // Verify port mapping
    let containers = runtime.list_containers(false).await.unwrap();
    let nginx = containers.iter().find(|c| c.name == "test-nginx");
    assert!(nginx.is_some());
    assert!(nginx.unwrap().ports.contains(&"8080:80".to_string()));

    // Cleanup
    runtime.stop_container("test-nginx").await.ok();
    runtime.remove_container("test-nginx", true).await.ok();
}

#[tokio::test]
async fn test_container_with_volumes() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let host_path = temp_dir.path().join("data");
    std::fs::create_dir_all(&host_path).unwrap();

    let volume_spec = format!("{}:/data", host_path.display());

    let result = runtime.run_container(
        "alpine:latest",
        Some("test-volume"),
        &[],
        &[],
        &[volume_spec],
        true
    ).await;

    assert!(result.is_ok());

    // Cleanup
    runtime.stop_container("test-volume").await.ok();
    runtime.remove_container("test-volume", true).await.ok();
}

#[tokio::test]
async fn test_image_operations() {
    let runtime = BoltRuntime::new().unwrap();

    // Test pulling an image
    let pull_result = runtime.pull_image("alpine:latest").await;
    assert!(pull_result.is_ok(), "Should pull image successfully");

    // Test building an image
    let temp_dir = TempDir::new().unwrap();
    let dockerfile_path = temp_dir.path().join("Dockerfile");
    std::fs::write(&dockerfile_path, "FROM alpine:latest\nRUN echo 'test'").unwrap();

    let build_result = runtime.build_image(
        temp_dir.path().to_str().unwrap(),
        Some("test-image:latest"),
        "Dockerfile"
    ).await;

    assert!(build_result.is_ok(), "Should build image successfully");
}

#[tokio::test]
async fn test_network_operations() {
    let runtime = BoltRuntime::new().unwrap();

    // Create a network
    let create_result = runtime.create_network(
        "test-network",
        "bridge",
        Some("172.30.0.0/16")
    ).await;
    assert!(create_result.is_ok());

    // List networks
    let networks = runtime.list_networks().await;
    assert!(networks.is_ok());
    let networks = networks.unwrap();
    assert!(networks.iter().any(|n| n.name == "test-network"));

    // Remove network
    let remove_result = runtime.remove_network("test-network").await;
    assert!(remove_result.is_ok());
}

#[tokio::test]
async fn test_gaming_config() {
    let nvidia_config = NvidiaConfig {
        device: Some(0),
        dlss: Some(true),
        raytracing: Some(true),
        cuda: Some(false),
    };

    let gpu_config = GpuConfig {
        nvidia: Some(nvidia_config),
        amd: None,
        passthrough: Some(true),
    };

    let gaming_config = GamingConfig {
        gpu: Some(gpu_config),
        audio: None,
        wine: None,
        performance: None,
    };

    assert!(gaming_config.gpu.is_some());
    assert!(gaming_config.gpu.as_ref().unwrap().nvidia.is_some());
    assert_eq!(gaming_config.gpu.as_ref().unwrap().passthrough, Some(true));
}

#[tokio::test]
async fn test_error_handling() {
    let runtime = BoltRuntime::new().unwrap();

    // Test invalid image
    let result = runtime.run_container(
        "invalid:image:tag:format",
        None,
        &[],
        &[],
        &[],
        false
    ).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        BoltError::Runtime(_) => (),
        _ => panic!("Expected Runtime error"),
    }

    // Test stopping non-existent container
    let stop_result = runtime.stop_container("non-existent").await;
    assert!(stop_result.is_err());
}

#[tokio::test]
async fn test_container_isolation() {
    let runtime = BoltRuntime::new().unwrap();

    // Create two isolated containers
    let result1 = runtime.run_container(
        "alpine:latest",
        Some("isolated-1"),
        &[],
        &["INSTANCE=1".to_string()],
        &[],
        true
    ).await;
    assert!(result1.is_ok());

    let result2 = runtime.run_container(
        "alpine:latest",
        Some("isolated-2"),
        &[],
        &["INSTANCE=2".to_string()],
        &[],
        true
    ).await;
    assert!(result2.is_ok());

    // Verify both containers are running
    let containers = runtime.list_containers(false).await.unwrap();
    assert!(containers.iter().any(|c| c.name == "isolated-1"));
    assert!(containers.iter().any(|c| c.name == "isolated-2"));

    // Cleanup
    runtime.stop_container("isolated-1").await.ok();
    runtime.stop_container("isolated-2").await.ok();
    runtime.remove_container("isolated-1", true).await.ok();
    runtime.remove_container("isolated-2", true).await.ok();
}

#[cfg(feature = "gaming")]
#[tokio::test]
async fn test_gaming_setup() {
    let runtime = BoltRuntime::new().unwrap();

    // Test gaming setup with Proton
    let setup_result = runtime.setup_gaming(
        Some("8.0"),
        Some("win10")
    ).await;

    if setup_result.is_ok() {
        // Gaming setup succeeded (GPU available)
        assert!(true);
    } else {
        // Gaming setup failed (likely no GPU)
        match setup_result.unwrap_err() {
            BoltError::Gaming(_) => assert!(true),
            _ => panic!("Expected Gaming error when GPU not available"),
        }
    }
}

#[cfg(feature = "quic-networking")]
#[tokio::test]
async fn test_quic_network() {
    let runtime = BoltRuntime::new().unwrap();

    // Create QUIC-enabled network
    let result = runtime.create_network(
        "quic-net",
        "bolt",
        Some("10.99.0.0/16")
    ).await;

    assert!(result.is_ok(), "Should create QUIC network");

    // Cleanup
    runtime.remove_network("quic-net").await.ok();
}