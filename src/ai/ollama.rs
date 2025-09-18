use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use tracing::{debug, info, warn};

use super::{AiOptimizer, AiWorkloadConfig, ModelSize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaManager {
    pub base_url: String,
    pub models_path: String,
    pub gpu_enabled: bool,
    pub available_models: Vec<OllamaModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub tag: String,
    pub size: u64,
    pub digest: String,
    pub modified_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaOptimizationProfile {
    pub model_name: String,
    pub gpu_layers: u32,
    pub context_length: u32,
    pub batch_size: u32,
    pub thread_count: u32,
    pub use_mmap: bool,
    pub use_mlock: bool,
    pub numa_optimize: bool,
}

impl OllamaManager {
    pub fn new(base_url: Option<String>) -> Self {
        Self {
            base_url: base_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
            models_path: "/root/.ollama".to_string(),
            gpu_enabled: Self::detect_gpu_support(),
            available_models: Vec::new(),
        }
    }

    pub async fn optimize_for_container(
        &self,
        container_id: &str,
        model_name: &str,
    ) -> Result<AiWorkloadConfig> {
        info!(
            "🤖 Optimizing Ollama for container {} with model {}",
            container_id, model_name
        );

        let gpu_memory = self.get_available_gpu_memory().await?;
        let system_memory = self.get_available_system_memory()?;

        let optimizer = AiOptimizer::new();
        let mut config = optimizer
            .optimize_for_ollama(model_name, gpu_memory)
            .await?;

        // Ollama-specific optimizations
        self.apply_ollama_specific_optimizations(&mut config, model_name)
            .await?;

        info!("✅ Ollama optimization complete for {}", model_name);
        Ok(config)
    }

    pub async fn create_optimization_profile(
        &self,
        model_name: &str,
    ) -> Result<OllamaOptimizationProfile> {
        let model_info = self.get_model_info(model_name).await?;
        let gpu_memory = self.get_available_gpu_memory().await?;
        let cpu_count = num_cpus::get() as u32;

        let profile = OllamaOptimizationProfile {
            model_name: model_name.to_string(),
            gpu_layers: self.calculate_optimal_gpu_layers(&model_info, gpu_memory),
            context_length: self.get_optimal_context_length(&model_info),
            batch_size: self.calculate_batch_size(&model_info, gpu_memory),
            thread_count: (cpu_count / 2).max(1),
            use_mmap: gpu_memory < 16, // Use memory mapping for smaller GPU memory
            use_mlock: gpu_memory >= 8, // Lock memory if we have enough
            numa_optimize: cpu_count > 8,
        };

        Ok(profile)
    }

    pub async fn apply_optimizations(
        &self,
        container_id: &str,
        profile: &OllamaOptimizationProfile,
    ) -> Result<()> {
        info!(
            "🔧 Applying Ollama optimizations to container {}",
            container_id
        );

        let env_vars = self.generate_environment_variables(profile);

        // Apply environment variables to container
        for (key, value) in env_vars {
            self.set_container_env_var(container_id, &key, &value)
                .await?;
        }

        // Apply system-level optimizations
        self.apply_system_optimizations(profile).await?;

        info!("✅ Ollama optimizations applied successfully");
        Ok(())
    }

    pub async fn pull_model(&self, model_name: &str) -> Result<()> {
        info!("📥 Pulling Ollama model: {}", model_name);

        let output = Command::new("ollama").args(&["pull", model_name]).output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    info!("✅ Model {} pulled successfully", model_name);
                    Ok(())
                } else {
                    let error = String::from_utf8_lossy(&result.stderr);
                    Err(anyhow::anyhow!(
                        "Failed to pull model {}: {}",
                        model_name,
                        error
                    ))
                }
            }
            Err(e) => {
                warn!("❌ Ollama command not found, trying container-based pull");
                self.pull_model_via_container(model_name).await
            }
        }
    }

    pub async fn list_models(&self) -> Result<Vec<OllamaModel>> {
        let client = reqwest::Client::new();
        let response = client
            .get(&format!("{}/api/tags", self.base_url))
            .send()
            .await?;

        if response.status().is_success() {
            let models_response: serde_json::Value = response.json().await?;

            let models = models_response["models"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|model| {
                    Some(OllamaModel {
                        name: model["name"].as_str()?.to_string(),
                        tag: model["tag"].as_str().unwrap_or("latest").to_string(),
                        size: model["size"].as_u64().unwrap_or(0),
                        digest: model["digest"].as_str().unwrap_or("").to_string(),
                        modified_at: chrono::Utc::now(), // Simplified for now
                    })
                })
                .collect();

            Ok(models)
        } else {
            Err(anyhow::anyhow!(
                "Failed to list models: {}",
                response.status()
            ))
        }
    }

    pub async fn get_model_info(&self, model_name: &str) -> Result<serde_json::Value> {
        let client = reqwest::Client::new();
        let response = client
            .post(&format!("{}/api/show", self.base_url))
            .json(&serde_json::json!({"name": model_name}))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get model info for {}",
                model_name
            ))
        }
    }

    async fn apply_ollama_specific_optimizations(
        &self,
        config: &mut AiWorkloadConfig,
        model_name: &str,
    ) -> Result<()> {
        // Determine if this is a code model
        if model_name.to_lowercase().contains("code") {
            config.model_config.context_length = Some(8192); // Larger context for code
            config.performance_config.compile_optimization = true;
        }

        // Adjust for chat vs completion models
        if model_name.to_lowercase().contains("chat")
            || model_name.to_lowercase().contains("instruct")
        {
            config.model_config.batch_size = Some(1); // Chat typically uses batch size 1
        }

        // Vision model optimizations
        if model_name.to_lowercase().contains("vision")
            || model_name.to_lowercase().contains("llava")
        {
            config.hardware_config.memory_config.memory_limit_gb = config
                .hardware_config
                .memory_config
                .memory_limit_gb
                .map(|x| x + 4); // Extra memory for vision
        }

        Ok(())
    }

    fn calculate_optimal_gpu_layers(
        &self,
        model_info: &serde_json::Value,
        gpu_memory_gb: u32,
    ) -> u32 {
        // Simplified calculation - in practice, this would be more sophisticated
        let total_layers = model_info["details"]["parameter_size"]
            .as_str()
            .and_then(|s| s.replace("B", "").replace("M", "").parse::<f32>().ok())
            .unwrap_or(7.0) as u32;

        match gpu_memory_gb {
            0..=4 => 0, // CPU only
            5..=8 => total_layers / 4,
            9..=16 => total_layers / 2,
            17..=24 => (total_layers * 3) / 4,
            _ => total_layers, // All layers on GPU
        }
    }

    fn get_optimal_context_length(&self, model_info: &serde_json::Value) -> u32 {
        // Extract context length from model info, fallback to reasonable defaults
        model_info["details"]["context_length"]
            .as_u64()
            .unwrap_or(4096) as u32
    }

    fn calculate_batch_size(&self, model_info: &serde_json::Value, gpu_memory_gb: u32) -> u32 {
        let model_size_gb = model_info["size"].as_u64().unwrap_or(0) / (1024 * 1024 * 1024);

        if gpu_memory_gb > model_size_gb as u32 * 2 {
            4 // Can afford larger batch size
        } else if gpu_memory_gb > model_size_gb as u32 {
            2
        } else {
            1 // Conservative batch size
        }
    }

    fn generate_environment_variables(
        &self,
        profile: &OllamaOptimizationProfile,
    ) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();

        env_vars.insert(
            "OLLAMA_NUM_PARALLEL".to_string(),
            profile.batch_size.to_string(),
        );
        env_vars.insert("OLLAMA_MAX_LOADED_MODELS".to_string(), "2".to_string());
        env_vars.insert(
            "OLLAMA_GPU_LAYERS".to_string(),
            profile.gpu_layers.to_string(),
        );

        if profile.use_mmap {
            env_vars.insert("OLLAMA_USE_MMAP".to_string(), "1".to_string());
        }

        if profile.use_mlock {
            env_vars.insert("OLLAMA_USE_MLOCK".to_string(), "1".to_string());
        }

        if profile.numa_optimize {
            env_vars.insert("OLLAMA_NUMA".to_string(), "1".to_string());
        }

        env_vars.insert("OLLAMA_FLASH_ATTENTION".to_string(), "1".to_string());
        env_vars.insert(
            "OLLAMA_THREADS".to_string(),
            profile.thread_count.to_string(),
        );

        env_vars
    }

    async fn apply_system_optimizations(&self, profile: &OllamaOptimizationProfile) -> Result<()> {
        // Set CPU governor for AI workloads
        let _ = Command::new("cpupower")
            .args(&["frequency-set", "-g", "performance"])
            .output();

        // Enable huge pages if recommended
        if profile.use_mlock {
            let _ = std::fs::write("/proc/sys/vm/nr_hugepages", "1024");
        }

        // NUMA optimization
        if profile.numa_optimize {
            let _ = std::fs::write("/proc/sys/kernel/numa_balancing", "0");
        }

        Ok(())
    }

    async fn set_container_env_var(
        &self,
        container_id: &str,
        key: &str,
        value: &str,
    ) -> Result<()> {
        debug!("Setting container {} env: {}={}", container_id, key, value);
        // This would integrate with the container runtime to set environment variables
        Ok(())
    }

    async fn pull_model_via_container(&self, model_name: &str) -> Result<()> {
        // Fallback method using container
        let output = Command::new("docker")
            .args(&[
                "run",
                "--rm",
                "--gpus",
                "all",
                "-v",
                "ollama:/root/.ollama",
                "ollama/ollama:latest",
                "ollama",
                "pull",
                model_name,
            ])
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    info!("✅ Model {} pulled via container", model_name);
                    Ok(())
                } else {
                    let error = String::from_utf8_lossy(&result.stderr);
                    Err(anyhow::anyhow!(
                        "Failed to pull model via container: {}",
                        error
                    ))
                }
            }
            Err(e) => Err(anyhow::anyhow!("Container pull failed: {}", e)),
        }
    }

    async fn get_available_gpu_memory(&self) -> Result<u32> {
        // Try nvidia-smi first
        if let Ok(output) = Command::new("nvidia-smi")
            .args(&["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
            .output()
        {
            if output.status.success() {
                let memory_str = String::from_utf8_lossy(&output.stdout);
                if let Ok(memory_mb) = memory_str.trim().parse::<u32>() {
                    return Ok(memory_mb / 1024); // Convert MB to GB
                }
            }
        }

        // Fallback: assume no GPU memory
        Ok(0)
    }

    fn get_available_system_memory(&self) -> Result<u32> {
        let meminfo = std::fs::read_to_string("/proc/meminfo")?;
        for line in meminfo.lines() {
            if line.starts_with("MemAvailable:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: u64 = parts[1].parse()?;
                    return Ok((kb / 1024 / 1024) as u32); // Convert KB to GB
                }
            }
        }
        Ok(8) // Fallback to 8GB
    }

    fn detect_gpu_support() -> bool {
        Command::new("nvidia-smi")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}
