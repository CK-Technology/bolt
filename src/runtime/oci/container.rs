use anyhow::Result;
use tracing::{info, warn};

pub struct Container {
    pub id: String,
    pub status: ContainerStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContainerStatus {
    Created,
    Running,
    Stopped,
}

impl Container {
    pub fn new(id: String) -> Self {
        Self {
            id,
            status: ContainerStatus::Created,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting container: {}", self.id);
        self.status = ContainerStatus::Running;
        warn!("Container execution not yet implemented");
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        info!("🛑 Stopping container: {}", self.id);
        self.status = ContainerStatus::Stopped;
        Ok(())
    }
}