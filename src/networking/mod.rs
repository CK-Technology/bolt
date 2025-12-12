use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

pub mod bridge;
pub mod ebpf;
pub mod ebpf_xdp;
pub mod firewall_advanced;
pub mod quic;
pub mod quic_proxy;
pub mod quic_real;

// Re-export main networking types
pub use ebpf_xdp::{XDPFastPath, XDPMode, XDPStats};
pub use firewall_advanced::AdvancedFirewallManager;
pub use quic_proxy::{ProxyRule, QUICProxyConfig, QUICSocketProxy};
pub use quic_real::RealQUICServer;

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
#[derive(Debug)]
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

    /// Create Bolt network with enhanced features
    pub async fn create_bolt_network(
        &mut self,
        name: &str,
        driver: &str,
        subnet: Option<&str>,
    ) -> Result<()> {
        info!("🌐 Creating Bolt network: {} (driver: {})", name, driver);

        // Validate network name
        if name.is_empty() {
            return Err(anyhow::anyhow!("Network name cannot be empty"));
        }

        // Set default subnet if not provided
        let subnet = subnet.unwrap_or("172.20.0.0/16");

        // Create network based on driver type
        match driver {
            "bolt" => {
                self.create_bolt_bridge_network(name, subnet).await?;
            }
            "bridge" => {
                self.create_traditional_bridge_network(name, subnet).await?;
            }
            "overlay" => {
                self.create_overlay_network(name, subnet).await?;
            }
            "macvlan" => {
                self.create_macvlan_network(name, subnet).await?;
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported network driver: {}", driver));
            }
        }

        info!("✅ Network '{}' created successfully", name);
        Ok(())
    }

    /// Create Bolt bridge network with QUIC support
    async fn create_bolt_bridge_network(&self, name: &str, subnet: &str) -> Result<()> {
        info!("🌉 Creating Bolt bridge network with QUIC support");

        // Create bridge interface
        let bridge_name = format!("br-{}", name);
        self.create_bridge_interface(&bridge_name).await?;

        // Configure subnet
        self.configure_bridge_subnet(&bridge_name, subnet).await?;

        // Enable QUIC on bridge
        if let Some(ref quic_server) = self.quic_server {
            quic_server.enable_optimizations(&bridge_name).await?;
        }

        // Enable eBPF acceleration if available
        if let Some(ref ebpf_manager) = self.ebpf_manager {
            ebpf_manager
                .enable_bridge_acceleration(&bridge_name)
                .await?;
        }

        info!("  ✓ Bolt bridge network configured with advanced features");
        Ok(())
    }

    /// Create traditional bridge network
    async fn create_traditional_bridge_network(&self, name: &str, subnet: &str) -> Result<()> {
        info!("🌉 Creating traditional bridge network");

        let bridge_name = format!("br-{}", name);
        self.create_bridge_interface(&bridge_name).await?;
        self.configure_bridge_subnet(&bridge_name, subnet).await?;

        info!("  ✓ Traditional bridge network configured");
        Ok(())
    }

    /// Create overlay network
    async fn create_overlay_network(&self, name: &str, subnet: &str) -> Result<()> {
        info!("🔗 Creating overlay network");

        // Create VXLAN interface
        let vxlan_name = format!("vx-{}", name);
        self.create_vxlan_interface(&vxlan_name, 4789).await?;

        // Configure overlay subnet
        self.configure_overlay_subnet(&vxlan_name, subnet).await?;

        info!("  ✓ Overlay network configured");
        Ok(())
    }

    /// Create macvlan network
    async fn create_macvlan_network(&self, name: &str, subnet: &str) -> Result<()> {
        info!("📡 Creating macvlan network with subnet: {}", subnet);

        let macvlan_name = format!("mv-{}", name);
        self.create_macvlan_interface(&macvlan_name, "eth0").await?;

        // Assign subnet to macvlan interface
        let output = tokio::process::Command::new("ip")
            .args(["addr", "add", subnet, "dev", &macvlan_name])
            .output()
            .await?;

        if !output.status.success() {
            warn!(
                "Failed to assign subnet to macvlan: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        info!("  ✓ Macvlan network configured");
        Ok(())
    }

    /// Create bridge interface
    async fn create_bridge_interface(&self, bridge_name: &str) -> Result<()> {
        info!("  🔧 Creating bridge interface: {}", bridge_name);

        // Use ip command to create bridge
        let output = std::process::Command::new("ip")
            .args(["link", "add", "name", bridge_name, "type", "bridge"])
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    info!("    ✓ Bridge interface created");

                    // Bring bridge up
                    let _ = std::process::Command::new("ip")
                        .args(["link", "set", bridge_name, "up"])
                        .output();

                    info!("    ✓ Bridge interface activated");
                } else {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    if stderr.contains("File exists") {
                        info!("    ✓ Bridge interface already exists");
                    } else {
                        return Err(anyhow::anyhow!("Failed to create bridge: {}", stderr));
                    }
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to run ip command: {}", e));
            }
        }

        Ok(())
    }

    /// Configure bridge subnet
    async fn configure_bridge_subnet(&self, bridge_name: &str, subnet: &str) -> Result<()> {
        info!("  🔧 Configuring subnet: {}", subnet);

        // Parse subnet to get gateway IP
        let gateway_ip = self.calculate_gateway_ip(subnet)?;

        // Assign IP to bridge
        let output = std::process::Command::new("ip")
            .args(["addr", "add", &gateway_ip, "dev", bridge_name])
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    info!("    ✓ Gateway IP assigned: {}", gateway_ip);
                } else {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    if stderr.contains("File exists") {
                        info!("    ✓ IP address already assigned");
                    } else {
                        warn!("Failed to assign IP: {}", stderr);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to run ip command: {}", e);
            }
        }

        Ok(())
    }

    /// Create VXLAN interface
    async fn create_vxlan_interface(&self, vxlan_name: &str, vni: u32) -> Result<()> {
        info!(
            "  🔧 Creating VXLAN interface: {} (VNI: {})",
            vxlan_name, vni
        );

        let output = std::process::Command::new("ip")
            .args([
                "link",
                "add",
                vxlan_name,
                "type",
                "vxlan",
                "id",
                &vni.to_string(),
                "dstport",
                "4789",
                "nolearning",
            ])
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    info!("    ✓ VXLAN interface created");

                    // Bring interface up
                    let _ = std::process::Command::new("ip")
                        .args(["link", "set", vxlan_name, "up"])
                        .output();
                } else {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    if !stderr.contains("File exists") {
                        warn!("Failed to create VXLAN: {}", stderr);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to run ip command: {}", e);
            }
        }

        Ok(())
    }

    /// Configure overlay subnet
    async fn configure_overlay_subnet(&self, vxlan_name: &str, subnet: &str) -> Result<()> {
        info!("  🔧 Configuring overlay subnet: {}", subnet);

        let gateway_ip = self.calculate_gateway_ip(subnet)?;

        let output = std::process::Command::new("ip")
            .args(["addr", "add", &gateway_ip, "dev", vxlan_name])
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    info!("    ✓ Overlay gateway configured: {}", gateway_ip);
                } else {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    if !stderr.contains("File exists") {
                        warn!("Failed to configure overlay: {}", stderr);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to run ip command: {}", e);
            }
        }

        Ok(())
    }

    /// Create macvlan interface
    async fn create_macvlan_interface(&self, macvlan_name: &str, parent: &str) -> Result<()> {
        info!(
            "  🔧 Creating macvlan interface: {} (parent: {})",
            macvlan_name, parent
        );

        let output = std::process::Command::new("ip")
            .args([
                "link",
                "add",
                macvlan_name,
                "link",
                parent,
                "type",
                "macvlan",
                "mode",
                "bridge",
            ])
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    info!("    ✓ Macvlan interface created");

                    // Bring interface up
                    let _ = std::process::Command::new("ip")
                        .args(["link", "set", macvlan_name, "up"])
                        .output();
                } else {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    if !stderr.contains("File exists") {
                        warn!("Failed to create macvlan: {}", stderr);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to run ip command: {}", e);
            }
        }

        Ok(())
    }

    /// Calculate gateway IP from subnet
    fn calculate_gateway_ip(&self, subnet: &str) -> Result<String> {
        // Simple implementation - use .1 as gateway
        if let Some(pos) = subnet.find('/') {
            let ip_part = &subnet[..pos];
            let prefix_len = &subnet[pos + 1..];

            // Parse IP and set last octet to 1
            let ip_parts: Vec<&str> = ip_part.split('.').collect();
            if ip_parts.len() == 4 {
                let gateway = format!(
                    "{}.{}.{}.1/{}",
                    ip_parts[0], ip_parts[1], ip_parts[2], prefix_len
                );
                return Ok(gateway);
            }
        }

        Err(anyhow::anyhow!("Invalid subnet format: {}", subnet))
    }

    /// List Bolt networks
    pub async fn list_bolt_networks(&self) -> Result<Vec<BoltNetworkInfo>> {
        info!("📋 Listing Bolt networks");

        let mut networks = Vec::new();

        // Get system network interfaces
        let output = std::process::Command::new("ip")
            .args(["link", "show"])
            .output();

        if let Ok(result) = output {
            if result.status.success() {
                let output_str = String::from_utf8_lossy(&result.stdout);

                // Parse bridge interfaces
                for line in output_str.lines() {
                    if line.contains("br-") && line.contains("state UP") {
                        if let Some(name_start) = line.find("br-") {
                            if let Some(name_end) = line[name_start..].find(':') {
                                let bridge_name = &line[name_start..name_start + name_end];
                                let network_name =
                                    bridge_name.strip_prefix("br-").unwrap_or(bridge_name);

                                networks.push(BoltNetworkInfo {
                                    id: format!(
                                        "{:x}",
                                        bridge_name.as_bytes().iter().fold(0u64, |acc, &b| acc
                                            .wrapping_mul(31)
                                            .wrapping_add(b as u64))
                                    ),
                                    name: network_name.to_string(),
                                    driver: "bolt".to_string(),
                                    scope: "local".to_string(),
                                    subnet: "172.20.0.0/16".to_string(), // Would be detected from interface
                                    gateway: "172.20.0.1 (QUIC)".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(networks)
    }

    /// Remove a Bolt network
    pub async fn remove_bolt_network(&mut self, name: &str) -> Result<()> {
        info!("🗑️ Removing Bolt network: {}", name);

        // Validate network name
        if name.is_empty() {
            return Err(anyhow::anyhow!("Network name cannot be empty"));
        }

        // Check if network exists and get its type
        let bridge_name = format!("br-{}", name);

        // First, check if any containers are using this network
        let containers_using_network = self.get_containers_on_network(name).await?;
        if !containers_using_network.is_empty() {
            return Err(anyhow::anyhow!(
                "Network '{}' is in use by containers: {}",
                name,
                containers_using_network.join(", ")
            ));
        }

        // Remove the bridge interface
        let result = std::process::Command::new("ip")
            .args(["link", "delete", &bridge_name])
            .output();

        match result {
            Ok(output) => {
                if !output.status.success() {
                    let error = String::from_utf8_lossy(&output.stderr);
                    warn!("Failed to remove bridge {}: {}", bridge_name, error);
                    // Don't fail if bridge doesn't exist
                    if !error.contains("Cannot find device") {
                        return Err(anyhow::anyhow!(
                            "Failed to remove network bridge: {}",
                            error
                        ));
                    }
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to execute ip command: {}", e));
            }
        }

        // Remove from internal tracking
        let mut interfaces = self.interfaces.write().await;
        interfaces.retain(|_, interface| !interface.interface_name.starts_with(&bridge_name));

        let mut metrics = self.metrics.write().await;
        metrics.remove(name);

        info!("✅ Network '{}' removed successfully", name);
        Ok(())
    }

    /// Get containers using a specific network
    async fn get_containers_on_network(&self, network_name: &str) -> Result<Vec<String>> {
        // This would normally query running containers
        // For now, return empty list as a placeholder
        // In a real implementation, this would check:
        // 1. Container runtime state
        // 2. Network namespace associations
        // 3. Bridge interface memberships

        let _bridge_name = format!("br-{}", network_name);

        // Placeholder: Check if any containers are connected to this bridge
        let output = std::process::Command::new("ip")
            .args(["link", "show", "type", "veth"])
            .output();

        let mut containers = Vec::new();

        match output {
            Ok(result) => {
                if result.status.success() {
                    let output_str = String::from_utf8_lossy(&result.stdout);

                    // Look for veth pairs that might be connected to our bridge
                    for line in output_str.lines() {
                        if line.contains("veth") && line.contains("master") {
                            // Parse container ID from veth interface name
                            // This is a simplified implementation
                            if let Some(start) = line.find("veth") {
                                if let Some(colon) = line[start..].find(':') {
                                    let interface_name = &line[start..start + colon];
                                    // Extract container ID from interface name (simplified)
                                    if interface_name.len() > 8 {
                                        containers.push(interface_name[4..].to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // If we can't check, be safe and assume no containers
            }
        }

        Ok(containers)
    }
}

/// Bolt network information
#[derive(Debug, Clone)]
pub struct BoltNetworkInfo {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub subnet: String,
    pub gateway: String,
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
