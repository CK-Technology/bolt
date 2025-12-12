//! Network statistics tool
//!
//! Provides network metrics and statistics for containers

use crate::mcp::{McpError, Result, tools::McpTool};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;

/// Network statistics tool
///
/// Provides network metrics including:
/// - Interface statistics
/// - Bytes transmitted/received
/// - Packet counts
/// - Error rates
pub struct NetworkTool;

impl NetworkTool {
    /// Create a new network statistics tool
    pub fn new() -> Self {
        Self
    }

    async fn get_interface_stats(&self, interface: Option<String>) -> Result<Vec<InterfaceStats>> {
        tracing::info!("Querying network interface statistics");

        let net_dev = fs::read_to_string("/proc/net/dev")
            .map_err(|e| McpError::ToolExecution(format!("Failed to read /proc/net/dev: {}", e)))?;

        let mut stats = Vec::new();

        for line in net_dev.lines().skip(2) {
            // Skip header lines
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 17 {
                continue;
            }

            let iface_name = parts[0].trim_end_matches(':').to_string();

            // If specific interface requested, skip others
            if let Some(ref target) = interface {
                if &iface_name != target {
                    continue;
                }
            }

            stats.push(InterfaceStats {
                interface: iface_name,
                rx_bytes: parts[1].parse().unwrap_or(0),
                rx_packets: parts[2].parse().unwrap_or(0),
                rx_errors: parts[3].parse().unwrap_or(0),
                rx_dropped: parts[4].parse().unwrap_or(0),
                tx_bytes: parts[9].parse().unwrap_or(0),
                tx_packets: parts[10].parse().unwrap_or(0),
                tx_errors: parts[11].parse().unwrap_or(0),
                tx_dropped: parts[12].parse().unwrap_or(0),
            });
        }

        if stats.is_empty() && interface.is_some() {
            return Err(McpError::ToolExecution(format!(
                "Interface '{}' not found",
                interface.unwrap()
            )));
        }

        Ok(stats)
    }

    async fn get_total_stats(&self) -> Result<NetworkSummary> {
        let all_stats = self.get_interface_stats(None).await?;

        let total_rx_bytes: u64 = all_stats.iter().map(|s| s.rx_bytes).sum();
        let total_tx_bytes: u64 = all_stats.iter().map(|s| s.tx_bytes).sum();
        let total_rx_packets: u64 = all_stats.iter().map(|s| s.rx_packets).sum();
        let total_tx_packets: u64 = all_stats.iter().map(|s| s.tx_packets).sum();

        Ok(NetworkSummary {
            interface_count: all_stats.len(),
            total_rx_bytes,
            total_tx_bytes,
            total_rx_packets,
            total_tx_packets,
            total_rx_mb: total_rx_bytes as f64 / 1024.0 / 1024.0,
            total_tx_mb: total_tx_bytes as f64 / 1024.0 / 1024.0,
        })
    }
}

impl Default for NetworkTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation")]
enum NetworkInput {
    #[serde(rename = "stats")]
    Stats { interface: Option<String> },
    #[serde(rename = "summary")]
    Summary,
}

#[derive(Debug, Serialize)]
struct InterfaceStats {
    interface: String,
    rx_bytes: u64,
    rx_packets: u64,
    rx_errors: u64,
    rx_dropped: u64,
    tx_bytes: u64,
    tx_packets: u64,
    tx_errors: u64,
    tx_dropped: u64,
}

#[derive(Debug, Serialize)]
struct NetworkSummary {
    interface_count: usize,
    total_rx_bytes: u64,
    total_tx_bytes: u64,
    total_rx_packets: u64,
    total_tx_packets: u64,
    total_rx_mb: f64,
    total_tx_mb: f64,
}

impl McpTool for NetworkTool {
    fn name(&self) -> &str {
        "bolt_network_stats"
    }

    fn description(&self) -> &str {
        "Get network interface statistics and metrics"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "oneOf": [
                {
                    "properties": {
                        "operation": { "const": "stats" },
                        "interface": {
                            "type": "string",
                            "description": "Specific interface name (e.g., 'eth0'), or omit for all"
                        }
                    },
                    "required": ["operation"]
                },
                {
                    "properties": {
                        "operation": { "const": "summary" }
                    },
                    "required": ["operation"]
                }
            ]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let args: NetworkInput = serde_json::from_value(input)?;

        match args {
            NetworkInput::Stats { interface } => {
                let stats = self.get_interface_stats(interface).await?;
                Ok(json!({
                    "operation": "stats",
                    "interfaces": stats
                }))
            }
            NetworkInput::Summary => {
                let summary = self.get_total_stats().await?;
                Ok(serde_json::to_value(summary)?)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_creation() {
        let tool = NetworkTool::new();
        assert_eq!(tool.name(), "bolt_network_stats");
    }

    #[tokio::test]
    async fn test_get_stats() {
        let tool = NetworkTool::new();

        // This test will only work on Linux systems
        #[cfg(target_os = "linux")]
        {
            let result = tool.get_interface_stats(None).await;
            if result.is_ok() {
                let stats = result.unwrap();
                assert!(!stats.is_empty());
            }
        }
    }
}
