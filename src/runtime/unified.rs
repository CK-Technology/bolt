use crate::Result;
use anyhow::anyhow;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::gpu_integration::GpuConfig;
use super::native::{BoltNativeRuntime, NativeContainerConfig, NativeContainerInfo};

#[derive(Debug, Clone, Default)]
pub struct ContainerRunOptions {
    pub rm: bool,
    pub command: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub user: Option<String>,
    pub hostname: Option<String>,
    pub cpus: Option<f32>,
    pub memory: Option<String>,
    pub network: Option<String>,
    pub cap_add: Vec<String>,
    pub cap_drop: Vec<String>,
    pub privileged: bool,
    pub tty: bool,
    pub interactive: bool,
    pub readonly_rootfs: bool,
    pub pids_limit: Option<i64>,
    pub seccomp: Option<String>,
}

/// Unified runtime interface that can switch between native and delegation modes
#[derive(Debug)]
pub struct UnifiedRuntime {
    native: Option<Arc<RwLock<BoltNativeRuntime>>>,
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

        Self::from_native_or_delegate(
            BoltNativeRuntime::new().await,
            super::detect_container_runtime(),
        )
        .await
    }

    async fn from_native_or_delegate<F>(
        native_result: Result<BoltNativeRuntime>,
        fallback_runtime: F,
    ) -> Result<Self>
    where
        F: Future<Output = Result<String>>,
    {
        match native_result {
            Ok(native) => {
                info!("✅ Native runtime initialized successfully");
                Ok(Self {
                    native: Some(Arc::new(RwLock::new(native))),
                    mode: RuntimeMode::Native,
                })
            }
            Err(e) => {
                warn!("⚠️  Native runtime failed to initialize: {}", e);
                warn!("   Falling back to delegation mode");

                Ok(Self {
                    native: None,
                    mode: RuntimeMode::Delegate(fallback_runtime.await?),
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
        self.run_container_with_options(
            image,
            name,
            ports,
            env,
            volumes,
            detach,
            None,
            ContainerRunOptions::default(),
        )
        .await
    }

    /// Run a container with GPU configuration
    #[allow(clippy::too_many_arguments)]
    pub async fn run_container_with_gpu(
        &self,
        image: &str,
        name: Option<&str>,
        ports: &[String],
        env: &[String],
        volumes: &[String],
        detach: bool,
        gpu_config: Option<GpuConfig>,
    ) -> Result<String> {
        self.run_container_with_options(
            image,
            name,
            ports,
            env,
            volumes,
            detach,
            gpu_config,
            ContainerRunOptions::default(),
        )
        .await
    }

    /// Run a container with full Docker-compatible run options.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_container_with_options(
        &self,
        image: &str,
        name: Option<&str>,
        ports: &[String],
        env: &[String],
        volumes: &[String],
        detach: bool,
        gpu_config: Option<GpuConfig>,
        options: ContainerRunOptions,
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
                    rm: options.rm,
                    command: options.command,
                    entrypoint: options.entrypoint,
                    working_dir: options.working_dir,
                    user: options.user,
                    hostname: options.hostname,
                    cpus: options.cpus,
                    memory: options.memory,
                    network: options.network,
                    cap_add: options.cap_add,
                    cap_drop: options.cap_drop,
                    privileged: options.privileged,
                    tty: options.tty,
                    interactive: options.interactive,
                    readonly_rootfs: options.readonly_rootfs,
                    pids_limit: options.pids_limit,
                    seccomp: options.seccomp,
                    gpu_config,
                    cpu_affinity: None,
                    workload_hint: None,
                };

                let native = self.native_runtime()?;
                let mut native = native.write().await;
                native.run_container(config).await
            }
            RuntimeMode::Delegate(runtime) => {
                // Fall back to old delegation method
                // Note: GPU passthrough not supported in delegation mode
                if gpu_config.is_some() {
                    warn!(
                        "⚠️  GPU passthrough not supported in delegation mode, using runtime's GPU flags"
                    );
                }
                super::run_oci_container_delegate_with_options(
                    runtime, image, name, ports, env, volumes, detach, &options,
                )
                .await
            }
        }
    }

    /// Stop a container
    pub async fn stop_container(&self, id: &str) -> Result<()> {
        match &self.mode {
            RuntimeMode::Native => {
                let native = self.native_runtime()?;
                let mut native = native.write().await;
                native.stop_container(id).await
            }
            RuntimeMode::Delegate(runtime) => super::stop_container_delegate(runtime, id).await,
        }
    }

    /// Remove a container
    pub async fn remove_container(&self, id: &str, force: bool) -> Result<()> {
        match &self.mode {
            RuntimeMode::Native => {
                let native = self.native_runtime()?;
                let mut native = native.write().await;
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
                let native = self.native_runtime()?;
                let native = native.read().await;
                native.list_containers(all).await
            }
            RuntimeMode::Delegate(runtime) => super::list_containers_delegate(runtime, all).await,
        }
    }

    /// Pull an image
    pub async fn pull_image(&self, image: &str) -> Result<()> {
        match &self.mode {
            RuntimeMode::Native => {
                let native = self.native_runtime()?;
                let mut native = native.write().await;
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
                let native = self.native_runtime()?;
                let mut native = native.write().await;
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
    pub fn get_native_runtime(&self) -> Result<Arc<RwLock<BoltNativeRuntime>>> {
        self.native_runtime()
    }

    fn native_runtime(&self) -> Result<Arc<RwLock<BoltNativeRuntime>>> {
        self.native
            .as_ref()
            .map(Arc::clone)
            .ok_or_else(|| anyhow!("native runtime unavailable in delegation mode").into())
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeMode, UnifiedRuntime};
    use anyhow::anyhow;

    #[tokio::test]
    async fn fallback_mode_does_not_construct_dummy_native_runtime() {
        let runtime = UnifiedRuntime::from_native_or_delegate(
            Err(anyhow!("native init failed").into()),
            async { Ok("podman".to_string()) },
        )
        .await
        .expect("delegation fallback should initialize");

        assert!(matches!(runtime.get_mode(), RuntimeMode::Delegate(name) if name == "podman"));
        assert!(!runtime.is_native());
        assert!(runtime.get_native_runtime().is_err());
    }
}
