use serde::{Deserialize, Serialize};
use super::{OptimizationStep, ToOptimizationSteps};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryOptimizations {
    pub huge_pages: Option<bool>,
    pub swap_disabled: Option<bool>,
    pub page_lock: Option<bool>,
}

impl ToOptimizationSteps for MemoryOptimizations {
    fn to_steps(&self) -> Vec<OptimizationStep> {
        let mut steps = Vec::new();

        if let Some(huge_pages) = self.huge_pages {
            steps.push(OptimizationStep {
                optimization_type: format!("memory_huge_pages_{}", huge_pages),
                priority: 50,
                description: format!("Set huge pages to {}", huge_pages),
            });
        }

        if let Some(swap_disabled) = self.swap_disabled {
            steps.push(OptimizationStep {
                optimization_type: format!("memory_swap_disabled_{}", swap_disabled),
                priority: 45,
                description: format!("Set swap disabled to {}", swap_disabled),
            });
        }

        if let Some(page_lock) = self.page_lock {
            steps.push(OptimizationStep {
                optimization_type: format!("memory_page_lock_{}", page_lock),
                priority: 55,
                description: format!("Set memory page lock to {}", page_lock),
            });
        }

        steps
    }
}