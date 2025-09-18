use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod cpu;
pub mod gpu;
pub mod memory;
pub mod network;
pub mod storage;

use crate::plugins::{OptimizationContext, OptimizationPlugin, PluginManager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationProfile {
    pub name: String,
    pub description: String,
    pub priority: u32,
    pub cpu_optimizations: cpu::CpuOptimizations,
    pub gpu_optimizations: gpu::GpuOptimizations,
    pub memory_optimizations: memory::MemoryOptimizations,
    pub network_optimizations: network::NetworkOptimizations,
    pub storage_optimizations: storage::StorageOptimizations,
    pub conditions: Vec<OptimizationCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationCondition {
    GameTitle(String),
    GpuVendor(crate::plugins::GpuVendor),
    CpuCores(u32),
    MemoryGb(u32),
    NetworkLatencyMs(u32),
}

pub struct OptimizationManager {
    profiles: Arc<RwLock<HashMap<String, OptimizationProfile>>>,
    plugin_manager: Arc<PluginManager>,
    active_optimizations: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl OptimizationManager {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self {
            profiles: Arc::new(RwLock::new(HashMap::new())),
            plugin_manager,
            active_optimizations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn load_default_profiles(&self) -> Result<()> {
        let profiles = vec![
            self.create_steam_gaming_profile(),
            self.create_competitive_gaming_profile(),
            self.create_streaming_profile(),
            self.create_ai_ml_profile(),
            self.create_development_profile(),
        ];

        let mut profile_map = self.profiles.write().await;
        for profile in profiles {
            profile_map.insert(profile.name.clone(), profile);
        }

        Ok(())
    }

    pub async fn apply_profile(
        &self,
        profile_name: &str,
        context: &OptimizationContext,
    ) -> Result<()> {
        let profiles = self.profiles.read().await;
        let profile = profiles
            .get(profile_name)
            .ok_or_else(|| anyhow::anyhow!("Profile not found: {}", profile_name))?;

        if !self.profile_matches_conditions(profile, context) {
            return Err(anyhow::anyhow!("Profile conditions not met"));
        }

        let optimizations = self.build_optimization_pipeline(profile, context).await?;

        for optimization in &optimizations {
            self.plugin_manager
                .apply_optimizations(&optimization.optimization_type, context)
                .await?;
        }

        self.active_optimizations.write().await.insert(
            context.container_id.clone(),
            optimizations
                .iter()
                .map(|o| o.optimization_type.clone())
                .collect(),
        );

        Ok(())
    }

    pub async fn remove_optimizations(&self, container_id: &str) -> Result<()> {
        let mut active = self.active_optimizations.write().await;
        if let Some(optimizations) = active.remove(container_id) {
            for optimization_type in optimizations {
                // Remove optimizations - this would call plugin-specific removal
                tracing::info!(
                    "Removing optimization: {} for container: {}",
                    optimization_type,
                    container_id
                );
            }
        }
        Ok(())
    }

    pub async fn hot_reload_profile(
        &self,
        profile_name: &str,
        profile: OptimizationProfile,
    ) -> Result<()> {
        let mut profiles = self.profiles.write().await;
        profiles.insert(profile_name.to_string(), profile);

        // Reapply to active containers using this profile
        let active = self.active_optimizations.read().await;
        for (container_id, active_opts) in active.iter() {
            if active_opts.contains(&profile_name.to_string()) {
                // Hot-reload logic here
                tracing::info!(
                    "Hot-reloading profile {} for container {}",
                    profile_name,
                    container_id
                );
            }
        }

        Ok(())
    }

    async fn build_optimization_pipeline(
        &self,
        profile: &OptimizationProfile,
        context: &OptimizationContext,
    ) -> Result<Vec<OptimizationStep>> {
        let mut steps = Vec::new();

        steps.extend(profile.cpu_optimizations.to_steps());
        steps.extend(profile.gpu_optimizations.to_steps());
        steps.extend(profile.memory_optimizations.to_steps());
        steps.extend(profile.network_optimizations.to_steps());
        steps.extend(profile.storage_optimizations.to_steps());

        steps.sort_by_key(|step| step.priority);
        Ok(steps)
    }

    fn profile_matches_conditions(
        &self,
        profile: &OptimizationProfile,
        context: &OptimizationContext,
    ) -> bool {
        for condition in &profile.conditions {
            match condition {
                OptimizationCondition::GameTitle(title) => {
                    if let Some(game) = &context.game_title {
                        if !game.to_lowercase().contains(&title.to_lowercase()) {
                            return false;
                        }
                    }
                }
                OptimizationCondition::GpuVendor(vendor) => {
                    if let Some(ctx_vendor) = &context.gpu_vendor {
                        if std::mem::discriminant(ctx_vendor) != std::mem::discriminant(vendor) {
                            return false;
                        }
                    }
                }
                OptimizationCondition::CpuCores(min_cores) => {
                    if context.system_resources.cpu_cores < *min_cores {
                        return false;
                    }
                }
                OptimizationCondition::MemoryGb(min_memory) => {
                    if context.system_resources.memory_gb < *min_memory {
                        return false;
                    }
                }
                OptimizationCondition::NetworkLatencyMs(_) => {
                    // Network latency check would require actual measurement
                }
            }
        }
        true
    }

    fn create_steam_gaming_profile(&self) -> OptimizationProfile {
        OptimizationProfile {
            name: "steam-gaming".to_string(),
            description: "Optimized for Steam gaming with DLSS and Reflex".to_string(),
            priority: 100,
            cpu_optimizations: cpu::CpuOptimizations {
                governor: Some(cpu::CpuGovernor::Performance),
                priority: Some(10),
                affinity: Some(cpu::CpuAffinity::Gaming),
                boost: Some(true),
            },
            gpu_optimizations: gpu::GpuOptimizations {
                nvidia: Some(gpu::NvidiaOptimizations {
                    dlss: Some(true),
                    reflex: Some(true),
                    power_limit: Some(100),
                    memory_clock_offset: Some(500),
                    core_clock_offset: Some(100),
                }),
                amd: Some(gpu::AmdOptimizations {
                    rocm_optimization: Some(true),
                    power_limit: Some(100),
                }),
            },
            memory_optimizations: memory::MemoryOptimizations {
                huge_pages: Some(true),
                swap_disabled: Some(true),
                page_lock: Some(true),
            },
            network_optimizations: network::NetworkOptimizations {
                priority: Some(network::NetworkPriority::Gaming),
                latency_optimization: Some(true),
                packet_batching: Some(false),
            },
            storage_optimizations: storage::StorageOptimizations {
                io_scheduler: Some(storage::IoScheduler::Mq_deadline),
                read_ahead: Some(256),
            },
            conditions: vec![OptimizationCondition::GpuVendor(
                crate::plugins::GpuVendor::Nvidia,
            )],
        }
    }

    fn create_competitive_gaming_profile(&self) -> OptimizationProfile {
        OptimizationProfile {
            name: "competitive-gaming".to_string(),
            description: "Ultra-low latency for competitive gaming".to_string(),
            priority: 120,
            cpu_optimizations: cpu::CpuOptimizations {
                governor: Some(cpu::CpuGovernor::Performance),
                priority: Some(19),
                affinity: Some(cpu::CpuAffinity::Isolated),
                boost: Some(true),
            },
            gpu_optimizations: gpu::GpuOptimizations {
                nvidia: Some(gpu::NvidiaOptimizations {
                    dlss: Some(false), // Disable for minimum latency
                    reflex: Some(true),
                    power_limit: Some(110),
                    memory_clock_offset: Some(1000),
                    core_clock_offset: Some(200),
                }),
                amd: None,
            },
            memory_optimizations: memory::MemoryOptimizations {
                huge_pages: Some(true),
                swap_disabled: Some(true),
                page_lock: Some(true),
            },
            network_optimizations: network::NetworkOptimizations {
                priority: Some(network::NetworkPriority::Critical),
                latency_optimization: Some(true),
                packet_batching: Some(false),
            },
            storage_optimizations: storage::StorageOptimizations {
                io_scheduler: Some(storage::IoScheduler::None),
                read_ahead: Some(0),
            },
            conditions: vec![],
        }
    }

    fn create_streaming_profile(&self) -> OptimizationProfile {
        OptimizationProfile {
            name: "streaming".to_string(),
            description: "Balanced for game streaming and recording".to_string(),
            priority: 80,
            cpu_optimizations: cpu::CpuOptimizations {
                governor: Some(cpu::CpuGovernor::Ondemand),
                priority: Some(5),
                affinity: Some(cpu::CpuAffinity::Balanced),
                boost: Some(true),
            },
            gpu_optimizations: gpu::GpuOptimizations {
                nvidia: Some(gpu::NvidiaOptimizations {
                    dlss: Some(true),
                    reflex: Some(false),
                    power_limit: Some(95),
                    memory_clock_offset: Some(300),
                    core_clock_offset: Some(50),
                }),
                amd: None,
            },
            memory_optimizations: memory::MemoryOptimizations {
                huge_pages: Some(false),
                swap_disabled: Some(false),
                page_lock: Some(false),
            },
            network_optimizations: network::NetworkOptimizations {
                priority: Some(network::NetworkPriority::Streaming),
                latency_optimization: Some(false),
                packet_batching: Some(true),
            },
            storage_optimizations: storage::StorageOptimizations {
                io_scheduler: Some(storage::IoScheduler::Cfq),
                read_ahead: Some(512),
            },
            conditions: vec![],
        }
    }

    fn create_ai_ml_profile(&self) -> OptimizationProfile {
        OptimizationProfile {
            name: "ai-ml".to_string(),
            description: "Optimized for AI/ML workloads like Ollama".to_string(),
            priority: 90,
            cpu_optimizations: cpu::CpuOptimizations {
                governor: Some(cpu::CpuGovernor::Performance),
                priority: Some(0),
                affinity: Some(cpu::CpuAffinity::NumaAware),
                boost: Some(true),
            },
            gpu_optimizations: gpu::GpuOptimizations {
                nvidia: Some(gpu::NvidiaOptimizations {
                    dlss: Some(false),
                    reflex: Some(false),
                    power_limit: Some(110),
                    memory_clock_offset: Some(0),
                    core_clock_offset: Some(0),
                }),
                amd: Some(gpu::AmdOptimizations {
                    rocm_optimization: Some(true),
                    power_limit: Some(100),
                }),
            },
            memory_optimizations: memory::MemoryOptimizations {
                huge_pages: Some(true),
                swap_disabled: Some(true),
                page_lock: Some(true),
            },
            network_optimizations: network::NetworkOptimizations {
                priority: Some(network::NetworkPriority::Background),
                latency_optimization: Some(false),
                packet_batching: Some(true),
            },
            storage_optimizations: storage::StorageOptimizations {
                io_scheduler: Some(storage::IoScheduler::Noop),
                read_ahead: Some(1024),
            },
            conditions: vec![OptimizationCondition::MemoryGb(8)],
        }
    }

    fn create_development_profile(&self) -> OptimizationProfile {
        OptimizationProfile {
            name: "development".to_string(),
            description: "Balanced for development and compilation".to_string(),
            priority: 60,
            cpu_optimizations: cpu::CpuOptimizations {
                governor: Some(cpu::CpuGovernor::Ondemand),
                priority: Some(0),
                affinity: Some(cpu::CpuAffinity::Balanced),
                boost: Some(false),
            },
            gpu_optimizations: gpu::GpuOptimizations {
                nvidia: None,
                amd: None,
            },
            memory_optimizations: memory::MemoryOptimizations {
                huge_pages: Some(false),
                swap_disabled: Some(false),
                page_lock: Some(false),
            },
            network_optimizations: network::NetworkOptimizations {
                priority: Some(network::NetworkPriority::Background),
                latency_optimization: Some(false),
                packet_batching: Some(true),
            },
            storage_optimizations: storage::StorageOptimizations {
                io_scheduler: Some(storage::IoScheduler::Cfq),
                read_ahead: Some(128),
            },
            conditions: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptimizationStep {
    pub optimization_type: String,
    pub priority: u32,
    pub description: String,
}

pub trait ToOptimizationSteps {
    fn to_steps(&self) -> Vec<OptimizationStep>;
}
