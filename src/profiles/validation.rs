use anyhow::Result;

use crate::optimizations::OptimizationProfile;
use super::SystemRequirements;

pub fn validate_profile(profile: &OptimizationProfile) -> Result<()> {
    if profile.name.is_empty() {
        return Err(anyhow::anyhow!("Profile name cannot be empty"));
    }

    if profile.name.len() > 64 {
        return Err(anyhow::anyhow!("Profile name too long (max 64 characters)"));
    }

    if profile.description.is_empty() {
        return Err(anyhow::anyhow!("Profile description cannot be empty"));
    }

    if profile.priority > 1000 {
        return Err(anyhow::anyhow!("Profile priority too high (max 1000)"));
    }

    // Validate GPU optimizations
    if let Some(gpu_opts) = &profile.gpu_optimizations.nvidia {
        if let Some(power_limit) = gpu_opts.power_limit {
            if power_limit > 150 {
                return Err(anyhow::anyhow!("NVIDIA power limit too high (max 150%)"));
            }
        }

        if let Some(memory_offset) = gpu_opts.memory_clock_offset {
            if memory_offset.abs() > 2000 {
                return Err(anyhow::anyhow!("NVIDIA memory clock offset too extreme (max ±2000 MHz)"));
            }
        }

        if let Some(core_offset) = gpu_opts.core_clock_offset {
            if core_offset.abs() > 1000 {
                return Err(anyhow::anyhow!("NVIDIA core clock offset too extreme (max ±1000 MHz)"));
            }
        }
    }

    // Validate CPU optimizations
    if let Some(priority) = profile.cpu_optimizations.priority {
        if priority < -20 || priority > 19 {
            return Err(anyhow::anyhow!("CPU priority out of range (-20 to 19)"));
        }
    }

    Ok(())
}

pub fn check_system_requirements(requirements: &SystemRequirements) -> Result<()> {
    let system_info = get_system_info()?;

    if let Some(min_cores) = requirements.min_cpu_cores {
        if system_info.cpu_cores < min_cores {
            return Err(anyhow::anyhow!(
                "Insufficient CPU cores: required {}, available {}",
                min_cores, system_info.cpu_cores
            ));
        }
    }

    if let Some(min_memory) = requirements.min_memory_gb {
        if system_info.memory_gb < min_memory {
            return Err(anyhow::anyhow!(
                "Insufficient memory: required {} GB, available {} GB",
                min_memory, system_info.memory_gb
            ));
        }
    }

    if let Some(required_vendor) = &requirements.required_gpu_vendor {
        if let Some(system_vendor) = &system_info.gpu_vendor {
            if std::mem::discriminant(required_vendor) != std::mem::discriminant(system_vendor) {
                return Err(anyhow::anyhow!(
                    "Incompatible GPU vendor: required {:?}, found {:?}",
                    required_vendor, system_vendor
                ));
            }
        } else {
            return Err(anyhow::anyhow!("No compatible GPU found"));
        }
    }

    if let Some(min_gpu_memory) = requirements.min_gpu_memory_gb {
        if let Some(gpu_memory) = system_info.gpu_memory_gb {
            if gpu_memory < min_gpu_memory {
                return Err(anyhow::anyhow!(
                    "Insufficient GPU memory: required {} GB, available {} GB",
                    min_gpu_memory, gpu_memory
                ));
            }
        } else {
            return Err(anyhow::anyhow!("Unable to determine GPU memory"));
        }
    }

    Ok(())
}

struct SystemInfo {
    cpu_cores: u32,
    memory_gb: u32,
    gpu_vendor: Option<crate::plugins::GpuVendor>,
    gpu_memory_gb: Option<u32>,
}

fn get_system_info() -> Result<SystemInfo> {
    Ok(SystemInfo {
        cpu_cores: num_cpus::get() as u32,
        memory_gb: get_total_memory_gb()?,
        gpu_vendor: detect_gpu_vendor()?,
        gpu_memory_gb: get_gpu_memory_gb()?,
    })
}

fn get_total_memory_gb() -> Result<u32> {
    let meminfo = std::fs::read_to_string("/proc/meminfo")?;
    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let kb: u64 = parts[1].parse()?;
                return Ok((kb / 1024 / 1024) as u32);
            }
        }
    }
    Err(anyhow::anyhow!("Could not determine total memory"))
}

fn detect_gpu_vendor() -> Result<Option<crate::plugins::GpuVendor>> {
    let lspci_output = std::process::Command::new("lspci")
        .arg("-nn")
        .output();

    if let Ok(output) = lspci_output {
        let output_str = String::from_utf8_lossy(&output.stdout);
        if output_str.to_lowercase().contains("nvidia") {
            return Ok(Some(crate::plugins::GpuVendor::Nvidia));
        }
        if output_str.to_lowercase().contains("amd") || output_str.to_lowercase().contains("radeon") {
            return Ok(Some(crate::plugins::GpuVendor::Amd));
        }
        if output_str.to_lowercase().contains("intel") {
            return Ok(Some(crate::plugins::GpuVendor::Intel));
        }
    }

    Ok(None)
}

fn get_gpu_memory_gb() -> Result<Option<u32>> {
    // This is a simplified implementation
    // In practice, you'd query the GPU driver directly
    Ok(None)
}