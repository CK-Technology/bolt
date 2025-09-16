//! Nova Integration Example
//!
//! This example demonstrates how to integrate Bolt into Nova's Velocity Manager
//! for unified VM and container management.

use bolt::api::*;
use std::collections::HashMap;
use tokio;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 Bolt-Nova Integration Example");
    println!("================================");

    // 1. Initialize Bolt Nova Runtime
    println!("\n1. Initializing Bolt Nova Runtime...");
    let runtime = BoltNovaRuntime::new().await?;

    // 2. Initialize Bridge Network Manager
    println!("2. Setting up Nova bridge networks...");
    let mut bridge_manager = NovaBridgeManager::new();

    // Create a Nova bridge network
    let bridge_config = NovaBridgeConfig {
        name: "nova-br0".to_string(),
        subnet: "172.20.0.0/16".to_string(),
        gateway: "172.20.0.1".to_string(),
        dns_servers: vec!["8.8.8.8".to_string(), "1.1.1.1".to_string()],
        mtu: 1500,
        enable_quic: true,
    };

    bridge_manager.create_bridge(bridge_config.clone()).await?;
    println!("✓ Created bridge network: {}", bridge_config.name);

    // 3. Configure container from Nova TOML configuration
    println!("\n3. Configuring container from Nova TOML...");

    // This simulates parsing a NovaFile section:
    // [container.web]
    // capsule = "nginx:latest"
    // volumes = ["./html:/usr/share/nginx/html"]
    // network = "nova-br0"
    // env.ENVIRONMENT = "production"
    // gpu_passthrough = false

    let mut env = HashMap::new();
    env.insert("ENVIRONMENT".to_string(), "production".to_string());

    let nova_config = NovaContainerConfig {
        capsule: "nginx:latest".to_string(),
        volumes: vec!["./html:/usr/share/nginx/html".to_string()],
        network: "nova-br0".to_string(),
        env,
        gpu_passthrough: false,
        memory_mb: Some(512),
        cpus: Some(1),
    };

    // 4. Start the container
    println!("4. Starting container 'web'...");
    let handle = runtime.start_capsule("web", &nova_config).await?;
    println!("✓ Container started: {} (status: {:?})", handle.name, handle.status);

    // 5. Connect to Nova bridge network
    println!("5. Connecting to Nova bridge network...");
    let veth_interface = bridge_manager.connect_container(&bridge_config.name, &handle.id).await?;
    println!("✓ Connected via interface: {}", veth_interface);

    // 6. Register with Nova service discovery
    println!("6. Registering with Nova service discovery...");
    let mut service_discovery = NovaServiceDiscovery::new();

    let service_entry = ServiceEntry {
        name: "web-service".to_string(),
        container_id: handle.id.clone(),
        ip_address: "172.20.0.10".to_string(), // Would be dynamically assigned
        ports: vec![80, 443],
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("environment".to_string(), "production".to_string());
            meta.insert("type".to_string(), "web".to_string());
            meta
        },
    };

    service_discovery.register_service(service_entry)?;
    println!("✓ Service registered in Nova DNS");

    // 7. Monitor container status and metrics
    println!("\n7. Monitoring container...");
    let status = runtime.get_capsule_status("web").await?;
    println!("Status: {:?}", status);

    let metrics = runtime.get_capsule_metrics("web").await?;
    println!("Metrics: CPU: {:.1}%, Memory: {} MB",
             metrics.cpu_usage_percent, metrics.memory_usage_mb);

    // 8. Gaming container example
    println!("\n8. Creating gaming container example...");

    let mut gaming_env = HashMap::new();
    gaming_env.insert("DISPLAY".to_string(), ":0".to_string());
    gaming_env.insert("PULSE_RUNTIME_PATH".to_string(), "/run/user/1000/pulse".to_string());

    let gaming_config = NovaContainerConfig {
        capsule: "lutris/games:latest".to_string(),
        volumes: vec![
            "/home/user/Games:/games".to_string(),
            "/tmp/.X11-unix:/tmp/.X11-unix".to_string(),
        ],
        network: "nova-br0".to_string(),
        env: gaming_env,
        gpu_passthrough: true,
        memory_mb: Some(8192),
        cpus: Some(4),
    };

    let gaming_handle = runtime.start_capsule("gaming", &gaming_config).await?;
    println!("✓ Gaming container started: {}", gaming_handle.name);

    // Configure GPU passthrough
    runtime.configure_gpu_passthrough("gaming", "nvidia0").await?;
    println!("✓ GPU passthrough configured");

    // 9. List all containers managed by Nova
    println!("\n9. Listing all containers...");
    let containers = runtime.list_capsules().await?;
    for container in containers {
        println!("  - {} ({}): {:?}", container.name, container.id, container.status);
    }

    // 10. Demonstrate lifecycle management
    println!("\n10. Demonstrating lifecycle management...");

    // Stop the web container
    runtime.stop_capsule("web").await?;
    println!("✓ Stopped web container");

    // Check status
    let status = runtime.get_capsule_status("web").await?;
    println!("Web container status: {:?}", status);

    // Restart it
    // runtime.restart_capsule("web").await?;
    // println!("✓ Restarted web container");

    // 11. Cleanup
    println!("\n11. Cleaning up...");

    // Disconnect from bridge
    bridge_manager.disconnect_container(&bridge_config.name, &gaming_handle.id).await?;
    println!("✓ Disconnected gaming container from bridge");

    // Remove containers
    runtime.remove_capsule("web", true).await?;
    runtime.remove_capsule("gaming", true).await?;
    println!("✓ Removed containers");

    println!("\n🎉 Nova integration example completed successfully!");
    println!("\nThis demonstrates how Nova can manage Bolt containers alongside VMs:");
    println!("  • Unified configuration via NovaFiles");
    println!("  • Bridge network integration");
    println!("  • Service discovery");
    println!("  • GPU passthrough for gaming");
    println!("  • Lifecycle management");
    println!("  • Resource monitoring");

    Ok(())
}

/// Example of how Nova would integrate this into its GUI
pub struct NovaGuiIntegration {
    runtime: BoltNovaRuntime,
    bridge_manager: NovaBridgeManager,
    service_discovery: NovaServiceDiscovery,
}

impl NovaGuiIntegration {
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            runtime: BoltNovaRuntime::new().await?,
            bridge_manager: NovaBridgeManager::new(),
            service_discovery: NovaServiceDiscovery::new(),
        })
    }

    /// Create a new container from Nova GUI
    pub async fn create_container_from_gui(
        &self,
        name: String,
        image: String,
        memory_mb: u64,
        cpus: u32,
        gpu_enabled: bool,
        volumes: Vec<String>,
        env_vars: HashMap<String, String>,
    ) -> anyhow::Result<CapsuleHandle> {
        let config = NovaContainerConfig {
            capsule: image,
            volumes,
            network: "nova-br0".to_string(),
            env: env_vars,
            gpu_passthrough: gpu_enabled,
            memory_mb: Some(memory_mb),
            cpus: Some(cpus),
        };

        self.runtime.start_capsule(&name, &config).await
    }

    /// Get container status for GUI display
    pub async fn get_container_status_for_gui(&self, name: &str) -> anyhow::Result<(NovaStatus, CapsuleMetrics)> {
        let status = self.runtime.get_capsule_status(name).await?;
        let metrics = self.runtime.get_capsule_metrics(name).await?;
        Ok((status, metrics))
    }

    /// List all containers for GUI table view
    pub async fn list_containers_for_gui(&self) -> anyhow::Result<Vec<(CapsuleHandle, CapsuleMetrics)>> {
        let containers = self.runtime.list_capsules().await?;
        let mut result = Vec::new();

        for container in containers {
            let metrics = self.runtime.get_capsule_metrics(&container.name).await?;
            result.push((container, metrics));
        }

        Ok(result)
    }
}