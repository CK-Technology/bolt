//! MCP tools for Bolt containers
//!
//! This module contains all the MCP tools that Bolt exposes:
//! - GPU statistics
//! - Container filesystem access
//! - Shell command execution
//! - Process management
//! - Network statistics
//! - OMEN AI router (Phase 3)

pub mod gpu;
pub mod filesystem;
pub mod shell;
pub mod process;
pub mod network;

#[cfg(feature = "omen")]
pub mod omen_router;

pub use gpu::GpuStatsTool;
pub use filesystem::FilesystemTool;
pub use shell::ShellTool;
pub use process::ProcessTool;
pub use network::NetworkTool;

#[cfg(feature = "omen")]
pub use omen_router::OmenRouterTool;

use crate::mcp::Result;
use serde_json::Value;

/// Base trait for MCP tools
///
/// All tools must implement this trait to be registered with the MCP server.
pub trait McpTool: Send + Sync {
    /// Get the tool name
    fn name(&self) -> &str;

    /// Get the tool description
    fn description(&self) -> &str;

    /// Get the input schema (JSON Schema)
    fn input_schema(&self) -> Value;

    /// Execute the tool
    fn execute(&self, input: Value) -> impl std::future::Future<Output = Result<Value>> + Send;
}
