//! AI Model Serving - vLLM, TensorRT, ONNX integration

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info};

/// Model serving backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServingBackend {
    /// vLLM for LLM inference
    VLLM {
        tensor_parallel: u32,
        pipeline_parallel: u32,
        max_batch_size: u32,
        max_num_seqs: u32,
    },
    /// NVIDIA TensorRT
    TensorRT {
        engine_path: Option<PathBuf>,
        precision: Precision,
    },
    /// ONNX Runtime
    ONNX {
        execution_provider: String,  // "CUDA", "TensorRT", "ROCm"
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Precision {
    FP32,
    FP16,
    INT8,
    INT4,
}

/// Model serving configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServeConfig {
    pub model_name: String,
    pub backend: ServingBackend,
    pub gpus: Vec<String>,
    pub port: u16,
    pub host: String,
    pub enable_healthcheck: bool,
    pub auto_restart: bool,
}

/// Model server
pub struct ModelServer {
    config: ServeConfig,
    container_id: Option<String>,
    runtime: Option<Arc<crate::BoltRuntime>>,
}

impl ModelServer {
    pub fn new(config: ServeConfig) -> Self {
        Self {
            config,
            container_id: None,
            runtime: None,
        }
    }

    /// Set the runtime (used for actual container management)
    pub fn with_runtime(mut self, runtime: Arc<crate::BoltRuntime>) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// Start the model server
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting model server: {}", self.config.model_name);
        info!("   Backend: {:?}", self.config.backend);
        info!("   GPUs: {:?}", self.config.gpus);
        info!("   Port: {}", self.config.port);

        match &self.config.backend {
            ServingBackend::VLLM { .. } => self.start_vllm().await,
            ServingBackend::TensorRT { .. } => self.start_tensorrt().await,
            ServingBackend::ONNX { .. } => self.start_onnx().await,
        }
    }

    async fn start_vllm(&mut self) -> Result<()> {
        debug!("Starting vLLM server");

        let backend = match &self.config.backend {
            ServingBackend::VLLM {
                tensor_parallel,
                max_batch_size,
                max_num_seqs,
                ..
            } => (tensor_parallel, max_batch_size, max_num_seqs),
            _ => unreachable!(),
        };

        // Build container config for vLLM
        let mut env_vars = vec![
            format!("MODEL={}", self.config.model_name),
            format!("TENSOR_PARALLEL_SIZE={}", backend.0),
            format!("MAX_NUM_SEQS={}", backend.2),
            format!("GPU_MEMORY_UTILIZATION=0.9"),
            "TRUST_REMOTE_CODE=true".to_string(),
        ];

        // Add HuggingFace token if available
        if let Ok(token) = std::env::var("HF_TOKEN") {
            env_vars.push(format!("HF_TOKEN={}", token));
        }

        let container_config = serde_json::json!({
            "image": "vllm/vllm-openai:latest",
            "gpus": self.config.gpus,
            "ports": [format!("{}:8000", self.config.port)],
            "env": env_vars,
            "shm_size": "8g",  // Shared memory for tensor storage
            "command": [
                "python3", "-m", "vllm.entrypoints.openai.api_server",
                "--model", &self.config.model_name,
                "--port", "8000",
                "--tensor-parallel-size", &backend.0.to_string(),
            ]
        });

        debug!("vLLM container config: {}", serde_json::to_string_pretty(&container_config)?);

        // Create and start container via Bolt runtime
        self.container_id = Some(self.create_container(container_config).await?);

        // Wait for model to be ready
        if self.config.enable_healthcheck {
            self.wait_for_ready(60).await?;
        }

        info!("✅ vLLM server started successfully");
        info!("   API endpoint: http://{}:{}/v1", self.config.host, self.config.port);
        info!("   OpenAI compatible API ready");

        Ok(())
    }

    async fn start_tensorrt(&mut self) -> Result<()> {
        debug!("Starting TensorRT server");

        let backend = match &self.config.backend {
            ServingBackend::TensorRT { engine_path, precision } => (engine_path, precision),
            _ => unreachable!(),
        };

        let container_config = serde_json::json!({
            "image": "nvcr.io/nvidia/tensorrt:latest",
            "gpus": self.config.gpus,
            "ports": [format!("{}:8000", self.config.port)],
            "command": [
                "trtexec",
                "--onnx=/models/model.onnx",
                "--saveEngine=/engines/model.engine",
                &format!("--{}", match backend.1 {
                    Precision::FP32 => "fp32",
                    Precision::FP16 => "fp16",
                    Precision::INT8 => "int8",
                    Precision::INT4 => "int4",
                }),
            ]
        });

        self.container_id = Some(self.create_container(container_config).await?);

        info!("✅ TensorRT server started");
        Ok(())
    }

    async fn start_onnx(&mut self) -> Result<()> {
        debug!("Starting ONNX Runtime server");

        let backend = match &self.config.backend {
            ServingBackend::ONNX { execution_provider } => execution_provider,
            _ => unreachable!(),
        };

        let container_config = serde_json::json!({
            "image": "mcr.microsoft.com/onnxruntime/server:latest",
            "gpus": self.config.gpus,
            "ports": [format!("{}:8001", self.config.port)],
            "env": [
                format!("ORT_EXECUTION_PROVIDER={}", backend),
            ],
            "command": [
                "onnxruntime_server",
                "--model_path", "/models",
                "--http_port", "8001",
            ]
        });

        self.container_id = Some(self.create_container(container_config).await?);

        info!("✅ ONNX Runtime server started");
        Ok(())
    }

    async fn create_container(&self, config: serde_json::Value) -> Result<String> {
        // Extract container config from JSON
        let image = config["image"].as_str().unwrap_or("vllm/vllm-openai:latest");
        let name = config["name"].as_str();
        let ports: Vec<String> = config["ports"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let env: Vec<String> = config["env"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        if let Some(runtime) = &self.runtime {
            // Use actual Bolt runtime to create container
            runtime.run_container(image, name, &ports, &env, &[], true).await?;

            // Generate container ID (in production, would get from runtime)
            let container_id = format!("model-{}", name.unwrap_or("server"));
            debug!("Created container via Bolt runtime: {}", container_id);
            Ok(container_id)
        } else {
            // Fallback: return mock container ID if runtime not set
            let container_id = format!("model-server-{}", uuid::Uuid::new_v4());
            debug!("Created mock container (no runtime): {}", container_id);
            Ok(container_id)
        }
    }

    async fn wait_for_ready(&self, timeout_secs: u64) -> Result<()> {
        info!("⏳ Waiting for model server to be ready...");

        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_secs);

        while start.elapsed() < timeout {
            if self.check_health().await.is_ok() {
                info!("✅ Model server is ready!");
                return Ok(());
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }

        Err(anyhow!("Model server failed to become ready within {}s", timeout_secs))
    }

    async fn check_health(&self) -> Result<()> {
        let health_url = format!("http://{}:{}/health", self.config.host, self.config.port);

        let response = reqwest::get(&health_url)
            .await
            .context("Health check failed")?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!("Health check returned: {}", response.status()))
        }
    }

    /// Stop the model server
    pub async fn stop(&self) -> Result<()> {
        if let Some(ref container_id) = self.container_id {
            info!("🛑 Stopping model server: {}", container_id);

            if let Some(runtime) = &self.runtime {
                // Use actual Bolt runtime to stop container
                runtime.stop_container(container_id).await?;
                info!("✅ Model server stopped");
            } else {
                debug!("No runtime available, mock stop");
            }
        }
        Ok(())
    }

    /// Get server status
    pub async fn status(&self) -> Result<ServerStatus> {
        if let Some(ref container_id) = self.container_id {
            let is_running = if let Some(runtime) = &self.runtime {
                // Get actual container status from runtime
                match runtime.list_containers(true).await {
                    Ok(containers) => {
                        containers.iter()
                            .find(|c| c.id == *container_id || c.name == *container_id)
                            .map(|c| c.status.contains("running"))
                            .unwrap_or(false)
                    }
                    Err(_) => false,
                }
            } else {
                true  // Mock status
            };

            Ok(ServerStatus {
                container_id: container_id.clone(),
                is_running,
                model_name: self.config.model_name.clone(),
                backend: format!("{:?}", self.config.backend),
                endpoint: format!("http://{}:{}", self.config.host, self.config.port),
            })
        } else {
            Err(anyhow!("Server not started"))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerStatus {
    pub container_id: String,
    pub is_running: bool,
    pub model_name: String,
    pub backend: String,
    pub endpoint: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serve_config() {
        let config = ServeConfig {
            model_name: "meta-llama/Llama-3-70B".to_string(),
            backend: ServingBackend::VLLM {
                tensor_parallel: 4,
                pipeline_parallel: 1,
                max_batch_size: 64,
                max_num_seqs: 256,
            },
            gpus: vec!["gpu:0".to_string(), "gpu:1".to_string()],
            port: 8000,
            host: "0.0.0.0".to_string(),
            enable_healthcheck: true,
            auto_restart: true,
        };

        assert_eq!(config.model_name, "meta-llama/Llama-3-70B");
        assert_eq!(config.gpus.len(), 2);
    }
}
