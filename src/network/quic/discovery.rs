use anyhow::Result;
use std::net::SocketAddr;
use tracing::{info, warn};

use super::ServiceType;

pub struct ServiceDiscovery {
    pub node_id: String,
}

impl ServiceDiscovery {
    pub async fn new(node_id: String) -> Result<Self> {
        Ok(Self { node_id })
    }

    pub async fn start(&self) -> Result<()> {
        info!("🔍 Starting service discovery for node: {}", self.node_id);
        Ok(())
    }

    pub async fn register_service(
        &self,
        name: &str,
        addr: SocketAddr,
        service_type: ServiceType,
    ) -> Result<()> {
        info!("📝 Registering service {} ({:?}) at {}", name, service_type, addr);
        warn!("Service discovery not yet implemented");
        Ok(())
    }
}