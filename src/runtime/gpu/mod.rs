use anyhow::{Result, Context};
use tracing::{info, warn, debug, error};
use std::path::Path;
use std::process::Command;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod nvidia;
pub mod amd;

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
                nvidia_manager.setup_container_access(container_id, nvidia_config).await?;
            } else {
                warn!("⚠️ NVIDIA GPU requested but not available");
            }
        }

        if let Some(ref amd_config) = gpu_config.amd {
            if let Some(ref amd_manager) = self.amd {
                amd_manager.setup_container_access(container_id, amd_config).await?;
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

    pub async fn run_gpu_workload(
        &self,
        container_id: &str,
        workload: GPUWorkload,
    ) -> Result<()> {
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
                    nvidia.run_opencl_application(container_id, &opencl_app).await?;
                } else if let Some(ref amd) = self.amd {
                    amd.run_opencl_application(container_id, &opencl_app).await?;
                }
            }
            GPUWorkload::Vulkan(vulkan_app) => {
                // Gaming and compute with Vulkan
                self.run_vulkan_application(container_id, &vulkan_app).await?;
            }
            GPUWorkload::Gaming(game_config) => {
                self.setup_gaming_gpu(container_id, &game_config).await?;
            }
        }

        Ok(())
    }

    async fn run_vulkan_application(&self, container_id: &str, app: &VulkanApplication) -> Result<()> {
        info!("🎮 Setting up Vulkan application: {}", app.name);

        // Configure Vulkan drivers for container
        self.mount_vulkan_drivers(container_id).await?;

        // Set environment variables
        unsafe {
            std::env::set_var("VK_ICD_FILENAMES", "/usr/share/vulkan/icd.d/nvidia_icd.json");
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
            unsafe { std::env::set_var("VKD3D_CONFIG", "dxr,dxr11"); }
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GPUWorkload {
    CUDA(CudaApplication),
    OpenCL(OpenCLApplication),
    Vulkan(VulkanApplication),
    Gaming(GamingConfig),
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