use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{AiWorkloadConfig, ModelSize, QuantizationType};

/// LLM-specific optimizations and utilities for various inference engines

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmEngine {
    pub name: String,
    pub engine_type: LlmEngineType,
    pub supported_formats: Vec<ModelFormat>,
    pub gpu_support: bool,
    pub quantization_support: Vec<QuantizationType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmEngineType {
    Ollama,
    LlamaCpp,
    Oobabooga,
    Vllm,
    TensorrtLlm,
    Transformers,
    Mlc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelFormat {
    Gguf,        // llama.cpp format
    Ggml,        // Legacy llama.cpp
    Safetensors, // Hugging Face
    Pytorch,     // PyTorch format
    Onnx,        // ONNX format
    TensorRT,    // NVIDIA TensorRT
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmOptimizationProfile {
    pub engine: LlmEngineType,
    pub model_path: String,
    pub quantization: QuantizationType,
    pub context_length: u32,
    pub batch_size: u32,
    pub gpu_layers: u32,
    pub tensor_parallel_size: u32,
    pub pipeline_parallel_size: u32,
}

pub struct LlmOptimizer {
    supported_engines: Vec<LlmEngine>,
}

impl LlmOptimizer {
    pub fn new() -> Self {
        let mut optimizer = Self {
            supported_engines: Vec::new(),
        };
        optimizer.initialize_engines();
        optimizer
    }

    pub fn get_recommended_engine(
        &self,
        model_size: &ModelSize,
        gpu_memory_gb: u32,
    ) -> LlmEngineType {
        match (model_size, gpu_memory_gb) {
            // Large models need specialized engines
            (ModelSize::XLarge, mem) if mem >= 80 => LlmEngineType::Vllm,
            (ModelSize::XLarge, _) => LlmEngineType::LlamaCpp, // CPU offload

            // Medium-large models
            (ModelSize::Large, mem) if mem >= 48 => LlmEngineType::Vllm,
            (ModelSize::Large, mem) if mem >= 24 => LlmEngineType::TensorrtLlm,
            (ModelSize::Large, _) => LlmEngineType::LlamaCpp,

            // Medium models - Ollama is great here
            (ModelSize::Medium, mem) if mem >= 8 => LlmEngineType::Ollama,
            (ModelSize::Medium, _) => LlmEngineType::LlamaCpp,

            // Small models
            (ModelSize::Small, _) => LlmEngineType::Ollama,
        }
    }

    pub fn create_optimization_profile(
        &self,
        engine: LlmEngineType,
        model_name: &str,
        model_size: &ModelSize,
        gpu_memory_gb: u32,
    ) -> Result<LlmOptimizationProfile> {
        let profile = match engine {
            LlmEngineType::Ollama => {
                self.create_ollama_profile(model_name, model_size, gpu_memory_gb)
            }
            LlmEngineType::LlamaCpp => {
                self.create_llamacpp_profile(model_name, model_size, gpu_memory_gb)
            }
            LlmEngineType::Vllm => self.create_vllm_profile(model_name, model_size, gpu_memory_gb),
            LlmEngineType::TensorrtLlm => {
                self.create_tensorrt_profile(model_name, model_size, gpu_memory_gb)
            }
            _ => self.create_default_profile(engine, model_name, model_size, gpu_memory_gb),
        }?;

        Ok(profile)
    }

    pub fn get_container_environment(
        &self,
        profile: &LlmOptimizationProfile,
    ) -> HashMap<String, String> {
        let mut env = HashMap::new();

        match profile.engine {
            LlmEngineType::Ollama => {
                env.insert(
                    "OLLAMA_GPU_LAYERS".to_string(),
                    profile.gpu_layers.to_string(),
                );
                env.insert(
                    "OLLAMA_NUM_PARALLEL".to_string(),
                    profile.batch_size.to_string(),
                );
                env.insert("OLLAMA_FLASH_ATTENTION".to_string(), "1".to_string());
            }
            LlmEngineType::LlamaCpp => {
                env.insert(
                    "LLAMA_CUDA_LAYERS".to_string(),
                    profile.gpu_layers.to_string(),
                );
                env.insert(
                    "LLAMA_CONTEXT_SIZE".to_string(),
                    profile.context_length.to_string(),
                );
                env.insert(
                    "LLAMA_BATCH_SIZE".to_string(),
                    profile.batch_size.to_string(),
                );
            }
            LlmEngineType::Vllm => {
                env.insert("VLLM_GPU_MEMORY_UTILIZATION".to_string(), "0.9".to_string());
                env.insert(
                    "VLLM_TENSOR_PARALLEL_SIZE".to_string(),
                    profile.tensor_parallel_size.to_string(),
                );
                env.insert(
                    "VLLM_MAX_MODEL_LEN".to_string(),
                    profile.context_length.to_string(),
                );
            }
            LlmEngineType::TensorrtLlm => {
                env.insert(
                    "TRTLLM_MAX_BATCH_SIZE".to_string(),
                    profile.batch_size.to_string(),
                );
                env.insert(
                    "TRTLLM_MAX_INPUT_LEN".to_string(),
                    (profile.context_length / 2).to_string(),
                );
                env.insert(
                    "TRTLLM_MAX_OUTPUT_LEN".to_string(),
                    (profile.context_length / 2).to_string(),
                );
            }
            _ => {}
        }

        // Common optimizations
        env.insert("OMP_NUM_THREADS".to_string(), num_cpus::get().to_string());
        env.insert("CUDA_VISIBLE_DEVICES".to_string(), "all".to_string());
        env.insert("NVIDIA_VISIBLE_DEVICES".to_string(), "all".to_string());

        env
    }

    pub fn get_recommended_container_image(&self, engine: &LlmEngineType) -> String {
        match engine {
            LlmEngineType::Ollama => "ollama/ollama:latest".to_string(),
            LlmEngineType::LlamaCpp => "ghcr.io/ggerganov/llama.cpp:latest".to_string(),
            LlmEngineType::Vllm => "vllm/vllm-openai:latest".to_string(),
            LlmEngineType::TensorrtLlm => "nvcr.io/nvidia/tritonserver:latest".to_string(),
            LlmEngineType::Oobabooga => {
                "ghcr.io/oobabooga/text-generation-webui:latest".to_string()
            }
            LlmEngineType::Transformers => {
                "huggingface/transformers-pytorch-gpu:latest".to_string()
            }
            LlmEngineType::Mlc => "mlcai/mlc-llm:latest".to_string(),
        }
    }

    fn create_ollama_profile(
        &self,
        model_name: &str,
        model_size: &ModelSize,
        gpu_memory_gb: u32,
    ) -> Result<LlmOptimizationProfile> {
        Ok(LlmOptimizationProfile {
            engine: LlmEngineType::Ollama,
            model_path: model_name.to_string(),
            quantization: self.select_quantization(model_size, gpu_memory_gb),
            context_length: self.get_context_length(model_size),
            batch_size: 1, // Ollama typically uses batch size 1
            gpu_layers: self.calculate_gpu_layers(model_size, gpu_memory_gb),
            tensor_parallel_size: 1,
            pipeline_parallel_size: 1,
        })
    }

    fn create_llamacpp_profile(
        &self,
        model_name: &str,
        model_size: &ModelSize,
        gpu_memory_gb: u32,
    ) -> Result<LlmOptimizationProfile> {
        Ok(LlmOptimizationProfile {
            engine: LlmEngineType::LlamaCpp,
            model_path: format!("{}.gguf", model_name),
            quantization: QuantizationType::GGML_Q4_1,
            context_length: self.get_context_length(model_size),
            batch_size: if gpu_memory_gb > 8 { 512 } else { 256 },
            gpu_layers: self.calculate_gpu_layers(model_size, gpu_memory_gb),
            tensor_parallel_size: 1,
            pipeline_parallel_size: 1,
        })
    }

    fn create_vllm_profile(
        &self,
        model_name: &str,
        model_size: &ModelSize,
        gpu_memory_gb: u32,
    ) -> Result<LlmOptimizationProfile> {
        let tensor_parallel = match model_size {
            ModelSize::XLarge => (gpu_memory_gb / 40).max(1),
            ModelSize::Large => (gpu_memory_gb / 24).max(1),
            _ => 1,
        };

        Ok(LlmOptimizationProfile {
            engine: LlmEngineType::Vllm,
            model_path: model_name.to_string(),
            quantization: QuantizationType::FP16,
            context_length: self.get_context_length(model_size),
            batch_size: 64,  // vLLM handles batching internally
            gpu_layers: 999, // vLLM loads entire model on GPU
            tensor_parallel_size: tensor_parallel,
            pipeline_parallel_size: 1,
        })
    }

    fn create_tensorrt_profile(
        &self,
        model_name: &str,
        model_size: &ModelSize,
        gpu_memory_gb: u32,
    ) -> Result<LlmOptimizationProfile> {
        Ok(LlmOptimizationProfile {
            engine: LlmEngineType::TensorrtLlm,
            model_path: format!("{}_trt", model_name),
            quantization: QuantizationType::INT8, // TensorRT loves INT8
            context_length: self.get_context_length(model_size),
            batch_size: (gpu_memory_gb / 4).max(1).min(32),
            gpu_layers: 999, // TensorRT optimizes entire model
            tensor_parallel_size: if gpu_memory_gb >= 48 { 2 } else { 1 },
            pipeline_parallel_size: 1,
        })
    }

    fn create_default_profile(
        &self,
        engine: LlmEngineType,
        model_name: &str,
        model_size: &ModelSize,
        gpu_memory_gb: u32,
    ) -> Result<LlmOptimizationProfile> {
        Ok(LlmOptimizationProfile {
            engine,
            model_path: model_name.to_string(),
            quantization: self.select_quantization(model_size, gpu_memory_gb),
            context_length: self.get_context_length(model_size),
            batch_size: 1,
            gpu_layers: self.calculate_gpu_layers(model_size, gpu_memory_gb),
            tensor_parallel_size: 1,
            pipeline_parallel_size: 1,
        })
    }

    fn select_quantization(&self, model_size: &ModelSize, gpu_memory_gb: u32) -> QuantizationType {
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

    fn get_context_length(&self, model_size: &ModelSize) -> u32 {
        match model_size {
            ModelSize::XLarge => 8192,
            ModelSize::Large => 4096,
            ModelSize::Medium => 4096,
            ModelSize::Small => 2048,
        }
    }

    fn calculate_gpu_layers(&self, model_size: &ModelSize, gpu_memory_gb: u32) -> u32 {
        let base_layers = match model_size {
            ModelSize::XLarge => 100,
            ModelSize::Large => 80,
            ModelSize::Medium => 32,
            ModelSize::Small => 16,
        };

        match gpu_memory_gb {
            0..=4 => 0,
            5..=8 => base_layers / 4,
            9..=16 => base_layers / 2,
            17..=24 => (base_layers * 3) / 4,
            _ => base_layers,
        }
    }

    fn initialize_engines(&mut self) {
        self.supported_engines = vec![
            LlmEngine {
                name: "Ollama".to_string(),
                engine_type: LlmEngineType::Ollama,
                supported_formats: vec![ModelFormat::Gguf, ModelFormat::Ggml],
                gpu_support: true,
                quantization_support: vec![
                    QuantizationType::FP16,
                    QuantizationType::GGML_Q4_0,
                    QuantizationType::GGML_Q4_1,
                    QuantizationType::GGML_Q5_0,
                    QuantizationType::GGML_Q8_0,
                ],
            },
            LlmEngine {
                name: "vLLM".to_string(),
                engine_type: LlmEngineType::Vllm,
                supported_formats: vec![ModelFormat::Safetensors, ModelFormat::Pytorch],
                gpu_support: true,
                quantization_support: vec![QuantizationType::FP16, QuantizationType::INT8],
            },
            LlmEngine {
                name: "TensorRT-LLM".to_string(),
                engine_type: LlmEngineType::TensorrtLlm,
                supported_formats: vec![ModelFormat::TensorRT],
                gpu_support: true,
                quantization_support: vec![
                    QuantizationType::FP16,
                    QuantizationType::INT8,
                    QuantizationType::INT4,
                ],
            },
            // Add more engines as needed
        ];
    }
}

impl Default for LlmOptimizer {
    fn default() -> Self {
        Self::new()
    }
}
