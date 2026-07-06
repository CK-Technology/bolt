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

        let container = self.resolve_running_container(&self.container).await?;
        debug!("Container ID: {}", container.id);

        let exit_code = bolt::runtime::oci::exec_runtime_container(
            &container.id,
            &self.command,
            self.interactive,
            self.tty,
            self.detach,
            self.workdir.as_deref(),
            self.user.as_deref(),
            &self.env,
        )
        .await?;

        if exit_code != 0 {
            std::process::exit(exit_code);
        }

        Ok(())
    }

    async fn resolve_running_container(
        &self,
        container_id: &str,
    ) -> Result<bolt::runtime::oci::ContainerState> {
        use bolt::runtime::state;

        // Resolve the reference (id or --name) against persisted state so exec
        // works across a restart of the Bolt process.
        let container = state::resolve_ref(container_id)?
            .ok_or_else(|| anyhow!("Container not found: {}", container_id))?;

        let pid = container
            .pid
            .ok_or_else(|| anyhow!("Container {} is not running", container_id))?;

        // Verify the process is actually alive before entering its namespaces.
        if !state::pid_is_alive(pid) {
            return Err(anyhow!(
                "Container {} is not running (process {} has exited)",
                container_id,
                pid
            ));
        }

        Ok(container)
    }
}
