use anyhow::{Result, Context};
use quinn::{Endpoint, ServerConfig, ClientConfig, Connection, Incoming};
use rustls::{Certificate, PrivateKey, ServerConfig as TlsServerConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, debug, warn, error};
use uuid::Uuid;

pub mod mesh;
pub mod discovery;
pub mod gaming;

/// Bolt QUIC Fabric - Ultra-fast, encrypted service mesh
///
/// Features:
/// 1. 0-RTT connection establishment
/// 2. Built-in encryption and authentication
/// 3. Gaming-optimized low latency
/// 4. Automatic service discovery
/// 5. Load balancing and failover
/// 6. Bandwidth management
#[derive(Debug)]
pub struct QuicFabric {
    pub node_id: String,
    pub bind_addr: SocketAddr,
    pub endpoint: Option<Endpoint>,
    pub connections: Arc<RwLock<HashMap<String, QuicConnection>>>,
    pub services: Arc<RwLock<HashMap<String, ServiceEndpoint>>>,
    pub gaming_mode: bool,
    pub discovery: discovery::ServiceDiscovery,
}

#[derive(Debug, Clone)]
pub struct QuicConnection {
    pub id: String,
    pub remote_node: String,
    pub connection: Connection,
    pub established_at: chrono::DateTime<chrono::Utc>,
    pub stats: ConnectionStats,
    pub service_type: ServiceType,
}

#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub rtt_ms: f64,
    pub congestion_window: u64,
    pub gaming_optimized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceType {
    Web,
    Database,
    Gaming,
    Storage,
    Compute,
    Mesh, // Service mesh control plane
}

#[derive(Debug, Clone)]
pub struct ServiceEndpoint {
    pub name: String,
    pub service_type: ServiceType,
    pub endpoint_addr: SocketAddr,
    pub health_status: HealthStatus,
    pub load_factor: f32, // 0.0 to 1.0
    pub gaming_priority: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuicMessage {
    pub id: String,
    pub source: String,
    pub destination: String,
    pub message_type: MessageType,
    pub payload: Vec<u8>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub priority: MessagePriority,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MessageType {
    ServiceRequest,
    ServiceResponse,
    HealthCheck,
    ServiceDiscovery,
    LoadBalancing,
    Gaming,
    Mesh,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    Low,
    Normal,
    High,
    Gaming,     // Ultra-high priority for gaming traffic
    Critical,   // System-critical messages
}

impl QuicFabric {
    pub async fn new(
        node_id: String,
        bind_addr: SocketAddr,
        gaming_mode: bool,
    ) -> Result<Self> {
        info!("🌐 Initializing QUIC Fabric node: {} at {}", node_id, bind_addr);

        let discovery = discovery::ServiceDiscovery::new(node_id.clone()).await?;

        Ok(Self {
            node_id,
            bind_addr,
            endpoint: None,
            connections: Arc::new(RwLock::new(HashMap::new())),
            services: Arc::new(RwLock::new(HashMap::new())),
            gaming_mode,
            discovery,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting QUIC Fabric");

        // Create TLS configuration
        let server_config = self.create_server_config().await?;

        // Create QUIC endpoint
        let endpoint = Endpoint::server(server_config, self.bind_addr)?;
        let local_addr = endpoint.local_addr()?;

        info!("✅ QUIC endpoint listening on: {}", local_addr);

        // Handle incoming connections
        let connections = self.connections.clone();
        let services = self.services.clone();
        let gaming_mode = self.gaming_mode;
        let node_id = self.node_id.clone();

        let mut incoming = endpoint.accept();
        tokio::spawn(async move {
            while let Some(conn) = incoming.next().await {
                let connections = connections.clone();
                let services = services.clone();
                let node_id = node_id.clone();

                tokio::spawn(async move {
                    if let Err(e) = Self::handle_incoming_connection(
                        conn,
                        connections,
                        services,
                        gaming_mode,
                        node_id,
                    ).await {
                        error!("Failed to handle incoming connection: {}", e);
                    }
                });
            }
        });

        self.endpoint = Some(endpoint);

        // Start service discovery
        self.discovery.start().await?;

        // Start gaming optimizations if enabled
        if self.gaming_mode {
            self.start_gaming_optimizations().await?;
        }

        info!("✅ QUIC Fabric started successfully");
        Ok(())
    }

    async fn create_server_config(&self) -> Result<ServerConfig> {
        info!("🔐 Creating QUIC server configuration");

        // Generate self-signed certificate for now
        // In production, use proper certificates
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
        let cert_der = cert.serialize_der()?;
        let priv_key = cert.serialize_private_key_der();

        let cert_chain = vec![Certificate(cert_der)];
        let key = PrivateKey(priv_key);

        let mut tls_config = TlsServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(cert_chain, key)?;

        // Gaming optimizations
        if self.gaming_mode {
            info!("🎮 Applying gaming network optimizations");
            // Configure for low latency
            tls_config.alpn_protocols = vec![b"bolt-gaming".to_vec(), b"bolt".to_vec()];
        } else {
            tls_config.alpn_protocols = vec![b"bolt".to_vec()];
        }

        let mut server_config = ServerConfig::with_crypto(Arc::new(tls_config));

        // Configure transport parameters
        let mut transport_config = quinn::TransportConfig::default();

        if self.gaming_mode {
            // Gaming optimizations
            transport_config.max_idle_timeout(Some(std::time::Duration::from_secs(60).try_into()?));
            transport_config.keep_alive_interval(Some(std::time::Duration::from_secs(5)));
            transport_config.max_concurrent_bidi_streams(1000u32.try_into()?);
            transport_config.max_concurrent_uni_streams(1000u32.try_into()?);
            transport_config.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
        } else {
            transport_config.max_idle_timeout(Some(std::time::Duration::from_secs(300).try_into()?));
            transport_config.keep_alive_interval(Some(std::time::Duration::from_secs(30)));
        }

        server_config.transport_config(Arc::new(transport_config));

        Ok(server_config)
    }

    async fn handle_incoming_connection(
        conn: quinn::Connecting,
        connections: Arc<RwLock<HashMap<String, QuicConnection>>>,
        _services: Arc<RwLock<HashMap<String, ServiceEndpoint>>>,
        gaming_mode: bool,
        node_id: String,
    ) -> Result<()> {
        let connection = conn.await?;
        let remote_addr = connection.remote_address();

        info!("📡 New QUIC connection from: {}", remote_addr);

        // Create connection record
        let conn_id = Uuid::new_v4().to_string();
        let quic_conn = QuicConnection {
            id: conn_id.clone(),
            remote_node: remote_addr.to_string(),
            connection: connection.clone(),
            established_at: chrono::Utc::now(),
            stats: ConnectionStats {
                bytes_sent: 0,
                bytes_received: 0,
                packets_sent: 0,
                packets_received: 0,
                rtt_ms: 0.0,
                congestion_window: 0,
                gaming_optimized: gaming_mode,
            },
            service_type: ServiceType::Web, // Default, will be updated
        };

        connections.write().await.insert(conn_id.clone(), quic_conn);

        // Handle streams
        tokio::spawn(async move {
            loop {
                match connection.accept_bi().await {
                    Ok((mut send, mut recv)) => {
                        tokio::spawn(async move {
                            // Handle bidirectional stream
                            let mut buffer = vec![0u8; 1024];
                            match recv.read(&mut buffer).await {
                                Ok(Some(n)) => {
                                    debug!("Received {} bytes", n);
                                    // Echo back for now
                                    if let Err(e) = send.write_all(&buffer[..n]).await {
                                        error!("Failed to send response: {}", e);
                                    }
                                }
                                Ok(None) => {
                                    debug!("Stream closed by peer");
                                }
                                Err(e) => {
                                    error!("Stream read error: {}", e);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        if let quinn::ConnectionError::ApplicationClosed(_) = e {
                            info!("Connection closed normally");
                            break;
                        } else {
                            error!("Connection error: {}", e);
                            break;
                        }
                    }
                }
            }

            // Clean up connection
            connections.write().await.remove(&conn_id);
            info!("Connection {} cleaned up", conn_id);
        });

        Ok(())
    }

    pub async fn connect_to_service(
        &self,
        service_name: &str,
        target_addr: SocketAddr,
    ) -> Result<Connection> {
        info!("🔌 Connecting to service: {} at {}", service_name, target_addr);

        if self.endpoint.is_none() {
            return Err(anyhow::anyhow!("QUIC endpoint not started"));
        }

        let endpoint = self.endpoint.as_ref().unwrap();

        // Create client configuration
        let mut client_config = ClientConfig::with_native_roots();

        if self.gaming_mode {
            client_config.alpn_protocols = vec![b"bolt-gaming".to_vec(), b"bolt".to_vec()];
        } else {
            client_config.alpn_protocols = vec![b"bolt".to_vec()];
        }

        // Configure transport for gaming if needed
        let mut transport_config = quinn::TransportConfig::default();
        if self.gaming_mode {
            transport_config.max_idle_timeout(Some(std::time::Duration::from_secs(60).try_into()?));
            transport_config.keep_alive_interval(Some(std::time::Duration::from_secs(5)));
            transport_config.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
        }

        client_config.transport_config(Arc::new(transport_config));

        // Connect
        let connecting = endpoint.connect_with(client_config, target_addr, "localhost")?;
        let connection = connecting.await?;

        info!("✅ Connected to service: {}", service_name);

        // Store connection
        let conn_id = Uuid::new_v4().to_string();
        let quic_conn = QuicConnection {
            id: conn_id.clone(),
            remote_node: target_addr.to_string(),
            connection: connection.clone(),
            established_at: chrono::Utc::now(),
            stats: ConnectionStats {
                bytes_sent: 0,
                bytes_received: 0,
                packets_sent: 0,
                packets_received: 0,
                rtt_ms: 0.0,
                congestion_window: 0,
                gaming_optimized: self.gaming_mode,
            },
            service_type: ServiceType::Web,
        };

        self.connections.write().await.insert(conn_id, quic_conn);

        Ok(connection)
    }

    pub async fn send_message(
        &self,
        connection: &Connection,
        message: QuicMessage,
    ) -> Result<Vec<u8>> {
        debug!("📤 Sending message: {:?}", message.message_type);

        // Serialize message
        let serialized = bincode::serialize(&message)?;

        // Open bidirectional stream
        let (mut send, mut recv) = connection.open_bi().await?;

        // Send message
        send.write_all(&serialized).await?;
        send.finish().await?;

        // Read response
        let response = recv.read_to_end(1024 * 1024).await?; // 1MB limit

        debug!("📥 Received response: {} bytes", response.len());
        Ok(response)
    }

    pub async fn register_service(
        &mut self,
        name: String,
        service_type: ServiceType,
        endpoint_addr: SocketAddr,
    ) -> Result<()> {
        info!("📝 Registering service: {} ({:?}) at {}", name, service_type, endpoint_addr);

        let service = ServiceEndpoint {
            name: name.clone(),
            service_type,
            endpoint_addr,
            health_status: HealthStatus::Unknown,
            load_factor: 0.0,
            gaming_priority: matches!(service_type, ServiceType::Gaming),
        };

        self.services.write().await.insert(name.clone(), service);

        // Register with service discovery
        self.discovery.register_service(&name, endpoint_addr, service_type).await?;

        info!("✅ Service registered: {}", name);
        Ok(())
    }

    async fn start_gaming_optimizations(&self) -> Result<()> {
        info!("🎮 Starting gaming network optimizations");

        // Gaming-specific network optimizations:
        // 1. Priority packet scheduling
        // 2. Jitter buffer management
        // 3. Latency monitoring
        // 4. Bandwidth allocation
        // 5. Connection preheating

        tokio::spawn(async {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
            loop {
                interval.tick().await;
                // Monitor and optimize gaming connections
                // TODO: Implement gaming optimization loop
            }
        });

        Ok(())
    }

    pub async fn get_connection_stats(&self) -> Vec<ConnectionStats> {
        let connections = self.connections.read().await;
        connections.values()
            .map(|conn| conn.stats.clone())
            .collect()
    }

    pub async fn list_services(&self) -> Vec<ServiceEndpoint> {
        let services = self.services.read().await;
        services.values().cloned().collect()
    }

    pub fn is_gaming_mode(&self) -> bool {
        self.gaming_mode
    }
}

impl Default for MessagePriority {
    fn default() -> Self {
        MessagePriority::Normal
    }
}

/// Gaming-specific QUIC extensions
pub mod gaming_extensions {
    use super::*;

    pub struct GamingOptimizer {
        pub target_latency_ms: f64,
        pub max_jitter_ms: f64,
        pub bandwidth_limit_mbps: Option<u64>,
        pub priority_scheduling: bool,
    }

    impl GamingOptimizer {
        pub fn new() -> Self {
            Self {
                target_latency_ms: 10.0,  // 10ms target for competitive gaming
                max_jitter_ms: 2.0,       // 2ms maximum jitter
                bandwidth_limit_mbps: None,
                priority_scheduling: true,
            }
        }

        pub async fn optimize_connection(
            &self,
            connection: &Connection,
        ) -> Result<()> {
            info!("🎯 Optimizing connection for gaming");

            // Set congestion control for gaming
            // Configure packet pacing
            // Set up priority queues
            // Enable 0-RTT for reconnections

            // TODO: Implement actual gaming optimizations
            Ok(())
        }

        pub fn calculate_gaming_score(
            &self,
            stats: &ConnectionStats,
        ) -> f64 {
            // Gaming quality score based on:
            // - Latency (most important)
            // - Jitter (second most important)
            // - Packet loss (third)
            // - Bandwidth utilization

            let latency_score = if stats.rtt_ms <= self.target_latency_ms {
                1.0
            } else {
                (self.target_latency_ms / stats.rtt_ms).max(0.0)
            };

            // TODO: Add jitter and packet loss calculations
            let jitter_score = 1.0; // Placeholder
            let packet_loss_score = 1.0; // Placeholder

            (latency_score * 0.6) + (jitter_score * 0.3) + (packet_loss_score * 0.1)
        }
    }
}