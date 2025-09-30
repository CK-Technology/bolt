use crate::{BoltError, Result};
use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

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
    pub detach: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
}

/// Execute a container using native OCI runtime
pub async fn execute_container(
    state: &ContainerState,
    spec: &oci_spec::runtime::Spec,
) -> Result<u32> {
    info!("🚀 Executing container: {}", state.id);

    // Write the OCI spec to the bundle directory
    let spec_path = state.bundle_path.join("config.json");
    let spec_json = serde_json::to_string_pretty(spec)?;
    std::fs::write(&spec_path, spec_json).context("Failed to write OCI runtime spec")?;

    // Try to use runc or crun for OCI runtime
    let runtime = detect_oci_runtime().await?;

    debug!("Using OCI runtime: {}", runtime);
    debug!("Bundle path: {}", state.bundle_path.display());

    if state.config.detach {
        // Create container with OCI runtime
        let mut cmd = Command::new(&runtime);
        cmd.arg("create")
            .arg("--bundle")
            .arg(&state.bundle_path)
            .arg(&state.id);

        let output = cmd
            .output()
            .await
            .context("Failed to create container with OCI runtime")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Container creation failed: {}", stderr).into());
        }

        // Start the container
        let mut start_cmd = Command::new(&runtime);
        start_cmd.arg("start").arg(&state.id);

        let start_output = start_cmd
            .output()
            .await
            .context("Failed to start container")?;

        if !start_output.status.success() {
            let stderr = String::from_utf8_lossy(&start_output.stderr);
            return Err(anyhow!("Container start failed: {}", stderr).into());
        }

        // Get container PID
        let pid = get_container_pid(&runtime, &state.id).await?;

        info!("✅ Container started with PID: {}", pid);
        Ok(pid)
    } else {
        // Run container in attached mode (like `docker run`)
        let mut run_cmd = Command::new(&runtime);
        run_cmd
            .arg("run")
            .arg("--bundle")
            .arg(&state.bundle_path)
            .arg(&state.id)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = run_cmd
            .status()
            .await
            .context("Failed to run container in attached mode")?;

        let exit_code = status.code().unwrap_or_default();
        info!("✅ Container {} exited with status {}", state.id, exit_code);

        // Return a dummy PID for attached mode since we don't track it
        Ok(0)
    }
}

/// Detect available OCI runtime
async fn detect_oci_runtime() -> Result<String> {
    // Try runc first (most common)
    if Command::new("runc").arg("--version").output().await.is_ok() {
        return Ok("runc".to_string());
    }

    // Try crun (faster alternative)
    if Command::new("crun").arg("--version").output().await.is_ok() {
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

    let output = cmd
        .output()
        .await
        .context("Failed to get container state")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Failed to get container state: {}", stderr).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse JSON output to get PID
    let state_json: serde_json::Value =
        serde_json::from_str(&stdout).context("Failed to parse container state JSON")?;

    let pid = state_json
        .get("pid")
        .and_then(|p| p.as_u64())
        .ok_or_else(|| anyhow!("Container PID not found in state"))?;

    Ok(pid as u32)
}

/// Check if a container exists
pub async fn container_exists(runtime: &str, container_id: &str) -> bool {
    let mut cmd = Command::new(runtime);
    cmd.arg("state").arg(container_id);

    match cmd.output().await {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// List all containers managed by the runtime
pub async fn list_runtime_containers(runtime: &str) -> Result<Vec<String>> {
    let mut cmd = Command::new(runtime);
    cmd.arg("list").arg("--format").arg("json");

    let output = cmd.output().await.context("Failed to list containers")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Failed to list containers: {}", stderr).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut container_ids = Vec::new();

    // Parse each line as JSON (runc outputs one JSON object per line)
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(container) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(id) = container.get("id").and_then(|v| v.as_str()) {
                container_ids.push(id.to_string());
            }
        }
    }

    Ok(container_ids)
}

/// Stop a container gracefully
pub async fn stop_container(runtime: &str, container_id: &str) -> Result<()> {
    info!("🛑 Stopping container: {}", container_id);

    // First, try to kill the container gracefully
    let mut cmd = Command::new(runtime);
    cmd.arg("kill").arg(container_id).arg("TERM");

    let output = cmd
        .output()
        .await
        .context("Failed to send SIGTERM to container")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // If container doesn't exist, that's ok
        if stderr.contains("does not exist") {
            debug!(
                "Container {} already stopped or doesn't exist",
                container_id
            );
            return Ok(());
        }

        warn!("SIGTERM failed, trying SIGKILL: {}", stderr);

        // Force kill if graceful stop failed
        let mut kill_cmd = Command::new(runtime);
        kill_cmd.arg("kill").arg(container_id).arg("KILL");

        let kill_output = kill_cmd
            .output()
            .await
            .context("Failed to send SIGKILL to container")?;

        if !kill_output.status.success() {
            let stderr = String::from_utf8_lossy(&kill_output.stderr);
            if !stderr.contains("does not exist") {
                return Err(anyhow!("Failed to kill container: {}", stderr).into());
            }
        }
    }

    // Wait a moment for the container to stop
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Delete the container
    let mut delete_cmd = Command::new(runtime);
    delete_cmd.arg("delete").arg(container_id);

    let delete_output = delete_cmd
        .output()
        .await
        .context("Failed to delete container")?;

    if !delete_output.status.success() {
        let stderr = String::from_utf8_lossy(&delete_output.stderr);
        // Ignore if container doesn't exist
        if !stderr.contains("does not exist") {
            warn!("Failed to delete container: {}", stderr);
        }
    }

    info!("✅ Container stopped: {}", container_id);
    Ok(())
}

/// Setup network namespace for container
pub async fn setup_network_namespace(container_id: &str) -> Result<()> {
    debug!("🌐 Setting up network namespace for: {}", container_id);

    // Check if we have permissions to create network namespaces
    if !has_network_admin_capability() {
        debug!("Network namespace creation requires CAP_NET_ADMIN - skipping");
        return Ok(());
    }

    // Create network namespace
    let mut cmd = Command::new("ip");
    cmd.arg("netns").arg("add").arg(container_id);

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check if namespace already exists
        if stderr.contains("File exists") {
            debug!("Network namespace already exists for: {}", container_id);
            return Ok(());
        }
        return Err(anyhow!("Failed to create network namespace: {}", stderr).into());
    }

    // Create veth pair for container networking
    let veth_host = format!("veth-h-{}", &container_id[..8]);
    let veth_container = format!("veth-c-{}", &container_id[..8]);

    // Create veth pair
    debug!("Creating veth pair: {} <-> {}", veth_host, veth_container);
    let mut veth_cmd = Command::new("ip");
    veth_cmd
        .arg("link")
        .arg("add")
        .arg(&veth_host)
        .arg("type")
        .arg("veth")
        .arg("peer")
        .arg("name")
        .arg(&veth_container);

    let veth_output = veth_cmd.output().await?;
    if !veth_output.status.success() {
        let stderr = String::from_utf8_lossy(&veth_output.stderr);
        return Err(anyhow!("Failed to create veth pair: {}", stderr).into());
    }

    // Move container end to the namespace
    debug!(
        "Moving {} to network namespace {}",
        veth_container, container_id
    );
    let mut move_cmd = Command::new("ip");
    move_cmd
        .arg("link")
        .arg("set")
        .arg(&veth_container)
        .arg("netns")
        .arg(container_id);

    let move_output = move_cmd.output().await?;
    if !move_output.status.success() {
        let stderr = String::from_utf8_lossy(&move_output.stderr);
        return Err(anyhow!("Failed to move veth to namespace: {}", stderr).into());
    }

    // Attach host veth to the default bridge (br-bolt0)
    let bridge_name = "br-bolt0";
    debug!("Attaching {} to bridge {}", veth_host, bridge_name);

    // First, ensure the bridge exists
    create_default_bridge_if_needed().await?;

    let mut bridge_cmd = Command::new("ip");
    bridge_cmd
        .arg("link")
        .arg("set")
        .arg(&veth_host)
        .arg("master")
        .arg(bridge_name);

    let bridge_output = bridge_cmd.output().await?;
    if !bridge_output.status.success() {
        let stderr = String::from_utf8_lossy(&bridge_output.stderr);
        warn!("Failed to attach veth to bridge: {}", stderr);
    }

    // Bring up the host end
    debug!("Bringing up host veth: {}", veth_host);
    let mut up_cmd = Command::new("ip");
    up_cmd.arg("link").arg("set").arg(&veth_host).arg("up");
    let up_output = up_cmd.output().await?;
    if !up_output.status.success() {
        let stderr = String::from_utf8_lossy(&up_output.stderr);
        warn!("Failed to bring up host veth: {}", stderr);
    }

    // Configure container veth inside namespace
    setup_container_network_interface(container_id, &veth_container).await?;

    // Setup loopback in the container namespace
    let mut lo_cmd = Command::new("ip");
    lo_cmd
        .arg("netns")
        .arg("exec")
        .arg(container_id)
        .arg("ip")
        .arg("link")
        .arg("set")
        .arg("lo")
        .arg("up");
    let _ = lo_cmd.output().await;

    debug!("✅ Network namespace created for: {}", container_id);
    Ok(())
}

/// Create the default Bolt bridge if it doesn't exist
async fn create_default_bridge_if_needed() -> Result<()> {
    let bridge_name = "br-bolt0";

    // Check if bridge already exists
    let mut check_cmd = Command::new("ip");
    check_cmd.arg("link").arg("show").arg(bridge_name);
    let check_output = check_cmd.output().await?;

    if check_output.status.success() {
        debug!("Bridge {} already exists", bridge_name);
        return Ok(());
    }

    info!("🌉 Creating default Bolt bridge: {}", bridge_name);

    // Create bridge
    let mut create_cmd = Command::new("ip");
    create_cmd
        .arg("link")
        .arg("add")
        .arg("name")
        .arg(bridge_name)
        .arg("type")
        .arg("bridge");

    let create_output = create_cmd.output().await?;
    if !create_output.status.success() {
        let stderr = String::from_utf8_lossy(&create_output.stderr);
        if !stderr.contains("File exists") {
            return Err(anyhow!("Failed to create bridge: {}", stderr).into());
        }
    }

    // Configure bridge IP address (172.20.0.1/16)
    let mut addr_cmd = Command::new("ip");
    addr_cmd
        .arg("addr")
        .arg("add")
        .arg("172.20.0.1/16")
        .arg("dev")
        .arg(bridge_name);

    let addr_output = addr_cmd.output().await?;
    if !addr_output.status.success() {
        let stderr = String::from_utf8_lossy(&addr_output.stderr);
        if !stderr.contains("File exists") {
            warn!("Failed to add bridge IP: {}", stderr);
        }
    }

    // Bring up the bridge
    let mut up_cmd = Command::new("ip");
    up_cmd.arg("link").arg("set").arg(bridge_name).arg("up");

    let up_output = up_cmd.output().await?;
    if !up_output.status.success() {
        let stderr = String::from_utf8_lossy(&up_output.stderr);
        warn!("Failed to bring up bridge: {}", stderr);
    }

    info!("✅ Default Bolt bridge created: {}", bridge_name);
    Ok(())
}

/// Setup network interface inside container namespace
async fn setup_container_network_interface(container_id: &str, veth_name: &str) -> Result<()> {
    debug!("Configuring container network interface: {}", veth_name);

    // Generate IP address for container (172.20.x.x/16)
    let container_ip = generate_container_ip(container_id)?;

    // Rename veth to eth0 inside container
    let mut rename_cmd = Command::new("ip");
    rename_cmd
        .arg("netns")
        .arg("exec")
        .arg(container_id)
        .arg("ip")
        .arg("link")
        .arg("set")
        .arg(veth_name)
        .arg("name")
        .arg("eth0");

    let rename_output = rename_cmd.output().await?;
    if !rename_output.status.success() {
        let stderr = String::from_utf8_lossy(&rename_output.stderr);
        warn!("Failed to rename interface to eth0: {}", stderr);
    }

    // Assign IP address to container interface
    let mut ip_cmd = Command::new("ip");
    ip_cmd
        .arg("netns")
        .arg("exec")
        .arg(container_id)
        .arg("ip")
        .arg("addr")
        .arg("add")
        .arg(format!("{}/16", container_ip))
        .arg("dev")
        .arg("eth0");

    let ip_output = ip_cmd.output().await?;
    if !ip_output.status.success() {
        let stderr = String::from_utf8_lossy(&ip_output.stderr);
        warn!("Failed to assign IP to container: {}", stderr);
    }

    // Bring up the container interface
    let mut up_cmd = Command::new("ip");
    up_cmd
        .arg("netns")
        .arg("exec")
        .arg(container_id)
        .arg("ip")
        .arg("link")
        .arg("set")
        .arg("eth0")
        .arg("up");

    let up_output = up_cmd.output().await?;
    if !up_output.status.success() {
        let stderr = String::from_utf8_lossy(&up_output.stderr);
        warn!("Failed to bring up container interface: {}", stderr);
    }

    // Add default route
    let mut route_cmd = Command::new("ip");
    route_cmd
        .arg("netns")
        .arg("exec")
        .arg(container_id)
        .arg("ip")
        .arg("route")
        .arg("add")
        .arg("default")
        .arg("via")
        .arg("172.20.0.1");

    let route_output = route_cmd.output().await?;
    if !route_output.status.success() {
        let stderr = String::from_utf8_lossy(&route_output.stderr);
        warn!("Failed to add default route: {}", stderr);
    }

    info!(
        "✅ Container network configured: {} -> {}",
        container_id, container_ip
    );
    Ok(())
}

/// Generate a unique IP address for the container
fn generate_container_ip(container_id: &str) -> Result<String> {
    // Use a hash of the container ID to generate consistent IP addresses
    let mut hasher = DefaultHasher::new();
    container_id.hash(&mut hasher);
    let hash = hasher.finish();

    // Generate IP in 172.20.x.x range (avoiding .0.x and .255.x)
    let third_octet = ((hash >> 8) % 254) + 1; // 1-254
    let fourth_octet = (hash % 254) + 2; // 2-255 (avoid .1 which is gateway)

    Ok(format!("172.20.{}.{}", third_octet, fourth_octet))
}

fn has_network_admin_capability() -> bool {
    // Check if we're root or have CAP_NET_ADMIN
    #[cfg(unix)]
    {
        use nix::unistd::Uid;
        if Uid::current().is_root() {
            return true;
        }
    }

    // For non-root, check capabilities (would need caps crate)
    false
}

/// Cleanup network namespace
pub async fn cleanup_network_namespace(container_id: &str) -> Result<()> {
    debug!("🧹 Cleaning up network namespace for: {}", container_id);

    // Clean up veth pair if it exists
    let veth_host = format!("veth-h-{}", &container_id[..8]);
    let mut veth_cleanup = Command::new("ip");
    veth_cleanup.arg("link").arg("delete").arg(&veth_host);
    let _ = veth_cleanup.output().await; // Ignore errors

    // Delete the network namespace
    let mut cmd = Command::new("ip");
    cmd.arg("netns").arg("delete").arg(container_id);

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Don't fail if namespace doesn't exist
        if !stderr.contains("No such file") && !stderr.contains("does not exist") {
            debug!("Network namespace cleanup warning: {}", stderr);
        }
    }

    Ok(())
}

/// Setup cgroups v2 for resource limits
pub async fn setup_cgroups(container_id: &str, limits: &ResourceLimits) -> Result<()> {
    debug!("📊 Setting up cgroups v2 for: {}", container_id);

    // Check if we're running with appropriate permissions
    if !is_cgroup_available() {
        warn!("Cgroups not available or insufficient permissions - skipping resource limits");
        return Ok(());
    }

    let cgroup_base = get_cgroup_base_path();
    let cgroup_path = cgroup_base.join("bolt").join(container_id);

    // Create cgroup directory
    std::fs::create_dir_all(&cgroup_path)
        .with_context(|| format!("Failed to create cgroup directory at {:?}", cgroup_path))?;

    // Enable required controllers
    enable_cgroup_controllers(&cgroup_base)?;

    // Set memory limit
    if let Some(memory) = limits.memory {
        let memory_path = cgroup_path.join("memory.max");
        std::fs::write(&memory_path, memory.to_string())
            .with_context(|| format!("Failed to set memory limit at {:?}", memory_path))?;

        // Also set swap limit to prevent swap usage
        let swap_path = cgroup_path.join("memory.swap.max");
        if swap_path.exists() {
            let _ = std::fs::write(&swap_path, "0");
        }
    }

    // Set CPU limits
    if let Some(cpu_quota) = limits.cpu_quota {
        if let Some(cpu_period) = limits.cpu_period {
            let cpu_path = cgroup_path.join("cpu.max");
            let limit = format!("{} {}", cpu_quota, cpu_period);
            std::fs::write(&cpu_path, limit)
                .with_context(|| format!("Failed to set CPU quota at {:?}", cpu_path))?;
        }
    }

    // Set CPU shares for relative priority
    if let Some(cpu_shares) = limits.cpu_shares {
        let weight_path = cgroup_path.join("cpu.weight");
        // Convert shares to weight (shares: 2-262144, weight: 1-10000)
        let weight = (cpu_shares * 10000 / 262144).max(1).min(10000);
        if weight_path.exists() {
            let _ = std::fs::write(&weight_path, weight.to_string());
        }
    }

    debug!("✅ Cgroups v2 configured for: {}", container_id);
    Ok(())
}

fn is_cgroup_available() -> bool {
    // Check if cgroup v2 is mounted
    std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").exists()
}

fn get_cgroup_base_path() -> std::path::PathBuf {
    // For rootless containers, use systemd user slice if available
    if let Ok(user_slice) = std::env::var("CGROUP_PATH") {
        return std::path::PathBuf::from(user_slice);
    }

    // Default to system cgroup path
    std::path::PathBuf::from("/sys/fs/cgroup")
}

fn enable_cgroup_controllers(base_path: &std::path::Path) -> Result<()> {
    let subtree_control = base_path.join("cgroup.subtree_control");

    if subtree_control.exists() {
        // Try to enable memory and cpu controllers
        let controllers = "+memory +cpu";
        match std::fs::write(&subtree_control, controllers) {
            Ok(_) => debug!("Enabled cgroup controllers: {}", controllers),
            Err(e) => debug!(
                "Could not enable controllers (may already be enabled): {}",
                e
            ),
        }
    }

    Ok(())
}

/// Cleanup cgroups
pub async fn cleanup_cgroups(container_id: &str) -> Result<()> {
    debug!("🧹 Cleaning up cgroups for: {}", container_id);

    let cgroup_base = get_cgroup_base_path();
    let cgroup_path = cgroup_base.join("bolt").join(container_id);

    if cgroup_path.exists() {
        // First, try to kill any remaining processes in the cgroup
        let procs_path = cgroup_path.join("cgroup.procs");
        if procs_path.exists() {
            if let Ok(procs) = std::fs::read_to_string(&procs_path) {
                for pid_str in procs.lines() {
                    if let Ok(pid) = pid_str.parse::<i32>() {
                        let _ = nix::sys::signal::kill(
                            nix::unistd::Pid::from_raw(pid),
                            nix::sys::signal::Signal::SIGKILL,
                        );
                    }
                }
            }
        }

        // Wait a moment for processes to exit
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Now remove the cgroup directory
        match std::fs::remove_dir(&cgroup_path) {
            Ok(_) => debug!("Removed cgroup directory: {:?}", cgroup_path),
            Err(e) => {
                // Try rmdir on parent if this was the last container
                debug!("Could not remove cgroup (may have subgroups): {}", e);
            }
        }
    }

    Ok(())
}
