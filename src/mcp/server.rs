//! Bolt MCP Server implementation
//!
//! This module provides the main MCP server that wraps Glyph and exposes
//! Bolt-specific tools.

use crate::mcp::{config::McpConfig, tools::*, McpError, Result};
use anyhow::Context;
use std::sync::Arc;
use tracing::{info, warn};
use async_trait::async_trait;

/// Adapter to convert Bolt McpTool to Glyph Tool
struct BoltToolAdapter<T: McpTool> {
    inner: T,
}

impl<T: McpTool> BoltToolAdapter<T> {
    fn new(inner: T) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<T: McpTool + 'static> glyph::server::Tool for BoltToolAdapter<T> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> Option<&str> {
        Some(self.inner.description())
    }

    fn input_schema(&self) -> glyph::protocol::ToolInputSchema {
        // Convert Bolt's Value schema to Glyph's ToolInputSchema
        let schema_value = self.inner.input_schema();

        // Parse the JSON schema from Value
        if let Some(obj) = schema_value.as_object() {
            let properties = obj.get("properties")
                .and_then(|v| v.as_object())
                .map(|props| {
                    props.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                })
                .unwrap_or_default();

            let required = obj.get("required")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            glyph::protocol::ToolInputSchema::object()
                .with_properties(properties)
                .with_required(required)
        } else {
            // Fallback to empty schema
            glyph::protocol::ToolInputSchema::object()
        }
    }

    async fn call(&self, args: Option<serde_json::Value>) -> glyph::Result<glyph::protocol::CallToolResult> {
        let input = args.unwrap_or(serde_json::Value::Null);

        match self.inner.execute(input).await {
            Ok(result) => {
                // Convert result to Content
                let content = vec![glyph::protocol::Content::text(
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string())
                )];
                Ok(glyph::protocol::CallToolResult::success(content))
            }
            Err(e) => {
                // Convert error to glyph error
                let error_msg = format!("Tool execution failed: {}", e);
                let content = vec![glyph::protocol::Content::text(error_msg)];
                Ok(glyph::protocol::CallToolResult::error(content))
            }
        }
    }
}

/// Bolt MCP Server
///
/// Wraps Glyph's MCP server implementation and registers Bolt-specific tools.
pub struct BoltMcpServer {
    config: McpConfig,
}

impl BoltMcpServer {
    /// Create a new Bolt MCP server with the given configuration
    pub fn new(config: McpConfig) -> Self {
        Self { config }
    }

    /// Run the MCP server with the configured transport
    pub async fn run(self) -> Result<()> {
        if !self.config.enabled {
            warn!("MCP server is disabled in configuration");
            return Ok(());
        }

        info!(
            "Starting Bolt MCP server on {}:{}",
            self.config.address, self.config.port
        );

        match self.config.transport.as_str() {
            "stdio" => self.run_stdio().await,
            "websocket" => self.run_websocket().await,
            "http" => self.run_http().await,
            other => Err(McpError::Config(format!(
                "Unsupported transport: {}",
                other
            ))),
        }
    }

    /// Run with stdio transport
    async fn run_stdio(self) -> Result<()> {
        info!("Starting MCP server with stdio transport");

        // Build Glyph server
        let builder = glyph::server::ServerBuilder::new()
            .with_server_info("bolt-mcp", env!("CARGO_PKG_VERSION"))
            .with_tools();

        // Register tools
        let stdio_server = builder.for_stdio();
        self.register_tools(&stdio_server.server()).await?;

        info!("MCP server running on stdio");

        // Run the server
        stdio_server.run().await.map_err(|e| McpError::Config(format!("Glyph server error: {}", e)))
    }

    /// Run with WebSocket transport
    async fn run_websocket(self) -> Result<()> {
        let addr = format!("{}:{}", self.config.address, self.config.port);
        info!("Starting MCP server with WebSocket transport on {}", addr);

        // Build Glyph server
        let builder = glyph::server::ServerBuilder::new()
            .with_server_info("bolt-mcp", env!("CARGO_PKG_VERSION"))
            .with_tools();

        // Create WebSocket server
        let ws_server = builder.for_websocket(&addr).await
            .map_err(|e| McpError::Config(format!("Failed to bind WebSocket server: {}", e)))?;

        // Register tools
        self.register_tools(&ws_server.server()).await?;

        info!("MCP server running on ws://{}", addr);

        // Run the server
        ws_server.run().await.map_err(|e| McpError::Config(format!("Glyph server error: {}", e)))
    }

    /// Run with HTTP transport
    async fn run_http(self) -> Result<()> {
        let addr = format!("{}:{}", self.config.address, self.config.port);
        info!("Starting MCP server with HTTP transport on {}", addr);

        // NOTE: HTTP transport support is planned for Glyph
        // For now, recommend using WebSocket transport which provides similar functionality
        warn!("HTTP transport not yet implemented in Glyph - please use WebSocket transport instead");

        Err(McpError::Config(
            "HTTP transport not yet implemented - use 'websocket' or 'stdio' instead".to_string()
        ))
    }

    /// Register all Bolt-specific tools
    async fn register_tools(&self, server: &glyph::server::Server) -> Result<()> {
        info!("Registering Bolt MCP tools");

        // GPU stats tool
        if self.config.tools.gpu_stats.enabled {
            info!("Registering GPU stats tool");
            let gpu_tool = GpuStatsTool::new()?;
            let adapter = BoltToolAdapter::new(gpu_tool);
            server.register_tool(adapter).await
                .map_err(|e| McpError::Config(format!("Failed to register GPU tool: {}", e)))?;
        }

        // Filesystem tool
        if self.config.tools.filesystem.enabled {
            info!(
                "Registering filesystem tool (root: {:?})",
                self.config.tools.filesystem.root
            );
            let fs_tool = FilesystemTool::new(
                self.config.tools.filesystem.root.clone()
            );
            let adapter = BoltToolAdapter::new(fs_tool);
            server.register_tool(adapter).await
                .map_err(|e| McpError::Config(format!("Failed to register filesystem tool: {}", e)))?;
        }

        // Shell tool
        if self.config.tools.shell.enabled {
            info!(
                "Registering shell tool (allowed commands: {:?})",
                self.config.tools.shell.allowed_commands
            );
            let shell_tool = ShellTool::with_allowlist(
                "bolt-host".to_string(),  // MCP server runs on host, not in a container
                self.config.tools.shell.allowed_commands.clone()
            );
            let adapter = BoltToolAdapter::new(shell_tool);
            server.register_tool(adapter).await
                .map_err(|e| McpError::Config(format!("Failed to register shell tool: {}", e)))?;
        }

        // Process tool
        if self.config.tools.process.enabled {
            info!("Registering process management tool");
            let process_tool = ProcessTool::new();
            let adapter = BoltToolAdapter::new(process_tool);
            server.register_tool(adapter).await
                .map_err(|e| McpError::Config(format!("Failed to register process tool: {}", e)))?;
        }

        // Network tool
        if self.config.tools.network.enabled {
            info!("Registering network stats tool");
            let network_tool = NetworkTool::new();
            let adapter = BoltToolAdapter::new(network_tool);
            server.register_tool(adapter).await
                .map_err(|e| McpError::Config(format!("Failed to register network tool: {}", e)))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let config = McpConfig::default();
        let server = BoltMcpServer::new(config);
        assert!(!server.config.enabled);
    }

    #[tokio::test]
    async fn test_disabled_server() {
        let config = McpConfig::default();
        let server = BoltMcpServer::new(config);
        let result = server.run().await;
        assert!(result.is_ok());
    }
}
