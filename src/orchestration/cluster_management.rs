use crate::{BoltError, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use super::{
    ClusterNode, ClusterState, NodeRole, NodeSpecialization, NodeStatus, ResourceCapacity,
    ResourceUtilization,
};

/// Enterprise-Grade Cluster Management System
/// Provides intelligent node management and cluster optimization
#[derive(Debug)]
pub struct ClusterManager {
    cluster_state: Arc<RwLock<ClusterState>>,
    config: ClusterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Maximum number of nodes in the cluster
    pub max_nodes: u32,
    /// Minimum number of nodes to maintain
    pub min_nodes: u32,
    /// Auto-scaling enabled
    pub auto_scaling: bool,
    /// Node health check interval (seconds)
    pub health_check_interval: u32,
    /// Enable intelligent node placement
    pub intelligent_placement: bool,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            max_nodes: 1000,
            min_nodes: 3,
            auto_scaling: true,
            health_check_interval: 30,
            intelligent_placement: true,
        }
    }
}

impl ClusterManager {
    pub async fn new(config: ClusterConfig) -> Result<Self> {
        info!("🏗️ Initializing Enterprise Cluster Manager");
        info!("   Max Nodes: {}", config.max_nodes);
        info!("   Min Nodes: {}", config.min_nodes);
        info!("   Auto-scaling: {}", config.auto_scaling);
        info!(
            "   Health Check Interval: {}s",
            config.health_check_interval
        );

        let cluster_state = Arc::new(RwLock::new(ClusterState {
            nodes: Vec::new(),
            services: Vec::new(),
            total_capacity: ResourceCapacity {
                cpu_cores: 0,
                memory_gb: 0,
                storage_gb: 0,
                network_gbps: 0,
                gpu_count: 0,
                gpu_memory_gb: 0,
            },
            utilization: ResourceUtilization {
                cpu_percent: 0.0,
                memory_percent: 0.0,
                storage_percent: 0.0,
                network_percent: 0.0,
                gpu_percent: 0.0,
            },
            health_score: 100.0,
            last_updated: std::time::Instant::now(),
        }));

        Ok(Self {
            cluster_state,
            config,
        })
    }

    /// Add a new node to the cluster
    pub async fn add_node(&self, node: ClusterNode) -> Result<()> {
        info!("➕ Adding node to cluster: {}", node.name);

        let mut state = self.cluster_state.write().await;

        // Validate node doesn't already exist
        if state.nodes.iter().any(|n| n.id == node.id) {
            return Err(BoltError::InvalidInput(format!(
                "Node with ID {} already exists",
                node.id
            )));
        }

        // Check cluster capacity
        if state.nodes.len() >= self.config.max_nodes as usize {
            return Err(BoltError::ResourceExhausted(
                "Cluster has reached maximum node capacity".to_string(),
            ));
        }

        // Update total capacity
        state.total_capacity.cpu_cores += node.capacity.cpu_cores;
        state.total_capacity.memory_gb += node.capacity.memory_gb;
        state.total_capacity.storage_gb += node.capacity.storage_gb;
        state.total_capacity.network_gbps += node.capacity.network_gbps;
        state.total_capacity.gpu_count += node.capacity.gpu_count;
        state.total_capacity.gpu_memory_gb += node.capacity.gpu_memory_gb;

        state.nodes.push(node);
        state.last_updated = std::time::Instant::now();

        info!("✅ Node added successfully");
        Ok(())
    }

    /// Remove a node from the cluster
    pub async fn remove_node(&self, node_id: &str) -> Result<()> {
        info!("➖ Removing node from cluster: {}", node_id);

        let mut state = self.cluster_state.write().await;

        let node_index = state
            .nodes
            .iter()
            .position(|n| n.id == node_id)
            .ok_or_else(|| BoltError::NotFound(format!("Node {} not found", node_id)))?;

        // Clone the capacity before removing to avoid borrow issues
        let node_capacity = state.nodes[node_index].capacity.clone();

        // Update total capacity
        state.total_capacity.cpu_cores -= node_capacity.cpu_cores;
        state.total_capacity.memory_gb -= node_capacity.memory_gb;
        state.total_capacity.storage_gb -= node_capacity.storage_gb;
        state.total_capacity.network_gbps -= node_capacity.network_gbps;
        state.total_capacity.gpu_count -= node_capacity.gpu_count;
        state.total_capacity.gpu_memory_gb -= node_capacity.gpu_memory_gb;

        state.nodes.remove(node_index);
        state.last_updated = std::time::Instant::now();

        info!("✅ Node removed successfully");
        Ok(())
    }

    /// Update node status
    pub async fn update_node_status(&self, node_id: &str, status: NodeStatus) -> Result<()> {
        let mut state = self.cluster_state.write().await;

        let node = state
            .nodes
            .iter_mut()
            .find(|n| n.id == node_id)
            .ok_or_else(|| BoltError::NotFound(format!("Node {} not found", node_id)))?;

        node.status = status;
        state.last_updated = std::time::Instant::now();

        Ok(())
    }

    /// Get cluster health status
    pub async fn get_cluster_health(&self) -> Result<ClusterHealthReport> {
        let state = self.cluster_state.read().await;

        let total_nodes = state.nodes.len();
        let ready_nodes = state
            .nodes
            .iter()
            .filter(|n| matches!(n.status, NodeStatus::Ready))
            .count();
        let not_ready_nodes = total_nodes - ready_nodes;

        let health_percentage = if total_nodes > 0 {
            (ready_nodes as f64 / total_nodes as f64) * 100.0
        } else {
            0.0
        };

        Ok(ClusterHealthReport {
            health_score: health_percentage,
            total_nodes,
            ready_nodes,
            not_ready_nodes,
            total_capacity: state.total_capacity.clone(),
            utilization: state.utilization.clone(),
            last_updated: state.last_updated,
        })
    }

    /// Get nodes by role
    pub async fn get_nodes_by_role(&self, role: NodeRole) -> Result<Vec<ClusterNode>> {
        let state = self.cluster_state.read().await;

        let nodes = state
            .nodes
            .iter()
            .filter(|n| n.role == role)
            .cloned()
            .collect();

        Ok(nodes)
    }

    /// Find best nodes for a specific workload type
    pub async fn find_optimal_nodes(
        &self,
        specialization: NodeSpecialization,
        count: usize,
    ) -> Result<Vec<ClusterNode>> {
        let state = self.cluster_state.read().await;

        let mut suitable_nodes: Vec<_> = state
            .nodes
            .iter()
            .filter(|n| {
                n.specializations.contains(&specialization) && matches!(n.status, NodeStatus::Ready)
            })
            .cloned()
            .collect();

        // Sort by utilization (prefer less utilized nodes)
        suitable_nodes.sort_by(|a, b| {
            let a_util = (a.utilization.cpu_percent + a.utilization.memory_percent) / 2.0;
            let b_util = (b.utilization.cpu_percent + b.utilization.memory_percent) / 2.0;
            a_util.partial_cmp(&b_util).unwrap()
        });

        Ok(suitable_nodes.into_iter().take(count).collect())
    }

    /// Drain a node for maintenance
    pub async fn drain_node(&self, node_id: &str) -> Result<()> {
        info!("🚰 Draining node for maintenance: {}", node_id);

        self.update_node_status(node_id, NodeStatus::Draining)
            .await?;

        // In a real implementation, this would:
        // 1. Stop scheduling new workloads on the node
        // 2. Gracefully migrate existing workloads
        // 3. Wait for all workloads to be moved

        info!("✅ Node drained successfully");
        Ok(())
    }

    /// Cordon a node (prevent new workloads)
    pub async fn cordon_node(&self, node_id: &str) -> Result<()> {
        info!("🚫 Cordoning node: {}", node_id);
        self.update_node_status(node_id, NodeStatus::Cordoned).await
    }

    /// Uncordon a node (allow new workloads)
    pub async fn uncordon_node(&self, node_id: &str) -> Result<()> {
        info!("✅ Uncordoning node: {}", node_id);
        self.update_node_status(node_id, NodeStatus::Ready).await
    }
}

#[derive(Debug, Clone)]
pub struct ClusterHealthReport {
    pub health_score: f64,
    pub total_nodes: usize,
    pub ready_nodes: usize,
    pub not_ready_nodes: usize,
    pub total_capacity: ResourceCapacity,
    pub utilization: ResourceUtilization,
    pub last_updated: std::time::Instant,
}
