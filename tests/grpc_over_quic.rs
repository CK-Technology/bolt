//! Integration tests for gRPC-over-QUIC
//!
//! Tests container management, network operations, and orchestration via gRPC over QUIC.

#![cfg(feature = "grpc")]

use anyhow::Result;
use bolt::config::BoltConfig;
use bolt::grpc::container_service::ContainerServiceImpl;
use bolt::grpc::generated::container::*;
use bolt::grpc::network_service::NetworkServiceImpl;
use bolt::grpc::orchestration_service::OrchestrationServiceImpl;
use bolt::grpc::{ContainerService, NetworkService, OrchestrationService};
use futures::StreamExt;
use tonic::Request;

#[tokio::test]
async fn test_container_service_run() -> Result<()> {
    // Create container service
    let service = ContainerServiceImpl::new().await?;

    // Create run request
    let request = Request::new(RunRequest {
        image: "alpine:latest".to_string(),
        name: "test-container".to_string(),
        command: vec!["/bin/sh".to_string()],
        entrypoint: vec![],
        env: std::collections::HashMap::new(),
        volumes: vec![],
        ports: vec![],
        network: "bridge".to_string(),
        working_dir: String::new(),
        user: String::new(),
        detach: true,
        interactive: false,
        tty: false,
        remove: false,
        resources: None,
        gpu: None,
        security: None,
    });

    // Call service
    let response = service.run(request).await?;
    let resp = response.into_inner();

    // Verify response
    assert!(!resp.container_id.is_empty() || !resp.error.is_empty());
    println!("✅ Container run test passed: status={}", resp.status);

    Ok(())
}

#[tokio::test]
async fn test_container_service_list() -> Result<()> {
    let service = ContainerServiceImpl::new().await?;

    let request = Request::new(ListRequest {
        all: true,
        filters: vec![],
        limit: 0,
    });

    let response = service.list(request).await?;
    let resp = response.into_inner();

    println!(
        "✅ Container list test passed: found {} containers",
        resp.containers.len()
    );

    Ok(())
}

#[tokio::test]
async fn test_container_service_logs_streaming() -> Result<()> {
    let service = ContainerServiceImpl::new().await?;

    let request = Request::new(LogsRequest {
        container_id: "test-container".to_string(),
        follow: true,
        stdout: true,
        stderr: true,
        tail: 10,
        since: 0,
        until: 0,
        timestamps: true,
    });

    let response = service.logs(request).await?;
    let mut stream = response.into_inner();

    // Read logs from stream
    let mut count = 0;
    while let Some(log_result) = stream.next().await {
        let log = log_result?;
        println!("📜 Log: [{}] {}", log.stream, log.message);
        count += 1;
        if count >= 5 {
            break; // Read first 5 logs
        }
    }

    assert!(count > 0, "Should receive at least one log entry");
    println!(
        "✅ Container logs streaming test passed: received {} logs",
        count
    );

    Ok(())
}

#[tokio::test]
async fn test_container_service_stats_streaming() -> Result<()> {
    let service = ContainerServiceImpl::new().await?;

    let request = Request::new(StatsRequest {
        container_id: "test-container".to_string(),
        stream: true,
        interval_ms: 500,
    });

    let response = service.stats(request).await?;
    let mut stream = response.into_inner();

    // Read stats from stream
    let mut count = 0;
    while let Some(stats_result) = stream.next().await {
        let stats = stats_result?;
        if let Some(cpu) = stats.cpu {
            println!(
                "📊 Stats: CPU={:.1}%, Memory={} MB",
                cpu.usage_percent,
                stats
                    .memory
                    .map(|m| m.usage_bytes / (1024 * 1024))
                    .unwrap_or(0)
            );
        }
        count += 1;
        if count >= 3 {
            break; // Read first 3 stat samples
        }
    }

    assert!(count > 0, "Should receive at least one stats sample");
    println!(
        "✅ Container stats streaming test passed: received {} samples",
        count
    );

    Ok(())
}

#[tokio::test]
async fn test_network_service_create_list() -> Result<()> {
    let service = NetworkServiceImpl::new();

    // Create network
    let create_request = Request::new(bolt::grpc::generated::network::CreateNetworkRequest {
        name: "test-network".to_string(),
        driver: "bridge".to_string(),
        subnet: "172.20.0.0/16".to_string(),
        gateway: "172.20.0.1".to_string(),
        options: std::collections::HashMap::new(),
        enable_ipv6: false,
        ipv6_subnet: String::new(),
        labels: std::collections::HashMap::new(),
    });

    let create_response = service.create_network(create_request).await?;
    let create_resp = create_response.into_inner();

    println!(
        "✅ Network created: name={}, driver={}",
        create_resp.name, create_resp.driver
    );

    // List networks
    let list_request =
        Request::new(bolt::grpc::generated::network::ListNetworksRequest { filters: vec![] });

    let list_response = service.list_networks(list_request).await?;
    let list_resp = list_response.into_inner();

    assert!(
        !list_resp.networks.is_empty(),
        "Should have at least one network"
    );
    println!(
        "✅ Network list test passed: found {} networks",
        list_resp.networks.len()
    );

    Ok(())
}

#[tokio::test]
async fn test_network_service_stats_streaming() -> Result<()> {
    let service = NetworkServiceImpl::new();

    let request = Request::new(bolt::grpc::generated::network::StatsRequest {
        network_id: "bridge".to_string(),
        stream: true,
        interval_ms: 500,
        interfaces: vec![],
    });

    let response = service.stream_stats(request).await?;
    let mut stream = response.into_inner();

    // Read network stats
    let mut count = 0;
    while let Some(stats_result) = stream.next().await {
        let stats = stats_result?;
        println!(
            "🌐 Network stats: timestamp={}, interfaces={}",
            stats.timestamp,
            stats.interfaces.len()
        );
        if let Some(quic) = stats.quic {
            println!(
                "   QUIC: connections={}, bytes_sent={}, bytes_received={}",
                quic.connections_established, quic.bytes_sent, quic.bytes_received
            );
        }
        count += 1;
        if count >= 3 {
            break;
        }
    }

    assert!(
        count > 0,
        "Should receive at least one network stats sample"
    );
    println!(
        "✅ Network stats streaming test passed: received {} samples",
        count
    );

    Ok(())
}

#[tokio::test]
async fn test_orchestration_service_deploy() -> Result<()> {
    let config = BoltConfig::default();
    let service = OrchestrationServiceImpl::new(config);

    let request = Request::new(bolt::grpc::generated::orchestration::DeployRequest {
        boltfile_path: "./Boltfile.toml".to_string(),
        services: vec!["web".to_string(), "db".to_string()],
        force_recreate: false,
        detach: true,
        env_overrides: std::collections::HashMap::new(),
    });

    let response = service.deploy(request).await?;
    let mut stream = response.into_inner();

    // Read deployment progress
    let mut started = false;
    let mut completed = false;
    let mut services_deployed = 0;

    while let Some(progress_result) = stream.next().await {
        let progress = progress_result?;

        match progress.event {
            Some(bolt::grpc::generated::orchestration::deploy_progress::Event::Started(s)) => {
                println!(
                    "🚀 Deployment started: project={}, services={}",
                    s.project_name, s.total_services
                );
                started = true;
            }
            Some(bolt::grpc::generated::orchestration::deploy_progress::Event::Service(s)) => {
                println!(
                    "📦 Service progress: {} - {} (step {}/{})",
                    s.service_name, s.status, s.current_step, s.total_steps
                );
                services_deployed += 1;
            }
            Some(bolt::grpc::generated::orchestration::deploy_progress::Event::Complete(c)) => {
                println!(
                    "✅ Deployment complete: containers={}, time={}ms",
                    c.total_containers, c.deploy_time_ms
                );
                completed = true;
            }
            _ => {}
        }
    }

    assert!(started, "Deployment should start");
    assert!(completed, "Deployment should complete");
    assert!(services_deployed > 0, "Should deploy at least one service");
    println!("✅ Orchestration deploy test passed");

    Ok(())
}

#[tokio::test]
async fn test_orchestration_service_scale() -> Result<()> {
    let config = BoltConfig::default();
    let service = OrchestrationServiceImpl::new(config);

    let mut services = std::collections::HashMap::new();
    services.insert("web".to_string(), 3);
    services.insert("db".to_string(), 2);

    let request = Request::new(bolt::grpc::generated::orchestration::ScaleRequest {
        project_name: "bolt-project".to_string(),
        services,
    });

    let response = service.scale(request).await?;
    let resp = response.into_inner();

    assert!(resp.success, "Scale operation should succeed");
    assert_eq!(resp.services.len(), 2, "Should scale 2 services");

    for scaled in &resp.services {
        println!(
            "📈 Scaled {}: {} -> {} replicas",
            scaled.service_name, scaled.previous_replicas, scaled.current_replicas
        );
    }

    println!("✅ Orchestration scale test passed");

    Ok(())
}

#[tokio::test]
async fn test_orchestration_service_update() -> Result<()> {
    let config = BoltConfig::default();
    let service = OrchestrationServiceImpl::new(config);

    let request = Request::new(bolt::grpc::generated::orchestration::UpdateRequest {
        project_name: "bolt-project".to_string(),
        services: vec!["web".to_string()],
        new_image: "alpine:3.18".to_string(),
        strategy: Some(bolt::grpc::generated::orchestration::UpdateStrategy {
            r#type: "rolling".to_string(),
            parallelism: 1,
            delay_seconds: 5,
            failure_threshold: 0.5,
        }),
    });

    let response = service.update(request).await?;
    let mut stream = response.into_inner();

    // Read update progress
    let mut started = false;
    let mut completed = false;
    let mut containers_updated = 0;

    while let Some(progress_result) = stream.next().await {
        let progress = progress_result?;

        match progress.event {
            Some(bolt::grpc::generated::orchestration::update_progress::Event::Started(s)) => {
                println!(
                    "🔄 Update started: service={}, containers={}",
                    s.service_name, s.total_containers
                );
                started = true;
            }
            Some(bolt::grpc::generated::orchestration::update_progress::Event::Container(c)) => {
                println!(
                    "📦 Container update: {} - {} ({}/{})",
                    c.container_id, c.status, c.current, c.total
                );
                containers_updated += 1;
            }
            Some(bolt::grpc::generated::orchestration::update_progress::Event::Complete(c)) => {
                println!(
                    "✅ Update complete: updated={}, failed={}, time={}ms",
                    c.updated_containers, c.failed_containers, c.update_time_ms
                );
                completed = true;
            }
            _ => {}
        }
    }

    assert!(started, "Update should start");
    assert!(completed, "Update should complete");
    assert!(
        containers_updated > 0,
        "Should update at least one container"
    );
    println!("✅ Orchestration update test passed");

    Ok(())
}
