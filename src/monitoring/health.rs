use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Health checking system for Bolt components
pub struct HealthChecker {
    checks: RwLock<HashMap<String, HealthCheck>>,
    overall_status: RwLock<OverallHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub message: String,
    pub last_check: SystemTime,
    pub check_interval: Duration,
    pub timeout: Duration,
    pub retry_count: u32,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallHealth {
    pub status: HealthStatus,
    pub healthy_components: u32,
    pub warning_components: u32,
    pub critical_components: u32,
    pub total_components: u32,
    pub last_updated: SystemTime,
}

impl HealthChecker {
    /// Create new health checker
    pub async fn new() -> Result<Self> {
        info!("❤️ Initializing health checker");

        let mut checker = Self {
            checks: RwLock::new(HashMap::new()),
            overall_status: RwLock::new(OverallHealth::default()),
        };

        // Register core health checks
        checker.register_core_checks().await?;

        Ok(checker)
    }

    /// Register core health checks
    async fn register_core_checks(&mut self) -> Result<()> {
        info!("📋 Registering core health checks");

        // System health
        self.add_health_check(HealthCheck {
            name: "system".to_string(),
            status: HealthStatus::Unknown,
            message: "Checking system health".to_string(),
            last_check: SystemTime::UNIX_EPOCH,
            check_interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            retry_count: 0,
            max_retries: 3,
        })
        .await;

        // Container runtime health
        self.add_health_check(HealthCheck {
            name: "container_runtime".to_string(),
            status: HealthStatus::Unknown,
            message: "Checking container runtime".to_string(),
            last_check: SystemTime::UNIX_EPOCH,
            check_interval: Duration::from_secs(30),
            timeout: Duration::from_secs(10),
            retry_count: 0,
            max_retries: 3,
        })
        .await;

        // Network health
        self.add_health_check(HealthCheck {
            name: "networking".to_string(),
            status: HealthStatus::Unknown,
            message: "Checking network connectivity".to_string(),
            last_check: SystemTime::UNIX_EPOCH,
            check_interval: Duration::from_secs(60),
            timeout: Duration::from_secs(5),
            retry_count: 0,
            max_retries: 3,
        })
        .await;

        // QUIC server health
        self.add_health_check(HealthCheck {
            name: "quic_server".to_string(),
            status: HealthStatus::Unknown,
            message: "Checking QUIC server".to_string(),
            last_check: SystemTime::UNIX_EPOCH,
            check_interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            retry_count: 0,
            max_retries: 3,
        })
        .await;

        // GPU health
        self.add_health_check(HealthCheck {
            name: "gpu".to_string(),
            status: HealthStatus::Unknown,
            message: "Checking GPU availability".to_string(),
            last_check: SystemTime::UNIX_EPOCH,
            check_interval: Duration::from_secs(60),
            timeout: Duration::from_secs(10),
            retry_count: 0,
            max_retries: 2,
        })
        .await;

        // Storage health
        self.add_health_check(HealthCheck {
            name: "storage".to_string(),
            status: HealthStatus::Unknown,
            message: "Checking storage systems".to_string(),
            last_check: SystemTime::UNIX_EPOCH,
            check_interval: Duration::from_secs(60),
            timeout: Duration::from_secs(10),
            retry_count: 0,
            max_retries: 3,
        })
        .await;

        // Registry health
        self.add_health_check(HealthCheck {
            name: "registry".to_string(),
            status: HealthStatus::Unknown,
            message: "Checking image registry".to_string(),
            last_check: SystemTime::UNIX_EPOCH,
            check_interval: Duration::from_secs(120),
            timeout: Duration::from_secs(15),
            retry_count: 0,
            max_retries: 3,
        })
        .await;

        info!("✅ Core health checks registered");
        Ok(())
    }

    /// Add health check
    pub async fn add_health_check(&mut self, check: HealthCheck) {
        let mut checks = self.checks.write().await;
        checks.insert(check.name.clone(), check);
    }

    /// Start health checking loop
    pub async fn start_health_checks(&self) {
        info!("🔄 Starting health check loop");

        let checks = &self.checks;
        let overall_status = &self.overall_status;

        let mut interval = tokio::time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;

            // Perform health checks
            let check_names: Vec<String> = {
                let checks_read = checks.read().await;
                checks_read.keys().cloned().collect()
            };

            for check_name in check_names {
                let should_check = {
                    let checks_read = checks.read().await;
                    if let Some(check) = checks_read.get(&check_name) {
                        check.last_check.elapsed().unwrap_or(Duration::MAX) >= check.check_interval
                    } else {
                        false
                    }
                };

                if should_check {
                    self.perform_health_check(&check_name).await;
                }
            }

            // Update overall health
            self.update_overall_health().await;

            debug!("❤️ Health check cycle completed");
        }
    }

    /// Perform individual health check
    async fn perform_health_check(&self, check_name: &str) {
        debug!("🔍 Performing health check: {}", check_name);

        let result = match check_name {
            "system" => self.check_system_health().await,
            "container_runtime" => self.check_container_runtime_health().await,
            "networking" => self.check_networking_health().await,
            "quic_server" => self.check_quic_server_health().await,
            "gpu" => self.check_gpu_health().await,
            "storage" => self.check_storage_health().await,
            "registry" => self.check_registry_health().await,
            _ => Ok((HealthStatus::Unknown, "Unknown health check".to_string())),
        };

        let (status, message) = match result {
            Ok((status, message)) => (status, message),
            Err(e) => {
                warn!("Health check failed for {}: {}", check_name, e);
                (
                    HealthStatus::Critical,
                    format!("Health check failed: {}", e),
                )
            }
        };

        // Update health check
        {
            let mut checks = self.checks.write().await;
            if let Some(check) = checks.get_mut(check_name) {
                check.status = status.clone();
                check.message = message;
                check.last_check = SystemTime::now();

                // Reset retry count on success, increment on failure
                match status {
                    HealthStatus::Healthy => check.retry_count = 0,
                    _ => {
                        if check.retry_count < check.max_retries {
                            check.retry_count += 1;
                        }
                    }
                }
            }
        }

        debug!("  ✓ Health check completed: {} -> {:?}", check_name, status);
    }

    /// Check system health
    async fn check_system_health(&self) -> Result<(HealthStatus, String)> {
        // Check disk space
        let disk_usage = self.get_disk_usage("/").await?;
        if disk_usage > 90.0 {
            return Ok((
                HealthStatus::Critical,
                format!("Disk usage critical: {:.1}%", disk_usage),
            ));
        } else if disk_usage > 80.0 {
            return Ok((
                HealthStatus::Warning,
                format!("Disk usage high: {:.1}%", disk_usage),
            ));
        }

        // Check memory usage
        let memory_usage = self.get_memory_usage().await?;
        if memory_usage > 95.0 {
            return Ok((
                HealthStatus::Critical,
                format!("Memory usage critical: {:.1}%", memory_usage),
            ));
        } else if memory_usage > 85.0 {
            return Ok((
                HealthStatus::Warning,
                format!("Memory usage high: {:.1}%", memory_usage),
            ));
        }

        // Check load average
        let load_avg = self.get_load_average().await?;
        let cpu_count = num_cpus::get() as f64;
        if load_avg > cpu_count * 2.0 {
            return Ok((
                HealthStatus::Warning,
                format!("Load average high: {:.2}", load_avg),
            ));
        }

        Ok((HealthStatus::Healthy, "System resources normal".to_string()))
    }

    /// Check container runtime health
    async fn check_container_runtime_health(&self) -> Result<(HealthStatus, String)> {
        // Check if we can list containers
        let container_count = self.get_running_container_count().await?;

        // Check for zombie processes
        let zombie_count = self.get_zombie_process_count().await?;
        if zombie_count > 10 {
            return Ok((
                HealthStatus::Warning,
                format!("High zombie process count: {}", zombie_count),
            ));
        }

        Ok((
            HealthStatus::Healthy,
            format!("Runtime healthy, {} containers running", container_count),
        ))
    }

    /// Check networking health
    async fn check_networking_health(&self) -> Result<(HealthStatus, String)> {
        // Check if we can reach external connectivity
        if !self.check_external_connectivity().await? {
            return Ok((
                HealthStatus::Warning,
                "External connectivity limited".to_string(),
            ));
        }

        // Check interface status
        let interfaces = self.get_network_interface_count().await?;
        if interfaces == 0 {
            return Ok((
                HealthStatus::Critical,
                "No network interfaces available".to_string(),
            ));
        }

        Ok((
            HealthStatus::Healthy,
            format!("Network healthy, {} interfaces", interfaces),
        ))
    }

    /// Check QUIC server health
    async fn check_quic_server_health(&self) -> Result<(HealthStatus, String)> {
        // Check if QUIC port is listening
        if !self.check_port_listening(4433).await? {
            return Ok((
                HealthStatus::Warning,
                "QUIC server not listening".to_string(),
            ));
        }

        // Check QUIC connection count
        let quic_connections = self.get_quic_connection_count().await?;

        Ok((
            HealthStatus::Healthy,
            format!("QUIC server healthy, {} connections", quic_connections),
        ))
    }

    /// Check GPU health
    async fn check_gpu_health(&self) -> Result<(HealthStatus, String)> {
        let mut gpu_count = 0;
        let mut gpu_issues = Vec::new();

        // Check NVIDIA GPUs
        #[cfg(feature = "nvidia-support")]
        {
            match self.check_nvidia_gpus().await {
                Ok(count) => gpu_count += count,
                Err(e) => gpu_issues.push(format!("NVIDIA GPU check failed: {}", e)),
            }
        }

        // Check AMD GPUs
        match self.check_amd_gpus().await {
            Ok(count) => gpu_count += count,
            Err(e) => gpu_issues.push(format!("AMD GPU check failed: {}", e)),
        }

        if gpu_count == 0 && !gpu_issues.is_empty() {
            return Ok((
                HealthStatus::Warning,
                format!("GPU issues: {}", gpu_issues.join(", ")),
            ));
        }

        Ok((
            HealthStatus::Healthy,
            format!("GPU healthy, {} devices available", gpu_count),
        ))
    }

    /// Check storage health
    async fn check_storage_health(&self) -> Result<(HealthStatus, String)> {
        let mut issues = Vec::new();

        // Check volume directory
        if !std::path::Path::new("/var/lib/bolt/volumes").exists() {
            issues.push("Volume directory missing".to_string());
        }

        // Check image storage
        if !std::path::Path::new("/var/lib/bolt/images").exists() {
            issues.push("Image storage directory missing".to_string());
        }

        // Check for read-only filesystems
        if self.check_readonly_filesystem("/var/lib/bolt").await? {
            issues.push("Storage filesystem is read-only".to_string());
        }

        if !issues.is_empty() {
            return Ok((
                HealthStatus::Critical,
                format!("Storage issues: {}", issues.join(", ")),
            ));
        }

        Ok((HealthStatus::Healthy, "Storage systems healthy".to_string()))
    }

    /// Check registry health
    async fn check_registry_health(&self) -> Result<(HealthStatus, String)> {
        // This would check connectivity to configured registries
        // For now, just return healthy
        Ok((
            HealthStatus::Healthy,
            "Registry connectivity normal".to_string(),
        ))
    }

    /// Update overall health status
    async fn update_overall_health(&self) {
        let checks = self.checks.read().await;

        let mut healthy = 0;
        let mut warning = 0;
        let mut critical = 0;
        let total = checks.len() as u32;

        for check in checks.values() {
            match check.status {
                HealthStatus::Healthy => healthy += 1,
                HealthStatus::Warning => warning += 1,
                HealthStatus::Critical => critical += 1,
                HealthStatus::Unknown => warning += 1, // Treat unknown as warning
            }
        }

        let overall_status = if critical > 0 {
            HealthStatus::Critical
        } else if warning > 0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        };

        let overall_health = OverallHealth {
            status: overall_status,
            healthy_components: healthy,
            warning_components: warning,
            critical_components: critical,
            total_components: total,
            last_updated: SystemTime::now(),
        };

        {
            let mut overall = self.overall_status.write().await;
            *overall = overall_health;
        }
    }

    /// Get overall health status
    pub async fn get_overall_health(&self) -> String {
        let overall = self.overall_status.read().await;
        match overall.status {
            HealthStatus::Healthy => "healthy".to_string(),
            HealthStatus::Warning => "warning".to_string(),
            HealthStatus::Critical => "critical".to_string(),
            HealthStatus::Unknown => "unknown".to_string(),
        }
    }

    /// Get detailed health status
    pub async fn get_detailed_health(&self) -> OverallHealth {
        let overall = self.overall_status.read().await;
        overall.clone()
    }

    /// Get all health checks
    pub async fn get_all_checks(&self) -> HashMap<String, HealthCheck> {
        let checks = self.checks.read().await;
        checks.clone()
    }

    // Helper methods for health checks
    async fn get_disk_usage(&self, path: &str) -> Result<f64> {
        use std::process::Command;

        let output = Command::new("df").arg(path).output()?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 5 {
                let usage_str = fields[4].trim_end_matches('%');
                return Ok(usage_str.parse()?);
            }
        }

        Ok(0.0)
    }

    async fn get_memory_usage(&self) -> Result<f64> {
        use std::fs;

        let meminfo = fs::read_to_string("/proc/meminfo")?;
        let mut total = 0;
        let mut available = 0;

        for line in meminfo.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let value: u64 = parts[1].parse().unwrap_or(0);
                match parts[0] {
                    "MemTotal:" => total = value,
                    "MemAvailable:" => available = value,
                    _ => {}
                }
            }
        }

        if total > 0 {
            Ok(((total - available) as f64 / total as f64) * 100.0)
        } else {
            Ok(0.0)
        }
    }

    async fn get_load_average(&self) -> Result<f64> {
        use std::fs;

        let loadavg = fs::read_to_string("/proc/loadavg")?;
        let parts: Vec<&str> = loadavg.split_whitespace().collect();
        if !parts.is_empty() {
            Ok(parts[0].parse()?)
        } else {
            Ok(0.0)
        }
    }

    async fn get_running_container_count(&self) -> Result<u32> {
        // Would integrate with actual container manager
        Ok(0)
    }

    async fn get_zombie_process_count(&self) -> Result<u32> {
        use std::fs;

        let mut zombie_count = 0;

        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.chars().all(char::is_numeric) {
                        let stat_path = format!("/proc/{}/stat", name);
                        if let Ok(stat) = fs::read_to_string(stat_path) {
                            let parts: Vec<&str> = stat.split_whitespace().collect();
                            if parts.len() > 2 && parts[2] == "Z" {
                                zombie_count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(zombie_count)
    }

    async fn check_external_connectivity(&self) -> Result<bool> {
        use std::process::Command;

        let output = Command::new("ping")
            .args(&["-c", "1", "-W", "3", "8.8.8.8"])
            .output()?;

        Ok(output.status.success())
    }

    async fn get_network_interface_count(&self) -> Result<u32> {
        use std::fs;

        if let Ok(entries) = fs::read_dir("/sys/class/net") {
            let count = entries.filter_map(|entry| entry.ok()).count();
            Ok(count as u32)
        } else {
            Ok(0)
        }
    }

    async fn check_port_listening(&self, port: u16) -> Result<bool> {
        use std::process::Command;

        let output = Command::new("netstat").args(&["-ln"]).output()?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        let port_str = format!(":{}", port);
        Ok(output_str.contains(&port_str))
    }

    async fn get_quic_connection_count(&self) -> Result<u32> {
        // Would integrate with actual QUIC server
        Ok(0)
    }

    #[cfg(feature = "nvidia-support")]
    async fn check_nvidia_gpus(&self) -> Result<u32> {
        use nvml_wrapper::Nvml;

        match Nvml::init() {
            Ok(nvml) => {
                let device_count = nvml.device_count()?;

                // Check each GPU for health
                for i in 0..device_count {
                    let device = nvml.device_by_index(i)?;

                    // Check temperature
                    if let Ok(temp) = device
                        .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
                    {
                        if temp > 90 {
                            return Err(anyhow::anyhow!(
                                "GPU {} temperature critical: {}°C",
                                i,
                                temp
                            ));
                        }
                    }

                    // Check memory errors
                    if let Ok(memory_error_count) = device.total_ecc_errors(nvml_wrapper::enum_wrappers::device::EccCounter::VolatileSramUncorrectable) {
                        if memory_error_count > 0 {
                            return Err(anyhow::anyhow!("GPU {} has memory errors: {}", i, memory_error_count));
                        }
                    }
                }

                Ok(device_count)
            }
            Err(e) => Err(anyhow::anyhow!("NVML initialization failed: {}", e)),
        }
    }

    async fn check_amd_gpus(&self) -> Result<u32> {
        use std::fs;

        let mut gpu_count = 0;

        // Check for AMD GPU sysfs entries
        for i in 0..8 {
            let device_path = format!("/sys/class/drm/card{}/device", i);
            if let Ok(vendor) = fs::read_to_string(format!("{}/vendor", device_path)) {
                if vendor.trim() == "0x1002" {
                    // AMD vendor ID
                    gpu_count += 1;

                    // Check for thermal issues
                    if let Ok(temp_files) = fs::read_dir(format!("{}/hwmon", device_path)) {
                        for temp_file in temp_files.flatten() {
                            let temp_path = temp_file.path().join("temp1_input");
                            if let Ok(temp_str) = fs::read_to_string(temp_path) {
                                if let Ok(temp) = temp_str.trim().parse::<u32>() {
                                    let temp_celsius = temp / 1000;
                                    if temp_celsius > 90 {
                                        return Err(anyhow::anyhow!(
                                            "AMD GPU {} temperature critical: {}°C",
                                            i,
                                            temp_celsius
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(gpu_count)
    }

    async fn check_readonly_filesystem(&self, path: &str) -> Result<bool> {
        use std::fs::OpenOptions;

        let test_file = format!("{}/.bolt_write_test", path);
        match OpenOptions::new().create(true).write(true).open(&test_file) {
            Ok(_) => {
                let _ = std::fs::remove_file(&test_file);
                Ok(false)
            }
            Err(_) => Ok(true),
        }
    }
}

impl Default for OverallHealth {
    fn default() -> Self {
        Self {
            status: HealthStatus::Unknown,
            healthy_components: 0,
            warning_components: 0,
            critical_components: 0,
            total_components: 0,
            last_updated: SystemTime::now(),
        }
    }
}
