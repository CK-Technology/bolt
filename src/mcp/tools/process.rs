//! Process management tool
//!
//! Provides process listing and management capabilities within containers

use crate::mcp::{McpError, Result, tools::McpTool};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::process::Command;

/// Process management tool
///
/// Provides capabilities to:
/// - List running processes
/// - Get process details
/// - Monitor resource usage
pub struct ProcessTool;

impl ProcessTool {
    /// Create a new process management tool
    pub fn new() -> Self {
        Self
    }

    async fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        tracing::info!("Listing container processes");

        let output = Command::new("ps")
            .args(&["aux"])
            .output()
            .await
            .map_err(|e| McpError::ToolExecution(format!("Failed to list processes: {}", e)))?;

        if !output.status.success() {
            return Err(McpError::ToolExecution("ps command failed".to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut processes = Vec::new();

        // Skip header line
        for line in stdout.lines().skip(1) {
            if let Some(process) = self.parse_ps_line(line) {
                processes.push(process);
            }
        }

        Ok(processes)
    }

    fn parse_ps_line(&self, line: &str) -> Option<ProcessInfo> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 11 {
            return None;
        }

        Some(ProcessInfo {
            user: parts[0].to_string(),
            pid: parts[1].parse().ok()?,
            cpu_percent: parts[2].parse().ok()?,
            mem_percent: parts[3].parse().ok()?,
            vsz: parts[4].parse().ok()?,
            rss: parts[5].parse().ok()?,
            tty: parts[6].to_string(),
            stat: parts[7].to_string(),
            start: parts[8].to_string(),
            time: parts[9].to_string(),
            command: parts[10..].join(" "),
        })
    }

    async fn get_process_count(&self) -> Result<usize> {
        let processes = self.list_processes().await?;
        Ok(processes.len())
    }
}

impl Default for ProcessTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation")]
enum ProcessInput {
    #[serde(rename = "list")]
    List,
    #[serde(rename = "count")]
    Count,
}

#[derive(Debug, Serialize)]
struct ProcessInfo {
    user: String,
    pid: u32,
    cpu_percent: f32,
    mem_percent: f32,
    vsz: u64, // Virtual memory size in KB
    rss: u64, // Resident set size in KB
    tty: String,
    stat: String,
    start: String,
    time: String,
    command: String,
}

impl McpTool for ProcessTool {
    fn name(&self) -> &str {
        "bolt_process"
    }

    fn description(&self) -> &str {
        "Manage and monitor container processes"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "oneOf": [
                {
                    "properties": {
                        "operation": { "const": "list" }
                    },
                    "required": ["operation"]
                },
                {
                    "properties": {
                        "operation": { "const": "count" }
                    },
                    "required": ["operation"]
                }
            ]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let args: ProcessInput = serde_json::from_value(input)?;

        match args {
            ProcessInput::List => {
                let processes = self.list_processes().await?;
                Ok(json!({
                    "operation": "list",
                    "count": processes.len(),
                    "processes": processes
                }))
            }
            ProcessInput::Count => {
                let count = self.get_process_count().await?;
                Ok(json!({
                    "operation": "count",
                    "count": count
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_creation() {
        let tool = ProcessTool::new();
        assert_eq!(tool.name(), "bolt_process");
    }

    #[tokio::test]
    async fn test_list_processes() {
        let tool = ProcessTool::new();
        let result = tool.list_processes().await;

        // This test might fail in environments without 'ps' command
        // but it's useful for local testing
        if result.is_ok() {
            let processes = result.unwrap();
            assert!(!processes.is_empty());
        }
    }
}
