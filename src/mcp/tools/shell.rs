//! Shell command execution tool
//!
//! Provides secure shell command execution within containers with command allowlisting

use crate::mcp::{
    McpError, Result,
    tools::{McpTool, ToolStreamEvent},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// Shell execution tool
///
/// Executes shell commands within the container with security controls:
/// - Command allowlist enforcement
/// - Output capture (stdout/stderr)
/// - Exit code tracking
/// - Audit logging
pub struct ShellTool {
    container_id: String,
    allowed_commands: Vec<String>,
}

impl ShellTool {
    /// Create a new shell tool for a specific container
    ///
    /// By default, allows common safe commands. For production, configure per-container allowlists.
    pub fn new(container_id: String) -> Self {
        // Default safe command allowlist
        let allowed_commands = vec![
            "ls".to_string(),
            "cat".to_string(),
            "grep".to_string(),
            "echo".to_string(),
            "pwd".to_string(),
            "whoami".to_string(),
            "ps".to_string(),
            "cargo".to_string(),
            "npm".to_string(),
            "git".to_string(),
            "zig".to_string(),
            "rustc".to_string(),
        ];

        tracing::info!(
            container_id = %container_id,
            allowed_commands = allowed_commands.len(),
            "Shell tool created with default safe allowlist"
        );

        Self {
            container_id,
            allowed_commands,
        }
    }

    /// Create a shell tool with custom allowlist
    pub fn with_allowlist(container_id: String, allowed_commands: Vec<String>) -> Self {
        if allowed_commands.is_empty() {
            tracing::warn!(
                container_id = %container_id,
                "Shell tool created with no command restrictions - all commands allowed!"
            );
        } else {
            tracing::info!(
                container_id = %container_id,
                allowed_commands = allowed_commands.len(),
                "Shell tool created with custom allowlist"
            );
        }

        Self {
            container_id,
            allowed_commands,
        }
    }

    /// Check if a command is allowed
    fn is_command_allowed(&self, command: &str) -> bool {
        if self.allowed_commands.is_empty() {
            return true; // No restrictions
        }

        let cmd_name = command.split_whitespace().next().unwrap_or("");

        self.allowed_commands
            .iter()
            .any(|allowed| allowed == cmd_name)
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

    async fn execute_stream(&self, input: Value) -> Result<mpsc::Receiver<ToolStreamEvent>> {
        let args: ShellInput = serde_json::from_value(input)?;
        let (tx, rx) = mpsc::channel(100);

        // Validate command is allowed
        if !self.is_command_allowed(&args.command) {
            let cmd_name = args
                .command
                .split_whitespace()
                .next()
                .unwrap_or(&args.command);
            let _ = tx
                .send(ToolStreamEvent::Error {
                    error: format!(
                        "Command '{}' is not in the allowlist. Allowed commands: {:?}",
                        cmd_name, self.allowed_commands
                    ),
                    error_code: "PERMISSION_DENIED".to_string(),
                })
                .await;
            return Ok(rx);
        }

        let tool_name = self.name().to_string();
        let command = args.command.clone();

        tokio::spawn(async move {
            let start = std::time::Instant::now();

            // Send started event
            let _ = tx
                .send(ToolStreamEvent::Started {
                    tool_name: tool_name.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                })
                .await;

            tracing::info!(command = %command, "Executing shell command with streaming");

            // Spawn process with piped stdout/stderr
            let mut child = match Command::new("sh")
                .arg("-c")
                .arg(&command)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(e) => {
                    let _ = tx
                        .send(ToolStreamEvent::Error {
                            error: format!("Failed to spawn command: {}", e),
                            error_code: "SPAWN_FAILED".to_string(),
                        })
                        .await;
                    return;
                }
            };

            // Stream stdout
            if let Some(stdout) = child.stdout.take() {
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    let reader = BufReader::new(stdout);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let mut data = line.into_bytes();
                        data.push(b'\n');
                        let _ = tx_clone
                            .send(ToolStreamEvent::Output {
                                stream: "stdout".to_string(),
                                data,
                            })
                            .await;
                    }
                });
            }

            // Stream stderr
            if let Some(stderr) = child.stderr.take() {
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    let reader = BufReader::new(stderr);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let mut data = line.into_bytes();
                        data.push(b'\n');
                        let _ = tx_clone
                            .send(ToolStreamEvent::Output {
                                stream: "stderr".to_string(),
                                data,
                            })
                            .await;
                    }
                });
            }

            // Wait for process to complete
            match child.wait().await {
                Ok(status) => {
                    let execution_time_ms = start.elapsed().as_millis() as u64;
                    let _ = tx
                        .send(ToolStreamEvent::Complete {
                            result: json!({
                                "exit_code": status.code(),
                                "success": status.success(),
                            }),
                            execution_time_ms,
                        })
                        .await;
                }
                Err(e) => {
                    let _ = tx
                        .send(ToolStreamEvent::Error {
                            error: format!("Failed to wait for command: {}", e),
                            error_code: "WAIT_FAILED".to_string(),
                        })
                        .await;
                }
            }
        });

        Ok(rx)
    }

    fn supports_streaming(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_allowlist() {
        let tool = ShellTool::with_allowlist(
            "test-container".to_string(),
            vec!["ls".to_string(), "ps".to_string()],
        );

        assert!(tool.is_command_allowed("ls"));
        assert!(tool.is_command_allowed("ls -la"));
        assert!(tool.is_command_allowed("ps aux"));
        assert!(!tool.is_command_allowed("rm -rf /"));
        assert!(!tool.is_command_allowed("cat /etc/passwd"));
    }

    #[tokio::test]
    async fn test_execute_allowed_command() {
        let tool = ShellTool::new("test-container".to_string());

        let result = tool.execute_command("echo hello").await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.success);
        assert!(output.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_execute_disallowed_command() {
        let tool =
            ShellTool::with_allowlist("test-container".to_string(), vec!["echo".to_string()]);

        let result = tool.execute_command("rm -rf /").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_streaming_execution() {
        use crate::mcp::tools::{McpTool, ToolStreamEvent};

        let tool = ShellTool::new("test-container".to_string());

        let input = json!({"command": "echo 'line1' && echo 'line2'"});
        let mut rx = tool
            .execute_stream(input)
            .await
            .expect("Stream should start");

        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            events.push(event);
        }

        // Should have: Started, Output(s), Complete
        assert!(!events.is_empty());

        // First event should be Started
        if let ToolStreamEvent::Started { .. } = events[0] {
            // ok
        } else {
            panic!("First event should be Started, got: {:?}", events[0]);
        }

        // Last event should be Complete
        let last = events.last().unwrap();
        if let ToolStreamEvent::Complete { .. } = last {
            // ok
        } else {
            panic!("Last event should be Complete, got: {:?}", last);
        }

        // Should have at least one Output event
        let has_output = events
            .iter()
            .any(|e| matches!(e, ToolStreamEvent::Output { .. }));
        assert!(has_output, "Should have at least one Output event");
    }
}
