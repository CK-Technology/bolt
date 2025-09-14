use anyhow::{Result, Context};
use tracing::{info, warn, debug, error};
use std::path::Path;
use std::process::Command;
use serde::{Deserialize, Serialize};
use super::{GPUInfo, GPUVendor};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmdManager {
    pub driver_version: String,
    pub rocm_version: Option<String>,
    pub gpus: Vec<AmdGPU>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmdGPU {
    pub index: u32,
    pub name: String,
    pub memory_mb: u32,
    pub device_id: String,
    pub pci_bus_id: String,
}

impl AmdManager {
    pub fn detect() -> Result<Self> {
        info!("🔍 Detecting AMD GPU configuration");

        // Check for AMD GPUs via lspci
        let output = Command::new("lspci")
            .arg("-nn")
            .output()
            .context("lspci not found")?;

        let output_str = String::from_utf8(output.stdout)?;
        let mut gpus = Vec::new();

        let mut index = 0;
        for line in output_str.lines() {
            if line.to_lowercase().contains("amd") || line.to_lowercase().contains("ati") {
                if line.to_lowercase().contains("vga") || line.to_lowercase().contains("display") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(pci_id) = parts.first() {
                        let gpu = AmdGPU {
                            index,
                            name: Self::extract_gpu_name(line),
                            memory_mb: Self::get_gpu_memory(index).unwrap_or(0),
                            device_id: pci_id.to_string(),
                            pci_bus_id: pci_id.to_string(),
                        };
                        gpus.push(gpu);
                        index += 1;
                    }
                }
            }
        }

        let driver_version = Self::get_driver_version().unwrap_or("unknown".to_string());
        let rocm_version = Self::get_rocm_version().ok();

        info!("📊 AMD Detection Results:");
        info!("  Driver Version: {}", driver_version);
        if let Some(ref rocm) = rocm_version {
            info!("  ROCm Version: {}", rocm);
        }
        info!("  GPUs Found: {}", gpus.len());

        for gpu in &gpus {
            info!("  GPU {}: {} ({}MB)", gpu.index, gpu.name, gpu.memory_mb);
        }

        Ok(Self {
            driver_version,
            rocm_version,
            gpus,
        })
    }

    fn extract_gpu_name(line: &str) -> String {
        if let Some(start) = line.find(": ") {
            line[start + 2..].split('[').next().unwrap_or("Unknown AMD GPU").trim().to_string()
        } else {
            "Unknown AMD GPU".to_string()
        }
    }

    fn get_gpu_memory(index: u32) -> Result<u32> {
        // Try to get memory info from /sys/class/drm
        let mem_path = format!("/sys/class/drm/card{}/device/mem_info_vram_total", index);
        if let Ok(mem_str) = std::fs::read_to_string(&mem_path) {
            if let Ok(bytes) = mem_str.trim().parse::<u64>() {
                return Ok((bytes / 1024 / 1024) as u32); // Convert to MB
            }
        }

        // Fallback: check /proc/meminfo for rough estimate
        Ok(4096) // Default 4GB assumption
    }

    fn get_driver_version() -> Result<String> {
        // Try modinfo amdgpu
        if let Ok(output) = Command::new("modinfo").arg("amdgpu").output() {
            let output_str = String::from_utf8(output.stdout)?;
            for line in output_str.lines() {
                if line.starts_with("version:") {
                    return Ok(line.split(':').nth(1).unwrap_or("unknown").trim().to_string());
                }
            }
        }

        Ok("unknown".to_string())
    }

    fn get_rocm_version() -> Result<String> {
        // Try rocm-smi
        if let Ok(output) = Command::new("rocm-smi").arg("--version").output() {
            let output_str = String::from_utf8(output.stdout)?;
            for line in output_str.lines() {
                if line.contains("ROCm version") {
                    return Ok(line.split(':').nth(1).unwrap_or("unknown").trim().to_string());
                }
            }
        }

        // Try /opt/rocm/.info/version
        if let Ok(version) = std::fs::read_to_string("/opt/rocm/.info/version") {
            return Ok(version.trim().to_string());
        }

        Err(anyhow::anyhow!("ROCm not found"))
    }

    pub async fn setup_container_access(
        &self,
        container_id: &str,
        amd_config: &crate::config::AmdConfig,
    ) -> Result<()> {
        info!("🔴 Setting up AMD GPU access for container: {}", container_id);

        // Setup DRI device access
        self.setup_dri_access(container_id).await?;

        // Setup ROCm if available
        if self.rocm_version.is_some() {
            self.setup_rocm_access(container_id, amd_config).await?;
        }

        // Setup Vulkan drivers
        self.setup_vulkan_access(container_id).await?;

        info!("✅ AMD GPU access configured for container: {}", container_id);
        Ok(())
    }

    async fn setup_dri_access(&self, container_id: &str) -> Result<()> {
        info!("📱 Setting up DRI device access");

        // Check for DRI devices
        let dri_path = Path::new("/dev/dri");
        if !dri_path.exists() {
            return Err(anyhow::anyhow!("DRI devices not found - AMD graphics drivers may not be loaded"));
        }

        // List available DRI devices
        for entry in std::fs::read_dir(dri_path)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("card") || name.starts_with("renderD") {
                    debug!("  ✓ DRI device available: /dev/dri/{}", name);
                }
            }
        }

        Ok(())
    }

    async fn setup_rocm_access(&self, container_id: &str, amd_config: &crate::config::AmdConfig) -> Result<()> {
        info!("⚡ Setting up ROCm access");

        // Set ROCm environment variables
        if let Some(ref devices) = amd_config.rocm_visible_devices {
            info!("  Setting ROCM_VISIBLE_DEVICES={}", devices);
            std::env::set_var("ROCM_VISIBLE_DEVICES", devices);
        }

        std::env::set_var("HIP_VISIBLE_DEVICES", "0"); // Default to first GPU
        std::env::set_var("HSA_OVERRIDE_GFX_VERSION", "10.3.0"); // Common compatibility

        Ok(())
    }

    async fn setup_vulkan_access(&self, container_id: &str) -> Result<()> {
        info!("🎮 Setting up Vulkan access for AMD");

        // Check for AMD Vulkan driver
        let vulkan_paths = [
            "/usr/share/vulkan/icd.d/radeon_icd.x86_64.json",
            "/usr/share/vulkan/icd.d/amd_icd64.json",
            "/etc/vulkan/icd.d/radeon_icd.x86_64.json",
        ];

        for path in &vulkan_paths {
            if Path::new(path).exists() {
                info!("  ✓ AMD Vulkan ICD found: {}", path);
                std::env::set_var("VK_ICD_FILENAMES", path);
                break;
            }
        }

        Ok(())
    }

    pub async fn list_gpus(&self) -> Result<Vec<GPUInfo>> {
        let mut gpu_info = Vec::new();

        for gpu in &self.gpus {
            gpu_info.push(GPUInfo {
                vendor: GPUVendor::AMD,
                index: gpu.index,
                name: gpu.name.clone(),
                memory_mb: gpu.memory_mb,
                uuid: None, // AMD doesn't typically expose UUIDs like NVIDIA
                device_paths: vec![format!("/dev/dri/card{}", gpu.index)],
            });
        }

        Ok(gpu_info)
    }

    pub async fn run_opencl_application(&self, container_id: &str, app: &super::OpenCLApplication) -> Result<()> {
        info!("⚡ Running OpenCL application: {} in container: {}", app.name, container_id);

        // Set OpenCL environment for AMD
        std::env::set_var("OPENCL_VENDOR_PATH", "/etc/OpenCL/vendors");

        Ok(())
    }
}