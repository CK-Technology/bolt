//! GPU Profile Integration
//!
//! Bridges gaming profiles and AI workload configurations with CDI spec generation.
//! Provides unified profile-aware GPU configuration for containers.

use crate::ai::{
    AiHardwareConfig, AiMemoryConfig, AiPerformanceConfig, AiWorkloadConfig, AiWorkloadType,
    GpuAllocation, ModelConfig, ModelSize, QuantizationType, VectorInstructions,
};
use crate::gaming::profiles::{
    DlssMode, GamingProfile, GamingProfileManager, PerformanceMode, RaytracingQuality,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// Unified GPU profile type for CDI generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuProfile {
    /// Gaming profile with game-specific optimizations
    Gaming(GamingProfileSettings),
    /// AI/ML inference profile
    AiInference(AiProfileSettings),
    /// AI/ML training profile
    AiTraining(AiProfileSettings),
    /// General purpose (balanced settings)
    General,
}

/// Gaming profile settings for CDI generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingProfileSettings {
    pub profile_name: String,
    pub performance_mode: PerformanceMode,
    pub power_limit_watts: Option<u32>,
    pub gpu_clock_offset_mhz: Option<i32>,
    pub memory_clock_offset_mhz: Option<i32>,
    pub dlss_enabled: bool,
    pub dlss_mode: Option<DlssMode>,
    pub raytracing_enabled: bool,
    pub raytracing_quality: Option<RaytracingQuality>,
    pub reflex_enabled: bool,
    pub low_latency_mode: bool,
    pub target_fps: u32,
    pub expected_vram_mb: u32,
}

/// AI profile settings for CDI generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProfileSettings {
    pub model_name: String,
    pub model_size: ModelSize,
    pub quantization: Option<QuantizationType>,
    pub context_length: Option<u32>,
    pub batch_size: Option<u32>,
    pub flash_attention: bool,
    pub tensor_parallelism: bool,
    pub mixed_precision: bool,
    pub memory_limit_gb: Option<u32>,
    pub multi_gpu: bool,
}

/// GPU Profile Manager - bridges gaming/AI profiles with CDI generation
pub struct GpuProfileManager {
    gaming_profiles: GamingProfileManager,
    ai_profiles: HashMap<String, AiWorkloadConfig>,
}

impl GpuProfileManager {
    pub fn new() -> Self {
        let gaming_profiles = GamingProfileManager::new();
        let mut ai_profiles = HashMap::new();

        // Add built-in AI profiles
        ai_profiles.insert("ollama-small".to_string(), Self::ollama_small_profile());
        ai_profiles.insert("ollama-medium".to_string(), Self::ollama_medium_profile());
        ai_profiles.insert("ollama-large".to_string(), Self::ollama_large_profile());
        ai_profiles.insert("training".to_string(), Self::training_profile());
        ai_profiles.insert("inference".to_string(), Self::inference_profile());

        info!(
            "GPU Profile Manager initialized with {} gaming and {} AI profiles",
            gaming_profiles.list_profiles().len(),
            ai_profiles.len()
        );

        Self {
            gaming_profiles,
            ai_profiles,
        }
    }

    /// Get a gaming profile by name and convert to CDI-compatible settings
    pub fn get_gaming_profile(&self, name: &str) -> Option<GamingProfileSettings> {
        self.gaming_profiles
            .get_profile(name)
            .map(|p| self.convert_gaming_profile(p))
    }

    /// Get a gaming profile by Steam App ID
    pub fn get_gaming_profile_by_app_id(&self, app_id: &str) -> Option<GamingProfileSettings> {
        self.gaming_profiles
            .get_profile_by_app_id(app_id)
            .map(|p| self.convert_gaming_profile(p))
    }

    /// Get an AI profile by name
    pub fn get_ai_profile(&self, name: &str) -> Option<AiProfileSettings> {
        self.ai_profiles
            .get(name)
            .map(|c| self.convert_ai_config(c))
    }

    /// List all available gaming profiles
    pub fn list_gaming_profiles(&self) -> Vec<String> {
        self.gaming_profiles.list_profiles()
    }

    /// List all available AI profiles
    pub fn list_ai_profiles(&self) -> Vec<String> {
        self.ai_profiles.keys().cloned().collect()
    }

    /// Generate environment variables for a GPU profile
    pub fn get_profile_env_vars(&self, profile: &GpuProfile) -> HashMap<String, String> {
        let mut env = HashMap::new();

        match profile {
            GpuProfile::Gaming(settings) => {
                self.add_gaming_env_vars(&mut env, settings);
            }
            GpuProfile::AiInference(settings) | GpuProfile::AiTraining(settings) => {
                self.add_ai_env_vars(&mut env, settings);
            }
            GpuProfile::General => {
                // Minimal default settings
                env.insert("GPU_PROFILE".to_string(), "general".to_string());
            }
        }

        env
    }

    /// Generate NVIDIA-specific CDI environment for a profile
    pub fn get_nvidia_cdi_env(&self, profile: &GpuProfile) -> Vec<String> {
        let mut env = vec![
            "NVIDIA_VISIBLE_DEVICES=all".to_string(),
            "NVIDIA_DRIVER_CAPABILITIES=all".to_string(),
        ];

        match profile {
            GpuProfile::Gaming(settings) => {
                env.push("BOLT_GPU_PROFILE=gaming".to_string());
                env.push(format!("BOLT_GAME_PROFILE={}", settings.profile_name));

                // Performance mode
                let perf_mode = match settings.performance_mode {
                    PerformanceMode::UltraLowLatency => "ultra_low_latency",
                    PerformanceMode::Performance => "performance",
                    PerformanceMode::Balanced => "balanced",
                    PerformanceMode::Quiet => "quiet",
                };
                env.push(format!("NVIDIA_PERF_MODE={}", perf_mode));

                // DLSS settings
                if settings.dlss_enabled {
                    env.push("NVIDIA_DLSS_ENABLED=1".to_string());
                    if let Some(ref mode) = settings.dlss_mode {
                        let dlss_mode = match mode {
                            DlssMode::Off => "off",
                            DlssMode::Quality => "quality",
                            DlssMode::Balanced => "balanced",
                            DlssMode::Performance => "performance",
                            DlssMode::UltraPerformance => "ultra_performance",
                        };
                        env.push(format!("DLSS_PERFMODE={}", dlss_mode.to_uppercase()));
                    }
                }

                // Ray tracing settings
                if settings.raytracing_enabled {
                    env.push("NVIDIA_ENABLE_RTX=1".to_string());
                    if let Some(ref quality) = settings.raytracing_quality {
                        let rt_quality = match quality {
                            RaytracingQuality::Off => "off",
                            RaytracingQuality::Low => "low",
                            RaytracingQuality::Medium => "medium",
                            RaytracingQuality::High => "high",
                            RaytracingQuality::Ultra => "ultra",
                            RaytracingQuality::Psycho => "psycho",
                        };
                        env.push(format!("RTX_QUALITY={}", rt_quality));
                    }
                }

                // Reflex / low latency
                if settings.reflex_enabled {
                    env.push("NVIDIA_REFLEX_ENABLED=1".to_string());
                    env.push("NVIDIA_LOW_LATENCY_MODE=ultra".to_string());
                } else if settings.low_latency_mode {
                    env.push("NVIDIA_LOW_LATENCY_MODE=on".to_string());
                }

                // Vsync settings for target FPS
                if settings.target_fps >= 120 {
                    env.push("__GL_SYNC_TO_VBLANK=0".to_string());
                }
            }
            GpuProfile::AiInference(settings) => {
                env.push("BOLT_GPU_PROFILE=ai_inference".to_string());
                env.push(format!("BOLT_MODEL={}", settings.model_name));

                // CUDA settings for inference
                env.push("CUDA_DEVICE_ORDER=PCI_BUS_ID".to_string());

                if settings.flash_attention {
                    env.push("OLLAMA_FLASH_ATTENTION=1".to_string());
                }

                if settings.mixed_precision {
                    env.push("NVIDIA_TF32_OVERRIDE=1".to_string());
                }

                // Memory settings
                if let Some(limit) = settings.memory_limit_gb {
                    env.push(format!("GPU_MEMORY_LIMIT_GB={}", limit));
                }

                // Multi-GPU settings
                if settings.multi_gpu {
                    env.push("NCCL_P2P_DISABLE=0".to_string());
                    env.push("NCCL_IB_DISABLE=1".to_string());
                }
            }
            GpuProfile::AiTraining(settings) => {
                env.push("BOLT_GPU_PROFILE=ai_training".to_string());
                env.push(format!("BOLT_MODEL={}", settings.model_name));

                // CUDA settings for training
                env.push("CUDA_DEVICE_ORDER=PCI_BUS_ID".to_string());
                env.push("PYTORCH_CUDA_ALLOC_CONF=max_split_size_mb:128".to_string());

                if settings.tensor_parallelism {
                    env.push("NCCL_DEBUG=INFO".to_string());
                }

                if settings.mixed_precision {
                    env.push("NVIDIA_TF32_OVERRIDE=1".to_string());
                    env.push("TORCH_ALLOW_TF32_CUBLAS_OVERRIDE=1".to_string());
                }

                // Memory settings
                env.push("MALLOC_MMAP_THRESHOLD_=131072".to_string());
            }
            GpuProfile::General => {
                env.push("BOLT_GPU_PROFILE=general".to_string());
            }
        }

        env
    }

    /// Generate AMD-specific CDI environment for a profile
    pub fn get_amd_cdi_env(&self, profile: &GpuProfile) -> Vec<String> {
        let mut env = vec!["AMD_VISIBLE_DEVICES=all".to_string()];

        match profile {
            GpuProfile::Gaming(settings) => {
                env.push("BOLT_GPU_PROFILE=gaming".to_string());
                env.push(format!("BOLT_GAME_PROFILE={}", settings.profile_name));

                // Vulkan/RADV settings for gaming
                env.push("AMD_VULKAN_ICD=RADV".to_string());
                env.push("RADV_PERFTEST=gpl".to_string());
                env.push("VKD3D_CONFIG=dxr".to_string());

                // Low latency mode
                if settings.low_latency_mode {
                    env.push("MESA_VK_WSI_PRESENT_MODE=mailbox".to_string());
                }

                // Ray tracing (RDNA2+)
                if settings.raytracing_enabled {
                    env.push("RADV_PERFTEST=rt".to_string());
                }
            }
            GpuProfile::AiInference(settings) | GpuProfile::AiTraining(settings) => {
                let profile_type = if matches!(profile, GpuProfile::AiTraining(_)) {
                    "ai_training"
                } else {
                    "ai_inference"
                };
                env.push(format!("BOLT_GPU_PROFILE={}", profile_type));
                env.push(format!("BOLT_MODEL={}", settings.model_name));

                // ROCm settings
                env.push("HSA_OVERRIDE_GFX_VERSION=10.3.0".to_string());
                env.push("HIP_VISIBLE_DEVICES=0".to_string());
                env.push("ROCR_VISIBLE_DEVICES=0".to_string());

                if settings.multi_gpu {
                    env.push("NCCL_P2P_DISABLE=0".to_string());
                }
            }
            GpuProfile::General => {
                env.push("BOLT_GPU_PROFILE=general".to_string());
            }
        }

        env
    }

    /// Generate Intel-specific CDI environment for a profile
    pub fn get_intel_cdi_env(&self, profile: &GpuProfile) -> Vec<String> {
        let mut env = vec!["INTEL_VISIBLE_DEVICES=all".to_string()];

        match profile {
            GpuProfile::Gaming(settings) => {
                env.push("BOLT_GPU_PROFILE=gaming".to_string());
                env.push(format!("BOLT_GAME_PROFILE={}", settings.profile_name));

                // Intel ANV settings for gaming
                env.push("ANV_ENABLE_PIPELINE_CACHE=1".to_string());
                env.push("MESA_VK_DEVICE_SELECT=list".to_string());

                // XeSS for Arc GPUs (similar to DLSS)
                if settings.dlss_enabled {
                    env.push("ENABLE_XESS=1".to_string());
                }

                // Low latency
                if settings.low_latency_mode {
                    env.push("MESA_VK_WSI_PRESENT_MODE=mailbox".to_string());
                }
            }
            GpuProfile::AiInference(settings) | GpuProfile::AiTraining(settings) => {
                let profile_type = if matches!(profile, GpuProfile::AiTraining(_)) {
                    "ai_training"
                } else {
                    "ai_inference"
                };
                env.push(format!("BOLT_GPU_PROFILE={}", profile_type));
                env.push(format!("BOLT_MODEL={}", settings.model_name));

                // Level Zero / oneAPI settings
                env.push("ZE_AFFINITY_MASK=0".to_string());
                env.push("ZE_ENABLE_PCI_ID_DEVICE_ORDER=1".to_string());
                env.push("ONEAPI_DEVICE_SELECTOR=level_zero:*".to_string());
            }
            GpuProfile::General => {
                env.push("BOLT_GPU_PROFILE=general".to_string());
            }
        }

        env
    }

    // ============= Private Helper Methods =============

    fn convert_gaming_profile(&self, profile: &GamingProfile) -> GamingProfileSettings {
        let nvidia_settings = profile.nvidia_settings.as_ref();

        GamingProfileSettings {
            profile_name: profile.name.clone(),
            performance_mode: profile.gpu_config.performance_mode.clone(),
            power_limit_watts: profile.gpu_config.power_limit_watts,
            gpu_clock_offset_mhz: profile.gpu_config.gpu_clock_offset_mhz,
            memory_clock_offset_mhz: profile.gpu_config.memory_clock_offset_mhz,
            dlss_enabled: nvidia_settings
                .map(|s| !matches!(s.dlss_mode, Some(DlssMode::Off) | None))
                .unwrap_or(false),
            dlss_mode: nvidia_settings.and_then(|s| s.dlss_mode.clone()),
            raytracing_enabled: nvidia_settings
                .map(|s| !matches!(s.raytracing_quality, Some(RaytracingQuality::Off) | None))
                .unwrap_or(false),
            raytracing_quality: nvidia_settings.and_then(|s| s.raytracing_quality.clone()),
            reflex_enabled: nvidia_settings.map(|s| s.reflex_enabled).unwrap_or(false),
            low_latency_mode: matches!(
                profile.gpu_config.performance_mode,
                PerformanceMode::UltraLowLatency | PerformanceMode::Performance
            ),
            target_fps: profile.performance_hints.target_fps,
            expected_vram_mb: profile.performance_hints.expected_vram_mb,
        }
    }

    fn convert_ai_config(&self, config: &AiWorkloadConfig) -> AiProfileSettings {
        AiProfileSettings {
            model_name: config.model_config.model_name.clone(),
            model_size: config.model_config.model_size.clone(),
            quantization: config.model_config.quantization.clone(),
            context_length: config.model_config.context_length,
            batch_size: config.model_config.batch_size,
            flash_attention: config.performance_config.flash_attention,
            tensor_parallelism: config.performance_config.tensor_parallelism,
            mixed_precision: config.performance_config.mixed_precision,
            memory_limit_gb: config.hardware_config.memory_config.memory_limit_gb,
            multi_gpu: matches!(
                config.hardware_config.gpu_allocation,
                GpuAllocation::MultiGpu { .. }
            ),
        }
    }

    fn add_gaming_env_vars(
        &self,
        env: &mut HashMap<String, String>,
        settings: &GamingProfileSettings,
    ) {
        env.insert("GPU_PROFILE".to_string(), "gaming".to_string());
        env.insert("GAME_PROFILE".to_string(), settings.profile_name.clone());
        env.insert("TARGET_FPS".to_string(), settings.target_fps.to_string());
        env.insert(
            "EXPECTED_VRAM_MB".to_string(),
            settings.expected_vram_mb.to_string(),
        );

        if settings.dlss_enabled {
            env.insert("DLSS_ENABLED".to_string(), "1".to_string());
        }
        if settings.raytracing_enabled {
            env.insert("RAYTRACING_ENABLED".to_string(), "1".to_string());
        }
        if settings.reflex_enabled {
            env.insert("REFLEX_ENABLED".to_string(), "1".to_string());
        }
        if settings.low_latency_mode {
            env.insert("LOW_LATENCY_MODE".to_string(), "1".to_string());
        }
    }

    fn add_ai_env_vars(&self, env: &mut HashMap<String, String>, settings: &AiProfileSettings) {
        env.insert("GPU_PROFILE".to_string(), "ai".to_string());
        env.insert("MODEL_NAME".to_string(), settings.model_name.clone());

        if let Some(ctx_len) = settings.context_length {
            env.insert("CONTEXT_LENGTH".to_string(), ctx_len.to_string());
        }
        if let Some(batch) = settings.batch_size {
            env.insert("BATCH_SIZE".to_string(), batch.to_string());
        }
        if settings.flash_attention {
            env.insert("FLASH_ATTENTION".to_string(), "1".to_string());
        }
        if settings.tensor_parallelism {
            env.insert("TENSOR_PARALLELISM".to_string(), "1".to_string());
        }
        if settings.mixed_precision {
            env.insert("MIXED_PRECISION".to_string(), "1".to_string());
        }
        if settings.multi_gpu {
            env.insert("MULTI_GPU".to_string(), "1".to_string());
        }
    }

    // ============= Built-in AI Profiles =============

    fn ollama_small_profile() -> AiWorkloadConfig {
        AiWorkloadConfig {
            workload_type: AiWorkloadType::Inference,
            model_config: ModelConfig {
                model_name: "phi3:mini".to_string(),
                model_size: ModelSize::Small,
                quantization: Some(QuantizationType::GgmlQ4_0),
                context_length: Some(4096),
                batch_size: Some(1),
                max_tokens: Some(2048),
            },
            hardware_config: AiHardwareConfig {
                gpu_allocation: GpuAllocation::Shared { percentage: 40 },
                memory_config: AiMemoryConfig {
                    enable_huge_pages: false,
                    memory_limit_gb: Some(4),
                    swap_disabled: true,
                    numa_awareness: false,
                    memory_pooling: true,
                },
                cpu_config: crate::ai::AiCpuConfig {
                    thread_count: Some(4),
                    cpu_affinity: None,
                    simd_optimization: true,
                    vector_instructions: VectorInstructions::Auto,
                },
            },
            performance_config: AiPerformanceConfig {
                flash_attention: true,
                tensor_parallelism: false,
                pipeline_parallelism: false,
                gradient_checkpointing: false,
                mixed_precision: false,
                compile_optimization: false,
            },
        }
    }

    fn ollama_medium_profile() -> AiWorkloadConfig {
        AiWorkloadConfig {
            workload_type: AiWorkloadType::Inference,
            model_config: ModelConfig {
                model_name: "llama3:8b".to_string(),
                model_size: ModelSize::Medium,
                quantization: Some(QuantizationType::GgmlQ4_1),
                context_length: Some(8192),
                batch_size: Some(1),
                max_tokens: Some(4096),
            },
            hardware_config: AiHardwareConfig {
                gpu_allocation: GpuAllocation::Exclusive,
                memory_config: AiMemoryConfig {
                    enable_huge_pages: true,
                    memory_limit_gb: Some(12),
                    swap_disabled: true,
                    numa_awareness: true,
                    memory_pooling: true,
                },
                cpu_config: crate::ai::AiCpuConfig {
                    thread_count: Some(8),
                    cpu_affinity: None,
                    simd_optimization: true,
                    vector_instructions: VectorInstructions::Auto,
                },
            },
            performance_config: AiPerformanceConfig {
                flash_attention: true,
                tensor_parallelism: false,
                pipeline_parallelism: false,
                gradient_checkpointing: false,
                mixed_precision: true,
                compile_optimization: true,
            },
        }
    }

    fn ollama_large_profile() -> AiWorkloadConfig {
        AiWorkloadConfig {
            workload_type: AiWorkloadType::Inference,
            model_config: ModelConfig {
                model_name: "llama3:70b".to_string(),
                model_size: ModelSize::Large,
                quantization: Some(QuantizationType::GgmlQ4_0),
                context_length: Some(8192),
                batch_size: Some(1),
                max_tokens: Some(4096),
            },
            hardware_config: AiHardwareConfig {
                gpu_allocation: GpuAllocation::Exclusive,
                memory_config: AiMemoryConfig {
                    enable_huge_pages: true,
                    memory_limit_gb: Some(48),
                    swap_disabled: true,
                    numa_awareness: true,
                    memory_pooling: true,
                },
                cpu_config: crate::ai::AiCpuConfig {
                    thread_count: Some(16),
                    cpu_affinity: None,
                    simd_optimization: true,
                    vector_instructions: VectorInstructions::Auto,
                },
            },
            performance_config: AiPerformanceConfig {
                flash_attention: true,
                tensor_parallelism: true,
                pipeline_parallelism: false,
                gradient_checkpointing: false,
                mixed_precision: true,
                compile_optimization: true,
            },
        }
    }

    fn training_profile() -> AiWorkloadConfig {
        AiWorkloadConfig {
            workload_type: AiWorkloadType::Training,
            model_config: ModelConfig {
                model_name: "custom-training".to_string(),
                model_size: ModelSize::Medium,
                quantization: None, // Full precision for training
                context_length: Some(2048),
                batch_size: Some(8),
                max_tokens: None,
            },
            hardware_config: AiHardwareConfig {
                gpu_allocation: GpuAllocation::Exclusive,
                memory_config: AiMemoryConfig {
                    enable_huge_pages: true,
                    memory_limit_gb: None, // Use all available
                    swap_disabled: true,
                    numa_awareness: true,
                    memory_pooling: true,
                },
                cpu_config: crate::ai::AiCpuConfig {
                    thread_count: None, // Use all
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
        }
    }

    fn inference_profile() -> AiWorkloadConfig {
        AiWorkloadConfig {
            workload_type: AiWorkloadType::Inference,
            model_config: ModelConfig {
                model_name: "inference".to_string(),
                model_size: ModelSize::Medium,
                quantization: Some(QuantizationType::FP16),
                context_length: Some(4096),
                batch_size: Some(1),
                max_tokens: Some(2048),
            },
            hardware_config: AiHardwareConfig {
                gpu_allocation: GpuAllocation::Exclusive,
                memory_config: AiMemoryConfig {
                    enable_huge_pages: true,
                    memory_limit_gb: None,
                    swap_disabled: true,
                    numa_awareness: true,
                    memory_pooling: true,
                },
                cpu_config: crate::ai::AiCpuConfig {
                    thread_count: Some(8),
                    cpu_affinity: None,
                    simd_optimization: true,
                    vector_instructions: VectorInstructions::Auto,
                },
            },
            performance_config: AiPerformanceConfig {
                flash_attention: true,
                tensor_parallelism: false,
                pipeline_parallelism: false,
                gradient_checkpointing: false,
                mixed_precision: true,
                compile_optimization: true,
            },
        }
    }
}

impl Default for GpuProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_manager_creation() {
        let manager = GpuProfileManager::new();
        assert!(!manager.list_gaming_profiles().is_empty());
        assert!(!manager.list_ai_profiles().is_empty());
    }

    #[test]
    fn test_gaming_profile_conversion() {
        let manager = GpuProfileManager::new();
        let profile = manager.get_gaming_profile("cyberpunk 2077");
        assert!(profile.is_some());

        let settings = profile.unwrap();
        assert_eq!(settings.profile_name, "Cyberpunk 2077");
        assert!(settings.raytracing_enabled);
        assert!(settings.dlss_enabled);
    }

    #[test]
    fn test_ai_profile_conversion() {
        let manager = GpuProfileManager::new();
        let profile = manager.get_ai_profile("ollama-medium");
        assert!(profile.is_some());

        let settings = profile.unwrap();
        assert_eq!(settings.model_name, "llama3:8b");
        assert!(settings.flash_attention);
    }

    #[test]
    fn test_nvidia_cdi_env_gaming() {
        let manager = GpuProfileManager::new();
        let settings = manager.get_gaming_profile("counter-strike 2").unwrap();
        let profile = GpuProfile::Gaming(settings);

        let env = manager.get_nvidia_cdi_env(&profile);
        assert!(env.iter().any(|e| e.contains("REFLEX")));
        assert!(env.iter().any(|e| e.contains("LOW_LATENCY")));
    }

    #[test]
    fn test_nvidia_cdi_env_ai() {
        let manager = GpuProfileManager::new();
        let settings = manager.get_ai_profile("ollama-large").unwrap();
        let profile = GpuProfile::AiInference(settings);

        let env = manager.get_nvidia_cdi_env(&profile);
        assert!(env.iter().any(|e| e.contains("FLASH_ATTENTION")));
        assert!(env.iter().any(|e| e.contains("TF32")));
    }
}
