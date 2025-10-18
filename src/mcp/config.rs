//! MCP configuration parsing from Boltfile
//!
//! This module handles parsing the `[mcp]` section from Boltfile.toml

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// MCP server configuration from Boltfile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Enable MCP server
    #[serde(default)]
    pub enabled: bool,

    /// Transport type: "stdio", "websocket", "http"
    #[serde(default = "default_transport")]
    pub transport: String,

    /// Address to bind to (for websocket/http)
    #[serde(default = "default_address")]
    pub address: String,

    /// Port to bind to (for websocket/http)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Policy configuration
    #[serde(default)]
    pub policy: PolicyConfig,

    /// Tool configuration
    #[serde(default)]
    pub tools: ToolsConfig,

    /// Observability configuration
    #[serde(default)]
    pub observability: ObservabilityConfig,

    /// Omen AI Router configuration (optional)
    #[cfg(feature = "omen")]
    #[serde(default)]
    pub omen: Option<crate::mcp::omen_integration::OmenConfig>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            transport: default_transport(),
            address: default_address(),
            port: default_port(),
            policy: PolicyConfig::default(),
            tools: ToolsConfig::default(),
            observability: ObservabilityConfig::default(),
            #[cfg(feature = "omen")]
            omen: None,
        }
    }
}

/// Policy engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// Policy mode: "allow-all", "deny-all", "consent-required"
    #[serde(default = "default_policy_mode")]
    pub mode: String,

    /// Tools requiring explicit consent
    #[serde(default)]
    pub require_consent: Vec<String>,

    /// Enable audit logging for all operations
    #[serde(default)]
    pub audit_all: bool,

    /// Path to audit log file
    #[serde(default)]
    pub audit_log: Option<PathBuf>,

    /// Redact secrets in audit logs
    #[serde(default = "default_true")]
    pub redact_secrets: bool,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            mode: default_policy_mode(),
            require_consent: vec![
                "shell.execute".to_string(),
                "fs.write".to_string(),
            ],
            audit_all: true,
            audit_log: None,
            redact_secrets: true,
        }
    }
}

/// Tool-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    /// GPU stats tool configuration
    #[serde(default)]
    pub gpu_stats: ToolConfig,

    /// Filesystem tool configuration
    #[serde(default)]
    pub filesystem: FilesystemToolConfig,

    /// Shell execution tool configuration
    #[serde(default)]
    pub shell: ShellToolConfig,

    /// Process management tool configuration
    #[serde(default)]
    pub process: ToolConfig,

    /// Network stats tool configuration
    #[serde(default)]
    pub network: ToolConfig,
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            gpu_stats: ToolConfig { enabled: true },
            filesystem: FilesystemToolConfig::default(),
            shell: ShellToolConfig::default(),
            process: ToolConfig { enabled: true },
            network: ToolConfig { enabled: true },
        }
    }
}

/// Basic tool configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Filesystem tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemToolConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Root directory for filesystem access
    #[serde(default = "default_fs_root")]
    pub root: PathBuf,
}

impl Default for FilesystemToolConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            root: default_fs_root(),
        }
    }
}

/// Shell tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellToolConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Allowed commands (empty = all allowed)
    #[serde(default)]
    pub allowed_commands: Vec<String>,
}

impl Default for ShellToolConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_commands: vec![
                "ls".to_string(),
                "ps".to_string(),
                "nvidia-smi".to_string(),
                "cat".to_string(),
                "grep".to_string(),
            ],
        }
    }
}

/// Observability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Enable Prometheus metrics
    #[serde(default)]
    pub enable_metrics: bool,

    /// Metrics endpoint port
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,

    /// Enable tracing
    #[serde(default)]
    pub enable_tracing: bool,

    /// Tracing endpoint URL
    #[serde(default)]
    pub tracing_endpoint: Option<String>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            metrics_port: default_metrics_port(),
            enable_tracing: false,
            tracing_endpoint: None,
        }
    }
}

// Default value functions
fn default_transport() -> String {
    "websocket".to_string()
}

fn default_address() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    7331
}

fn default_policy_mode() -> String {
    "consent-required".to_string()
}

fn default_fs_root() -> PathBuf {
    PathBuf::from("/app")
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = McpConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.transport, "websocket");
        assert_eq!(config.port, 7331);
    }

    #[test]
    fn test_parse_config() {
        let toml = r#"
            enabled = true
            transport = "stdio"
            port = 8080

            [policy]
            mode = "allow-all"
            audit_all = false

            [tools.gpu_stats]
            enabled = false
        "#;

        let config: McpConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
        assert_eq!(config.transport, "stdio");
        assert_eq!(config.port, 8080);
        assert_eq!(config.policy.mode, "allow-all");
        assert!(!config.policy.audit_all);
        assert!(!config.tools.gpu_stats.enabled);
    }
}
