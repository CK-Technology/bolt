use crate::compat::{docker::DockerCompat, compose::ComposeCompat, DockerApiCompat};
use crate::error::Result;
use crate::runtime::BoltRuntime;
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
    },
    /// Show migration guide from Docker/Compose to Bolt
    Migrate {
        /// Path to compose file to analyze
        #[arg(short, long)]
        compose_file: Option<PathBuf>,
    },
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
        CompatCommands::Docker { args } => {
            handle_docker_command(args, runtime).await
        }
        CompatCommands::Compose { command } => {
            handle_compose_command(command).await
        }
        CompatCommands::ApiServer { port, bind } => {
            handle_api_server(port, bind, runtime).await
        }
        CompatCommands::Migrate { compose_file } => {
            handle_migration_guide(compose_file).await
        }
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
        ComposeCommands::Convert { input, output, notes } => {
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

async fn handle_api_server(port: u16, bind: String, runtime: BoltRuntime) -> Result<()> {
    use tokio::net::TcpListener;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    println!("🚀 Starting Docker API Compatibility Server");
    println!("   Address: http://{}:{}", bind, port);
    println!("   Docker API Version: 1.43");
    println!("   Backend: Bolt Runtime");
    println!();

    let api_compat = DockerApiCompat::new(runtime);
    let listener = TcpListener::bind(format!("{}:{}", bind, port)).await?;

    println!("✅ Server listening on {}:{}", bind, port);
    println!("💡 Test with: export DOCKER_HOST=tcp://{}:{}", bind, port);
    println!();

    loop {
        let (mut socket, addr) = listener.accept().await?;
        let api_compat = api_compat.clone();

        tokio::spawn(async move {
            let mut reader = BufReader::new(&mut socket);
            let mut request_line = String::new();

            if let Err(e) = reader.read_line(&mut request_line).await {
                eprintln!("Failed to read request: {}", e);
                return;
            }

            let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
            if parts.len() < 2 {
                return;
            }

            let method = parts[0];
            let path = parts[1];

            // Read headers (simplified)
            let mut headers = Vec::new();
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).await.is_err() {
                    break;
                }
                if header.trim().is_empty() {
                    break;
                }
                headers.push(header);
            }

            // Read body if present (simplified)
            let body = String::new(); // Would need proper content-length handling

            println!("📡 {} {} from {}", method, path, addr);

            match api_compat.handle_request(path, method, &body).await {
                Ok(response) => {
                    let http_response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                        response.len(),
                        response
                    );

                    if let Err(e) = socket.write_all(http_response.as_bytes()).await {
                        eprintln!("Failed to write response: {}", e);
                    }
                }
                Err(e) => {
                    let error_response = format!(
                        "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\n\r\n{{\"error\": \"{}\"}}\r\n",
                        e
                    );

                    if let Err(e) = socket.write_all(error_response.as_bytes()).await {
                        eprintln!("Failed to write error response: {}", e);
                    }
                }
            }
        });
    }
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