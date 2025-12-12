use crate::Result;
use crate::config::BoltConfig;
use crate::{NetworkInfo, ServiceInfo, SurgeStatus};
use std::process::Command;

// API-only functions for library usage
pub async fn status_info(config: &BoltConfig) -> Result<SurgeStatus> {
    let boltfile = config.load_boltfile()?;

    let mut services = Vec::new();
    for (name, _service_config) in boltfile.services.iter() {
        // Check if container is running by looking for container with this service name
        let status = check_container_status(name);

        services.push(ServiceInfo {
            name: name.clone(),
            status,
            replicas: 1, // Single replica for now (scale feature not yet implemented)
        });
    }

    // Get network status
    let networks = list_bolt_networks();

    Ok(SurgeStatus { services, networks })
}

fn check_container_status(service_name: &str) -> String {
    // Check if container exists and is running
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "test -d /run/bolt/containers/*{}* && echo running || echo stopped",
            service_name
        ))
        .output();

    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}

fn list_bolt_networks() -> Vec<NetworkInfo> {
    // List networks created by Bolt
    let output = Command::new("sh")
        .arg("-c")
        .arg("ip link show | grep br-bolt | awk '{print $2}' | sed 's/:$//'")
        .output();

    let network_names = match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect(),
        _ => vec!["br-bolt0".to_string()], // Default network
    };

    network_names
        .into_iter()
        .map(|name| NetworkInfo {
            id: format!("bolt-net-{}", &name[8..]), // Generate ID from name
            name: name.clone(),
            driver: "bridge".to_string(),
            subnet: Some("172.20.0.0/16".to_string()),
            created: None,
        })
        .collect()
}
