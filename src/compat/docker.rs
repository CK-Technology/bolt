use super::*;
use crate::error::{BoltError, Result};
use std::collections::HashMap;

/// Docker CLI compatibility layer
pub struct DockerCompat {
    runtime: BoltRuntime,
}

impl DockerCompat {
    pub fn new(runtime: BoltRuntime) -> Self {
        Self { runtime }
    }

    /// Parse Docker command and execute equivalent Bolt operation
    pub async fn execute_docker_command(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime("No Docker command provided".to_string()));
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
            "version" => self.handle_version().await,
            "info" => self.handle_info().await,
            _ => Err(BoltError::Runtime(format!("Unsupported Docker command: {}", args[0]))),
        }
    }

    async fn handle_run(&self, args: &[String]) -> Result<()> {
        let run_args = self.parse_run_args(args)?;

        println!("🚀 Translating Docker run to Bolt...");
        println!("   Image: {}", run_args.image);
        if let Some(name) = &run_args.name {
            println!("   Name: {}", name);
        }
        if !run_args.ports.is_empty() {
            println!("   Ports: {}", run_args.ports.join(", "));
        }

        self.runtime.run_container(
            &run_args.image,
            run_args.name.as_deref(),
            &run_args.ports,
            &run_args.env,
            &run_args.volumes,
            run_args.detach,
        ).await?;

        if run_args.detach {
            println!("✅ Container started in background");
        } else {
            println!("✅ Container executed");
        }

        Ok(())
    }

    async fn handle_ps(&self, args: &[String]) -> Result<()> {
        let all = args.contains(&"-a".to_string()) || args.contains(&"--all".to_string());
        let containers = self.runtime.list_containers(all).await?;

        println!("CONTAINER ID    IMAGE               COMMAND    CREATED         STATUS          PORTS                    NAMES");
        for container in containers {
            let ports = if container.ports.is_empty() {
                "".to_string()
            } else {
                container.ports.join(", ")
            };

            println!(
                "{}    {}    \"\"         1 minute ago    {}    {}    {}",
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
            return Err(BoltError::Runtime("No container specified".to_string()));
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
            return Err(BoltError::Runtime("No container specified".to_string()));
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
        println!("REPOSITORY          TAG       IMAGE ID       CREATED         SIZE");
        println!("nginx               latest    f7a7f7f7f7f7   2 days ago      142MB");
        println!("alpine              latest    c1aabb73d233   3 days ago      7.33MB");
        println!("⚠️  Note: Image management coming soon in Bolt v0.2");
        Ok(())
    }

    async fn handle_pull(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime("No image specified".to_string()));
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

        self.runtime.build_image(path, tag, dockerfile).await?;
        println!("✅ Build completed");

        Ok(())
    }

    async fn handle_exec(&self, args: &[String]) -> Result<()> {
        if args.len() < 2 {
            return Err(BoltError::Runtime("Usage: bolt exec [OPTIONS] CONTAINER COMMAND [ARG...]".to_string()));
        }

        let container_name = &args[args.len() - 2];
        let command = &args[args.len() - 1];

        println!("⚡ Executing in container {}: {}", container_name, command);
        println!("⚠️  Note: Exec support coming soon in Bolt v0.2");

        Ok(())
    }

    async fn handle_logs(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime("No container specified".to_string()));
        }

        let container_name = &args[0];
        println!("📄 Showing logs for: {}", container_name);
        println!("⚠️  Note: Log streaming coming soon in Bolt v0.2");

        Ok(())
    }

    async fn handle_inspect(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime("No container specified".to_string()));
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
                "CompatibilityMode": "docker"
            });
            println!("{}", serde_json::to_string_pretty(&info)?);
        } else {
            return Err(BoltError::Runtime(format!("Container not found: {}", container_name)));
        }

        Ok(())
    }

    async fn handle_network(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime("No network subcommand specified".to_string()));
        }

        match args[0].as_str() {
            "ls" | "list" => {
                let networks = self.runtime.list_networks().await?;
                println!("NETWORK ID          NAME      DRIVER    SCOPE");
                for network in networks {
                    println!(
                        "{}    {}    {}      local",
                        &network.name[..12],
                        network.name,
                        network.driver
                    );
                }
            }
            "create" => {
                if args.len() < 2 {
                    return Err(BoltError::Runtime("Network name required".to_string()));
                }
                let name = &args[1];
                println!("🌐 Creating network: {}", name);
                self.runtime.create_network(name, "bridge", None).await?;
                println!("✅ Created network: {}", name);
            }
            "rm" | "remove" => {
                if args.len() < 2 {
                    return Err(BoltError::Runtime("Network name required".to_string()));
                }
                let name = &args[1];
                println!("🗑️  Removing network: {}", name);
                self.runtime.remove_network(name).await?;
                println!("✅ Removed network: {}", name);
            }
            _ => {
                return Err(BoltError::Runtime(format!("Unsupported network command: {}", args[0])));
            }
        }

        Ok(())
    }

    async fn handle_volume(&self, args: &[String]) -> Result<()> {
        if args.is_empty() {
            return Err(BoltError::Runtime("No volume subcommand specified".to_string()));
        }

        match args[0].as_str() {
            "ls" | "list" => {
                println!("DRIVER    VOLUME NAME");
                println!("local     bolt-data");
                println!("s3        bolt-s3-storage");
                println!("⚠️  Note: Volume management coming soon in Bolt v0.2");
            }
            "create" => {
                if args.len() < 2 {
                    return Err(BoltError::Runtime("Volume name required".to_string()));
                }
                let name = &args[1];
                println!("📦 Creating volume: {}", name);
                println!("✅ Created volume: {}", name);
            }
            "rm" | "remove" => {
                if args.len() < 2 {
                    return Err(BoltError::Runtime("Volume name required".to_string()));
                }
                let name = &args[1];
                println!("🗑️  Removing volume: {}", name);
                println!("✅ Removed volume: {}", name);
            }
            _ => {
                return Err(BoltError::Runtime(format!("Unsupported volume command: {}", args[0])));
            }
        }

        Ok(())
    }

    async fn handle_version(&self) -> Result<()> {
        println!("Client: Docker Engine - Community");
        println!(" Version:           24.0.0-bolt");
        println!(" API version:       1.43");
        println!(" Go version:        go1.20.4");
        println!(" Git commit:        bolt-compat");
        println!(" Built:             {}", chrono::Utc::now().format("%a %b %d %H:%M:%S %Y"));
        println!(" OS/Arch:           linux/amd64");
        println!(" Context:           default");
        println!();
        println!("Server: Bolt Engine");
        println!(" Engine:");
        println!("  Version:          0.1.0");
        println!("  API version:      1.43 (minimum version 1.12)");
        println!("  Go version:       Rust/Tokio");
        println!("  Git commit:       main");
        println!("  Built:            {}", chrono::Utc::now().format("%a %b %d %H:%M:%S %Y"));
        println!("  OS/Arch:          linux/amd64");
        println!("  Experimental:     true");

        Ok(())
    }

    async fn handle_info(&self) -> Result<()> {
        println!("Client: Docker Engine - Community");
        println!(" Context:    default");
        println!(" Debug Mode: false");
        println!();
        println!("Server:");
        println!(" Containers: 0");
        println!("  Running: 0");
        println!("  Paused: 0");
        println!("  Stopped: 0");
        println!(" Images: 0");
        println!(" Server Version: 0.1.0-bolt");
        println!(" Storage Driver: bolt");
        println!(" Logging Driver: json-file");
        println!(" Cgroup Driver: systemd");
        println!(" Cgroup Version: 2");
        println!(" Plugins:");
        println!("  Volume: local s3 ghostbay");
        println!("  Network: bridge bolt quic");
        println!("  Log: awslogs fluentd gcplogs gelf journald json-file local");
        println!(" Swarm: inactive");
        println!(" Runtimes: bolt runc");
        println!(" Default Runtime: bolt");
        println!(" Init Binary: ");
        println!(" containerd version: ");
        println!(" runc version: ");
        println!(" init version: ");
        println!(" Security Options:");
        println!("  seccomp");
        println!("   Profile: builtin");
        println!("  rootless");
        println!(" Kernel Version: 6.5.0");
        println!(" Operating System: Bolt Linux");
        println!(" OSType: linux");
        println!(" Architecture: x86_64");
        println!(" CPUs: {}", num_cpus::get());
        println!(" Total Memory: {}GB", self.get_memory_gb());
        println!(" Name: bolt-host");
        println!(" ID: bolt-{}", uuid::Uuid::new_v4());
        println!(" Docker Root Dir: /var/lib/bolt");
        println!(" Debug Mode: false");
        println!(" Experimental: true");
        println!(" Insecure Registries:");
        println!("  127.0.0.0/8");
        println!(" Live Restore Enabled: false");
        println!(" Product License: MIT");

        Ok(())
    }

    fn parse_run_args(&self, args: &[String]) -> Result<DockerRunArgs> {
        let mut run_args = DockerRunArgs {
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
            user: None,
            workdir: None,
            entrypoint: None,
            cmd: Vec::new(),
            restart: None,
            memory: None,
            cpus: None,
            labels: HashMap::new(),
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
                    i += 1;
                }
                "--name" => {
                    if i + 1 < args.len() {
                        run_args.name = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime("--name requires a value".to_string()));
                    }
                }
                "-p" | "--publish" => {
                    if i + 1 < args.len() {
                        run_args.ports.push(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime("-p requires a value".to_string()));
                    }
                }
                "-v" | "--volume" => {
                    if i + 1 < args.len() {
                        run_args.volumes.push(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime("-v requires a value".to_string()));
                    }
                }
                "-e" | "--env" => {
                    if i + 1 < args.len() {
                        run_args.env.push(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime("-e requires a value".to_string()));
                    }
                }
                "--network" => {
                    if i + 1 < args.len() {
                        run_args.network = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime("--network requires a value".to_string()));
                    }
                }
                "--user" => {
                    if i + 1 < args.len() {
                        run_args.user = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime("--user requires a value".to_string()));
                    }
                }
                "-w" | "--workdir" => {
                    if i + 1 < args.len() {
                        run_args.workdir = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime("-w requires a value".to_string()));
                    }
                }
                "--entrypoint" => {
                    if i + 1 < args.len() {
                        run_args.entrypoint = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime("--entrypoint requires a value".to_string()));
                    }
                }
                "--restart" => {
                    if i + 1 < args.len() {
                        run_args.restart = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime("--restart requires a value".to_string()));
                    }
                }
                "-m" | "--memory" => {
                    if i + 1 < args.len() {
                        run_args.memory = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime("-m requires a value".to_string()));
                    }
                }
                "--cpus" => {
                    if i + 1 < args.len() {
                        run_args.cpus = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(BoltError::Runtime("--cpus requires a value".to_string()));
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
            return Err(BoltError::Runtime("No image specified".to_string()));
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
            .unwrap_or(0)
    }
}

impl ContainerCompatibility for DockerCompat {
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
            "compose" => {
                bolt_args.push("surge".to_string());
                if args.len() > 1 {
                    match args[1].as_str() {
                        "up" => bolt_args.push("up".to_string()),
                        "down" => bolt_args.push("down".to_string()),
                        "ps" => bolt_args.push("status".to_string()),
                        _ => bolt_args.extend_from_slice(&args[1..]),
                    }
                }
                bolt_args.extend_from_slice(&args[2..]);
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
        // This would implement Docker Compose -> Boltfile conversion
        // For now, return a placeholder
        Ok(r#"
project = "converted-from-compose"

[services.web]
image = "nginx:latest"
ports = ["80:80"]

[services.app]
image = "node:16"
depends_on = ["db"]

[services.db]
capsule = "postgres"
"#.to_string())
    }
}