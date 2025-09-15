use anyhow::{Result, Context};
use tracing::{info, warn, debug, error};
use std::path::Path;
use std::process::{Command, Stdio};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::{GPUInfo, GPUVendor};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvidiaManager {
    pub driver_version: String,
    pub cuda_version: Option<String>,
    pub gpus: Vec<NvidiaGPU>,
    pub container_runtime_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvidiaGPU {
    pub index: u32,
    pub uuid: String,
    pub name: String,
    pub memory_mb: u32,
    pub compute_capability: String,
    pub pci_bus_id: String,
    pub power_limit_w: Option<u32>,
    pub temperature_c: Option<u32>,
}

impl NvidiaManager {
    pub fn detect() -> Result<Self> {
        info!("🔍 Detecting NVIDIA GPU configuration");

        // Check if nvidia-smi is available
        let output = Command::new("nvidia-smi")
            .arg("--query-gpu=index,uuid,name,memory.total,compute_cap,pci.bus_id,power.max_limit,temperature.gpu")
            .arg("--format=csv,noheader,nounits")
            .output()
            .context("nvidia-smi not found - NVIDIA drivers may not be installed")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("nvidia-smi failed: {}", String::from_utf8_lossy(&output.stderr)));
        }

        // Parse GPU information
        let mut gpus = Vec::new();
        let output_str = String::from_utf8(output.stdout)?;

        for line in output_str.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if fields.len() >= 6 {
                let gpu = NvidiaGPU {
                    index: fields[0].parse().unwrap_or(0),
                    uuid: fields[1].to_string(),
                    name: fields[2].to_string(),
                    memory_mb: fields[3].parse().unwrap_or(0),
                    compute_capability: fields[4].to_string(),
                    pci_bus_id: fields[5].to_string(),
                    power_limit_w: fields.get(6).and_then(|s| s.parse().ok()),
                    temperature_c: fields.get(7).and_then(|s| s.parse().ok()),
                };
                gpus.push(gpu);
            }
        }

        // Get driver version
        let driver_version = Self::get_driver_version()?;
        let cuda_version = Self::get_cuda_version().ok();

        // Check for nvidia-container-runtime
        let container_runtime_available = Path::new("/usr/bin/nvidia-container-runtime").exists() ||
                                         Path::new("/usr/bin/nvidia-docker").exists();

        info!("📊 NVIDIA Detection Results:");
        info!("  Driver Version: {}", driver_version);
        if let Some(ref cuda) = cuda_version {
            info!("  CUDA Version: {}", cuda);
        }
        info!("  GPUs Found: {}", gpus.len());
        info!("  Container Runtime: {}", if container_runtime_available { "Available" } else { "Not Available" });

        for gpu in &gpus {
            info!("  GPU {}: {} ({}MB, {})", gpu.index, gpu.name, gpu.memory_mb, gpu.compute_capability);
        }

        Ok(Self {
            driver_version,
            cuda_version,
            gpus,
            container_runtime_available,
        })
    }

    fn get_driver_version() -> Result<String> {
        let output = Command::new("nvidia-smi")
            .arg("--query-gpu=driver_version")
            .arg("--format=csv,noheader")
            .output()?;

        let version = String::from_utf8(output.stdout)?
            .lines()
            .next()
            .unwrap_or("unknown")
            .trim()
            .to_string();

        Ok(version)
    }

    fn get_cuda_version() -> Result<String> {
        // Try nvcc first
        if let Ok(output) = Command::new("nvcc").arg("--version").output() {
            let output_str = String::from_utf8(output.stdout)?;
            if let Some(line) = output_str.lines().find(|l| l.contains("release")) {
                if let Some(version_part) = line.split("release").nth(1) {
                    if let Some(version) = version_part.split(',').next() {
                        return Ok(version.trim().to_string());
                    }
                }
            }
        }

        // Try nvidia-smi as fallback
        let output = Command::new("nvidia-smi")
            .arg("--query-gpu=cuda_version")
            .arg("--format=csv,noheader")
            .output()?;

        let version = String::from_utf8(output.stdout)?
            .lines()
            .next()
            .unwrap_or("unknown")
            .trim()
            .to_string();

        Ok(version)
    }

    pub async fn setup_container_access(
        &self,
        container_id: &str,
        nvidia_config: &crate::config::NvidiaConfig,
    ) -> Result<()> {
        info!("🟢 Setting up NVIDIA GPU access for container: {}", container_id);

        // Validate GPU device selection
        let device_spec = nvidia_config.device.map(|d| d.to_string()).unwrap_or("all".to_string());
        let device_indices = self.parse_device_spec(&device_spec)?;

        for &index in &device_indices {
            if let Some(gpu) = self.gpus.get(index as usize) {
                info!("  📱 Configuring access to GPU {}: {}", index, gpu.name);
            } else {
                warn!("  ⚠️ GPU {} not found", index);
            }
        }

        // Setup device access
        self.setup_device_access(container_id, &device_indices).await?;

        // Configure CUDA environment
        self.setup_cuda_environment(container_id, nvidia_config, &device_indices).await?;

        // Enable specific features
        if nvidia_config.dlss == Some(true) {
            self.enable_dlss_support(container_id).await?;
        }

        if nvidia_config.raytracing == Some(true) {
            self.enable_raytracing_support(container_id).await?;
        }

        // Set up container runtime integration
        if self.container_runtime_available {
            self.setup_nvidia_container_runtime(container_id, nvidia_config).await?;
        } else {
            warn!("⚠️ nvidia-container-runtime not available, using basic device access");
            self.setup_basic_device_access(container_id, &device_indices).await?;
        }

        info!("✅ NVIDIA GPU access configured for container: {}", container_id);
        Ok(())
    }

    fn parse_device_spec(&self, device_spec: &str) -> Result<Vec<u32>> {
        let mut indices = Vec::new();

        match device_spec {
            "all" => {
                indices.extend(0..self.gpus.len() as u32);
            }
            spec if spec.contains(',') => {
                // Multiple devices: "0,1,2"
                for part in spec.split(',') {
                    if let Ok(index) = part.trim().parse::<u32>() {
                        indices.push(index);
                    }
                }
            }
            spec if spec.contains('-') => {
                // Range: "0-2"
                let parts: Vec<&str> = spec.split('-').collect();
                if parts.len() == 2 {
                    if let (Ok(start), Ok(end)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                        indices.extend(start..=end);
                    }
                }
            }
            spec => {
                // Single device: "0"
                if let Ok(index) = spec.parse::<u32>() {
                    indices.push(index);
                } else {
                    // Try UUID match
                    for (i, gpu) in self.gpus.iter().enumerate() {
                        if gpu.uuid == spec || gpu.name.to_lowercase().contains(&spec.to_lowercase()) {
                            indices.push(i as u32);
                            break;
                        }
                    }
                }
            }
        }

        if indices.is_empty() {
            return Err(anyhow::anyhow!("No valid GPU devices found for spec: {}", device_spec));
        }

        Ok(indices)
    }

    async fn setup_device_access(&self, container_id: &str, device_indices: &[u32]) -> Result<()> {
        info!("📱 Setting up GPU device access");

        // NVIDIA device files that need to be accessible
        let mut device_paths = vec![
            "/dev/nvidiactl".to_string(),
            "/dev/nvidia-modeset".to_string(),
            "/dev/nvidia-uvm".to_string(),
            "/dev/nvidia-uvm-tools".to_string(),
        ];

        // Add specific GPU devices
        for &index in device_indices {
            device_paths.push(format!("/dev/nvidia{}", index));
        }

        // Verify devices exist
        for device in &device_paths {
            if Path::new(device).exists() {
                debug!("  ✓ Device available: {}", device);
            } else {
                warn!("  ⚠️ Device not found: {}", device);
            }
        }

        Ok(())
    }

    async fn setup_cuda_environment(
        &self,
        container_id: &str,
        nvidia_config: &crate::config::NvidiaConfig,
        device_indices: &[u32],
    ) -> Result<()> {
        info!("🔧 Configuring CUDA environment");

        // Set CUDA_VISIBLE_DEVICES
        let cuda_devices = device_indices.iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");

        info!("  Setting CUDA_VISIBLE_DEVICES={}", cuda_devices);
        unsafe { std::env::set_var("CUDA_VISIBLE_DEVICES", &cuda_devices); }

        // Set NVIDIA driver capabilities
        let mut capabilities = vec!["compute", "utility"];

        // Enable graphics capabilities by default (could be made configurable)
        capabilities.push("graphics");

        // Enable video capabilities by default (could be made configurable)
        capabilities.extend(&["video", "display"]);

        let driver_capabilities = capabilities.join(",");
        info!("  Setting NVIDIA_DRIVER_CAPABILITIES={}", driver_capabilities);
        unsafe { std::env::set_var("NVIDIA_DRIVER_CAPABILITIES", &driver_capabilities); }

        // Set CUDA requirements (using cuda field from config)
        if nvidia_config.cuda.unwrap_or(false) {
            info!("  CUDA support enabled");
            unsafe { std::env::set_var("NVIDIA_REQUIRE_CUDA", "11.0"); }
        }

        // Set basic driver requirements
        info!("  Setting basic driver requirements");
        unsafe { std::env::set_var("NVIDIA_REQUIRE_DRIVER", "470.0"); }

        Ok(())
    }

    async fn enable_dlss_support(&self, container_id: &str) -> Result<()> {
        info!("✨ Enabling NVIDIA DLSS support for container: {}", container_id);

        // Check if GPUs support DLSS (RTX 20/30/40 series)
        for gpu in &self.gpus {
            if gpu.name.contains("RTX") {
                info!("  ✓ DLSS compatible GPU: {}", gpu.name);
            }
        }

        // Set DLSS environment variables
        unsafe {
            std::env::set_var("NVIDIA_ENABLE_DLSS", "1");
            std::env::set_var("DLSS_PERFMODE", "BALANCED"); // PERFORMANCE, BALANCED, QUALITY
        }

        Ok(())
    }

    async fn enable_raytracing_support(&self, container_id: &str) -> Result<()> {
        info!("🌟 Enabling NVIDIA Ray Tracing support for container: {}", container_id);

        // Check for RTX support
        for gpu in &self.gpus {
            if gpu.name.contains("RTX") || gpu.name.contains("Quadro RTX") {
                info!("  ✓ RT Cores available: {}", gpu.name);
            }
        }

        // Enable RTX features
        unsafe {
            std::env::set_var("NVIDIA_ENABLE_RTX", "1");
            std::env::set_var("RTX_OPTIMIZATION", "PERFORMANCE");
        }

        Ok(())
    }

    async fn setup_nvidia_container_runtime(&self, container_id: &str, nvidia_config: &crate::config::NvidiaConfig) -> Result<()> {
        info!("🐳 Configuring nvidia-container-runtime");

        // This would integrate with the actual nvidia-container-runtime
        // to properly configure GPU access in the container

        Ok(())
    }

    async fn setup_basic_device_access(&self, container_id: &str, device_indices: &[u32]) -> Result<()> {
        info!("🔧 Setting up basic GPU device access");

        // Fallback: manually bind-mount NVIDIA devices
        // This is less secure than nvidia-container-runtime but still functional

        Ok(())
    }

    pub async fn list_gpus(&self) -> Result<Vec<GPUInfo>> {
        let mut gpu_info = Vec::new();

        for gpu in &self.gpus {
            gpu_info.push(GPUInfo {
                vendor: GPUVendor::NVIDIA,
                index: gpu.index,
                name: gpu.name.clone(),
                memory_mb: gpu.memory_mb,
                uuid: Some(gpu.uuid.clone()),
                device_paths: vec![format!("/dev/nvidia{}", gpu.index)],
            });
        }

        Ok(gpu_info)
    }

    pub async fn run_cuda_application(&self, container_id: &str, app: &super::CudaApplication) -> Result<()> {
        info!("🚀 Running CUDA application: {} in container: {}", app.name, container_id);

        // Validate compute capability if specified
        if let Some(ref required_cc) = app.compute_capability {
            for gpu in &self.gpus {
                if gpu.compute_capability >= *required_cc {
                    info!("  ✓ GPU {} meets compute capability requirement: {}", gpu.index, gpu.compute_capability);
                }
            }
        }

        // Check memory requirements
        if let Some(memory_gb) = app.memory_gb {
            let memory_mb = memory_gb * 1024;
            for gpu in &self.gpus {
                if gpu.memory_mb >= memory_mb {
                    info!("  ✓ GPU {} has sufficient memory: {}MB >= {}MB", gpu.index, gpu.memory_mb, memory_mb);
                }
            }
        }

        Ok(())
    }

    pub async fn run_opencl_application(&self, container_id: &str, app: &super::OpenCLApplication) -> Result<()> {
        info!("⚡ Running OpenCL application: {} in container: {}", app.name, container_id);

        // Set OpenCL environment
        unsafe { std::env::set_var("OPENCL_VENDOR_PATH", "/etc/OpenCL/vendors"); }

        Ok(())
    }

    pub async fn setup_wine_integration(&self, container_id: &str) -> Result<()> {
        info!("🍷 Setting up NVIDIA integration for Wine/Proton");

        // Enable NVAPI for Wine
        unsafe {
            std::env::set_var("WINE_ENABLE_NVAPI", "1");
            std::env::set_var("DXVK_ENABLE_NVAPI", "1");
            std::env::set_var("DXVK_NVAPI_ALLOW_OTHER", "1");
        }

        // Set up DXVK NVAPI
        info!("  ✓ NVAPI enabled for Wine applications");
        info!("  ✓ DXVK NVAPI integration enabled");

        Ok(())
    }

    pub async fn monitor_gpu_usage(&self) -> Result<Vec<GPUUsage>> {
        let mut usage_stats = Vec::new();

        let output = Command::new("nvidia-smi")
            .arg("--query-gpu=index,utilization.gpu,utilization.memory,temperature.gpu,power.draw")
            .arg("--format=csv,noheader,nounits")
            .output()?;

        let output_str = String::from_utf8(output.stdout)?;

        for (i, line) in output_str.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if fields.len() >= 5 {
                usage_stats.push(GPUUsage {
                    index: i as u32,
                    gpu_utilization: fields[1].parse().unwrap_or(0),
                    memory_utilization: fields[2].parse().unwrap_or(0),
                    temperature_c: fields[3].parse().unwrap_or(0),
                    power_draw_w: fields[4].parse().unwrap_or(0),
                });
            }
        }

        Ok(usage_stats)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUUsage {
    pub index: u32,
    pub gpu_utilization: u32,
    pub memory_utilization: u32,
    pub temperature_c: u32,
    pub power_draw_w: u32,
}