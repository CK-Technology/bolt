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
pub mod docker_compat;
pub mod error;
pub mod gaming;
pub mod monitoring;
pub mod network;
pub mod networking;
pub mod nova_api;
pub mod nova_bridge;
pub mod optimizations;
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
#[derive(Clone)]
pub struct BoltRuntime {
    config: BoltConfig,
}

impl BoltRuntime {
    /// Create a new Bolt runtime instance
    pub fn new() -> Result<Self> {
        Ok(Self {
            config: BoltConfig::load()?,
        })
    }

    /// Create a new Bolt runtime instance with custom config
    pub fn with_config(config: BoltConfig) -> Self {
        Self { config }
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
        runtime::run_container(image, name, ports, env, volumes, detach).await
    }

    /// Build an image
    pub async fn build_image(&self, path: &str, tag: Option<&str>, dockerfile: &str) -> Result<()> {
        runtime::build_image(path, tag, dockerfile).await
    }

    /// Pull an image
    pub async fn pull_image(&self, image: &str) -> Result<()> {
        runtime::pull_image(image).await
    }

    /// Push an image
    pub async fn push_image(&self, image: &str) -> Result<()> {
        runtime::push_image(image).await
    }

    /// List containers
    pub async fn list_containers(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        runtime::list_containers_info(all).await
    }

    /// Stop a container
    pub async fn stop_container(&self, container: &str) -> Result<()> {
        runtime::stop_container(container).await
    }

    /// Remove a container
    pub async fn remove_container(&self, container: &str, force: bool) -> Result<()> {
        runtime::remove_container(container, force).await
    }

    /// Restart a container
    pub async fn restart_container(&self, container: &str, timeout: u64) -> Result<()> {
        runtime::restart_container(container, timeout).await
    }

    /// Start Surge orchestration
    pub async fn surge_up(
        &self,
        services: &[String],
        detach: bool,
        force_recreate: bool,
    ) -> Result<()> {
        surge::up(&self.config, services, detach, force_recreate).await
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

    /// Get the runtime configuration
    pub fn config(&self) -> &BoltConfig {
        &self.config
    }
}

impl Default for BoltRuntime {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            config: BoltConfig::default(),
        })
    }
}

// Types moved to types.rs module
