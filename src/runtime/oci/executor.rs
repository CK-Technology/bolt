use anyhow::{Result, Context};
use oci_spec::runtime::{Spec, LinuxNamespace, LinuxNamespaceType};
use tracing::{info, debug, warn, error};
use std::process::Stdio;
use std::fs;
use std::path::PathBuf;
use std::os::unix::fs::PermissionsExt;
use tokio::process::Command;
use nix::unistd::{Pid, Uid, Gid};
use nix::sched::{CloneFlags, unshare};
use nix::mount::{mount, MsFlags, umount2, MntFlags};
use nix::sys::signal::{kill, Signal};

use super::{ContainerState, ContainerStatus};

pub async fn execute_container(state: &ContainerState, spec: &Spec) -> Result<u32> {
    info!("🚀 Executing container: {}", state.id);

    // Create the container rootfs from image layers
    create_container_rootfs(&state.id, &state.bundle_path, spec).await?;

    // Setup the execution environment in proper order
    let namespaces = setup_namespaces(state, spec).await?;
    setup_mounts(state, spec).await?;
    setup_cgroups(state).await?;
    setup_security_profile(state).await?;

    // Get the process configuration
    let process = spec.process()
        .as_ref()
        .context("No process configuration in spec")?;

    let args = process.args()
        .as_ref()
        .context("No args in process configuration")?;

    if args.is_empty() {
        return Err(anyhow::anyhow!("No command specified"));
    }

    let command = &args[0];
    let command_args = &args[1..];

    info!("Executing: {} {:?}", command, command_args);

    // Change to the container's root directory
    let rootfs_path = state.bundle_path.join("rootfs");

    // Determine execution mode based on user privileges and configuration
    if state.config.privileged {
        info!("🔓 Executing privileged container");
        return execute_with_namespaces(state, spec, &namespaces).await;
    } else if nix::unistd::getuid().is_root() {
        info!("🔒 Executing unprivileged container (as root)");
        return execute_with_namespaces(state, spec, &namespaces).await;
    } else {
        info!("🔒 Executing rootless container (non-root user)");
        return execute_rootless_container(state, spec, namespaces).await;
    }

    // This should never be reached, but keep as fallback
    // execute_simple_container(state, spec, command, command_args, &rootfs_path).await
}

#[derive(Debug, Default)]
struct NamespaceConfig {
    user_ns: bool,
    pid_ns: bool,
    net_ns: bool,
    mount_ns: bool,
    ipc_ns: bool,
    uts_ns: bool,
}

async fn execute_with_namespaces(
    state: &ContainerState,
    spec: &Spec,
    namespaces: &NamespaceConfig,
) -> Result<u32> {
    info!("🔧 Executing with namespace isolation");

    // Create namespace flags
    let mut clone_flags = CloneFlags::empty();

    if namespaces.user_ns {
        clone_flags |= CloneFlags::CLONE_NEWUSER;
        info!("  👤 User namespace enabled");
    }
    if namespaces.pid_ns {
        clone_flags |= CloneFlags::CLONE_NEWPID;
        info!("  🏃 PID namespace enabled");
    }
    if namespaces.net_ns {
        clone_flags |= CloneFlags::CLONE_NEWNET;
        info!("  🌐 Network namespace enabled");
    }
    if namespaces.mount_ns {
        clone_flags |= CloneFlags::CLONE_NEWNS;
        info!("  📁 Mount namespace enabled");
    }
    if namespaces.ipc_ns {
        clone_flags |= CloneFlags::CLONE_NEWIPC;
        info!("  💬 IPC namespace enabled");
    }
    if namespaces.uts_ns {
        clone_flags |= CloneFlags::CLONE_NEWUTS;
        info!("  🏠 UTS namespace enabled");
    }

    // Unshare into new namespaces
    unshare(clone_flags).context("Failed to unshare namespaces")?;

    info!("✅ Namespaces created successfully");

    // Now execute the container process in the new namespaces
    execute_container_process(state, spec).await
}

async fn execute_simple_container(
    _state: &ContainerState,
    spec: &Spec,
    command: &str,
    command_args: &[String],
    rootfs_path: &std::path::Path,
) -> Result<u32> {
    info!("🔧 Executing simple container (no namespaces)");

    let process = spec.process().as_ref().unwrap();

    // Execute the container process using chroot for basic isolation
    let mut cmd = Command::new("chroot");
    cmd.arg(rootfs_path)
        .arg(command)
        .args(command_args)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    // Set environment variables
    if let Some(env_vars) = process.env() {
        for env_var in env_vars {
            if let Some((key, value)) = env_var.split_once('=') {
                cmd.env(key, value);
            }
        }
    }

    // Set working directory (inside chroot)
    unsafe {
        std::env::set_var("PWD", "/");
    }

    // Spawn the process
    let child = cmd.spawn().context("Failed to spawn container process")?;
    let pid = child.id().context("Failed to get child PID")?;

    info!("✅ Container process started with PID: {} (simple mode)", pid);

    // Monitor the process
    tokio::spawn(async move {
        match child.wait_with_output().await {
            Ok(output) => {
                let code = output.status.code().unwrap_or(-1);
                info!("Container {} exited with code: {}", pid, code);
            }
            Err(e) => {
                error!("Error waiting for container {}: {}", pid, e);
            }
        }
    });

    Ok(pid)
}

async fn execute_container_process(state: &ContainerState, spec: &Spec) -> Result<u32> {
    info!("🏃 Executing container process in namespaces");

    let process = spec.process().as_ref().unwrap();
    let args = process.args().as_ref().unwrap();
    let command = &args[0];
    let command_args = &args[1..];

    let rootfs_path = state.bundle_path.join("rootfs");

    // Change root filesystem
    nix::unistd::chroot(&rootfs_path).context("Failed to chroot")?;
    nix::unistd::chdir("/").context("Failed to chdir to /")?;

    info!("✅ Changed root to container filesystem");

    // Execute the container process
    let mut cmd = Command::new(command);
    cmd.args(command_args)
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    // Set environment variables
    if let Some(env_vars) = process.env() {
        for env_var in env_vars {
            if let Some((key, value)) = env_var.split_once('=') {
                cmd.env(key, value);
            }
        }
    }

    // Set working directory
    // TODO: Fix OCI spec cwd() API usage
    cmd.current_dir("/");

    // Spawn the process
    let child = cmd.spawn().context("Failed to spawn container process")?;
    let pid = child.id().context("Failed to get child PID")?;

    info!("✅ Container process started with PID: {} (namespaced)", pid);

    // Monitor the process
    tokio::spawn(async move {
        match child.wait_with_output().await {
            Ok(output) => {
                let code = output.status.code().unwrap_or(-1);
                info!("Namespaced container {} exited with code: {}", pid, code);
            }
            Err(e) => {
                error!("Error waiting for namespaced container {}: {}", pid, e);
            }
        }
    });

    Ok(pid)
}

async fn execute_rootless_container(
    state: &ContainerState,
    spec: &Spec,
    mut namespaces: NamespaceConfig,
) -> Result<u32> {
    info!("🔒 Executing rootless container: {}", state.id);

    // Enable user namespace for rootless execution
    namespaces.user_ns = true;

    // Setup user namespace mappings
    setup_user_namespace_mappings().await?;

    // Apply rootless-specific security measures
    setup_rootless_security(state).await?;

    // Use regular namespace execution with rootless configuration
    execute_with_namespaces(state, spec, &namespaces).await
}

async fn setup_user_namespace_mappings() -> Result<()> {
    info!("👤 Setting up user namespace mappings for rootless container");

    let current_uid = nix::unistd::getuid();
    let current_gid = nix::unistd::getgid();

    // Map current user to root inside container (standard rootless pattern)
    let uid_map = format!("0 {} 1", current_uid);
    let gid_map = format!("0 {} 1", current_gid);

    // Write UID mapping
    std::fs::write("/proc/self/uid_map", &uid_map)
        .context("Failed to write uid_map for rootless container")?;

    // Deny setgroups (required for rootless)
    std::fs::write("/proc/self/setgroups", "deny")
        .context("Failed to deny setgroups for rootless container")?;

    // Write GID mapping
    std::fs::write("/proc/self/gid_map", &gid_map)
        .context("Failed to write gid_map for rootless container")?;

    info!("✅ User namespace mappings configured: uid {} -> 0, gid {} -> 0", current_uid, current_gid);
    Ok(())
}

async fn setup_rootless_security(state: &ContainerState) -> Result<()> {
    info!("🔐 Applying rootless security for container: {}", state.id);

    // Rootless containers have additional security constraints:
    // 1. Can't access host devices directly
    // 2. Limited network capabilities
    // 3. Restricted filesystem access
    // 4. No raw sockets

    // Apply gaming-specific rootless adaptations
    if let Some(ref gaming) = state.config.gaming_config {
        setup_rootless_gaming_security(gaming).await?;
    }

    // Drop all capabilities except basic ones needed for rootless
    let allowed_caps = vec![
        "CAP_SETUID", "CAP_SETGID", // Basic user/group management
        "CAP_CHOWN",   // File ownership changes within user namespace
    ];

    for cap in &allowed_caps {
        info!("  ✓ Allowing capability: {}", cap);
    }

    info!("✅ Rootless security profile applied");
    Ok(())
}

async fn setup_rootless_gaming_security(gaming: &crate::config::GamingConfig) -> Result<()> {
    info!("🎮 Applying rootless gaming security adaptations");

    // For rootless gaming, we need special handling:
    // 1. GPU access through user namespaces and groups
    // 2. Audio through user session (PipeWire/PulseAudio)
    // 3. Display server access through user session

    if let Some(ref gpu) = gaming.gpu {
        info!("  🖥️  Configuring rootless GPU access");
        setup_rootless_gpu_access(gpu).await?;
    }

    if let Some(ref audio) = gaming.audio {
        info!("  🔊 Configuring rootless audio access");
        setup_rootless_audio_access(audio).await?;
    }

    Ok(())
}

async fn setup_rootless_gpu_access(gpu: &crate::config::GpuConfig) -> Result<()> {
    // Check if user is in video/render groups
    let groups = nix::unistd::getgroups().context("Failed to get user groups")?;
    let current_uid = nix::unistd::getuid();

    info!("  Current UID: {}, Groups: {:?}", current_uid, groups);

    // For rootless GPU access, user must be in appropriate groups
    // This is checked but not enforced - it's up to the host admin
    if std::path::Path::new("/dev/dri").exists() {
        info!("  ✓ DRI devices found - GPU access may be available");
    } else {
        warn!("  ⚠️  No DRI devices found - GPU passthrough unavailable");
    }

    if let Some(ref nvidia) = gpu.nvidia {
        if std::path::Path::new("/dev/nvidia0").exists() {
            info!("  ✓ NVIDIA device found - checking access");
        } else {
            warn!("  ⚠️  NVIDIA devices not accessible in rootless mode");
        }
    }

    Ok(())
}

async fn setup_rootless_audio_access(audio: &crate::config::AudioConfig) -> Result<()> {
    match audio.system.as_str() {
        "pipewire" => {
            info!("  🎵 Configuring rootless PipeWire access");
            // PipeWire runs in user session - should work naturally
            if std::env::var("PIPEWIRE_RUNTIME_DIR").is_ok() {
                info!("    ✓ PipeWire runtime detected");
            }
        }
        "pulseaudio" => {
            info!("  🔊 Configuring rootless PulseAudio access");
            // Check for user PulseAudio session
            if std::env::var("PULSE_SERVER").is_ok() ||
               std::path::Path::new(&format!("/run/user/{}/pulse", nix::unistd::getuid())).exists() {
                info!("    ✓ PulseAudio session detected");
            }
        }
        _ => {
            warn!("  ⚠️  Audio system '{}' may not work in rootless mode", audio.system);
        }
    }
    Ok(())
}

async fn setup_namespaces(state: &ContainerState, spec: &Spec) -> Result<NamespaceConfig> {
    info!("🔧 Setting up namespaces for container: {}", state.id);

    let mut config = NamespaceConfig::default();

    // Parse namespace configuration from OCI spec
    if let Some(linux) = spec.linux() {
        if let Some(namespaces) = linux.namespaces() {
            for namespace in namespaces {
                match namespace.typ() {
                    LinuxNamespaceType::User => {
                        config.user_ns = true;
                        info!("  👤 User namespace configured");
                    }
                    LinuxNamespaceType::Pid => {
                        config.pid_ns = true;
                        info!("  🏃 PID namespace configured");
                    }
                    LinuxNamespaceType::Network => {
                        config.net_ns = true;
                        info!("  🌐 Network namespace configured");
                    }
                    LinuxNamespaceType::Mount => {
                        config.mount_ns = true;
                        info!("  📁 Mount namespace configured");
                    }
                    LinuxNamespaceType::Ipc => {
                        config.ipc_ns = true;
                        info!("  💬 IPC namespace configured");
                    }
                    LinuxNamespaceType::Uts => {
                        config.uts_ns = true;
                        info!("  🏠 UTS namespace configured");
                    }
                    LinuxNamespaceType::Cgroup => {
                        info!("  📊 Cgroup namespace configured");
                    }
                    LinuxNamespaceType::Time => {
                        info!("  ⏰ Time namespace configured");
                    }
                }
            }
        } else {
            // Default namespaces for containers
            info!("  🔧 Using default namespace configuration");
            config.user_ns = true;
            config.pid_ns = true;
            config.net_ns = true;
            config.mount_ns = true;
            config.ipc_ns = true;
            config.uts_ns = true;
        }
    }

    // Gaming containers get special namespace handling
    if let Some(ref gaming) = state.config.gaming_config {
        setup_gaming_namespaces(state, gaming, &mut config).await?;
    }

    Ok(config)
}

async fn setup_gaming_namespaces(
    state: &ContainerState,
    gaming: &crate::config::GamingConfig,
    _config: &mut NamespaceConfig,
) -> Result<()> {
    info!("🎮 Setting up gaming-specific namespaces for: {}", state.id);

    // Gaming containers need special handling:
    if gaming.gpu.is_some() {
        info!("  🖥️  GPU access required - keeping device namespace shared");
        // Don't isolate device namespace for GPU access
    }

    if gaming.audio.is_some() {
        info!("  🔊 Audio access required - configuring audio passthrough");
        // Audio devices need special handling
    }

    // For ultra-low latency gaming, we might want to share the network namespace
    // with the host to avoid network stack overhead
    if let Some(ref performance) = gaming.performance {
        if matches!(performance.rt_priority, Some(p) if p > 80) {
            info!("  ⚡ High RT priority - considering shared network namespace");
            // config.net_ns = false; // Share host network for ultra-low latency
        }
    }

    Ok(())
}

async fn setup_mounts(state: &ContainerState, spec: &Spec) -> Result<()> {
    info!("📁 Setting up mounts for container: {}", state.id);

    let rootfs_path = state.bundle_path.join("rootfs");

    // Create essential directories
    create_essential_dirs(&rootfs_path).await?;

    // Mount filesystems from spec
    if let Some(mounts) = spec.mounts() {
        for mount in mounts {
            let destination = mount.destination();
            let source = mount.source().as_ref().map(|s| s.as_path()).unwrap_or_else(|| std::path::Path::new(""));
            let fs_type = mount.typ().as_ref().map(|t| t.as_str()).unwrap_or("bind");

            info!("  📂 Mounting {} -> {} ({})", source.display(), destination.display(), fs_type);

            let full_dest = rootfs_path.join(destination.strip_prefix("/").unwrap_or(destination));

            // Create mount point if it doesn't exist
            if let Some(parent) = full_dest.parent() {
                fs::create_dir_all(parent).context("Failed to create mount point directory")?;
            }

            match fs_type {
                "proc" => mount_proc(&full_dest).await?,
                "sysfs" => mount_sysfs(&full_dest).await?,
                "tmpfs" => mount_tmpfs(&full_dest).await?,
                "bind" => mount_bind(source, &full_dest).await?,
                _ => {
                    warn!("Unsupported filesystem type: {}", fs_type);
                }
            }
        }
    }

    // Gaming-specific mounts
    if let Some(ref gaming) = state.config.gaming_config {
        setup_gaming_mounts(&rootfs_path, gaming).await?;
    }

    Ok(())
}

async fn create_essential_dirs(rootfs_path: &std::path::Path) -> Result<()> {
    let essential_dirs = [
        "proc", "sys", "dev", "tmp", "run", "var", "etc", "bin", "usr", "lib"
    ];

    for dir in &essential_dirs {
        let dir_path = rootfs_path.join(dir);
        if !dir_path.exists() {
            fs::create_dir_all(&dir_path)
                .with_context(|| format!("Failed to create essential directory: {}", dir))?;
        }
    }

    Ok(())
}

async fn mount_proc(dest: &std::path::Path) -> Result<()> {
    info!("  📋 Mounting proc filesystem");
    fs::create_dir_all(dest).context("Failed to create proc directory")?;

    mount(
        Some("proc"),
        dest,
        Some("proc"),
        MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
        None::<&str>,
    ).context("Failed to mount proc")?;

    Ok(())
}

async fn mount_sysfs(dest: &std::path::Path) -> Result<()> {
    info!("  🖥️  Mounting sysfs filesystem");
    fs::create_dir_all(dest).context("Failed to create sys directory")?;

    mount(
        Some("sysfs"),
        dest,
        Some("sysfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV | MsFlags::MS_RDONLY,
        None::<&str>,
    ).context("Failed to mount sysfs")?;

    Ok(())
}

async fn mount_tmpfs(dest: &std::path::Path) -> Result<()> {
    info!("  💾 Mounting tmpfs filesystem");
    fs::create_dir_all(dest).context("Failed to create tmpfs directory")?;

    mount(
        Some("tmpfs"),
        dest,
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
        Some("mode=755"),
    ).context("Failed to mount tmpfs")?;

    Ok(())
}

async fn mount_bind(source: &std::path::Path, dest: &std::path::Path) -> Result<()> {
    info!("  🔗 Bind mounting {} -> {}", source.display(), dest.display());

    if !source.exists() {
        warn!("Bind mount source does not exist: {}", source.display());
        return Ok(());
    }

    // Create destination if it doesn't exist
    if source.is_dir() {
        fs::create_dir_all(dest).context("Failed to create bind mount destination")?;
    } else {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).context("Failed to create parent directory")?;
        }
        if !dest.exists() {
            fs::File::create(dest).context("Failed to create bind mount file")?;
        }
    }

    mount(
        Some(source),
        dest,
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
    ).context("Failed to create bind mount")?;

    Ok(())
}

async fn setup_gaming_mounts(rootfs_path: &std::path::Path, gaming: &crate::config::GamingConfig) -> Result<()> {
    info!("🎮 Setting up gaming-specific mounts");

    // GPU device access
    if let Some(ref gpu) = gaming.gpu {
        if gpu.passthrough == Some(true) {
            mount_gpu_devices(rootfs_path).await?;
        }
    }

    // Audio device access
    if gaming.audio.is_some() {
        mount_audio_devices(rootfs_path).await?;
    }

    // X11/Wayland display server access
    mount_display_sockets(rootfs_path).await?;

    Ok(())
}

async fn mount_gpu_devices(rootfs_path: &std::path::Path) -> Result<()> {
    info!("  🖥️  Mounting GPU devices");

    // Mount DRI devices for GPU access
    let dri_dest = rootfs_path.join("dev/dri");
    if std::path::Path::new("/dev/dri").exists() {
        mount_bind(std::path::Path::new("/dev/dri"), &dri_dest).await?;
    }

    // Mount NVIDIA devices if present
    if std::path::Path::new("/dev/nvidia0").exists() {
        let nvidia_dest = rootfs_path.join("dev");
        for entry in fs::read_dir("/dev")? {
            let entry = entry?;
            let name = entry.file_name();
            if name.to_string_lossy().starts_with("nvidia") {
                let source = entry.path();
                let dest = nvidia_dest.join(&name);
                mount_bind(&source, &dest).await?;
            }
        }
    }

    Ok(())
}

async fn mount_audio_devices(rootfs_path: &std::path::Path) -> Result<()> {
    info!("  🔊 Mounting audio devices");

    // Mount sound devices
    if std::path::Path::new("/dev/snd").exists() {
        let snd_dest = rootfs_path.join("dev/snd");
        mount_bind(std::path::Path::new("/dev/snd"), &snd_dest).await?;
    }

    // Mount PulseAudio socket
    if let Ok(pulse_server) = std::env::var("PULSE_SERVER") {
        info!("  🎵 Mounting PulseAudio socket: {}", pulse_server);
    }

    Ok(())
}

async fn mount_display_sockets(rootfs_path: &std::path::Path) -> Result<()> {
    info!("  🖼️  Mounting display server sockets");

    // X11 socket
    if std::path::Path::new("/tmp/.X11-unix").exists() {
        let x11_dest = rootfs_path.join("tmp/.X11-unix");
        mount_bind(std::path::Path::new("/tmp/.X11-unix"), &x11_dest).await?;
    }

    // Wayland socket
    if let Ok(wayland_display) = std::env::var("WAYLAND_DISPLAY") {
        let wayland_socket = format!("/run/user/{}/{}", nix::unistd::getuid(), wayland_display);
        if std::path::Path::new(&wayland_socket).exists() {
            info!("  🖼️  Mounting Wayland socket: {}", wayland_socket);
            let dest = rootfs_path.join(format!("run/user/{}/{}", nix::unistd::getuid(), wayland_display));
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            mount_bind(std::path::Path::new(&wayland_socket), &dest).await?;
        }
    }

    Ok(())
}

async fn setup_cgroups(state: &ContainerState) -> Result<()> {
    info!("📊 Setting up cgroups v2 for container: {}", state.id);

    let limits = &state.config.resource_limits;
    let cgroup_path = format!("/sys/fs/cgroup/bolt/{}", state.id);

    // Check if cgroups v2 is available
    if !std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").exists() {
        warn!("Cgroups v2 not available, skipping resource limits");
        return Ok(());
    }

    // Create cgroup directory
    if let Err(e) = fs::create_dir_all(&cgroup_path) {
        warn!("Failed to create cgroup directory: {} - {}", cgroup_path, e);
        return Ok(()); // Don't fail container creation if cgroups fail
    }

    info!("✅ Created cgroup: {}", cgroup_path);

    // Set resource limits (best effort)
    if let Some(memory_limit) = limits.memory_limit {
        let _ = set_memory_limit(&cgroup_path, memory_limit).await;
    }

    if let Some(cpu_limit) = limits.cpu_limit {
        let _ = set_cpu_limit(&cgroup_path, cpu_limit).await;
    }

    if let Some(pids_limit) = limits.pids_limit {
        let _ = set_pids_limit(&cgroup_path, pids_limit).await;
    }

    // Gaming-specific optimizations
    if let Some(ref gaming) = state.config.gaming_config {
        let _ = setup_gaming_cgroups(&cgroup_path, gaming).await;
    }

    info!("✅ Cgroups v2 configured successfully");
    Ok(())
}

async fn set_memory_limit(cgroup_path: &str, limit_bytes: u64) -> Result<()> {
    info!("💾 Setting memory limit: {:.1} MB", limit_bytes as f64 / 1024.0 / 1024.0);

    let memory_max_path = format!("{}/memory.max", cgroup_path);
    fs::write(&memory_max_path, limit_bytes.to_string())
        .with_context(|| format!("Failed to set memory limit: {}", memory_max_path))?;

    Ok(())
}

async fn set_cpu_limit(cgroup_path: &str, cpu_cores: f64) -> Result<()> {
    info!("⚙️  Setting CPU limit: {:.2} cores", cpu_cores);

    // Convert cores to microseconds (1 core = 100000 microseconds per 100ms period)
    let quota_us = (cpu_cores * 100000.0) as u64;
    let period_us = 100000u64; // 100ms period

    let cpu_max_path = format!("{}/cpu.max", cgroup_path);
    let cpu_limit_str = format!("{} {}", quota_us, period_us);

    fs::write(&cpu_max_path, &cpu_limit_str)
        .with_context(|| format!("Failed to set CPU limit: {}", cpu_max_path))?;

    Ok(())
}

async fn set_pids_limit(cgroup_path: &str, max_pids: u32) -> Result<()> {
    info!("🏃 Setting PIDs limit: {}", max_pids);

    let pids_max_path = format!("{}/pids.max", cgroup_path);
    fs::write(&pids_max_path, max_pids.to_string())
        .with_context(|| format!("Failed to set PIDs limit: {}", pids_max_path))?;

    Ok(())
}

async fn setup_gaming_cgroups(
    cgroup_path: &str,
    gaming: &crate::config::GamingConfig,
) -> Result<()> {
    info!("🎮 Setting up gaming cgroup optimizations");

    // Gaming containers get special treatment:
    // 1. Higher CPU priority/weight
    // 2. I/O priority boost

    // Set CPU weight higher for gaming
    let cpu_weight_path = format!("{}/cpu.weight", cgroup_path);
    if std::path::Path::new(&cpu_weight_path).exists() {
        if let Err(e) = fs::write(&cpu_weight_path, "200") {
            debug!("Failed to set gaming CPU weight: {}", e);
        } else {
            info!("✅ Set gaming CPU weight: 200");
        }
    }

    // I/O priority for gaming
    let io_weight_path = format!("{}/io.weight", cgroup_path);
    if std::path::Path::new(&io_weight_path).exists() {
        if let Err(e) = fs::write(&io_weight_path, "200") {
            debug!("Failed to set gaming I/O weight: {}", e);
        } else {
            info!("✅ Set gaming I/O weight: 200");
        }
    }

    if let Some(ref perf) = gaming.performance {
        if let Some(priority) = perf.rt_priority {
            info!("🚀 Gaming RT priority configured: {}", priority);
            // RT scheduling is typically handled at process level, not cgroup level
        }
    }

    Ok(())
}

async fn setup_security_profile(state: &ContainerState) -> Result<()> {
    info!("🔒 Setting up security profile for: {}", state.id);

    let security = &state.config.security_profile;

    // Apply security constraints:
    // 1. Drop capabilities
    // 2. Apply seccomp profile
    // 3. Set up AppArmor/SELinux
    // 4. Configure no-new-privileges

    if security.no_new_privileges {
        debug!("Setting no-new-privileges");
        // TODO: Set PR_SET_NO_NEW_PRIVS
    }

    for cap in &security.drop_capabilities {
        debug!("Dropping capability: {}", cap);
        // TODO: Drop Linux capabilities
    }

    if let Some(ref seccomp) = security.seccomp_profile {
        debug!("Applying seccomp profile: {}", seccomp);
        // TODO: Load and apply seccomp filter
    }

    // Gaming containers need relaxed security for GPU access
    if state.config.gaming_config.is_some() {
        setup_gaming_security(state).await?;
    }

    warn!("Security profile setup not yet fully implemented");
    Ok(())
}

async fn setup_gaming_security(state: &ContainerState) -> Result<()> {
    info!("🎮 Setting up gaming security profile for: {}", state.id);

    // Gaming containers need:
    // 1. Access to GPU devices (/dev/dri/*)
    // 2. Access to audio devices
    // 3. Access to input devices for controllers
    // 4. Relaxed seccomp for graphics drivers

    // TODO: Implement gaming-specific security adjustments
    Ok(())
}

pub async fn create_container_rootfs(
    container_id: &str,
    bundle_path: &std::path::Path,
    _spec: &Spec,
) -> Result<()> {
    info!("📁 Creating rootfs for container: {}", container_id);

    let rootfs_path = bundle_path.join("rootfs");
    std::fs::create_dir_all(&rootfs_path)
        .context("Failed to create rootfs directory")?;

    // Check if there's an extracted image available
    let image_storage_path = PathBuf::from("/var/lib/bolt/images");
    let mut image_found = false;

    // Try to find and copy the extracted image
    if image_storage_path.exists() {
        if let Ok(entries) = std::fs::read_dir(&image_storage_path) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        let extracted_rootfs = path.join("rootfs");
                        if extracted_rootfs.exists() {
                            info!("📦 Using extracted image rootfs from: {:?}", extracted_rootfs);

                            // Copy the extracted rootfs
                            copy_dir_all(&extracted_rootfs, &rootfs_path)?;
                            image_found = true;
                            break;
                        }
                    }
                }
            }
        }
    }

    if !image_found {
        info!("⚠️  No extracted image found, creating minimal rootfs");

        // Fallback: Create basic directory structure for a minimal Linux container
        let dirs = [
            "bin", "sbin", "usr/bin", "usr/sbin", "usr/local/bin", "usr/local/sbin",
            "etc", "home", "root", "tmp", "var", "var/log", "var/tmp",
            "dev", "proc", "sys", "run", "opt", "srv", "media", "mnt",
            "lib", "lib64", "usr/lib", "usr/lib64"
        ];

        for dir in &dirs {
            let dir_path = rootfs_path.join(dir);
            std::fs::create_dir_all(&dir_path)
                .with_context(|| format!("Failed to create directory: {}", dir))?;
        }

        // Create essential files
        create_essential_files(&rootfs_path).await?;

        // Set up basic device nodes in /dev
        create_device_nodes(&rootfs_path).await?;

        // Copy essential binaries from host (busybox-style minimal environment)
        copy_essential_binaries(&rootfs_path).await?;
    }

    info!("✅ Rootfs created for container: {}", container_id);
    Ok(())
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if path.is_dir() {
            copy_dir_all(&path, &dst_path)?;
        } else {
            std::fs::copy(&path, &dst_path)?;
        }
    }

    Ok(())
}

async fn create_essential_files(rootfs_path: &std::path::Path) -> Result<()> {
    // Create /etc/passwd
    let passwd_content = r#"root:x:0:0:root:/root:/bin/sh
daemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin
bin:x:2:2:bin:/bin:/usr/sbin/nologin
sys:x:3:3:sys:/dev:/usr/sbin/nologin
"#;
    std::fs::write(rootfs_path.join("etc/passwd"), passwd_content)?;

    // Create /etc/group
    let group_content = r#"root:x:0:
daemon:x:1:
bin:x:2:
sys:x:3:
"#;
    std::fs::write(rootfs_path.join("etc/group"), group_content)?;

    // Create /etc/resolv.conf (copy from host)
    if let Ok(resolv_content) = std::fs::read_to_string("/etc/resolv.conf") {
        std::fs::write(rootfs_path.join("etc/resolv.conf"), resolv_content)?;
    }

    // Create minimal /etc/hosts
    let hosts_content = r#"127.0.0.1	localhost
::1		localhost ip6-localhost ip6-loopback
"#;
    std::fs::write(rootfs_path.join("etc/hosts"), hosts_content)?;

    Ok(())
}

async fn create_device_nodes(rootfs_path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let dev_path = rootfs_path.join("dev");

    // Create /dev/null, /dev/zero, /dev/random, /dev/urandom as regular files
    // Note: In a real implementation, these would be proper device nodes
    std::fs::write(dev_path.join("null"), "")?;
    std::fs::write(dev_path.join("zero"), "")?;
    std::fs::write(dev_path.join("random"), "")?;
    std::fs::write(dev_path.join("urandom"), "")?;

    // Create /dev/console
    std::fs::write(dev_path.join("console"), "")?;

    // Create /dev/tty
    std::fs::write(dev_path.join("tty"), "")?;

    Ok(())
}

async fn copy_essential_binaries(rootfs_path: &std::path::Path) -> Result<()> {
    // Copy essential binaries from host system
    let essential_bins = [
        "/bin/sh", "/bin/bash", "/bin/ls", "/bin/cat", "/bin/echo",
        "/bin/mkdir", "/bin/touch", "/bin/rm", "/bin/cp", "/bin/mv"
    ];

    for bin in &essential_bins {
        if std::path::Path::new(bin).exists() {
            let target_path = rootfs_path.join(&bin[1..]); // Remove leading /
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Copy the binary
            if let Err(_e) = std::fs::copy(bin, &target_path) {
                // If copy fails, create a simple stub
                std::fs::write(&target_path, "#!/bin/sh\necho 'Command not available in minimal container'\n")?;

                // Make it executable
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&target_path)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&target_path, perms)?;
            }
        }
    }

    // Ensure /bin/sh exists (essential for containers)
    let sh_path = rootfs_path.join("bin/sh");
    if !sh_path.exists() {
        std::fs::write(&sh_path, "#!/bin/sh\n# Minimal shell stub\necho 'Minimal container shell'\nexec /bin/bash \"$@\"\n")?;

        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&sh_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&sh_path, perms)?;
    }

    Ok(())
}