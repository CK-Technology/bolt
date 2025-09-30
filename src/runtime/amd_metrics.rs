//! AMD GPU Metrics via rocm-smi Integration
//!
//! Provides real-time GPU monitoring for AMD GPUs using rocm-smi.
//! Essential for AI/ML workloads running on AMD Instinct or RDNA GPUs.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use tokio::process::Command as AsyncCommand;
use tracing::{debug, info, warn};

/// AMD GPU metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmdGpuMetrics {
    pub device_id: String,
    pub gpu_utilization: f32, // 0-100%
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub memory_utilization: f32, // 0-100%
    pub temperature_c: f32,
    pub power_draw_watts: f32,
    pub power_cap_watts: f32,
    pub fan_speed_rpm: Option<u32>,
    pub fan_speed_percent: Option<f32>,
    pub clock_gpu_mhz: u32,
    pub clock_memory_mhz: u32,
    pub pcie_link_speed: Option<String>,
    pub compute_processes: Vec<ComputeProcess>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeProcess {
    pub pid: u32,
    pub name: String,
    pub memory_used_mb: u64,
}

/// AMD GPU device information
#[derive(Debug, Clone)]
pub struct AmdDevice {
    pub index: u32,
    pub device_id: String,
    pub name: String,
    pub vram_mb: u64,
    pub pci_bus: String,
    pub rocm_smi_path: PathBuf,
}

impl AmdDevice {
    /// Find rocm-smi binary
    pub fn find_rocm_smi() -> Option<PathBuf> {
        let paths = vec![
            "/opt/rocm/bin/rocm-smi",
            "/usr/bin/rocm-smi",
            "/usr/local/bin/rocm-smi",
        ];

        paths.into_iter().map(PathBuf::from).find(|p| p.exists())
    }

    /// Detect AMD GPUs via rocm-smi
    pub fn detect_all() -> Result<Vec<Self>> {
        let rocm_smi =
            Self::find_rocm_smi().ok_or_else(|| anyhow!("rocm-smi not found - install ROCm"))?;

        let output = Command::new(&rocm_smi)
            .arg("--showid")
            .arg("--showproductname")
            .arg("--showmeminfo")
            .arg("vram")
            .arg("--csv")
            .output()
            .context("Failed to run rocm-smi")?;

        if !output.status.success() {
            return Err(anyhow!("rocm-smi command failed"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut devices = Vec::new();

        for line in stdout.lines().skip(1) {
            // Skip CSV header
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() < 3 {
                continue;
            }

            // Parse: device, ID, Name, VRAM
            if let Some(device_str) = parts.first().and_then(|s| s.strip_prefix("card")) {
                if let Ok(index) = device_str.parse::<u32>() {
                    let device_id = parts.get(1).unwrap_or(&"Unknown").to_string();
                    let name = parts.get(2).unwrap_or(&"AMD GPU").to_string();
                    let vram_str = parts.get(3).unwrap_or(&"0");
                    let vram_mb = vram_str
                        .replace("MB", "")
                        .trim()
                        .parse::<u64>()
                        .unwrap_or(0);

                    // Get PCI bus ID
                    let pci_output = Command::new(&rocm_smi)
                        .arg("--showbus")
                        .arg("--device")
                        .arg(index.to_string())
                        .output()
                        .ok();

                    let pci_bus = pci_output
                        .and_then(|out| {
                            String::from_utf8(out.stdout)
                                .ok()
                                .and_then(|s| s.lines().nth(1).map(|l| l.trim().to_string()))
                        })
                        .unwrap_or_else(|| format!("Unknown:{}", index));

                    devices.push(Self {
                        index,
                        device_id,
                        name,
                        vram_mb,
                        pci_bus,
                        rocm_smi_path: rocm_smi.clone(),
                    });
                }
            }
        }

        if devices.is_empty() {
            return Err(anyhow!("No AMD GPUs detected"));
        }

        info!("✅ Detected {} AMD GPU(s) via rocm-smi", devices.len());
        Ok(devices)
    }

    /// Get real-time metrics for this GPU
    pub async fn get_metrics(&self) -> Result<AmdGpuMetrics> {
        let device_arg = self.index.to_string();

        // Run rocm-smi with multiple flags to get comprehensive metrics
        let output = AsyncCommand::new(&self.rocm_smi_path)
            .arg("--device")
            .arg(&device_arg)
            .arg("--showuse") // GPU utilization
            .arg("--showmemuse") // Memory usage
            .arg("--showtemp") // Temperature
            .arg("--showpower") // Power draw
            .arg("--showclocks") // Clock speeds
            .arg("--showfan") // Fan speed
            .arg("--csv")
            .output()
            .await
            .context("Failed to execute rocm-smi")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("rocm-smi failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_metrics(&stdout).await
    }

    async fn parse_metrics(&self, csv_output: &str) -> Result<AmdGpuMetrics> {
        let mut gpu_util = 0.0;
        let mut memory_used = 0u64;
        let memory_total = self.vram_mb;
        let mut temperature = 0.0;
        let mut power_draw = 0.0;
        let power_cap = 0.0;
        let mut clock_gpu = 0u32;
        let clock_mem = 0u32;
        let fan_rpm = None;
        let fan_percent = None;

        for line in csv_output.lines().skip(1) {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();

            // Parse based on column headers (simplified)
            // Real implementation would match column names
            if parts.len() > 1 {
                // GPU utilization (%)
                if let Some(util_str) = parts.get(1) {
                    if let Ok(util) = util_str.replace('%', "").trim().parse::<f32>() {
                        gpu_util = util;
                    }
                }

                // Memory usage
                if let Some(mem_str) = parts.get(2) {
                    if let Ok(mem) = mem_str.replace("MB", "").trim().parse::<u64>() {
                        memory_used = mem;
                    }
                }

                // Temperature
                if let Some(temp_str) = parts.get(3) {
                    if let Ok(temp) = temp_str.replace(['C', '°'], "").trim().parse::<f32>() {
                        temperature = temp;
                    }
                }

                // Power
                if let Some(power_str) = parts.get(4) {
                    if let Ok(power) = power_str.replace('W', "").trim().parse::<f32>() {
                        power_draw = power;
                    }
                }

                // Clocks
                if parts.len() > 6 {
                    if let Some(clock_str) = parts.get(6) {
                        if let Ok(clock) = clock_str.replace("MHz", "").trim().parse::<u32>() {
                            clock_gpu = clock;
                        }
                    }
                }
            }
        }

        // Get compute processes
        let processes = self.get_compute_processes().await.unwrap_or_default();

        let memory_util = if memory_total > 0 {
            (memory_used as f32 / memory_total as f32) * 100.0
        } else {
            0.0
        };

        Ok(AmdGpuMetrics {
            device_id: self.device_id.clone(),
            gpu_utilization: gpu_util,
            memory_used_mb: memory_used,
            memory_total_mb: memory_total,
            memory_utilization: memory_util,
            temperature_c: temperature,
            power_draw_watts: power_draw,
            power_cap_watts: power_cap,
            fan_speed_rpm: fan_rpm,
            fan_speed_percent: fan_percent,
            clock_gpu_mhz: clock_gpu,
            clock_memory_mhz: clock_mem,
            pcie_link_speed: None,
            compute_processes: processes,
        })
    }

    async fn get_compute_processes(&self) -> Result<Vec<ComputeProcess>> {
        // rocm-smi can show processes using GPU memory
        let output = AsyncCommand::new(&self.rocm_smi_path)
            .arg("--showpids")
            .arg("--device")
            .arg(self.index.to_string())
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut processes = Vec::new();

        for line in stdout.lines().skip(1) {
            if line.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(pid) = parts[0].parse::<u32>() {
                    let name = parts[1..].join(" ");
                    processes.push(ComputeProcess {
                        pid,
                        name,
                        memory_used_mb: 0, // Would need additional parsing
                    });
                }
            }
        }

        Ok(processes)
    }

    /// Monitor GPU continuously and report metrics
    pub async fn monitor_continuous<F>(&self, mut callback: F) -> Result<()>
    where
        F: FnMut(AmdGpuMetrics) + Send,
    {
        loop {
            match self.get_metrics().await {
                Ok(metrics) => {
                    debug!(
                        "AMD GPU {}: {}% util, {:.1}°C, {:.1}W",
                        self.index,
                        metrics.gpu_utilization,
                        metrics.temperature_c,
                        metrics.power_draw_watts
                    );
                    callback(metrics);
                }
                Err(e) => {
                    warn!("Failed to get AMD GPU metrics: {}", e);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}

/// AMD GPU monitoring manager
pub struct AmdGpuMonitor {
    devices: Vec<AmdDevice>,
}

impl AmdGpuMonitor {
    /// Initialize AMD GPU monitoring
    pub fn new() -> Result<Self> {
        let devices = AmdDevice::detect_all()?;
        Ok(Self { devices })
    }

    /// Get metrics for all AMD GPUs
    pub async fn get_all_metrics(&self) -> Vec<Result<AmdGpuMetrics>> {
        let mut results = Vec::new();
        for device in &self.devices {
            results.push(device.get_metrics().await);
        }
        results
    }

    /// Get metrics for specific GPU by index
    pub async fn get_metrics(&self, index: u32) -> Result<AmdGpuMetrics> {
        let device = self
            .devices
            .iter()
            .find(|d| d.index == index)
            .ok_or_else(|| anyhow!("AMD GPU {} not found", index))?;

        device.get_metrics().await
    }

    /// Get list of available AMD GPUs
    pub fn list_devices(&self) -> &[AmdDevice] {
        &self.devices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rocm_smi_detection() {
        let rocm_smi = AmdDevice::find_rocm_smi();
        if rocm_smi.is_some() {
            println!("rocm-smi found: {:?}", rocm_smi);
        } else {
            println!("rocm-smi not found (expected on non-AMD systems)");
        }
    }

    #[tokio::test]
    async fn test_amd_device_detection() {
        match AmdDevice::detect_all() {
            Ok(devices) => {
                println!("Found {} AMD GPU(s)", devices.len());
                for device in devices {
                    println!(
                        "  - {}: {} ({}MB VRAM)",
                        device.index, device.name, device.vram_mb
                    );
                }
            }
            Err(e) => {
                println!(
                    "AMD GPU detection failed (expected on non-AMD systems): {}",
                    e
                );
            }
        }
    }
}
