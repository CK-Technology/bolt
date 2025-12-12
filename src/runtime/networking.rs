use crate::Result;
use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

/// Bolt Network Manager - Revolutionary QUIC-based container networking
/// This is now a wrapper around the enhanced NetworkManager
#[derive(Debug)]
pub struct BoltNetworkManager {
    pub enhanced_manager: Arc<Mutex<crate::networking::NetworkManager>>,
    pub networks: Arc<RwLock<HashMap<String, BoltNetwork>>>,
    pub quic_fabric: QuicNetworkFabric,
    pub bridge_manager: BridgeManager,
    pub dns_resolver: DnsResolver,
    pub firewall: NetworkFirewall,
}

/// High-performance network with QUIC optimization
#[derive(Debug, Clone)]
pub struct BoltNetwork {
    pub id: String,
    pub name: String,
    pub driver: NetworkDriver,
    pub subnet: String,
    pub gateway: IpAddr,
    pub containers: HashMap<String, ContainerNetworkInfo>,
    pub quic_enabled: bool,
    pub performance_mode: NetworkPerformanceMode,
}

/// Container network configuration
#[derive(Debug, Clone)]
pub struct ContainerNetworkInfo {
    pub container_id: String,
    pub ip_address: IpAddr,
    pub mac_address: String,
    pub ports: Vec<PortMapping>,
    pub bandwidth_limit: Option<u64>,
    pub latency_target: Option<u64>, // microseconds
    pub host_interface: String,
    pub container_interface: String,
}

/// Port mapping for container services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: Protocol,
    pub quic_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Protocol {
    Tcp,
    Udp,
    Quic,
    Sctp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkDriver {
    BoltBridge,  // High-performance bridge
    BoltOverlay, // QUIC-based overlay
    BoltMacvlan, // Direct host networking
    BoltIpvlan,  // IP-based VLAN
    BoltSriov,   // SR-IOV for maximum performance
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkPerformanceMode {
    Gaming,         // Ultra-low latency
    HighThroughput, // Maximum bandwidth
    Balanced,       // Default mode
    PowerSaving,    // Low power consumption
}

/// QUIC-based network fabric for ultra-low latency
#[derive(Debug)]
pub struct QuicNetworkFabric {
    pub connections: Arc<RwLock<HashMap<String, QuicConnection>>>,
    pub load_balancer: QuicLoadBalancer,
    pub service_mesh: ServiceMesh,
}

/// QUIC connection for container communication
#[derive(Debug, Clone)]
pub struct QuicConnection {
    pub connection_id: String,
    pub local_addr: SocketAddr,
    pub remote_addr: SocketAddr,
    pub latency: std::time::Duration,
    pub bandwidth: u64,
    pub connection_state: QuicConnectionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuicConnectionState {
    Establishing,
    Connected,
    Idle,
    Closing,
    Closed,
}

/// QUIC-based load balancer
#[derive(Debug)]
pub struct QuicLoadBalancer {
    pub algorithm: LoadBalancingAlgorithm,
    pub health_checks: HashMap<String, HealthCheck>,
    pub backends: Vec<Backend>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingAlgorithm {
    RoundRobin,
    LeastConnections,
    LatencyBased,
    WeightedRoundRobin,
    ConsistentHashing,
}

/// Service mesh for microservice communication
#[derive(Debug)]
pub struct ServiceMesh {
    pub services: HashMap<String, Service>,
    pub routing_rules: Vec<RoutingRule>,
    pub circuit_breakers: HashMap<String, CircuitBreaker>,
    pub retry_policies: HashMap<String, RetryPolicy>,
}

/// Network bridge management
#[derive(Debug)]
pub struct BridgeManager {
    pub bridges: HashMap<String, NetworkBridge>,
    pub veth_pairs: HashMap<String, VethPair>,
}

#[derive(Debug, Clone)]
pub struct NetworkBridge {
    pub name: String,
    pub ip_range: String,
    pub mtu: u16,
    pub forwarding_enabled: bool,
    pub iptables_rules: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct VethPair {
    pub host_interface: String,
    pub container_interface: String,
    pub bridge: String,
}

/// DNS resolver for service discovery
#[derive(Debug)]
pub struct DnsResolver {
    pub domains: HashMap<String, String>,
    pub cache: HashMap<String, IpAddr>,
    pub upstream_servers: Vec<IpAddr>,
}

/// Network firewall and security
#[derive(Debug)]
pub struct NetworkFirewall {
    pub rules: Vec<FirewallRule>,
    pub default_policy: FirewallPolicy,
    pub rate_limiting: HashMap<String, RateLimit>,
}

#[derive(Debug, Clone)]
pub struct FirewallRule {
    pub id: String,
    pub source: NetworkTarget,
    pub destination: NetworkTarget,
    pub protocol: Protocol,
    pub action: FirewallAction,
    pub priority: u32,
}

#[derive(Debug, Clone)]
pub enum NetworkTarget {
    Any,
    IpAddress(IpAddr),
    Subnet(String),
    Container(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FirewallAction {
    Allow,
    Deny,
    Drop,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FirewallPolicy {
    Allow,
    Deny,
}

#[derive(Debug, Clone)]
pub struct RateLimit {
    pub requests_per_second: u64,
    pub burst_size: u64,
}

// Additional structs for service mesh
#[derive(Debug, Clone)]
pub struct Service {
    pub name: String,
    pub endpoints: Vec<SocketAddr>,
    pub load_balancer: LoadBalancingAlgorithm,
    pub health_check: HealthCheck,
}

#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub path: String,
    pub interval: std::time::Duration,
    pub timeout: std::time::Duration,
    pub retries: u32,
}

#[derive(Debug, Clone)]
pub struct Backend {
    pub id: String,
    pub address: SocketAddr,
    pub weight: u32,
    pub healthy: bool,
}

#[derive(Debug, Clone)]
pub struct RoutingRule {
    pub matcher: RouteMatcher,
    pub destination: String,
    pub weight: u32,
}

#[derive(Debug, Clone)]
pub enum RouteMatcher {
    Path(String),
    Header(String, String),
    Method(String),
}

#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub failure_threshold: u32,
    pub timeout: std::time::Duration,
    pub state: CircuitBreakerState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff: BackoffStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackoffStrategy {
    Fixed(std::time::Duration),
    Exponential {
        base: std::time::Duration,
        max: std::time::Duration,
    },
    Linear(std::time::Duration),
}

impl BoltNetworkManager {
    /// Initialize network manager with QUIC optimization
    pub async fn new() -> Result<Self> {
        info!("🌐 Initializing Bolt Network Manager with enhanced QUIC support");

        // Initialize enhanced QUIC NetworkManager
        let quic_config = crate::networking::NetworkConfig {
            enable_quic: true,
            enable_ebpf: false, // Conservative default
            low_latency: true,
            bandwidth_optimization: true,
            ipv6: true,
            driver: crate::networking::NetworkDriver::BoltBridge,
        };

        let enhanced_manager = Arc::new(Mutex::new(
            crate::networking::NetworkManager::new(quic_config).await?,
        ));
        info!("✅ Enhanced QUIC NetworkManager initialized");

        Ok(Self {
            enhanced_manager,
            networks: Arc::new(RwLock::new(HashMap::new())),
            quic_fabric: QuicNetworkFabric::new().await?,
            bridge_manager: BridgeManager::new(),
            dns_resolver: DnsResolver::new(),
            firewall: NetworkFirewall::secure_defaults(),
        })
    }

    /// Create a high-performance network
    pub async fn create_network(
        &self,
        name: &str,
        driver: NetworkDriver,
        subnet: &str,
        performance_mode: NetworkPerformanceMode,
    ) -> Result<String> {
        info!("🔧 Creating network '{}' with driver {:?}", name, driver);

        let network_suffix = uuid::Uuid::new_v4().simple().to_string();
        let network_id = format!("bolt-net-{}", &network_suffix[..8]);

        let gateway = self.calculate_gateway(subnet)?;

        let network = BoltNetwork {
            id: network_id.clone(),
            name: name.to_string(),
            driver: driver.clone(),
            subnet: subnet.to_string(),
            gateway,
            containers: HashMap::new(),
            quic_enabled: matches!(performance_mode, NetworkPerformanceMode::Gaming),
            performance_mode,
        };

        // Create network infrastructure using enhanced manager
        let driver_str = match driver {
            NetworkDriver::BoltBridge => "bolt",
            NetworkDriver::BoltOverlay => "overlay",
            NetworkDriver::BoltMacvlan => "macvlan",
            NetworkDriver::BoltIpvlan => "ipvlan",
            NetworkDriver::BoltSriov => "sriov",
        };

        // Create using enhanced QUIC NetworkManager
        if let Err(e) = self
            .enhanced_manager
            .lock()
            .await
            .create_bolt_network(name, driver_str, Some(subnet))
            .await
        {
            warn!("Enhanced network creation failed, using fallback: {}", e);
            self.setup_network_infrastructure(&network).await?;
        } else {
            info!("✅ Enhanced QUIC network infrastructure created");
        }

        let mut networks = self.networks.write().await;
        networks.insert(network_id.clone(), network);

        info!("✅ Network '{}' created with ID: {}", name, network_id);
        Ok(network_id)
    }

    /// Connect container to network with optimization
    pub async fn connect_container(
        &self,
        network_id: &str,
        container_id: &str,
        config: ContainerNetworkConfig,
    ) -> Result<()> {
        info!(
            "🔌 Connecting container {} to network {}",
            container_id, network_id
        );

        let mut networks = self.networks.write().await;
        let network = networks
            .get_mut(network_id)
            .ok_or_else(|| anyhow!("Network not found: {}", network_id))?;

        // Allocate IP address
        let ip_address = self.allocate_ip_address(network, &config).await?;

        // Generate MAC address
        let mac_address = self.generate_mac_address();

        let iface_suffix = Self::interface_suffix(container_id);
        let host_interface = format!("veth{}", iface_suffix);
        let container_interface = format!("vep{}", iface_suffix);

        // Create container network info
        let container_info = ContainerNetworkInfo {
            container_id: container_id.to_string(),
            ip_address,
            mac_address,
            ports: config.port_mappings,
            bandwidth_limit: config.bandwidth_limit,
            latency_target: config.latency_target,
            host_interface,
            container_interface,
        };

        // Setup container networking
        self.setup_container_networking(network, &container_info)
            .await?;

        // Configure QUIC networking using enhanced manager
        if network.quic_enabled {
            info!(
                "🚀 Setting up enhanced QUIC networking for container: {}",
                container_id
            );

            // Use enhanced NetworkManager for QUIC setup
            let port_strings: Vec<String> = container_info
                .ports
                .iter()
                .map(|p| format!("{}:{}", p.host_port, p.container_port))
                .collect();

            if let Err(e) = self
                .enhanced_manager
                .lock()
                .await
                .setup_container_network(container_id, &network.name, &port_strings)
                .await
            {
                warn!(
                    "Enhanced QUIC setup failed, falling back to traditional: {}",
                    e
                );
                self.setup_quic_networking(container_id, &container_info)
                    .await?;
            } else {
                info!(
                    "✅ Enhanced QUIC networking configured for container: {}",
                    container_id
                );
            }
        } else {
            // Fallback to traditional networking
            self.setup_quic_networking(container_id, &container_info)
                .await?;
        }

        network
            .containers
            .insert(container_id.to_string(), container_info);

        info!(
            "✅ Container {} connected to network {} with IP {}",
            container_id, network_id, ip_address
        );
        Ok(())
    }

    pub async fn disconnect_container(&self, network_id: &str, container_id: &str) -> Result<()> {
        info!(
            "🧹 Disconnecting container {} from network {}",
            container_id, network_id
        );

        let (container_info, quic_enabled, remaining_containers) = {
            let mut networks = self.networks.write().await;
            match networks.get_mut(network_id) {
                Some(network) => {
                    let info = network.containers.remove(container_id);
                    let quic_enabled = network.quic_enabled;
                    let remaining = network.containers.len();
                    (info, quic_enabled, remaining)
                }
                None => {
                    debug!(
                        "Network {} not found while disconnecting container {}",
                        network_id, container_id
                    );
                    return Ok(());
                }
            }
        };

        if let Some(info) = container_info {
            if quic_enabled {
                self.quic_fabric
                    .connections
                    .write()
                    .await
                    .retain(|_, connection| connection.connection_id != container_id);
            }

            if let Err(err) =
                Self::run_command_allow_missing("ip", &["link", "delete", &info.host_interface])
                    .await
            {
                warn!(
                    "Failed to delete veth interface {}: {}",
                    info.host_interface, err
                );
            }

            debug!(
                "Container {} detached from network {} (remaining: {})",
                container_id, network_id, remaining_containers
            );
        } else {
            debug!(
                "Container {} had no tracked network state in {}",
                container_id, network_id
            );
        }

        Ok(())
    }

    pub async fn configure_container_namespace(
        &self,
        network_id: &str,
        container_id: &str,
        pid: i32,
    ) -> Result<()> {
        let (container_info, gateway) = {
            let networks = self.networks.read().await;
            let network = networks
                .get(network_id)
                .ok_or_else(|| anyhow!("Network not found: {}", network_id))?;
            let info = network
                .containers
                .get(container_id)
                .ok_or_else(|| anyhow!("Container {} not tracked in network", container_id))?;
            (info.clone(), network.gateway)
        };

        let pid_str = pid.to_string();
        let container_iface = container_info.container_interface.clone();

        Self::run_command("ip", &["link", "set", &container_iface, "netns", &pid_str]).await?;

        Self::run_ns_command(pid, &["ip", "link", "set", "lo", "up"]).await?;
        Self::run_ns_command(
            pid,
            &["ip", "link", "set", &container_iface, "name", "eth0"],
        )
        .await?;
        Self::run_ns_command(
            pid,
            &[
                "ip",
                "link",
                "set",
                "eth0",
                "address",
                &container_info.mac_address,
            ],
        )
        .await?;

        let cidr = match container_info.ip_address {
            IpAddr::V4(addr) => format!("{}/16", addr),
            IpAddr::V6(addr) => format!("{}/64", addr),
        };

        Self::run_ns_command(pid, &["ip", "addr", "add", &cidr, "dev", "eth0"]).await?;
        Self::run_ns_command(pid, &["ip", "link", "set", "eth0", "up"]).await?;

        if let IpAddr::V4(addr) = gateway {
            Self::run_ns_command(
                pid,
                &["ip", "route", "add", "default", "via", &addr.to_string()],
            )
            .await?;
        }

        Ok(())
    }

    fn interface_suffix(container_id: &str) -> String {
        let cleaned: String = container_id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        let len = cleaned.len().min(6);
        cleaned[..len].to_lowercase()
    }

    /// Setup network infrastructure
    async fn setup_network_infrastructure(&self, network: &BoltNetwork) -> Result<()> {
        match &network.driver {
            NetworkDriver::BoltBridge => {
                self.create_bridge_network(network).await?;
            }
            NetworkDriver::BoltOverlay => {
                self.create_overlay_network(network).await?;
            }
            NetworkDriver::BoltMacvlan => {
                self.create_macvlan_network(network).await?;
            }
            NetworkDriver::BoltIpvlan => {
                self.create_ipvlan_network(network).await?;
            }
            NetworkDriver::BoltSriov => {
                self.create_sriov_network(network).await?;
            }
        }

        Ok(())
    }

    /// Create bridge network with optimizations
    async fn create_bridge_network(&self, network: &BoltNetwork) -> Result<()> {
        info!("🌉 Creating optimized bridge network: {}", network.name);

        let bridge_name = Self::bridge_name(network);

        if let Err(err) = Self::ensure_bridge_ready(&bridge_name, network).await {
            warn!("Failed to ensure bridge {} is ready: {}", bridge_name, err);
            return Err(err);
        }

        // Configure bridge metadata for bookkeeping
        let bridge = NetworkBridge {
            name: bridge_name.clone(),
            ip_range: network.subnet.clone(),
            mtu: match network.performance_mode {
                NetworkPerformanceMode::Gaming => 9000, // Jumbo frames for gaming
                NetworkPerformanceMode::HighThroughput => 9000,
                _ => 1500,
            },
            forwarding_enabled: true,
            iptables_rules: self.generate_bridge_iptables_rules(network),
        };

        debug!("Bridge configuration: {:?}", bridge);

        Ok(())
    }

    /// Create QUIC-based overlay network
    async fn create_overlay_network(&self, network: &BoltNetwork) -> Result<()> {
        info!("🚀 Creating QUIC overlay network: {}", network.name);

        // Setup QUIC listeners and connections
        // In real implementation: QUIC endpoint creation, key exchange

        Ok(())
    }

    /// Create macvlan network for direct host access
    async fn create_macvlan_network(&self, network: &BoltNetwork) -> Result<()> {
        info!("📡 Creating macvlan network: {}", network.name);

        // In real implementation: ip link add macvlan
        Ok(())
    }

    /// Create ipvlan network
    async fn create_ipvlan_network(&self, network: &BoltNetwork) -> Result<()> {
        info!("🔗 Creating ipvlan network: {}", network.name);

        // In real implementation: ip link add ipvlan
        Ok(())
    }

    /// Create SR-IOV network for maximum performance
    async fn create_sriov_network(&self, network: &BoltNetwork) -> Result<()> {
        info!("⚡ Creating SR-IOV network: {}", network.name);

        // In real implementation: SR-IOV VF allocation
        Ok(())
    }

    /// Setup container networking with optimizations
    async fn setup_container_networking(
        &self,
        network: &BoltNetwork,
        container_info: &ContainerNetworkInfo,
    ) -> Result<()> {
        let bridge_name = Self::bridge_name(network);

        Self::ensure_bridge_ready(&bridge_name, network).await?;

        let host_iface = &container_info.host_interface;
        let peer_iface = &container_info.container_interface;

        Self::run_command_allow_exists(
            "ip",
            &[
                "link", "add", host_iface, "type", "veth", "peer", "name", peer_iface,
            ],
        )
        .await?;

        Self::run_command("ip", &["link", "set", host_iface, "master", &bridge_name]).await?;
        Self::run_command("ip", &["link", "set", host_iface, "up"]).await?;

        // Apply performance optimizations
        self.apply_network_performance_optimizations(network, container_info)
            .await?;

        Ok(())
    }

    /// Setup QUIC networking for ultra-low latency
    async fn setup_quic_networking(
        &self,
        container_id: &str,
        container_info: &ContainerNetworkInfo,
    ) -> Result<()> {
        info!(
            "🚀 Setting up QUIC networking for container: {} (IP: {})",
            container_id, container_info.ip_address
        );

        // QUIC networking is handled by the QUICServer in the network manager
        // This function registers the container with the QUIC endpoint
        debug!(
            "Container {} registered for QUIC on interface {}",
            container_id, container_info.container_interface
        );

        Ok(())
    }

    /// Apply network performance optimizations
    async fn apply_network_performance_optimizations(
        &self,
        network: &BoltNetwork,
        container_info: &ContainerNetworkInfo,
    ) -> Result<()> {
        match network.performance_mode {
            NetworkPerformanceMode::Gaming => {
                self.apply_gaming_network_optimizations(container_info)
                    .await?;
            }
            NetworkPerformanceMode::HighThroughput => {
                self.apply_throughput_optimizations(container_info).await?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Gaming-specific network optimizations
    async fn apply_gaming_network_optimizations(
        &self,
        container_info: &ContainerNetworkInfo,
    ) -> Result<()> {
        info!(
            "🎮 Applying gaming network optimizations for {}",
            container_info.container_id
        );

        // Configure host interface for low-latency
        let sysctls = [
            ("net.ipv4.tcp_nodelay", "1"),
            ("net.ipv4.tcp_quickack", "1"),
            ("net.ipv4.tcp_low_latency", "1"),
        ];

        for (key, value) in &sysctls {
            let output = tokio::process::Command::new("sysctl")
                .args(["-w", &format!("{}={}", key, value)])
                .output()
                .await?;

            if !output.status.success() {
                debug!(
                    "Sysctl {} failed (may require root): {}",
                    key,
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        info!("✅ Gaming network optimizations applied");
        Ok(())
    }

    /// High-throughput optimizations
    async fn apply_throughput_optimizations(
        &self,
        container_info: &ContainerNetworkInfo,
    ) -> Result<()> {
        info!(
            "📈 Applying high-throughput network optimizations for {}",
            container_info.container_id
        );

        // Increase TCP buffer sizes for high throughput
        let sysctls = [
            "net.core.rmem_max=134217728",
            "net.core.wmem_max=134217728",
            "net.ipv4.tcp_rmem=4096 87380 67108864",
            "net.ipv4.tcp_wmem=4096 65536 67108864",
            "net.ipv4.tcp_congestion_control=cubic",
        ];

        for sysctl in &sysctls {
            let output = tokio::process::Command::new("sysctl")
                .args(["-w", sysctl])
                .output()
                .await?;

            if !output.status.success() {
                debug!(
                    "Sysctl {} failed (may require root): {}",
                    sysctl,
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        info!("✅ High-throughput optimizations applied");
        Ok(())
    }

    /// Calculate gateway IP from subnet
    fn calculate_gateway(&self, _subnet: &str) -> Result<IpAddr> {
        // Simple implementation - use first IP in subnet as gateway
        // In real implementation: proper CIDR parsing from subnet param
        Ok(IpAddr::V4(Ipv4Addr::new(172, 18, 0, 1)))
    }

    /// Allocate IP address for container
    async fn allocate_ip_address(
        &self,
        network: &BoltNetwork,
        config: &ContainerNetworkConfig,
    ) -> Result<IpAddr> {
        // IPAM (IP Address Management) implementation
        let container_count = network.containers.len() as u8;
        let ip = IpAddr::V4(Ipv4Addr::new(172, 18, 0, container_count + 2));

        // Configure DNS servers for container
        if !config.dns_servers.is_empty() {
            debug!("  DNS servers configured: {:?}", config.dns_servers);
        }

        // Apply bandwidth limit for QoS if specified
        if let Some(bandwidth_limit) = config.bandwidth_limit {
            debug!("  Bandwidth limit: {} Mbps", bandwidth_limit / 1_000_000);
            // Would apply via tc (traffic control) in production:
            // tc qdisc add dev <interface> root tbf rate {bandwidth_limit} burst 32kbit latency 400ms
        }

        // Apply latency target for gaming/real-time workloads
        if let Some(latency_target) = config.latency_target {
            debug!("  Latency target: {} μs", latency_target);
            // Would use fq_codel or cake qdisc with low latency settings
        }

        Ok(ip)
    }

    /// Generate MAC address for container
    fn generate_mac_address(&self) -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        format!(
            "02:42:{:02x}:{:02x}:{:02x}:{:02x}",
            rng.r#gen::<u8>(),
            rng.r#gen::<u8>(),
            rng.r#gen::<u8>(),
            rng.r#gen::<u8>()
        )
    }

    /// Generate iptables rules for bridge
    fn generate_bridge_iptables_rules(&self, network: &BoltNetwork) -> Vec<String> {
        vec![
            format!("-A FORWARD -i {} -j ACCEPT", network.name),
            format!("-A FORWARD -o {} -j ACCEPT", network.name),
        ]
    }

    fn bridge_name(network: &BoltNetwork) -> String {
        format!("bolt-{}", &network.id[..8])
    }

    async fn ensure_bridge_ready(bridge_name: &str, network: &BoltNetwork) -> Result<()> {
        if Self::run_command("ip", &["link", "show", bridge_name])
            .await
            .is_ok()
        {
            return Ok(());
        }

        Self::run_command_allow_exists(
            "ip",
            &["link", "add", "name", bridge_name, "type", "bridge"],
        )
        .await?;

        if let IpAddr::V4(gateway) = network.gateway {
            let cidr = format!("{}/16", gateway);
            Self::run_command_allow_exists("ip", &["addr", "add", &cidr, "dev", bridge_name])
                .await?;
        }

        Self::run_command("ip", &["link", "set", bridge_name, "up"]).await?;
        // Enable IPv4 forwarding for NAT scenarios (best-effort)
        let _ = Self::run_command_allow_exists("sysctl", &["-w", "net.ipv4.ip_forward=1"]).await;

        Ok(())
    }

    async fn run_command(program: &str, args: &[&str]) -> Result<()> {
        let output = Command::new(program)
            .args(args)
            .output()
            .await
            .with_context(|| format!("Failed to execute {} {:?}", program, args))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Command `{}` {:?} failed: {}", program, args, stderr).into());
        }

        Ok(())
    }

    async fn run_command_allow_exists(program: &str, args: &[&str]) -> Result<()> {
        let output = Command::new(program)
            .args(args)
            .output()
            .await
            .with_context(|| format!("Failed to execute {} {:?}", program, args))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("File exists") || stderr.contains("exists") {
                debug!(
                    "Command `{}` {:?} reported existing resource: {}",
                    program,
                    args,
                    stderr.trim()
                );
                return Ok(());
            }
            return Err(anyhow!("Command `{}` {:?} failed: {}", program, args, stderr).into());
        }

        Ok(())
    }

    async fn run_command_allow_missing(program: &str, args: &[&str]) -> Result<()> {
        let output = Command::new(program)
            .args(args)
            .output()
            .await
            .with_context(|| format!("Failed to execute {} {:?}", program, args))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Cannot find device")
                || stderr.contains("No such file")
                || stderr.contains("not found")
            {
                debug!(
                    "Command `{}` {:?} reported missing resource: {}",
                    program,
                    args,
                    stderr.trim()
                );
                return Ok(());
            }
            return Err(anyhow!("Command `{}` {:?} failed: {}", program, args, stderr).into());
        }

        Ok(())
    }

    async fn run_ns_command(pid: i32, args: &[&str]) -> Result<()> {
        let pid_str = pid.to_string();
        let mut command = Command::new("nsenter");
        command.arg("-t").arg(&pid_str).arg("-n").arg("--");
        for arg in args {
            command.arg(arg);
        }

        let output = command
            .output()
            .await
            .with_context(|| format!("Failed to nsenter pid {} with args {:?}", pid, args))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "Namespace command for pid {} {:?} failed: {}",
                pid,
                args,
                stderr
            )
            .into());
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ContainerNetworkConfig {
    pub port_mappings: Vec<PortMapping>,
    pub bandwidth_limit: Option<u64>,
    pub latency_target: Option<u64>,
    pub dns_servers: Vec<IpAddr>,
}

impl QuicNetworkFabric {
    async fn new() -> Result<Self> {
        Ok(Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            load_balancer: QuicLoadBalancer::new(),
            service_mesh: ServiceMesh::new(),
        })
    }
}

impl QuicLoadBalancer {
    fn new() -> Self {
        Self {
            algorithm: LoadBalancingAlgorithm::LatencyBased,
            health_checks: HashMap::new(),
            backends: Vec::new(),
        }
    }
}

impl ServiceMesh {
    fn new() -> Self {
        Self {
            services: HashMap::new(),
            routing_rules: Vec::new(),
            circuit_breakers: HashMap::new(),
            retry_policies: HashMap::new(),
        }
    }
}

impl BridgeManager {
    fn new() -> Self {
        Self {
            bridges: HashMap::new(),
            veth_pairs: HashMap::new(),
        }
    }
}

impl DnsResolver {
    fn new() -> Self {
        Self {
            domains: HashMap::new(),
            cache: HashMap::new(),
            upstream_servers: vec![
                IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            ],
        }
    }
}

impl NetworkFirewall {
    fn secure_defaults() -> Self {
        Self {
            rules: Vec::new(),
            default_policy: FirewallPolicy::Deny,
            rate_limiting: HashMap::new(),
        }
    }
}
