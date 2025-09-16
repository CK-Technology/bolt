use serde::{Deserialize, Serialize};
use super::{OptimizationStep, ToOptimizationSteps};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageOptimizations {
    pub io_scheduler: Option<IoScheduler>,
    pub read_ahead: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IoScheduler {
    Noop,        // No scheduling (good for NVMe SSDs)
    Cfq,         // Completely Fair Queuing (good for HDDs)
    Deadline,    // Deadline scheduler
    Mq_deadline, // Multi-queue deadline
    None,        // No scheduler for ultra-low latency
}

impl ToOptimizationSteps for StorageOptimizations {
    fn to_steps(&self) -> Vec<OptimizationStep> {
        let mut steps = Vec::new();

        if let Some(scheduler) = &self.io_scheduler {
            steps.push(OptimizationStep {
                optimization_type: format!("storage_io_scheduler_{:?}", scheduler).to_lowercase(),
                priority: 80,
                description: format!("Set I/O scheduler to {:?}", scheduler),
            });
        }

        if let Some(read_ahead) = self.read_ahead {
            steps.push(OptimizationStep {
                optimization_type: "storage_read_ahead".to_string(),
                priority: 85,
                description: format!("Set read-ahead to {} KB", read_ahead),
            });
        }

        steps
    }
}

impl IoScheduler {
    pub fn to_kernel_name(&self) -> &'static str {
        match self {
            IoScheduler::Noop => "noop",
            IoScheduler::Cfq => "cfq",
            IoScheduler::Deadline => "deadline",
            IoScheduler::Mq_deadline => "mq-deadline",
            IoScheduler::None => "none",
        }
    }
}