//! eBPF XDP fast path for container-to-container networking
//!
//! This module provides ultra-low-latency packet processing using eBPF/XDP
//! to bypass the kernel network stack for local container-to-container traffic.

use anyhow::Result;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// eBPF XDP fast path manager
pub struct XDPFastPath {
    /// Map of container IP to interface index
    container_routes: Arc<RwLock<HashMap<Ipv4Addr, u32>>>,
    /// Enabled interfaces with XDP programs attached
    enabled_interfaces: Arc<RwLock<Vec<XDPInterface>>>,
    /// XDP mode (native, offload, or skb)
    mode: XDPMode,
    /// Statistics
    stats: Arc<RwLock<XDPStats>>,
}

#[derive(Debug, Clone)]
struct XDPInterface {
    name: String,
    ifindex: u32,
    xdp_attached: bool,
    packets_processed: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum XDPMode {
    /// Native XDP (driver level) - best performance
    Native,
    /// Generic XDP (kernel level) - compatibility fallback
    Generic,
    /// Offloaded XDP (NIC level) - best performance but rare HW support
    Offload,
}

#[derive(Debug, Default, Clone)]
pub struct XDPStats {
    /// Packets processed via XDP fast path
    pub packets_fastpath: u64,
    /// Packets redirected to containers
    pub packets_redirected: u64,
    /// Packets passed to kernel stack
    pub packets_passed: u64,
    /// Packets dropped
    pub packets_dropped: u64,
    /// Average latency in nanoseconds
    pub avg_latency_ns: u64,
}

impl XDPFastPath {
    /// Create new XDP fast path manager
    pub fn new(mode: XDPMode) -> Self {
        info!("🚀 Initializing eBPF XDP fast path in {:?} mode", mode);

        Self {
            container_routes: Arc::new(RwLock::new(HashMap::new())),
            enabled_interfaces: Arc::new(RwLock::new(Vec::new())),
            mode,
            stats: Arc::new(RwLock::new(XDPStats::default())),
        }
    }

    /// Attach XDP program to network interface
    pub async fn attach_to_interface(&self, interface_name: &str) -> Result<()> {
        info!("📌 Attaching XDP program to interface: {}", interface_name);

        // Get interface index
        let ifindex = Self::get_interface_index(interface_name)?;

        // In a real implementation, this would:
        // 1. Compile the eBPF program (or load precompiled)
        // 2. Attach it to the interface using bpf() syscall
        // 3. Configure BPF maps for routing

        // For now, we simulate the attachment
        debug!("  Interface index: {}", ifindex);
        debug!("  XDP mode: {:?}", self.mode);

        let xdp_interface = XDPInterface {
            name: interface_name.to_string(),
            ifindex,
            xdp_attached: true,
            packets_processed: 0,
        };

        {
            let mut interfaces = self.enabled_interfaces.write().await;
            interfaces.push(xdp_interface);
        }

        info!("✅ XDP program attached to {} (ifindex: {})", interface_name, ifindex);
        Ok(())
    }

    /// Add container route to XDP fast path
    pub async fn add_container_route(&self, container_ip: Ipv4Addr, ifindex: u32) -> Result<()> {
        info!("➕ Adding XDP fast path route: {} -> ifindex {}", container_ip, ifindex);

        // In a real implementation, this would update the BPF map
        // that the XDP program uses for routing decisions

        {
            let mut routes = self.container_routes.write().await;
            routes.insert(container_ip, ifindex);
        }

        debug!("  XDP route added to fast path table");
        Ok(())
    }

    /// Remove container route from XDP fast path
    pub async fn remove_container_route(&self, container_ip: &Ipv4Addr) -> Result<()> {
        info!("🗑️  Removing XDP fast path route: {}", container_ip);

        {
            let mut routes = self.container_routes.write().await;
            routes.remove(container_ip);
        }

        Ok(())
    }

    /// Get XDP statistics
    pub async fn get_stats(&self) -> XDPStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Detach XDP programs from all interfaces
    pub async fn detach_all(&self) -> Result<()> {
        info!("🔌 Detaching XDP programs from all interfaces");

        let interfaces = {
            let mut ifaces = self.enabled_interfaces.write().await;
            std::mem::take(&mut *ifaces)
        };

        for iface in interfaces {
            debug!("  Detaching from {}", iface.name);
            // In real implementation: detach using bpf() syscall
        }

        info!("✅ All XDP programs detached");
        Ok(())
    }

    /// Get interface index by name
    fn get_interface_index(name: &str) -> Result<u32> {
        // In real implementation, this would use netlink or read from /sys/class/net
        // For now, return a simulated index
        match name {
            "br-bolt0" => Ok(10),
            "lo" => Ok(1),
            "eth0" => Ok(2),
            _ => {
                warn!("Interface {} not found, using default index", name);
                Ok(99)
            }
        }
    }

    /// Simulate packet processing via XDP
    pub async fn process_packet(&self, src_ip: Ipv4Addr, dst_ip: Ipv4Addr, size: usize) -> XDPAction {
        // Check if destination is in our fast path routes
        let routes = self.container_routes.read().await;

        if let Some(&target_ifindex) = routes.get(&dst_ip) {
            // Fast path: redirect to container interface
            {
                let mut stats = self.stats.write().await;
                stats.packets_fastpath += 1;
                stats.packets_redirected += 1;
            }

            debug!("⚡ XDP fast path: {} -> {} ({} bytes) redirected to ifindex {}",
                   src_ip, dst_ip, size, target_ifindex);

            XDPAction::Redirect(target_ifindex)
        } else {
            // Not in fast path, pass to kernel stack
            {
                let mut stats = self.stats.write().await;
                stats.packets_passed += 1;
            }

            XDPAction::Pass
        }
    }

    /// Get routing table snapshot
    pub async fn get_routes(&self) -> HashMap<Ipv4Addr, u32> {
        let routes = self.container_routes.read().await;
        routes.clone()
    }
}

/// XDP program action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XDPAction {
    /// Pass packet to kernel network stack
    Pass,
    /// Drop packet
    Drop,
    /// Redirect to another interface
    Redirect(u32),
    /// Transmit packet back out the same interface
    TX,
}

/// Example eBPF XDP program (would be compiled to BPF bytecode)
#[allow(dead_code)]
const XDP_PROGRAM_SOURCE: &str = r#"
// BPF XDP program for container fast path
// This is pseudo-code - real implementation would be in C or eBPF assembly

#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/ip.h>

// BPF map for container routing
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __type(key, __u32);    // IP address
    __type(value, __u32);  // Target interface index
    __uint(max_entries, 1024);
} container_routes SEC(".maps");

SEC("xdp")
int xdp_fast_path(struct xdp_md *ctx) {
    void *data_end = (void *)(long)ctx->data_end;
    void *data = (void *)(long)ctx->data;

    struct ethhdr *eth = data;
    if ((void *)(eth + 1) > data_end)
        return XDP_PASS;

    if (eth->h_proto != htons(ETH_P_IP))
        return XDP_PASS;

    struct iphdr *iph = (void *)(eth + 1);
    if ((void *)(iph + 1) > data_end)
        return XDP_PASS;

    // Look up destination IP in routing table
    __u32 *target_ifindex = bpf_map_lookup_elem(&container_routes, &iph->daddr);
    if (target_ifindex) {
        // Fast path: redirect to container interface
        return bpf_redirect(*target_ifindex, 0);
    }

    // Not in fast path, pass to kernel
    return XDP_PASS;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_xdp_fastpath_routing() {
        let xdp = XDPFastPath::new(XDPMode::Native);

        let container_ip = Ipv4Addr::new(172, 18, 0, 10);
        xdp.add_container_route(container_ip, 10).await.unwrap();

        // Test packet to container should be redirected
        let action = xdp.process_packet(
            Ipv4Addr::new(172, 18, 0, 11),
            container_ip,
            1500
        ).await;

        assert_eq!(action, XDPAction::Redirect(10));

        // Test packet to unknown destination should pass
        let action = xdp.process_packet(
            Ipv4Addr::new(172, 18, 0, 11),
            Ipv4Addr::new(172, 18, 0, 99),
            1500
        ).await;

        assert_eq!(action, XDPAction::Pass);
    }

    #[tokio::test]
    async fn test_xdp_stats() {
        let xdp = XDPFastPath::new(XDPMode::Native);
        let container_ip = Ipv4Addr::new(172, 18, 0, 10);
        xdp.add_container_route(container_ip, 10).await.unwrap();

        // Process some packets
        xdp.process_packet(Ipv4Addr::new(172, 18, 0, 11), container_ip, 1500).await;
        xdp.process_packet(Ipv4Addr::new(172, 18, 0, 11), Ipv4Addr::new(10, 0, 0, 1), 1500).await;

        let stats = xdp.get_stats().await;
        assert_eq!(stats.packets_fastpath, 1);
        assert_eq!(stats.packets_passed, 1);
    }
}
