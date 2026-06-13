//! Interactive shell execution in running containers

use crate::Result;
use anyhow::anyhow;
use clap::Parser;
use tracing::{debug, info};

#[derive(Parser)]
pub struct ExecCommand {
    /// Container name or ID
    pub container: String,

    /// Command to execute
    pub command: Vec<String>,

    /// Keep STDIN open (interactive)
    #[arg(short, long)]
    pub interactive: bool,

    /// Allocate a pseudo-TTY
    #[arg(short, long)]
    pub tty: bool,

    /// Detached mode (run in background)
    #[arg(short, long)]
    pub detach: bool,

    /// Working directory inside container
    #[arg(short = 'w', long)]
    pub workdir: Option<String>,

    /// User to execute as (user:group)
    #[arg(short = 'u', long)]
    pub user: Option<String>,

    /// Environment variables
    #[arg(short = 'e', long)]
    pub env: Vec<String>,

    /// Set additional group IDs
    #[arg(long)]
    pub group_add: Vec<String>,

    /// Set privileged mode
    #[arg(long)]
    pub privileged: bool,
}

impl ExecCommand {
    pub async fn execute(&self) -> Result<()> {
        info!("📟 Executing command in container: {}", self.container);
        debug!("Command: {:?}", self.command);

        if self.command.is_empty() {
            return Err(anyhow!("No command specified"));
        }

        // Get container PID
        let container_pid = self.get_container_pid(&self.container).await?;
        debug!("Container PID: {}", container_pid);

        // Build nsenter command to enter container namespaces
        let mut cmd = tokio::process::Command::new("nsenter");

        // Enter all namespaces
        cmd.args([
            "-t",
            &container_pid.to_string(),
            "-m", // mount namespace
            "-u", // UTS namespace
            "-i", // IPC namespace
            "-n", // network namespace
            "-p", // PID namespace
        ]);

        // Set user if specified
        if let Some(ref user) = self.user {
            if let Some((uid, gid)) = user.split_once(':') {
                cmd.args(["--setuid", uid]);
                cmd.args(["--setgid", gid]);
            } else {
                cmd.args(["--setuid", user]);
            }
        }

        // Set working directory
        if let Some(ref workdir) = self.workdir {
            cmd.current_dir(workdir);
        }

        // Set environment variables
        for env_var in &self.env {
            if let Some((key, value)) = env_var.split_once('=') {
                cmd.env(key, value);
            }
        }

        // Add the command to execute
        cmd.args(&self.command);

        // Setup TTY and interactive mode
        if self.tty && self.interactive {
            self.setup_interactive_tty(&mut cmd).await?;
        } else if self.interactive {
            cmd.stdin(std::process::Stdio::inherit());
        }

        cmd.stdout(std::process::Stdio::inherit());
        cmd.stderr(std::process::Stdio::inherit());

        // Execute the command
        if self.detach {
            // Spawn in background
            let child = cmd.spawn()?;
            println!("{}", child.id().unwrap_or(0));
            Ok(())
        } else {
            // Wait for completion
            let status = cmd.status().await?;

            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }

            Ok(())
        }
    }

    async fn get_container_pid(&self, container_id: &str) -> Result<u32> {
        // Read PID from container runtime state
        let runtime_dir = std::path::PathBuf::from("/run/bolt/containers");
        let state_file = runtime_dir.join(container_id).join("state.json");

        if !state_file.exists() {
            return Err(anyhow!(
                "Container {} not found or not running",
                container_id
            ));
        }

        let state_json = tokio::fs::read_to_string(&state_file).await?;
        let state: serde_json::Value = serde_json::from_str(&state_json)?;

        let pid = state["pid"]
            .as_u64()
            .ok_or_else(|| anyhow!("Container PID not found in state"))?;

        Ok(pid as u32)
    }

    async fn setup_interactive_tty(&self, cmd: &mut tokio::process::Command) -> Result<()> {
        use nix::sys::termios::{LocalFlags, SetArg, tcgetattr, tcsetattr};
        use nix::unistd::isatty;
        use std::os::fd::{AsRawFd, BorrowedFd};

        let stdin = std::io::stdin();
        let stdin_fd = stdin.as_raw_fd();

        // Check if stdin is a TTY
        if !isatty(stdin_fd).unwrap_or(false) {
            return Ok(());
        }

        // Get current terminal settings
        let stdin_borrowed = unsafe { BorrowedFd::borrow_raw(stdin_fd) };
        let mut termios = tcgetattr(stdin_borrowed)?;

        // Store original settings for restoration
        let original_termios = termios.clone();

        // Set raw mode (disable canonical mode and echo)
        termios.local_flags &= !(LocalFlags::ICANON | LocalFlags::ECHO);

        // Apply raw mode
        tcsetattr(stdin_borrowed, SetArg::TCSANOW, &termios)?;

        // Setup signal handler to restore terminal on exit
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            let stdin_borrowed_restore = unsafe { BorrowedFd::borrow_raw(stdin_fd) };
            let _ = tcsetattr(stdin_borrowed_restore, SetArg::TCSANOW, &original_termios);
            std::process::exit(0);
        });

        cmd.stdin(std::process::Stdio::inherit());

        Ok(())
    }
}
