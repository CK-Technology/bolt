use crate::runtime::environment::env_manager;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use tracing::{debug, info, warn};

pub mod amd;
pub mod nvidia;
pub mod velocity;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUManager {
    pub nvidia: Option<nvidia::NvidiaManager>,
    pub amd: Option<amd::AmdManager>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUInfo {
    pub vendor: GPUVendor,
    pub index: u32,
    pub name: String,
    pub memory_mb: u32,
    pub uuid: Option<String>,
    pub device_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GPUVendor {
    NVIDIA,
    AMD,
    Intel,
}

impl GPUManager {
    pub fn new() -> Result<Self> {
        info!("🖥️ Initializing GPU manager");

        let nvidia = nvidia::NvidiaManager::detect().ok();
        let amd = amd::AmdManager::detect().ok();

        if nvidia.is_some() {
            info!("✅ NVIDIA GPU support detected");
        }
        if amd.is_some() {
            info!("✅ AMD GPU support detected");
        }

        Ok(Self { nvidia, amd })
    }

    pub async fn setup_container_gpu_access(
        &self,
        container_id: &str,
        gpu_config: &crate::config::GpuConfig,
    ) -> Result<()> {
        info!("🚀 Setting up GPU access for container: {}", container_id);

        if let Some(ref nvidia_config) = gpu_config.nvidia {
            if let Some(ref nvidia_manager) = self.nvidia {
                nvidia_manager
                    .setup_container_access(container_id, nvidia_config)
                    .await?;
            } else {
                warn!("⚠️ NVIDIA GPU requested but not available");
            }
        }

        if let Some(ref amd_config) = gpu_config.amd {
            if let Some(ref amd_manager) = self.amd {
                amd_manager
                    .setup_container_access(container_id, amd_config)
                    .await?;
            } else {
                warn!("⚠️ AMD GPU requested but not available");
            }
        }

        Ok(())
    }

    pub async fn get_available_gpus(&self) -> Result<Vec<GPUInfo>> {
        let mut gpus = Vec::new();

        if let Some(ref nvidia) = self.nvidia {
            gpus.extend(nvidia.list_gpus().await?);
        }

        if let Some(ref amd) = self.amd {
            gpus.extend(amd.list_gpus().await?);
        }

        Ok(gpus)
    }

    pub async fn run_gpu_workload(&self, container_id: &str, workload: GPUWorkload) -> Result<()> {
        info!("💻 Running GPU workload in container: {}", container_id);

        match workload {
            GPUWorkload::CUDA(cuda_app) => {
                if let Some(ref nvidia) = self.nvidia {
                    nvidia.run_cuda_application(container_id, &cuda_app).await?;
                }
            }
            GPUWorkload::OpenCL(opencl_app) => {
                // Can run on both NVIDIA and AMD
                if let Some(ref nvidia) = self.nvidia {
                    nvidia
                        .run_opencl_application(container_id, &opencl_app)
                        .await?;
                } else if let Some(ref amd) = self.amd {
                    amd.run_opencl_application(container_id, &opencl_app)
                        .await?;
                }
            }
            GPUWorkload::Vulkan(vulkan_app) => {
                // Gaming and compute with Vulkan
                self.run_vulkan_application(container_id, &vulkan_app)
                    .await?;
            }
            GPUWorkload::Gaming(game_config) => {
                self.setup_gaming_gpu(container_id, &game_config).await?;
                // Integrate with Wayland if available using safe environment management
                self.setup_wayland_gaming_integration(container_id, &game_config)
                    .await?;
            }
            GPUWorkload::AI(ai_workload) => {
                info!("🤖 Setting up AI workload: {}", ai_workload.name);
                self.setup_ai_workload(container_id, &ai_workload).await?;
            }
            GPUWorkload::MachineLearning(ml_workload) => {
                info!("🧠 Setting up ML workload: {}", ml_workload.name);
                self.setup_ml_workload(container_id, &ml_workload).await?;
            }
            GPUWorkload::ComputeGeneral(compute_workload) => {
                info!("⚙️ Setting up compute workload: {}", compute_workload.name);
                self.setup_compute_workload(container_id, &compute_workload)
                    .await?;
            }
        }

        Ok(())
    }

    async fn run_vulkan_application(
        &self,
        container_id: &str,
        app: &VulkanApplication,
    ) -> Result<()> {
        info!("🎮 Setting up Vulkan application: {}", app.name);

        // Configure Vulkan drivers for container
        self.mount_vulkan_drivers(container_id).await?;

        // Set environment variables
        unsafe {
            std::env::set_var(
                "VK_ICD_FILENAMES",
                "/usr/share/vulkan/icd.d/nvidia_icd.json",
            );
            std::env::set_var("VK_LAYER_PATH", "/usr/share/vulkan/explicit_layer.d");
        }

        Ok(())
    }

    async fn mount_vulkan_drivers(&self, container_id: &str) -> Result<()> {
        info!("📦 Mounting Vulkan drivers for container: {}", container_id);

        // Mount Vulkan ICD files
        let vulkan_paths = [
            "/usr/share/vulkan",
            "/usr/lib/x86_64-linux-gnu/libvulkan.so.1",
        ];

        for path in &vulkan_paths {
            if Path::new(path).exists() {
                debug!("  Mounting: {}", path);
                // Would bind-mount these paths into container
            }
        }

        Ok(())
    }

    async fn setup_gaming_gpu(&self, container_id: &str, game: &GamingConfig) -> Result<()> {
        info!("🎮 Setting up gaming GPU configuration");

        // Enable GameMode if available
        if game.gamemode_enabled {
            self.enable_gamemode(container_id).await?;
        }

        // Configure for VR if needed
        if game.vr_enabled {
            self.setup_vr_support(container_id).await?;
        }

        // Set up game-specific optimizations
        match game.game_type.as_str() {
            "wine" | "proton" => {
                self.setup_wine_gaming_gpu(container_id, game).await?;
            }
            "native" => {
                self.setup_native_gaming_gpu(container_id).await?;
            }
            _ => {
                info!("  Using default gaming configuration");
            }
        }

        Ok(())
    }

    async fn setup_wine_gaming_gpu(&self, container_id: &str, game: &GamingConfig) -> Result<()> {
        info!("🍷 Configuring GPU for Wine/Proton gaming");

        // Enable DXVK if specified
        if game.dxvk_enabled {
            info!("  ✓ DXVK enabled (DirectX → Vulkan)");
            unsafe {
                std::env::set_var("DXVK_ENABLE_NVAPI", "1");
                std::env::set_var("DXVK_NVAPI_ALLOW_OTHER", "1");
            }
        }

        // Enable VKD3D for DirectX 12
        if game.vkd3d_enabled {
            info!("  ✓ VKD3D enabled (DirectX 12 → Vulkan)");
            unsafe {
                std::env::set_var("VKD3D_CONFIG", "dxr,dxr11");
            }
        }

        // Configure for NVIDIA specific Wine features
        if let Some(ref nvidia) = self.nvidia {
            nvidia.setup_wine_integration(container_id).await?;
        }

        Ok(())
    }

    async fn setup_native_gaming_gpu(&self, container_id: &str) -> Result<()> {
        info!("🎯 Configuring GPU for native gaming");

        // Set up OpenGL/Vulkan for native games
        unsafe {
            std::env::set_var("__GL_THREADED_OPTIMIZATIONS", "1");
            std::env::set_var("__GL_SHADER_CACHE", "1");
        }

        Ok(())
    }

    async fn enable_gamemode(&self, container_id: &str) -> Result<()> {
        info!("⚡ Enabling GameMode for container: {}", container_id);

        // Check if gamemode is available
        if Command::new("gamemoded").arg("--version").output().is_ok() {
            info!("  ✓ GameMode daemon available");
            // Would configure GameMode for this container
        } else {
            warn!("  ⚠️ GameMode not available, skipping");
        }

        Ok(())
    }

    async fn setup_vr_support(&self, container_id: &str) -> Result<()> {
        info!("🥽 Setting up VR support for container: {}", container_id);

        // Mount VR runtime libraries
        let vr_paths = [
            "/usr/lib/steamvr",
            "/usr/lib/openvr",
            "/dev/hidraw*", // VR controllers
        ];

        for path in &vr_paths {
            debug!("  VR path: {}", path);
        }

        Ok(())
    }

    async fn setup_wayland_gaming_integration(
        &self,
        container_id: &str,
        game_config: &GamingConfig,
    ) -> Result<()> {
        // Check if Wayland is available
        if std::env::var("WAYLAND_DISPLAY").is_ok()
            || std::env::var("XDG_SESSION_TYPE").unwrap_or_default() == "wayland"
        {
            info!(
                "🌊 Setting up Wayland gaming integration for {}",
                container_id
            );

            // Setup Wayland-specific GPU optimizations
            self.configure_wayland_gpu_environment(container_id).await?;

            // Detect and optimize for specific desktop environments
            if Self::is_kde_session() {
                info!("  🔷 KDE/Plasma detected - applying specific optimizations");
                self.setup_kde_wayland_gaming(container_id, game_config)
                    .await?;
            } else if Self::is_gnome_session() {
                info!("  🔵 GNOME detected - applying Mutter optimizations");
                self.setup_gnome_wayland_gaming(container_id).await?;
            } else {
                info!("  🌊 Generic Wayland compositor - applying standard optimizations");
                self.setup_generic_wayland_gaming(container_id).await?;
            }
        } else {
            info!("  🎲 X11 session detected - using traditional GPU setup");
        }

        Ok(())
    }

    async fn configure_wayland_gpu_environment(&self, container_id: &str) -> Result<()> {
        // Use safe environment management instead of unsafe operations
        let wayland_display =
            std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".to_string());
        let desktop_env = if Self::is_kde_session() {
            "kde"
        } else if Self::is_gnome_session() {
            "gnome"
        } else {
            "generic"
        };

        env_manager().configure_gaming_environment(container_id, desktop_env, &wayland_display)?;
        Ok(())
    }

    fn is_kde_session() -> bool {
        std::env::var("KDE_SESSION_VERSION").is_ok()
            || std::env::var("KDE_FULL_SESSION").is_ok()
            || std::env::var("XDG_CURRENT_DESKTOP")
                .unwrap_or_default()
                .contains("KDE")
    }

    fn is_gnome_session() -> bool {
        std::env::var("GNOME_DESKTOP_SESSION_ID").is_ok()
            || std::env::var("XDG_CURRENT_DESKTOP")
                .unwrap_or_default()
                .contains("GNOME")
    }

    async fn setup_kde_wayland_gaming(
        &self,
        container_id: &str,
        _game_config: &GamingConfig,
    ) -> Result<()> {
        info!("    🔷 Configuring KDE/Plasma Wayland gaming optimizations");

        // Use safe environment management - configurations are already applied in configure_gaming_environment
        info!("      ✅ KDE/Plasma gaming optimizations applied via safe environment manager");
        Ok(())
    }

    async fn setup_gnome_wayland_gaming(&self, _container_id: &str) -> Result<()> {
        info!("    🔵 Configuring GNOME/Mutter Wayland gaming optimizations");
        // Use safe environment management - configurations are already applied in configure_gaming_environment
        info!("      ✅ GNOME/Mutter gaming optimizations applied via safe environment manager");
        Ok(())
    }

    async fn setup_generic_wayland_gaming(&self, _container_id: &str) -> Result<()> {
        info!("    🌊 Configuring generic Wayland gaming optimizations");
        // Use safe environment management - configurations are already applied in configure_gaming_environment
        info!("      ✅ Generic Wayland gaming optimizations applied via safe environment manager");
        Ok(())
    }

    // AI/ML Workload Setup Methods
    async fn setup_ai_workload(&self, container_id: &str, ai_workload: &AIWorkload) -> Result<()> {
        info!(
            "🤖 Configuring AI workload: {} ({})",
            ai_workload.name,
            match &ai_workload.ai_backend {
                AIBackend::Ollama => "Ollama",
                AIBackend::LocalAI => "LocalAI",
                AIBackend::VLLM => "vLLM",
                AIBackend::HuggingFace => "HuggingFace",
                AIBackend::OpenAI => "OpenAI API",
                AIBackend::Custom(name) => name,
            }
        );

        // Configure AI-specific environment
        let backend_name = match &ai_workload.ai_backend {
            AIBackend::Ollama => "ollama",
            AIBackend::LocalAI => "localai",
            AIBackend::VLLM => "vllm",
            AIBackend::HuggingFace => "transformers",
            AIBackend::OpenAI => "openai",
            AIBackend::Custom(name) => name,
        };

        env_manager().configure_ai_environment(container_id, backend_name)?;

        // Setup GPU access for AI workload
        if let Some(ref nvidia) = self.nvidia {
            nvidia
                .setup_ai_gpu_access(container_id, ai_workload)
                .await?;
        }

        if let Some(ref amd) = self.amd {
            amd.setup_ai_gpu_access(container_id, ai_workload).await?;
        }

        info!("  ✅ AI workload environment configured");
        info!("    • Model: {}", ai_workload.model_name);
        info!("    • Context Length: {:?}", ai_workload.context_length);
        info!("    • Quantization: {:?}", ai_workload.quantization);
        info!(
            "    • Flash Attention: {}",
            ai_workload.enable_flash_attention
        );
        info!("    • Multi-GPU: {}", ai_workload.multi_gpu);

        Ok(())
    }

    async fn setup_ml_workload(&self, container_id: &str, ml_workload: &MLWorkload) -> Result<()> {
        info!(
            "🧠 Configuring ML workload: {} ({})",
            ml_workload.name,
            match &ml_workload.ml_framework {
                MLFramework::PyTorch => "PyTorch",
                MLFramework::TensorFlow => "TensorFlow",
                MLFramework::JAX => "JAX",
                MLFramework::Flax => "Flax",
                MLFramework::MLX => "MLX",
                MLFramework::Custom(name) => name,
            }
        );

        // Configure framework-specific environment
        let framework_name = match &ml_workload.ml_framework {
            MLFramework::PyTorch => "pytorch",
            MLFramework::TensorFlow => "tensorflow",
            MLFramework::JAX => "jax",
            MLFramework::Flax => "flax",
            MLFramework::MLX => "mlx",
            MLFramework::Custom(name) => name,
        };

        env_manager().configure_ai_environment(container_id, framework_name)?;

        // Setup GPU access for ML workload
        if let Some(ref nvidia) = self.nvidia {
            nvidia
                .setup_ml_gpu_access(container_id, ml_workload)
                .await?;
        }

        if let Some(ref amd) = self.amd {
            amd.setup_ml_gpu_access(container_id, ml_workload).await?;
        }

        info!("  ✅ ML workload environment configured");
        info!("    • Framework: {:?}", ml_workload.ml_framework);
        info!("    • Model Type: {}", ml_workload.model_type);
        info!("    • Training Mode: {}", ml_workload.training_mode);
        info!("    • Mixed Precision: {}", ml_workload.mixed_precision);
        info!("    • Distributed: {}", ml_workload.distributed_training);

        Ok(())
    }

    async fn setup_compute_workload(
        &self,
        container_id: &str,
        compute_workload: &ComputeWorkload,
    ) -> Result<()> {
        info!(
            "⚙️ Configuring compute workload: {} ({:?})",
            compute_workload.name, compute_workload.compute_type
        );

        // Setup GPU access for compute workload
        if let Some(ref nvidia) = self.nvidia {
            nvidia
                .setup_compute_gpu_access(container_id, compute_workload)
                .await?;
        }

        if let Some(ref amd) = self.amd {
            amd.setup_compute_gpu_access(container_id, compute_workload)
                .await?;
        }

        info!("  ✅ Compute workload environment configured");
        info!("    • Compute Type: {:?}", compute_workload.compute_type);
        info!("    • Precision: {:?}", compute_workload.precision);
        info!("    • CPU/GPU Ratio: {:.1}", compute_workload.cpu_gpu_ratio);
        info!(
            "    • Memory Requirements: {:?} GB",
            compute_workload.memory_requirements_gb
        );
        info!(
            "    • P2P Enabled: {}",
            compute_workload.enable_peer_to_peer
        );

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GPUWorkload {
    CUDA(CudaApplication),
    OpenCL(OpenCLApplication),
    Vulkan(VulkanApplication),
    Gaming(GamingConfig),
    AI(AIWorkload),
    MachineLearning(MLWorkload),
    ComputeGeneral(ComputeWorkload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CudaApplication {
    pub name: String,
    pub compute_capability: Option<String>,
    pub memory_gb: Option<u32>,
    pub multi_gpu: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCLApplication {
    pub name: String,
    pub platform: String,
    pub device_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulkanApplication {
    pub name: String,
    pub api_version: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingConfig {
    pub game_type: String, // "wine", "proton", "native"
    pub dxvk_enabled: bool,
    pub vkd3d_enabled: bool,
    pub gamemode_enabled: bool,
    pub vr_enabled: bool,
    pub performance_profile: String, // "power_save", "balanced", "performance"
}

impl Default for GamingConfig {
    fn default() -> Self {
        Self {
            game_type: "native".to_string(),
            dxvk_enabled: false,
            vkd3d_enabled: false,
            gamemode_enabled: true,
            vr_enabled: false,
            performance_profile: "performance".to_string(),
        }
    }
}

/// AI workload configuration for LLMs and inference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIWorkload {
    pub name: String,
    pub ai_backend: AIBackend,
    pub model_path: Option<String>,
    pub model_name: String,
    pub memory_gb: Option<u32>,
    pub context_length: Option<u32>,
    pub batch_size: Option<u32>,
    pub quantization: Option<String>, // "fp16", "int8", "int4"
    pub multi_gpu: bool,
    pub enable_flash_attention: bool,
    pub enable_kv_cache: bool,
}

/// Machine Learning training workload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLWorkload {
    pub name: String,
    pub ml_framework: MLFramework,
    pub model_type: String,  // "transformer", "cnn", "rnn", etc.
    pub training_mode: bool, // true for training, false for inference
    pub dataset_path: Option<String>,
    pub checkpoint_path: Option<String>,
    pub distributed_training: bool,
    pub mixed_precision: bool,
    pub gradient_accumulation_steps: Option<u32>,
}

/// General compute workload for scientific computing, crypto, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeWorkload {
    pub name: String,
    pub compute_type: ComputeType,
    pub memory_requirements_gb: Option<u32>,
    pub cpu_gpu_ratio: f32, // 0.0 = pure GPU, 1.0 = pure CPU
    pub precision: ComputePrecision,
    pub enable_peer_to_peer: bool, // For multi-GPU communication
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AIBackend {
    Ollama,
    LocalAI,
    VLLM,
    HuggingFace,
    OpenAI, // For API compatibility
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MLFramework {
    PyTorch,
    TensorFlow,
    JAX,
    Flax,
    MLX, // Apple's framework
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComputeType {
    Cryptocurrency,
    Scientific,
    Rendering,
    VideoProcessing,
    AudioProcessing,
    DataAnalysis,
    Simulation,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComputePrecision {
    Float32,
    Float16,
    BFloat16,
    Int32,
    Int16,
    Int8,
    Custom(String),
}

impl Default for AIWorkload {
    fn default() -> Self {
        Self {
            name: "ai-inference".to_string(),
            ai_backend: AIBackend::Ollama,
            model_path: None,
            model_name: "llama2".to_string(),
            memory_gb: None,
            context_length: Some(4096),
            batch_size: Some(1),
            quantization: Some("fp16".to_string()),
            multi_gpu: false,
            enable_flash_attention: true,
            enable_kv_cache: true,
        }
    }
}

impl Default for MLWorkload {
    fn default() -> Self {
        Self {
            name: "ml-training".to_string(),
            ml_framework: MLFramework::PyTorch,
            model_type: "transformer".to_string(),
            training_mode: false,
            dataset_path: None,
            checkpoint_path: None,
            distributed_training: false,
            mixed_precision: true,
            gradient_accumulation_steps: Some(8),
        }
    }
}

impl Default for ComputeWorkload {
    fn default() -> Self {
        Self {
            name: "compute-task".to_string(),
            compute_type: ComputeType::Scientific,
            memory_requirements_gb: None,
            cpu_gpu_ratio: 0.2,
            precision: ComputePrecision::Float32,
            enable_peer_to_peer: false,
        }
    }
}
