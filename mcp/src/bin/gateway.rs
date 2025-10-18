//! Bolt MCP Gateway Binary
//!
//! Standalone gateway for managing MCP servers

use anyhow::Result;
use bolt_mcp::{GatewayConfig, McpGateway};
use clap::Parser;
use std::path::PathBuf;
use tracing::info;

#[derive(Parser)]
#[command(name = "bolt-mcp-gateway")]
#[command(about = "MCP Gateway for Bolt Container Runtime")]
struct Cli {
    /// Transport type (stdio, websocket, http)
    #[arg(long, default_value = "websocket")]
    transport: String,

    /// Address to bind to
    #[arg(long, default_value = "0.0.0.0")]
    address: String,

    /// Port to bind to
    #[arg(long, default_value = "7331")]
    port: u16,

    /// Path to catalog file
    #[arg(long)]
    catalog: Option<PathBuf>,

    /// Enabled servers (comma-separated)
    #[arg(long)]
    servers: Vec<String>,

    /// Enabled tools (format: server:tool)
    #[arg(long)]
    tools: Vec<String>,

    /// Secret sources
    #[arg(long, default_values_t = vec!["docker-desktop".to_string(), ".env".to_string()])]
    secrets: Vec<String>,

    /// Watch for config changes
    #[arg(long)]
    watch: bool,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(level.parse()?),
        )
        .init();

    info!("🚀 Starting Bolt MCP Gateway");

    // Build gateway config
    let mut config = GatewayConfig {
        transport: cli.transport,
        address: cli.address,
        port: cli.port,
        catalog_path: cli.catalog.unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("bolt/mcp-catalog.toml")
        }),
        enabled_servers: cli.servers,
        enabled_tools: cli.tools,
        secret_sources: cli.secrets,
        watch: cli.watch,
        verbose: cli.verbose,
        ..Default::default()
    };

    // Create and run gateway
    let gateway = McpGateway::new(config).await?;
    gateway.run().await?;

    Ok(())
}
