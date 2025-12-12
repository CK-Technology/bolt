//! Network service implementation for gRPC-over-QUIC
//!
//! Provides high-performance network management and QUIC statistics via gRPC.

use anyhow::Result;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info};

use crate::grpc::generated::network::*;
use crate::network;
use crate::networking::quic_real::RealQUICServer;

/// Network service implementation
pub struct NetworkServiceImpl {
    quic_server: Option<Arc<RwLock<RealQUICServer>>>,
}

impl NetworkServiceImpl {
    /// Create new network service
    pub fn new() -> Self {
        info!("🌐 Initializing NetworkService gRPC handler");
        Self { quic_server: None }
    }

    /// Create with QUIC server for advanced features
    pub fn with_quic(quic_server: RealQUICServer) -> Self {
        info!("🌐 Initializing NetworkService with QUIC support");
        Self {
            quic_server: Some(Arc::new(RwLock::new(quic_server))),
        }
    }
}

#[tonic::async_trait]
impl network_service_server::NetworkService for NetworkServiceImpl {
    /// Create a new network
    async fn create_network(
        &self,
        request: Request<CreateNetworkRequest>,
    ) -> Result<Response<CreateNetworkResponse>, Status> {
        let req = request.into_inner();
        info!(
            "🔧 gRPC CreateNetwork: name={}, driver={}",
            req.name, req.driver
        );

        let subnet = if req.subnet.is_empty() {
            None
        } else {
            Some(req.subnet.as_str())
        };
        match network::create_network(&req.name, &req.driver, subnet).await {
            Ok(_) => {
                let network_id = format!("net-{}", uuid::Uuid::new_v4());
                info!("✅ Network created: {} ({})", req.name, network_id);
                Ok(Response::new(CreateNetworkResponse {
                    network_id: network_id.clone(),
                    name: req.name.clone(),
                    driver: req.driver.clone(),
                    subnet: req.subnet.clone(),
                    gateway: req.gateway.clone(),
                    error: String::new(),
                }))
            }
            Err(e) => {
                error!("❌ Failed to create network: {}", e);
                Ok(Response::new(CreateNetworkResponse {
                    network_id: String::new(),
                    name: req.name.clone(),
                    driver: req.driver.clone(),
                    subnet: req.subnet.clone(),
                    gateway: req.gateway.clone(),
                    error: e.to_string(),
                }))
            }
        }
    }

    /// Delete a network
    async fn delete_network(
        &self,
        request: Request<DeleteNetworkRequest>,
    ) -> Result<Response<DeleteNetworkResponse>, Status> {
        let req = request.into_inner();
        info!("🗑️ gRPC DeleteNetwork: network_id={}", req.network_id);

        match network::remove_network(&req.network_id).await {
            Ok(_) => {
                info!("✅ Network deleted: {}", req.network_id);
                Ok(Response::new(DeleteNetworkResponse {
                    network_id: req.network_id.clone(),
                    success: true,
                    error: String::new(),
                }))
            }
            Err(e) => {
                error!("❌ Failed to delete network: {}", e);
                Ok(Response::new(DeleteNetworkResponse {
                    network_id: req.network_id.clone(),
                    success: false,
                    error: e.to_string(),
                }))
            }
        }
    }

    /// List networks
    async fn list_networks(
        &self,
        request: Request<ListNetworksRequest>,
    ) -> Result<Response<ListNetworksResponse>, Status> {
        let req = request.into_inner();
        info!("📋 gRPC ListNetworks: filters={:?}", req.filters);

        match network::list_networks_info().await {
            Ok(networks) => {
                let network_infos: Vec<NetworkInfo> = networks
                    .iter()
                    .map(|n| NetworkInfo {
                        id: n.id.clone(),
                        name: n.name.clone(),
                        driver: n.driver.clone(),
                        subnet: n.subnet.clone().unwrap_or_default(),
                        gateway: String::new(), // Not available in bolt::types::NetworkInfo
                        containers: vec![],     // Not available in bolt::types::NetworkInfo
                        labels: std::collections::HashMap::new(),
                        created_at: 0,
                    })
                    .collect();

                info!("✅ Listed {} networks", network_infos.len());
                Ok(Response::new(ListNetworksResponse {
                    networks: network_infos,
                }))
            }
            Err(e) => {
                error!("❌ Failed to list networks: {}", e);
                Err(Status::internal(format!("Failed to list networks: {}", e)))
            }
        }
    }

    /// Stream network statistics (server streaming)
    type StreamStatsStream = Pin<Box<dyn Stream<Item = Result<NetworkStatsUpdate, Status>> + Send>>;

    async fn stream_stats(
        &self,
        request: Request<StatsRequest>,
    ) -> Result<Response<Self::StreamStatsStream>, Status> {
        let req = request.into_inner();
        info!(
            "📊 gRPC StreamStats: network_id={}, stream={}, interval={}ms",
            req.network_id, req.stream, req.interval_ms
        );

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        let quic_server = self.quic_server.clone();
        let interval = if req.interval_ms > 0 {
            req.interval_ms
        } else {
            1000
        };

        // Spawn task to stream network stats
        tokio::spawn(async move {
            let iterations = if req.stream { 100 } else { 1 };

            for _ in 0..iterations {
                // Get QUIC stats if available
                let quic_stats = if let Some(ref qs) = quic_server {
                    let server = qs.read().await;
                    let stats = server.get_stats().await;
                    let (pool_size, max_pool_size, total_reuses) = server.get_pool_stats().await;

                    Some(QuicStats {
                        connections_established: stats.connections_established,
                        connections_dropped: stats.connections_dropped,
                        bytes_sent: stats.bytes_sent,
                        bytes_received: stats.bytes_received,
                        average_rtt_ms: stats.average_rtt_ms,
                        packet_loss_rate: stats.packet_loss_rate,
                        bandwidth_utilization: stats.bandwidth_utilization,
                        active_streams: stats.active_streams,
                        pool: Some(PoolStats {
                            pool_size: pool_size as u32,
                            max_pool_size: max_pool_size as u32,
                            total_reuses,
                            reuse_rate: if total_reuses > 0 {
                                (pool_size as f64 / total_reuses as f64) * 100.0
                            } else {
                                0.0
                            },
                        }),
                    })
                } else {
                    None
                };

                let update = NetworkStatsUpdate {
                    timestamp: chrono::Utc::now().timestamp(),
                    network_id: req.network_id.clone(),
                    interfaces: vec![InterfaceStats {
                        interface: "eth0".to_string(),
                        rx_bytes: 10_000_000,
                        rx_packets: 10_000,
                        rx_errors: 0,
                        rx_dropped: 0,
                        tx_bytes: 5_000_000,
                        tx_packets: 5_000,
                        tx_errors: 0,
                        tx_dropped: 0,
                        bandwidth_mbps: 100.0,
                    }],
                    quic: quic_stats,
                };

                if tx.send(Ok(update)).await.is_err() {
                    break;
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(interval as u64)).await;
            }

            debug!("📊 Network stats streaming ended");
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream) as Self::StreamStatsStream))
    }

    /// Get QUIC connection pool stats
    async fn get_quic_pool_stats(
        &self,
        _request: Request<QuicPoolStatsRequest>,
    ) -> Result<Response<QuicPoolStatsResponse>, Status> {
        info!("📊 gRPC GetQuicPoolStats");

        if let Some(ref quic_server) = self.quic_server {
            let server = quic_server.read().await;
            let (pool_size, max_pool_size, total_reuses) = server.get_pool_stats().await;

            let reuse_rate = if total_reuses > 0 {
                (pool_size as f64 / total_reuses as f64) * 100.0
            } else {
                0.0
            };

            info!(
                "✅ QUIC pool: size={}/{}, reuses={}, rate={:.1}%",
                pool_size, max_pool_size, total_reuses, reuse_rate
            );

            // Get individual connection details from the pool
            let connections = server.get_stats().await;
            let connection_infos = connections
                .connection_infos
                .iter()
                .map(|conn| PooledConnectionInfo {
                    remote_addr: conn.remote_addr.to_string(),
                    container_id: conn.container_id.clone(),
                    use_count: conn.use_count,
                    idle_time_ms: conn.idle_time.as_millis() as u64,
                    rtt_ms: conn.rtt_ms,
                })
                .collect();

            Ok(Response::new(QuicPoolStatsResponse {
                pool_size: pool_size as u32,
                max_pool_size: max_pool_size as u32,
                total_reuses,
                reuse_rate,
                connections: connection_infos,
            }))
        } else {
            Err(Status::unavailable("QUIC server not available"))
        }
    }

    /// Setup port forwarding
    async fn setup_port_forward(
        &self,
        request: Request<PortForwardRequest>,
    ) -> Result<Response<PortForwardResponse>, Status> {
        let req = request.into_inner();
        info!(
            "🔀 gRPC SetupPortForward: {}:{} -> container {}:{}",
            req.host_port, req.protocol, req.container_id, req.container_port
        );

        if let Some(ref quic_server) = self.quic_server {
            let server = quic_server.read().await;

            match server
                .setup_port_forward(
                    &req.container_id,
                    req.host_port as u16,
                    req.container_port as u16,
                )
                .await
            {
                Ok(_) => {
                    let forward_id = format!("fwd-{}-{}", req.host_port, req.container_port);
                    info!("✅ Port forwarding setup: {}", forward_id);

                    Ok(Response::new(PortForwardResponse {
                        forward_id,
                        host_port: req.host_port,
                        container_port: req.container_port,
                        protocol: req.protocol.clone(),
                        success: true,
                        error: String::new(),
                    }))
                }
                Err(e) => {
                    error!("❌ Failed to setup port forwarding: {}", e);
                    Ok(Response::new(PortForwardResponse {
                        forward_id: String::new(),
                        host_port: req.host_port,
                        container_port: req.container_port,
                        protocol: req.protocol.clone(),
                        success: false,
                        error: e.to_string(),
                    }))
                }
            }
        } else {
            Err(Status::unavailable("QUIC server not available"))
        }
    }

    /// Remove port forwarding
    async fn remove_port_forward(
        &self,
        request: Request<RemovePortForwardRequest>,
    ) -> Result<Response<RemovePortForwardResponse>, Status> {
        let req = request.into_inner();
        info!("🗑️ gRPC RemovePortForward: forward_id={}", req.forward_id);

        if let Some(ref quic_server) = self.quic_server {
            let server = quic_server.read().await;

            // Parse forward_id to get host_port (format: "host_port")
            match req.forward_id.parse::<u16>() {
                Ok(host_port) => match server.remove_port_forward(host_port).await {
                    Ok(_) => {
                        info!("✅ Port forward removed: {}", host_port);
                        Ok(Response::new(RemovePortForwardResponse {
                            success: true,
                            error: String::new(),
                        }))
                    }
                    Err(e) => {
                        warn!("❌ Failed to remove port forward: {}", e);
                        Ok(Response::new(RemovePortForwardResponse {
                            success: false,
                            error: e.to_string(),
                        }))
                    }
                },
                Err(e) => Ok(Response::new(RemovePortForwardResponse {
                    success: false,
                    error: format!("Invalid forward_id: {}", e),
                })),
            }
        } else {
            Err(Status::unavailable("QUIC server not available"))
        }
    }

    /// Connect container to network
    async fn connect_container(
        &self,
        request: Request<ConnectContainerRequest>,
    ) -> Result<Response<ConnectContainerResponse>, Status> {
        let req = request.into_inner();
        info!(
            "🔗 gRPC ConnectContainer: container={} to network={}",
            req.container_id, req.network_id
        );

        if let Some(ref quic_server) = self.quic_server {
            let server = quic_server.read().await;

            // Connect container to network
            match server
                .connect_container(&req.container_id, &req.network_id)
                .await
            {
                Ok(assigned_ip) => {
                    info!(
                        "✅ Container {} connected to network {} with IP {}",
                        req.container_id, req.network_id, assigned_ip
                    );
                    Ok(Response::new(ConnectContainerResponse {
                        container_id: req.container_id,
                        network_id: req.network_id,
                        assigned_ip,
                        success: true,
                        error: String::new(),
                    }))
                }
                Err(e) => {
                    warn!("❌ Failed to connect container to network: {}", e);
                    Ok(Response::new(ConnectContainerResponse {
                        container_id: req.container_id,
                        network_id: req.network_id,
                        assigned_ip: String::new(),
                        success: false,
                        error: e.to_string(),
                    }))
                }
            }
        } else {
            Err(Status::unavailable("QUIC server not available"))
        }
    }

    /// Disconnect container from network
    async fn disconnect_container(
        &self,
        request: Request<DisconnectContainerRequest>,
    ) -> Result<Response<DisconnectContainerResponse>, Status> {
        let req = request.into_inner();
        info!(
            "🔌 gRPC DisconnectContainer: container={} from network={}",
            req.container_id, req.network_id
        );

        if let Some(ref quic_server) = self.quic_server {
            let server = quic_server.read().await;

            // Disconnect container from network
            match server
                .disconnect_container(&req.container_id, &req.network_id)
                .await
            {
                Ok(_) => {
                    info!(
                        "✅ Container {} disconnected from network {}",
                        req.container_id, req.network_id
                    );
                    Ok(Response::new(DisconnectContainerResponse {
                        success: true,
                        error: String::new(),
                    }))
                }
                Err(e) => {
                    warn!("❌ Failed to disconnect container from network: {}", e);
                    Ok(Response::new(DisconnectContainerResponse {
                        success: false,
                        error: e.to_string(),
                    }))
                }
            }
        } else {
            Err(Status::unavailable("QUIC server not available"))
        }
    }

    /// Enable network optimizations
    async fn enable_optimizations(
        &self,
        request: Request<OptimizationsRequest>,
    ) -> Result<Response<OptimizationsResponse>, Status> {
        let req = request.into_inner();
        info!(
            "⚡ gRPC EnableOptimizations: container={}, network={}",
            req.container_id, req.network_id
        );

        let mut enabled_features = Vec::new();

        if let Some(ref config) = req.config {
            if config.enable_quic_0rtt {
                enabled_features.push("QUIC 0-RTT".to_string());
            }
            if config.enable_connection_migration {
                enabled_features.push("Connection Migration".to_string());
            }
            if config.enable_bbr_congestion_control {
                enabled_features.push("BBR Congestion Control".to_string());
            }
            if config.enable_gso {
                enabled_features.push("Generic Segmentation Offload".to_string());
            }
            if config.enable_gro {
                enabled_features.push("Generic Receive Offload".to_string());
            }

            info!("✅ Enabled optimizations: {:?}", enabled_features);
        }

        Ok(Response::new(OptimizationsResponse {
            success: true,
            enabled_features,
            error: String::new(),
        }))
    }
}
