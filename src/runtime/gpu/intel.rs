//! Intel Arc GPU management
//!
//! Native Intel Arc GPU detection and management with oneAPI/Level Zero integration.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use tracing::info;

/// Intel GPU Architecture generations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntelArchitecture {
    /// Xe-LP (Integrated) - Tiger Lake, Alder Lake, Raptor Lake
    XeLP,
    /// Xe-HPG (Arc) - Alchemist (DG2)
    XeHPG,
    /// Xe-HPC (Data Center) - Ponte Vecchio
    XeHPC,
    /// Xe2-LPG (Lunar Lake, Battlemage)
    Xe2LPG,
    /// Xe2-HPG (Battlemage discrete)
    Xe2HPG,
    /// Gen9 (Skylake, Kaby Lake)
    Gen9,
    /// Gen11 (Ice Lake)
    Gen11,
    /// Gen12 (Tiger Lake integrated)
    Gen12,
    /// Unknown architecture
    Unknown,
}

impl IntelArchitecture {
    pub fn name(&self) -> &'static str {
        match self {
            IntelArchitecture::XeLP => "Xe-LP (Integrated)",
            IntelArchitecture::XeHPG => "Xe-HPG (Arc Alchemist)",
            IntelArchitecture::XeHPC => "Xe-HPC (Data Center)",
            IntelArchitecture::Xe2LPG => "Xe2-LPG (Lunar Lake)",
            IntelArchitecture::Xe2HPG => "Xe2-HPG (Battlemage)",
            IntelArchitecture::Gen9 => "Gen9 (Skylake/Kaby Lake)",
            IntelArchitecture::Gen11 => "Gen11 (Ice Lake)",
            IntelArchitecture::Gen12 => "Gen12 (Tiger Lake)",
            IntelArchitecture::Unknown => "Unknown",
        }
    }

    pub fn supports_raytracing(&self) -> bool {
        matches!(
            self,
            IntelArchitecture::XeHPG | IntelArchitecture::Xe2LPG | IntelArchitecture::Xe2HPG
        )
    }

    pub fn supports_xess(&self) -> bool {
        matches!(
            self,
            IntelArchitecture::XeHPG | IntelArchitecture::Xe2LPG | IntelArchitecture::Xe2HPG
        )
    }

    pub fn supports_oneapi(&self) -> bool {
        // oneAPI supports most Intel GPUs Gen9+
        !matches!(self, IntelArchitecture::Unknown)
    }

    /// Detect architecture from GPU name
    pub fn from_gpu_name(name: &str) -> Self {
        let name_lower = name.to_lowercase();

        // Xe2 / Battlemage
        if name_lower.contains("battlemage") || name_lower.contains("xe2") {
            if name_lower.contains("arc") {
                return IntelArchitecture::Xe2HPG;
            }
            return IntelArchitecture::Xe2LPG;
        }

        // Arc Alchemist (A770, A750, A580, A380, A310)
        if name_lower.contains("arc")
            && (name_lower.contains("a7") || name_lower.contains("a5") || name_lower.contains("a3"))
        {
            return IntelArchitecture::XeHPG;
        }

        // Lunar Lake
        if name_lower.contains("lunar") || name_lower.contains("core ultra") {
            return IntelArchitecture::Xe2LPG;
        }

        // Ponte Vecchio
        if name_lower.contains("max") || name_lower.contains("ponte vecchio") {
            return IntelArchitecture::XeHPC;
        }

        // Integrated graphics detection
        if name_lower.contains("iris xe") || name_lower.contains("uhd 7") {
            return IntelArchitecture::XeLP;
        }

        if name_lower.contains("iris plus") || name_lower.contains("uhd 6") {
            return IntelArchitecture::Gen11;
        }

        if name_lower.contains("uhd 630") || name_lower.contains("uhd 620") {
            return IntelArchitecture::Gen9;
        }

        IntelArchitecture::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelManager {
    pub is_available: bool,
    pub driver_version: String,
    pub driver_type: IntelDriverType,
    pub oneapi_version: Option<String>,
    pub level_zero_version: Option<String>,
    pub gpus: Vec<IntelGPU>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntelDriverType {
    /// i915 kernel driver
    I915,
    /// Xe kernel driver (new)
    Xe,
    /// Unknown
    Unknown,
}

impl IntelDriverType {
    pub fn name(&self) -> &'static str {
        match self {
            IntelDriverType::I915 => "i915 (Legacy)",
            IntelDriverType::Xe => "Xe (Modern)",
            IntelDriverType::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelGPU {
    pub index: u32,
    pub name: String,
    pub memory_mb: u32,
    pub device_id: String,
    pub pci_bus_id: String,
    pub architecture: IntelArchitecture,
    pub device_path: String,
    pub render_path: String,
    pub is_discrete: bool,
}

impl IntelManager {
    pub fn detect() -> Result<Self> {
        info!("🔍 Detecting Intel GPU configuration (native)");

        let mut gpus = Vec::new();

        // Method 1: Check /sys/class/drm for Intel GPUs
        if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("card")
                    && !name.contains("-")
                    && let Some(gpu) = Self::detect_gpu_from_drm(&name)
                {
                    gpus.push(gpu);
                }
            }
        }

        // Method 2: Fallback to lspci if no GPUs found via /sys
        if gpus.is_empty()
            && let Ok(output) = Command::new("lspci").arg("-nn").output()
        {
            let output_str = String::from_utf8_lossy(&output.stdout);
            let mut index = 0;
            for line in output_str.lines() {
                if line.to_lowercase().contains("intel")
                    && (line.to_lowercase().contains("vga")
                        || line.to_lowercase().contains("display")
                        || line.to_lowercase().contains("3d"))
                {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(pci_id) = parts.first() {
                        let gpu_name = Self::extract_gpu_name(line);
                        let is_discrete = gpu_name.to_lowercase().contains("arc");
                        let gpu = IntelGPU {
                            index,
                            name: gpu_name.clone(),
                            memory_mb: Self::get_gpu_memory(index).unwrap_or(0),
                            device_id: pci_id.to_string(),
                            pci_bus_id: pci_id.to_string(),
                            architecture: IntelArchitecture::from_gpu_name(&gpu_name),
                            device_path: format!("/dev/dri/card{}", index),
                            render_path: format!("/dev/dri/renderD{}", 128 + index),
                            is_discrete,
                        };
                        gpus.push(gpu);
                        index += 1;
                    }
                }
            }
        }

        let driver_version = Self::get_driver_version().unwrap_or_else(|_| "unknown".to_string());
        let driver_type = Self::detect_driver_type();
        let oneapi_version = Self::get_oneapi_version().ok();
        let level_zero_version = Self::get_level_zero_version().ok();
        let is_available = !gpus.is_empty();

        if is_available {
            info!("✅ Native Intel GPU detection successful");
            info!("  • Driver: {} ({})", driver_version, driver_type.name());
            if let Some(ref oneapi) = oneapi_version {
                info!("  • oneAPI: {}", oneapi);
            }
            if let Some(ref l0) = level_zero_version {
                info!("  • Level Zero: {}", l0);
            }
            info!("  • GPUs detected: {}", gpus.len());
            for gpu in &gpus {
                let gpu_type = if gpu.is_discrete {
                    "discrete"
                } else {
                    "integrated"
                };
                info!(
                    "    - GPU {}: {} ({:?}, {})",
                    gpu.index, gpu.name, gpu.architecture, gpu_type
                );
            }
        } else {
            info!("ℹ️  No Intel GPUs detected");
        }

        Ok(Self {
            is_available,
            driver_version,
            driver_type,
            oneapi_version,
            level_zero_version,
            gpus,
        })
    }

    /// Detect Intel GPU from /sys/class/drm entry
    fn detect_gpu_from_drm(card_name: &str) -> Option<IntelGPU> {
        let base_path = format!("/sys/class/drm/{}/device", card_name);
        let base = Path::new(&base_path);

        // Check if this is an Intel GPU
        let vendor_path = base.join("vendor");
        if let Ok(vendor) = std::fs::read_to_string(&vendor_path) {
            let vendor = vendor.trim();
            // Intel vendor ID is 0x8086
            if vendor != "0x8086" {
                return None;
            }
        } else {
            return None;
        }

        // Get device ID
        let device_id = std::fs::read_to_string(base.join("device"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        // Get PCI address
        let pci_bus_id = std::fs::read_to_string(base.join("uevent"))
            .ok()
            .and_then(|content| {
                for line in content.lines() {
                    if line.starts_with("PCI_SLOT_NAME=") {
                        return Some(line.replace("PCI_SLOT_NAME=", ""));
                    }
                }
                None
            })
            .unwrap_or_default();

        // Get GPU name
        let gpu_name = Self::get_gpu_name_from_sysfs(&base_path)
            .or_else(|| Self::get_gpu_name_from_lspci(&pci_bus_id))
            .unwrap_or_else(|| "Intel GPU".to_string());

        // Check if discrete (Arc)
        let is_discrete = gpu_name.to_lowercase().contains("arc")
            || gpu_name.to_lowercase().contains("dg2")
            || gpu_name.to_lowercase().contains("battlemage");

        // Get card index
        let index: u32 = card_name
            .strip_prefix("card")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Get memory
        let memory_mb = Self::get_gpu_memory(index).unwrap_or(0);

        Some(IntelGPU {
            index,
            name: gpu_name.clone(),
            memory_mb,
            device_id,
            pci_bus_id,
            architecture: IntelArchitecture::from_gpu_name(&gpu_name),
            device_path: format!("/dev/dri/card{}", index),
            render_path: format!("/dev/dri/renderD{}", 128 + index),
            is_discrete,
        })
    }

    fn get_gpu_name_from_sysfs(base_path: &str) -> Option<String> {
        let paths = [
            format!("{}/product_name", base_path),
            format!("{}/label", base_path),
        ];

        for path in &paths {
            if let Ok(name) = std::fs::read_to_string(path) {
                let name = name.trim();
                if !name.is_empty() && name != "Unknown" {
                    return Some(name.to_string());
                }
            }
        }

        None
    }

    fn get_gpu_name_from_lspci(pci_bus_id: &str) -> Option<String> {
        if pci_bus_id.is_empty() {
            return None;
        }

        if let Ok(output) = Command::new("lspci").arg("-s").arg(pci_bus_id).output() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                if let Some(start) = line.find(": ") {
                    return Some(line[start + 2..].trim().to_string());
                }
            }
        }

        None
    }

    fn extract_gpu_name(line: &str) -> String {
        if let Some(start) = line.find(": ") {
            line[start + 2..]
                .split('[')
                .next()
                .unwrap_or("Unknown Intel GPU")
                .trim()
                .to_string()
        } else {
            "Unknown Intel GPU".to_string()
        }
    }

    fn get_gpu_memory(index: u32) -> Result<u32> {
        // Try to get memory from /sys/class/drm
        let mem_path = format!("/sys/class/drm/card{}/device/resource0", index);
        if let Ok(metadata) = std::fs::metadata(&mem_path) {
            // Resource0 size gives approximate VRAM for discrete
            let size = metadata.len();
            if size > 0 {
                return Ok((size / 1024 / 1024) as u32);
            }
        }

        // Fallback: Check lmem (local memory) for discrete GPUs
        let lmem_path = format!("/sys/class/drm/card{}/lmem_total_bytes", index);
        if let Ok(mem_str) = std::fs::read_to_string(&lmem_path)
            && let Ok(bytes) = mem_str.trim().parse::<u64>()
        {
            return Ok((bytes / 1024 / 1024) as u32);
        }

        // Default for integrated (shared memory)
        Ok(0)
    }

    fn detect_driver_type() -> IntelDriverType {
        // Check for xe module (modern driver)
        if let Ok(modules) = std::fs::read_to_string("/proc/modules") {
            if modules.contains("xe ") {
                return IntelDriverType::Xe;
            }
            if modules.contains("i915 ") {
                return IntelDriverType::I915;
            }
        }

        IntelDriverType::Unknown
    }

    fn get_driver_version() -> Result<String> {
        // Try modinfo for xe driver first
        if let Ok(output) = Command::new("modinfo").arg("xe").output() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                if line.starts_with("version:") {
                    return Ok(line
                        .split(':')
                        .nth(1)
                        .unwrap_or("unknown")
                        .trim()
                        .to_string());
                }
            }
        }

        // Fallback to i915
        if let Ok(output) = Command::new("modinfo").arg("i915").output() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                if line.starts_with("version:") {
                    return Ok(line
                        .split(':')
                        .nth(1)
                        .unwrap_or("unknown")
                        .trim()
                        .to_string());
                }
            }
        }

        Ok("unknown".to_string())
    }

    fn get_oneapi_version() -> Result<String> {
        // Check for oneAPI installation
        let oneapi_paths = [
            "/opt/intel/oneapi/compiler/latest/env/vars.sh",
            "/opt/intel/oneapi/setvars.sh",
        ];

        for path in &oneapi_paths {
            if Path::new(path).exists() {
                // Try to get version from release file
                if let Ok(version) =
                    std::fs::read_to_string("/opt/intel/oneapi/compiler/latest/lib/version")
                {
                    return Ok(version.trim().to_string());
                }
                return Ok("installed".to_string());
            }
        }

        Err(anyhow::anyhow!("oneAPI not found"))
    }

    fn get_level_zero_version() -> Result<String> {
        // Check for Level Zero
        if let Ok(output) = Command::new("pkg-config")
            .args(["--modversion", "level-zero"])
            .output()
            && output.status.success()
        {
            return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }

        // Check for ze_loader library
        let l0_paths = [
            "/usr/lib/x86_64-linux-gnu/libze_loader.so",
            "/usr/lib64/libze_loader.so",
        ];

        for path in &l0_paths {
            if Path::new(path).exists() {
                return Ok("installed".to_string());
            }
        }

        Err(anyhow::anyhow!("Level Zero not found"))
    }

    // ============= CDI Generation Methods =============

    /// Get required device paths for Intel GPU passthrough
    pub fn get_required_devices(&self) -> Vec<String> {
        let mut devices = Vec::new();

        // Add DRI devices for each GPU
        for gpu in &self.gpus {
            devices.push(gpu.device_path.clone());
            devices.push(gpu.render_path.clone());
        }

        devices
    }

    /// Generate a gaming-optimized CDI specification
    pub async fn generate_gaming_cdi_spec(&self) -> Result<IntelCdiSpec> {
        info!("🎮 Generating gaming-optimized CDI spec for Intel (native)");

        let mut env = self.get_base_env();
        env.extend(vec![
            "MESA_VK_DEVICE_SELECT=list".to_string(),
            "ANV_ENABLE_PIPELINE_CACHE=1".to_string(),
            "INTEL_DEBUG=".to_string(), // Disable debug for performance
        ]);

        // Add XeSS support for Arc GPUs
        if self.gpus.iter().any(|g| g.architecture.supports_xess()) {
            env.push("ENABLE_XESS=1".to_string());
        }

        Ok(self.build_cdi_spec("gaming", env))
    }

    /// Generate an AI/ML-optimized CDI specification
    pub async fn generate_aiml_cdi_spec(&self) -> Result<IntelCdiSpec> {
        info!("🧠 Generating AI/ML-optimized CDI spec for Intel (native)");

        let mut env = self.get_base_env();

        // Level Zero / oneAPI settings
        if self.level_zero_version.is_some() {
            env.extend(vec![
                "ZE_AFFINITY_MASK=0".to_string(),
                "ZE_ENABLE_PCI_ID_DEVICE_ORDER=1".to_string(),
            ]);
        }

        if self.oneapi_version.is_some() {
            env.push("ONEAPI_DEVICE_SELECTOR=level_zero:*".to_string());
        }

        Ok(self.build_cdi_spec("aiml", env))
    }

    /// Generate a general-purpose CDI specification
    pub async fn generate_default_cdi_spec(&self) -> Result<IntelCdiSpec> {
        info!("📊 Generating general-purpose CDI spec for Intel (native)");

        let env = self.get_base_env();
        Ok(self.build_cdi_spec("general", env))
    }

    fn get_base_env(&self) -> Vec<String> {
        let mut env = vec!["INTEL_VISIBLE_DEVICES=all".to_string()];

        if let Some(ref l0) = self.level_zero_version {
            env.push(format!("LEVEL_ZERO_VERSION={}", l0));
        }

        env
    }

    fn build_cdi_spec(&self, profile: &str, env: Vec<String>) -> IntelCdiSpec {
        let devices: Vec<IntelCdiDevice> = self
            .gpus
            .iter()
            .map(|gpu| IntelCdiDevice {
                name: format!("gpu{}", gpu.index),
                container_edits: IntelCdiContainerEdits {
                    env: vec![],
                    device_nodes: vec![
                        IntelCdiDeviceNode::new(&gpu.device_path),
                        IntelCdiDeviceNode::new(&gpu.render_path),
                    ],
                    mounts: vec![],
                },
            })
            .collect();

        // Build container edits with all devices
        let mut device_nodes: Vec<IntelCdiDeviceNode> = self
            .get_required_devices()
            .iter()
            .map(|path| IntelCdiDeviceNode::new(path))
            .collect();

        device_nodes.dedup_by(|a, b| a.path == b.path);

        let mounts = self.find_library_mounts();

        IntelCdiSpec {
            cdi_version: "0.6.0".to_string(),
            kind: format!("intel.com/{}", profile),
            devices,
            container_edits: IntelCdiContainerEdits {
                env,
                device_nodes,
                mounts,
            },
        }
    }

    fn find_library_mounts(&self) -> Vec<IntelCdiMount> {
        let mut mounts = Vec::new();

        // Vulkan ICD files
        let icd_paths = ["/usr/share/vulkan/icd.d", "/etc/vulkan/icd.d"];

        for path in &icd_paths {
            if Path::new(path).exists() {
                mounts.push(IntelCdiMount {
                    host_path: path.to_string(),
                    container_path: path.to_string(),
                    options: vec!["bind".to_string(), "ro".to_string()],
                });
            }
        }

        // oneAPI libraries
        if self.oneapi_version.is_some() {
            let oneapi_paths = ["/opt/intel/oneapi"];
            for path in &oneapi_paths {
                if Path::new(path).exists() {
                    mounts.push(IntelCdiMount {
                        host_path: path.to_string(),
                        container_path: path.to_string(),
                        options: vec!["bind".to_string(), "ro".to_string()],
                    });
                }
            }
        }

        // Mesa / Intel driver libraries
        let driver_paths = ["/usr/lib/x86_64-linux-gnu/dri", "/usr/lib64/dri"];
        for path in &driver_paths {
            if Path::new(path).exists() {
                mounts.push(IntelCdiMount {
                    host_path: path.to_string(),
                    container_path: path.to_string(),
                    options: vec!["bind".to_string(), "ro".to_string()],
                });
            }
        }

        mounts
    }
}

// ============= Intel CDI Specification Types =============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelCdiSpec {
    pub cdi_version: String,
    pub kind: String,
    pub devices: Vec<IntelCdiDevice>,
    pub container_edits: IntelCdiContainerEdits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelCdiDevice {
    pub name: String,
    pub container_edits: IntelCdiContainerEdits,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelCdiContainerEdits {
    pub env: Vec<String>,
    pub device_nodes: Vec<IntelCdiDeviceNode>,
    pub mounts: Vec<IntelCdiMount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelCdiDeviceNode {
    pub path: String,
    pub host_path: Option<String>,
    #[serde(rename = "type")]
    pub device_type: Option<String>,
    pub permissions: Option<String>,
}

impl IntelCdiDeviceNode {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            host_path: Some(path.to_string()),
            device_type: Some("c".to_string()),
            permissions: Some("rw".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelCdiMount {
    pub host_path: String,
    pub container_path: String,
    pub options: Vec<String>,
}
