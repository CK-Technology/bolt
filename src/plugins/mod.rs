use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod loader;
pub mod registry;
pub mod traits;

pub use traits::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub plugin_type: PluginType,
    pub entry_point: String,
    pub dependencies: Vec<String>,
    pub permissions: Vec<Permission>,
    pub supported_gpus: Vec<GpuVendor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginType {
    GpuOptimization,
    AudioOptimization,
    NetworkOptimization,
    PerformanceMonitoring,
    GameSpecific,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Permission {
    GpuAccess,
    SystemControl,
    NetworkControl,
    FileSystem,
    ProcessControl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Any,
}

pub struct PluginManager {
    plugins: Arc<RwLock<HashMap<String, Box<dyn Plugin>>>>,
    gpu_plugins: Arc<RwLock<HashMap<String, Box<dyn GpuPlugin>>>>,
    optimization_plugins: Arc<RwLock<HashMap<String, Box<dyn OptimizationPlugin>>>>,
    plugin_registry: registry::PluginRegistry,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            gpu_plugins: Arc::new(RwLock::new(HashMap::new())),
            optimization_plugins: Arc::new(RwLock::new(HashMap::new())),
            plugin_registry: registry::PluginRegistry::new(),
        }
    }

    pub async fn load_plugin(&self, path: &PathBuf) -> Result<()> {
        let manifest = self.load_manifest(path).await?;
        self.validate_permissions(&manifest)?;

        let plugin = loader::load_plugin(path, &manifest).await?;

        match manifest.plugin_type {
            PluginType::GpuOptimization => {
                if let Some(_gpu_plugin) = plugin.as_any().downcast_ref::<Box<dyn GpuPlugin>>() {
                    // Cannot clone trait objects, needs redesign
                    tracing::warn!("GPU plugin loading needs redesign for trait object handling");
                }
            }
            PluginType::NetworkOptimization | PluginType::PerformanceMonitoring => {
                if let Some(_opt_plugin) = plugin.as_any().downcast_ref::<Box<dyn OptimizationPlugin>>() {
                    // Cannot clone trait objects, needs redesign
                    tracing::warn!("Optimization plugin loading needs redesign for trait object handling");
                }
            }
            _ => {
                self.plugins.write().await.insert(manifest.name.clone(), plugin);
            }
        }

        Ok(())
    }

    pub async fn get_gpu_plugins_for_vendor(&self, _vendor: GpuVendor) -> Vec<String> {
        let _plugins = self.gpu_plugins.read().await;
        // Placeholder implementation until plugin system redesign
        Vec::new()
    }

    pub async fn apply_optimizations(&self, optimization_type: &str, context: &OptimizationContext) -> Result<()> {
        let plugins = self.optimization_plugins.read().await;

        for plugin in plugins.values() {
            if plugin.supports_optimization(optimization_type) {
                plugin.apply_optimization(context).await?;
            }
        }

        Ok(())
    }

    pub async fn enable_plugin(&self, name: &str) -> Result<()> {
        if let Some(plugin) = self.plugins.read().await.get(name) {
            plugin.enable().await?;
        }
        Ok(())
    }

    pub async fn disable_plugin(&self, name: &str) -> Result<()> {
        if let Some(plugin) = self.plugins.read().await.get(name) {
            plugin.disable().await?;
        }
        Ok(())
    }

    pub async fn list_plugins(&self) -> Vec<PluginInfo> {
        let mut plugins = Vec::new();

        for (name, plugin) in self.plugins.read().await.iter() {
            plugins.push(PluginInfo {
                name: name.clone(),
                enabled: plugin.is_enabled().await,
                plugin_type: plugin.plugin_type(),
            });
        }

        plugins
    }

    async fn load_manifest(&self, path: &PathBuf) -> Result<PluginManifest> {
        let manifest_path = path.join("plugin.toml");
        let content = tokio::fs::read_to_string(manifest_path).await?;
        Ok(toml::from_str(&content)?)
    }

    fn validate_permissions(&self, manifest: &PluginManifest) -> Result<()> {
        for permission in &manifest.permissions {
            match permission {
                Permission::SystemControl => {
                    if !self.has_root_privileges() {
                        return Err(anyhow::anyhow!("Plugin requires system control but running without privileges"));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn has_root_privileges(&self) -> bool {
        #[cfg(feature = "oci-runtime")]
        {
            unsafe { nix::libc::geteuid() == 0 }
        }
        #[cfg(not(feature = "oci-runtime"))]
        {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub enabled: bool,
    pub plugin_type: PluginType,
}

#[derive(Debug)]
pub struct OptimizationContext {
    pub container_id: String,
    pub gpu_vendor: Option<GpuVendor>,
    pub performance_profile: String,
    pub game_title: Option<String>,
    pub system_resources: SystemResources,
}

#[derive(Debug)]
pub struct SystemResources {
    pub cpu_cores: u32,
    pub memory_gb: u32,
    pub gpu_memory_gb: Option<u32>,
}