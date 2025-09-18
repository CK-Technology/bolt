use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod llm;
pub mod ollama;

/// AI/ML workload optimization module for Bolt
/// Provides specialized optimizations for local LLMs, training, and inference

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiWorkloadConfig {
    pub workload_type: AiWorkloadType,
    pub model_config: ModelConfig,
    pub hardware_config: AiHardwareConfig,
    pub performance_config: AiPerformanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiWorkloadType {
    Inference,
    Training,
    FineTuning,
    Embedding,
    TextGeneration,
    ImageGeneration,
    MultiModal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub model_name: String,
    pub model_size: ModelSize,
    pub quantization: Option<QuantizationType>,
    pub context_length: Option<u32>,
    pub batch_size: Option<u32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelSize {
    Small,  // < 3B parameters (Phi, small Llama)
    Medium, // 3B - 13B parameters (Llama 3 8B, Code Llama)
    Large,  // 13B - 70B parameters (Llama 3 70B, CodeLlama 34B)
    XLarge, // > 70B parameters (Llama 3 405B, GPT-4 class)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantizationType {
    FP16,
    INT8,
    INT4,
    GGML_Q4_0,
    GGML_Q4_1,
    GGML_Q5_0,
    GGML_Q5_1,
    GGML_Q8_0,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiHardwareConfig {
    pub gpu_allocation: GpuAllocation,
    pub memory_config: AiMemoryConfig,
    pub cpu_config: AiCpuConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuAllocation {
    Exclusive,                      // Entire GPU for this workload
    Shared { percentage: u32 },     // Share GPU with other workloads
    MultiGpu { gpu_ids: Vec<u32> }, // Use multiple GPUs
    CpuOnly,                        // CPU-only inference
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMemoryConfig {
    pub enable_huge_pages: bool,
    pub memory_limit_gb: Option<u32>,
    pub swap_disabled: bool,
    pub numa_awareness: bool,
    pub memory_pooling: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCpuConfig {
    pub thread_count: Option<u32>,
    pub cpu_affinity: Option<Vec<u32>>,
    pub simd_optimization: bool,
    pub vector_instructions: VectorInstructions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VectorInstructions {
    Auto,
    Avx2,
    Avx512,
    Neon, // ARM
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiPerformanceConfig {
    pub flash_attention: bool,
    pub tensor_parallelism: bool,
    pub pipeline_parallelism: bool,
    pub gradient_checkpointing: bool,
    pub mixed_precision: bool,
    pub compile_optimization: bool,
}

pub struct AiOptimizer {
    workload_configs: HashMap<String, AiWorkloadConfig>,
}

impl AiOptimizer {
    pub fn new() -> Self {
        Self {
            workload_configs: HashMap::new(),
        }
    }

    pub async fn optimize_for_ollama(
        &self,
        model_name: &str,
        gpu_memory_gb: u32,
    ) -> Result<AiWorkloadConfig> {
        let model_size = self.determine_model_size(model_name);
        let quantization = self.select_optimal_quantization(&model_size, gpu_memory_gb);
        let gpu_allocation = self.determine_gpu_allocation(&model_size, gpu_memory_gb);

        let config = AiWorkloadConfig {
            workload_type: AiWorkloadType::Inference,
            model_config: ModelConfig {
                model_name: model_name.to_string(),
                model_size,
                quantization: Some(quantization),
                context_length: Some(4096),
                batch_size: Some(1),
                max_tokens: Some(2048),
            },
            hardware_config: AiHardwareConfig {
                gpu_allocation,
                memory_config: AiMemoryConfig {
                    enable_huge_pages: true,
                    memory_limit_gb: None,
                    swap_disabled: true,
                    numa_awareness: true,
                    memory_pooling: true,
                },
                cpu_config: AiCpuConfig {
                    thread_count: Some(num_cpus::get() as u32 / 2),
                    cpu_affinity: None,
                    simd_optimization: true,
                    vector_instructions: VectorInstructions::Auto,
                },
            },
            performance_config: AiPerformanceConfig {
                flash_attention: true,
                tensor_parallelism: gpu_memory_gb > 16,
                pipeline_parallelism: false,
                gradient_checkpointing: false,
                mixed_precision: true,
                compile_optimization: true,
            },
        };

        Ok(config)
    }

    pub async fn optimize_for_training(
        &self,
        model_size: &ModelSize,
        available_memory_gb: u32,
    ) -> Result<AiWorkloadConfig> {
        let config = AiWorkloadConfig {
            workload_type: AiWorkloadType::Training,
            model_config: ModelConfig {
                model_name: "custom-training".to_string(),
                model_size: model_size.clone(),
                quantization: None, // Full precision for training
                context_length: Some(2048),
                batch_size: Some(
                    self.calculate_optimal_batch_size(model_size, available_memory_gb),
                ),
                max_tokens: None,
            },
            hardware_config: AiHardwareConfig {
                gpu_allocation: GpuAllocation::Exclusive,
                memory_config: AiMemoryConfig {
                    enable_huge_pages: true,
                    memory_limit_gb: Some(available_memory_gb * 90 / 100), // 90% of available
                    swap_disabled: true,
                    numa_awareness: true,
                    memory_pooling: true,
                },
                cpu_config: AiCpuConfig {
                    thread_count: Some(num_cpus::get() as u32),
                    cpu_affinity: None,
                    simd_optimization: true,
                    vector_instructions: VectorInstructions::Auto,
                },
            },
            performance_config: AiPerformanceConfig {
                flash_attention: true,
                tensor_parallelism: true,
                pipeline_parallelism: true,
                gradient_checkpointing: true,
                mixed_precision: true,
                compile_optimization: true,
            },
        };

        Ok(config)
    }

    pub fn get_recommended_environment_vars(
        &self,
        config: &AiWorkloadConfig,
    ) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();

        match config.workload_type {
            AiWorkloadType::Inference => {
                env_vars.insert(
                    "OMP_NUM_THREADS".to_string(),
                    config
                        .hardware_config
                        .cpu_config
                        .thread_count
                        .unwrap_or(1)
                        .to_string(),
                );

                if config.performance_config.flash_attention {
                    env_vars.insert("OLLAMA_FLASH_ATTENTION".to_string(), "1".to_string());
                }

                match &config.hardware_config.gpu_allocation {
                    GpuAllocation::Exclusive => {
                        env_vars.insert("CUDA_VISIBLE_DEVICES".to_string(), "0".to_string());
                        env_vars.insert("NVIDIA_VISIBLE_DEVICES".to_string(), "all".to_string());
                    }
                    GpuAllocation::MultiGpu { gpu_ids } => {
                        let gpu_list = gpu_ids
                            .iter()
                            .map(|id| id.to_string())
                            .collect::<Vec<_>>()
                            .join(",");
                        env_vars.insert("CUDA_VISIBLE_DEVICES".to_string(), gpu_list);
                    }
                    GpuAllocation::CpuOnly => {
                        env_vars.insert("CUDA_VISIBLE_DEVICES".to_string(), "-1".to_string());
                    }
                    _ => {}
                }
            }
            AiWorkloadType::Training => {
                env_vars.insert(
                    "PYTORCH_CUDA_ALLOC_CONF".to_string(),
                    "max_split_size_mb:128".to_string(),
                );
                env_vars.insert("NCCL_DEBUG".to_string(), "INFO".to_string());

                if config.performance_config.mixed_precision {
                    env_vars.insert("NVIDIA_TF32_OVERRIDE".to_string(), "1".to_string());
                }
            }
            _ => {}
        }

        // Memory optimizations
        if config.hardware_config.memory_config.enable_huge_pages {
            env_vars.insert("MALLOC_MMAP_THRESHOLD_".to_string(), "131072".to_string());
        }

        env_vars
    }

    fn determine_model_size(&self, model_name: &str) -> ModelSize {
        let model_lower = model_name.to_lowercase();

        if model_lower.contains("405b") || model_lower.contains("gpt-4") {
            ModelSize::XLarge
        } else if model_lower.contains("70b") || model_lower.contains("34b") {
            ModelSize::Large
        } else if model_lower.contains("13b")
            || model_lower.contains("8b")
            || model_lower.contains("7b")
        {
            ModelSize::Medium
        } else {
            ModelSize::Small
        }
    }

    fn select_optimal_quantization(
        &self,
        model_size: &ModelSize,
        gpu_memory_gb: u32,
    ) -> QuantizationType {
        match (model_size, gpu_memory_gb) {
            (ModelSize::XLarge, mem) if mem >= 80 => QuantizationType::FP16,
            (ModelSize::XLarge, _) => QuantizationType::GGML_Q4_0,
            (ModelSize::Large, mem) if mem >= 48 => QuantizationType::FP16,
            (ModelSize::Large, _) => QuantizationType::GGML_Q4_1,
            (ModelSize::Medium, mem) if mem >= 16 => QuantizationType::FP16,
            (ModelSize::Medium, _) => QuantizationType::GGML_Q5_0,
            (ModelSize::Small, _) => QuantizationType::FP16,
        }
    }

    fn determine_gpu_allocation(
        &self,
        model_size: &ModelSize,
        gpu_memory_gb: u32,
    ) -> GpuAllocation {
        match model_size {
            ModelSize::XLarge => {
                if gpu_memory_gb >= 80 {
                    GpuAllocation::Exclusive
                } else {
                    GpuAllocation::CpuOnly
                }
            }
            ModelSize::Large => {
                if gpu_memory_gb >= 24 {
                    GpuAllocation::Exclusive
                } else if gpu_memory_gb >= 12 {
                    GpuAllocation::Shared { percentage: 80 }
                } else {
                    GpuAllocation::CpuOnly
                }
            }
            ModelSize::Medium => {
                if gpu_memory_gb >= 8 {
                    GpuAllocation::Exclusive
                } else {
                    GpuAllocation::Shared { percentage: 60 }
                }
            }
            ModelSize::Small => GpuAllocation::Shared { percentage: 40 },
        }
    }

    fn calculate_optimal_batch_size(
        &self,
        model_size: &ModelSize,
        available_memory_gb: u32,
    ) -> u32 {
        match model_size {
            ModelSize::XLarge => (available_memory_gb / 40).max(1),
            ModelSize::Large => (available_memory_gb / 20).max(1),
            ModelSize::Medium => (available_memory_gb / 8).max(1),
            ModelSize::Small => (available_memory_gb / 2).max(1),
        }
    }
}

impl Default for AiOptimizer {
    fn default() -> Self {
        Self::new()
    }
}
