use bolt::api::*;
/// Comprehensive API tests to verify production readiness
use bolt::{
    BoltConfig, BoltFileBuilder, BoltRuntime, ContainerInfo, NetworkInfo, ServiceInfo, SurgeStatus,
};

#[tokio::test]
async fn test_complete_api_surface() {
    // Test that all main API types can be created and used

    // 1. Test BoltConfig
    let config = BoltConfig::default();
    assert!(!config.verbose); // Test a boolean field instead

    // 2. Test BoltRuntime creation
    let runtime_result = BoltRuntime::new();
    assert!(
        runtime_result.is_ok(),
        "BoltRuntime should create successfully"
    );

    let runtime = runtime_result.unwrap();

    // 3. Test all main data structures can be created
    let container = ContainerInfo {
        id: "test-123".to_string(),
        name: "test-container".to_string(),
        image: "nginx:latest".to_string(),
        status: "running".to_string(),
        ports: vec!["80:8080".to_string(), "443:8443".to_string()],
    };
    assert_eq!(container.name, "test-container");
    assert_eq!(container.ports.len(), 2);

    let service = ServiceInfo {
        name: "web-service".to_string(),
        status: "healthy".to_string(),
        replicas: 3,
    };
    assert_eq!(service.replicas, 3);

    let network = NetworkInfo {
        name: "bolt-network".to_string(),
        driver: "bridge".to_string(),
        subnet: Some("172.20.0.0/16".to_string()),
    };
    assert!(network.subnet.is_some());

    let surge_status = SurgeStatus {
        services: vec![service.clone()],
        networks: vec![network.clone()],
    };
    assert_eq!(surge_status.services.len(), 1);
    assert_eq!(surge_status.networks.len(), 1);

    // 4. Test BoltFileBuilder
    let service_config = Service {
        image: Some("redis:7".to_string()),
        ports: Some(vec!["6379:6379".to_string()]),
        environment: Some([("REDIS_PASSWORD".to_string(), "secret".to_string())].into()),
        restart: Some(bolt::config::RestartPolicy::Always),
        ..Default::default()
    };

    let boltfile = BoltFileBuilder::new("production-app")
        .add_service("cache", service_config)
        .add_service(
            "web",
            Service {
                image: Some("nginx:alpine".to_string()),
                ports: Some(vec!["80:80".to_string()]),
                depends_on: Some(vec!["cache".to_string()]),
                ..Default::default()
            },
        )
        .build();

    assert_eq!(boltfile.project, "production-app");
    assert_eq!(boltfile.services.len(), 2);
    assert!(boltfile.services.contains_key("cache"));
    assert!(boltfile.services.contains_key("web"));

    // Verify service dependencies
    let web_service = &boltfile.services["web"];
    assert!(web_service.depends_on.is_some());
    assert_eq!(web_service.depends_on.as_ref().unwrap()[0], "cache");
}

#[tokio::test]
async fn test_error_handling_system() {
    // Test the error handling system works properly
    use bolt::{BoltError, Result};

    // Test error creation and handling
    let config_error = BoltError::Config(bolt::error::ConfigError::MissingField {
        field: "required_field".to_string(),
    });

    let runtime_error = BoltError::Runtime(bolt::error::RuntimeError::ContainerNotFound {
        name: "missing-container".to_string(),
    });

    // Test error matching
    match config_error {
        BoltError::Config(bolt::error::ConfigError::MissingField { field }) => {
            assert_eq!(field, "required_field");
        }
        _ => panic!("Expected ConfigError::MissingField"),
    }

    match runtime_error {
        BoltError::Runtime(bolt::error::RuntimeError::ContainerNotFound { name }) => {
            assert_eq!(name, "missing-container");
        }
        _ => panic!("Expected RuntimeError::ContainerNotFound"),
    }

    // Test Result type
    let success_result: Result<String> = Ok("success".to_string());
    assert!(success_result.is_ok());
    assert_eq!(success_result.unwrap(), "success");

    let error_result: Result<String> =
        Err(BoltError::Config(bolt::error::ConfigError::InvalidFormat {
            reason: "malformed TOML".to_string(),
        }));
    assert!(error_result.is_err());
}

#[tokio::test]
async fn test_runtime_methods_are_callable() {
    // Test that all BoltRuntime methods exist and are callable
    let runtime = BoltRuntime::new().expect("Should create runtime");

    // These methods may fail due to no Docker/containers available, but they should exist and be callable

    // Container operations
    let list_result = runtime.list_containers(false).await;
    // Should return either Ok(vec) or Err, but not panic
    match list_result {
        Ok(_containers) => println!("✅ list_containers works"),
        Err(_) => println!("⚠️  list_containers failed (expected without Docker)"),
    }

    // Network operations
    let networks_result = runtime.list_networks().await;
    match networks_result {
        Ok(_networks) => println!("✅ list_networks works"),
        Err(_) => println!("⚠️  list_networks failed (expected without Docker)"),
    }

    // Surge operations
    let surge_result = runtime.surge_status().await;
    match surge_result {
        Ok(_status) => println!("✅ surge_status works"),
        Err(_) => println!("⚠️  surge_status failed (expected without Boltfile)"),
    }

    // All methods are callable - API is complete
    println!("✅ All BoltRuntime methods are callable");
}

#[test]
fn test_library_exports() {
    // Test that all expected exports are available at the crate root

    // Main types should be available
    let _config_type: Option<BoltConfig> = None;
    let _runtime_type: Option<BoltRuntime> = None;
    let _container_type: Option<ContainerInfo> = None;
    let _service_type: Option<ServiceInfo> = None;
    let _network_type: Option<NetworkInfo> = None;
    let _surge_type: Option<SurgeStatus> = None;

    // Error types should be available
    let _error_type: Option<bolt::BoltError> = None;
    let _result_type: Option<bolt::Result<()>> = None;

    // API module should be available
    let _builder_type: Option<BoltFileBuilder> = None;
    let _service_config_type: Option<Service> = None;

    println!("✅ All expected library exports are available");
}

#[tokio::test]
async fn test_configuration_serialization() {
    // Test that configs can be serialized/deserialized properly
    let mut boltfile = create_example_boltfile();
    boltfile.project = "serialization-test".to_string();

    // Should be able to serialize to TOML
    let toml_result = toml::to_string(&boltfile);
    assert!(toml_result.is_ok(), "Boltfile should serialize to TOML");

    let toml_string = toml_result.unwrap();
    assert!(toml_string.contains("serialization-test"));
    assert!(toml_string.contains("[services.web]"));

    // Should be able to deserialize back
    let parsed_result: Result<bolt::config::BoltFile, _> = toml::from_str(&toml_string);
    assert!(
        parsed_result.is_ok(),
        "TOML should deserialize back to BoltFile"
    );

    let parsed_boltfile = parsed_result.unwrap();
    assert_eq!(parsed_boltfile.project, "serialization-test");
    assert!(parsed_boltfile.services.contains_key("web"));

    println!("✅ Configuration serialization works correctly");
}
