use anyhow::Result;
use tracing::{info, warn};

pub struct OverlayDriver {
    pub root_path: std::path::PathBuf,
}

impl OverlayDriver {
    pub fn new(root_path: std::path::PathBuf) -> Result<Self> {
        info!("📂 Initializing overlay storage driver");
        Ok(Self { root_path })
    }

    pub async fn create_layer(&self, layer_id: &str) -> Result<()> {
        info!("Creating overlay layer: {}", layer_id);
        warn!("Overlay driver not yet implemented");
        Ok(())
    }
}