//! MCP Server Catalog
//!
//! TOML-based catalog system for defining MCP servers

use crate::{GatewayError, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use tracing::{info, warn};

/// MCP Server Catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    /// Catalog metadata
    #[serde(default)]
    pub metadata: CatalogMetadata,

    /// Server definitions
    #[serde(default)]
    pub servers: IndexMap<String, ServerDefinition>,
}

/// Catalog metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogMetadata {
    /// Catalog name
    #[serde(default)]
    pub name: String,

    /// Catalog version
    #[serde(default)]
    pub version: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// Author
    #[serde(default)]
    pub author: String,
}

impl Default for CatalogMetadata {
    fn default() -> Self {
        Self {
            name: "Bolt MCP Catalog".to_string(),
            version: "1.0.0".to_string(),
            description: "MCP servers for Bolt containers".to_string(),
            author: "Bolt".to_string(),
        }
    }
}

/// Server definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerDefinition {
    /// Server name
    pub name: String,

    /// Server type (embedded, container, external)
    #[serde(default = "default_server_type")]
    pub server_type: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// Container image (for container type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Command (for container type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,

    /// Environment variables
    #[serde(default)]
    pub env: IndexMap<String, String>,

    /// Volume mounts
    #[serde(default)]
    pub volumes: Vec<String>,

    /// Network mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_mode: Option<String>,

    /// Available tools
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,

    /// Resource limits
    #[serde(default)]
    pub resources: ResourceLimits,

    /// Policy mode
    #[serde(default = "default_policy_mode")]
    pub policy_mode: String,

    /// Enabled by default
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,

    /// Tool description
    #[serde(default)]
    pub description: String,

    /// Input schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<serde_json::Value>,

    /// Enabled by default
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Resource limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// CPU limit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpus: Option<f32>,

    /// Memory limit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,

    /// GPU devices
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpus: Option<String>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpus: None,
            memory: None,
            gpus: None,
        }
    }
}

impl Catalog {
    /// Load catalog from TOML file
    pub async fn load(path: &Path) -> Result<Self> {
        info!("Loading MCP catalog from {:?}", path);

        if !path.exists() {
            warn!("Catalog file not found, creating default catalog");
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path).await?;
        let catalog: Catalog = toml::from_str(&content)?;

        info!("Loaded {} servers from catalog", catalog.servers.len());
        Ok(catalog)
    }

    /// Save catalog to TOML file
    pub async fn save(&self, path: &Path) -> Result<()> {
        info!("Saving MCP catalog to {:?}", path);

        let content = toml::to_string_pretty(self)
            .map_err(|e| GatewayError::Catalog(format!("Failed to serialize catalog: {}", e)))?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(path, content).await?;
        info!("Catalog saved successfully");
        Ok(())
    }

    /// Get a server definition by name
    pub fn get_server(&self, name: &str) -> Option<&ServerDefinition> {
        self.servers.get(name)
    }

    /// Add a server to the catalog
    pub fn add_server(&mut self, name: String, server: ServerDefinition) {
        self.servers.insert(name, server);
    }

    /// Remove a server from the catalog
    pub fn remove_server(&mut self, name: &str) -> Option<ServerDefinition> {
        self.servers.shift_remove(name)
    }

    /// Get count of servers
    pub fn server_count(&self) -> usize {
        self.servers.len()
    }

    /// Get enabled servers
    pub fn enabled_servers(&self) -> impl Iterator<Item = (&String, &ServerDefinition)> {
        self.servers.iter().filter(|(_, server)| server.enabled)
    }

    /// Create a default catalog with example servers
    pub fn default_with_examples() -> Self {
        let mut catalog = Self::default();

        // Add Bolt runtime server
        catalog.add_server(
            "bolt-runtime".to_string(),
            ServerDefinition {
                name: "bolt-runtime".to_string(),
                server_type: "embedded".to_string(),
                description: "Bolt container runtime MCP server".to_string(),
                image: None,
                command: None,
                env: IndexMap::new(),
                volumes: vec![],
                network_mode: None,
                tools: vec![
                    ToolDefinition {
                        name: "bolt_gpu_stats".to_string(),
                        description: "Get GPU statistics".to_string(),
                        schema: None,
                        enabled: true,
                    },
                    ToolDefinition {
                        name: "bolt_filesystem".to_string(),
                        description: "Container filesystem access".to_string(),
                        schema: None,
                        enabled: true,
                    },
                    ToolDefinition {
                        name: "bolt_shell_exec".to_string(),
                        description: "Execute shell commands".to_string(),
                        schema: None,
                        enabled: true,
                    },
                ],
                resources: ResourceLimits::default(),
                policy_mode: "consent-required".to_string(),
                enabled: true,
            },
        );

        catalog
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self {
            metadata: CatalogMetadata::default(),
            servers: IndexMap::new(),
        }
    }
}

fn default_server_type() -> String {
    "container".to_string()
}

fn default_policy_mode() -> String {
    "consent-required".to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_catalog() {
        let catalog = Catalog::default();
        assert_eq!(catalog.server_count(), 0);
    }

    #[test]
    fn test_catalog_with_examples() {
        let catalog = Catalog::default_with_examples();
        assert!(catalog.server_count() > 0);
        assert!(catalog.get_server("bolt-runtime").is_some());
    }

    #[test]
    fn test_add_remove_server() {
        let mut catalog = Catalog::default();

        let server = ServerDefinition {
            name: "test".to_string(),
            server_type: "embedded".to_string(),
            description: "Test server".to_string(),
            image: None,
            command: None,
            env: IndexMap::new(),
            volumes: vec![],
            network_mode: None,
            tools: vec![],
            resources: ResourceLimits::default(),
            policy_mode: "allow-all".to_string(),
            enabled: true,
        };

        catalog.add_server("test".to_string(), server);
        assert_eq!(catalog.server_count(), 1);

        let removed = catalog.remove_server("test");
        assert!(removed.is_some());
        assert_eq!(catalog.server_count(), 0);
    }

    #[test]
    fn test_serialize_catalog() {
        let catalog = Catalog::default_with_examples();
        let toml = toml::to_string_pretty(&catalog).unwrap();
        assert!(toml.contains("bolt-runtime"));
    }
}
