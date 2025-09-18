use super::{OptimizationStep, ToOptimizationSteps};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuOptimizations {
    pub governor: Option<CpuGovernor>,
    pub priority: Option<i32>,
    pub affinity: Option<CpuAffinity>,
    pub boost: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CpuGovernor {
    Performance,
    Ondemand,
    Conservative,
    Powersave,
    Userspace,
    Schedutil,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CpuAffinity {
    Gaming,    // Isolate to specific cores for gaming
    Balanced,  // Use all cores efficiently
    Isolated,  // Isolate from system processes
    NumaAware, // NUMA-aware allocation for AI/ML
}

impl ToOptimizationSteps for CpuOptimizations {
    fn to_steps(&self) -> Vec<OptimizationStep> {
        let mut steps = Vec::new();

        if let Some(governor) = &self.governor {
            steps.push(OptimizationStep {
                optimization_type: format!("cpu_governor_{:?}", governor).to_lowercase(),
                priority: 10,
                description: format!("Set CPU governor to {:?}", governor),
            });
        }

        if let Some(priority) = self.priority {
            steps.push(OptimizationStep {
                optimization_type: "cpu_priority".to_string(),
                priority: 15,
                description: format!("Set CPU priority to {}", priority),
            });
        }

        if let Some(affinity) = &self.affinity {
            steps.push(OptimizationStep {
                optimization_type: format!("cpu_affinity_{:?}", affinity).to_lowercase(),
                priority: 20,
                description: format!("Set CPU affinity to {:?}", affinity),
            });
        }

        if let Some(boost) = self.boost {
            steps.push(OptimizationStep {
                optimization_type: format!("cpu_boost_{}", boost),
                priority: 5,
                description: format!("Set CPU boost to {}", boost),
            });
        }

        steps
    }
}

impl CpuGovernor {
    pub fn to_kernel_name(&self) -> &'static str {
        match self {
            CpuGovernor::Performance => "performance",
            CpuGovernor::Ondemand => "ondemand",
            CpuGovernor::Conservative => "conservative",
            CpuGovernor::Powersave => "powersave",
            CpuGovernor::Userspace => "userspace",
            CpuGovernor::Schedutil => "schedutil",
        }
    }
}

impl CpuAffinity {
    pub fn get_cpu_mask(&self, total_cpus: u32) -> Vec<u32> {
        match self {
            CpuAffinity::Gaming => {
                // For gaming, use the last 4 cores (assuming they're performance cores)
                let start = if total_cpus >= 4 { total_cpus - 4 } else { 0 };
                (start..total_cpus).collect()
            }
            CpuAffinity::Balanced => {
                // Use all cores
                (0..total_cpus).collect()
            }
            CpuAffinity::Isolated => {
                // Use only the last 2 cores, isolated from system processes
                let start = if total_cpus >= 2 { total_cpus - 2 } else { 0 };
                (start..total_cpus).collect()
            }
            CpuAffinity::NumaAware => {
                // For AI/ML, prefer cores from the same NUMA node as the GPU
                // This is a simplified implementation
                (0..total_cpus).collect()
            }
        }
    }
}
