use serde::{Deserialize, Serialize};
use super::{OptimizationStep, ToOptimizationSteps};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuOptimizations {
    pub nvidia: Option<NvidiaOptimizations>,
    pub amd: Option<AmdOptimizations>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvidiaOptimizations {
    pub dlss: Option<bool>,
    pub reflex: Option<bool>,
    pub power_limit: Option<u32>,
    pub memory_clock_offset: Option<i32>,
    pub core_clock_offset: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmdOptimizations {
    pub rocm_optimization: Option<bool>,
    pub power_limit: Option<u32>,
}

impl ToOptimizationSteps for GpuOptimizations {
    fn to_steps(&self) -> Vec<OptimizationStep> {
        let mut steps = Vec::new();

        if let Some(nvidia) = &self.nvidia {
            steps.extend(nvidia.to_steps());
        }

        if let Some(amd) = &self.amd {
            steps.extend(amd.to_steps());
        }

        steps
    }
}

impl ToOptimizationSteps for NvidiaOptimizations {
    fn to_steps(&self) -> Vec<OptimizationStep> {
        let mut steps = Vec::new();

        if let Some(dlss) = self.dlss {
            steps.push(OptimizationStep {
                optimization_type: format!("nvidia_dlss_{}", dlss),
                priority: 30,
                description: format!("Set NVIDIA DLSS to {}", dlss),
            });
        }

        if let Some(reflex) = self.reflex {
            steps.push(OptimizationStep {
                optimization_type: format!("nvidia_reflex_{}", reflex),
                priority: 25,
                description: format!("Set NVIDIA Reflex to {}", reflex),
            });
        }

        if let Some(power_limit) = self.power_limit {
            steps.push(OptimizationStep {
                optimization_type: "nvidia_power_limit".to_string(),
                priority: 40,
                description: format!("Set NVIDIA power limit to {}%", power_limit),
            });
        }

        if let Some(memory_offset) = self.memory_clock_offset {
            steps.push(OptimizationStep {
                optimization_type: "nvidia_memory_clock".to_string(),
                priority: 35,
                description: format!("Set NVIDIA memory clock offset to {} MHz", memory_offset),
            });
        }

        if let Some(core_offset) = self.core_clock_offset {
            steps.push(OptimizationStep {
                optimization_type: "nvidia_core_clock".to_string(),
                priority: 35,
                description: format!("Set NVIDIA core clock offset to {} MHz", core_offset),
            });
        }

        steps
    }
}

impl ToOptimizationSteps for AmdOptimizations {
    fn to_steps(&self) -> Vec<OptimizationStep> {
        let mut steps = Vec::new();

        if let Some(rocm) = self.rocm_optimization {
            steps.push(OptimizationStep {
                optimization_type: format!("amd_rocm_{}", rocm),
                priority: 30,
                description: format!("Set AMD ROCm optimization to {}", rocm),
            });
        }

        if let Some(power_limit) = self.power_limit {
            steps.push(OptimizationStep {
                optimization_type: "amd_power_limit".to_string(),
                priority: 40,
                description: format!("Set AMD power limit to {}%", power_limit),
            });
        }

        steps
    }
}