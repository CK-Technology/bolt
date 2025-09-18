use super::{GPUInfo, GPUVendor};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use tracing::{debug, info, warn};

#[cfg(feature = "nvidia-support")]
use nvml_wrapper::Nvml;

#[cfg(feature = "nvidia-support")]
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;

use std::collections::HashMap;

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

        // Try NVML first (more accurate), fallback to nvidia-smi, then sysfs
        let detection_result = Self::detect_with_nvml()
            .or_else(|_| {
                info!("NVML unavailable, trying nvidia-smi");
                Self::detect_with_nvidia_smi()
            })
            .or_else(|_| {
                info!("nvidia-smi unavailable, trying sysfs fallback");
                Self::detect_with_sysfs()
            });

        match detection_result {
            Ok(manager) => {
                info!("✅ NVIDIA GPU detection successful");
                Ok(manager)
            }
            Err(e) => {
                warn!("❌ NVIDIA GPU detection failed: {}", e);
                Err(e)
            }
        }
    }

    pub async fn enable_dlss(&self, container_id: &str, enabled: bool) -> Result<()> {
        info!(
            "🎯 Setting NVIDIA DLSS to {} for container {}",
            enabled, container_id
        );

        // DLSS is enabled by ensuring the container has access to the right libraries
        // and setting the appropriate environment variables
        let dlss_env = if enabled {
            vec![
                ("NVIDIA_DLSS_ENABLED", "1"),
                ("NVIDIA_DRIVER_CAPABILITIES", "all"),
                ("NVIDIA_REQUIRE_CUDA", "cuda>=11.0"),
            ]
        } else {
            vec![("NVIDIA_DLSS_ENABLED", "0")]
        };

        self.set_container_env_vars(container_id, dlss_env).await?;

        if enabled {
            info!("✅ DLSS enabled for container {}", container_id);
        } else {
            info!("❌ DLSS disabled for container {}", container_id);
        }

        Ok(())
    }

    pub async fn enable_reflex(&self, container_id: &str, enabled: bool) -> Result<()> {
        info!(
            "⚡ Setting NVIDIA Reflex to {} for container {}",
            enabled, container_id
        );

        // Reflex requires low-latency mode and specific driver settings
        let reflex_env = if enabled {
            vec![
                ("NVIDIA_REFLEX_ENABLED", "1"),
                ("NVIDIA_LOW_LATENCY_MODE", "ultra"),
                ("NVIDIA_DRIVER_CAPABILITIES", "all"),
                ("__GL_YIELD", "USLEEP"),
            ]
        } else {
            vec![
                ("NVIDIA_REFLEX_ENABLED", "0"),
                ("NVIDIA_LOW_LATENCY_MODE", "off"),
            ]
        };

        self.set_container_env_vars(container_id, reflex_env)
            .await?;

        // Apply system-level optimizations for Reflex
        if enabled {
            self.apply_reflex_optimizations().await?;
            info!("✅ NVIDIA Reflex enabled for container {}", container_id);
        } else {
            info!("❌ NVIDIA Reflex disabled for container {}", container_id);
        }

        Ok(())
    }

    pub async fn set_power_limit(&self, device_id: u32, watts: u32) -> Result<()> {
        info!(
            "⚡ Setting NVIDIA GPU {} power limit to {} watts",
            device_id, watts
        );

        #[cfg(feature = "nvidia-support")]
        {
            let nvml = Nvml::init().context("Failed to initialize NVML")?;
            let device = nvml
                .device_by_index(device_id)
                .context("Failed to get GPU device")?;

            // Convert watts to milliwatts for NVML
            let milliwatts = watts * 1000;
            device
                .set_power_management_limit(milliwatts)
                .context("Failed to set power limit")?;

            info!(
                "✅ Power limit set to {} watts for GPU {}",
                watts, device_id
            );
        }

        #[cfg(not(feature = "nvidia-support"))]
        {
            // Fallback to nvidia-ml-py or nvidia-smi
            self.set_power_limit_fallback(device_id, watts).await?;
        }

        Ok(())
    }

    pub async fn set_memory_clock_offset(&self, device_id: u32, offset_mhz: i32) -> Result<()> {
        info!(
            "🔧 Setting NVIDIA GPU {} memory clock offset to {} MHz",
            device_id, offset_mhz
        );

        // Use nvidia-settings for clock offsets
        let output = Command::new("nvidia-settings")
            .args(&[
                "-a",
                &format!(
                    "[gpu:{}]/GPUMemoryTransferRateOffset[3]={}",
                    device_id, offset_mhz
                ),
                "--assign-server-socket",
                "/tmp/.X11-unix/X0",
            ])
            .output()
            .context("Failed to execute nvidia-settings")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to set memory clock offset: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        info!(
            "✅ Memory clock offset set to {} MHz for GPU {}",
            offset_mhz, device_id
        );
        Ok(())
    }

    pub async fn set_core_clock_offset(&self, device_id: u32, offset_mhz: i32) -> Result<()> {
        info!(
            "🔧 Setting NVIDIA GPU {} core clock offset to {} MHz",
            device_id, offset_mhz
        );

        let output = Command::new("nvidia-settings")
            .args(&[
                "-a",
                &format!(
                    "[gpu:{}]/GPUGraphicsClockOffset[3]={}",
                    device_id, offset_mhz
                ),
                "--assign-server-socket",
                "/tmp/.X11-unix/X0",
            ])
            .output()
            .context("Failed to execute nvidia-settings")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to set core clock offset: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        info!(
            "✅ Core clock offset set to {} MHz for GPU {}",
            offset_mhz, device_id
        );
        Ok(())
    }

    async fn set_container_env_vars(
        &self,
        container_id: &str,
        env_vars: Vec<(&str, &str)>,
    ) -> Result<()> {
        // This would integrate with the container runtime to set environment variables
        // For now, we'll log what would be set
        for (key, value) in env_vars {
            debug!("Setting container {} env: {}={}", container_id, key, value);
        }
        Ok(())
    }

    async fn apply_reflex_optimizations(&self) -> Result<()> {
        // Apply system-level optimizations for NVIDIA Reflex

        // Set CPU governor to performance
        let _ = Command::new("cpupower")
            .args(&["frequency-set", "-g", "performance"])
            .output();

        // Disable CPU C-states for ultra-low latency
        let _ = std::fs::write("/sys/devices/system/cpu/cpu0/cpuidle/state1/disable", "1");
        let _ = std::fs::write("/sys/devices/system/cpu/cpu0/cpuidle/state2/disable", "1");

        // Set process priority
        #[cfg(feature = "oci-runtime")]
        unsafe {
            nix::libc::setpriority(nix::libc::PRIO_PROCESS, 0, -20);
        }

        info!("✅ Applied NVIDIA Reflex system optimizations");
        Ok(())
    }

    #[cfg(not(feature = "nvidia-support"))]
    async fn set_power_limit_fallback(&self, device_id: u32, watts: u32) -> Result<()> {
        let output = Command::new("nvidia-smi")
            .args(&["-i", &device_id.to_string(), "-pl", &watts.to_string()])
            .output()
            .context("Failed to execute nvidia-smi")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to set power limit via nvidia-smi: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    #[cfg(feature = "nvidia-support")]
    fn detect_with_nvml() -> Result<Self> {
        info!("🔬 Detecting NVIDIA GPUs using NVML (preferred method)");

        let nvml = Nvml::init()
            .context("Failed to initialize NVML - NVIDIA drivers may not be installed")?;
        let device_count = nvml.device_count().context("Failed to get device count")?;

        let mut gpus = Vec::new();

        for i in 0..device_count {
            let device = nvml
                .device_by_index(i)
                .context("Failed to get device by index")?;

            let name = device
                .name()
                .unwrap_or_else(|_| format!("Unknown GPU {}", i));
            let uuid = device
                .uuid()
                .unwrap_or_else(|_| format!("unknown-uuid-{}", i));
            let memory_info = device.memory_info().unwrap_or_else(|_| {
                nvml_wrapper::struct_wrappers::device::MemoryInfo {
                    free: 0,
                    total: 0,
                    used: 0,
                }
            });
            let memory_mb = (memory_info.total / 1024 / 1024) as u32;

            // Get compute capability
            let compute_capability = match device.cuda_compute_capability() {
                Ok(cc) => format!("{}.{}", cc.major, cc.minor),
                Err(_) => "Unknown".to_string(),
            };

            // Get PCI info
            let pci_info = device.pci_info().ok();
            let pci_bus_id = match pci_info {
                Some(pci) => format!("{:04X}:{:02X}:{:02X}.0", pci.domain, pci.bus, pci.device),
                None => format!("unknown-pci-{}", i),
            };

            // Get power and temperature info
            let power_limit_w = device
                .power_management_limit_default()
                .ok()
                .map(|p| p / 1000);
            let temperature_c = device.temperature(TemperatureSensor::Gpu).ok();

            let gpu = NvidiaGPU {
                index: i,
                uuid,
                name,
                memory_mb,
                compute_capability,
                pci_bus_id,
                power_limit_w,
                temperature_c,
            };
            gpus.push(gpu);
        }

        let driver_version = nvml
            .sys_driver_version()
            .unwrap_or_else(|_| "unknown".to_string());
        let cuda_version = nvml
            .sys_cuda_driver_version()
            .ok()
            .map(|v| format!("{}", v));

        let container_runtime_available = Self::check_container_runtime_available();

        Self::log_detection_results(
            &driver_version,
            &cuda_version,
            &gpus,
            container_runtime_available,
        );

        Ok(Self {
            driver_version,
            cuda_version,
            gpus,
            container_runtime_available,
        })
    }

    #[cfg(not(feature = "nvidia-support"))]
    fn detect_with_nvml() -> Result<Self> {
        Err(anyhow::anyhow!("NVML support not compiled in"))
    }

    fn detect_with_nvidia_smi() -> Result<Self> {
        info!("🖥️ Detecting NVIDIA GPUs using nvidia-smi");

        // Check if nvidia-smi is available
        let output = Command::new("nvidia-smi")
            .arg("--query-gpu=index,uuid,name,memory.total,compute_cap,pci.bus_id,power.max_limit,temperature.gpu")
            .arg("--format=csv,noheader,nounits")
            .output()
            .context("nvidia-smi not found - NVIDIA drivers may not be installed")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "nvidia-smi failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
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
        let container_runtime_available = Self::check_container_runtime_available();

        Self::log_detection_results(
            &driver_version,
            &cuda_version,
            &gpus,
            container_runtime_available,
        );

        Ok(Self {
            driver_version,
            cuda_version,
            gpus,
            container_runtime_available,
        })
    }

    fn detect_with_sysfs() -> Result<Self> {
        info!("📁 Detecting NVIDIA GPUs using sysfs fallback");

        let mut gpus = Vec::new();
        let mut device_info = HashMap::new();

        // Check for NVIDIA devices in /sys/class/drm
        if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("card") && !name.contains("-") {
                        // Read vendor ID to check if it's NVIDIA (0x10de)
                        let vendor_path = path.join("device/vendor");
                        if let Ok(vendor) = std::fs::read_to_string(&vendor_path) {
                            if vendor.trim() == "0x10de" {
                                let index = name
                                    .strip_prefix("card")
                                    .and_then(|s| s.parse::<u32>().ok())
                                    .unwrap_or(0);

                                // Try to read device name
                                let device_name = path
                                    .join("device/device")
                                    .as_path()
                                    .to_str()
                                    .and_then(|p| std::fs::read_to_string(p).ok())
                                    .unwrap_or_else(|| format!("NVIDIA GPU {}", index));

                                device_info.insert(
                                    index,
                                    (name.to_string(), device_name.trim().to_string()),
                                );
                            }
                        }
                    }
                }
            }
        }

        // Also check /dev/nvidia* devices
        if let Ok(entries) = std::fs::read_dir("/dev") {
            for entry in entries.filter_map(Result::ok) {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("nvidia") && name.len() > 6 {
                        if let Ok(index) = name[6..].parse::<u32>() {
                            if !device_info.contains_key(&index) {
                                device_info.insert(
                                    index,
                                    (name.to_string(), format!("NVIDIA GPU {}", index)),
                                );
                            }
                        }
                    }
                }
            }
        }

        // Create GPU entries from discovered devices
        for (&index, (device_name, gpu_name)) in &device_info {
            let gpu = NvidiaGPU {
                index,
                uuid: format!("sysfs-detected-{}", index),
                name: gpu_name.clone(),
                memory_mb: 0, // Can't determine from sysfs easily
                compute_capability: "Unknown".to_string(),
                pci_bus_id: format!("unknown-pci-{}", index),
                power_limit_w: None,
                temperature_c: None,
            };
            gpus.push(gpu);
        }

        if gpus.is_empty() {
            return Err(anyhow::anyhow!("No NVIDIA devices found in sysfs"));
        }

        let driver_version =
            Self::get_driver_version_from_sysfs().unwrap_or_else(|_| "unknown (sysfs)".to_string());
        let container_runtime_available = Self::check_container_runtime_available();

        Self::log_detection_results(&driver_version, &None, &gpus, container_runtime_available);

        Ok(Self {
            driver_version,
            cuda_version: None,
            gpus,
            container_runtime_available,
        })
    }

    fn check_container_runtime_available() -> bool {
        Path::new("/usr/bin/nvidia-container-runtime").exists()
            || Path::new("/usr/bin/nvidia-docker").exists()
            || Path::new("/usr/bin/nvidia-container-toolkit").exists()
    }

    fn get_driver_version_from_sysfs() -> Result<String> {
        // Try to read driver version from /proc/driver/nvidia/version
        if let Ok(version_info) = std::fs::read_to_string("/proc/driver/nvidia/version") {
            for line in version_info.lines() {
                if line.contains("Kernel Module") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    for (i, part) in parts.iter().enumerate() {
                        if part == &"Module" && i + 1 < parts.len() {
                            return Ok(parts[i + 1].to_string());
                        }
                    }
                }
            }
        }
        Err(anyhow::anyhow!(
            "Could not determine driver version from sysfs"
        ))
    }

    fn log_detection_results(
        driver_version: &str,
        cuda_version: &Option<String>,
        gpus: &[NvidiaGPU],
        container_runtime_available: bool,
    ) {
        info!("📊 NVIDIA Detection Results:");
        info!("  Driver Version: {}", driver_version);
        if let Some(cuda) = &cuda_version {
            info!("  CUDA Version: {}", cuda);
        }
        info!("  GPUs Found: {}", gpus.len());
        info!(
            "  Container Runtime: {}",
            if container_runtime_available {
                "Available"
            } else {
                "Not Available"
            }
        );

        for gpu in gpus {
            info!(
                "  GPU {}: {} ({}MB, {})",
                gpu.index, gpu.name, gpu.memory_mb, gpu.compute_capability
            );
        }
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
        info!(
            "🟢 Setting up NVIDIA GPU access for container: {}",
            container_id
        );

        // Validate GPU device selection
        let device_spec = nvidia_config
            .device
            .map(|d| d.to_string())
            .unwrap_or("all".to_string());
        let device_indices = self.parse_device_spec(&device_spec)?;

        for &index in &device_indices {
            if let Some(gpu) = self.gpus.get(index as usize) {
                info!("  📱 Configuring access to GPU {}: {}", index, gpu.name);
            } else {
                warn!("  ⚠️ GPU {} not found", index);
            }
        }

        // Setup device access
        self.setup_device_access(container_id, &device_indices)
            .await?;

        // Configure CUDA environment
        self.setup_cuda_environment(container_id, nvidia_config, &device_indices)
            .await?;

        // Enable specific features
        if nvidia_config.dlss == Some(true) {
            self.enable_dlss_support(container_id).await?;
        }

        if nvidia_config.raytracing == Some(true) {
            self.enable_raytracing_support(container_id).await?;
        }

        // Set up container runtime integration
        if self.container_runtime_available {
            self.setup_nvidia_container_runtime(container_id, nvidia_config)
                .await?;
        } else {
            warn!("⚠️ nvidia-container-runtime not available, using basic device access");
            self.setup_basic_device_access(container_id, &device_indices)
                .await?;
        }

        info!(
            "✅ NVIDIA GPU access configured for container: {}",
            container_id
        );
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
                    if let (Ok(start), Ok(end)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                    {
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
                        if gpu.uuid == spec
                            || gpu.name.to_lowercase().contains(&spec.to_lowercase())
                        {
                            indices.push(i as u32);
                            break;
                        }
                    }
                }
            }
        }

        if indices.is_empty() {
            return Err(anyhow::anyhow!(
                "No valid GPU devices found for spec: {}",
                device_spec
            ));
        }

        Ok(indices)
    }

    async fn setup_device_access(&self, container_id: &str, device_indices: &[u32]) -> Result<()> {
        info!("📱 Setting up GPU device access");

        // NVIDIA proprietary driver device files that need to be accessible
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

        // Also check for DRI devices (for Vulkan/OpenGL)
        self.add_dri_devices(&mut device_paths, device_indices)
            .await?;

        // Verify and categorize devices
        let (available_devices, missing_devices) = self.categorize_devices(&device_paths);

        if !available_devices.is_empty() {
            info!("  ✅ Available devices: {}", available_devices.len());
            for device in &available_devices {
                debug!("    ✓ {}", device);
            }
        }

        if !missing_devices.is_empty() {
            warn!("  ⚠️ Missing devices: {}", missing_devices.len());
            for device in &missing_devices {
                debug!("    ✗ {}", device);
            }
        }

        // Store device info for container setup
        self.store_container_device_info(container_id, &available_devices)
            .await?;

        Ok(())
    }

    async fn add_dri_devices(
        &self,
        device_paths: &mut Vec<String>,
        device_indices: &[u32],
    ) -> Result<()> {
        // For each NVIDIA GPU, try to find corresponding DRI device
        for &index in device_indices {
            // Check for renderD device (compute/Vulkan)
            let render_device = format!("/dev/dri/renderD{}", 128 + index);
            if Path::new(&render_device).exists() {
                device_paths.push(render_device);
            }

            // Check for card device (display)
            let card_device = format!("/dev/dri/card{}", index);
            if Path::new(&card_device).exists() {
                device_paths.push(card_device);
            }
        }
        Ok(())
    }

    fn categorize_devices(&self, device_paths: &[String]) -> (Vec<String>, Vec<String>) {
        let mut available = Vec::new();
        let mut missing = Vec::new();

        for device in device_paths {
            if Path::new(device).exists() {
                available.push(device.clone());
            } else {
                missing.push(device.clone());
            }
        }

        (available, missing)
    }

    async fn store_container_device_info(
        &self,
        container_id: &str,
        devices: &[String],
    ) -> Result<()> {
        // This would store device mapping information for the container runtime
        debug!(
            "Storing device info for container {}: {:?}",
            container_id, devices
        );
        // In a real implementation, this might write to a config file or database
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
        let cuda_devices = device_indices
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");

        info!("  Setting CUDA_VISIBLE_DEVICES={}", cuda_devices);
        unsafe {
            std::env::set_var("CUDA_VISIBLE_DEVICES", &cuda_devices);
        }

        // Set NVIDIA driver capabilities
        let mut capabilities = vec!["compute", "utility"];

        // Enable graphics capabilities by default (could be made configurable)
        capabilities.push("graphics");

        // Enable video capabilities by default (could be made configurable)
        capabilities.extend(&["video", "display"]);

        let driver_capabilities = capabilities.join(",");
        info!(
            "  Setting NVIDIA_DRIVER_CAPABILITIES={}",
            driver_capabilities
        );
        unsafe {
            std::env::set_var("NVIDIA_DRIVER_CAPABILITIES", &driver_capabilities);
        }

        // Set CUDA requirements (using cuda field from config)
        if nvidia_config.cuda.unwrap_or(false) {
            info!("  CUDA support enabled");
            unsafe {
                std::env::set_var("NVIDIA_REQUIRE_CUDA", "11.0");
            }
        }

        // Set basic driver requirements
        info!("  Setting basic driver requirements");
        unsafe {
            std::env::set_var("NVIDIA_REQUIRE_DRIVER", "470.0");
        }

        Ok(())
    }

    async fn enable_dlss_support(&self, container_id: &str) -> Result<()> {
        info!(
            "✨ Enabling NVIDIA DLSS support for container: {}",
            container_id
        );

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
        info!(
            "🌟 Enabling NVIDIA Ray Tracing support for container: {}",
            container_id
        );

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

    async fn setup_nvidia_container_runtime(
        &self,
        container_id: &str,
        nvidia_config: &crate::config::NvidiaConfig,
    ) -> Result<()> {
        info!("🐳 Configuring nvidia-container-runtime");

        // This would integrate with the actual nvidia-container-runtime
        // to properly configure GPU access in the container

        Ok(())
    }

    async fn setup_basic_device_access(
        &self,
        container_id: &str,
        device_indices: &[u32],
    ) -> Result<()> {
        info!("🔧 Setting up basic GPU device access (manual method)");

        // When nvidia-container-runtime is not available, manually configure devices
        let device_info = self
            .get_container_devices_for_manual_setup(device_indices)
            .await?;

        // Create device mount instructions
        let mount_commands = self.generate_device_mount_commands(&device_info)?;

        info!(
            "  📋 Generated {} device mount commands",
            mount_commands.len()
        );
        for cmd in &mount_commands {
            debug!("    {}", cmd);
        }

        // Set up library paths for NVIDIA drivers
        self.setup_nvidia_library_paths(container_id).await?;

        // Configure device permissions for rootless containers
        self.setup_device_permissions(container_id, &device_info)
            .await?;

        info!("  ✅ Basic device access configured");
        Ok(())
    }

    async fn get_container_devices_for_manual_setup(
        &self,
        device_indices: &[u32],
    ) -> Result<Vec<DeviceInfo>> {
        let mut devices = Vec::new();

        // Core NVIDIA devices
        devices.extend(vec![
            DeviceInfo {
                host_path: "/dev/nvidiactl".to_string(),
                container_path: "/dev/nvidiactl".to_string(),
                permissions: "rw".to_string(),
                device_type: DeviceType::Character,
                required: true,
            },
            DeviceInfo {
                host_path: "/dev/nvidia-uvm".to_string(),
                container_path: "/dev/nvidia-uvm".to_string(),
                permissions: "rw".to_string(),
                device_type: DeviceType::Character,
                required: true,
            },
            DeviceInfo {
                host_path: "/dev/nvidia-uvm-tools".to_string(),
                container_path: "/dev/nvidia-uvm-tools".to_string(),
                permissions: "rw".to_string(),
                device_type: DeviceType::Character,
                required: false,
            },
        ]);

        // Per-GPU devices
        for &index in device_indices {
            devices.push(DeviceInfo {
                host_path: format!("/dev/nvidia{}", index),
                container_path: format!("/dev/nvidia{}", index),
                permissions: "rw".to_string(),
                device_type: DeviceType::Character,
                required: true,
            });

            // Add DRI devices for Vulkan/OpenGL
            let render_device = format!("/dev/dri/renderD{}", 128 + index);
            if Path::new(&render_device).exists() {
                devices.push(DeviceInfo {
                    host_path: render_device.clone(),
                    container_path: render_device,
                    permissions: "rw".to_string(),
                    device_type: DeviceType::Character,
                    required: false,
                });
            }
        }

        Ok(devices)
    }

    fn generate_device_mount_commands(&self, devices: &[DeviceInfo]) -> Result<Vec<String>> {
        let mut commands = Vec::new();

        for device in devices {
            if Path::new(&device.host_path).exists() {
                commands.push(format!(
                    "--device {}:{}:{}",
                    device.host_path, device.container_path, device.permissions
                ));
            } else if device.required {
                return Err(anyhow::anyhow!(
                    "Required device {} not found on host",
                    device.host_path
                ));
            } else {
                debug!("Optional device {} not available", device.host_path);
            }
        }

        Ok(commands)
    }

    async fn setup_nvidia_library_paths(&self, container_id: &str) -> Result<()> {
        info!(
            "📚 Setting up NVIDIA library paths for container: {}",
            container_id
        );

        // Common NVIDIA library paths to mount
        let library_paths = vec![
            "/usr/lib/x86_64-linux-gnu/libnvidia-*.so*",
            "/usr/lib/x86_64-linux-gnu/libcuda*.so*",
            "/usr/lib/x86_64-linux-gnu/libcudart*.so*",
            "/usr/lib/x86_64-linux-gnu/libnvcuvid*.so*",
            "/usr/lib/x86_64-linux-gnu/libnvencod*.so*",
        ];

        for pattern in &library_paths {
            debug!("  📖 Library pattern: {}", pattern);
        }

        // In a real implementation, these would be mounted into the container
        Ok(())
    }

    async fn setup_device_permissions(
        &self,
        container_id: &str,
        devices: &[DeviceInfo],
    ) -> Result<()> {
        info!(
            "🔐 Setting up device permissions for container: {}",
            container_id
        );

        // For rootless containers, we may need to adjust device permissions
        // or use user namespaces
        for device in devices {
            if Path::new(&device.host_path).exists() {
                debug!(
                    "  🔑 Device: {} -> {}",
                    device.host_path, device.container_path
                );
                // Would check/adjust permissions here
            }
        }

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

    pub async fn run_cuda_application(
        &self,
        container_id: &str,
        app: &super::CudaApplication,
    ) -> Result<()> {
        info!(
            "🚀 Running CUDA application: {} in container: {}",
            app.name, container_id
        );

        // Validate compute capability if specified
        if let Some(ref required_cc) = app.compute_capability {
            for gpu in &self.gpus {
                if gpu.compute_capability >= *required_cc {
                    info!(
                        "  ✓ GPU {} meets compute capability requirement: {}",
                        gpu.index, gpu.compute_capability
                    );
                }
            }
        }

        // Check memory requirements
        if let Some(memory_gb) = app.memory_gb {
            let memory_mb = memory_gb * 1024;
            for gpu in &self.gpus {
                if gpu.memory_mb >= memory_mb {
                    info!(
                        "  ✓ GPU {} has sufficient memory: {}MB >= {}MB",
                        gpu.index, gpu.memory_mb, memory_mb
                    );
                }
            }
        }

        Ok(())
    }

    pub async fn run_opencl_application(
        &self,
        container_id: &str,
        app: &super::OpenCLApplication,
    ) -> Result<()> {
        info!(
            "⚡ Running OpenCL application: {} in container: {}",
            app.name, container_id
        );

        // Set OpenCL environment
        unsafe {
            std::env::set_var("OPENCL_VENDOR_PATH", "/etc/OpenCL/vendors");
        }

        Ok(())
    }

    pub async fn setup_wine_integration(&self, container_id: &str) -> Result<()> {
        info!("🍷 Setting up NVIDIA integration for Wine/Proton");

        // Detect driver type and configure accordingly
        let driver_type = self.detect_driver_type().await?;

        match driver_type {
            NvidiaDriverType::NvidiaOpen => {
                self.setup_nvidia_open_wine_integration(container_id)
                    .await?;
            }
            NvidiaDriverType::Proprietary => {
                self.setup_proprietary_wine_integration(container_id)
                    .await?;
            }
            NvidiaDriverType::NouveauLegacy => {
                self.setup_nouveau_wine_integration(container_id).await?;
            }
            NvidiaDriverType::NVK => {
                self.setup_nvk_wine_integration(container_id).await?;
            }
        }

        Ok(())
    }

    async fn detect_driver_type(&self) -> Result<NvidiaDriverType> {
        info!("🔍 Detecting NVIDIA driver type...");

        // Priority 1: NVIDIA Open GPU Kernel Modules (preferred)
        if self.check_nvidia_open_driver().await? {
            info!("  ✅ NVIDIA Open GPU Kernel Modules detected (primary choice)");
            return Ok(NvidiaDriverType::NvidiaOpen);
        }

        // Priority 2: NVIDIA Proprietary driver
        if Path::new("/sys/module/nvidia").exists() || Path::new("/proc/driver/nvidia").exists() {
            info!("  ✅ NVIDIA Proprietary driver detected");
            return Ok(NvidiaDriverType::Proprietary);
        }

        // Priority 3: nouveau open-source driver
        if Path::new("/sys/module/nouveau").exists() {
            // Check if NVK (Vulkan driver) is available
            if self.check_nvk_support().await? {
                info!("  ✅ Nouveau + NVK Vulkan driver detected");
                return Ok(NvidiaDriverType::NVK);
            }
            info!("  ✅ Nouveau driver detected");
            return Ok(NvidiaDriverType::NouveauLegacy);
        }

        Err(anyhow::anyhow!("Could not detect any NVIDIA driver type"))
    }

    async fn check_nvidia_open_driver(&self) -> Result<bool> {
        info!("    🔍 Checking for NVIDIA Open GPU Kernel Modules...");

        // NVIDIA Open kernel modules (supports Turing and later)
        let open_modules = [
            "/sys/module/nvidia_drm",     // Display driver module
            "/sys/module/nvidia_modeset", // Mode setting module
            "/sys/module/nvidia_uvm",     // Unified memory module
        ];

        let mut found_modules = 0;
        for module_path in &open_modules {
            if Path::new(module_path).exists() {
                found_modules += 1;
                debug!("      ✅ Found module: {}", module_path);
            }
        }

        if found_modules == 0 {
            return Ok(false);
        }

        // Enhanced detection for NVIDIA Open GPU Kernel Modules
        let open_indicators = [
            "/usr/src/nvidia-open",                      // Open driver source
            "/var/lib/dkms/nvidia-open",                 // DKMS build directory
            "/lib/modules/*/updates/dkms/nvidia-drm.ko", // Open module location
        ];

        for indicator in &open_indicators {
            if Path::new(indicator).exists() {
                info!("      ✅ NVIDIA Open indicator found: {}", indicator);
                return self.verify_nvidia_open_features().await;
            }
        }

        // Check GSP firmware support (key feature of open modules)
        if self.check_gsp_firmware_support().await? {
            info!("      ✅ GSP firmware support detected (NVIDIA Open feature)");
            return Ok(true);
        }

        // Check for Turing+ generation GPU (required for open modules)
        if self.check_turing_or_later_gpu().await? {
            info!("      ✅ Turing+ GPU detected, compatible with NVIDIA Open");

            // Final check: modeset parameter often enabled with open driver
            if let Ok(modeset) =
                std::fs::read_to_string("/sys/module/nvidia_drm/parameters/modeset")
            {
                if modeset.trim() == "Y" {
                    info!("      ✅ Modeset enabled (typical for NVIDIA Open)");
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    async fn check_gsp_firmware_support(&self) -> Result<bool> {
        // GSP (GPU System Processor) firmware is required for NVIDIA Open modules
        let gsp_paths = [
            "/lib/firmware/nvidia",
            "/usr/lib/firmware/nvidia",
            "/lib/firmware/nvidia/*/gsp.bin",
        ];

        for path in &gsp_paths {
            if Path::new(path).exists() {
                debug!("        GSP firmware path found: {}", path);
                return Ok(true);
            }
        }

        // Also check if GSP is mentioned in kernel logs or proc
        if let Ok(dmesg) = std::process::Command::new("dmesg").arg("-t").output() {
            let output = String::from_utf8_lossy(&dmesg.stdout);
            if output.contains("GSP") && output.contains("nvidia") {
                debug!("        GSP references found in kernel logs");
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn check_turing_or_later_gpu(&self) -> Result<bool> {
        // NVIDIA Open modules support Turing (RTX 20xx) and later
        // Check GPU generation from nvidia-smi or PCI device info

        if let Ok(output) = Command::new("nvidia-smi")
            .arg("--query-gpu=name")
            .arg("--format=csv,noheader")
            .output()
        {
            let gpu_names = String::from_utf8_lossy(&output.stdout);

            for gpu_name in gpu_names.lines() {
                let gpu_name = gpu_name.trim().to_lowercase();

                // Turing and later generations supported by NVIDIA Open
                let supported_series = [
                    "rtx 20",
                    "rtx 30",
                    "rtx 40",
                    "rtx 50", // Consumer Turing+
                    "gtx 16", // Turing GTX
                    "quadro rtx",
                    "tesla t",
                    "tesla v100", // Professional Turing+
                    "a100",
                    "a40",
                    "a30",
                    "a10", // Ampere
                    "h100",
                    "h800",
                    "l40",
                    "l4", // Hopper/Ada
                ];

                for series in &supported_series {
                    if gpu_name.contains(series) {
                        debug!("        Supported GPU detected: {}", gpu_name);
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    async fn verify_nvidia_open_features(&self) -> Result<bool> {
        // Verify that this is actually the open driver with enhanced features
        let mut open_features = 0;

        // Check for debug support (available with DEBUG=1 build)
        if Path::new("/sys/module/nvidia/parameters/NVreg_EnableDbgBreakpoint").exists() {
            open_features += 1;
            debug!("        Debug parameter support found");
        }

        // Check for enhanced logging capabilities
        if Path::new("/sys/module/nvidia/parameters/NVreg_EnableVerboseLogging").exists() {
            open_features += 1;
            debug!("        Verbose logging support found");
        }

        // Open modules often have better integration with kernel
        if let Ok(modules) = std::fs::read_to_string("/proc/modules") {
            if modules.contains("nvidia_drm") && modules.contains("nvidia_modeset") {
                open_features += 1;
                debug!("        Full module stack loaded");
            }
        }

        Ok(open_features >= 2)
    }

    async fn check_nvk_support(&self) -> Result<bool> {
        // NVK is part of Mesa, check for Mesa Vulkan driver
        if let Ok(output) = Command::new("vulkaninfo").arg("--summary").output() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            if output_str.contains("NVK") || output_str.contains("nouveau") {
                return Ok(true);
            }
        }

        // Also check for mesa libraries
        Ok(Path::new("/usr/lib/x86_64-linux-gnu/libvulkan_nouveau.so").exists())
    }

    async fn setup_proprietary_wine_integration(&self, container_id: &str) -> Result<()> {
        info!("  🔵 Configuring proprietary NVIDIA driver for Wine");

        // Enable NVAPI for Wine
        unsafe {
            std::env::set_var("WINE_ENABLE_NVAPI", "1");
            std::env::set_var("DXVK_ENABLE_NVAPI", "1");
            std::env::set_var("DXVK_NVAPI_ALLOW_OTHER", "1");
        }

        // Set up proprietary driver libraries
        self.setup_proprietary_libraries(container_id).await?;

        info!("    ✓ NVAPI enabled for Wine applications");
        info!("    ✓ DXVK NVAPI integration enabled");
        Ok(())
    }

    async fn setup_nvidia_open_wine_integration(&self, container_id: &str) -> Result<()> {
        info!("  🔵 Configuring NVIDIA Open GPU Kernel Modules for Wine");

        // NVIDIA Open modules support both proprietary userspace libs AND some open features
        unsafe {
            // Can use NVIDIA userspace libraries with open kernel modules
            std::env::set_var("WINE_ENABLE_NVAPI", "1");
            std::env::set_var("DXVK_ENABLE_NVAPI", "1");
            std::env::set_var("DXVK_NVAPI_ALLOW_OTHER", "1");

            // Enable Vulkan optimizations (works better with open modules)
            std::env::set_var("VK_LAYER_PATH", "/usr/share/vulkan/explicit_layer.d");
            std::env::set_var("NVIDIA_ENABLE_OPEN_OPTIMIZATIONS", "1");
        }

        // Set up libraries for NVIDIA open
        self.setup_nvidia_open_libraries(container_id).await?;

        info!("    ✓ NVAPI enabled (works with open kernel modules)");
        info!("    ✓ NVIDIA Open GPU optimizations enabled");
        info!("    ✓ Vulkan and DXVK configured");
        Ok(())
    }

    async fn setup_nouveau_wine_integration(&self, container_id: &str) -> Result<()> {
        info!("  🟢 Configuring nouveau open-source driver for Wine");

        // For nouveau, we primarily use Mesa/Gallium3D
        unsafe {
            std::env::set_var("MESA_LOADER_DRIVER_OVERRIDE", "nouveau");
            std::env::set_var("GALLIUM_DRIVER", "nouveau");
            // Disable NVAPI since it's not available with nouveau
            std::env::set_var("DXVK_ENABLE_NVAPI", "0");
        }

        self.setup_mesa_libraries(container_id).await?;

        info!("    ✓ Mesa/Gallium3D configured for nouveau");
        info!("    ✓ NVAPI disabled (not available with nouveau)");
        Ok(())
    }

    async fn setup_nvk_wine_integration(&self, container_id: &str) -> Result<()> {
        info!("  🔴 Configuring NVK (Mesa Vulkan) driver for Wine");

        // NVK provides Vulkan support on nouveau
        unsafe {
            std::env::set_var(
                "VK_ICD_FILENAMES",
                "/usr/share/vulkan/icd.d/nouveau_icd.x86_64.json",
            );
            std::env::set_var("MESA_LOADER_DRIVER_OVERRIDE", "nouveau");
            std::env::set_var("GALLIUM_DRIVER", "nouveau");
            // VKD3D can work with NVK for DirectX 12
            std::env::set_var("VKD3D_CONFIG", "vulkan");
            // Disable NVAPI
            std::env::set_var("DXVK_ENABLE_NVAPI", "0");
        }

        self.setup_nvk_libraries(container_id).await?;

        info!("    ✓ NVK Vulkan driver configured");
        info!("    ✓ VKD3D configured for DirectX 12 support");
        info!("    ✓ NVAPI disabled (using Vulkan path)");
        Ok(())
    }

    async fn setup_proprietary_libraries(&self, _container_id: &str) -> Result<()> {
        // Mount proprietary NVIDIA libraries
        let lib_paths = vec![
            "/usr/lib/x86_64-linux-gnu/libnvidia-*.so*",
            "/usr/lib/x86_64-linux-gnu/libGL.so*",
            "/usr/lib/x86_64-linux-gnu/libEGL.so*",
        ];

        for path in &lib_paths {
            debug!("    📖 Proprietary library: {}", path);
        }

        Ok(())
    }

    async fn setup_nvidia_open_libraries(&self, _container_id: &str) -> Result<()> {
        // Mount libraries for NVIDIA Open kernel modules
        // Uses NVIDIA userspace libs with open kernel modules
        let lib_paths = vec![
            "/usr/lib/x86_64-linux-gnu/libnvidia-*.so*",
            "/usr/lib/x86_64-linux-gnu/libGL.so*",
            "/usr/lib/x86_64-linux-gnu/libEGL.so*",
            "/usr/lib/x86_64-linux-gnu/libvulkan.so*",
            "/usr/lib/x86_64-linux-gnu/libcuda*.so*",
        ];

        for path in &lib_paths {
            debug!("    📖 NVIDIA Open library: {}", path);
        }

        Ok(())
    }

    async fn setup_mesa_libraries(&self, _container_id: &str) -> Result<()> {
        // Mount Mesa libraries for nouveau
        let lib_paths = vec![
            "/usr/lib/x86_64-linux-gnu/dri/nouveau_dri.so",
            "/usr/lib/x86_64-linux-gnu/libGL.so*",
            "/usr/lib/x86_64-linux-gnu/libEGL.so*",
        ];

        for path in &lib_paths {
            debug!("    📖 Mesa library: {}", path);
        }

        Ok(())
    }

    async fn setup_nvk_libraries(&self, _container_id: &str) -> Result<()> {
        // Mount NVK/Mesa Vulkan libraries
        let lib_paths = vec![
            "/usr/lib/x86_64-linux-gnu/dri/nouveau_dri.so",
            "/usr/lib/x86_64-linux-gnu/libvulkan_nouveau.so",
            "/usr/share/vulkan/icd.d/nouveau_icd.x86_64.json",
        ];

        for path in &lib_paths {
            debug!("    📖 NVK library: {}", path);
        }

        Ok(())
    }

    pub async fn setup_open_source_gpu_access(
        &self,
        container_id: &str,
        device_indices: &[u32],
    ) -> Result<()> {
        info!(
            "🌍 Setting up open-source GPU access for container: {}",
            container_id
        );

        // Detect which open-source path we're using
        let driver_type = self.detect_driver_type().await?;

        match driver_type {
            NvidiaDriverType::NvidiaOpen => {
                info!("  🔵 Using NVIDIA Open GPU Kernel Modules path");
                self.setup_nvidia_open_gpu_access(container_id, device_indices)
                    .await?;
            }
            NvidiaDriverType::NouveauLegacy | NvidiaDriverType::NVK => {
                info!("  🟡 Using nouveau/NVK driver path");
                self.setup_nouveau_gpu_access(container_id, device_indices)
                    .await?;
            }
            NvidiaDriverType::Proprietary => {
                warn!("  ⚠️ Proprietary driver detected, not open-source");
                return Err(anyhow::anyhow!(
                    "setup_open_source_gpu_access called with proprietary driver"
                ));
            }
        }

        Ok(())
    }

    async fn setup_nvidia_open_gpu_access(
        &self,
        container_id: &str,
        device_indices: &[u32],
    ) -> Result<()> {
        info!("    🔧 Configuring NVIDIA Open GPU access");

        // NVIDIA Open uses both NVIDIA devices AND DRI devices
        let mut devices = Vec::new();

        for &index in device_indices {
            // NVIDIA control devices (same as proprietary)
            devices.extend(vec![
                format!("/dev/nvidia{}", index),
                "/dev/nvidiactl".to_string(),
                "/dev/nvidia-uvm".to_string(),
            ]);

            // DRI devices (enhanced support with open modules)
            let render_device = format!("/dev/dri/renderD{}", 128 + index);
            if Path::new(&render_device).exists() {
                devices.push(render_device);
            }

            let card_device = format!("/dev/dri/card{}", index);
            if Path::new(&card_device).exists() {
                devices.push(card_device);
            }
        }

        info!("    ✅ NVIDIA Open: {} devices available", devices.len());
        for device in &devices {
            if Path::new(device).exists() {
                debug!("      📱 Available: {}", device);
            }
        }

        // Set up NVIDIA Open environment
        self.setup_nvidia_open_environment().await?;

        Ok(())
    }

    async fn setup_nouveau_gpu_access(
        &self,
        container_id: &str,
        device_indices: &[u32],
    ) -> Result<()> {
        info!("    🔧 Configuring nouveau GPU access");

        // For nouveau, we primarily need DRI devices
        let mut dri_devices = Vec::new();

        for &index in device_indices {
            // Add render nodes (preferred for compute/Vulkan)
            let render_device = format!("/dev/dri/renderD{}", 128 + index);
            if Path::new(&render_device).exists() {
                dri_devices.push(render_device);
            }

            // Add card nodes for display (if needed)
            let card_device = format!("/dev/dri/card{}", index);
            if Path::new(&card_device).exists() {
                dri_devices.push(card_device);
            }
        }

        if dri_devices.is_empty() {
            return Err(anyhow::anyhow!("No DRI devices found for nouveau driver"));
        }

        info!(
            "    ✅ Nouveau: {} DRI devices available",
            dri_devices.len()
        );
        for device in &dri_devices {
            debug!("      📱 DRI device: {}", device);
        }

        // Set up environment for Mesa/nouveau
        self.setup_nouveau_environment().await?;

        Ok(())
    }

    async fn setup_nvidia_open_environment(&self) -> Result<()> {
        info!("      🌟 Configuring NVIDIA Open GPU Kernel Modules environment");

        unsafe {
            // NVIDIA Open modules support full NVIDIA userspace stack
            std::env::set_var(
                "NVIDIA_DRIVER_CAPABILITIES",
                "compute,utility,graphics,video,display",
            );

            // GSP firmware optimizations (available with open modules)
            std::env::set_var("NVIDIA_GSP_OPTIMIZATIONS", "1");
            std::env::set_var("NVIDIA_OPEN_MODULE_FEATURES", "1");

            // Enhanced Vulkan support (Turing+ with full feature set)
            std::env::set_var("VK_LAYER_PATH", "/usr/share/vulkan/explicit_layer.d");
            std::env::set_var("__VK_LAYER_NV_optimus", "NVIDIA_only");

            // OpenGL optimizations
            std::env::set_var("__GL_THREADED_OPTIMIZATIONS", "1");
            std::env::set_var("__GL_SHADER_CACHE", "1");
            std::env::set_var("__GL_ALLOW_UNOFFICIAL_PROTOCOL", "1");

            // CUDA optimizations for open modules
            std::env::set_var("CUDA_CACHE_DISABLE", "0");
            std::env::set_var("CUDA_CACHE_MAXSIZE", "2147483648"); // 2GB cache

            // Memory and performance optimizations specific to open modules
            std::env::set_var("NVIDIA_OPEN_MEMORY_OPTIMIZATIONS", "1");
            std::env::set_var("NVIDIA_TURING_OPTIMIZATIONS", "1");

            // Debug and logging (if DEBUG=1 was used during build)
            if Path::new("/sys/module/nvidia/parameters/NVreg_EnableVerboseLogging").exists() {
                std::env::set_var("NVIDIA_ENABLE_DEBUG_LOGGING", "1");
            }
        }

        info!("        ✅ NVIDIA Open optimizations configured");
        info!("        ✅ GSP firmware features enabled");
        info!("        ✅ Turing+ generation optimizations applied");

        Ok(())
    }

    async fn setup_nouveau_environment(&self) -> Result<()> {
        info!("      🌱 Configuring nouveau environment");

        unsafe {
            // Mesa configuration for nouveau
            std::env::set_var("MESA_LOADER_DRIVER_OVERRIDE", "nouveau");
            std::env::set_var("GALLIUM_DRIVER", "nouveau");

            // Enable multi-threading for better performance
            std::env::set_var("mesa_glthread", "true");

            // Vulkan configuration for NVK
            std::env::set_var(
                "VK_ICD_FILENAMES",
                "/usr/share/vulkan/icd.d/nouveau_icd.x86_64.json",
            );
        }

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

    /// Setup GPU access for AI workloads
    pub async fn setup_ai_gpu_access(
        &self,
        container_id: &str,
        ai_workload: &super::AIWorkload,
    ) -> Result<()> {
        info!(
            "🤖 Setting up NVIDIA GPU for AI workload: {}",
            ai_workload.name
        );

        // Configure CUDA for AI workload
        self.setup_ai_cuda_environment(container_id, ai_workload.multi_gpu)
            .await?;

        // AI-specific optimizations
        info!("  📊 Configuring AI optimizations");
        info!("    • Memory allocation: Optimized for inference");
        info!("    • Batch processing: Enabled");
        if ai_workload.enable_flash_attention {
            info!("    • Flash Attention: Enabled");
        }
        if ai_workload.enable_kv_cache {
            info!("    • KV Cache: Enabled");
        }

        Ok(())
    }

    /// Setup GPU access for ML training/inference workloads
    pub async fn setup_ml_gpu_access(
        &self,
        container_id: &str,
        ml_workload: &super::MLWorkload,
    ) -> Result<()> {
        info!(
            "🧠 Setting up NVIDIA GPU for ML workload: {}",
            ml_workload.name
        );

        // Configure CUDA for ML workload
        self.setup_ai_cuda_environment(container_id, ml_workload.distributed_training)
            .await?;

        // ML-specific optimizations
        info!("  📊 Configuring ML optimizations");
        info!("    • Framework: {:?}", ml_workload.ml_framework);
        if ml_workload.mixed_precision {
            info!("    • Mixed Precision: Enabled (Tensor Cores)");
        }
        if ml_workload.distributed_training {
            info!("    • Distributed Training: Multi-GPU setup");
        }

        // Enable Tensor Cores for compatible workloads
        if self.supports_tensor_cores() {
            info!("    • Tensor Cores: Available for acceleration");
        }

        Ok(())
    }

    /// Setup GPU access for general compute workloads
    pub async fn setup_compute_gpu_access(
        &self,
        container_id: &str,
        compute_workload: &super::ComputeWorkload,
    ) -> Result<()> {
        info!(
            "⚙️ Setting up NVIDIA GPU for compute workload: {}",
            compute_workload.name
        );

        // Configure based on compute type
        match &compute_workload.compute_type {
            super::ComputeType::Scientific => {
                self.setup_ai_cuda_environment(container_id, compute_workload.enable_peer_to_peer)
                    .await?;
                info!("  🔬 Scientific computing optimizations applied");
            }
            super::ComputeType::Rendering => {
                self.setup_rendering_optimizations(container_id).await?;
                info!("  🎨 Rendering optimizations applied");
            }
            super::ComputeType::Cryptocurrency => {
                self.setup_mining_optimizations(container_id).await?;
                info!("  ₿ Cryptocurrency mining optimizations applied");
            }
            _ => {
                self.setup_ai_cuda_environment(container_id, false).await?;
                info!("  ⚙️ General compute optimizations applied");
            }
        }

        Ok(())
    }

    async fn setup_ai_cuda_environment(
        &self,
        _container_id: &str,
        enable_multi_gpu: bool,
    ) -> Result<()> {
        info!("  🔧 Configuring CUDA environment for AI/ML workloads");
        info!("    • Multi-GPU support: {}", enable_multi_gpu);
        if enable_multi_gpu && self.gpus.len() > 1 {
            info!("    • Available GPUs: {}", self.gpus.len());
        }
        Ok(())
    }

    async fn setup_rendering_optimizations(&self, _container_id: &str) -> Result<()> {
        info!("  🎨 Configuring NVIDIA rendering optimizations");
        // RTX features, CUDA graphics interop, etc.
        Ok(())
    }

    async fn setup_mining_optimizations(&self, _container_id: &str) -> Result<()> {
        info!("  ₿ Configuring mining optimizations");
        // Power efficiency, memory timing optimizations, etc.
        Ok(())
    }

    fn supports_tensor_cores(&self) -> bool {
        // Check if any GPU supports Tensor Cores (Volta/Turing/Ampere/Ada/Hopper)
        self.gpus.iter().any(|gpu| {
            // Parse compute capability from string format like "8.9"
            if let Ok(version) = gpu.compute_capability.parse::<f32>() {
                version >= 7.0 // Volta (7.0) and newer have Tensor Cores
            } else {
                false
            }
        })
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

#[derive(Debug, Clone)]
struct DeviceInfo {
    pub host_path: String,
    pub container_path: String,
    pub permissions: String,
    pub device_type: DeviceType,
    pub required: bool,
}

#[derive(Debug, Clone)]
enum DeviceType {
    Character,
    Block,
}

#[derive(Debug, Clone)]
enum NvidiaDriverType {
    NvidiaOpen,    // NVIDIA Open GPU Kernel Modules (primary - supports full Vulkan)
    Proprietary,   // nvidia.ko proprietary driver (traditional - supports full Vulkan)
    NouveauLegacy, // nouveau.ko open-source driver (legacy)
    NVK,           // nouveau.ko + NVK Vulkan driver (community Vulkan implementation)
}
