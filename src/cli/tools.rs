use anyhow::{Context, Result};
use bolt::config::BoltFile;
use clap::Subcommand;
use std::path::Path;

#[derive(Subcommand)]
pub enum ToolCommands {
    /// List all native Bolt tools
    List,

    /// Inspect native tools enabled for a Boltfile service
    Inspect {
        /// Service name from Boltfile.toml
        service: String,
    },
}

pub async fn execute(command: &ToolCommands, config_path: &Path) -> Result<()> {
    match command {
        ToolCommands::List => list_tools(),
        ToolCommands::Inspect { service } => inspect_service_tools(config_path, service),
    }
}

fn list_tools() -> Result<()> {
    println!("Native Bolt service tools");
    println!();
    for tool in bolt::tools::all_tools() {
        println!(
            "{:<22} {:<18} {}",
            tool.name, tool.permission_scope, tool.description
        );
    }
    Ok(())
}

fn inspect_service_tools(config_path: &Path, service_name: &str) -> Result<()> {
    let boltfile = BoltFile::load(config_path)
        .with_context(|| format!("Failed to load Boltfile at {}", config_path.display()))?;
    let service = boltfile
        .services
        .get(service_name)
        .with_context(|| format!("Service '{}' not found in Boltfile", service_name))?;

    println!("Native tools for service '{}'", service_name);
    println!();

    let enabled = bolt::tools::enabled_tools_for_service(service);
    if enabled.is_empty() {
        println!("No native tools enabled.");
        return Ok(());
    }

    for tool in enabled {
        println!("{:<22} {}", tool.name, tool.description);
    }

    if let Some(permissions) = bolt::tools::permissions_for_service(service) {
        println!();
        println!("Permissions");
        println!("  filesystem_roots: {:?}", permissions.filesystem_roots);
        println!("  shell_commands:   {:?}", permissions.shell_commands);
        println!("  gpu_access:       {:?}", permissions.gpu_access);
        println!("  network_access:   {:?}", permissions.network_access);
        println!("  process_access:   {:?}", permissions.process_access);
    }

    Ok(())
}
