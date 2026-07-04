use crate::{BoltError, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::time::SystemTime;
use tokio::process::Command as AsyncCommand;
use tracing::{debug, info, warn};
use unified::ContainerRunOptions;

pub mod amd_metrics;
pub mod environment;
pub mod gpu_integration;
pub mod gpu_scheduler;
pub mod hardware_detection;
pub mod input;
pub mod native;
pub mod networking;
pub mod nvbind;
pub mod oci;
pub mod performance;
pub mod quick_sync;
pub mod security;
pub mod state;
pub mod storage;
pub mod unified;

#[cfg(feature = "gaming")]
pub mod gpu;

// Helper function to detect available container runtime
pub async fn detect_container_runtime() -> Result<String> {
    // Try podman first (preferred for rootless)
    if AsyncCommand::new("podman")
        .arg("--version")
        .output()
        .await
        .is_ok()
    {
        return Ok("podman".to_string());
    }

    // Fall back to docker
    if AsyncCommand::new("docker")
        .arg("--version")
        .output()
        .await
        .is_ok()
    {
        return Ok("docker".to_string());
    }

    Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
        message: "No container runtime found (podman or docker required)".to_string(),
    }))
}

pub async fn run_container(
    image: &str,
    name: Option<&str>,
    ports: &[String],
    env: &[String],
    volumes: &[String],
    detach: bool,
) -> Result<()> {
    info!("🔥 Running container with image: {}", image);

    if image.starts_with("bolt://") {
        info!("Using Bolt native image format");
        run_bolt_capsule(image, name, ports, env, volumes, detach).await
    } else {
        info!("Using OCI image format");
        let runtime = detect_container_runtime().await?;
        run_oci_container_delegate(&runtime, image, name, ports, env, volumes, detach).await?;
        Ok(())
    }
}

pub async fn run_bolt_capsule(
    image: &str,
    name: Option<&str>,
    _ports: &[String],
    _env: &[String],
    _volumes: &[String],
    _detach: bool,
) -> Result<()> {
    let capsule_name = image.strip_prefix("bolt://").unwrap_or(image);
    info!("🔧 Creating Bolt capsule: {}", capsule_name);

    match name {
        Some(name) => info!("Container name: {}", name),
        None => info!("Auto-generating container name"),
    }

    warn!("Bolt capsules not yet implemented - using OCI fallback");
    Ok(())
}

// Delegation functions for fallback compatibility
pub async fn run_oci_container_delegate(
    runtime: &str,
    image: &str,
    name: Option<&str>,
    ports: &[String],
    env: &[String],
    volumes: &[String],
    detach: bool,
) -> Result<String> {
    run_oci_container_delegate_with_options(
        runtime,
        image,
        name,
        ports,
        env,
        volumes,
        detach,
        &ContainerRunOptions::default(),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn run_oci_container_delegate_with_options(
    runtime: &str,
    image: &str,
    name: Option<&str>,
    ports: &[String],
    env: &[String],
    volumes: &[String],
    detach: bool,
    options: &ContainerRunOptions,
) -> Result<String> {
    info!("🐳 Starting OCI container: {}", image);

    debug!("Container config:");
    debug!("  Image: {}", image);
    if let Some(name) = name {
        debug!("  Name: {}", name);
    }
    debug!("  Ports: {:?}", ports);
    debug!("  Environment: {:?}", env);
    debug!("  Volumes: {:?}", volumes);
    debug!("  Detached: {}", detach);

    // Use provided runtime
    let mut cmd = AsyncCommand::new(runtime);
    cmd.arg("run");

    if options.rm {
        cmd.arg("--rm");
    }

    if detach {
        cmd.arg("-d");
    }

    if options.interactive {
        cmd.arg("-i");
    }

    if options.tty {
        cmd.arg("-t");
    }

    if let Some(name) = name {
        cmd.arg("--name").arg(name);
    }

    if let Some(ref workdir) = options.working_dir {
        cmd.arg("--workdir").arg(workdir);
    }

    if let Some(ref user) = options.user {
        cmd.arg("--user").arg(user);
    }

    if let Some(ref hostname) = options.hostname {
        cmd.arg("--hostname").arg(hostname);
    }

    if let Some(ref entrypoint) = options.entrypoint
        && let Some(first) = entrypoint.first()
    {
        cmd.arg("--entrypoint").arg(first);
    }

    if let Some(cpus) = options.cpus {
        cmd.arg("--cpus").arg(cpus.to_string());
    }

    if let Some(ref memory) = options.memory {
        cmd.arg("--memory").arg(memory);
    }

    if let Some(ref network) = options.network {
        cmd.arg("--network").arg(network);
    }

    for cap in &options.cap_add {
        cmd.arg("--cap-add").arg(cap);
    }

    for cap in &options.cap_drop {
        cmd.arg("--cap-drop").arg(cap);
    }

    if options.privileged {
        cmd.arg("--privileged");
    }

    // Add port mappings
    for port in ports {
        cmd.arg("-p").arg(port);
    }

    // Add environment variables
    for env_var in env {
        cmd.arg("-e").arg(env_var);
    }

    // Add volume mounts
    for volume in volumes {
        cmd.arg("-v").arg(volume);
    }

    cmd.arg(image);
    if let Some(ref command) = options.command {
        cmd.args(command);
    }

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(
            crate::error::RuntimeError::StartFailed {
                reason: format!("Failed to run container: {}", stderr),
            },
        ));
    }

    let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    info!("✅ Container started: {}", container_id);

    Ok(container_id)
}

pub async fn stop_container_delegate(runtime: &str, id: &str) -> Result<()> {
    info!("🛑 Stopping container: {}", id);

    let mut cmd = AsyncCommand::new(runtime);
    cmd.arg("stop").arg(id);

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(
            crate::error::RuntimeError::StartFailed {
                reason: format!("Failed to stop container: {}", stderr),
            },
        ));
    }

    info!("✅ Container stopped: {}", id);
    Ok(())
}

pub async fn remove_container_delegate(runtime: &str, id: &str, force: bool) -> Result<()> {
    info!("🗑️  Removing container: {}", id);

    let mut cmd = AsyncCommand::new(runtime);
    cmd.arg("rm");
    if force {
        cmd.arg("-f");
    }
    cmd.arg(id);

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(
            crate::error::RuntimeError::StartFailed {
                reason: format!("Failed to remove container: {}", stderr),
            },
        ));
    }

    info!("✅ Container removed: {}", id);
    Ok(())
}

pub async fn list_containers_delegate(
    runtime: &str,
    all: bool,
) -> Result<Vec<native::NativeContainerInfo>> {
    info!("📋 Listing containers...");

    let mut cmd = AsyncCommand::new(runtime);
    cmd.arg("ps");
    if all {
        cmd.arg("-a");
    }
    cmd.arg("--format").arg("json");

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(
            crate::error::RuntimeError::StartFailed {
                reason: format!("Failed to list containers: {}", stderr),
            },
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut containers = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => {
                let id = value
                    .get("ID")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();

                let name = value
                    .get("Names")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let image = value
                    .get("Image")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();

                let created = value
                    .get("CreatedAt")
                    .and_then(|v| v.as_str())
                    .and_then(|s| {
                        DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S %z %Z")
                            .or_else(|_| DateTime::parse_from_rfc3339(s))
                            .map(|dt| dt.with_timezone(&Utc).into())
                            .ok()
                    })
                    .unwrap_or_else(SystemTime::now);

                let status_raw = value
                    .get("Status")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();

                let status = if status_raw.to_lowercase().contains("up")
                    || status_raw.to_lowercase().contains("running")
                {
                    native::ContainerStatus::Running
                } else if let Some(code) = status_raw
                    .split(['(', ')'])
                    .filter_map(|segment| segment.trim().strip_prefix("Exited "))
                    .filter_map(|segment| segment.parse::<i32>().ok())
                    .next()
                {
                    native::ContainerStatus::Exited(code)
                } else if status_raw.to_lowercase().contains("paused") {
                    native::ContainerStatus::Paused
                } else if status_raw.to_lowercase().contains("created") {
                    native::ContainerStatus::Created
                } else {
                    native::ContainerStatus::Stopped
                };

                let ports = value
                    .get("Ports")
                    .and_then(|v| v.as_str())
                    .map(|ports| {
                        ports
                            .split(',')
                            .map(|p| p.trim().to_string())
                            .filter(|p| !p.is_empty())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                containers.push(native::NativeContainerInfo {
                    id,
                    name,
                    image,
                    status,
                    created,
                    ports,
                    pid: None,
                });
            }
            Err(err) => {
                warn!("Failed to parse container JSON line: {}", err);
            }
        }
    }

    Ok(containers)
}

pub async fn pull_image_delegate(runtime: &str, image: &str) -> Result<()> {
    info!("⬇️  Pulling image: {}", image);

    let mut cmd = AsyncCommand::new(runtime);
    cmd.arg("pull").arg(image);

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(
            crate::error::RuntimeError::StartFailed {
                reason: format!("Failed to pull image: {}", stderr),
            },
        ));
    }

    info!("✅ Image pulled: {}", image);
    Ok(())
}

pub async fn build_image(path: &str, tag: Option<&str>, dockerfile: &str) -> Result<()> {
    info!("🔨 Building image from path: {}", path);

    let runtime = detect_container_runtime().await?;
    build_image_delegate(&runtime, path, tag, dockerfile).await
}

pub async fn build_image_delegate(
    runtime: &str,
    path: &str,
    tag: Option<&str>,
    dockerfile: &str,
) -> Result<()> {
    debug!("Building image using {} runtime", runtime);
    info!("🔨 Building image from path: {}", path);
    debug!("Dockerfile: {}", dockerfile);
    if let Some(tag) = tag {
        debug!("Tag: {}", tag);
    }

    let runtime = detect_container_runtime().await?;
    let mut cmd = AsyncCommand::new(&runtime);
    cmd.arg("build");

    if let Some(tag) = tag {
        cmd.arg("-t").arg(tag);
    }

    cmd.arg("-f").arg(dockerfile);
    cmd.arg(path);

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
            message: format!("Failed to build image: {}", stderr),
        }));
    }

    info!("✅ Image built successfully");
    Ok(())
}

pub async fn pull_image(image: &str) -> Result<()> {
    info!("⬇️  Pulling image: {}", image);

    let runtime = detect_container_runtime().await?;
    let mut cmd = AsyncCommand::new(&runtime);
    cmd.arg("pull").arg(image);

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(
            crate::error::RuntimeError::ImagePullFailed {
                image: format!("Failed to pull image: {}", stderr),
            },
        ));
    }

    info!("✅ Image pulled successfully: {}", image);
    Ok(())
}

pub async fn push_image(image: &str) -> Result<()> {
    info!("⬆️  Pushing image: {}", image);

    let runtime = detect_container_runtime().await?;
    let mut cmd = AsyncCommand::new(&runtime);
    cmd.arg("push").arg(image);

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
            message: format!("Failed to push image: {}", stderr),
        }));
    }

    info!("✅ Image pushed successfully: {}", image);
    Ok(())
}

pub async fn list_containers(all: bool) -> Result<()> {
    info!("📋 Listing containers (all: {})", all);

    let runtime = detect_container_runtime().await?;
    let mut cmd = AsyncCommand::new(&runtime);
    cmd.arg("ps");

    if all {
        cmd.arg("-a");
    }

    cmd.arg("--format").arg("table {{.ID}}\t{{.Image}}\t{{.Command}}\t{{.CreatedAt}}\t{{.Status}}\t{{.Ports}}\t{{.Names}}");

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
            message: format!("Failed to list containers: {}", stderr),
        }));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("{}", stdout);
    Ok(())
}

// API-only functions for library usage
use crate::ContainerInfo;

pub async fn list_containers_info(all: bool) -> Result<Vec<ContainerInfo>> {
    info!("📋 Listing containers (all: {})", all);

    let runtime = detect_container_runtime().await?;
    let mut cmd = AsyncCommand::new(&runtime);
    cmd.arg("ps");

    if all {
        cmd.arg("-a");
    }

    cmd.arg("--format").arg("json");

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
            message: format!("Failed to list containers: {}", stderr),
        }));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut containers = Vec::new();

    // Parse JSON output line by line (podman/docker outputs one JSON object per line)
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
            let container = ContainerInfo {
                id: value
                    .get("Id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: value
                    .get("Names")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                names: vec![
                    value
                        .get("Names")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                ],
                image: value
                    .get("Image")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                image_id: value
                    .get("ImageID")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                labels: value
                    .get("Labels")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default(),
                uptime: value
                    .get("CreatedAt")
                    .and_then(|v| v.as_str())
                    .and_then(|created_str| chrono::DateTime::parse_from_rfc3339(created_str).ok())
                    .map(|created| {
                        let now = chrono::Utc::now();
                        let created_utc = created.with_timezone(&chrono::Utc);
                        let duration = now.signed_duration_since(created_utc);
                        let seconds = duration.num_seconds();
                        format!("{}s", seconds)
                    }),
                command: value
                    .get("Command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/bin/sh")
                    .to_string(),
                created: value
                    .get("CreatedAt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                status: value
                    .get("Status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                ports: value
                    .get("Ports")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect(),
                runtime: value
                    .get("Labels")
                    .and_then(|labels| labels.get("bolt.runtime"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            };
            containers.push(container);
        }
    }

    Ok(containers)
}

pub async fn stop_container(container: &str) -> Result<()> {
    info!("🛑 Stopping container: {}", container);

    let runtime = detect_container_runtime().await?;
    let mut cmd = AsyncCommand::new(&runtime);
    cmd.arg("stop").arg(container);

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
            message: format!("Failed to stop container: {}", stderr),
        }));
    }

    info!("✅ Container stopped: {}", container);
    Ok(())
}

pub async fn remove_container(container: &str, force: bool) -> Result<()> {
    info!("🗑️  Removing container: {} (force: {})", container, force);

    let runtime = detect_container_runtime().await?;
    let mut cmd = AsyncCommand::new(&runtime);
    cmd.arg("rm");

    if force {
        cmd.arg("-f");
    }

    cmd.arg(container);

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
            message: format!("Failed to remove container: {}", stderr),
        }));
    }

    info!("✅ Container removed: {}", container);
    Ok(())
}

pub async fn restart_container(container: &str, timeout: u64) -> Result<()> {
    info!(
        "🔄 Restarting container: {} (timeout: {}s)",
        container, timeout
    );

    let runtime = detect_container_runtime().await?;
    let mut cmd = AsyncCommand::new(&runtime);
    cmd.arg("restart")
        .arg("--time")
        .arg(timeout.to_string())
        .arg(container);

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
            message: format!("Failed to restart container: {}", stderr),
        }));
    }

    info!("✅ Container restarted: {}", container);
    Ok(())
}
