use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

pub mod health;
pub mod prometheus;
pub mod tracing_setup;

/// Comprehensive monitoring system for Bolt
pub struct MonitoringSystem {
    metrics_collector: Arc<MetricsCollector>,
    health_checker: Arc<health::HealthChecker>,
    prometheus_exporter: Option<prometheus::PrometheusExporter>,
    tracing_config: tracing_setup::TracingConfig,
}

/// Central metrics collector
#[derive(Debug)]
pub struct MetricsCollector {
    container_metrics: Arc<RwLock<HashMap<String, ContainerMetrics>>>,
    gpu_metrics: Arc<RwLock<HashMap<String, GPUMetrics>>>,
    network_metrics: Arc<RwLock<HashMap<String, NetworkMetrics>>>,
    storage_metrics: Arc<RwLock<HashMap<String, StorageMetrics>>>,
    system_metrics: Arc<RwLock<SystemMetrics>>,
    runtime_metrics: Arc<RwLock<RuntimeMetrics>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerMetrics {
    pub container_id: String,
    pub name: String,
    pub status: String,
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub memory_limit_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub uptime_seconds: u64,
    pub restart_count: u32,
    pub exit_code: Option<i32>,
    pub last_updated: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUMetrics {
    pub gpu_id: String,
    pub gpu_name: String,
    pub gpu_vendor: String,
    pub utilization_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub temperature_celsius: f64,
    pub power_usage_watts: f64,
    pub fan_speed_percent: f64,
    pub container_assignments: Vec<String>,
    pub last_updated: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub interface_name: String,
    pub container_id: Option<String>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
    pub latency_ms: f64,
    pub bandwidth_mbps: f64,
    pub last_updated: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetrics {
    pub volume_name: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub inode_total: u64,
    pub inode_used: u64,
    pub read_iops: f64,
    pub write_iops: f64,
    pub read_throughput_mbps: f64,
    pub write_throughput_mbps: f64,
    pub container_usage: HashMap<String, u64>,
    pub last_updated: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub hostname: String,
    pub uptime_seconds: u64,
    pub load_average: [f64; 3],
    pub cpu_count: u32,
    pub cpu_usage_percent: f64,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub memory_available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub network_connections: u32,
    pub processes_total: u32,
    pub processes_running: u32,
    pub last_updated: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMetrics {
    pub bolt_version: String,
    pub containers_running: u32,
    pub containers_total: u32,
    pub images_total: u32,
    pub volumes_total: u32,
    pub networks_total: u32,
    pub quic_connections: u32,
    pub ebpf_programs_loaded: u32,
    pub gpu_containers: u32,
    pub average_startup_time_ms: f64,
    pub api_requests_total: u64,
    pub api_requests_failed: u64,
    pub last_updated: SystemTime,
}

impl MonitoringSystem {
    /// Create new monitoring system
    pub async fn new() -> Result<Self> {
        info!("🔍 Initializing comprehensive monitoring system");

        let metrics_collector = Arc::new(MetricsCollector::new().await?);
        let health_checker = Arc::new(health::HealthChecker::new().await?);
        let tracing_config = tracing_setup::TracingConfig::default();

        // Initialize Prometheus exporter
        let prometheus_exporter =
            Some(prometheus::PrometheusExporter::new(Arc::clone(&metrics_collector)).await?);

        let system = Self {
            metrics_collector,
            health_checker,
            prometheus_exporter,
            tracing_config,
        };

        // Start background monitoring tasks
        system.start_monitoring_tasks().await?;

        info!("✅ Monitoring system initialized successfully");
        Ok(system)
    }

    /// Start background monitoring tasks
    async fn start_monitoring_tasks(&self) -> Result<()> {
        info!("🚀 Starting background monitoring tasks");

        // Start metrics collection
        let metrics_collector = Arc::clone(&self.metrics_collector);
        tokio::spawn(async move {
            metrics_collector.start_collection_loop().await;
        });

        // Start health checking
        let health_checker = Arc::clone(&self.health_checker);
        tokio::spawn(async move {
            health_checker.start_health_checks().await;
        });

        // Start Prometheus server
        if let Some(ref exporter) = self.prometheus_exporter {
            exporter.start_server().await?;
        }

        Ok(())
    }

    /// Get current system status
    pub async fn get_system_status(&self) -> Result<SystemStatus> {
        let system_metrics = self.metrics_collector.get_system_metrics().await;
        let health_status = self.health_checker.get_overall_health().await;
        let container_count = self.metrics_collector.get_container_count().await;
        let gpu_count = self.metrics_collector.get_gpu_count().await;

        Ok(SystemStatus {
            overall_health: health_status,
            uptime_seconds: system_metrics.uptime_seconds,
            containers_running: container_count.running,
            containers_total: container_count.total,
            gpu_utilization: gpu_count.average_utilization,
            memory_usage_percent: (system_metrics.memory_used_bytes as f64
                / system_metrics.memory_total_bytes as f64)
                * 100.0,
            cpu_usage_percent: system_metrics.cpu_usage_percent,
            network_throughput_mbps: self.metrics_collector.get_total_network_throughput().await,
            last_updated: SystemTime::now(),
        })
    }

    /// Record container metric
    pub async fn record_container_metric(&self, metric: ContainerMetrics) {
        self.metrics_collector.record_container_metric(metric).await;
    }

    /// Record GPU metric
    pub async fn record_gpu_metric(&self, metric: GPUMetrics) {
        self.metrics_collector.record_gpu_metric(metric).await;
    }

    /// Get metrics for Prometheus export
    pub async fn get_prometheus_metrics(&self) -> String {
        if let Some(ref exporter) = self.prometheus_exporter {
            exporter.generate_metrics().await
        } else {
            String::new()
        }
    }
}

impl MetricsCollector {
    /// Create new metrics collector
    pub async fn new() -> Result<Self> {
        info!("📊 Initializing metrics collector");

        Ok(Self {
            container_metrics: Arc::new(RwLock::new(HashMap::new())),
            gpu_metrics: Arc::new(RwLock::new(HashMap::new())),
            network_metrics: Arc::new(RwLock::new(HashMap::new())),
            storage_metrics: Arc::new(RwLock::new(HashMap::new())),
            system_metrics: Arc::new(RwLock::new(SystemMetrics::default())),
            runtime_metrics: Arc::new(RwLock::new(RuntimeMetrics::default())),
        })
    }

    /// Start metrics collection loop
    pub async fn start_collection_loop(&self) {
        info!("🔄 Starting metrics collection loop");

        let mut interval = tokio::time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;

            // Collect system metrics
            if let Err(e) = self.collect_system_metrics().await {
                warn!("Failed to collect system metrics: {}", e);
            }

            // Collect container metrics
            if let Err(e) = self.collect_container_metrics().await {
                warn!("Failed to collect container metrics: {}", e);
            }

            // Collect GPU metrics
            if let Err(e) = self.collect_gpu_metrics().await {
                warn!("Failed to collect GPU metrics: {}", e);
            }

            // Collect network metrics
            if let Err(e) = self.collect_network_metrics().await {
                warn!("Failed to collect network metrics: {}", e);
            }

            // Collect storage metrics
            if let Err(e) = self.collect_storage_metrics().await {
                warn!("Failed to collect storage metrics: {}", e);
            }

            debug!("📈 Metrics collection cycle completed");
        }
    }

    /// Collect system metrics
    async fn collect_system_metrics(&self) -> Result<()> {
        // Get system information
        let uptime = self.get_system_uptime().await?;
        let load_avg = self.get_load_average().await?;
        let cpu_usage = self.get_cpu_usage().await?;
        let memory_info = self.get_memory_info().await?;
        let disk_info = self.get_disk_info().await?;

        let metrics = SystemMetrics {
            hostname: hostname::get()?.to_string_lossy().to_string(),
            uptime_seconds: uptime,
            load_average: load_avg,
            cpu_count: num_cpus::get() as u32,
            cpu_usage_percent: cpu_usage,
            memory_total_bytes: memory_info.total,
            memory_used_bytes: memory_info.used,
            memory_available_bytes: memory_info.available,
            swap_total_bytes: memory_info.swap_total,
            swap_used_bytes: memory_info.swap_used,
            disk_total_bytes: disk_info.total,
            disk_used_bytes: disk_info.used,
            network_connections: self.get_network_connections().await?,
            processes_total: self.get_process_count().await?,
            processes_running: self.get_running_process_count().await?,
            last_updated: SystemTime::now(),
        };

        {
            let mut system_metrics = self.system_metrics.write().await;
            *system_metrics = metrics;
        }

        Ok(())
    }

    /// Collect container metrics
    async fn collect_container_metrics(&self) -> Result<()> {
        // In a real implementation, this would:
        // 1. List all running containers
        // 2. Read cgroup stats for each container
        // 3. Calculate CPU/memory/network usage
        // 4. Update container metrics

        debug!("Collecting container metrics");
        Ok(())
    }

    /// Collect GPU metrics
    async fn collect_gpu_metrics(&self) -> Result<()> {
        // Use nvml-wrapper to collect NVIDIA GPU metrics
        #[cfg(feature = "nvidia-support")]
        {
            use nvml_wrapper::Nvml;

            match Nvml::init() {
                Ok(nvml) => {
                    if let Ok(device_count) = nvml.device_count() {
                        for i in 0..device_count {
                            if let Ok(device) = nvml.device_by_index(i) {
                                if let Ok(name) = device.name() {
                                    let gpu_metrics = GPUMetrics {
                                        gpu_id: format!("gpu-{}", i),
                                        gpu_name: name,
                                        gpu_vendor: "NVIDIA".to_string(),
                                        utilization_percent: device.utilization_rates().map(|u| u.gpu as f64).unwrap_or(0.0),
                                        memory_used_bytes: device.memory_info().map(|m| m.used).unwrap_or(0),
                                        memory_total_bytes: device.memory_info().map(|m| m.total).unwrap_or(0),
                                        temperature_celsius: device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu).unwrap_or(0) as f64,
                                        power_usage_watts: device.power_usage().unwrap_or(0) as f64 / 1000.0,
                                        fan_speed_percent: device.fan_speed(0).unwrap_or(0) as f64,
                                        container_assignments: Vec::new(), // Would be populated from container assignments
                                        last_updated: SystemTime::now(),
                                    };

                                    self.record_gpu_metric(gpu_metrics).await;
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    debug!("NVML not available, skipping NVIDIA GPU metrics");
                }
            }
        }

        // Collect AMD GPU metrics via sysfs
        self.collect_amd_gpu_metrics().await?;

        Ok(())
    }

    /// Collect AMD GPU metrics
    async fn collect_amd_gpu_metrics(&self) -> Result<()> {
        use std::fs;

        // Check for AMD GPU sysfs entries
        let amd_gpu_paths = ["/sys/class/drm/card0/device", "/sys/class/drm/card1/device"];

        for (i, path) in amd_gpu_paths.iter().enumerate() {
            if std::path::Path::new(path).exists() {
                let gpu_metrics = GPUMetrics {
                    gpu_id: format!("amd-gpu-{}", i),
                    gpu_name: self
                        .read_amd_gpu_name(path)
                        .await
                        .unwrap_or_else(|| "AMD GPU".to_string()),
                    gpu_vendor: "AMD".to_string(),
                    utilization_percent: self.read_amd_gpu_utilization(path).await.unwrap_or(0.0),
                    memory_used_bytes: self.read_amd_gpu_memory_used(path).await.unwrap_or(0),
                    memory_total_bytes: self.read_amd_gpu_memory_total(path).await.unwrap_or(0),
                    temperature_celsius: self.read_amd_gpu_temperature(path).await.unwrap_or(0.0),
                    power_usage_watts: self.read_amd_gpu_power(path).await.unwrap_or(0.0),
                    fan_speed_percent: 0.0, // Not easily available via sysfs
                    container_assignments: Vec::new(),
                    last_updated: SystemTime::now(),
                };

                self.record_gpu_metric(gpu_metrics).await;
            }
        }

        Ok(())
    }

    /// Collect network metrics
    async fn collect_network_metrics(&self) -> Result<()> {
        use std::fs;

        // Read network interface statistics from /proc/net/dev
        if let Ok(content) = fs::read_to_string("/proc/net/dev") {
            for line in content.lines().skip(2) {
                // Skip header lines
                let fields: Vec<&str> = line.split_whitespace().collect();
                if fields.len() >= 17 {
                    let interface_name = fields[0].trim_end_matches(':').to_string();

                    // Skip loopback and other virtual interfaces for main metrics
                    if !interface_name.starts_with("lo") && !interface_name.starts_with("docker") {
                        let network_metrics = NetworkMetrics {
                            interface_name: interface_name.clone(),
                            container_id: None, // Would be determined from interface mapping
                            rx_bytes: fields[1].parse().unwrap_or(0),
                            rx_packets: fields[2].parse().unwrap_or(0),
                            rx_errors: fields[3].parse().unwrap_or(0),
                            rx_dropped: fields[4].parse().unwrap_or(0),
                            tx_bytes: fields[9].parse().unwrap_or(0),
                            tx_packets: fields[10].parse().unwrap_or(0),
                            tx_errors: fields[11].parse().unwrap_or(0),
                            tx_dropped: fields[12].parse().unwrap_or(0),
                            latency_ms: 0.0,     // Would be measured separately
                            bandwidth_mbps: 0.0, // Would be calculated from rate
                            last_updated: SystemTime::now(),
                        };

                        {
                            let mut network_metrics_map = self.network_metrics.write().await;
                            network_metrics_map.insert(interface_name, network_metrics);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Collect storage metrics
    async fn collect_storage_metrics(&self) -> Result<()> {
        use std::process::Command;

        // Get filesystem usage with df command
        if let Ok(output) = Command::new("df").arg("-B1").output() {
            let output_str = String::from_utf8_lossy(&output.stdout);

            for line in output_str.lines().skip(1) {
                // Skip header
                let fields: Vec<&str> = line.split_whitespace().collect();
                if fields.len() >= 6 {
                    let mount_point = fields[5].to_string();

                    // Focus on main filesystem mounts
                    if mount_point == "/" || mount_point.starts_with("/var/lib/bolt") {
                        let storage_metrics = StorageMetrics {
                            volume_name: format!("fs-{}", mount_point.replace('/', "-")),
                            mount_point: mount_point.clone(),
                            total_bytes: fields[1].parse().unwrap_or(0),
                            used_bytes: fields[2].parse().unwrap_or(0),
                            available_bytes: fields[3].parse().unwrap_or(0),
                            inode_total: 0, // Would be collected separately
                            inode_used: 0,
                            read_iops: 0.0, // Would be measured from iostat
                            write_iops: 0.0,
                            read_throughput_mbps: 0.0,
                            write_throughput_mbps: 0.0,
                            container_usage: HashMap::new(),
                            last_updated: SystemTime::now(),
                        };

                        {
                            let mut storage_metrics_map = self.storage_metrics.write().await;
                            storage_metrics_map.insert(mount_point, storage_metrics);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Record container metric
    pub async fn record_container_metric(&self, metric: ContainerMetrics) {
        let mut container_metrics = self.container_metrics.write().await;
        container_metrics.insert(metric.container_id.clone(), metric);
    }

    /// Record GPU metric
    pub async fn record_gpu_metric(&self, metric: GPUMetrics) {
        let mut gpu_metrics = self.gpu_metrics.write().await;
        gpu_metrics.insert(metric.gpu_id.clone(), metric);
    }

    /// Get system metrics
    pub async fn get_system_metrics(&self) -> SystemMetrics {
        let system_metrics = self.system_metrics.read().await;
        system_metrics.clone()
    }

    /// Get container count
    pub async fn get_container_count(&self) -> ContainerCount {
        let container_metrics = self.container_metrics.read().await;
        let total = container_metrics.len() as u32;
        let running = container_metrics
            .values()
            .filter(|m| m.status == "running")
            .count() as u32;

        ContainerCount { total, running }
    }

    /// Get GPU count and utilization
    pub async fn get_gpu_count(&self) -> GPUCount {
        let gpu_metrics = self.gpu_metrics.read().await;
        let total = gpu_metrics.len() as u32;
        let average_utilization = if total > 0 {
            gpu_metrics
                .values()
                .map(|m| m.utilization_percent)
                .sum::<f64>()
                / total as f64
        } else {
            0.0
        };

        GPUCount {
            total,
            average_utilization,
        }
    }

    /// Get total network throughput
    pub async fn get_total_network_throughput(&self) -> f64 {
        let network_metrics = self.network_metrics.read().await;
        network_metrics.values().map(|m| m.bandwidth_mbps).sum()
    }

    // Helper methods for system metric collection
    async fn get_system_uptime(&self) -> Result<u64> {
        use std::fs;
        let uptime_content = fs::read_to_string("/proc/uptime")?;
        let uptime_seconds: f64 = uptime_content
            .split_whitespace()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Invalid uptime format"))?
            .parse()?;
        Ok(uptime_seconds as u64)
    }

    async fn get_load_average(&self) -> Result<[f64; 3]> {
        use std::fs;
        let loadavg_content = fs::read_to_string("/proc/loadavg")?;
        let parts: Vec<&str> = loadavg_content.split_whitespace().collect();
        if parts.len() >= 3 {
            Ok([parts[0].parse()?, parts[1].parse()?, parts[2].parse()?])
        } else {
            Ok([0.0, 0.0, 0.0])
        }
    }

    async fn get_cpu_usage(&self) -> Result<f64> {
        // Simple CPU usage calculation - would be improved with delta measurements
        Ok(0.0) // Placeholder
    }

    async fn get_memory_info(&self) -> Result<MemoryInfo> {
        use std::fs;
        let meminfo_content = fs::read_to_string("/proc/meminfo")?;

        let mut total = 0;
        let mut available = 0;
        let mut swap_total = 0;
        let mut swap_free = 0;

        for line in meminfo_content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let value: u64 = parts[1].parse().unwrap_or(0) * 1024; // Convert KB to bytes
                match parts[0] {
                    "MemTotal:" => total = value,
                    "MemAvailable:" => available = value,
                    "SwapTotal:" => swap_total = value,
                    "SwapFree:" => swap_free = value,
                    _ => {}
                }
            }
        }

        Ok(MemoryInfo {
            total,
            used: total - available,
            available,
            swap_total,
            swap_used: swap_total - swap_free,
        })
    }

    async fn get_disk_info(&self) -> Result<DiskInfo> {
        use std::process::Command;

        if let Ok(output) = Command::new("df").arg("-B1").arg("/").output() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines().skip(1) {
                let fields: Vec<&str> = line.split_whitespace().collect();
                if fields.len() >= 4 {
                    return Ok(DiskInfo {
                        total: fields[1].parse().unwrap_or(0),
                        used: fields[2].parse().unwrap_or(0),
                    });
                }
            }
        }

        Ok(DiskInfo { total: 0, used: 0 })
    }

    async fn get_network_connections(&self) -> Result<u32> {
        use std::fs;
        if let Ok(content) = fs::read_to_string("/proc/net/tcp") {
            Ok(content.lines().count() as u32 - 1) // Subtract header
        } else {
            Ok(0)
        }
    }

    async fn get_process_count(&self) -> Result<u32> {
        use std::fs;
        if let Ok(entries) = fs::read_dir("/proc") {
            let count = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .chars()
                        .all(char::is_numeric)
                })
                .count();
            Ok(count as u32)
        } else {
            Ok(0)
        }
    }

    async fn get_running_process_count(&self) -> Result<u32> {
        // Simplified - would parse /proc/*/stat for actual running processes
        Ok(0)
    }

    // AMD GPU helper methods
    async fn read_amd_gpu_name(&self, path: &str) -> Option<String> {
        use std::fs;
        fs::read_to_string(format!("{}/vendor", path))
            .ok()
            .and_then(|vendor| {
                if vendor.trim() == "0x1002" {
                    // AMD vendor ID
                    Some("AMD GPU".to_string())
                } else {
                    None
                }
            })
    }

    async fn read_amd_gpu_utilization(&self, _path: &str) -> Option<f64> {
        // Would read from GPU-specific sysfs files
        Some(0.0)
    }

    async fn read_amd_gpu_memory_used(&self, _path: &str) -> Option<u64> {
        Some(0)
    }

    async fn read_amd_gpu_memory_total(&self, _path: &str) -> Option<u64> {
        Some(0)
    }

    async fn read_amd_gpu_temperature(&self, _path: &str) -> Option<f64> {
        Some(0.0)
    }

    async fn read_amd_gpu_power(&self, _path: &str) -> Option<f64> {
        Some(0.0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemStatus {
    pub overall_health: String,
    pub uptime_seconds: u64,
    pub containers_running: u32,
    pub containers_total: u32,
    pub gpu_utilization: f64,
    pub memory_usage_percent: f64,
    pub cpu_usage_percent: f64,
    pub network_throughput_mbps: f64,
    pub last_updated: SystemTime,
}

#[derive(Debug)]
struct ContainerCount {
    total: u32,
    running: u32,
}

#[derive(Debug)]
struct GPUCount {
    total: u32,
    average_utilization: f64,
}

#[derive(Debug)]
struct MemoryInfo {
    total: u64,
    used: u64,
    available: u64,
    swap_total: u64,
    swap_used: u64,
}

#[derive(Debug)]
struct DiskInfo {
    total: u64,
    used: u64,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            hostname: "unknown".to_string(),
            uptime_seconds: 0,
            load_average: [0.0, 0.0, 0.0],
            cpu_count: num_cpus::get() as u32,
            cpu_usage_percent: 0.0,
            memory_total_bytes: 0,
            memory_used_bytes: 0,
            memory_available_bytes: 0,
            swap_total_bytes: 0,
            swap_used_bytes: 0,
            disk_total_bytes: 0,
            disk_used_bytes: 0,
            network_connections: 0,
            processes_total: 0,
            processes_running: 0,
            last_updated: UNIX_EPOCH,
        }
    }
}

impl Default for RuntimeMetrics {
    fn default() -> Self {
        Self {
            bolt_version: env!("CARGO_PKG_VERSION").to_string(),
            containers_running: 0,
            containers_total: 0,
            images_total: 0,
            volumes_total: 0,
            networks_total: 0,
            quic_connections: 0,
            ebpf_programs_loaded: 0,
            gpu_containers: 0,
            average_startup_time_ms: 0.0,
            api_requests_total: 0,
            api_requests_failed: 0,
            last_updated: UNIX_EPOCH,
        }
    }
}
