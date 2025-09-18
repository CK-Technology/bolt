use super::{OptimizationStep, ToOptimizationSteps};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkOptimizations {
    pub priority: Option<NetworkPriority>,
    pub latency_optimization: Option<bool>,
    pub packet_batching: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkPriority {
    Gaming,
    Streaming,
    Background,
    Critical,
}

impl ToOptimizationSteps for NetworkOptimizations {
    fn to_steps(&self) -> Vec<OptimizationStep> {
        let mut steps = Vec::new();

        if let Some(priority) = &self.priority {
            steps.push(OptimizationStep {
                optimization_type: format!("network_priority_{:?}", priority).to_lowercase(),
                priority: 60,
                description: format!("Set network priority to {:?}", priority),
            });
        }

        if let Some(latency_opt) = self.latency_optimization {
            steps.push(OptimizationStep {
                optimization_type: format!("network_latency_optimization_{}", latency_opt),
                priority: 65,
                description: format!("Set network latency optimization to {}", latency_opt),
            });
        }

        if let Some(packet_batching) = self.packet_batching {
            steps.push(OptimizationStep {
                optimization_type: format!("network_packet_batching_{}", packet_batching),
                priority: 70,
                description: format!("Set network packet batching to {}", packet_batching),
            });
        }

        steps
    }
}

impl NetworkPriority {
    pub fn to_qos_class(&self) -> u32 {
        match self {
            NetworkPriority::Critical => 0, // Highest priority
            NetworkPriority::Gaming => 1,
            NetworkPriority::Streaming => 2,
            NetworkPriority::Background => 3, // Lowest priority
        }
    }

    pub fn to_dscp_value(&self) -> u8 {
        match self {
            NetworkPriority::Critical => 46,   // EF (Expedited Forwarding)
            NetworkPriority::Gaming => 34,     // AF41 (Assured Forwarding)
            NetworkPriority::Streaming => 26,  // AF31
            NetworkPriority::Background => 10, // AF11
        }
    }
}
