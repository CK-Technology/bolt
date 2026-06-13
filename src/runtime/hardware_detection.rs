//! Hardware Detection and Optimization Module
//!
//! Provides comprehensive CPU and GPU detection with vendor-specific optimizations:
//! - AMD Ryzen Zen/Zen2/Zen3/Zen4 with 3D V-Cache support
//! - Intel Alder Lake / Raptor Lake hybrid architecture (P-cores + E-cores)
//! - Intel GPU (i915/Xe) with Quick Sync video acceleration
//! - CPU cache topology and NUMA awareness

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// Complete hardware profile for container optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub cpu: CpuInfo,
    pub gpu: Vec<GpuInfo>,
    pub memory: MemoryInfo,
}

/// CPU information with vendor-specific optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub vendor: CpuVendor,
    pub model_name: String,
    pub architecture: CpuArchitecture,
    pub cores_physical: usize,
    pub cores_logical: usize,
    pub features: Vec<String>,
    pub cache_l3_kb: Option<u64>,
    pub numa_nodes: usize,

    // AMD-specific
    pub zen_generation: Option<ZenGeneration>,
    pub has_3d_vcache: bool,
    pub ccd_count: Option<usize>, // Chiplet die count

    // Intel-specific
    pub hybrid_architecture: Option<IntelHybridInfo>,
    pub efficiency_cores: usize,
    pub performance_cores: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CpuVendor {
    AMD,
    Intel,
    ARM,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CpuArchitecture {
    // AMD Zen family
    Zen,  // Ryzen 1000
    Zen2, // Ryzen 3000
    Zen3, // Ryzen 5000
    Zen4, // Ryzen 7000/9000

    // Intel modern architectures
    Skylake,
    CoffeeLake,
    IceLake,
    TigerLake,
    AlderLake,  // 12th gen hybrid
    RaptorLake, // 13th/14th gen hybrid
    MeteorLake, // 14th gen mobile

    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ZenGeneration {
    Zen,
    ZenPlus,
    Zen2,
    Zen3,
    Zen3Plus, // With 3D V-Cache
    Zen4,
    Zen4C, // Dense variant
    Zen5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelHybridInfo {
    pub p_cores: usize,
    pub e_cores: usize,
    pub p_core_base_freq: f32,
    pub e_core_base_freq: f32,
    pub thread_director: bool, // Hardware thread scheduling
}

/// GPU information with vendor-specific capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub vendor: GpuVendor,
    pub device_path: PathBuf,
    pub render_node: Option<PathBuf>,
    pub pci_id: Option<String>,
    pub memory_mb: Option<u64>,
    pub capabilities: GpuCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GpuVendor {
    NVIDIA,
    AMD,
    Intel,
    ARM,      // Mali, Immortalis
    Qualcomm, // Adreno
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GpuCapabilities {
    // Compute
    pub cuda: bool,
    pub rocm: bool,
    pub opencl: bool,
    pub vulkan: bool,

    // Video acceleration
    pub vaapi: bool,       // Intel/AMD on Linux
    pub quick_sync: bool,  // Intel hardware video
    pub nvenc_nvdec: bool, // NVIDIA video
    pub vce_vcn: bool,     // AMD video

    // Gaming features
    pub ray_tracing: bool,
    pub dlss: bool, // NVIDIA
    pub fsr: bool,  // AMD
    pub xess: bool, // Intel
    pub mesh_shaders: bool,
    pub variable_rate_shading: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_mb: u64,
    pub numa_nodes: usize,
    pub hugepages_2mb: u64,
    pub hugepages_1gb: u64,
}

impl HardwareProfile {
    /// Detect hardware profile for the current system
    pub async fn detect() -> Result<Self> {
        let cpu = CpuInfo::detect().await?;
        let gpu = GpuInfo::detect_all().await?;
        let memory = MemoryInfo::detect().await?;

        Ok(Self { cpu, gpu, memory })
    }

    /// Generate optimal container CPU affinity based on workload
    pub fn optimal_cpu_affinity(&self, workload_type: WorkloadType) -> CpuAffinity {
        match (&self.cpu.vendor, &self.cpu.hybrid_architecture) {
            (CpuVendor::Intel, Some(hybrid)) => self.intel_hybrid_affinity(workload_type, hybrid),
            (CpuVendor::AMD, _) if self.cpu.has_3d_vcache => {
                self.amd_vcache_affinity(workload_type)
            }
            _ => CpuAffinity::All, // Use all cores
        }
    }

    fn intel_hybrid_affinity(
        &self,
        workload: WorkloadType,
        hybrid: &IntelHybridInfo,
    ) -> CpuAffinity {
        match workload {
            WorkloadType::Gaming | WorkloadType::HighPerformance => {
                // Pin to P-cores for gaming/performance
                CpuAffinity::Specific((0..hybrid.p_cores * 2).collect())
            }
            WorkloadType::Background | WorkloadType::Batch => {
                // Use E-cores for background tasks
                let e_core_start = hybrid.p_cores * 2;
                CpuAffinity::Specific((e_core_start..e_core_start + hybrid.e_cores).collect())
            }
            WorkloadType::Balanced => CpuAffinity::All,
        }
    }

    fn amd_vcache_affinity(&self, workload: WorkloadType) -> CpuAffinity {
        match workload {
            WorkloadType::Gaming => {
                // AMD 3D V-Cache: pin to CCD with V-Cache (usually CCD0)
                // For Ryzen 7 5800X3D and 7800X3D, use first 8 threads
                if let Some(ccd_count) = self.cpu.ccd_count {
                    if ccd_count > 1 {
                        CpuAffinity::Specific((0..8).collect())
                    } else {
                        CpuAffinity::All
                    }
                } else {
                    CpuAffinity::All
                }
            }
            _ => CpuAffinity::All,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CpuAffinity {
    All,
    Specific(Vec<usize>), // CPU IDs
    NUMA(usize),          // NUMA node
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WorkloadType {
    Gaming,
    HighPerformance,
    Balanced,
    Background,
    Batch,
}

impl CpuInfo {
    pub async fn detect() -> Result<Self> {
        let cpuinfo_path = Path::new("/proc/cpuinfo");
        let content = fs::read_to_string(cpuinfo_path).context("Failed to read /proc/cpuinfo")?;

        let mut vendor = CpuVendor::Other("unknown".to_string());
        let mut model_name = String::new();
        let mut features = Vec::new();
        let mut cores_logical = 0;

        for line in content.lines() {
            if let Some(value) = line.strip_prefix("vendor_id") {
                if let Some(v) = value.split(':').nth(1) {
                    vendor = match v.trim() {
                        "AuthenticAMD" => CpuVendor::AMD,
                        "GenuineIntel" => CpuVendor::Intel,
                        "ARM" => CpuVendor::ARM,
                        other => CpuVendor::Other(other.to_string()),
                    };
                }
            } else if let Some(value) = line.strip_prefix("model name") {
                if let Some(m) = value.split(':').nth(1) {
                    model_name = m.trim().to_string();
                }
            } else if let Some(value) = line.strip_prefix("flags") {
                if let Some(f) = value.split(':').nth(1) {
                    features = f.split_whitespace().map(|s| s.to_string()).collect();
                }
            } else if line.starts_with("processor") {
                cores_logical += 1;
            }
        }

        let cores_physical = Self::detect_physical_cores()?;
        let cache_l3_kb = Self::detect_l3_cache();
        let numa_nodes = Self::detect_numa_nodes();

        // Detect AMD Zen generation
        let (zen_generation, has_3d_vcache, ccd_count) = if vendor == CpuVendor::AMD {
            Self::detect_amd_details(&model_name, &features)
        } else {
            (None, false, None)
        };

        // Detect Intel hybrid architecture
        let (hybrid_architecture, efficiency_cores, performance_cores) =
            if vendor == CpuVendor::Intel {
                Self::detect_intel_hybrid(&model_name, cores_logical)
            } else {
                (None, 0, cores_physical)
            };

        let architecture = Self::detect_architecture(&vendor, &model_name, &features);

        Ok(Self {
            vendor,
            model_name,
            architecture,
            cores_physical,
            cores_logical,
            features,
            cache_l3_kb,
            numa_nodes,
            zen_generation,
            has_3d_vcache,
            ccd_count,
            hybrid_architecture,
            efficiency_cores,
            performance_cores,
        })
    }

    fn detect_physical_cores() -> Result<usize> {
        let cpu_path = Path::new("/sys/devices/system/cpu");
        let mut cores = std::collections::HashSet::new();

        if let Ok(entries) = fs::read_dir(cpu_path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("cpu") && name_str[3..].chars().all(|c| c.is_ascii_digit())
                {
                    let core_id_path = entry.path().join("topology/core_id");
                    if let Ok(core_id) = fs::read_to_string(&core_id_path)
                        && let Ok(id) = core_id.trim().parse::<usize>()
                    {
                        cores.insert(id);
                    }
                }
            }
        }

        Ok(if cores.is_empty() { 1 } else { cores.len() })
    }

    fn detect_l3_cache() -> Option<u64> {
        // Try to read L3 cache size from sysfs
        let cache_path = Path::new("/sys/devices/system/cpu/cpu0/cache");
        if !cache_path.exists() {
            return None;
        }

        if let Ok(entries) = fs::read_dir(cache_path) {
            for entry in entries.flatten() {
                let level_path = entry.path().join("level");
                if let Ok(level) = fs::read_to_string(&level_path)
                    && level.trim() == "3"
                {
                    let size_path = entry.path().join("size");
                    if let Ok(size_str) = fs::read_to_string(&size_path) {
                        // Size is in format "32768K" or "32M"
                        return Self::parse_cache_size(&size_str);
                    }
                }
            }
        }
        None
    }

    fn parse_cache_size(size_str: &str) -> Option<u64> {
        let trimmed = size_str.trim();
        if let Some(kb_str) = trimmed.strip_suffix('K') {
            kb_str.parse::<u64>().ok()
        } else if let Some(mb_str) = trimmed.strip_suffix('M') {
            mb_str.parse::<u64>().ok().map(|mb| mb * 1024)
        } else {
            None
        }
    }

    fn detect_numa_nodes() -> usize {
        let numa_path = Path::new("/sys/devices/system/node");
        if !numa_path.exists() {
            return 1;
        }

        let mut node_count = 0;
        if let Ok(entries) = fs::read_dir(numa_path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with("node") {
                    node_count += 1;
                }
            }
        }

        if node_count == 0 { 1 } else { node_count }
    }

    fn detect_amd_details(
        model_name: &str,
        _features: &[String],
    ) -> (Option<ZenGeneration>, bool, Option<usize>) {
        let model_lower = model_name.to_lowercase();
        let has_3d = model_lower.contains("3d") || model_lower.contains("x3d");

        let zen_gen = if model_lower.contains("ryzen") {
            // Ryzen 1000/2000 = Zen/Zen+
            // Ryzen 3000 = Zen 2
            // Ryzen 5000 = Zen 3
            // Ryzen 7000/9000 = Zen 4
            if model_lower.contains("9000") || model_lower.contains("9 9") {
                Some(ZenGeneration::Zen4)
            } else if model_lower.contains("7000") || model_lower.contains("7 7") {
                // Zen 4 for both 3D and non-3D variants
                Some(ZenGeneration::Zen4)
            } else if model_lower.contains("5000") || model_lower.contains("5 5") {
                if has_3d {
                    Some(ZenGeneration::Zen3Plus)
                } else {
                    Some(ZenGeneration::Zen3)
                }
            } else if model_lower.contains("3000") || model_lower.contains("3 3") {
                Some(ZenGeneration::Zen2)
            } else if model_lower.contains("2000") || model_lower.contains("2 2") {
                Some(ZenGeneration::ZenPlus)
            } else if model_lower.contains("1000") || model_lower.contains("1 1") {
                Some(ZenGeneration::Zen)
            } else {
                Some(ZenGeneration::Zen3) // Default to Zen 3 for unknown
            }
        } else if model_lower.contains("epyc") {
            // EPYC server CPUs
            if model_lower.contains("genoa") || model_lower.contains("9004") {
                Some(ZenGeneration::Zen4)
            } else if model_lower.contains("milan") || model_lower.contains("7003") {
                Some(ZenGeneration::Zen3)
            } else {
                Some(ZenGeneration::Zen2)
            }
        } else {
            None
        };

        // Detect CCD count (rough heuristic based on core count)
        // Most Ryzen 5/7 = 1 CCD, Ryzen 9 = 2 CCD
        let ccd_count = if model_lower.contains("ryzen 9") {
            Some(2)
        } else {
            Some(1)
        };

        (zen_gen, has_3d, ccd_count)
    }

    fn detect_intel_hybrid(
        model_name: &str,
        total_threads: usize,
    ) -> (Option<IntelHybridInfo>, usize, usize) {
        let model_lower = model_name.to_lowercase();

        // Alder Lake (12th gen) and Raptor Lake (13th/14th gen) have hybrid architecture
        let is_hybrid = model_lower.contains("12th gen")
            || model_lower.contains("13th gen")
            || model_lower.contains("14th gen")
            || model_lower.contains("ultra");

        if !is_hybrid {
            return (None, 0, total_threads / 2);
        }

        // Detect P-core/E-core split based on SKU
        // These are rough heuristics - ideally would query CPUID
        let (p_cores, e_cores) = if model_lower.contains("i9") {
            if model_lower.contains("13900") || model_lower.contains("14900") {
                (8, 16) // i9-13900K/14900K: 8P+16E
            } else {
                (8, 8) // i9-12900K: 8P+8E
            }
        } else if model_lower.contains("i7") {
            if model_lower.contains("13700") || model_lower.contains("14700") {
                (8, 8) // i7-13700K/14700K: 8P+8E
            } else {
                (8, 4) // i7-12700K: 8P+4E
            }
        } else if model_lower.contains("i5") {
            (6, 4) // i5 SKUs: typically 6P+4E
        } else {
            (4, 4) // Default guess
        };

        let hybrid_info = IntelHybridInfo {
            p_cores,
            e_cores,
            p_core_base_freq: 3.5, // Would need CPUID for real values
            e_core_base_freq: 2.5,
            thread_director: true, // All Alder/Raptor Lake have Thread Director
        };

        (Some(hybrid_info), e_cores, p_cores)
    }

    fn detect_architecture(
        vendor: &CpuVendor,
        model_name: &str,
        _features: &[String],
    ) -> CpuArchitecture {
        let model_lower = model_name.to_lowercase();

        match vendor {
            CpuVendor::AMD => {
                if model_lower.contains("9000") || model_lower.contains("7000") {
                    CpuArchitecture::Zen4
                } else if model_lower.contains("5000") {
                    CpuArchitecture::Zen3
                } else if model_lower.contains("3000") || model_lower.contains("2000") {
                    CpuArchitecture::Zen2
                } else {
                    CpuArchitecture::Zen
                }
            }
            CpuVendor::Intel => {
                if model_lower.contains("14th gen") || model_lower.contains("13th gen") {
                    CpuArchitecture::RaptorLake
                } else if model_lower.contains("12th gen") {
                    CpuArchitecture::AlderLake
                } else if model_lower.contains("11th gen") {
                    CpuArchitecture::TigerLake
                } else if model_lower.contains("10th gen") {
                    CpuArchitecture::IceLake
                } else {
                    CpuArchitecture::Skylake
                }
            }
            _ => CpuArchitecture::Unknown(model_name.to_string()),
        }
    }
}

impl GpuInfo {
    pub async fn detect_all() -> Result<Vec<Self>> {
        let mut gpus = Vec::new();

        // Detect via DRM (covers Intel, AMD, others)
        if let Ok(drm_gpus) = Self::detect_drm().await {
            gpus.extend(drm_gpus);
        }

        Ok(gpus)
    }

    async fn detect_drm() -> Result<Vec<Self>> {
        let drm_path = Path::new("/sys/class/drm");
        if !drm_path.exists() {
            return Ok(Vec::new());
        }

        let mut gpus = Vec::new();

        for entry in fs::read_dir(drm_path)?.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if !name_str.starts_with("card") || name_str.contains('-') {
                continue;
            }

            let vendor_path = entry.path().join("device/vendor");
            let vendor_id = fs::read_to_string(&vendor_path).ok();

            let vendor = vendor_id
                .as_ref()
                .and_then(|id| match id.trim() {
                    "0x10de" => Some(GpuVendor::NVIDIA),
                    "0x1002" => Some(GpuVendor::AMD),
                    "0x8086" => Some(GpuVendor::Intel),
                    _ => None,
                })
                .unwrap_or_else(|| GpuVendor::Other("unknown".to_string()));

            let device_path = PathBuf::from(format!("/dev/dri/{}", name_str));
            if !device_path.exists() {
                continue;
            }

            let idx = name_str.trim_start_matches("card").parse::<u32>().ok();
            let render_node = idx
                .map(|i| PathBuf::from(format!("/dev/dri/renderD{}", 128 + i)))
                .filter(|p| p.exists());

            let pci_id = fs::read_to_string(entry.path().join("device/uevent"))
                .ok()
                .and_then(|content| {
                    content
                        .lines()
                        .find_map(|line| line.strip_prefix("PCI_SLOT_NAME="))
                        .map(|s| s.trim().to_string())
                });

            let memory_mb = None; // Would need vendor-specific APIs

            let capabilities = Self::detect_capabilities(&vendor, &device_path);

            gpus.push(Self {
                vendor,
                device_path,
                render_node,
                pci_id,
                memory_mb,
                capabilities,
            });
        }

        Ok(gpus)
    }

    fn detect_capabilities(vendor: &GpuVendor, _device_path: &Path) -> GpuCapabilities {
        let mut caps = GpuCapabilities::default();

        // Base capabilities by vendor
        match vendor {
            GpuVendor::NVIDIA => {
                caps.cuda = true;
                caps.nvenc_nvdec = true;
                caps.vulkan = true;
                caps.ray_tracing = true; // Most modern NVIDIA
                caps.dlss = true;
            }
            GpuVendor::AMD => {
                caps.rocm = true;
                caps.opencl = true;
                caps.vulkan = true;
                caps.vaapi = true;
                caps.vce_vcn = true;
                caps.fsr = true;
                caps.ray_tracing = true; // RDNA2+
            }
            GpuVendor::Intel => {
                caps.opencl = true;
                caps.vulkan = true;
                caps.vaapi = true;
                caps.quick_sync = true; // Intel's hardware video
                caps.xess = true; // Arc GPUs
            }
            _ => {
                caps.vulkan = true; // Most modern GPUs
            }
        }

        caps
    }
}

impl MemoryInfo {
    pub async fn detect() -> Result<Self> {
        let meminfo_path = Path::new("/proc/meminfo");
        let content = fs::read_to_string(meminfo_path).context("Failed to read /proc/meminfo")?;

        let mut total_mb = 0;
        let mut hugepages_2mb = 0;
        let mut hugepages_1gb = 0;

        for line in content.lines() {
            if let Some(value) = line.strip_prefix("MemTotal:") {
                if let Some(kb) = value.split_whitespace().next() {
                    total_mb = kb.parse::<u64>().unwrap_or(0) / 1024;
                }
            } else if let Some(value) = line.strip_prefix("HugePages_Total:") {
                if let Some(pages) = value.split_whitespace().next() {
                    hugepages_2mb = pages.parse::<u64>().unwrap_or(0);
                }
            } else if let Some(value) = line.strip_prefix("Hugepagesize:")
                && let Some(size) = value.split_whitespace().next()
                && size == "1048576"
            {
                // 1GB in KB
                if let Some(pages) = line.split_whitespace().nth(1) {
                    hugepages_1gb = pages.parse::<u64>().unwrap_or(0);
                }
            }
        }

        let numa_nodes = CpuInfo::detect_numa_nodes();

        Ok(Self {
            total_mb,
            numa_nodes,
            hugepages_2mb,
            hugepages_1gb,
        })
    }
}

/// CPU governor management for performance tuning
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CpuGovernor {
    Performance,  // Maximum performance, no frequency scaling
    Powersave,    // Minimum power consumption
    Ondemand,     // Dynamic scaling based on load (legacy)
    Conservative, // Like ondemand but slower transitions
    Schedutil,    // Modern scheduler-driven scaling (default on most systems)
    Userspace,    // Manual frequency control
}

impl CpuGovernor {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Performance => "performance",
            Self::Powersave => "powersave",
            Self::Ondemand => "ondemand",
            Self::Conservative => "conservative",
            Self::Schedutil => "schedutil",
            Self::Userspace => "userspace",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "performance" => Some(Self::Performance),
            "powersave" => Some(Self::Powersave),
            "ondemand" => Some(Self::Ondemand),
            "conservative" => Some(Self::Conservative),
            "schedutil" => Some(Self::Schedutil),
            "userspace" => Some(Self::Userspace),
            _ => None,
        }
    }

    /// Get current CPU governor for a specific CPU core
    pub fn get_current(cpu_index: usize) -> Result<Self> {
        let path = format!(
            "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor",
            cpu_index
        );
        let governor_str = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read governor for CPU {}", cpu_index))?;

        Self::parse(&governor_str)
            .ok_or_else(|| anyhow!("Unknown governor: {}", governor_str.trim()))
    }

    /// Get current CPU governor (checks CPU 0 as representative)
    pub fn get_system_governor() -> Result<Self> {
        Self::get_current(0)
    }

    /// List available governors on the system
    pub fn list_available() -> Result<Vec<Self>> {
        let path = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_available_governors";
        let governors_str =
            fs::read_to_string(path).context("Failed to read available governors")?;

        Ok(governors_str
            .split_whitespace()
            .filter_map(Self::parse)
            .collect())
    }

    /// Set CPU governor for a specific core
    pub fn set_for_cpu(cpu_index: usize, governor: Self) -> Result<()> {
        let path = format!(
            "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor",
            cpu_index
        );

        // Check if we have write permission (requires root)
        if let Err(e) = fs::write(&path, governor.as_str()) {
            return Err(anyhow!(
                "Failed to set governor for CPU {} to '{}': {}. Root privileges required.",
                cpu_index,
                governor.as_str(),
                e
            ));
        }

        info!("✅ Set CPU {} governor to {}", cpu_index, governor.as_str());
        Ok(())
    }

    /// Set CPU governor for all cores
    pub fn set_system_governor(governor: Self) -> Result<()> {
        // Detect CPU count synchronously via sysfs
        let cpu_count = Self::count_online_cpus()?;

        info!(
            "🎯 Setting system-wide CPU governor to '{}' for {} cores",
            governor.as_str(),
            cpu_count
        );

        let mut errors = Vec::new();
        for cpu_idx in 0..cpu_count {
            if let Err(e) = Self::set_for_cpu(cpu_idx, governor.clone()) {
                errors.push(format!("CPU {}: {}", cpu_idx, e));
            }
        }

        if !errors.is_empty() {
            return Err(anyhow!(
                "Failed to set governor on some CPUs:\n{}",
                errors.join("\n")
            ));
        }

        info!("✅ System-wide CPU governor set to '{}'", governor.as_str());
        Ok(())
    }

    /// Count online CPUs synchronously
    fn count_online_cpus() -> Result<usize> {
        // Try /sys/devices/system/cpu/online first
        if let Ok(online_str) = fs::read_to_string("/sys/devices/system/cpu/online") {
            // Format: "0-7" or "0-3,6-7"
            let mut count = 0;
            for part in online_str.trim().split(',') {
                if let Some((start, end)) = part.split_once('-') {
                    if let (Ok(s), Ok(e)) = (start.parse::<usize>(), end.parse::<usize>()) {
                        count += e - s + 1;
                    }
                } else if part.parse::<usize>().is_ok() {
                    count += 1;
                }
            }
            if count > 0 {
                return Ok(count);
            }
        }

        // Fallback: count cpu* directories
        let cpu_dir = Path::new("/sys/devices/system/cpu");
        let mut count = 0;
        if let Ok(entries) = fs::read_dir(cpu_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("cpu") && name_str[3..].chars().all(|c| c.is_ascii_digit())
                {
                    count += 1;
                }
            }
        }

        if count > 0 {
            Ok(count)
        } else {
            Err(anyhow!("Failed to detect CPU count"))
        }
    }

    /// Get current CPU frequency (in kHz)
    pub fn get_current_frequency(cpu_index: usize) -> Result<u64> {
        let path = format!(
            "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_cur_freq",
            cpu_index
        );
        let freq_str = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read frequency for CPU {}", cpu_index))?;

        freq_str
            .trim()
            .parse::<u64>()
            .context("Failed to parse frequency")
    }

    /// Get min/max frequency range (in kHz)
    pub fn get_frequency_range(cpu_index: usize) -> Result<(u64, u64)> {
        let min_path = format!(
            "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_min_freq",
            cpu_index
        );
        let max_path = format!(
            "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_max_freq",
            cpu_index
        );

        let min_freq = fs::read_to_string(&min_path)?.trim().parse::<u64>()?;
        let max_freq = fs::read_to_string(&max_path)?.trim().parse::<u64>()?;

        Ok((min_freq, max_freq))
    }

    /// Recommend governor based on workload type
    pub fn recommend_for_workload(workload: WorkloadType) -> Self {
        match workload {
            WorkloadType::Gaming | WorkloadType::HighPerformance => Self::Performance,
            WorkloadType::Background | WorkloadType::Batch => Self::Powersave,
            WorkloadType::Balanced => Self::Schedutil,
        }
    }
}
