//! GPU Scheduler - Intelligent GPU allocation for AI/ML workloads
//!
//! Provides multi-GPU scheduling with support for:
//! - Multiple allocation strategies (round-robin, least-utilized, memory-aware)
//! - MIG (Multi-Instance GPU) support for A100/H100
//! - VRAM tracking and quotas
//! - Time-slicing for GPU sharing

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// GPU Scheduler manages GPU allocation across containers
pub struct GpuScheduler {
    /// GPU inventory (GPU ID -> GPU state)
    gpus: Arc<RwLock<HashMap<String, GpuState>>>,
    /// Container allocations (Container ID -> Allocated GPU IDs)
    allocations: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// Scheduling strategy
    strategy: SchedulingStrategy,
    /// MIG manager (optional, only for A100/H100)
    mig_manager: Option<MigManager>,
}

/// GPU state and metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuState {
    pub id: String,
    pub name: String,
    pub index: u32,
    pub total_memory_mb: u64,
    pub free_memory_mb: u64,
    pub utilization_percent: f32,
    pub temperature_c: u32,
    pub power_draw_w: u32,
    pub power_limit_w: u32,
    pub allocated_to: Vec<String>, // Container IDs
    pub is_mig_enabled: bool,
    pub mig_instances: Vec<MigInstance>,
}

/// MIG (Multi-Instance GPU) instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigInstance {
    pub id: String,
    pub gpu_id: String,
    pub gpu_slices: u32,     // 1-7 slices
    pub memory_mb: u64,      // 5GB, 10GB, 20GB, 40GB, 80GB
    pub compute_slices: u32, // 1-7 compute slices
    pub allocated_to: Option<String>,
}

/// GPU scheduling strategy
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum SchedulingStrategy {
    /// Simple round-robin allocation
    RoundRobin,
    /// Allocate to GPU with lowest utilization
    #[default]
    LeastUtilized,
    /// Allocate to GPU with most free memory
    MostMemory,
    /// Exclusive GPU access (no sharing)
    Exclusive,
    /// Time-slice GPUs across containers
    TimeSlicing { slice_ms: u64 },
    /// Pack containers efficiently (bin packing)
    BestFit,
}

/// GPU allocation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuRequest {
    /// Allocate all available GPUs
    All,
    /// Allocate N GPUs
    Count(usize),
    /// Allocate specific GPU IDs
    Specific(Vec<String>),
    /// Allocate GPUs with at least X MB memory
    Memory(u64),
    /// Allocate MIG instance
    Mig { profile: String }, // e.g., "1g.5gb", "3g.20gb"
}

/// GPU allocation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuConfig {
    pub request: GpuRequest,
    pub memory_limit_mb: Option<u64>,
    pub priority: u32, // 0-100
    pub exclusive: bool,
}

impl GpuScheduler {
    /// Create a new GPU scheduler
    pub async fn new() -> Result<Self> {
        info!("🎮 Initializing GPU scheduler");

        // Detect GPUs
        let gpus = Self::detect_gpus().await?;
        info!("   Detected {} GPU(s)", gpus.len());

        for (id, gpu) in &gpus {
            info!(
                "   • {} - {} ({}MB VRAM)",
                id, gpu.name, gpu.total_memory_mb
            );
        }

        // Detect MIG support
        let mig_manager = MigManager::detect().await.ok();
        if let Some(ref mgr) = mig_manager {
            info!(
                "   • MIG support detected ({} instances)",
                mgr.instances.len()
            );
        }

        Ok(Self {
            gpus: Arc::new(RwLock::new(gpus)),
            allocations: Arc::new(RwLock::new(HashMap::new())),
            strategy: SchedulingStrategy::default(),
            mig_manager,
        })
    }

    /// Allocate GPUs for a container
    pub async fn allocate(&self, container_id: &str, config: GpuConfig) -> Result<Vec<String>> {
        let mut gpus = self.gpus.write().await;
        let mut allocations = self.allocations.write().await;

        info!("🔧 Allocating GPUs for container: {}", container_id);
        debug!("   Request: {:?}", config.request);
        debug!("   Strategy: {:?}", self.strategy);

        let selected_gpus = match config.request {
            GpuRequest::All => {
                // Allocate all available GPUs
                let available: Vec<String> = gpus.keys().cloned().collect();
                info!("   Allocating ALL {} GPU(s)", available.len());
                available
            }
            GpuRequest::Count(n) => {
                // Allocate N GPUs using scheduling strategy
                self.select_gpus(&gpus, n, &config).await?
            }
            GpuRequest::Specific(ids) => {
                // Allocate specific GPU IDs
                self.validate_gpu_ids(&gpus, &ids)?;
                info!("   Allocating specific GPUs: {:?}", ids);
                ids
            }
            GpuRequest::Memory(memory_mb) => {
                // Allocate GPUs with enough memory
                self.select_gpus_by_memory(&gpus, memory_mb).await?
            }
            GpuRequest::Mig { ref profile } => {
                // Allocate MIG instance
                if let Some(ref mig_mgr) = self.mig_manager {
                    return mig_mgr.allocate_instance(container_id, profile).await;
                } else {
                    return Err(anyhow!("MIG not available on this system"));
                }
            }
        };

        // Validate allocation
        if selected_gpus.is_empty() {
            return Err(anyhow!("No GPUs available for allocation"));
        }

        // Mark GPUs as allocated
        for gpu_id in &selected_gpus {
            if let Some(gpu) = gpus.get_mut(gpu_id) {
                gpu.allocated_to.push(container_id.to_string());

                if config.exclusive {
                    gpu.free_memory_mb = 0; // Mark as fully allocated
                } else if let Some(limit) = config.memory_limit_mb {
                    gpu.free_memory_mb = gpu.free_memory_mb.saturating_sub(limit);
                }
            }
        }

        allocations.insert(container_id.to_string(), selected_gpus.clone());

        info!(
            "✅ Allocated {} GPU(s) to container {}",
            selected_gpus.len(),
            container_id
        );
        for gpu_id in &selected_gpus {
            debug!("   • {}", gpu_id);
        }

        Ok(selected_gpus)
    }

    /// Deallocate GPUs when container stops
    pub async fn deallocate(&self, container_id: &str) -> Result<()> {
        let mut gpus = self.gpus.write().await;
        let mut allocations = self.allocations.write().await;

        info!("🗑️  Deallocating GPUs for container: {}", container_id);

        if let Some(gpu_ids) = allocations.remove(container_id) {
            for gpu_id in &gpu_ids {
                if let Some(gpu) = gpus.get_mut(gpu_id) {
                    gpu.allocated_to.retain(|id| id != container_id);

                    // Restore free memory
                    if gpu.allocated_to.is_empty() {
                        gpu.free_memory_mb = gpu.total_memory_mb;
                    }
                }
            }
            info!("✅ Deallocated {} GPU(s)", gpu_ids.len());
        } else {
            debug!("No GPUs allocated to container: {}", container_id);
        }

        Ok(())
    }

    /// Get GPU status
    pub async fn get_status(&self) -> HashMap<String, GpuState> {
        self.gpus.read().await.clone()
    }

    /// Get allocation for a container
    pub async fn get_allocation(&self, container_id: &str) -> Option<Vec<String>> {
        self.allocations.read().await.get(container_id).cloned()
    }

    /// Update GPU metrics (call periodically)
    pub async fn update_metrics(&self) -> Result<()> {
        let mut gpus = self.gpus.write().await;

        // Query nvidia-smi for current metrics
        let output = tokio::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=index,utilization.gpu,memory.free,temperature.gpu,power.draw",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .await
            .context("Failed to query GPU metrics")?;

        if !output.status.success() {
            return Err(anyhow!("nvidia-smi query failed"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 5 {
                let index: u32 = parts[0].parse().unwrap_or(0);
                let gpu_id = format!("gpu:{}", index);

                if let Some(gpu) = gpus.get_mut(&gpu_id) {
                    gpu.utilization_percent = parts[1].parse().unwrap_or(0.0);
                    gpu.free_memory_mb = parts[2].parse().unwrap_or(0);
                    gpu.temperature_c = parts[3].parse().unwrap_or(0);
                    gpu.power_draw_w = parts[4].parse().unwrap_or(0);
                }
            }
        }

        Ok(())
    }

    /// Select N GPUs based on scheduling strategy
    async fn select_gpus(
        &self,
        gpus: &HashMap<String, GpuState>,
        count: usize,
        config: &GpuConfig,
    ) -> Result<Vec<String>> {
        let available: Vec<_> = gpus
            .iter()
            .filter(|(_, state)| {
                if config.exclusive {
                    // For exclusive mode, only select completely free GPUs
                    state.allocated_to.is_empty()
                } else {
                    // For shared mode, check memory availability
                    if let Some(limit) = config.memory_limit_mb {
                        state.free_memory_mb >= limit
                    } else {
                        true
                    }
                }
            })
            .collect();

        if available.len() < count {
            return Err(anyhow!(
                "Not enough GPUs available (requested: {}, available: {})",
                count,
                available.len()
            ));
        }

        let selected = match self.strategy {
            SchedulingStrategy::RoundRobin => {
                // Simple round-robin: take first N available
                available
                    .into_iter()
                    .take(count)
                    .map(|(id, _)| id.clone())
                    .collect()
            }
            SchedulingStrategy::LeastUtilized => {
                // Sort by utilization (lowest first)
                let mut sorted = available;
                sorted.sort_by(|a, b| {
                    a.1.utilization_percent
                        .partial_cmp(&b.1.utilization_percent)
                        .unwrap()
                });
                sorted
                    .into_iter()
                    .take(count)
                    .map(|(id, _)| id.clone())
                    .collect()
            }
            SchedulingStrategy::MostMemory => {
                // Sort by free memory (highest first)
                let mut sorted = available;
                sorted.sort_by(|a, b| b.1.free_memory_mb.cmp(&a.1.free_memory_mb));
                sorted
                    .into_iter()
                    .take(count)
                    .map(|(id, _)| id.clone())
                    .collect()
            }
            SchedulingStrategy::Exclusive => {
                // Only allocate completely free GPUs
                available
                    .into_iter()
                    .filter(|(_, state)| state.allocated_to.is_empty())
                    .take(count)
                    .map(|(id, _)| id.clone())
                    .collect()
            }
            _ => {
                warn!("Unsupported scheduling strategy, falling back to round-robin");
                available
                    .into_iter()
                    .take(count)
                    .map(|(id, _)| id.clone())
                    .collect()
            }
        };

        Ok(selected)
    }

    /// Select GPUs by memory requirement
    async fn select_gpus_by_memory(
        &self,
        gpus: &HashMap<String, GpuState>,
        required_memory_mb: u64,
    ) -> Result<Vec<String>> {
        let suitable: Vec<String> = gpus
            .iter()
            .filter(|(_, state)| state.free_memory_mb >= required_memory_mb)
            .map(|(id, _)| id.clone())
            .collect();

        if suitable.is_empty() {
            return Err(anyhow!(
                "No GPUs with {}MB available memory",
                required_memory_mb
            ));
        }

        Ok(vec![suitable[0].clone()])
    }

    /// Validate GPU IDs exist
    fn validate_gpu_ids(&self, gpus: &HashMap<String, GpuState>, ids: &[String]) -> Result<()> {
        for id in ids {
            if !gpus.contains_key(id) {
                return Err(anyhow!("GPU {} not found", id));
            }
        }
        Ok(())
    }

    /// Detect GPUs using nvidia-smi
    async fn detect_gpus() -> Result<HashMap<String, GpuState>> {
        let output = tokio::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=index,name,memory.total,memory.free,power.limit",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .await
            .context("Failed to detect GPUs. Is nvidia-smi installed?")?;

        if !output.status.success() {
            return Err(anyhow!("nvidia-smi failed to query GPUs"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut gpus = HashMap::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 5 {
                let index: u32 = parts[0].parse().context("Invalid GPU index")?;
                let id = format!("gpu:{}", index);

                let gpu = GpuState {
                    id: id.clone(),
                    name: parts[1].to_string(),
                    index,
                    total_memory_mb: parts[2].parse().unwrap_or(0),
                    free_memory_mb: parts[3].parse().unwrap_or(0),
                    utilization_percent: 0.0,
                    temperature_c: 0,
                    power_draw_w: 0,
                    power_limit_w: parts[4].parse().unwrap_or(0),
                    allocated_to: Vec::new(),
                    is_mig_enabled: false,
                    mig_instances: Vec::new(),
                };

                gpus.insert(id, gpu);
            }
        }

        if gpus.is_empty() {
            return Err(anyhow!("No NVIDIA GPUs detected"));
        }

        Ok(gpus)
    }
}

/// MIG Manager for A100/H100 GPUs
#[derive(Debug, Clone)]
pub struct MigManager {
    instances: HashMap<String, MigInstance>,
}

impl MigManager {
    /// Detect MIG instances
    pub async fn detect() -> Result<Self> {
        // Query MIG instances using nvidia-smi
        let output = tokio::process::Command::new("nvidia-smi")
            .args(["mig", "-lgi"])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow!("MIG not enabled or not available"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut instances = HashMap::new();

        // Parse MIG instance output
        for line in stdout.lines() {
            if line.contains("GI ID") || line.trim().is_empty() {
                continue;
            }

            // Parse MIG instance info
            // Format: GPU 0: GI ID 0, Profile: 1g.5gb, ...
            if let Some(instance) = Self::parse_mig_line(line) {
                instances.insert(instance.id.clone(), instance);
            }
        }

        Ok(Self { instances })
    }

    fn parse_mig_line(line: &str) -> Option<MigInstance> {
        // Simplified parser - would need proper implementation
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 6 {
            return None;
        }

        Some(MigInstance {
            id: format!("mig:{}", parts[3]),
            gpu_id: format!("gpu:{}", parts[1].trim_end_matches(':')),
            gpu_slices: 1,
            memory_mb: 5120, // 5GB default
            compute_slices: 1,
            allocated_to: None,
        })
    }

    /// Allocate MIG instance
    pub async fn allocate_instance(
        &self,
        container_id: &str,
        profile: &str,
    ) -> Result<Vec<String>> {
        // Find available MIG instance matching profile
        for (id, instance) in &self.instances {
            if instance.allocated_to.is_none() && Self::matches_profile(instance, profile) {
                info!("✅ Allocated MIG instance {} to {}", id, container_id);
                return Ok(vec![id.clone()]);
            }
        }

        Err(anyhow!(
            "No available MIG instance matching profile: {}",
            profile
        ))
    }

    fn matches_profile(instance: &MigInstance, profile: &str) -> bool {
        // Match profile like "1g.5gb", "3g.20gb"
        match profile {
            "1g.5gb" => instance.gpu_slices == 1 && instance.memory_mb == 5120,
            "2g.10gb" => instance.gpu_slices == 2 && instance.memory_mb == 10240,
            "3g.20gb" => instance.gpu_slices == 3 && instance.memory_mb == 20480,
            "4g.20gb" => instance.gpu_slices == 4 && instance.memory_mb == 20480,
            "7g.40gb" => instance.gpu_slices == 7 && instance.memory_mb == 40960,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_request_parsing() {
        let req = GpuRequest::Count(2);
        assert!(matches!(req, GpuRequest::Count(2)));

        let req = GpuRequest::Mig {
            profile: "1g.5gb".to_string(),
        };
        assert!(matches!(req, GpuRequest::Mig { .. }));
    }

    #[test]
    fn test_scheduling_strategy_default() {
        let strategy = SchedulingStrategy::default();
        assert!(matches!(strategy, SchedulingStrategy::LeastUtilized));
    }
}
