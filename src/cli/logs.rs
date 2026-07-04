//! Container log streaming

use crate::Result;
use anyhow::anyhow;
use chrono::Utc;
use clap::Parser;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, info};

#[derive(Parser)]
pub struct LogsCommand {
    /// Container name or ID
    pub container: String,

    /// Follow log output (stream)
    #[arg(short, long)]
    pub follow: bool,

    /// Show timestamps
    #[arg(short = 't', long)]
    pub timestamps: bool,

    /// Number of lines to show from end
    #[arg(long)]
    pub tail: Option<usize>,

    /// Show logs since timestamp (e.g., "2023-01-01T00:00:00Z")
    #[arg(long)]
    pub since: Option<String>,

    /// Show logs before timestamp
    #[arg(long)]
    pub until: Option<String>,

    /// Show extra details
    #[arg(long)]
    pub details: bool,
}

impl LogsCommand {
    pub async fn execute(&self) -> Result<()> {
        info!("📜 Fetching logs for container: {}", self.container);

        // Resolve the reference (id or --name) against persisted state so logs
        // work across a restart and accept container names.
        let id = match bolt::runtime::state::resolve_ref(&self.container)? {
            Some(state) => state.id,
            None => self.container.clone(),
        };

        let log_path = self.get_log_path(&id).await?;

        if !log_path.exists() {
            return Err(anyhow!(
                "No logs found for container: {} (foreground/tty containers are not captured yet)",
                self.container
            ));
        }

        if self.follow {
            self.stream_logs(&log_path).await
        } else {
            self.show_logs(&log_path).await
        }
    }

    async fn stream_logs(&self, log_path: &std::path::Path) -> Result<()> {
        debug!("Streaming logs from: {}", log_path.display());

        // Use tail -f for streaming
        let mut cmd = tokio::process::Command::new("tail");
        cmd.arg("-f");

        if let Some(n) = self.tail {
            cmd.args(["-n", &n.to_string()]);
        }

        cmd.arg(log_path);

        let mut child = cmd.stdout(std::process::Stdio::piped()).spawn()?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stdout"))?;

        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            self.print_log_line(&line);
        }

        Ok(())
    }

    async fn show_logs(&self, log_path: &std::path::Path) -> Result<()> {
        debug!("Reading logs from: {}", log_path.display());

        let content = tokio::fs::read_to_string(log_path).await?;
        let mut lines: Vec<&str> = content.lines().collect();

        // Apply tail filter
        if let Some(n) = self.tail {
            let start = lines.len().saturating_sub(n);
            lines = lines[start..].to_vec();
        }

        // Apply since/until filters
        if self.since.is_some() || self.until.is_some() {
            lines = self.filter_by_time(lines)?;
        }

        // Print logs
        for line in lines {
            self.print_log_line(line);
        }

        Ok(())
    }

    fn print_log_line(&self, line: &str) {
        if self.timestamps {
            let timestamp = Utc::now().to_rfc3339();
            println!("{} {}", timestamp, line);
        } else if self.details {
            // Parse log line for structured data
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&parsed).unwrap_or(line.to_string())
                );
            } else {
                println!("{}", line);
            }
        } else {
            println!("{}", line);
        }
    }

    fn filter_by_time<'a>(&self, lines: Vec<&'a str>) -> Result<Vec<&'a str>> {
        // Simplified time filtering - would need proper timestamp parsing
        Ok(lines)
    }

    async fn get_log_path(&self, container_id: &str) -> Result<std::path::PathBuf> {
        let log_dir = std::env::var_os("BOLT_LOG_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/var/log/bolt/containers"));
        tokio::fs::create_dir_all(&log_dir).await?;

        let log_file = log_dir.join(format!("{}.log", container_id));
        Ok(log_file)
    }
}
