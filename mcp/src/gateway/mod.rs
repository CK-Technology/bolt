//! MCP Gateway components

pub mod catalog;
pub mod client_manager;
pub mod config;
pub mod interceptor;
pub mod registry;
pub mod secrets;

#[cfg(feature = "omen")]
pub mod omen_adapter;

use crate::{GatewayError, Result};
use catalog::Catalog;
use client_manager::ClientManager;
use config::GatewayConfig;
use interceptor::InterceptorChain;
use registry::ToolRegistry;
use secrets::SecretStore;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// MCP Gateway
///
/// Centralized gateway for managing MCP servers across multiple containers
pub struct McpGateway {
    config: GatewayConfig,
    catalog: Arc<RwLock<Catalog>>,
    tool_registry: Arc<RwLock<ToolRegistry>>,
    client_manager: Arc<ClientManager>,
    secret_store: Arc<SecretStore>,
    interceptors: Arc<InterceptorChain>,
}

impl McpGateway {
    /// Create a new MCP gateway
    pub async fn new(config: GatewayConfig) -> Result<Self> {
        info!("Initializing MCP Gateway");

        // Load catalog
        let catalog = Catalog::load(&config.catalog_path).await?;
        info!("Loaded catalog with {} servers", catalog.server_count());

        // Initialize tool registry
        let tool_registry = ToolRegistry::new();

        // Initialize client manager
        let client_manager = ClientManager::new();

        // Initialize secret store
        let secret_store = SecretStore::new(&config.secret_sources).await?;

        // Initialize interceptor chain
        let interceptors = InterceptorChain::new();

        Ok(Self {
            config,
            catalog: Arc::new(RwLock::new(catalog)),
            tool_registry: Arc::new(RwLock::new(tool_registry)),
            client_manager: Arc::new(client_manager),
            secret_store: Arc::new(secret_store),
            interceptors: Arc::new(interceptors),
        })
    }

    /// Run the gateway
    pub async fn run(self) -> Result<()> {
        info!(
            "Starting MCP Gateway on {}:{}",
            self.config.address, self.config.port
        );

        match self.config.transport.as_str() {
            "stdio" => self.run_stdio().await,
            "websocket" => self.run_websocket().await,
            "http" => self.run_http().await,
            other => Err(GatewayError::Catalog(format!(
                "Unsupported transport: {}",
                other
            ))),
        }
    }

    async fn run_stdio(self) -> Result<()> {
        info!("Gateway running with stdio transport");
        // TODO: Implement stdio transport with Glyph
        Ok(())
    }

    async fn run_websocket(self) -> Result<()> {
        let addr = format!("{}:{}", self.config.address, self.config.port);
        info!("Gateway running with WebSocket transport on {}", addr);
        // TODO: Implement WebSocket transport with Glyph
        Ok(())
    }

    async fn run_http(self) -> Result<()> {
        let addr = format!("{}:{}", self.config.address, self.config.port);
        info!("Gateway running with HTTP transport on {}", addr);
        // TODO: Implement HTTP transport with Glyph
        Ok(())
    }

    /// Get gateway configuration
    pub fn config(&self) -> &GatewayConfig {
        &self.config
    }

    /// Get catalog
    pub fn catalog(&self) -> Arc<RwLock<Catalog>> {
        self.catalog.clone()
    }

    /// Get tool registry
    pub fn tool_registry(&self) -> Arc<RwLock<ToolRegistry>> {
        self.tool_registry.clone()
    }

    /// Get client manager
    pub fn client_manager(&self) -> Arc<ClientManager> {
        self.client_manager.clone()
    }

    /// Get secret store
    pub fn secret_store(&self) -> Arc<SecretStore> {
        self.secret_store.clone()
    }
}
