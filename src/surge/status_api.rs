use crate::Result;
use crate::config::BoltConfig;
use crate::{ServiceInfo, SurgeStatus};

// API-only functions for library usage
pub async fn status_info(config: &BoltConfig) -> Result<SurgeStatus> {
    let boltfile = config.load_boltfile()?;

    let mut services = Vec::new();
    for (name, _service) in &boltfile.services {
        services.push(ServiceInfo {
            name: name.clone(),
            status: "not running".to_string(), // TODO: Implement actual status
            replicas: 1,                       // TODO: Implement actual replica count
        });
    }

    Ok(SurgeStatus {
        services,
        networks: vec![], // TODO: Implement network status
    })
}
