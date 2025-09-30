//! Bolt - Performance-first container runtime with revolutionary networking
//!
//! This crate provides programmatic access to Bolt's high-performance container runtime,
//! advanced networking capabilities, and intelligent optimization features.

#![recursion_limit = "512"]

pub mod ai;
pub mod builds;
pub mod capsules;
pub mod compat;
pub mod config;
pub mod dev_workflows;
pub mod docker_compat;
pub mod error;
pub mod gaming;
pub mod monitoring;
pub mod network;
pub mod networking;
pub mod nova_api;
pub mod nova_bridge;
pub mod optimizations;
pub mod orchestration;
pub mod plugins;
pub mod profiles;
pub mod registry;
pub mod runtime;
pub mod surge;
pub mod types;
pub mod volume;

pub use config::*;
pub use error::{BoltError, Result};

// Export main types at root level
pub use types::{ContainerInfo, NetworkInfo, ServiceInfo, SurgeStatus};

// Re-export anyhow for compatibility
pub use anyhow;

/// Re-exports for easier API usage
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use tokio::sync::OnceCell;

use crate::runtime::native::ContainerStatus as NativeContainerStatus;
use crate::runtime::unified::RuntimeMode;

pub mod api {
    pub use crate::config::{BoltConfig, BoltFile, GamingConfig, Service, create_example_boltfile};
    pub use crate::docker_compat::{DockerCompatLayer, DockerEnvironmentAnalysis};
    pub use crate::gaming::advanced_optimizations::{
        AdvancedGamingConfig, AdvancedGamingOptimizer, PerformanceProfile,
    };
    pub use crate::networking::{AdvancedFirewallManager, BoltAdvancedNetworking, QUICSocketProxy};
    pub use crate::nova_api::{
        BoltNovaRuntime, CapsuleHandle, CapsuleMetrics, NovaContainerConfig, NovaStatus,
    };
    pub use crate::nova_bridge::{
        NovaBridgeConfig, NovaBridgeManager, NovaServiceDiscovery, ServiceEntry,
    };
    pub use crate::registry::drift_integration::{BoltPackage, DriftRegistryClient};
    pub use crate::{BoltRuntime, ContainerInfo, NetworkInfo, ServiceInfo, SurgeStatus};
}

/// Builder for creating Boltfiles programmatically
pub struct BoltFileBuilder {
    project: String,
    services: std::collections::HashMap<String, config::Service>,
}

impl BoltFileBuilder {
    pub fn new(project: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            services: std::collections::HashMap::new(),
        }
    }

    pub fn add_service(mut self, name: impl Into<String>, service: config::Service) -> Self {
        self.services.insert(name.into(), service);
        self
    }

    pub fn add_gaming_service(
        self,
        name: impl Into<String>,
        image: impl Into<String>,
        gaming_config: config::GamingConfig,
    ) -> Self {
        let service = config::Service {
            image: Some(image.into()),
            gaming: Some(gaming_config),
            ..Default::default()
        };
        self.add_service(name, service)
    }

    pub fn build(self) -> config::BoltFile {
        config::BoltFile {
            project: self.project,
            services: self.services,
            networks: None,
            volumes: None,
            snapshots: None,
        }
    }
}

/// Core Bolt API for container management
pub struct BoltRuntime {
    config: BoltConfig,
    unified_runtime: Arc<OnceCell<Arc<runtime::unified::UnifiedRuntime>>>,
}

impl Clone for BoltRuntime {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            unified_runtime: self.unified_runtime.clone(),
        }
    }
}

impl BoltRuntime {
    /// Create a new Bolt runtime instance
    pub fn new() -> Result<Self> {
        Ok(Self {
            config: BoltConfig::load()?,
            unified_runtime: Arc::new(OnceCell::new()),
        })
    }

    /// Create a new Bolt runtime instance with custom config
    pub fn with_config(config: BoltConfig) -> Self {
        Self {
            config,
            unified_runtime: Arc::new(OnceCell::new()),
        }
    }

    async fn unified_runtime(&self) -> Result<Arc<runtime::unified::UnifiedRuntime>> {
        let runtime_ref = self
            .unified_runtime
            .get_or_try_init(|| async {
                let runtime = runtime::unified::UnifiedRuntime::new().await?;
                Ok::<Arc<runtime::unified::UnifiedRuntime>, BoltError>(Arc::new(runtime))
            })
            .await?;

        Ok(runtime_ref.clone())
    }

    /// Run a container
    pub async fn run_container(
        &self,
        image: &str,
        name: Option<&str>,
        ports: &[String],
        env: &[String],
        volumes: &[String],
        detach: bool,
    ) -> Result<()> {
        let runtime = self.unified_runtime().await?;
        runtime
            .run_container(image, name, ports, env, volumes, detach)
            .await
            .map(|_| ())
    }

    /// Build an image
    pub async fn build_image(&self, path: &str, tag: Option<&str>, dockerfile: &str) -> Result<()> {
        let runtime = self.unified_runtime().await?;
        runtime.build_image(path, tag, dockerfile).await
    }

    /// Pull an image
    pub async fn pull_image(&self, image: &str) -> Result<()> {
        let runtime = self.unified_runtime().await?;
        runtime.pull_image(image).await
    }

    /// Push an image
    pub async fn push_image(&self, image: &str) -> Result<()> {
        runtime::push_image(image).await
    }

    /// List containers
    pub async fn list_containers(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let runtime = self.unified_runtime().await?;
        let mode = runtime.get_mode().clone();
        let containers = runtime.list_containers(all).await?;

        let mut results = Vec::with_capacity(containers.len());

        for container in containers {
            let name = container
                .name
                .clone()
                .unwrap_or_else(|| container.id.clone());

            let created: DateTime<Utc> = DateTime::<Utc>::from(container.created);
            let uptime = SystemTime::now()
                .duration_since(container.created)
                .ok()
                .map(|duration| format!("{}s", duration.as_secs()));

            let status = match container.status {
                NativeContainerStatus::Created => "created".to_string(),
                NativeContainerStatus::Running => "running".to_string(),
                NativeContainerStatus::Stopped => "stopped".to_string(),
                NativeContainerStatus::Paused => "paused".to_string(),
                NativeContainerStatus::Exited(code) => format!("exited ({})", code),
                NativeContainerStatus::Error(message) => format!("error: {}", message),
            };

            let runtime_label = match &mode {
                RuntimeMode::Native => Some("bolt-native".to_string()),
                RuntimeMode::Delegate(delegate) => Some(delegate.clone()),
            };

            results.push(ContainerInfo {
                id: container.id.clone(),
                name: name.clone(),
                names: vec![name],
                image: container.image.clone(),
                image_id: String::new(),
                command: String::new(),
                created: created.to_rfc3339(),
                status,
                ports: container.ports.clone(),
                labels: HashMap::new(),
                uptime,
                runtime: runtime_label,
            });
        }

        Ok(results)
    }

    /// Stop a container
    pub async fn stop_container(&self, container: &str) -> Result<()> {
        let runtime = self.unified_runtime().await?;
        runtime.stop_container(container).await
    }

    /// Remove a container
    pub async fn remove_container(&self, container: &str, force: bool) -> Result<()> {
        let runtime = self.unified_runtime().await?;
        runtime.remove_container(container, force).await
    }

    /// Restart a container
    pub async fn restart_container(&self, container: &str, timeout: u64) -> Result<()> {
        runtime::restart_container(container, timeout).await
    }

    /// Start Surge orchestration with native runtime
    pub async fn surge_up(
        &self,
        services: &[String],
        detach: bool,
        force_recreate: bool,
    ) -> Result<()> {
        let runtime = self.unified_runtime().await?;
        // Use the new native runtime integration
        surge::up_with_native_runtime(&self.config, runtime, services, detach, force_recreate).await
    }

    /// Stop Surge services
    pub async fn surge_down(&self, services: &[String], volumes: bool) -> Result<()> {
        surge::down(&self.config, services, volumes).await
    }

    /// Get Surge status
    pub async fn surge_status(&self) -> Result<SurgeStatus> {
        surge::status_api::status_info(&self.config).await
    }

    /// Scale Surge services
    pub async fn surge_scale(&self, services: &[String]) -> Result<()> {
        surge::scale(&self.config, services).await
    }

    /// Setup gaming environment
    pub async fn setup_gaming(&self, proton: Option<&str>, winver: Option<&str>) -> Result<()> {
        gaming::setup_wine(proton, winver).await
    }

    /// Launch a game
    pub async fn launch_game(&self, game: &str, args: &[String]) -> Result<()> {
        gaming::launch_game(game, args).await
    }

    /// Create a network
    pub async fn create_network(
        &self,
        name: &str,
        driver: &str,
        subnet: Option<&str>,
    ) -> Result<()> {
        network::create_network(name, driver, subnet).await
    }

    /// List networks
    pub async fn list_networks(&self) -> Result<Vec<NetworkInfo>> {
        network::list_networks_info().await
    }

    /// Remove a network
    pub async fn remove_network(&self, name: &str) -> Result<()> {
        network::remove_network(name).await
    }

    /// Create a volume
    pub async fn create_volume(
        &self,
        name: &str,
        driver: &str,
        size: Option<&str>,
        options: &[String],
    ) -> Result<volume::VolumeInfo> {
        let mut volume_manager =
            volume::VolumeManager::new_async(self.config.data_dir.clone()).await?;

        let labels = std::collections::HashMap::new();
        let mut volume_options = std::collections::HashMap::new();

        for opt in options {
            if let Some((key, value)) = opt.split_once('=') {
                volume_options.insert(key.to_string(), value.to_string());
            }
        }

        let request = volume::VolumeCreateRequest {
            name: name.to_string(),
            driver: driver.to_string(),
            labels: Some(labels),
            options: Some(volume_options),
            size: size.map(|s| s.to_string()),
        };

        volume_manager.create_volume_async(request).await
    }

    /// List volumes
    pub async fn list_volumes(&self) -> Result<Vec<volume::VolumeInfo>> {
        let volume_manager = volume::VolumeManager::new_async(self.config.data_dir.clone()).await?;
        volume_manager.list_volumes_async().await
    }

    /// Remove a volume
    pub async fn remove_volume(&self, name: &str, force: bool) -> Result<()> {
        let mut volume_manager =
            volume::VolumeManager::new_async(self.config.data_dir.clone()).await?;
        volume_manager.remove_volume_async(name, force).await
    }

    /// Get the runtime configuration
    pub fn config(&self) -> &BoltConfig {
        &self.config
    }
}

impl Default for BoltRuntime {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            config: BoltConfig::default(),
            unified_runtime: Arc::new(OnceCell::new()),
        })
    }
}

// Types moved to types.rs module
