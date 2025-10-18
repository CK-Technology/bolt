//! Bolt MCP Gateway
//!
//! This crate provides MCP gateway functionality for Bolt containers,
//! allowing centralized management of MCP servers across multiple containers.

pub mod gateway;

pub use gateway::{
    catalog::Catalog,
    client_manager::ClientManager,
    config::GatewayConfig,
    interceptor::Interceptor,
    registry::ToolRegistry,
    secrets::SecretStore,
    McpGateway,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("Catalog error: {0}")]
    Catalog(String),

    #[error("Registry error: {0}")]
    Registry(String),

    #[error("Client error: {0}")]
    Client(String),

    #[error("Secret error: {0}")]
    Secret(String),

    #[error("Interceptor error: {0}")]
    Interceptor(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Glyph error: {0}")]
    Glyph(String),
}

pub type Result<T> = std::result::Result<T, GatewayError>;
