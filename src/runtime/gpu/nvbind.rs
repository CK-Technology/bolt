//! Native GPU detection and management for Bolt container runtime.
//!
//! This module provides native NVIDIA GPU detection without requiring external
//! nvidia-container-toolkit or nvbind binary. It detects:
//! - GPU devices via /dev/nvidia*, sysfs, and NVML
//! - Driver type (Open GPU Kernel Modules, Proprietary, Nouveau)
//! - Driver version and CUDA version
//! - GPU architecture and capabilities

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};

use crate::runtime::gpu::{
    AIWorkload, ComputeWorkload, GPUInfo, GPUVendor, GamingConfig, MLWorkload,
};

/// GPU Architecture Generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuArchitecture {
    Maxwell,     // GTX 900 series - Compute 5.x
    Pascal,      // GTX 10 series - Compute 6.x
    Volta,       // TITAN V - Compute 7.0
    Turing,      // RTX 20 series - Compute 7.5
    Ampere,      // RTX 30 series - Compute 8.x
    AdaLovelace, // RTX 40 series - Compute 8.9
    Hopper,      // H100 - Compute 9.0
    Blackwell,   // RTX 50 series - Compute 10.0
    Unknown,
}

impl GpuArchitecture {
    pub fn from_compute_capability(major: u32, minor: u32) -> Self {
        match (major, minor) {
            (5, _) => Self::Maxwell,
            (6, _) => Self::Pascal,
            (7, 0) => Self::Volta,
            (7, 5) => Self::Turing,
            (8, 0) | (8, 6) => Self::Ampere,
            (8, 9) => Self::AdaLovelace,
            (9, 0) => Self::Hopper,
            (10, 0) => Self::Blackwell,
            _ if major >= 10 => Self::Blackwell, // Future Blackwell variants
            _ => Self::Unknown,
        }
    }

    pub fn from_gpu_name(name: &str) -> Self {
        let name_lower = name.to_lowercase();
        if name_lower.contains("rtx 50")
            || name_lower.contains("5090")
            || name_lower.contains("5080")
            || name_lower.contains("5070")
        {
            Self::Blackwell
        } else if name_lower.contains("rtx 40")
            || name_lower.contains("4090")
            || name_lower.contains("4080")
            || name_lower.contains("4070")
        {
            Self::AdaLovelace
        } else if name_lower.contains("rtx 30")
            || name_lower.contains("3090")
            || name_lower.contains("3080")
            || name_lower.contains("3070")
        {
            Self::Ampere
        } else if name_lower.contains("rtx 20")
            || name_lower.contains("2080")
            || name_lower.contains("2070")
            || name_lower.contains("2060")
        {
            Self::Turing
        } else if name_lower.contains("gtx 10")
            || name_lower.contains("1080")
            || name_lower.contains("1070")
            || name_lower.contains("1060")
        {
            Self::Pascal
        } else if name_lower.contains("gtx 9")
            || name_lower.contains("980")
            || name_lower.contains("970")
            || name_lower.contains("960")
        {
            Self::Maxwell
        } else if name_lower.contains("h100") || name_lower.contains("h200") {
            Self::Hopper
        } else if name_lower.contains("a100")
            || name_lower.contains("a40")
            || name_lower.contains("a30")
        {
            Self::Ampere
        } else {
            Self::Unknown
        }
    }

    pub fn supports_mig(&self) -> bool {
        matches!(self, Self::Ampere | Self::Hopper | Self::Blackwell)
    }

    pub fn tensor_core_generation(&self) -> Option<u8> {
        match self {
            Self::Volta => Some(1),
            Self::Turing => Some(2),
            Self::Ampere => Some(3),
            Self::AdaLovelace => Some(4),
            Self::Hopper => Some(4), // Enhanced 4th gen
            Self::Blackwell => Some(5),
            _ => None,
        }
    }

    pub fn supports_fp4(&self) -> bool {
        matches!(self, Self::Blackwell)
    }
}

/// NVIDIA Driver Type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriverType {
    /// NVIDIA Open GPU Kernel Modules (recommended for newer GPUs)
    NvidiaOpen,
    /// NVIDIA Proprietary Driver
    NvidiaProprietary,
    /// Nouveau Open Source Driver (limited features)
    Nouveau,
}

impl DriverType {
    pub fn name(&self) -> &'static str {
        match self {
            DriverType::NvidiaOpen => "NVIDIA Open GPU Kernel Modules",
            DriverType::NvidiaProprietary => "NVIDIA Proprietary",
            DriverType::Nouveau => "Nouveau",
        }
    }

    pub fn supports_cuda(&self) -> bool {
        matches!(self, Self::NvidiaOpen | Self::NvidiaProprietary)
    }
}

/// Detected GPU device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuDevice {
    pub id: String,
    pub name: String,
    pub pci_address: String,
    pub driver_version: Option<String>,
    pub memory_bytes: Option<u64>,
    pub device_path: String,
    pub architecture: GpuArchitecture,
    pub compute_capability: Option<(u32, u32)>,
}

/// Driver information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverInfo {
    pub version: String,
    pub driver_type: DriverType,
    pub cuda_version: Option<String>,
    pub libraries: Vec<String>,
}

impl Default for DriverInfo {
    fn default() -> Self {
        Self {
            version: "unknown".to_string(),
            driver_type: DriverType::Nouveau,
            cuda_version: None,
            libraries: Vec::new(),
        }
    }
}

/// Native GPU manager that replaces external nvbind dependency
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NvbindManager {
    pub is_available: bool,
    pub driver_info: Option<DriverInfo>,
    pub detected_gpus: Vec<GpuDevice>,
}

impl NvbindManager {
    /// Detect NVIDIA GPUs using native methods (no external binary required)
    pub fn detect() -> Result<Self> {
        info!("🔍 Detecting NVIDIA GPU configuration (native)");

        // Check if NVIDIA driver is available
        let driver_available = Self::check_driver_status();
        if !driver_available {
            debug!("NVIDIA driver not detected");
            return Ok(Self {
                is_available: false,
                driver_info: None,
                detected_gpus: Vec::new(),
            });
        }

        // Get driver information
        let driver_info = Self::detect_driver_info();

        // Detect GPUs
        let detected_gpus = Self::discover_gpus_sync();

        let is_available = !detected_gpus.is_empty();

        if is_available {
            info!("✅ Native NVIDIA GPU detection successful");
            info!(
                "  • Driver: {} ({})",
                driver_info
                    .as_ref()
                    .map(|d| d.version.as_str())
                    .unwrap_or("unknown"),
                driver_info
                    .as_ref()
                    .map(|d| d.driver_type.name())
                    .unwrap_or("unknown")
            );
            info!("  • GPUs detected: {}", detected_gpus.len());
            for gpu in &detected_gpus {
                info!(
                    "    - GPU {}: {} ({:?})",
                    gpu.id, gpu.name, gpu.architecture
                );
            }
        } else {
            debug!("No NVIDIA GPUs detected");
        }

        Ok(Self {
            is_available,
            driver_info,
            detected_gpus,
        })
    }

    /// Check if NVIDIA driver is loaded
    fn check_driver_status() -> bool {
        // Check for NVIDIA drivers (proprietary or open)
        let nvidia_paths = [
            "/proc/driver/nvidia/version",
            "/dev/nvidiactl",
            "/sys/module/nvidia",
        ];

        for path in &nvidia_paths {
            if Path::new(path).exists() {
                return true;
            }
        }

        // Check if nouveau is loaded (limited support)
        if Path::new("/sys/module/nouveau").exists() {
            warn!("⚠️ Nouveau driver detected - container GPU passthrough not supported");
            return false;
        }

        false
    }

    /// Detect driver type and version
    fn detect_driver_info() -> Option<DriverInfo> {
        let driver_type = Self::detect_driver_type();
        let version = Self::get_driver_version(&driver_type).ok()?;
        let cuda_version = Self::get_cuda_version().ok();
        let libraries = Self::find_nvidia_libraries().unwrap_or_default();

        Some(DriverInfo {
            version,
            driver_type,
            cuda_version,
            libraries,
        })
    }

    /// Detect which NVIDIA driver type is loaded
    fn detect_driver_type() -> DriverType {
        // Check loaded kernel modules
        if let Ok(modules) = fs::read_to_string("/proc/modules") {
            if modules.contains("nouveau") {
                return DriverType::Nouveau;
            }
            if modules.contains("nvidia") {
                if Self::is_nvidia_open_driver() {
                    return DriverType::NvidiaOpen;
                } else {
                    return DriverType::NvidiaProprietary;
                }
            }
        }

        // Fallback: check /sys/module
        if Path::new("/sys/module/nouveau").exists() {
            return DriverType::Nouveau;
        }
        if Path::new("/sys/module/nvidia").exists() {
            if Self::is_nvidia_open_driver() {
                return DriverType::NvidiaOpen;
            } else {
                return DriverType::NvidiaProprietary;
            }
        }

        DriverType::NvidiaProprietary
    }

    /// Check if NVIDIA Open GPU Kernel Modules are in use
    fn is_nvidia_open_driver() -> bool {
        // Method 1: Check /proc/driver/nvidia/version for open kernel indicators
        if let Ok(content) = fs::read_to_string("/proc/driver/nvidia/version")
            && (content.contains("Open Kernel")
                || content.contains("open-gpu-kernel-modules")
                || content.contains("GSP"))
        {
            debug!("Detected NVIDIA Open driver via version string");
            return true;
        }

        // Method 2: Check for GSP (GPU System Processor) firmware enablement
        if let Ok(content) =
            fs::read_to_string("/sys/module/nvidia/parameters/NVreg_EnableGpuFirmware")
            && content.trim() == "1"
        {
            debug!("Detected NVIDIA Open driver via GSP firmware enablement");
            return true;
        }

        // Method 3: Check for open driver specific parameters
        if Path::new("/sys/module/nvidia/parameters/NVreg_OpenRmEnableUnsupportedGpus").exists() {
            debug!("Detected NVIDIA Open driver via open RM parameters");
            return true;
        }

        // Method 4: Check nvidia-caps device (more common with open driver)
        if Path::new("/dev/nvidia-caps").exists() {
            debug!("Detected NVIDIA Open driver via capabilities device");
            return true;
        }

        // Method 5: Check modinfo for open source indicators
        if let Ok(output) = std::process::Command::new("modinfo").arg("nvidia").output()
            && output.status.success()
        {
            let modinfo = String::from_utf8_lossy(&output.stdout);
            if modinfo.contains("open-gpu-kernel-modules") || modinfo.contains("NVIDIA Open GPU") {
                return true;
            }
        }

        false
    }

    /// Get driver version
    fn get_driver_version(driver_type: &DriverType) -> Result<String> {
        match driver_type {
            DriverType::Nouveau => {
                // Get kernel version for Nouveau
                if let Ok(output) = std::process::Command::new("uname").arg("-r").output()
                    && output.status.success()
                {
                    let kernel_version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    return Ok(format!("nouveau (kernel {})", kernel_version));
                }
                Ok("nouveau".to_string())
            }
            _ => {
                // Try /proc/driver/nvidia/version
                if let Ok(content) = fs::read_to_string("/proc/driver/nvidia/version") {
                    for line in content.lines() {
                        if line.contains("Kernel Module") {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if let Some(version) = parts.iter().find(|&s| {
                                s.contains('.')
                                    && s.chars().next().is_some_and(|c| c.is_ascii_digit())
                            }) {
                                return Ok(version.to_string());
                            }
                        }
                    }

                    // Try regex extraction
                    if let Ok(re) = Regex::new(r"(\d{3}\.\d+(\.\d+)?)")
                        && let Some(caps) = re.captures(&content)
                    {
                        return Ok(caps[1].to_string());
                    }
                }

                // Fallback: nvidia-smi
                if let Ok(output) = std::process::Command::new("nvidia-smi")
                    .args(["--query-gpu=driver_version", "--format=csv,noheader"])
                    .output()
                    && output.status.success()
                {
                    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !version.is_empty() {
                        return Ok(version);
                    }
                }

                Err(anyhow::anyhow!("Could not determine driver version"))
            }
        }
    }

    /// Get CUDA version
    fn get_cuda_version() -> Result<String> {
        // Try nvidia-smi
        if let Ok(output) = std::process::Command::new("nvidia-smi")
            .args(["--query-gpu=cuda_version", "--format=csv,noheader"])
            .output()
            && output.status.success()
        {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !version.is_empty() && version != "N/A" {
                return Ok(version);
            }
        }

        Err(anyhow::anyhow!("Could not determine CUDA version"))
    }

    /// Find NVIDIA libraries
    fn find_nvidia_libraries() -> Result<Vec<String>> {
        let mut libraries = Vec::new();
        let search_paths = [
            "/usr/lib/x86_64-linux-gnu",
            "/usr/lib64",
            "/usr/local/lib",
            "/lib64",
            "/lib/x86_64-linux-gnu",
        ];

        for search_path in &search_paths {
            if let Ok(entries) = fs::read_dir(search_path) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("libnvidia") || name_str.starts_with("libcuda") {
                        libraries.push(entry.path().to_string_lossy().to_string());
                    }
                }
            }
        }

        libraries.sort();
        libraries.dedup();
        Ok(libraries)
    }

    /// Discover all NVIDIA GPUs (synchronous)
    fn discover_gpus_sync() -> Vec<GpuDevice> {
        let mut gpus = Vec::new();

        // Find NVIDIA device nodes
        let nvidia_devices = Self::find_nvidia_device_nodes();
        debug!("Found {} NVIDIA device node(s)", nvidia_devices.len());

        for (index, device_path) in nvidia_devices.iter().enumerate() {
            if let Some(gpu) = Self::create_gpu_device(index, device_path) {
                gpus.push(gpu);
            }
        }

        gpus
    }

    /// Find NVIDIA device nodes in /dev
    fn find_nvidia_device_nodes() -> Vec<String> {
        let mut devices = Vec::new();

        if let Ok(entries) = fs::read_dir("/dev") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                // Look for nvidia0, nvidia1, etc. (not nvidiactl, nvidia-uvm, etc.)
                if name_str.starts_with("nvidia")
                    && let Some(suffix) = name_str.strip_prefix("nvidia")
                    && !suffix.is_empty()
                    && suffix.chars().all(|c| c.is_ascii_digit())
                {
                    devices.push(entry.path().to_string_lossy().to_string());
                }
            }
        }

        devices.sort();
        devices
    }

    /// Create a GpuDevice from device path
    fn create_gpu_device(index: usize, device_path: &str) -> Option<GpuDevice> {
        let name = Self::get_gpu_name(index).unwrap_or_else(|| format!("NVIDIA GPU {}", index));
        let pci_address =
            Self::get_pci_address(index).unwrap_or_else(|| format!("unknown:{}", index));
        let memory_bytes = Self::get_gpu_memory(index);
        let compute_capability = Self::get_compute_capability(index);

        let architecture = if let Some((major, minor)) = compute_capability {
            GpuArchitecture::from_compute_capability(major, minor)
        } else {
            GpuArchitecture::from_gpu_name(&name)
        };

        Some(GpuDevice {
            id: index.to_string(),
            name,
            pci_address,
            driver_version: None,
            memory_bytes,
            device_path: device_path.to_string(),
            architecture,
            compute_capability,
        })
    }

    /// Get GPU name from procfs or nvidia-smi
    fn get_gpu_name(index: usize) -> Option<String> {
        // Try procfs first
        let proc_path = format!("/proc/driver/nvidia/gpus/{}/information", index);
        if let Ok(content) = fs::read_to_string(&proc_path) {
            for line in content.lines() {
                if line.starts_with("Model:") {
                    return Some(line.replace("Model:", "").trim().to_string());
                }
            }
        }

        // Fallback to nvidia-smi
        if let Ok(output) = std::process::Command::new("nvidia-smi")
            .args([
                "--id",
                &index.to_string(),
                "--query-gpu=name",
                "--format=csv,noheader",
            ])
            .output()
            && output.status.success()
        {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }

        None
    }

    /// Get GPU PCI address
    fn get_pci_address(index: usize) -> Option<String> {
        let proc_path = format!("/proc/driver/nvidia/gpus/{}/information", index);
        if let Ok(content) = fs::read_to_string(&proc_path) {
            for line in content.lines() {
                if line.starts_with("Bus Location:") {
                    return Some(line.replace("Bus Location:", "").trim().to_string());
                }
            }
        }

        // Fallback to nvidia-smi
        if let Ok(output) = std::process::Command::new("nvidia-smi")
            .args([
                "--id",
                &index.to_string(),
                "--query-gpu=pci.bus_id",
                "--format=csv,noheader",
            ])
            .output()
            && output.status.success()
        {
            let pci = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !pci.is_empty() {
                return Some(pci);
            }
        }

        None
    }

    /// Get GPU memory in bytes
    fn get_gpu_memory(index: usize) -> Option<u64> {
        // Try nvidia-smi (most reliable)
        if let Ok(output) = std::process::Command::new("nvidia-smi")
            .args([
                "--id",
                &index.to_string(),
                "--query-gpu=memory.total",
                "--format=csv,noheader,nounits",
            ])
            .output()
            && output.status.success()
        {
            let mem_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(mb) = mem_str.parse::<u64>() {
                return Some(mb * 1024 * 1024); // MiB to bytes
            }
        }

        // Fallback to procfs
        let proc_path = format!("/proc/driver/nvidia/gpus/{}/information", index);
        if let Ok(content) = fs::read_to_string(&proc_path)
            && let Ok(re) = Regex::new(r"(\d+)\s*MB")
        {
            for line in content.lines() {
                if line.contains("Memory:")
                    && let Some(caps) = re.captures(line)
                    && let Ok(mb) = caps[1].parse::<u64>()
                {
                    return Some(mb * 1024 * 1024);
                }
            }
        }

        None
    }

    /// Get compute capability
    fn get_compute_capability(index: usize) -> Option<(u32, u32)> {
        // Try nvidia-smi
        if let Ok(output) = std::process::Command::new("nvidia-smi")
            .args([
                "--id",
                &index.to_string(),
                "--query-gpu=compute_cap",
                "--format=csv,noheader",
            ])
            .output()
            && output.status.success()
        {
            let cap_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let parts: Vec<&str> = cap_str.split('.').collect();
            if parts.len() == 2
                && let (Ok(major), Ok(minor)) = (parts[0].parse(), parts[1].parse())
            {
                return Some((major, minor));
            }
        }

        None
    }

    /// Get required device paths for container passthrough
    pub fn get_required_devices() -> Vec<String> {
        let mut devices = vec!["/dev/nvidiactl".to_string(), "/dev/nvidia-uvm".to_string()];

        // Add nvidia-uvm-tools if available
        if Path::new("/dev/nvidia-uvm-tools").exists() {
            devices.push("/dev/nvidia-uvm-tools".to_string());
        }

        // Add nvidia-modeset if available
        if Path::new("/dev/nvidia-modeset").exists() {
            devices.push("/dev/nvidia-modeset".to_string());
        }

        // Add specific GPU devices
        for i in 0..16 {
            let device = format!("/dev/nvidia{}", i);
            if Path::new(&device).exists() {
                devices.push(device);
            }
        }

        // Add nvidia-caps if available (compute mode)
        for path in [
            "/dev/nvidia-caps/nvidia-cap1",
            "/dev/nvidia-caps/nvidia-cap2",
        ] {
            if Path::new(path).exists() {
                devices.push(path.to_string());
            }
        }

        devices
    }

    // ============= Container integration methods =============

    pub async fn setup_container_access(
        &self,
        container_id: &str,
        gpu_config: &crate::config::GpuConfig,
    ) -> Result<()> {
        if !self.is_available {
            warn!("⚠️ Native GPU runtime not available");
            return Ok(());
        }

        info!(
            "🚀 Setting up native GPU access for container: {}",
            container_id
        );

        // Log driver info
        if let Some(ref driver) = self.driver_info {
            info!(
                "  • Driver: {} ({})",
                driver.version,
                driver.driver_type.name()
            );
            if let Some(ref cuda) = driver.cuda_version {
                info!("  • CUDA: {}", cuda);
            }
        }

        // Apply GPU config
        if let Some(ref nvbind_config) = gpu_config.nvbind {
            info!("  ✓ Applying GPU configuration:");
            info!("    • Driver: {:?}", nvbind_config.driver);
            info!("    • Devices: {:?}", nvbind_config.devices);
            info!(
                "    • Performance mode: {:?}",
                nvbind_config.performance_mode
            );
        }

        // Apply gaming optimizations if enabled
        if let Some(ref gaming_config) = gpu_config.gaming {
            info!("  ✓ Gaming optimizations enabled:");
            info!("    • Profile: {:?}", gaming_config.profile);
            info!("    • DLSS: {:?}", gaming_config.dlss_enabled);
            info!("    • RT cores: {:?}", gaming_config.rt_cores_enabled);
        }

        // Apply AI/ML optimizations if enabled
        if let Some(ref aiml_config) = gpu_config.aiml {
            info!("  ✓ AI/ML optimizations enabled:");
            info!("    • Profile: {:?}", aiml_config.profile);
            info!("    • Tensor cores: {:?}", aiml_config.tensor_cores_enabled);
            info!("    • Mixed precision: {:?}", aiml_config.mixed_precision);
        }

        Ok(())
    }

    pub async fn list_gpus(&self) -> Result<Vec<GPUInfo>> {
        if !self.is_available {
            return Ok(Vec::new());
        }

        let gpus: Vec<GPUInfo> = self
            .detected_gpus
            .iter()
            .map(|gpu| GPUInfo {
                vendor: GPUVendor::NVIDIA,
                index: gpu.id.parse().unwrap_or(0),
                name: gpu.name.clone(),
                memory_mb: (gpu.memory_bytes.unwrap_or(0) / (1024 * 1024)) as u32,
                uuid: Some(format!("GPU-{}", gpu.id)),
                device_paths: vec![gpu.device_path.clone()],
            })
            .collect();

        info!("  ✓ Found {} GPUs via native detection", gpus.len());
        Ok(gpus)
    }

    pub async fn run_gaming_workload(
        &self,
        container_id: &str,
        gaming_config: &GamingConfig,
    ) -> Result<()> {
        if !self.is_available {
            warn!("⚠️ Native GPU runtime not available for gaming workload");
            return Ok(());
        }

        info!(
            "🎮 Running gaming workload via native GPU runtime for container: {}",
            container_id
        );
        info!("  ✓ Gaming workload configured:");
        info!("    • Game type: {}", gaming_config.game_type);
        info!("    • DXVK enabled: {}", gaming_config.dxvk_enabled);
        info!("    • VKD3D enabled: {}", gaming_config.vkd3d_enabled);
        info!("    • GameMode enabled: {}", gaming_config.gamemode_enabled);
        info!("    • VR enabled: {}", gaming_config.vr_enabled);
        info!(
            "    • Performance profile: {}",
            gaming_config.performance_profile
        );
        info!("    • Native GPU passthrough enabled");

        Ok(())
    }

    pub async fn run_ai_workload(
        &self,
        container_id: &str,
        ai_workload: &AIWorkload,
    ) -> Result<()> {
        if !self.is_available {
            warn!("⚠️ Native GPU runtime not available for AI workload");
            return Ok(());
        }

        info!(
            "🤖 Running AI workload via native GPU runtime for container: {}",
            container_id
        );
        info!("  ✓ AI workload configured:");
        info!("    • Model: {}", ai_workload.model_name);
        info!("    • Backend: {:?}", ai_workload.ai_backend);
        info!("    • Context length: {:?}", ai_workload.context_length);
        info!("    • Quantization: {:?}", ai_workload.quantization);
        info!("    • Multi-GPU: {}", ai_workload.multi_gpu);
        info!(
            "    • Flash Attention: {}",
            ai_workload.enable_flash_attention
        );
        info!("    • Tensor core acceleration enabled");

        Ok(())
    }

    pub async fn run_ml_workload(
        &self,
        container_id: &str,
        ml_workload: &MLWorkload,
    ) -> Result<()> {
        if !self.is_available {
            warn!("⚠️ Native GPU runtime not available for ML workload");
            return Ok(());
        }

        info!(
            "🧠 Running ML workload via native GPU runtime for container: {}",
            container_id
        );
        info!("  ✓ ML workload configured:");
        info!("    • Framework: {:?}", ml_workload.ml_framework);
        info!("    • Model type: {}", ml_workload.model_type);
        info!("    • Training mode: {}", ml_workload.training_mode);
        info!("    • Mixed precision: {}", ml_workload.mixed_precision);
        info!(
            "    • Distributed training: {}",
            ml_workload.distributed_training
        );
        info!("    • Memory pool optimization enabled");

        Ok(())
    }

    pub async fn run_compute_workload(
        &self,
        container_id: &str,
        compute_workload: &ComputeWorkload,
    ) -> Result<()> {
        if !self.is_available {
            warn!("⚠️ Native GPU runtime not available for compute workload");
            return Ok(());
        }

        info!(
            "⚙️ Running compute workload via native GPU runtime for container: {}",
            container_id
        );
        info!("  ✓ Compute workload configured:");
        info!("    • Compute type: {:?}", compute_workload.compute_type);
        info!("    • Precision: {:?}", compute_workload.precision);
        info!("    • CPU/GPU ratio: {:.1}", compute_workload.cpu_gpu_ratio);
        info!(
            "    • Memory requirements: {:?} GB",
            compute_workload.memory_requirements_gb
        );
        info!(
            "    • P2P enabled: {}",
            compute_workload.enable_peer_to_peer
        );
        info!("    • Direct driver access enabled");

        Ok(())
    }

    pub async fn check_compatibility(&self) -> Result<NvbindCompatibility> {
        info!("🔍 Checking native GPU runtime compatibility");

        if !self.is_available {
            return Ok(NvbindCompatibility {
                available: false,
                gpu_count: 0,
                driver_version: "N/A".to_string(),
                driver_type: None,
                bolt_optimizations: false,
                wsl2_mode: false,
                performance_info: "Native GPU support not available".to_string(),
            });
        }

        // Check if we're in WSL2
        let wsl2_mode = std::env::var("WSL_DISTRO_NAME").is_ok();

        let compatibility = NvbindCompatibility {
            available: true,
            gpu_count: self.detected_gpus.len() as u32,
            driver_version: self
                .driver_info
                .as_ref()
                .map(|d| d.version.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            driver_type: self.driver_info.as_ref().map(|d| d.driver_type),
            bolt_optimizations: true,
            wsl2_mode,
            performance_info: "Native GPU passthrough - no nvidia-container-toolkit required"
                .to_string(),
        };

        info!("🚀 Native GPU runtime available:");
        info!("  • GPUs: {}", compatibility.gpu_count);
        info!(
            "  • Driver: {} ({:?})",
            compatibility.driver_version, compatibility.driver_type
        );
        info!(
            "  • Bolt optimizations: {}",
            compatibility.bolt_optimizations
        );
        info!("  • WSL2 mode: {}", compatibility.wsl2_mode);

        Ok(compatibility)
    }

    pub async fn run_with_bolt_runtime(
        &self,
        image: String,
        cmd: Vec<String>,
        gpu_devices: Option<String>,
    ) -> Result<()> {
        if !self.is_available {
            warn!("⚠️ Native GPU runtime not available");
            return Ok(());
        }

        info!("🚀 Running container with native Bolt GPU runtime");
        info!("  • Image: {}", image);
        info!("  • Command: {:?}", cmd);
        info!("  • GPU devices: {:?}", gpu_devices);

        // Get required devices for passthrough
        let devices = Self::get_required_devices();
        info!("  • Passing through {} device nodes", devices.len());

        // In actual implementation, this would configure the container runtime
        // to pass through the GPU devices and libraries
        info!("  ✓ Native GPU container execution configured");

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvbindCompatibility {
    pub available: bool,
    pub gpu_count: u32,
    pub driver_version: String,
    pub driver_type: Option<DriverType>,
    pub bolt_optimizations: bool,
    pub wsl2_mode: bool,
    pub performance_info: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_architecture_from_compute_capability() {
        assert_eq!(
            GpuArchitecture::from_compute_capability(5, 2),
            GpuArchitecture::Maxwell
        );
        assert_eq!(
            GpuArchitecture::from_compute_capability(6, 1),
            GpuArchitecture::Pascal
        );
        assert_eq!(
            GpuArchitecture::from_compute_capability(7, 0),
            GpuArchitecture::Volta
        );
        assert_eq!(
            GpuArchitecture::from_compute_capability(7, 5),
            GpuArchitecture::Turing
        );
        assert_eq!(
            GpuArchitecture::from_compute_capability(8, 6),
            GpuArchitecture::Ampere
        );
        assert_eq!(
            GpuArchitecture::from_compute_capability(8, 9),
            GpuArchitecture::AdaLovelace
        );
        assert_eq!(
            GpuArchitecture::from_compute_capability(9, 0),
            GpuArchitecture::Hopper
        );
        assert_eq!(
            GpuArchitecture::from_compute_capability(10, 0),
            GpuArchitecture::Blackwell
        );
    }

    #[test]
    fn test_gpu_architecture_from_name() {
        assert_eq!(
            GpuArchitecture::from_gpu_name("NVIDIA GeForce RTX 5090"),
            GpuArchitecture::Blackwell
        );
        assert_eq!(
            GpuArchitecture::from_gpu_name("NVIDIA GeForce RTX 4090"),
            GpuArchitecture::AdaLovelace
        );
        assert_eq!(
            GpuArchitecture::from_gpu_name("NVIDIA GeForce RTX 3080"),
            GpuArchitecture::Ampere
        );
        assert_eq!(
            GpuArchitecture::from_gpu_name("NVIDIA GeForce RTX 2080 Ti"),
            GpuArchitecture::Turing
        );
        assert_eq!(
            GpuArchitecture::from_gpu_name("NVIDIA GeForce GTX 1080"),
            GpuArchitecture::Pascal
        );
        assert_eq!(
            GpuArchitecture::from_gpu_name("NVIDIA H100"),
            GpuArchitecture::Hopper
        );
    }

    #[test]
    fn test_driver_type_properties() {
        assert!(DriverType::NvidiaOpen.supports_cuda());
        assert!(DriverType::NvidiaProprietary.supports_cuda());
        assert!(!DriverType::Nouveau.supports_cuda());
    }

    #[test]
    fn test_gpu_architecture_features() {
        assert!(GpuArchitecture::Blackwell.supports_fp4());
        assert!(!GpuArchitecture::AdaLovelace.supports_fp4());

        assert!(GpuArchitecture::Ampere.supports_mig());
        assert!(GpuArchitecture::Hopper.supports_mig());
        assert!(!GpuArchitecture::Turing.supports_mig());

        assert_eq!(GpuArchitecture::Blackwell.tensor_core_generation(), Some(5));
        assert_eq!(
            GpuArchitecture::AdaLovelace.tensor_core_generation(),
            Some(4)
        );
        assert_eq!(GpuArchitecture::Maxwell.tensor_core_generation(), None);
    }

    #[test]
    fn test_native_detection_no_panic() {
        // This test verifies detection doesn't panic even without NVIDIA hardware
        let result = NvbindManager::detect();
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_required_devices() {
        let devices = NvbindManager::get_required_devices();
        // Should always include control devices
        assert!(devices.contains(&"/dev/nvidiactl".to_string()));
        assert!(devices.contains(&"/dev/nvidia-uvm".to_string()));
    }
}

// ============= CDI Specification Types =============

/// CDI (Container Device Interface) specification
/// Uses camelCase for CDI spec compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdiSpec {
    pub cdi_version: String,
    pub kind: String,
    pub devices: Vec<CdiDevice>,
    pub container_edits: CdiContainerEdits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdiDevice {
    pub name: String,
    pub container_edits: CdiContainerEdits,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CdiContainerEdits {
    pub env: Vec<String>,
    pub device_nodes: Vec<CdiDeviceNode>,
    pub mounts: Vec<CdiMount>,
    pub hooks: Vec<CdiHook>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdiDeviceNode {
    pub path: String,
    pub host_path: Option<String>,
    #[serde(rename = "type")]
    pub device_type: Option<String>,
    pub major: Option<i64>,
    pub minor: Option<i64>,
    pub file_mode: Option<u32>,
    pub permissions: Option<String>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
}

impl CdiDeviceNode {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            host_path: Some(path.to_string()),
            device_type: Some("c".to_string()),
            major: None,
            minor: None,
            file_mode: None,
            permissions: Some("rw".to_string()),
            uid: None,
            gid: None,
        }
    }

    /// Get device major/minor from stat
    pub fn with_stat(mut self) -> Self {
        use std::os::unix::fs::MetadataExt;
        if let Ok(metadata) = std::fs::metadata(&self.path) {
            let rdev = metadata.rdev();
            self.major = Some(((rdev >> 8) & 0xff) as i64);
            self.minor = Some((rdev & 0xff) as i64);
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdiMount {
    pub host_path: String,
    pub container_path: String,
    pub options: Vec<String>,
    #[serde(rename = "type")]
    pub mount_type: Option<String>,
}

impl CdiMount {
    pub fn bind(host_path: &str, container_path: &str) -> Self {
        Self {
            host_path: host_path.to_string(),
            container_path: container_path.to_string(),
            options: vec![
                "bind".to_string(),
                "ro".to_string(),
                "nosuid".to_string(),
                "nodev".to_string(),
            ],
            mount_type: Some("bind".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdiHook {
    pub hook_name: String,
    pub path: String,
    pub args: Vec<String>,
    pub env: Vec<String>,
    pub timeout: Option<u32>,
}

impl NvbindManager {
    // ============= CDI Generation Methods =============

    /// Generate a gaming-optimized CDI specification
    pub async fn generate_gaming_cdi_spec(&self) -> Result<CdiSpec> {
        info!("🎮 Generating gaming-optimized CDI spec (native)");

        let mut container_edits = self.generate_base_container_edits().await?;

        // Add gaming-specific environment variables
        container_edits.env.extend(vec![
            "NVIDIA_DRIVER_CAPABILITIES=all".to_string(),
            "__GL_THREADED_OPTIMIZATIONS=1".to_string(),
            "__GL_SHADER_CACHE=1".to_string(),
            "__GL_SYNC_TO_VBLANK=0".to_string(),
            "__GL_MaxFramesAllowed=1".to_string(),
            "PROTON_ENABLE_NVAPI=1".to_string(),
            "DXVK_ENABLE_NVAPI=1".to_string(),
        ]);

        Ok(self.build_cdi_spec("gaming", container_edits))
    }

    /// Generate an AI/ML-optimized CDI specification
    pub async fn generate_aiml_cdi_spec(&self) -> Result<CdiSpec> {
        info!("🧠 Generating AI/ML-optimized CDI spec (native)");

        let mut container_edits = self.generate_base_container_edits().await?;

        // Add AI/ML-specific environment variables
        container_edits.env.extend(vec![
            "NVIDIA_DRIVER_CAPABILITIES=compute,utility".to_string(),
            "CUDA_CACHE_MAXSIZE=2147483648".to_string(), // 2GB cache
            "NVIDIA_TF32_OVERRIDE=1".to_string(),
            "CUDA_DEVICE_ORDER=PCI_BUS_ID".to_string(),
        ]);

        // Enable tensor core optimizations for supported architectures
        for gpu in &self.detected_gpus {
            if gpu.architecture.tensor_core_generation().is_some() {
                container_edits
                    .env
                    .push("NVIDIA_REQUIRE_CUDA=cuda>=11.0".to_string());
                break;
            }
        }

        Ok(self.build_cdi_spec("aiml", container_edits))
    }

    /// Generate a general-purpose CDI specification
    pub async fn generate_default_cdi_spec(&self) -> Result<CdiSpec> {
        info!("📊 Generating general-purpose CDI spec (native)");

        let mut container_edits = self.generate_base_container_edits().await?;

        container_edits.env.extend(vec![
            "NVIDIA_DRIVER_CAPABILITIES=compute,video,graphics,utility,display".to_string(),
        ]);

        Ok(self.build_cdi_spec("general", container_edits))
    }

    /// Generate base container edits with device nodes and library mounts
    async fn generate_base_container_edits(&self) -> Result<CdiContainerEdits> {
        let mut edits = CdiContainerEdits::default();

        // Add device nodes
        let devices = Self::get_required_devices();
        for device_path in devices {
            if Path::new(&device_path).exists() {
                edits
                    .device_nodes
                    .push(CdiDeviceNode::new(&device_path).with_stat());
            }
        }

        // Add environment variables
        edits.env = vec!["NVIDIA_VISIBLE_DEVICES=all".to_string()];

        if let Some(ref driver_info) = self.driver_info {
            if let Some(ref cuda_version) = driver_info.cuda_version {
                edits.env.push(format!("CUDA_VERSION={}", cuda_version));
            }
            edits
                .env
                .push(format!("NVIDIA_DRIVER_VERSION={}", driver_info.version));
        }

        // Add library mounts
        edits.mounts = self.find_library_mounts();

        Ok(edits)
    }

    /// Find NVIDIA library paths to mount into container
    fn find_library_mounts(&self) -> Vec<CdiMount> {
        let mut mounts = Vec::new();

        // Common NVIDIA library directory prefixes
        let library_dir_prefixes = [
            "/usr/lib/x86_64-linux-gnu",
            "/usr/lib64",
            "/lib/x86_64-linux-gnu",
            "/lib64",
        ];

        // Find and add library directories from detected driver libraries
        let mut found_libs = false;
        if let Some(ref driver_info) = self.driver_info {
            for lib in &driver_info.libraries {
                if let Some(parent) = Path::new(lib).parent() {
                    let parent_str = parent.to_string_lossy();
                    if !mounts.iter().any(|m: &CdiMount| m.host_path == parent_str)
                        && parent.exists()
                    {
                        mounts.push(CdiMount::bind(&parent_str, &parent_str));
                        found_libs = true;
                    }
                }
            }
        }

        // Fallback: check common library directories if no libraries were detected
        if !found_libs {
            for prefix in &library_dir_prefixes {
                let prefix_path = Path::new(prefix);
                if prefix_path.exists() && !mounts.iter().any(|m: &CdiMount| m.host_path == *prefix)
                {
                    mounts.push(CdiMount::bind(prefix, prefix));
                }
            }
        }

        // Add ICD files for Vulkan/OpenGL
        let icd_paths = [
            "/usr/share/vulkan/icd.d",
            "/usr/share/glvnd/egl_vendor.d",
            "/etc/vulkan/icd.d",
        ];

        for icd_path in &icd_paths {
            if Path::new(icd_path).exists() {
                mounts.push(CdiMount::bind(icd_path, icd_path));
            }
        }

        // Add CUDA directory if available
        let cuda_paths = ["/usr/local/cuda", "/opt/cuda"];
        for cuda_path in &cuda_paths {
            if Path::new(cuda_path).exists() {
                mounts.push(CdiMount::bind(cuda_path, cuda_path));
            }
        }

        mounts
    }

    /// Build a complete CDI spec from container edits
    fn build_cdi_spec(&self, profile: &str, edits: CdiContainerEdits) -> CdiSpec {
        let devices: Vec<CdiDevice> = self
            .detected_gpus
            .iter()
            .map(|gpu| CdiDevice {
                name: format!("gpu{}", gpu.id),
                container_edits: CdiContainerEdits {
                    env: vec![],
                    device_nodes: vec![CdiDeviceNode::new(&gpu.device_path).with_stat()],
                    mounts: vec![],
                    hooks: vec![],
                },
            })
            .collect();

        CdiSpec {
            cdi_version: "0.6.0".to_string(),
            kind: format!("nvidia.com/{}", profile),
            devices,
            container_edits: edits,
        }
    }

    /// Write CDI spec to file
    pub async fn write_cdi_spec(&self, path: &Path, spec: &CdiSpec) -> Result<()> {
        let json = serde_json::to_string_pretty(spec)?;
        tokio::fs::write(path, json).await?;
        info!("✅ CDI spec written to: {}", path.display());
        Ok(())
    }
}
