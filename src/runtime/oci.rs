use crate::{BoltError, Result};
use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn, error};

/// OCI container configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub id: String,
    pub name: Option<String>,
    pub image: String,
    pub command: Vec<String>,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_dir: Option<String>,
    pub user: Option<String>,
    pub ports: Vec<PortMapping>,
    pub volumes: Vec<VolumeMount>,
    pub capabilities: Vec<String>,
    pub resource_limits: Option<ResourceLimits>,
    pub gaming_config: Option<GamingConfig>,
}

/// Container runtime state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerState {
    pub id: String,
    pub status: ContainerStatus,
    pub pid: Option<u32>,
    pub bundle_path: PathBuf,
    pub config: ContainerConfig,
    pub created: std::time::SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerStatus {
    Created,
    Running,
    Stopped,
    Exited(i32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMount {
    pub source: String,
    pub destination: String,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub memory: Option<u64>,
    pub cpu_shares: Option<u64>,
    pub cpu_quota: Option<i64>,
    pub cpu_period: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingConfig {
    pub gpu_enabled: bool,
    pub nvidia_runtime: bool,
    pub audio_enabled: bool,
    pub realtime_priority: bool,
}

/// Execute a container using native OCI runtime
pub async fn execute_container(state: &ContainerState, spec: &oci_spec::runtime::Spec) -> Result<u32> {
    info!("🚀 Executing container: {}", state.id);

    // Try to use runc or crun for OCI runtime
    let runtime = detect_oci_runtime().await?;

    debug!("Using OCI runtime: {}", runtime);

    // Create container with OCI runtime
    let mut cmd = Command::new(&runtime);
    cmd.arg("create")
        .arg("--bundle")
        .arg(&state.bundle_path)
        .arg(&state.id);

    let output = cmd.output().await
        .context("Failed to create container with OCI runtime")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Container creation failed: {}", stderr).into());
    }

    // Start the container
    let mut start_cmd = Command::new(&runtime);
    start_cmd.arg("start").arg(&state.id);

    let start_output = start_cmd.output().await
        .context("Failed to start container")?;

    if !start_output.status.success() {
        let stderr = String::from_utf8_lossy(&start_output.stderr);
        return Err(anyhow!("Container start failed: {}", stderr).into());
    }

    // Get container PID
    let pid = get_container_pid(&runtime, &state.id).await?;

    info!("✅ Container started with PID: {}", pid);
    Ok(pid)
}

/// Detect available OCI runtime
async fn detect_oci_runtime() -> Result<String> {
    // Try runc first (most common)
    if Command::new("runc")
        .arg("--version")
        .output()
        .await
        .is_ok()
    {
        return Ok("runc".to_string());
    }

    // Try crun (faster alternative)
    if Command::new("crun")
        .arg("--version")
        .output()
        .await
        .is_ok()
    {
        return Ok("crun".to_string());
    }

    // Try kata-runtime for secure containers
    if Command::new("kata-runtime")
        .arg("--version")
        .output()
        .await
        .is_ok()
    {
        return Ok("kata-runtime".to_string());
    }

    Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
        message: "No OCI runtime found (runc, crun, or kata-runtime required)".to_string(),
    }))
}

/// Get the PID of a running container
async fn get_container_pid(runtime: &str, container_id: &str) -> Result<u32> {
    let mut cmd = Command::new(runtime);
    cmd.arg("state").arg(container_id);

    let output = cmd.output().await
        .context("Failed to get container state")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Failed to get container state: {}", stderr).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse JSON output to get PID
    let state_json: serde_json::Value = serde_json::from_str(&stdout)
        .context("Failed to parse container state JSON")?;

    let pid = state_json
        .get("pid")
        .and_then(|p| p.as_u64())
        .ok_or_else(|| anyhow!("Container PID not found in state"))?;

    Ok(pid as u32)
}

/// Stop a container gracefully
pub async fn stop_container(runtime: &str, container_id: &str) -> Result<()> {
    info!("🛑 Stopping container: {}", container_id);

    let mut cmd = Command::new(runtime);
    cmd.arg("kill").arg(container_id).arg("TERM");

    let output = cmd.output().await
        .context("Failed to send SIGTERM to container")?;

    if !output.status.success() {
        warn!("SIGTERM failed, trying SIGKILL");

        // Force kill if graceful stop failed
        let mut kill_cmd = Command::new(runtime);
        kill_cmd.arg("kill").arg(container_id).arg("KILL");

        kill_cmd.output().await
            .context("Failed to send SIGKILL to container")?;
    }

    // Delete the container
    let mut delete_cmd = Command::new(runtime);
    delete_cmd.arg("delete").arg(container_id);

    delete_cmd.output().await
        .context("Failed to delete container")?;

    Ok(())
}

/// Setup network namespace for container
pub async fn setup_network_namespace(container_id: &str) -> Result<()> {
    debug!("🌐 Setting up network namespace for: {}", container_id);

    // Create network namespace
    let mut cmd = Command::new("ip");
    cmd.arg("netns").arg("add").arg(container_id);

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Failed to create network namespace: {}", stderr).into());
    }

    // TODO: Setup veth pair and bridge networking
    debug!("✅ Network namespace created for: {}", container_id);
    Ok(())
}

/// Cleanup network namespace
pub async fn cleanup_network_namespace(container_id: &str) -> Result<()> {
    debug!("🧹 Cleaning up network namespace for: {}", container_id);

    let mut cmd = Command::new("ip");
    cmd.arg("netns").arg("delete").arg(container_id);

    let output = cmd.output().await?;

    if !output.status.success() {
        // Don't fail if namespace doesn't exist
        debug!("Network namespace cleanup completed (may not have existed)");
    }

    Ok(())
}

/// Setup cgroups for resource limits
pub async fn setup_cgroups(container_id: &str, limits: &ResourceLimits) -> Result<()> {
    debug!("📊 Setting up cgroups for: {}", container_id);

    let cgroup_path = format!("/sys/fs/cgroup/bolt/{}", container_id);

    // Create cgroup directory
    std::fs::create_dir_all(&cgroup_path)
        .context("Failed to create cgroup directory")?;

    // Set memory limit
    if let Some(memory) = limits.memory {
        let memory_path = format!("{}/memory.max", cgroup_path);
        std::fs::write(memory_path, memory.to_string())
            .context("Failed to set memory limit")?;
    }

    // Set CPU limits
    if let Some(cpu_quota) = limits.cpu_quota {
        if let Some(cpu_period) = limits.cpu_period {
            let quota_path = format!("{}/cpu.max", cgroup_path);
            let limit = format!("{} {}", cpu_quota, cpu_period);
            std::fs::write(quota_path, limit)
                .context("Failed to set CPU quota")?;
        }
    }

    debug!("✅ Cgroups configured for: {}", container_id);
    Ok(())
}

/// Cleanup cgroups
pub async fn cleanup_cgroups(container_id: &str) -> Result<()> {
    debug!("🧹 Cleaning up cgroups for: {}", container_id);

    let cgroup_path = format!("/sys/fs/cgroup/bolt/{}", container_id);

    if std::path::Path::new(&cgroup_path).exists() {
        std::fs::remove_dir_all(&cgroup_path)
            .context("Failed to cleanup cgroup directory")?;
    }

    Ok(())
}