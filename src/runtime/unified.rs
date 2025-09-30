use crate::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::native::{BoltNativeRuntime, NativeContainerConfig, NativeContainerInfo};

/// Unified runtime interface that can switch between native and delegation modes
#[derive(Debug)]
pub struct UnifiedRuntime {
    native: Arc<RwLock<BoltNativeRuntime>>,
    mode: RuntimeMode,
}

#[derive(Debug, Clone)]
pub enum RuntimeMode {
    Native,           // Use native Bolt runtime
    Delegate(String), // Delegate to docker/podman (fallback)
}

impl UnifiedRuntime {
    pub async fn new() -> Result<Self> {
        info!("🚀 Initializing Unified Bolt Runtime");

        // Try to initialize native runtime
        match BoltNativeRuntime::new().await {
            Ok(native) => {
                info!("✅ Native runtime initialized successfully");
                Ok(Self {
                    native: Arc::new(RwLock::new(native)),
                    mode: RuntimeMode::Native,
                })
            }
            Err(e) => {
                warn!("⚠️  Native runtime failed to initialize: {}", e);
                warn!("   Falling back to delegation mode");

                // Fallback to delegation mode
                let fallback_runtime = super::detect_container_runtime().await?;

                // Create a dummy native runtime for API compatibility
                let dummy_native = BoltNativeRuntime::new().await?;

                Ok(Self {
                    native: Arc::new(RwLock::new(dummy_native)),
                    mode: RuntimeMode::Delegate(fallback_runtime),
                })
            }
        }
    }

    /// Run a container - unified interface
    pub async fn run_container(
        &self,
        image: &str,
        name: Option<&str>,
        ports: &[String],
        env: &[String],
        volumes: &[String],
        detach: bool,
    ) -> Result<String> {
        match &self.mode {
            RuntimeMode::Native => {
                let config = NativeContainerConfig {
                    image: image.to_string(),
                    name: name.map(|s| s.to_string()),
                    ports: ports.to_vec(),
                    env: env.to_vec(),
                    volumes: volumes.to_vec(),
                    detach,
                    command: None,
                    working_dir: None,
                    user: None,
                    gpu_config: None,
                    cpu_affinity: None,
                    workload_hint: None,
                };

                let mut native = self.native.write().await;
                native.run_container(config).await
            }
            RuntimeMode::Delegate(runtime) => {
                // Fall back to old delegation method
                super::run_oci_container_delegate(runtime, image, name, ports, env, volumes, detach)
                    .await
            }
        }
    }

    /// Stop a container
    pub async fn stop_container(&self, id: &str) -> Result<()> {
        match &self.mode {
            RuntimeMode::Native => {
                let mut native = self.native.write().await;
                native.stop_container(id).await
            }
            RuntimeMode::Delegate(runtime) => super::stop_container_delegate(runtime, id).await,
        }
    }

    /// Remove a container
    pub async fn remove_container(&self, id: &str, force: bool) -> Result<()> {
        match &self.mode {
            RuntimeMode::Native => {
                let mut native = self.native.write().await;
                native.remove_container(id, force).await
            }
            RuntimeMode::Delegate(runtime) => {
                super::remove_container_delegate(runtime, id, force).await
            }
        }
    }

    /// List containers
    pub async fn list_containers(&self, all: bool) -> Result<Vec<NativeContainerInfo>> {
        match &self.mode {
            RuntimeMode::Native => {
                let native = self.native.read().await;
                native.list_containers(all).await
            }
            RuntimeMode::Delegate(runtime) => super::list_containers_delegate(runtime, all).await,
        }
    }

    /// Pull an image
    pub async fn pull_image(&self, image: &str) -> Result<()> {
        match &self.mode {
            RuntimeMode::Native => {
                let mut native = self.native.write().await;
                native.pull_image_native(image).await
            }
            RuntimeMode::Delegate(runtime) => super::pull_image_delegate(runtime, image).await,
        }
    }

    /// Build an image
    pub async fn build_image(
        &self,
        context: &str,
        tag: Option<&str>,
        dockerfile: &str,
    ) -> Result<()> {
        match &self.mode {
            RuntimeMode::Native => {
                let mut native = self.native.write().await;
                native.build_image_native(context, tag, dockerfile).await
            }
            RuntimeMode::Delegate(runtime) => {
                super::build_image_delegate(runtime, context, tag, dockerfile).await
            }
        }
    }

    /// Get runtime mode info
    pub fn get_mode(&self) -> &RuntimeMode {
        &self.mode
    }

    /// Check if running in native mode
    pub fn is_native(&self) -> bool {
        matches!(self.mode, RuntimeMode::Native)
    }

    /// Get access to the native runtime (for GPU integration)
    pub fn get_native_runtime(&self) -> Arc<RwLock<BoltNativeRuntime>> {
        Arc::clone(&self.native)
    }
}
