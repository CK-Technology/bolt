use bolt::compat::compose::ComposeCompat;
use bolt::compat::docker::DockerCompat;
use bolt::{BoltRuntime, Result};
use clap::{Args, Subcommand};
use std::fs;
use std::path::PathBuf;

#[derive(Args)]
pub struct CompatArgs {
    #[command(subcommand)]
    pub command: CompatCommands,
}

#[derive(Subcommand)]
pub enum CompatCommands {
    /// Run Docker CLI commands through Bolt compatibility layer
    Docker {
        /// Docker command and arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Convert Docker Compose file to Boltfile
    Compose {
        #[command(subcommand)]
        command: ComposeCommands,
    },
    /// Start Docker API compatibility server
    ApiServer {
        /// Port to bind to
        #[arg(short, long, default_value = "2375")]
        port: u16,
        /// Bind address
        #[arg(short, long, default_value = "127.0.0.1")]
        bind: String,
        /// Also listen on Unix socket (e.g., /var/run/bolt/docker.sock)
        #[arg(short, long)]
        socket: Option<String>,
        /// Create symlink from /var/run/docker.sock to bolt socket (requires sudo)
        #[arg(long)]
        docker_compat: bool,
    },
    /// Show migration guide from Docker/Compose to Bolt
    Migrate {
        /// Path to compose file to analyze
        #[arg(short, long)]
        compose_file: Option<PathBuf>,
    },
    /// Setup Grafana integration for Bolt metrics
    Grafana {
        #[command(subcommand)]
        command: GrafanaCommands,
    },
}

#[derive(Subcommand)]
pub enum GrafanaCommands {
    /// Install Grafana datasource and dashboard configurations
    Setup {
        /// Grafana provisioning directory (default: /etc/grafana/provisioning)
        #[arg(short, long)]
        grafana_dir: Option<PathBuf>,
    },
    /// Show Grafana setup instructions
    Instructions,
}

#[derive(Subcommand)]
pub enum ComposeCommands {
    /// Convert compose file to Boltfile
    Convert {
        /// Input compose file (docker-compose.yml)
        #[arg(short, long, default_value = "docker-compose.yml")]
        input: PathBuf,
        /// Output Boltfile path
        #[arg(short, long, default_value = "Boltfile.toml")]
        output: PathBuf,
        /// Show migration notes
        #[arg(short, long)]
        notes: bool,
    },
    /// Validate compose file for conversion
    Validate {
        /// Compose file to validate
        #[arg(default_value = "docker-compose.yml")]
        file: PathBuf,
    },
    /// Show migration recommendations
    Analyze {
        /// Compose file to analyze
        #[arg(default_value = "docker-compose.yml")]
        file: PathBuf,
    },
}

pub async fn handle_compat_command(args: CompatArgs, runtime: BoltRuntime) -> Result<()> {
    match args.command {
        CompatCommands::Docker { args } => handle_docker_command(args, runtime).await,
        CompatCommands::Compose { command } => handle_compose_command(command).await,
        CompatCommands::ApiServer { port, bind, socket, docker_compat } => {
            handle_api_server(port, bind, socket, docker_compat, runtime).await
        }
        CompatCommands::Migrate { compose_file } => handle_migration_guide(compose_file).await,
        CompatCommands::Grafana { command } => handle_grafana_command(command).await,
    }
}

async fn handle_docker_command(args: Vec<String>, runtime: BoltRuntime) -> Result<()> {
    if args.is_empty() {
        print_docker_help();
        return Ok(());
    }

    println!("🐳 Docker Compatibility Mode");
    println!("   Running: docker {}", args.join(" "));
    println!("   Via: Bolt Runtime");
    println!();

    let docker_compat = DockerCompat::new(runtime);
    docker_compat.execute_docker_command(&args).await?;

    Ok(())
}

async fn handle_compose_command(command: ComposeCommands) -> Result<()> {
    match command {
        ComposeCommands::Convert {
            input,
            output,
            notes,
        } => {
            println!("🔄 Converting Docker Compose to Boltfile");
            println!("   Input: {}", input.display());
            println!("   Output: {}", output.display());
            println!();

            let compose_content = fs::read_to_string(&input)?;

            // Validate first
            let warnings = ComposeCompat::validate_compose_file(&compose_content)?;
            if !warnings.is_empty() {
                println!("⚠️  Validation Warnings:");
                for warning in &warnings {
                    println!("   • {}", warning);
                }
                println!();
            }

            // Convert
            let boltfile_content = ComposeCompat::convert_compose_file(&compose_content)?;
            fs::write(&output, boltfile_content)?;

            println!("✅ Conversion completed!");
            println!("   Generated: {}", output.display());

            if notes {
                println!();
                let migration_notes = ComposeCompat::generate_migration_notes(&compose_content)?;
                println!("{}", migration_notes);
            }

            println!();
            println!("🚀 Next steps:");
            println!("   1. Review the generated Boltfile.toml");
            println!("   2. Test with: bolt surge up");
            println!("   3. Check status: bolt surge status");
        }

        ComposeCommands::Validate { file } => {
            println!("🔍 Validating Compose file: {}", file.display());

            let compose_content = fs::read_to_string(&file)?;
            let warnings = ComposeCompat::validate_compose_file(&compose_content)?;

            if warnings.is_empty() {
                println!("✅ No conversion issues found");
            } else {
                println!("⚠️  Found {} potential issues:", warnings.len());
                for (i, warning) in warnings.iter().enumerate() {
                    println!("   {}. {}", i + 1, warning);
                }
            }
        }

        ComposeCommands::Analyze { file } => {
            println!("📊 Analyzing Compose file: {}", file.display());

            let compose_content = fs::read_to_string(&file)?;
            let analysis = ComposeCompat::generate_migration_notes(&compose_content)?;

            println!();
            println!("{}", analysis);
        }
    }

    Ok(())
}

async fn handle_api_server(port: u16, bind: String, socket: Option<String>, docker_compat: bool, runtime: BoltRuntime) -> Result<()> {
    use bolt::docker_compat::api_server::DockerAPIServer;
    use std::sync::Arc;

    println!("🚀 Starting Docker API Compatibility Server");
    println!("   Address: http://{}:{}", bind, port);
    println!("   Docker API Version: 1.43");
    println!("   Backend: Bolt Runtime");

    let runtime_arc = Arc::new(runtime);
    let api_server = DockerAPIServer::new(runtime_arc.clone()).with_address(bind.clone(), port);

    // Determine socket path
    let socket_path = if docker_compat {
        "/var/run/bolt/bolt.sock".to_string()
    } else {
        socket.unwrap_or_else(|| "/var/run/bolt/bolt.sock".to_string())
    };

    // Start Unix socket server
    println!("   Unix Socket: {}", socket_path);
    api_server.start_unix_socket(&socket_path).await?;

    // Create Docker-compatible symlink if requested
    if docker_compat {
        create_docker_symlink(&socket_path)?;
    }

    if docker_compat {
        println!("💡 Docker CLI works directly: docker ps");
        println!("💡 Or set: export DOCKER_HOST=unix:///var/run/docker.sock");
    } else {
        println!("💡 Test with: export DOCKER_HOST=unix://{}", socket_path);
    }
    println!("💡 Or use TCP: export DOCKER_HOST=tcp://{}:{}", bind, port);
    println!();

    // Start the main TCP API server (this will block)
    api_server.start().await?;

    Ok(())
}

fn create_docker_symlink(bolt_socket: &str) -> Result<()> {
    use std::os::unix::fs::symlink;
    use std::path::Path;

    let docker_sock = Path::new("/var/run/docker.sock");

    // Remove existing socket/symlink if present
    if docker_sock.exists() || docker_sock.symlink_metadata().is_ok() {
        println!("⚠️  Removing existing /var/run/docker.sock");
        std::fs::remove_file(docker_sock).ok();
    }

    // Create parent directory
    std::fs::create_dir_all("/var/run")?;

    // Create symlink
    symlink(bolt_socket, docker_sock)?;

    println!("✅ Created symlink: /var/run/docker.sock -> {}", bolt_socket);
    println!("   Docker CLI commands now work natively!");

    Ok(())
}

async fn handle_migration_guide(compose_file: Option<PathBuf>) -> Result<()> {
    println!("🚚 Docker to Bolt Migration Guide");
    println!("=====================================");
    println!();

    if let Some(file) = compose_file {
        println!("📁 Analyzing your compose file: {}", file.display());
        let compose_content = fs::read_to_string(&file)?;
        let analysis = ComposeCompat::generate_migration_notes(&compose_content)?;
        println!("{}", analysis);
    } else {
        print_general_migration_guide();
    }

    Ok(())
}

fn print_docker_help() {
    println!("🐳 Docker Compatibility Layer for Bolt");
    println!();
    println!("SUPPORTED COMMANDS:");
    println!("  run      Run a container (maps to Bolt runtime)");
    println!("  ps       List containers");
    println!("  stop     Stop containers");
    println!("  rm       Remove containers");
    println!("  images   List images");
    println!("  pull     Pull images");
    println!("  build    Build images");
    println!("  network  Network management");
    println!("  volume   Volume management (coming soon)");
    println!("  version  Show version info");
    println!("  info     Show system info");
    println!();
    println!("EXAMPLES:");
    println!("  bolt compat docker run -d -p 8080:80 nginx:latest");
    println!("  bolt compat docker ps");
    println!("  bolt compat docker stop mycontainer");
    println!();
    println!("💡 For full compatibility, consider using 'bolt compat api-server'");
}

fn print_general_migration_guide() {
    println!("## Overview");
    println!("Bolt provides multiple pathways to migrate from Docker/Compose:");
    println!();
    println!("## 1. CLI Compatibility Layer");
    println!("Run Docker commands through Bolt:");
    println!("```bash");
    println!("bolt compat docker run -d nginx:latest");
    println!("bolt compat docker ps");
    println!("```");
    println!();
    println!("## 2. Docker Compose Migration");
    println!("Convert existing compose files:");
    println!("```bash");
    println!("bolt compat compose convert -i docker-compose.yml -o Boltfile.toml");
    println!("bolt surge up");
    println!("```");
    println!();
    println!("## 3. Docker API Compatibility");
    println!("For tools that use Docker API:");
    println!("```bash");
    println!("bolt compat api-server --port 2375");
    println!("export DOCKER_HOST=tcp://localhost:2375");
    println!("```");
    println!();
    println!("## 4. Native Bolt Migration");
    println!("For best performance and features:");
    println!("- Create Boltfiles manually for optimal configuration");
    println!("- Use Bolt capsules for databases and stateful services");
    println!("- Leverage QUIC networking for distributed applications");
    println!("- Enable gaming optimizations for relevant workloads");
    println!();
    println!("## Key Differences");
    println!("| Feature | Docker | Bolt |");
    println!("|---------| -------|------|");
    println!("| Config Format | YAML/CLI | TOML (Boltfiles) |");
    println!("| Orchestration | docker-compose | surge (built-in) |");
    println!("| Networking | bridge/overlay | bridge/bolt/quic |");
    println!("| Storage | volumes | local/s3/ghostbay |");
    println!("| Runtime | runc | bolt-runtime + capsules |");
    println!();
    println!("## Migration Strategy");
    println!("1. **Assessment**: Use `bolt compat compose analyze` on existing files");
    println!("2. **Conversion**: Convert compose files to Boltfiles");
    println!("3. **Testing**: Deploy individual services first");
    println!("4. **Optimization**: Leverage Bolt-specific features");
    println!("5. **Production**: Full migration with monitoring");
    println!();
    println!("Run 'bolt compat compose --help' for conversion tools.");
}

async fn handle_grafana_command(command: GrafanaCommands) -> Result<()> {
    match command {
        GrafanaCommands::Setup { grafana_dir } => {
            println!("📊 Setting up Grafana integration for Bolt metrics");
            println!();

            // Determine grafana provisioning directory
            let grafana_base = grafana_dir.unwrap_or_else(|| PathBuf::from("/etc/grafana/provisioning"));
            let datasources_dir = grafana_base.join("datasources");
            let dashboards_dir = grafana_base.join("dashboards");

            // Check if running as root/sudo for system installation
            let is_root = std::env::var("USER").unwrap_or_default() == "root"
                || std::env::var("SUDO_USER").is_ok();

            if !is_root && grafana_base == PathBuf::from("/etc/grafana/provisioning") {
                println!("⚠️  System Grafana installation detected but not running as root.");
                println!("   Run with sudo to install to system Grafana:");
                println!("   sudo bolt compat grafana setup");
                println!();
                println!("   Or install to custom directory:");
                println!("   bolt compat grafana setup --grafana-dir ./grafana");
                return Ok(());
            }

            // Create directories
            println!("📁 Creating provisioning directories...");
            std::fs::create_dir_all(&datasources_dir)?;
            std::fs::create_dir_all(&dashboards_dir)?;

            // Copy datasource config
            println!("📝 Installing Prometheus datasource config...");
            let datasource_content = include_str!("../../grafana/datasource.yaml");
            let datasource_path = datasources_dir.join("bolt.yaml");
            std::fs::write(&datasource_path, datasource_content)?;
            println!("   ✅ {}", datasource_path.display());

            // Copy dashboard JSON
            println!("📝 Installing Bolt metrics dashboard...");
            let dashboard_content = include_str!("../../grafana/bolt-dashboard.json");
            let dashboard_path = dashboards_dir.join("bolt-dashboard.json");
            std::fs::write(&dashboard_path, dashboard_content)?;
            println!("   ✅ {}", dashboard_path.display());

            // Create dashboard provider config
            let provider_config = r#"apiVersion: 1

providers:
  - name: 'Bolt Dashboards'
    orgId: 1
    folder: 'Bolt'
    type: file
    disableDeletion: false
    updateIntervalSeconds: 10
    allowUiUpdates: true
    options:
      path: /etc/grafana/provisioning/dashboards
"#;
            let provider_path = dashboards_dir.join("bolt-provider.yaml");
            std::fs::write(&provider_path, provider_config)?;
            println!("   ✅ {}", provider_path.display());

            println!();
            println!("✅ Grafana integration installed successfully!");
            println!();
            println!("📋 Next steps:");
            println!("   1. Ensure Prometheus is running on localhost:9090");
            println!("      bolt monitoring prometheus --port 9090");
            println!();
            println!("   2. Restart Grafana to load the new configuration:");
            println!("      sudo systemctl restart grafana-server");
            println!();
            println!("   3. Access the dashboard:");
            println!("      • Open http://localhost:3000 (default Grafana)");
            println!("      • Navigate to: Dashboards → Bolt → Bolt Container Runtime");
            println!();
            println!("   4. View real-time metrics:");
            println!("      • Container CPU/Memory usage");
            println!("      • GPU utilization and VRAM");
            println!("      • Network throughput (RX/TX)");
            println!("      • QUIC networking latency");
            println!();
        }
        GrafanaCommands::Instructions => {
            println!("📊 Grafana Integration Instructions for Bolt");
            println!("=============================================");
            println!();
            println!("## Prerequisites");
            println!("1. Install Grafana (if not already installed):");
            println!("   # Ubuntu/Debian");
            println!("   sudo apt-get install -y grafana");
            println!();
            println!("   # Fedora/RHEL");
            println!("   sudo dnf install grafana");
            println!();
            println!("   # Using Docker");
            println!("   docker run -d -p 3000:3000 grafana/grafana");
            println!();
            println!("2. Install Prometheus:");
            println!("   # Ubuntu/Debian");
            println!("   sudo apt-get install -y prometheus");
            println!();
            println!("   # Or use Bolt's built-in Prometheus exporter");
            println!("   bolt monitoring prometheus");
            println!();
            println!("## Quick Setup");
            println!("Run the automated setup command:");
            println!("   sudo bolt compat grafana setup");
            println!();
            println!("## Manual Setup");
            println!("1. Copy datasource config:");
            println!("   sudo cp grafana/datasource.yaml /etc/grafana/provisioning/datasources/bolt.yaml");
            println!();
            println!("2. Copy dashboard config:");
            println!("   sudo cp grafana/bolt-dashboard.json /etc/grafana/provisioning/dashboards/");
            println!();
            println!("3. Restart Grafana:");
            println!("   sudo systemctl restart grafana-server");
            println!();
            println!("## Available Metrics");
            println!("• bolt_container_cpu_usage_seconds_total - Container CPU time");
            println!("• bolt_container_memory_usage_bytes - Container memory usage");
            println!("• bolt_containers_running_total - Total running containers");
            println!("• bolt_gpu_utilization_percent - GPU usage percentage");
            println!("• bolt_gpu_memory_used_bytes - GPU VRAM usage");
            println!("• bolt_gpu_memory_total_bytes - Total GPU VRAM");
            println!("• bolt_network_rx_bytes_total - Network bytes received");
            println!("• bolt_network_tx_bytes_total - Network bytes transmitted");
            println!("• bolt_quic_latency_microseconds - QUIC connection latency");
            println!("• bolt_container_started_total - Container start events");
            println!();
            println!("## Dashboard Features");
            println!("✓ Real-time container CPU and memory charts");
            println!("✓ GPU utilization and VRAM monitoring");
            println!("✓ Network throughput visualization");
            println!("✓ QUIC networking latency tracking");
            println!("✓ Container lifecycle annotations");
            println!("✓ Auto-refresh every 5 seconds");
            println!();
            println!("## Troubleshooting");
            println!("• If datasource shows as disconnected:");
            println!("  - Ensure Prometheus is running: systemctl status prometheus");
            println!("  - Check Prometheus URL: http://localhost:9090");
            println!();
            println!("• If no data appears:");
            println!("  - Verify Bolt containers are running: bolt ps");
            println!("  - Check metrics endpoint: curl http://localhost:9090/metrics");
            println!();
            println!("For more help, visit: https://github.com/CK-Technology/bolt/docs");
        }
    }

    Ok(())
}
