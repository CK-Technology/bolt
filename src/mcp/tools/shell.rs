//! Shell command execution tool
//!
//! Provides secure shell command execution within containers with command allowlisting

use crate::mcp::{tools::McpTool, McpError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::process::Command;

/// Shell execution tool
///
/// Executes shell commands within the container with security controls:
/// - Command allowlist enforcement
/// - Output capture (stdout/stderr)
/// - Exit code tracking
/// - Audit logging
pub struct ShellTool {
    allowed_commands: Vec<String>,
}

impl ShellTool {
    /// Create a new shell tool with an allowlist of commands
    ///
    /// If `allowed_commands` is empty, all commands are allowed (use with caution!)
    pub fn new(allowed_commands: Vec<String>) -> Self {
        if allowed_commands.is_empty() {
            tracing::warn!("Shell tool created with no command restrictions - all commands allowed!");
        } else {
            tracing::info!("Shell tool created with {} allowed commands", allowed_commands.len());
        }

        Self { allowed_commands }
    }

    /// Check if a command is allowed
    fn is_command_allowed(&self, command: &str) -> bool {
        if self.allowed_commands.is_empty() {
            return true; // No restrictions
        }

        let cmd_name = command
            .split_whitespace()
            .next()
            .unwrap_or("");

        self.allowed_commands.iter().any(|allowed| allowed == cmd_name)
    }

    async fn execute_command(&self, command: &str) -> Result<CommandOutput> {
        // Validate command is allowed
        if !self.is_command_allowed(command) {
            let cmd_name = command.split_whitespace().next().unwrap_or(command);
            return Err(McpError::PermissionDenied(format!(
                "Command '{}' is not in the allowlist. Allowed commands: {:?}",
                cmd_name, self.allowed_commands
            )));
        }

        tracing::warn!(
            command = %command,
            "Executing shell command"
        );

        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await
            .map_err(|e| McpError::ToolExecution(format!("Failed to execute command: {}", e)))?;

        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            success: output.status.success(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct ShellInput {
    command: String,
}

#[derive(Debug, Serialize)]
struct CommandOutput {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    success: bool,
}

impl McpTool for ShellTool {
    fn name(&self) -> &str {
        "bolt_shell_exec"
    }

    fn description(&self) -> &str {
        "Execute shell commands in the container with security controls"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let args: ShellInput = serde_json::from_value(input)?;

        let output = self.execute_command(&args.command).await?;

        Ok(serde_json::to_value(output)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_allowlist() {
        let tool = ShellTool::new(vec!["ls".to_string(), "ps".to_string()]);

        assert!(tool.is_command_allowed("ls"));
        assert!(tool.is_command_allowed("ls -la"));
        assert!(tool.is_command_allowed("ps aux"));
        assert!(!tool.is_command_allowed("rm -rf /"));
        assert!(!tool.is_command_allowed("cat /etc/passwd"));
    }

    #[tokio::test]
    async fn test_execute_allowed_command() {
        let tool = ShellTool::new(vec!["echo".to_string()]);

        let result = tool.execute_command("echo hello").await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.success);
        assert!(output.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_execute_disallowed_command() {
        let tool = ShellTool::new(vec!["echo".to_string()]);

        let result = tool.execute_command("rm -rf /").await;
        assert!(result.is_err());
    }
}
