//! MCP tools for Bolt containers
//!
//! This module contains all the MCP tools that Bolt exposes:
//! - GPU statistics
//! - Container filesystem access
//! - Shell command execution
//! - Process management
//! - Network statistics
//! - OMEN AI router (Phase 3)

pub mod filesystem;
pub mod gpu;
pub mod network;
pub mod process;
pub mod shell;

#[cfg(feature = "omen")]
pub mod omen_router;

pub use filesystem::FilesystemTool;
pub use gpu::GpuStatsTool;
pub use network::NetworkTool;
pub use process::ProcessTool;
pub use shell::ShellTool;

#[cfg(feature = "omen")]
pub use omen_router::OmenRouterTool;

use crate::mcp::Result;
use serde_json::Value;
use tokio::sync::mpsc;

/// Stream events for tool execution
#[derive(Debug, Clone)]
pub enum ToolStreamEvent {
    /// Tool execution started
    Started { tool_name: String, timestamp: i64 },
    /// Progress update
    Progress { message: String, percent: u32 },
    /// Output chunk (stdout/stderr)
    Output { stream: String, data: Vec<u8> },
    /// Tool completed successfully
    Complete {
        result: Value,
        execution_time_ms: u64,
    },
    /// Tool execution failed
    Error { error: String, error_code: String },
}

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

    /// Execute the tool (blocking, returns final result)
    fn execute(&self, input: Value) -> impl std::future::Future<Output = Result<Value>> + Send;

    /// Execute the tool with streaming support (optional)
    ///
    /// Default implementation falls back to non-streaming execute.
    /// Tools that support streaming (like shell, filesystem watch) should override this.
    fn execute_stream(
        &self,
        input: Value,
    ) -> impl std::future::Future<Output = Result<mpsc::Receiver<ToolStreamEvent>>> + Send {
        async move {
            let (tx, rx) = mpsc::channel(100);
            let tool_name = self.name().to_string();

            // Send started event
            let _ = tx
                .send(ToolStreamEvent::Started {
                    tool_name: tool_name.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                })
                .await;

            // Execute tool
            let start = std::time::Instant::now();
            match self.execute(input).await {
                Ok(result) => {
                    let _ = tx
                        .send(ToolStreamEvent::Complete {
                            result,
                            execution_time_ms: start.elapsed().as_millis() as u64,
                        })
                        .await;
                }
                Err(e) => {
                    let _ = tx
                        .send(ToolStreamEvent::Error {
                            error: e.to_string(),
                            error_code: "EXECUTION_FAILED".to_string(),
                        })
                        .await;
                }
            }

            Ok(rx)
        }
    }

    /// Whether this tool supports real-time streaming
    fn supports_streaming(&self) -> bool {
        false
    }
}
