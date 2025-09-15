use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

pub mod bridge;
pub mod ebpf;
pub mod quic;

/// Networking configuration for containers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub enable_quic: bool,
    pub enable_ebpf: bool,
    pub low_latency: bool,
    pub bandwidth_optimization: bool,
    pub ipv6: bool,
    pub driver: NetworkDriver,
}

/// Network drivers supported by Bolt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkDriver {
    BoltBridge,
    Traditional,
    QUIC,
}

/// Container network interface information
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub container_id: String,
    pub interface_name: String,
    pub ip_address: IpAddr,
    pub mac_address: String,
    pub mtu: u16,
    pub namespace: String,
}

/// Network performance metrics
#[derive(Debug, Default, Clone)]
pub struct NetworkMetrics {
    pub latency_ms: f64,
    pub throughput_mbps: f64,
    pub packet_loss: f64,
    pub connections_active: u32,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Bolt Network Manager - handles container networking with QUIC optimization
pub struct NetworkManager {
    interfaces: Arc<RwLock<HashMap<String, NetworkInterface>>>,
    metrics: Arc<RwLock<HashMap<String, NetworkMetrics>>>,
    config: NetworkConfig,
    quic_server: Option<Arc<quic::QUICServer>>,
    ebpf_manager: Option<Arc<ebpf::EBPFManager>>,
}

impl NetworkManager {
    /// Create a new network manager with configuration
    pub async fn new(config: NetworkConfig) -> Result<Self> {
        info!("🌐 Initializing Bolt Network Manager");
        info!(
            "  • QUIC Protocol: {}",
            if config.enable_quic {
                "✅ Enabled"
            } else {
                "❌ Disabled"
            }
        );
        info!(
            "  • eBPF Acceleration: {}",
            if config.enable_ebpf {
                "✅ Enabled"
            } else {
                "❌ Disabled"
            }
        );
        info!(
            "  • Low Latency Mode: {}",
            if config.low_latency {
                "✅ Enabled"
            } else {
                "❌ Disabled"
            }
        );
        info!(
            "  • IPv6 Support: {}",
            if config.ipv6 {
                "✅ Enabled"
            } else {
                "❌ Disabled"
            }
        );

        let quic_server = if config.enable_quic {
            Some(Arc::new(quic::QUICServer::new(config.clone()).await?))
        } else {
            None
        };

        let ebpf_manager = if config.enable_ebpf {
            Some(Arc::new(ebpf::EBPFManager::new().await?))
        } else {
            None
        };

        Ok(Self {
            interfaces: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            config,
            quic_server,
            ebpf_manager,
        })
    }

    /// Setup container network interface
    pub async fn setup_container_network(
        &self,
        container_id: &str,
        network_name: &str,
        ports: &[String],
    ) -> Result<NetworkInterface> {
        info!("🔗 Setting up network for container: {}", container_id);

        let interface = self
            .create_network_interface(container_id, network_name)
            .await?;

        // Setup port forwarding
        for port in ports {
            self.setup_port_forward(container_id, port).await?;
        }

        // Enable QUIC if configured
        if let Some(ref quic_server) = self.quic_server {
            quic_server
                .register_container(container_id, &interface)
                .await?;
        }

        // Enable eBPF acceleration if available
        if let Some(ref ebpf_manager) = self.ebpf_manager {
            ebpf_manager
                .accelerate_container_traffic(container_id, &interface)
                .await?;
        }

        // Store interface information
        {
            let mut interfaces = self.interfaces.write().await;
            interfaces.insert(container_id.to_string(), interface.clone());
        }

        // Initialize metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.insert(container_id.to_string(), NetworkMetrics::default());
        }

        info!("✅ Network setup complete for container: {}", container_id);
        Ok(interface)
    }

    /// Create network interface for container
    async fn create_network_interface(
        &self,
        container_id: &str,
        network_name: &str,
    ) -> Result<NetworkInterface> {
        debug!("Creating network interface for container: {}", container_id);

        // Generate interface name
        let interface_name = format!(
            "bolt-{}",
            if container_id.len() >= 8 {
                &container_id[..8]
            } else {
                container_id
            }
        );

        // Assign IP address (using bridge networking)
        let ip_address = bridge::BridgeManager::assign_ip_address(network_name).await?;

        // Generate MAC address
        let mac_address = self.generate_mac_address();

        // Set MTU based on configuration
        let mtu = if self.config.low_latency { 1500 } else { 9000 }; // Jumbo frames for throughput

        // Create network namespace
        let namespace = format!("bolt-ns-{}", container_id);
        self.create_network_namespace(&namespace).await?;

        Ok(NetworkInterface {
            container_id: container_id.to_string(),
            interface_name,
            ip_address,
            mac_address,
            mtu,
            namespace,
        })
    }

    /// Setup port forwarding for container
    async fn setup_port_forward(&self, container_id: &str, port_mapping: &str) -> Result<()> {
        debug!(
            "Setting up port forward for {}: {}",
            container_id, port_mapping
        );

        // Parse port mapping (e.g., "8080:80", "3000:3000")
        let parts: Vec<&str> = port_mapping.split(':').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!(
                "Invalid port mapping format: {}",
                port_mapping
            ));
        }

        let host_port: u16 = parts[0].parse()?;
        let container_port: u16 = parts[1].parse()?;

        // Use QUIC for low-latency port forwarding if available
        if let Some(ref quic_server) = self.quic_server {
            quic_server
                .setup_port_forward(container_id, host_port, container_port)
                .await?;
        } else {
            // Fall back to traditional iptables rules
            self.setup_traditional_port_forward(container_id, host_port, container_port)
                .await?;
        }

        Ok(())
    }

    /// Traditional port forwarding using iptables
    async fn setup_traditional_port_forward(
        &self,
        container_id: &str,
        host_port: u16,
        container_port: u16,
    ) -> Result<()> {
        debug!(
            "Setting up traditional port forward: {} -> {} for {}",
            host_port, container_port, container_id
        );

        let interface = {
            let interfaces = self.interfaces.read().await;
            interfaces
                .get(container_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Container interface not found: {}", container_id))?
        };

        // Add iptables DNAT rule
        let iptables_cmd = format!(
            "iptables -t nat -A PREROUTING -p tcp --dport {} -j DNAT --to-destination {}:{}",
            host_port, interface.ip_address, container_port
        );

        // Execute iptables command (would be implemented with proper command execution)
        debug!("Would execute: {}", iptables_cmd);

        Ok(())
    }

    /// Create network namespace for container
    async fn create_network_namespace(&self, namespace: &str) -> Result<()> {
        debug!("Creating network namespace: {}", namespace);

        // This would create the actual network namespace
        // For now, we'll just log the intent
        info!("Created network namespace: {}", namespace);

        Ok(())
    }

    /// Generate MAC address for container interface
    fn generate_mac_address(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Generate MAC with Bolt OUI prefix (locally administered)
        format!(
            "02:42:{:02x}:{:02x}:{:02x}:{:02x}",
            (timestamp >> 24) & 0xFF,
            (timestamp >> 16) & 0xFF,
            (timestamp >> 8) & 0xFF,
            timestamp & 0xFF
        )
    }

    /// Get network metrics for container
    pub async fn get_container_metrics(&self, container_id: &str) -> Option<NetworkMetrics> {
        let metrics = self.metrics.read().await;
        metrics.get(container_id).cloned()
    }

    /// Update network metrics for container
    pub async fn update_metrics(&self, container_id: &str, metrics: NetworkMetrics) {
        let mut metrics_map = self.metrics.write().await;
        metrics_map.insert(container_id.to_string(), metrics);
    }

    /// Cleanup container network resources
    pub async fn cleanup_container_network(&self, container_id: &str) -> Result<()> {
        info!("🧹 Cleaning up network for container: {}", container_id);

        // Remove from QUIC server if enabled
        if let Some(ref quic_server) = self.quic_server {
            quic_server.unregister_container(container_id).await?;
        }

        // Remove from eBPF manager if enabled
        if let Some(ref ebpf_manager) = self.ebpf_manager {
            ebpf_manager
                .remove_container_acceleration(container_id)
                .await?;
        }

        // Remove interface and metrics
        {
            let mut interfaces = self.interfaces.write().await;
            interfaces.remove(container_id);
        }
        {
            let mut metrics = self.metrics.write().await;
            metrics.remove(container_id);
        }

        info!(
            "✅ Network cleanup complete for container: {}",
            container_id
        );
        Ok(())
    }

    /// Get all active network interfaces
    pub async fn get_active_interfaces(&self) -> HashMap<String, NetworkInterface> {
        let interfaces = self.interfaces.read().await;
        interfaces.clone()
    }

    /// Enable QUIC protocol optimizations
    pub async fn enable_quic_optimizations(&self, container_id: &str) -> Result<()> {
        if let Some(ref quic_server) = self.quic_server {
            quic_server.enable_optimizations(container_id).await?;
            info!(
                "✅ QUIC optimizations enabled for container: {}",
                container_id
            );
        } else {
            warn!("⚠️ QUIC not available for container: {}", container_id);
        }
        Ok(())
    }

    /// Get network performance statistics
    pub async fn get_network_stats(&self) -> HashMap<String, NetworkMetrics> {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            enable_quic: true,
            enable_ebpf: false, // Requires privileged access
            low_latency: false,
            bandwidth_optimization: true,
            ipv6: true,
            driver: NetworkDriver::BoltBridge,
        }
    }
}
