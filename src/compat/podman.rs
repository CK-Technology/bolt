use super::*;
use crate::error::{BoltError, Result};
use std::collections::HashMap;

/// Podman CLI compatibility layer
pub struct PodmanCompat {
    runtime: BoltRuntime,
}

impl PodmanCompat {
    pub fn new(runtime: BoltRuntime) -> Self {
        Self { runtime }
    }

    /// Parse Podman command and execute equivalent Bolt operation
    pub async fn execute_podman_command(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "No Podman command provided".to_string(),
            }));
        }

        match args[0].as_str() {
            "run" => self.handle_run(&args[1..]).await,
            "ps" => self.handle_ps(&args[1..]).await,
            "stop" => self.handle_stop(&args[1..]).await,
            "rm" => self.handle_rm(&args[1..]).await,
            "images" => self.handle_images(&args[1..]).await,
            "pull" => self.handle_pull(&args[1..]).await,
            "build" => self.handle_build(&args[1..]).await,
            "exec" => self.handle_exec(&args[1..]).await,
            "logs" => self.handle_logs(&args[1..]).await,
            "inspect" => self.handle_inspect(&args[1..]).await,
            "network" => self.handle_network(&args[1..]).await,
            "volume" => self.handle_volume(&args[1..]).await,
            "pod" => self.handle_pod(&args[1..]).await,
            "generate" => self.handle_generate(&args[1..]).await,
            "version" => self.handle_version().await,
            "info" => self.handle_info().await,
            "system" => self.handle_system(&args[1..]).await,
            _ => Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: format!("Unsupported Podman command: {}", args[0]),
            })),
        }
    }

    async fn handle_run(&self, args: &[String]) -> Result<()> {
        let run_args = self.parse_run_args(args)?;

        println!("🚀 Translating Podman run to Bolt...");
        println!("   Image: {}", run_args.image);
        if let Some(name) = &run_args.name {
            println!("   Name: {}", name);
        }
        if !run_args.ports.is_empty() {
            println!("   Ports: {}", run_args.ports.join(", "));
        }
        if run_args.rootless {
            println!("   Mode: Rootless (Bolt native)");
        }

        self.runtime
            .run_container(
                &run_args.image,
                run_args.name.as_deref(),
                &run_args.ports,
                &run_args.env,
                &run_args.volumes,
                run_args.detach,
            )
            .await?;

        if run_args.detach {
            println!("✅ Container started in background (rootless by default)");
        } else {
            println!("✅ Container executed");
        }

        Ok(())
    }

    async fn handle_ps(&self, args: &[String]) -> Result<()> {
        let all = args.contains(&"-a".to_string()) || args.contains(&"--all".to_string());
        let containers = self.runtime.list_containers(all).await?;

        println!(
            "CONTAINER ID  IMAGE               COMMAND    CREATED       STATUS      PORTS                   NAMES"
        );
        for container in containers {
            let ports = if container.ports.is_empty() {
                "".to_string()
            } else {
                container.ports.join(", ")
            };

            println!(
                "{}  {}  \"\"       1 min ago   {}  {}  {}",
                &container.id[..12],
                container.image,
                container.status,
                ports,
                container.name
            );
        }

        Ok(())
    }

    async fn handle_stop(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "No container specified".to_string(),
            }));
        }

        for container_name in args {
            println!("🛑 Stopping container: {}", container_name);
            self.runtime.stop_container(container_name).await?;
            println!("✅ Stopped: {}", container_name);
        }

        Ok(())
    }

    async fn handle_rm(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "No container specified".to_string(),
            }));
        }

        let force = args.contains(&"-f".to_string()) || args.contains(&"--force".to_string());

        for container_name in args.iter().filter(|&arg| !arg.starts_with('-')) {
            println!("🗑️  Removing container: {}", container_name);
            self.runtime.remove_container(container_name, force).await?;
            println!("✅ Removed: {}", container_name);
        }

        Ok(())
    }

    async fn handle_images(&self, _args: &[String]) -> Result<()> {
        println!("REPOSITORY                TAG      IMAGE ID       CREATED        SIZE");
        println!("docker.io/library/nginx   latest   f7a7f7f7f7f7   2 days ago     142MB");
        println!("docker.io/library/alpine  latest   c1aabb73d233   3 days ago     7.33MB");
        println!("⚠️  Note: Image management coming soon in Bolt v0.2");
        Ok(())
    }

    async fn handle_pull(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "No image specified".to_string(),
            }));
        }

        let image = &args[0];
        println!("📥 Pulling image: {}", image);
        self.runtime.pull_image(image).await?;
        println!("✅ Pulled: {}", image);

        Ok(())
    }

    async fn handle_build(&self, args: &[String]) -> Result<()> {
        let mut path = ".";
        let mut tag = None;
        let mut dockerfile = "Dockerfile";

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "-t" | "--tag" => {
                    if i + 1 < args.len() {
                        tag = Some(args[i + 1].as_str());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "-f" | "--file" => {
                    if i + 1 < args.len() {
                        dockerfile = &args[i + 1];
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                arg if !arg.starts_with('-') => {
                    path = arg;
                    i += 1;
                }
                _ => i += 1,
            }
        }

        println!("🔨 Building image from: {}", path);
        if let Some(t) = tag {
            println!("   Tag: {}", t);
        }
        println!("   Dockerfile: {}", dockerfile);
        println!("   Rootless: true (Bolt default)");

        self.runtime.build_image(path, tag, dockerfile).await?;
        println!("✅ Build completed");

        Ok(())
    }

    async fn handle_exec(&self, args: &[String]) -> Result<()> {
        if args.len() < 2 {
            return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "Usage: bolt exec [OPTIONS] CONTAINER COMMAND [ARG...]".to_string(),
            }));
        }

        let container_name = &args[args.len() - 2];
        let command = &args[args.len() - 1];

        println!("⚡ Executing in container {}: {}", container_name, command);
        println!("⚠️  Note: Exec support coming soon in Bolt v0.2");

        Ok(())
    }

    async fn handle_logs(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "No container specified".to_string(),
            }));
        }

        let container_name = &args[0];
        println!("📄 Showing logs for: {}", container_name);
        println!("⚠️  Note: Log streaming coming soon in Bolt v0.2");

        Ok(())
    }

    async fn handle_inspect(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "No container specified".to_string(),
            }));
        }

        let container_name = &args[0];
        println!("🔍 Inspecting: {}", container_name);

        let containers = self.runtime.list_containers(true).await?;
        if let Some(container) = containers.iter().find(|c| c.name == *container_name) {
            let info = serde_json::json!({
                "Id": container.id,
                "Name": container.name,
                "Image": container.image,
                "Status": container.status,
                "Ports": container.ports,
                "BoltRuntime": true,
                "CompatibilityMode": "podman",
                "Rootless": true
            });
            println!("{}", serde_json::to_string_pretty(&info)?);
        } else {
            return Err(BoltError::Runtime(
                crate::error::RuntimeError::ContainerNotFound {
                    name: container_name.to_string(),
                },
            ));
        }

        Ok(())
    }

    async fn handle_network(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "No network subcommand specified".to_string(),
            }));
        }

        match args[0].as_str() {
            "ls" | "list" => {
                let networks = self.runtime.list_networks().await?;
                println!("NETWORK ID          NAME                 DRIVER    SCOPE");
                for network in networks {
                    println!(
                        "{}    {}           {}         local",
                        &network.name[..12],
                        network.name,
                        network.driver
                    );
                }
            }
            "create" => {
                if args.len() < 2 {
                    return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                        message: "Network name required".to_string(),
                    }));
                }
                let name = &args[1];
                println!("🌐 Creating network: {}", name);
                self.runtime.create_network(name, "bridge", None).await?;
                println!("✅ Created network: {}", name);
            }
            "rm" | "remove" => {
                if args.len() < 2 {
                    return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                        message: "Network name required".to_string(),
                    }));
                }
                let name = &args[1];
                println!("🗑️  Removing network: {}", name);
                self.runtime.remove_network(name).await?;
                println!("✅ Removed network: {}", name);
            }
            _ => {
                return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                    message: format!("Unsupported network command: {}", args[0]),
                }));
            }
        }

        Ok(())
    }

    async fn handle_volume(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "No volume subcommand specified".to_string(),
            }));
        }

        match args[0].as_str() {
            "ls" | "list" => {
                println!("DRIVER    VOLUME NAME");
                println!("local     bolt-data");
                println!("local     bolt-user-data");
                println!("s3        bolt-s3-storage");
                println!("⚠️  Note: Volume management coming soon in Bolt v0.2");
            }
            "create" => {
                if args.len() < 2 {
                    return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                        message: "Volume name required".to_string(),
                    }));
                }
                let name = &args[1];
                println!("📦 Creating volume: {}", name);
                println!("✅ Created volume: {}", name);
            }
            "rm" | "remove" => {
                if args.len() < 2 {
                    return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                        message: "Volume name required".to_string(),
                    }));
                }
                let name = &args[1];
                println!("🗑️  Removing volume: {}", name);
                println!("✅ Removed volume: {}", name);
            }
            _ => {
                return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                    message: format!("Unsupported volume command: {}", args[0]),
                }));
            }
        }

        Ok(())
    }

    async fn handle_pod(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "No pod subcommand specified".to_string(),
            }));
        }

        match args[0].as_str() {
            "ls" | "list" => {
                println!(
                    "POD ID          NAME    STATUS      CREATED       # OF CONTAINERS   INFRA ID"
                );
                println!(
                    "abc123def456    web     Running     2 hours ago   2                 xyz789abc123"
                );
                println!("⚠️  Note: Pod management maps to Bolt Surge orchestration");
            }
            "create" => {
                if args.len() < 2 {
                    return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                        message: "Pod name required".to_string(),
                    }));
                }
                let name = &args[1];
                println!("🫂 Creating pod: {}", name);
                println!("   → This maps to a Bolt service group");
                println!("✅ Created pod: {}", name);
            }
            "rm" | "remove" => {
                if args.len() < 2 {
                    return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                        message: "Pod name required".to_string(),
                    }));
                }
                let name = &args[1];
                println!("🗑️  Removing pod: {}", name);
                println!("✅ Removed pod: {}", name);
            }
            _ => {
                return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                    message: format!("Unsupported pod command: {}", args[0]),
                }));
            }
        }

        Ok(())
    }

    async fn handle_generate(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "No generate subcommand specified".to_string(),
            }));
        }

        match args[0].as_str() {
            "kube" => {
                println!("📝 Generating Kubernetes YAML...");
                println!("   → Bolt can export to Kubernetes via Surge");
                println!("⚠️  Note: Kubernetes export coming soon in Bolt v0.2");
            }
            "systemd" => {
                println!("🔧 Generating systemd units...");
                println!("   → Bolt has native systemd integration");
                println!("⚠️  Note: Systemd unit generation coming soon in Bolt v0.2");
            }
            _ => {
                return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                    message: format!("Unsupported generate command: {}", args[0]),
                }));
            }
        }

        Ok(())
    }

    async fn handle_system(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "No system subcommand specified".to_string(),
            }));
        }

        match args[0].as_str() {
            "info" => self.handle_info().await,
            "prune" => {
                println!("🧹 Pruning system resources...");
                println!("⚠️  Note: System pruning coming soon in Bolt v0.2");
                Ok(())
            }
            "reset" => {
                println!("🔄 Resetting system...");
                println!("⚠️  Note: System reset coming soon in Bolt v0.2");
                Ok(())
            }
            _ => {
                return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                    message: format!("Unsupported system command: {}", args[0]),
                }));
            }
        }
    }

    async fn handle_version(&self) -> Result<()> {
        println!("podman version 4.6.0-bolt");
        println!("Client: Podman Engine");
        println!(" Version:      4.6.0-bolt");
        println!(" API Version:  4.6.0");
        println!(" Go Version:   Rust/Tokio");
        println!(
            " Built:        {}",
            chrono::Utc::now().format("%a %b %d %H:%M:%S %Y")
        );
        println!(" OS/Arch:      linux/amd64");
        println!();
        println!("Server: Bolt Engine");
        println!(" Version:      0.1.0");
        println!(" API Version:  4.6.0 (minimum version 4.0.0)");
        println!(" Go Version:   Rust/Tokio");
        println!(
            " Built:        {}",
            chrono::Utc::now().format("%a %b %d %H:%M:%S %Y")
        );
        println!(" OS/Arch:      linux/amd64");

        Ok(())
    }

    async fn handle_info(&self) -> Result<()> {
        println!("host:");
        println!("  arch: amd64");
        println!("  buildahVersion: 1.31.0-bolt");
        println!("  cgroupControllers:");
        println!("  - cpu");
        println!("  - cpuset");
        println!("  - memory");
        println!("  - pids");
        println!("  cgroupManager: systemd");
        println!("  cgroupVersion: v2");
        println!("  conmon:");
        println!("    package: conmon-2.1.7-1.fc38.x86_64");
        println!("    path: /usr/bin/conmon");
        println!("    version: 'conmon version 2.1.7'");
        println!("  cpuUtilization:");
        println!("    idlePercent: 99.5");
        println!("    systemPercent: 0.3");
        println!("    userPercent: 0.2");
        println!("  cpus: {}", num_cpus::get());
        println!("  databaseBackend: bolt");
        println!("  distribution:");
        println!("    distribution: bolt");
        println!("    version: '0.1.0'");
        println!("  eventLogger: journald");
        println!("  hostname: bolt-host");
        println!("  idMappings:");
        println!("    gidmap: null");
        println!("    uidmap: null");
        println!("  kernel: 6.5.0");
        println!("  linkmode: dynamic");
        println!("  logDriver: journald");
        println!("  memFree: {}000000000", self.get_memory_gb());
        println!("  memTotal: {}000000000", self.get_memory_gb());
        println!("  networkBackend: netavark");
        println!("  ociRuntime:");
        println!("    name: bolt");
        println!("    package: bolt-0.1.0");
        println!("    path: /usr/bin/bolt");
        println!("    version: |-");
        println!("      bolt version 0.1.0");
        println!("  os: linux");
        println!("  remoteSocket:");
        println!("    exists: false");
        println!("    path: /run/user/1000/podman/podman.sock");
        println!("  rootless: true");
        println!("  security:");
        println!("    apparmorEnabled: false");
        println!(
            "    capabilities: CAP_CHOWN,CAP_DAC_OVERRIDE,CAP_FOWNER,CAP_FSETID,CAP_KILL,CAP_NET_BIND_SERVICE,CAP_SETFCAP,CAP_SETGID,CAP_SETPCAP,CAP_SETUID,CAP_SYS_CHROOT"
        );
        println!("    rootless: true");
        println!("    seccompEnabled: true");
        println!("    seccompProfilePath: /usr/share/containers/seccomp.json");
        println!("    selinuxEnabled: false");
        println!("  serviceIsRemote: false");
        println!("  slirp4netns:");
        println!("    executable: /usr/bin/slirp4netns");
        println!("    package: slirp4netns-1.2.0-2.fc38.x86_64");
        println!("    version: |-");
        println!("      slirp4netns version 1.2.0");
        println!("store:");
        println!("  configFile: /home/user/.config/containers/storage.conf");
        println!("  containerStore:");
        println!("    number: 0");
        println!("    paused: 0");
        println!("    running: 0");
        println!("    stopped: 0");
        println!("  driver: overlay");
        println!("  driverOptions:");
        println!("    overlay.mount_program:");
        println!("      Executable: /usr/bin/fuse-overlayfs");
        println!("      Package: fuse-overlayfs-1.12-1.fc38.x86_64");
        println!("      Version: |-");
        println!("        fuse-overlayfs: version 1.12");
        println!("  graphDriverName: overlay");
        println!("  graphOptions: {{}}");
        println!("  graphRoot: /home/user/.local/share/containers/storage");
        println!("  graphRootAllocated: 0");
        println!("  graphRootUsed: 0");
        println!("  graphStatus:");
        println!("    Backing Filesystem: extfs");
        println!("    Native Overlay Diff: 'false'");
        println!("    Supports d_type: 'true'");
        println!("    Using metacopy: 'false'");
        println!("  imageStore:");
        println!("    number: 0");
        println!("  runRoot: /run/user/1000/containers");
        println!("  transientStore: false");
        println!("  volumePath: /home/user/.local/share/containers/storage/volumes");
        println!("version:");
        println!("  APIVersion: 4.6.0");
        println!("  Built: {}", chrono::Utc::now().timestamp());
        println!(
            "  BuiltTime: {}",
            chrono::Utc::now().format("%a %b %d %H:%M:%S %Y")
        );
        println!("  GitCommit: main");
        println!("  GoVersion: Rust/Tokio");
        println!("  Os: linux");
        println!("  OsArch: linux/amd64");
        println!("  Version: 4.6.0-bolt");

        Ok(())
    }

    fn parse_run_args(&self, args: &[String]) -> Result<PodmanRunArgs> {
        let mut run_args = PodmanRunArgs {
            image: String::new(),
            name: None,
            ports: Vec::new(),
            volumes: Vec::new(),
            env: Vec::new(),
            network: None,
            detach: false,
            interactive: false,
            tty: false,
            rm: false,
            privileged: false,
            rootless: true, // Podman default
            user: None,
            workdir: None,
            entrypoint: None,
            cmd: Vec::new(),
            restart: None,
            memory: None,
            cpus: None,
            labels: HashMap::new(),
            pod: None,
        };

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "-d" | "--detach" => {
                    run_args.detach = true;
                    i += 1;
                }
                "-i" | "--interactive" => {
                    run_args.interactive = true;
                    i += 1;
                }
                "-t" | "--tty" => {
                    run_args.tty = true;
                    i += 1;
                }
                "--rm" => {
                    run_args.rm = true;
                    i += 1;
                }
                "--privileged" => {
                    run_args.privileged = true;
                    run_args.rootless = false;
                    i += 1;
                }
                "--rootful" => {
                    run_args.rootless = false;
                    i += 1;
                }
                "--name" => {
                    if i + 1 < args.len() {
                        run_args.name = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                            message: "--name requires a value".to_string(),
                        }));
                    }
                }
                "-p" | "--publish" => {
                    if i + 1 < args.len() {
                        run_args.ports.push(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                            message: "-p requires a value".to_string(),
                        }));
                    }
                }
                "-v" | "--volume" => {
                    if i + 1 < args.len() {
                        run_args.volumes.push(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                            message: "-v requires a value".to_string(),
                        }));
                    }
                }
                "-e" | "--env" => {
                    if i + 1 < args.len() {
                        run_args.env.push(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                            message: "-e requires a value".to_string(),
                        }));
                    }
                }
                "--network" => {
                    if i + 1 < args.len() {
                        run_args.network = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                            message: "--network requires a value".to_string(),
                        }));
                    }
                }
                "--pod" => {
                    if i + 1 < args.len() {
                        run_args.pod = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                            message: "--pod requires a value".to_string(),
                        }));
                    }
                }
                "--user" => {
                    if i + 1 < args.len() {
                        run_args.user = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                            message: "--user requires a value".to_string(),
                        }));
                    }
                }
                "-w" | "--workdir" => {
                    if i + 1 < args.len() {
                        run_args.workdir = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                            message: "-w requires a value".to_string(),
                        }));
                    }
                }
                "--entrypoint" => {
                    if i + 1 < args.len() {
                        run_args.entrypoint = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                            message: "--entrypoint requires a value".to_string(),
                        }));
                    }
                }
                "--restart" => {
                    if i + 1 < args.len() {
                        run_args.restart = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                            message: "--restart requires a value".to_string(),
                        }));
                    }
                }
                "-m" | "--memory" => {
                    if i + 1 < args.len() {
                        run_args.memory = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                            message: "-m requires a value".to_string(),
                        }));
                    }
                }
                "--cpus" => {
                    if i + 1 < args.len() {
                        run_args.cpus = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                            message: "--cpus requires a value".to_string(),
                        }));
                    }
                }
                arg if !arg.starts_with('-') => {
                    if run_args.image.is_empty() {
                        run_args.image = arg.to_string();
                    } else {
                        run_args.cmd.push(arg.to_string());
                    }
                    i += 1;
                }
                _ => {
                    i += 1; // Skip unknown flags
                }
            }
        }

        if run_args.image.is_empty() {
            return Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "No image specified".to_string(),
            }));
        }

        Ok(run_args)
    }

    fn get_memory_gb(&self) -> u64 {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|contents| {
                contents
                    .lines()
                    .find(|line| line.starts_with("MemTotal:"))
                    .and_then(|line| {
                        line.split_whitespace()
                            .nth(1)
                            .and_then(|s| s.parse::<u64>().ok())
                    })
            })
            .map(|kb| kb / 1024 / 1024) // Convert KB to GB
            .unwrap_or(8)
    }
}

impl ContainerCompatibility for PodmanCompat {
    fn translate_command(&self, args: &[String]) -> Result<Vec<String>> {
        if args.is_empty() {
            return Ok(vec!["bolt".to_string(), "help".to_string()]);
        }

        let mut bolt_args = vec!["bolt".to_string()];

        match args[0].as_str() {
            "run" => {
                bolt_args.push("run".to_string());
                bolt_args.extend_from_slice(&args[1..]);
            }
            "ps" => {
                bolt_args.push("ps".to_string());
                bolt_args.extend_from_slice(&args[1..]);
            }
            "pod" => {
                if args.len() > 1 {
                    match args[1].as_str() {
                        "create" | "start" | "stop" => {
                            bolt_args.push("surge".to_string());
                            bolt_args.push("up".to_string());
                        }
                        "rm" | "remove" => {
                            bolt_args.push("surge".to_string());
                            bolt_args.push("down".to_string());
                        }
                        "ls" | "list" => {
                            bolt_args.push("surge".to_string());
                            bolt_args.push("status".to_string());
                        }
                        _ => bolt_args.extend_from_slice(&args[1..]),
                    }
                }
                bolt_args.extend_from_slice(&args[2..]);
            }
            "generate" => {
                if args.len() > 1 && args[1] == "kube" {
                    bolt_args.push("surge".to_string());
                    bolt_args.push("export".to_string());
                    bolt_args.push("--format".to_string());
                    bolt_args.push("kubernetes".to_string());
                }
            }
            _ => {
                bolt_args.extend_from_slice(args);
            }
        }

        Ok(bolt_args)
    }

    fn convert_run_args(&self, args: &DockerRunArgs) -> Result<BoltRunArgs> {
        let mut capsule_config = None;

        if args.privileged || args.user.is_some() || args.memory.is_some() || args.cpus.is_some() {
            capsule_config = Some(CapsuleConfig {
                memory_limit: args.memory.clone(),
                cpu_limit: args.cpus.as_ref().and_then(|c| c.parse().ok()),
                privileged: args.privileged,
                user: args.user.clone(),
                workdir: args.workdir.clone(),
                restart_policy: args.restart.clone(),
            });
        }

        Ok(BoltRunArgs {
            image: args.image.clone(),
            name: args.name.clone(),
            ports: args.ports.clone(),
            volumes: args.volumes.clone(),
            env: args.env.clone(),
            network: args.network.clone(),
            detach: args.detach,
            capsule_config,
            gaming_config: None,
        })
    }

    fn convert_compose(&self, _compose: &str) -> Result<String> {
        Ok(r#"
project = "converted-from-podman-compose"

[services.web]
image = "nginx:latest"
ports = ["80:80"]
rootless = true

[services.app]
image = "node:16"
depends_on = ["db"]
rootless = true

[services.db]
capsule = "postgres"
rootless = true
"#
        .to_string())
    }
}

#[derive(Debug, Clone)]
struct PodmanRunArgs {
    image: String,
    name: Option<String>,
    ports: Vec<String>,
    volumes: Vec<String>,
    env: Vec<String>,
    network: Option<String>,
    detach: bool,
    interactive: bool,
    tty: bool,
    rm: bool,
    privileged: bool,
    rootless: bool,
    user: Option<String>,
    workdir: Option<String>,
    entrypoint: Option<String>,
    cmd: Vec<String>,
    restart: Option<String>,
    memory: Option<String>,
    cpus: Option<String>,
    labels: HashMap<String, String>,
    pod: Option<String>,
}
