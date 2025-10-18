//! Model Context Protocol (MCP) integration for Bolt
//!
//! This module provides MCP server capabilities for Bolt containers, allowing
//! AI assistants to interact with containers through standardized tools.
//!
//! # Features
//!
//! - GPU statistics and monitoring
//! - Container filesystem access
//! - Shell command execution
//! - Process management
//! - Network statistics
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │         Bolt Container Runtime          │
//! │  ┌──────────────────────────────────┐  │
//! │  │     BoltMcpServer (Glyph)        │  │
//! │  │  • GPU Stats Tool                │  │
//! │  │  • Filesystem Tool                │  │
//! │  │  • Shell Tool                     │  │
//! │  │  • Process Tool                   │  │
//! │  │  • Network Tool                   │  │
//! │  └──────────────────────────────────┘  │
//! └─────────────────┬───────────────────────┘
//!                   │ MCP Protocol (WebSocket/stdio)
//!                   ▼
//!              ┌────────┐
//!              │ Client │
//!              └────────┘
//! ```

pub mod config;
pub mod server;
pub mod tools;

// Omen AI Router integration (optional)
#[cfg(feature = "omen")]
pub mod omen_integration;

pub use config::McpConfig;
pub use server::BoltMcpServer;

#[cfg(feature = "omen")]
pub use omen_integration::{OmenRouter, OmenConfig, RoutingStrategy, CompletionRequest, CompletionResponse};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("Glyph error: {0}")]
    Glyph(#[from] anyhow::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Tool execution error: {0}")]
    ToolExecution(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, McpError>;
