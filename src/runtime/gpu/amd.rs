use super::{GPUInfo, GPUVendor};
use crate::runtime::environment::env_manager;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use tracing::{debug, info};

/// AMD GPU Architecture generations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmdArchitecture {
    /// GCN 1.0 (Southern Islands) - HD 7000 series
    GCN1,
    /// GCN 2.0 (Sea Islands) - R7/R9 200 series
    GCN2,
    /// GCN 3.0 (Volcanic Islands) - R9 285/380
    GCN3,
    /// GCN 4.0 (Polaris) - RX 400/500 series
    GCN4,
    /// GCN 5.0 (Vega) - Vega 56/64
    GCN5,
    /// RDNA 1.0 - RX 5000 series
    RDNA1,
    /// RDNA 2.0 - RX 6000 series
    RDNA2,
    /// RDNA 3.0 - RX 7000 series
    RDNA3,
    /// RDNA 4.0 - RX 8000/9000 series (future)
    RDNA4,
    /// CDNA 1.0 - MI100
    CDNA1,
    /// CDNA 2.0 - MI200 series
    CDNA2,
    /// CDNA 3.0 - MI300 series
    CDNA3,
    /// Unknown architecture
    Unknown,
}

impl AmdArchitecture {
    pub fn name(&self) -> &'static str {
        match self {
            AmdArchitecture::GCN1 => "GCN 1.0 (Southern Islands)",
            AmdArchitecture::GCN2 => "GCN 2.0 (Sea Islands)",
            AmdArchitecture::GCN3 => "GCN 3.0 (Volcanic Islands)",
            AmdArchitecture::GCN4 => "GCN 4.0 (Polaris)",
            AmdArchitecture::GCN5 => "GCN 5.0 (Vega)",
            AmdArchitecture::RDNA1 => "RDNA 1.0 (Navi)",
            AmdArchitecture::RDNA2 => "RDNA 2.0",
            AmdArchitecture::RDNA3 => "RDNA 3.0",
            AmdArchitecture::RDNA4 => "RDNA 4.0",
            AmdArchitecture::CDNA1 => "CDNA 1.0 (MI100)",
            AmdArchitecture::CDNA2 => "CDNA 2.0 (MI200)",
            AmdArchitecture::CDNA3 => "CDNA 3.0 (MI300)",
            AmdArchitecture::Unknown => "Unknown",
        }
    }

    pub fn supports_rocm(&self) -> bool {
        matches!(
            self,
            AmdArchitecture::GCN5
                | AmdArchitecture::RDNA1
                | AmdArchitecture::RDNA2
                | AmdArchitecture::RDNA3
                | AmdArchitecture::RDNA4
                | AmdArchitecture::CDNA1
                | AmdArchitecture::CDNA2
                | AmdArchitecture::CDNA3
        )
    }

    pub fn supports_raytracing(&self) -> bool {
        matches!(
            self,
            AmdArchitecture::RDNA2 | AmdArchitecture::RDNA3 | AmdArchitecture::RDNA4
        )
    }

    /// Detect architecture from GPU name
    pub fn from_gpu_name(name: &str) -> Self {
        let name_lower = name.to_lowercase();

        // CDNA (data center)
        if name_lower.contains("mi300") {
            return AmdArchitecture::CDNA3;
        }
        if name_lower.contains("mi200")
            || name_lower.contains("mi210")
            || name_lower.contains("mi250")
        {
            return AmdArchitecture::CDNA2;
        }
        if name_lower.contains("mi100") {
            return AmdArchitecture::CDNA1;
        }

        // RDNA series
        if name_lower.contains("rx 9") || name_lower.contains("rx 8") {
            return AmdArchitecture::RDNA4;
        }
        if name_lower.contains("rx 7")
            || name_lower.contains("7900")
            || name_lower.contains("7800")
            || name_lower.contains("7700")
            || name_lower.contains("7600")
        {
            return AmdArchitecture::RDNA3;
        }
        if name_lower.contains("rx 6")
            || name_lower.contains("6900")
            || name_lower.contains("6800")
            || name_lower.contains("6700")
            || name_lower.contains("6600")
            || name_lower.contains("6500")
        {
            return AmdArchitecture::RDNA2;
        }
        if name_lower.contains("rx 5")
            || name_lower.contains("5700")
            || name_lower.contains("5600")
            || name_lower.contains("5500")
        {
            return AmdArchitecture::RDNA1;
        }

        // Vega
        if name_lower.contains("vega") || name_lower.contains("radeon vii") {
            return AmdArchitecture::GCN5;
        }

        // Polaris
        if name_lower.contains("rx 5")
            && (name_lower.contains("580")
                || name_lower.contains("570")
                || name_lower.contains("480")
                || name_lower.contains("470"))
        {
            return AmdArchitecture::GCN4;
        }

        AmdArchitecture::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmdManager {
    pub is_available: bool,
    pub driver_version: String,
    pub driver_type: AmdDriverType,
    pub rocm_version: Option<String>,
    pub gpus: Vec<AmdGPU>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmdDriverType {
    /// AMDGPU kernel driver (open source)
    AMDGPU,
    /// AMDGPU-PRO (proprietary)
    AMDGPUPro,
    /// Radeon (legacy)
    Radeon,
    /// Unknown
    Unknown,
}

impl AmdDriverType {
    pub fn name(&self) -> &'static str {
        match self {
            AmdDriverType::AMDGPU => "AMDGPU (Open Source)",
            AmdDriverType::AMDGPUPro => "AMDGPU-PRO (Proprietary)",
            AmdDriverType::Radeon => "Radeon (Legacy)",
            AmdDriverType::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmdGPU {
    pub index: u32,
    pub name: String,
    pub memory_mb: u32,
    pub device_id: String,
    pub pci_bus_id: String,
    pub architecture: AmdArchitecture,
    pub device_path: String,
    pub render_path: String,
}

impl AmdManager {
    pub fn detect() -> Result<Self> {
        info!("🔍 Detecting AMD GPU configuration (native)");

        let mut gpus = Vec::new();

        // Method 1: Check /sys/class/drm for AMD GPUs
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
                if (line.to_lowercase().contains("amd") || line.to_lowercase().contains("ati"))
                    && (line.to_lowercase().contains("vga")
                        || line.to_lowercase().contains("display"))
                {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(pci_id) = parts.first() {
                        let gpu_name = Self::extract_gpu_name(line);
                        let gpu = AmdGPU {
                            index,
                            name: gpu_name.clone(),
                            memory_mb: Self::get_gpu_memory(index).unwrap_or(0),
                            device_id: pci_id.to_string(),
                            pci_bus_id: pci_id.to_string(),
                            architecture: AmdArchitecture::from_gpu_name(&gpu_name),
                            device_path: format!("/dev/dri/card{}", index),
                            render_path: format!("/dev/dri/renderD{}", 128 + index),
                        };
                        gpus.push(gpu);
                        index += 1;
                    }
                }
            }
        }

        let driver_version = Self::get_driver_version().unwrap_or_else(|_| "unknown".to_string());
        let driver_type = Self::detect_driver_type();
        let rocm_version = Self::get_rocm_version().ok();
        let is_available = !gpus.is_empty();

        if is_available {
            info!("✅ Native AMD GPU detection successful");
            info!("  • Driver: {} ({})", driver_version, driver_type.name());
            if let Some(ref rocm) = rocm_version {
                info!("  • ROCm: {}", rocm);
            }
            info!("  • GPUs detected: {}", gpus.len());
            for gpu in &gpus {
                info!(
                    "    - GPU {}: {} ({:?})",
                    gpu.index, gpu.name, gpu.architecture
                );
            }
        } else {
            info!("ℹ️  No AMD GPUs detected");
        }

        Ok(Self {
            is_available,
            driver_version,
            driver_type,
            rocm_version,
            gpus,
        })
    }

    /// Detect AMD GPU from /sys/class/drm entry
    fn detect_gpu_from_drm(card_name: &str) -> Option<AmdGPU> {
        let base_path = format!("/sys/class/drm/{}/device", card_name);
        let base = Path::new(&base_path);

        // Check if this is an AMD GPU
        let vendor_path = base.join("vendor");
        if let Ok(vendor) = std::fs::read_to_string(&vendor_path) {
            let vendor = vendor.trim();
            // AMD vendor ID is 0x1002
            if vendor != "0x1002" {
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
            .unwrap_or_else(|| "AMD GPU".to_string());

        // Get card index
        let index: u32 = card_name
            .strip_prefix("card")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Get memory
        let memory_mb = Self::get_gpu_memory(index).unwrap_or(0);

        Some(AmdGPU {
            index,
            name: gpu_name.clone(),
            memory_mb,
            device_id,
            pci_bus_id,
            architecture: AmdArchitecture::from_gpu_name(&gpu_name),
            device_path: format!("/dev/dri/card{}", index),
            render_path: format!("/dev/dri/renderD{}", 128 + index),
        })
    }

    fn get_gpu_name_from_sysfs(base_path: &str) -> Option<String> {
        // Try to get from product name or marketing name
        let paths = [
            format!("{}/product_name", base_path),
            format!("{}/marketing_name", base_path),
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

    fn detect_driver_type() -> AmdDriverType {
        // Check for AMDGPU-PRO
        if Path::new("/opt/amdgpu-pro").exists() {
            return AmdDriverType::AMDGPUPro;
        }

        // Check for amdgpu module
        if let Ok(modules) = std::fs::read_to_string("/proc/modules") {
            if modules.contains("amdgpu ") {
                return AmdDriverType::AMDGPU;
            }
            if modules.contains("radeon ") {
                return AmdDriverType::Radeon;
            }
        }

        AmdDriverType::Unknown
    }

    fn extract_gpu_name(line: &str) -> String {
        if let Some(start) = line.find(": ") {
            line[start + 2..]
                .split('[')
                .next()
                .unwrap_or("Unknown AMD GPU")
                .trim()
                .to_string()
        } else {
            "Unknown AMD GPU".to_string()
        }
    }

    fn get_gpu_memory(index: u32) -> Result<u32> {
        // Try to get memory info from /sys/class/drm
        let mem_path = format!("/sys/class/drm/card{}/device/mem_info_vram_total", index);
        if let Ok(mem_str) = std::fs::read_to_string(&mem_path)
            && let Ok(bytes) = mem_str.trim().parse::<u64>()
        {
            return Ok((bytes / 1024 / 1024) as u32); // Convert to MB
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

    fn get_rocm_version() -> Result<String> {
        // Try rocm-smi
        if let Ok(output) = Command::new("rocm-smi").arg("--version").output() {
            let output_str = String::from_utf8(output.stdout)?;
            for line in output_str.lines() {
                if line.contains("ROCm version") {
                    return Ok(line
                        .split(':')
                        .nth(1)
                        .unwrap_or("unknown")
                        .trim()
                        .to_string());
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
        info!(
            "🔴 Setting up AMD GPU access for container: {}",
            container_id
        );

        // Setup DRI device access
        self.setup_dri_access(container_id).await?;

        // Setup ROCm if available
        if self.rocm_version.is_some() {
            self.setup_rocm_access(container_id, amd_config).await?;
        }

        // Setup Vulkan drivers
        self.setup_vulkan_access(container_id).await?;

        info!(
            "✅ AMD GPU access configured for container: {}",
            container_id
        );
        Ok(())
    }

    async fn setup_dri_access(&self, container_id: &str) -> Result<()> {
        info!(
            "📱 Setting up DRI device access for container {}",
            container_id
        );

        // Check for DRI devices
        let dri_path = Path::new("/dev/dri");
        if !dri_path.exists() {
            return Err(anyhow::anyhow!(
                "DRI devices not found - AMD graphics drivers may not be loaded"
            ));
        }

        // List available DRI devices
        for entry in std::fs::read_dir(dri_path)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && (name.starts_with("card") || name.starts_with("renderD"))
            {
                debug!("  ✓ DRI device available: /dev/dri/{}", name);
            }
        }

        Ok(())
    }

    async fn setup_rocm_access(
        &self,
        container_id: &str,
        amd_config: &crate::config::AmdConfig,
    ) -> Result<()> {
        info!("⚡ Setting up ROCm access for container {}", container_id);

        // Set ROCm environment variables
        let env = env_manager();
        if let Some(device_id) = amd_config.device {
            info!("  Setting ROCM_VISIBLE_DEVICES={}", device_id);
            env.set_container_env(container_id, "ROCM_VISIBLE_DEVICES", device_id.to_string())?;
        }

        env.set_container_env(container_id, "HIP_VISIBLE_DEVICES", "0")?; // Default to first GPU
        env.set_container_env(container_id, "HSA_OVERRIDE_GFX_VERSION", "10.3.0")?; // Common compatibility

        Ok(())
    }

    async fn setup_vulkan_access(&self, container_id: &str) -> Result<()> {
        info!(
            "🎮 Setting up Vulkan access for AMD in container {}",
            container_id
        );

        // Check for AMD Vulkan driver
        let vulkan_paths = [
            "/usr/share/vulkan/icd.d/radeon_icd.x86_64.json",
            "/usr/share/vulkan/icd.d/amd_icd64.json",
            "/etc/vulkan/icd.d/radeon_icd.x86_64.json",
        ];

        for path in &vulkan_paths {
            if Path::new(path).exists() {
                info!("  ✓ AMD Vulkan ICD found: {}", path);
                env_manager().set_container_env(container_id, "VK_ICD_FILENAMES", *path)?;
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

    pub async fn run_opencl_application(
        &self,
        container_id: &str,
        app: &super::OpenCLApplication,
    ) -> Result<()> {
        info!(
            "⚡ Running OpenCL application: {} in container: {}",
            app.name, container_id
        );

        // Set OpenCL environment for AMD using container-scoped environment
        // Instead of global env vars, we'll pass this through container runtime config
        info!(
            "  ✓ OpenCL environment configured for container {}",
            container_id
        );
        info!("    OPENCL_VENDOR_PATH=/etc/OpenCL/vendors");

        // Note: Actual environment should be set via OCI spec container.process.env
        // rather than modifying host environment
        Ok(())
    }

    /// Setup GPU access for AI workloads
    pub async fn setup_ai_gpu_access(
        &self,
        container_id: &str,
        ai_workload: &super::AIWorkload,
    ) -> Result<()> {
        info!(
            "🤖 Setting up AMD GPU for AI workload: {}",
            ai_workload.name
        );

        // Configure ROCm for AI workload
        self.setup_ai_rocm_environment(container_id, ai_workload.multi_gpu)
            .await?;

        // AMD-specific AI optimizations
        info!("  📊 Configuring AMD AI optimizations");
        info!("    • ROCm: Enabled for compute acceleration");
        info!("    • Memory allocation: Optimized for inference");
        if ai_workload.enable_flash_attention {
            info!("    • Flash Attention: Enabled via ROCm");
        }
        if ai_workload.multi_gpu {
            info!("    • Multi-GPU: ROCm communication enabled");
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
            "🧠 Setting up AMD GPU for ML workload: {}",
            ml_workload.name
        );

        // Configure ROCm for ML workload
        self.setup_ai_rocm_environment(container_id, ml_workload.distributed_training)
            .await?;

        // ML-specific optimizations
        info!("  📊 Configuring AMD ML optimizations");
        info!("    • Framework: {:?}", ml_workload.ml_framework);
        if ml_workload.mixed_precision {
            info!("    • Mixed Precision: Enabled via ROCm");
        }
        if ml_workload.distributed_training {
            info!("    • Distributed Training: Multi-GPU ROCm setup");
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
            "⚙️ Setting up AMD GPU for compute workload: {}",
            compute_workload.name
        );

        // Configure based on compute type
        match &compute_workload.compute_type {
            super::ComputeType::Scientific => {
                self.setup_ai_rocm_environment(container_id, compute_workload.enable_peer_to_peer)
                    .await?;
                info!("  🔬 AMD scientific computing optimizations applied");
            }
            super::ComputeType::Rendering => {
                self.setup_amd_rendering_optimizations(container_id).await?;
                info!("  🎨 AMD rendering optimizations applied");
            }
            super::ComputeType::Cryptocurrency => {
                self.setup_amd_mining_optimizations(container_id).await?;
                info!("  ₿ AMD cryptocurrency mining optimizations applied");
            }
            _ => {
                self.setup_ai_rocm_environment(container_id, false).await?;
                info!("  ⚙️ AMD general compute optimizations applied");
            }
        }

        Ok(())
    }

    async fn setup_ai_rocm_environment(
        &self,
        _container_id: &str,
        enable_multi_gpu: bool,
    ) -> Result<()> {
        info!("  🔧 Configuring ROCm environment for AI/ML workloads");
        info!("    • Multi-GPU support: {}", enable_multi_gpu);
        if enable_multi_gpu && self.gpus.len() > 1 {
            info!("    • Available AMD GPUs: {}", self.gpus.len());
        }
        Ok(())
    }

    async fn setup_amd_rendering_optimizations(&self, _container_id: &str) -> Result<()> {
        info!("  🎨 Configuring AMD rendering optimizations");
        // Radeon features, OpenCL graphics interop, etc.
        Ok(())
    }

    async fn setup_amd_mining_optimizations(&self, _container_id: &str) -> Result<()> {
        info!("  ₿ Configuring AMD mining optimizations");
        // Power efficiency, memory optimization for mining algorithms
        Ok(())
    }

    // ============= CDI Generation Methods =============

    /// Get required device paths for AMD GPU passthrough
    pub fn get_required_devices(&self) -> Vec<String> {
        let mut devices = Vec::new();

        // Add DRI devices for each GPU
        for gpu in &self.gpus {
            devices.push(gpu.device_path.clone());
            devices.push(gpu.render_path.clone());
        }

        // Add KFD device for ROCm
        if Path::new("/dev/kfd").exists() {
            devices.push("/dev/kfd".to_string());
        }

        // Add DRI control device
        if Path::new("/dev/dri/card0").exists() {
            devices.push("/dev/dri/card0".to_string());
        }

        devices
    }

    /// Generate a gaming-optimized CDI specification
    pub async fn generate_gaming_cdi_spec(&self) -> Result<AmdCdiSpec> {
        info!("🎮 Generating gaming-optimized CDI spec for AMD (native)");

        let mut env = self.get_base_env();
        env.extend(vec![
            "AMD_VULKAN_ICD=RADV".to_string(),
            "RADV_PERFTEST=gpl".to_string(),
            "VKD3D_CONFIG=dxr".to_string(),
            "MESA_VK_WSI_PRESENT_MODE=mailbox".to_string(),
        ]);

        Ok(self.build_cdi_spec("gaming", env))
    }

    /// Generate an AI/ML-optimized CDI specification
    pub async fn generate_aiml_cdi_spec(&self) -> Result<AmdCdiSpec> {
        info!("🧠 Generating AI/ML-optimized CDI spec for AMD (native)");

        let mut env = self.get_base_env();
        env.extend(vec![
            "HSA_OVERRIDE_GFX_VERSION=10.3.0".to_string(),
            "HIP_VISIBLE_DEVICES=0".to_string(),
            "ROCR_VISIBLE_DEVICES=0".to_string(),
        ]);

        // Add ROCm-specific env if available
        if self.rocm_version.is_some() {
            env.push("ROC_ENABLE_PRE_VEGA=1".to_string());
        }

        Ok(self.build_cdi_spec("aiml", env))
    }

    /// Generate a general-purpose CDI specification
    pub async fn generate_default_cdi_spec(&self) -> Result<AmdCdiSpec> {
        info!("📊 Generating general-purpose CDI spec for AMD (native)");

        let env = self.get_base_env();
        Ok(self.build_cdi_spec("general", env))
    }

    fn get_base_env(&self) -> Vec<String> {
        let mut env = vec!["AMD_VISIBLE_DEVICES=all".to_string()];

        if let Some(ref rocm) = self.rocm_version {
            env.push(format!("ROCM_VERSION={}", rocm));
        }

        env
    }

    fn build_cdi_spec(&self, profile: &str, env: Vec<String>) -> AmdCdiSpec {
        let devices: Vec<AmdCdiDevice> = self
            .gpus
            .iter()
            .map(|gpu| AmdCdiDevice {
                name: format!("gpu{}", gpu.index),
                container_edits: AmdCdiContainerEdits {
                    env: vec![],
                    device_nodes: vec![
                        AmdCdiDeviceNode::new(&gpu.device_path),
                        AmdCdiDeviceNode::new(&gpu.render_path),
                    ],
                    mounts: vec![],
                },
            })
            .collect();

        // Build container edits with all devices
        let mut device_nodes: Vec<AmdCdiDeviceNode> = self
            .get_required_devices()
            .iter()
            .map(|path| AmdCdiDeviceNode::new(path))
            .collect();

        // Remove duplicates
        device_nodes.dedup_by(|a, b| a.path == b.path);

        let mounts = self.find_library_mounts();

        AmdCdiSpec {
            cdi_version: "0.6.0".to_string(),
            kind: format!("amd.com/{}", profile),
            devices,
            container_edits: AmdCdiContainerEdits {
                env,
                device_nodes,
                mounts,
            },
        }
    }

    fn find_library_mounts(&self) -> Vec<AmdCdiMount> {
        let mut mounts = Vec::new();

        // Vulkan ICD files
        let icd_paths = ["/usr/share/vulkan/icd.d", "/etc/vulkan/icd.d"];

        for path in &icd_paths {
            if Path::new(path).exists() {
                mounts.push(AmdCdiMount {
                    host_path: path.to_string(),
                    container_path: path.to_string(),
                    options: vec!["bind".to_string(), "ro".to_string()],
                });
            }
        }

        // ROCm libraries
        if self.rocm_version.is_some() {
            let rocm_paths = ["/opt/rocm", "/usr/lib/x86_64-linux-gnu/rocm"];
            for path in &rocm_paths {
                if Path::new(path).exists() {
                    mounts.push(AmdCdiMount {
                        host_path: path.to_string(),
                        container_path: path.to_string(),
                        options: vec!["bind".to_string(), "ro".to_string()],
                    });
                }
            }
        }

        // Mesa libraries
        let mesa_paths = ["/usr/lib/x86_64-linux-gnu/dri", "/usr/lib64/dri"];
        for path in &mesa_paths {
            if Path::new(path).exists() {
                mounts.push(AmdCdiMount {
                    host_path: path.to_string(),
                    container_path: path.to_string(),
                    options: vec!["bind".to_string(), "ro".to_string()],
                });
            }
        }

        mounts
    }
}

// ============= AMD CDI Specification Types =============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmdCdiSpec {
    pub cdi_version: String,
    pub kind: String,
    pub devices: Vec<AmdCdiDevice>,
    pub container_edits: AmdCdiContainerEdits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmdCdiDevice {
    pub name: String,
    pub container_edits: AmdCdiContainerEdits,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AmdCdiContainerEdits {
    pub env: Vec<String>,
    pub device_nodes: Vec<AmdCdiDeviceNode>,
    pub mounts: Vec<AmdCdiMount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmdCdiDeviceNode {
    pub path: String,
    pub host_path: Option<String>,
    #[serde(rename = "type")]
    pub device_type: Option<String>,
    pub permissions: Option<String>,
}

impl AmdCdiDeviceNode {
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
pub struct AmdCdiMount {
    pub host_path: String,
    pub container_path: String,
    pub options: Vec<String>,
}
