//! Gateway configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Transport type (stdio, websocket, http)
    #[serde(default = "default_transport")]
    pub transport: String,

    /// Address to bind to
    #[serde(default = "default_address")]
    pub address: String,

    /// Port to bind to
    #[serde(default = "default_port")]
    pub port: u16,

    /// Path to catalog file
    #[serde(default = "default_catalog_path")]
    pub catalog_path: PathBuf,

    /// Enabled servers (empty = all)
    #[serde(default)]
    pub enabled_servers: Vec<String>,

    /// Enabled tools (format: "server:tool" or "server:*")
    #[serde(default)]
    pub enabled_tools: Vec<String>,

    /// Secret sources
    #[serde(default = "default_secret_sources")]
    pub secret_sources: Vec<String>,

    /// Watch for config changes
    #[serde(default = "default_true")]
    pub watch: bool,

    /// Enable verbose logging
    #[serde(default)]
    pub verbose: bool,

    /// Keep stopped containers
    #[serde(default)]
    pub keep_containers: bool,

    /// CPU limit per server
    #[serde(default = "default_cpus")]
    pub cpus: u32,

    /// Memory limit per server
    #[serde(default = "default_memory")]
    pub memory: String,

    /// Block network access
    #[serde(default)]
    pub block_network: bool,

    /// Block secrets
    #[serde(default = "default_true")]
    pub block_secrets: bool,

    /// Log tool calls
    #[serde(default = "default_true")]
    pub log_calls: bool,

    /// Verify signatures
    #[serde(default)]
    pub verify_signatures: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            transport: default_transport(),
            address: default_address(),
            port: default_port(),
            catalog_path: default_catalog_path(),
            enabled_servers: Vec::new(),
            enabled_tools: Vec::new(),
            secret_sources: default_secret_sources(),
            watch: true,
            verbose: false,
            keep_containers: false,
            cpus: default_cpus(),
            memory: default_memory(),
            block_network: false,
            block_secrets: true,
            log_calls: true,
            verify_signatures: false,
        }
    }
}

fn default_transport() -> String {
    "websocket".to_string()
}

fn default_address() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    7331
}

fn default_catalog_path() -> PathBuf {
    PathBuf::from("~/.config/bolt/mcp-catalog.toml")
}

fn default_secret_sources() -> Vec<String> {
    vec!["docker-desktop".to_string(), ".env".to_string()]
}

fn default_cpus() -> u32 {
    1
}

fn default_memory() -> String {
    "2Gb".to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GatewayConfig::default();
        assert_eq!(config.transport, "websocket");
        assert_eq!(config.port, 7331);
        assert!(config.watch);
    }

    #[test]
    fn test_parse_config() {
        let toml = r#"
            transport = "stdio"
            port = 8080
            watch = false
            enabled_servers = ["server1", "server2"]
        "#;

        let config: GatewayConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.transport, "stdio");
        assert_eq!(config.port, 8080);
        assert!(!config.watch);
        assert_eq!(config.enabled_servers.len(), 2);
    }
}
