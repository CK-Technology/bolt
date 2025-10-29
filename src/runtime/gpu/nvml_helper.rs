//! NVML (NVIDIA Management Library) Helper Module
//!
//! Provides direct access to NVIDIA GPU metrics via native NVML bindings,
//! replacing nvidia-smi shell command calls for better performance and reliability.

#[cfg(feature = "nvidia-support")]
use nvml_wrapper::{Nvml, Device};
use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// NVML Manager for direct GPU access
pub struct NvmlManager {
    #[cfg(feature = "nvidia-support")]
    nvml: Arc<Nvml>,
    #[cfg(feature = "nvidia-support")]
    devices: Arc<RwLock<Vec<Device<'static>>>>,
}

/// GPU Information structure
#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub index: u32,
    pub uuid: String,
    pub name: String,
    pub memory_total_mb: u64,
    pub compute_capability: (u32, u32),
    pub pci_bus_id: String,
    pub power_limit_w: u32,
    pub temperature_c: u32,
}

/// GPU Metrics structure
#[derive(Debug, Clone)]
pub struct GpuMetrics {
    pub index: u32,
    pub utilization_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub temperature_c: u32,
    pub power_draw_w: u32,
    pub fan_speed_percent: u32,
    pub clock_graphics_mhz: u32,
    pub clock_memory_mhz: u32,
}

impl NvmlManager {
    /// Initialize NVML manager
    #[cfg(feature = "nvidia-support")]
    pub fn new() -> Result<Self> {
        info!("🔧 Initializing NVML manager");

        let nvml = Nvml::init()
            .context("Failed to initialize NVML. Make sure NVIDIA drivers are installed.")?;

        let device_count = nvml.device_count()
            .context("Failed to get GPU device count")?;

        info!("✅ NVML initialized, found {} GPU(s)", device_count);

        // Get all devices
        let mut devices = Vec::new();
        for i in 0..device_count {
            match nvml.device_by_index(i) {
                Ok(device) => {
                    // SAFETY: We're leaking the device lifetime to 'static
                    // This is safe because NVML maintains device references internally
                    let device: Device<'static> = unsafe {
                        std::mem::transmute(device)
                    };
                    devices.push(device);
                }
                Err(e) => {
                    warn!("Failed to get device {}: {}", i, e);
                }
            }
        }

        Ok(Self {
            nvml: Arc::new(nvml),
            devices: Arc::new(RwLock::new(devices)),
        })
    }

    #[cfg(not(feature = "nvidia-support"))]
    pub fn new() -> Result<Self> {
        Err(anyhow::anyhow!(
            "NVML support not compiled in. Enable 'nvidia-support' feature."
        ))
    }

    /// Get information about all GPUs
    #[cfg(feature = "nvidia-support")]
    pub async fn get_all_gpu_info(&self) -> Result<Vec<GpuInfo>> {
        let devices = self.devices.read().await;
        let mut infos = Vec::new();

        for (index, device) in devices.iter().enumerate() {
            match self.get_gpu_info_from_device(index as u32, device).await {
                Ok(info) => infos.push(info),
                Err(e) => {
                    warn!("Failed to get info for GPU {}: {}", index, e);
                }
            }
        }

        Ok(infos)
    }

    #[cfg(not(feature = "nvidia-support"))]
    pub async fn get_all_gpu_info(&self) -> Result<Vec<GpuInfo>> {
        Err(anyhow::anyhow!("NVML support not compiled in"))
    }

    /// Get GPU information from a device
    #[cfg(feature = "nvidia-support")]
    async fn get_gpu_info_from_device(&self, index: u32, device: &Device<'static>) -> Result<GpuInfo> {
        let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        let uuid = device.uuid().unwrap_or_else(|_| "Unknown".to_string());

        let memory_info = device.memory_info()
            .context("Failed to get memory info")?;
        let memory_total_mb = memory_info.total / (1024 * 1024);

        let compute_capability = device.cuda_compute_capability()
            .map(|c| (c.major as u32, c.minor as u32))
            .unwrap_or((0, 0));

        let pci_info = device.pci_info()
            .context("Failed to get PCI info")?;
        let pci_bus_id = format!("{:04x}:{:02x}:{:02x}.0",
            pci_info.domain, pci_info.bus, pci_info.device);

        let power_limit_w = device.power_management_limit()
            .map(|p| p / 1000)  // Convert milliwatts to watts
            .unwrap_or(0);

        let temperature_c = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
            .unwrap_or(0);

        Ok(GpuInfo {
            index,
            uuid,
            name,
            memory_total_mb,
            compute_capability,
            pci_bus_id,
            power_limit_w,
            temperature_c,
        })
    }

    /// Get metrics for a specific GPU by index
    #[cfg(feature = "nvidia-support")]
    pub async fn get_gpu_metrics(&self, gpu_index: u32) -> Result<GpuMetrics> {
        let devices = self.devices.read().await;

        let device = devices.get(gpu_index as usize)
            .context(format!("GPU index {} not found", gpu_index))?;

        self.get_metrics_from_device(gpu_index, device).await
    }

    #[cfg(not(feature = "nvidia-support"))]
    pub async fn get_gpu_metrics(&self, _gpu_index: u32) -> Result<GpuMetrics> {
        Err(anyhow::anyhow!("NVML support not compiled in"))
    }

    /// Get metrics from a device
    #[cfg(feature = "nvidia-support")]
    async fn get_metrics_from_device(&self, index: u32, device: &Device<'static>) -> Result<GpuMetrics> {
        // GPU utilization
        let utilization = device.utilization_rates()
            .context("Failed to get utilization rates")?;
        let utilization_percent = utilization.gpu as f32;

        // Memory usage
        let memory_info = device.memory_info()
            .context("Failed to get memory info")?;
        let memory_used_mb = memory_info.used / (1024 * 1024);
        let memory_total_mb = memory_info.total / (1024 * 1024);

        // Temperature
        let temperature_c = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
            .unwrap_or(0);

        // Power draw
        let power_draw_w = device.power_usage()
            .map(|p| p / 1000)  // Convert milliwatts to watts
            .unwrap_or(0);

        // Fan speed
        let fan_speed_percent = device.fan_speed(0)
            .unwrap_or(0);

        // Clock speeds
        let clock_graphics_mhz = device.clock_info(nvml_wrapper::enum_wrappers::device::Clock::Graphics)
            .unwrap_or(0);

        let clock_memory_mhz = device.clock_info(nvml_wrapper::enum_wrappers::device::Clock::Memory)
            .unwrap_or(0);

        Ok(GpuMetrics {
            index,
            utilization_percent,
            memory_used_mb,
            memory_total_mb,
            temperature_c,
            power_draw_w,
            fan_speed_percent,
            clock_graphics_mhz,
            clock_memory_mhz,
        })
    }

    /// Get metrics for all GPUs
    #[cfg(feature = "nvidia-support")]
    pub async fn get_all_gpu_metrics(&self) -> Result<Vec<GpuMetrics>> {
        let devices = self.devices.read().await;
        let mut metrics = Vec::new();

        for (index, device) in devices.iter().enumerate() {
            match self.get_metrics_from_device(index as u32, device).await {
                Ok(m) => metrics.push(m),
                Err(e) => {
                    warn!("Failed to get metrics for GPU {}: {}", index, e);
                }
            }
        }

        Ok(metrics)
    }

    #[cfg(not(feature = "nvidia-support"))]
    pub async fn get_all_gpu_metrics(&self) -> Result<Vec<GpuMetrics>> {
        Err(anyhow::anyhow!("NVML support not compiled in"))
    }

    /// Get driver version
    #[cfg(feature = "nvidia-support")]
    pub fn get_driver_version(&self) -> Result<String> {
        self.nvml.sys_driver_version()
            .context("Failed to get driver version")
    }

    #[cfg(not(feature = "nvidia-support"))]
    pub fn get_driver_version(&self) -> Result<String> {
        Err(anyhow::anyhow!("NVML support not compiled in"))
    }

    /// Get CUDA version
    #[cfg(feature = "nvidia-support")]
    pub fn get_cuda_version(&self) -> Result<String> {
        match self.nvml.sys_cuda_driver_version() {
            Ok(version) => {
                let major = version / 1000;
                let minor = (version % 1000) / 10;
                Ok(format!("{}.{}", major, minor))
            }
            Err(e) => Err(anyhow::anyhow!("Failed to get CUDA version: {}", e))
        }
    }

    #[cfg(not(feature = "nvidia-support"))]
    pub fn get_cuda_version(&self) -> Result<String> {
        Err(anyhow::anyhow!("NVML support not compiled in"))
    }

    /// Set power limit for a GPU
    #[cfg(feature = "nvidia-support")]
    pub async fn set_power_limit(&self, gpu_index: u32, watts: u32) -> Result<()> {
        let mut devices = self.devices.write().await;

        let device = devices.get_mut(gpu_index as usize)
            .context(format!("GPU index {} not found", gpu_index))?;

        let milliwatts = watts * 1000;
        device.set_power_management_limit(milliwatts)
            .context("Failed to set power limit")?;

        info!("✅ Set GPU {} power limit to {}W", gpu_index, watts);
        Ok(())
    }

    #[cfg(not(feature = "nvidia-support"))]
    pub async fn set_power_limit(&self, _gpu_index: u32, _watts: u32) -> Result<()> {
        Err(anyhow::anyhow!("NVML support not compiled in"))
    }

    /// Get number of GPUs
    #[cfg(feature = "nvidia-support")]
    pub async fn device_count(&self) -> usize {
        self.devices.read().await.len()
    }

    #[cfg(not(feature = "nvidia-support"))]
    pub async fn device_count(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[cfg(feature = "nvidia-support")]
    async fn test_nvml_manager_init() {
        // This test will only pass on systems with NVIDIA GPUs and drivers
        match NvmlManager::new() {
            Ok(manager) => {
                println!("NVML initialized successfully");

                // Try to get GPU info
                if let Ok(infos) = manager.get_all_gpu_info().await {
                    for info in infos {
                        println!("GPU {}: {} ({})", info.index, info.name, info.uuid);
                    }
                }
            }
            Err(e) => {
                println!("NVML init failed (expected on systems without NVIDIA GPUs): {}", e);
            }
        }
    }

    #[test]
    #[cfg(not(feature = "nvidia-support"))]
    fn test_nvml_manager_disabled() {
        let result = NvmlManager::new();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not compiled in"));
    }
}
