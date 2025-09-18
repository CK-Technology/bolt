use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{info, warn};
use warp::Filter;

use super::{
    ContainerMetrics, GPUMetrics, MetricsCollector, NetworkMetrics, RuntimeMetrics, StorageMetrics,
    SystemMetrics,
};

/// Prometheus metrics exporter for Bolt
pub struct PrometheusExporter {
    metrics_collector: Arc<MetricsCollector>,
    server_addr: String,
    custom_metrics: Arc<RwLock<HashMap<String, CustomMetric>>>,
}

#[derive(Debug, Clone)]
pub struct CustomMetric {
    pub name: String,
    pub metric_type: MetricType,
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub help: String,
}

#[derive(Debug, Clone)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

impl PrometheusExporter {
    /// Create new Prometheus exporter
    pub async fn new(metrics_collector: Arc<MetricsCollector>) -> Result<Self> {
        info!("📊 Initializing Prometheus metrics exporter");

        Ok(Self {
            metrics_collector,
            server_addr: "0.0.0.0:9090".to_string(),
            custom_metrics: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Start Prometheus metrics server
    pub async fn start_server(&self) -> Result<()> {
        info!(
            "🚀 Starting Prometheus metrics server on {}",
            self.server_addr
        );

        let metrics_collector = Arc::clone(&self.metrics_collector);
        let custom_metrics = Arc::clone(&self.custom_metrics);

        // Create metrics endpoint
        let metrics_route = warp::path("metrics").and(warp::get()).and_then(move || {
            let metrics_collector = Arc::clone(&metrics_collector);
            let custom_metrics = Arc::clone(&custom_metrics);
            async move {
                match generate_prometheus_metrics(metrics_collector, custom_metrics).await {
                    Ok(metrics) => Ok(warp::reply::with_header(
                        metrics,
                        "content-type",
                        "text/plain; version=0.0.4; charset=utf-8",
                    )),
                    Err(e) => {
                        warn!("Failed to generate metrics: {}", e);
                        Err(warp::reject::custom(MetricsError))
                    }
                }
            }
        });

        // Create health endpoint
        let health_route = warp::path("health").and(warp::get()).map(|| {
            warp::reply::json(&serde_json::json!({
                "status": "healthy",
                "timestamp": SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            }))
        });

        // Create info endpoint
        let info_route = warp::path("info").and(warp::get()).map(|| {
            warp::reply::json(&serde_json::json!({
                "bolt_version": env!("CARGO_PKG_VERSION"),
                "prometheus_exporter": "bolt-monitoring",
                "metrics_endpoint": "/metrics",
                "health_endpoint": "/health"
            }))
        });

        let routes = metrics_route.or(health_route).or(info_route);

        // Parse bind address
        let bind_addr: std::net::SocketAddr = self.server_addr.parse()?;

        tokio::spawn(async move {
            warp::serve(routes).run(bind_addr).await;
        });

        info!("✅ Prometheus metrics server started successfully");
        info!(
            "📊 Metrics available at: http://{}/metrics",
            self.server_addr
        );
        info!("❤️ Health check at: http://{}/health", self.server_addr);

        Ok(())
    }

    /// Generate Prometheus metrics output
    pub async fn generate_metrics(&self) -> String {
        match generate_prometheus_metrics(
            Arc::clone(&self.metrics_collector),
            Arc::clone(&self.custom_metrics),
        )
        .await
        {
            Ok(metrics) => metrics,
            Err(e) => {
                warn!("Failed to generate metrics: {}", e);
                String::new()
            }
        }
    }

    /// Add custom metric
    pub async fn add_custom_metric(&self, metric: CustomMetric) {
        let mut custom_metrics = self.custom_metrics.write().await;
        custom_metrics.insert(metric.name.clone(), metric);
    }

    /// Record API request
    pub async fn record_api_request(
        &self,
        method: &str,
        endpoint: &str,
        status_code: u16,
        duration_ms: f64,
    ) {
        let labels: HashMap<String, String> = vec![
            ("method".to_string(), method.to_string()),
            ("endpoint".to_string(), endpoint.to_string()),
            ("status".to_string(), status_code.to_string()),
        ]
        .into_iter()
        .collect();

        // Request counter
        let counter_metric = CustomMetric {
            name: "bolt_api_requests_total".to_string(),
            metric_type: MetricType::Counter,
            value: 1.0,
            labels: labels.clone(),
            help: "Total number of API requests".to_string(),
        };

        // Duration histogram
        let duration_metric = CustomMetric {
            name: "bolt_api_request_duration_seconds".to_string(),
            metric_type: MetricType::Histogram,
            value: duration_ms / 1000.0,
            labels,
            help: "API request duration in seconds".to_string(),
        };

        self.add_custom_metric(counter_metric).await;
        self.add_custom_metric(duration_metric).await;
    }

    /// Record container operation
    pub async fn record_container_operation(
        &self,
        operation: &str,
        container_id: &str,
        success: bool,
        duration_ms: f64,
    ) {
        let labels: HashMap<String, String> = vec![
            ("operation".to_string(), operation.to_string()),
            ("container_id".to_string(), container_id.to_string()),
            ("success".to_string(), success.to_string()),
        ]
        .into_iter()
        .collect();

        let metric = CustomMetric {
            name: "bolt_container_operations_total".to_string(),
            metric_type: MetricType::Counter,
            value: 1.0,
            labels: labels.clone(),
            help: "Total number of container operations".to_string(),
        };

        self.add_custom_metric(metric).await;

        // Duration metric
        let duration_metric = CustomMetric {
            name: "bolt_container_operation_duration_seconds".to_string(),
            metric_type: MetricType::Histogram,
            value: duration_ms / 1000.0,
            labels,
            help: "Container operation duration in seconds".to_string(),
        };

        self.add_custom_metric(duration_metric).await;
    }
}

/// Generate Prometheus metrics format
async fn generate_prometheus_metrics(
    metrics_collector: Arc<MetricsCollector>,
    custom_metrics: Arc<RwLock<HashMap<String, CustomMetric>>>,
) -> Result<String> {
    let mut output = String::new();

    // Add header comment
    output.push_str("# Bolt Container Runtime Metrics\n");
    output.push_str(&format!(
        "# Generated at: {}\n",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    ));
    output.push('\n');

    // System metrics
    let system_metrics = metrics_collector.get_system_metrics().await;
    output.push_str(&format_system_metrics(&system_metrics));

    // Container metrics
    output.push_str(&format_container_metrics(&metrics_collector).await);

    // GPU metrics
    output.push_str(&format_gpu_metrics(&metrics_collector).await);

    // Network metrics
    output.push_str(&format_network_metrics(&metrics_collector).await);

    // Storage metrics
    output.push_str(&format_storage_metrics(&metrics_collector).await);

    // Runtime metrics
    output.push_str(&format_runtime_metrics(&metrics_collector).await);

    // Custom metrics
    output.push_str(&format_custom_metrics(&custom_metrics).await);

    Ok(output)
}

/// Format system metrics for Prometheus
fn format_system_metrics(metrics: &SystemMetrics) -> String {
    let mut output = String::new();

    // System uptime
    output.push_str("# HELP bolt_system_uptime_seconds System uptime in seconds\n");
    output.push_str("# TYPE bolt_system_uptime_seconds gauge\n");
    output.push_str(&format!(
        "bolt_system_uptime_seconds {}\n",
        metrics.uptime_seconds
    ));
    output.push('\n');

    // Load average
    output.push_str("# HELP bolt_system_load_average System load average\n");
    output.push_str("# TYPE bolt_system_load_average gauge\n");
    output.push_str(&format!(
        "bolt_system_load_average{{period=\"1m\"}} {}\n",
        metrics.load_average[0]
    ));
    output.push_str(&format!(
        "bolt_system_load_average{{period=\"5m\"}} {}\n",
        metrics.load_average[1]
    ));
    output.push_str(&format!(
        "bolt_system_load_average{{period=\"15m\"}} {}\n",
        metrics.load_average[2]
    ));
    output.push('\n');

    // CPU metrics
    output.push_str("# HELP bolt_system_cpu_usage_percent System CPU usage percentage\n");
    output.push_str("# TYPE bolt_system_cpu_usage_percent gauge\n");
    output.push_str(&format!(
        "bolt_system_cpu_usage_percent {}\n",
        metrics.cpu_usage_percent
    ));
    output.push('\n');

    output.push_str("# HELP bolt_system_cpu_count Number of CPU cores\n");
    output.push_str("# TYPE bolt_system_cpu_count gauge\n");
    output.push_str(&format!("bolt_system_cpu_count {}\n", metrics.cpu_count));
    output.push('\n');

    // Memory metrics
    output.push_str("# HELP bolt_system_memory_bytes System memory in bytes\n");
    output.push_str("# TYPE bolt_system_memory_bytes gauge\n");
    output.push_str(&format!(
        "bolt_system_memory_bytes{{type=\"total\"}} {}\n",
        metrics.memory_total_bytes
    ));
    output.push_str(&format!(
        "bolt_system_memory_bytes{{type=\"used\"}} {}\n",
        metrics.memory_used_bytes
    ));
    output.push_str(&format!(
        "bolt_system_memory_bytes{{type=\"available\"}} {}\n",
        metrics.memory_available_bytes
    ));
    output.push('\n');

    // Swap metrics
    output.push_str("# HELP bolt_system_swap_bytes System swap in bytes\n");
    output.push_str("# TYPE bolt_system_swap_bytes gauge\n");
    output.push_str(&format!(
        "bolt_system_swap_bytes{{type=\"total\"}} {}\n",
        metrics.swap_total_bytes
    ));
    output.push_str(&format!(
        "bolt_system_swap_bytes{{type=\"used\"}} {}\n",
        metrics.swap_used_bytes
    ));
    output.push('\n');

    // Disk metrics
    output.push_str("# HELP bolt_system_disk_bytes System disk usage in bytes\n");
    output.push_str("# TYPE bolt_system_disk_bytes gauge\n");
    output.push_str(&format!(
        "bolt_system_disk_bytes{{type=\"total\"}} {}\n",
        metrics.disk_total_bytes
    ));
    output.push_str(&format!(
        "bolt_system_disk_bytes{{type=\"used\"}} {}\n",
        metrics.disk_used_bytes
    ));
    output.push('\n');

    // Process metrics
    output.push_str("# HELP bolt_system_processes Total number of processes\n");
    output.push_str("# TYPE bolt_system_processes gauge\n");
    output.push_str(&format!(
        "bolt_system_processes {{}} {}\n",
        metrics.processes_total
    ));
    output.push('\n');

    // Network connections
    output.push_str("# HELP bolt_system_network_connections Active network connections\n");
    output.push_str("# TYPE bolt_system_network_connections gauge\n");
    output.push_str(&format!(
        "bolt_system_network_connections {}\n",
        metrics.network_connections
    ));
    output.push('\n');

    output
}

/// Format container metrics for Prometheus
async fn format_container_metrics(metrics_collector: &Arc<MetricsCollector>) -> String {
    let mut output = String::new();
    let container_metrics = metrics_collector.container_metrics.read().await;

    if !container_metrics.is_empty() {
        // Container CPU usage
        output.push_str("# HELP bolt_container_cpu_usage_percent Container CPU usage percentage\n");
        output.push_str("# TYPE bolt_container_cpu_usage_percent gauge\n");
        for (_, metric) in container_metrics.iter() {
            output.push_str(&format!(
                "bolt_container_cpu_usage_percent{{container_id=\"{}\",name=\"{}\"}} {}\n",
                metric.container_id, metric.name, metric.cpu_usage_percent
            ));
        }
        output.push('\n');

        // Container memory usage
        output.push_str("# HELP bolt_container_memory_bytes Container memory usage in bytes\n");
        output.push_str("# TYPE bolt_container_memory_bytes gauge\n");
        for (_, metric) in container_metrics.iter() {
            output.push_str(&format!(
                "bolt_container_memory_bytes{{container_id=\"{}\",name=\"{}\",type=\"used\"}} {}\n",
                metric.container_id, metric.name, metric.memory_usage_bytes
            ));
            output.push_str(&format!(
                "bolt_container_memory_bytes{{container_id=\"{}\",name=\"{}\",type=\"limit\"}} {}\n",
                metric.container_id, metric.name, metric.memory_limit_bytes
            ));
        }
        output.push('\n');

        // Container network metrics
        output.push_str("# HELP bolt_container_network_bytes Container network traffic in bytes\n");
        output.push_str("# TYPE bolt_container_network_bytes counter\n");
        for (_, metric) in container_metrics.iter() {
            output.push_str(&format!(
                "bolt_container_network_bytes{{container_id=\"{}\",name=\"{}\",direction=\"rx\"}} {}\n",
                metric.container_id, metric.name, metric.network_rx_bytes
            ));
            output.push_str(&format!(
                "bolt_container_network_bytes{{container_id=\"{}\",name=\"{}\",direction=\"tx\"}} {}\n",
                metric.container_id, metric.name, metric.network_tx_bytes
            ));
        }
        output.push('\n');

        // Container uptime
        output.push_str("# HELP bolt_container_uptime_seconds Container uptime in seconds\n");
        output.push_str("# TYPE bolt_container_uptime_seconds gauge\n");
        for (_, metric) in container_metrics.iter() {
            output.push_str(&format!(
                "bolt_container_uptime_seconds{{container_id=\"{}\",name=\"{}\"}} {}\n",
                metric.container_id, metric.name, metric.uptime_seconds
            ));
        }
        output.push('\n');
    }

    output
}

/// Format GPU metrics for Prometheus
async fn format_gpu_metrics(metrics_collector: &Arc<MetricsCollector>) -> String {
    let mut output = String::new();
    let gpu_metrics = metrics_collector.gpu_metrics.read().await;

    if !gpu_metrics.is_empty() {
        // GPU utilization
        output.push_str("# HELP bolt_gpu_utilization_percent GPU utilization percentage\n");
        output.push_str("# TYPE bolt_gpu_utilization_percent gauge\n");
        for (_, metric) in gpu_metrics.iter() {
            output.push_str(&format!(
                "bolt_gpu_utilization_percent{{gpu_id=\"{}\",name=\"{}\",vendor=\"{}\"}} {}\n",
                metric.gpu_id, metric.gpu_name, metric.gpu_vendor, metric.utilization_percent
            ));
        }
        output.push('\n');

        // GPU memory
        output.push_str("# HELP bolt_gpu_memory_bytes GPU memory in bytes\n");
        output.push_str("# TYPE bolt_gpu_memory_bytes gauge\n");
        for (_, metric) in gpu_metrics.iter() {
            output.push_str(&format!(
                "bolt_gpu_memory_bytes{{gpu_id=\"{}\",name=\"{}\",type=\"used\"}} {}\n",
                metric.gpu_id, metric.gpu_name, metric.memory_used_bytes
            ));
            output.push_str(&format!(
                "bolt_gpu_memory_bytes{{gpu_id=\"{}\",name=\"{}\",type=\"total\"}} {}\n",
                metric.gpu_id, metric.gpu_name, metric.memory_total_bytes
            ));
        }
        output.push('\n');

        // GPU temperature
        output.push_str("# HELP bolt_gpu_temperature_celsius GPU temperature in Celsius\n");
        output.push_str("# TYPE bolt_gpu_temperature_celsius gauge\n");
        for (_, metric) in gpu_metrics.iter() {
            output.push_str(&format!(
                "bolt_gpu_temperature_celsius{{gpu_id=\"{}\",name=\"{}\"}} {}\n",
                metric.gpu_id, metric.gpu_name, metric.temperature_celsius
            ));
        }
        output.push('\n');

        // GPU power usage
        output.push_str("# HELP bolt_gpu_power_watts GPU power usage in watts\n");
        output.push_str("# TYPE bolt_gpu_power_watts gauge\n");
        for (_, metric) in gpu_metrics.iter() {
            output.push_str(&format!(
                "bolt_gpu_power_watts{{gpu_id=\"{}\",name=\"{}\"}} {}\n",
                metric.gpu_id, metric.gpu_name, metric.power_usage_watts
            ));
        }
        output.push('\n');
    }

    output
}

/// Format network metrics for Prometheus
async fn format_network_metrics(metrics_collector: &Arc<MetricsCollector>) -> String {
    let mut output = String::new();
    let network_metrics = metrics_collector.network_metrics.read().await;

    if !network_metrics.is_empty() {
        // Network bytes
        output.push_str("# HELP bolt_network_bytes Network traffic in bytes\n");
        output.push_str("# TYPE bolt_network_bytes counter\n");
        for (_, metric) in network_metrics.iter() {
            let container_label = metric
                .container_id
                .as_ref()
                .map(|id| format!(",container_id=\"{}\"", id))
                .unwrap_or_default();

            output.push_str(&format!(
                "bolt_network_bytes{{interface=\"{}\"{},direction=\"rx\"}} {}\n",
                metric.interface_name, container_label, metric.rx_bytes
            ));
            output.push_str(&format!(
                "bolt_network_bytes{{interface=\"{}\"{},direction=\"tx\"}} {}\n",
                metric.interface_name, container_label, metric.tx_bytes
            ));
        }
        output.push('\n');

        // Network packets
        output.push_str("# HELP bolt_network_packets Network packets\n");
        output.push_str("# TYPE bolt_network_packets counter\n");
        for (_, metric) in network_metrics.iter() {
            let container_label = metric
                .container_id
                .as_ref()
                .map(|id| format!(",container_id=\"{}\"", id))
                .unwrap_or_default();

            output.push_str(&format!(
                "bolt_network_packets{{interface=\"{}\"{},direction=\"rx\"}} {}\n",
                metric.interface_name, container_label, metric.rx_packets
            ));
            output.push_str(&format!(
                "bolt_network_packets{{interface=\"{}\"{},direction=\"tx\"}} {}\n",
                metric.interface_name, container_label, metric.tx_packets
            ));
        }
        output.push('\n');
    }

    output
}

/// Format storage metrics for Prometheus
async fn format_storage_metrics(metrics_collector: &Arc<MetricsCollector>) -> String {
    let mut output = String::new();
    let storage_metrics = metrics_collector.storage_metrics.read().await;

    if !storage_metrics.is_empty() {
        // Storage bytes
        output.push_str("# HELP bolt_storage_bytes Storage usage in bytes\n");
        output.push_str("# TYPE bolt_storage_bytes gauge\n");
        for (_, metric) in storage_metrics.iter() {
            output.push_str(&format!(
                "bolt_storage_bytes{{volume=\"{}\",mount=\"{}\",type=\"total\"}} {}\n",
                metric.volume_name, metric.mount_point, metric.total_bytes
            ));
            output.push_str(&format!(
                "bolt_storage_bytes{{volume=\"{}\",mount=\"{}\",type=\"used\"}} {}\n",
                metric.volume_name, metric.mount_point, metric.used_bytes
            ));
            output.push_str(&format!(
                "bolt_storage_bytes{{volume=\"{}\",mount=\"{}\",type=\"available\"}} {}\n",
                metric.volume_name, metric.mount_point, metric.available_bytes
            ));
        }
        output.push('\n');

        // Storage IOPS
        output.push_str("# HELP bolt_storage_iops Storage operations per second\n");
        output.push_str("# TYPE bolt_storage_iops gauge\n");
        for (_, metric) in storage_metrics.iter() {
            output.push_str(&format!(
                "bolt_storage_iops{{volume=\"{}\",mount=\"{}\",operation=\"read\"}} {}\n",
                metric.volume_name, metric.mount_point, metric.read_iops
            ));
            output.push_str(&format!(
                "bolt_storage_iops{{volume=\"{}\",mount=\"{}\",operation=\"write\"}} {}\n",
                metric.volume_name, metric.mount_point, metric.write_iops
            ));
        }
        output.push('\n');
    }

    output
}

/// Format runtime metrics for Prometheus
async fn format_runtime_metrics(metrics_collector: &Arc<MetricsCollector>) -> String {
    let mut output = String::new();
    let runtime_metrics = metrics_collector.runtime_metrics.read().await;

    // Bolt version info
    output.push_str("# HELP bolt_version_info Bolt version information\n");
    output.push_str("# TYPE bolt_version_info gauge\n");
    output.push_str(&format!(
        "bolt_version_info{{version=\"{}\"}} 1\n",
        runtime_metrics.bolt_version
    ));
    output.push('\n');

    // Container counts
    output.push_str("# HELP bolt_containers_total Total number of containers\n");
    output.push_str("# TYPE bolt_containers_total gauge\n");
    output.push_str(&format!(
        "bolt_containers_total{{state=\"running\"}} {}\n",
        runtime_metrics.containers_running
    ));
    output.push_str(&format!(
        "bolt_containers_total{{state=\"total\"}} {}\n",
        runtime_metrics.containers_total
    ));
    output.push('\n');

    // Image count
    output.push_str("# HELP bolt_images_total Total number of images\n");
    output.push_str("# TYPE bolt_images_total gauge\n");
    output.push_str(&format!(
        "bolt_images_total {}\n",
        runtime_metrics.images_total
    ));
    output.push('\n');

    // Volume count
    output.push_str("# HELP bolt_volumes_total Total number of volumes\n");
    output.push_str("# TYPE bolt_volumes_total gauge\n");
    output.push_str(&format!(
        "bolt_volumes_total {}\n",
        runtime_metrics.volumes_total
    ));
    output.push('\n');

    // Network count
    output.push_str("# HELP bolt_networks_total Total number of networks\n");
    output.push_str("# TYPE bolt_networks_total gauge\n");
    output.push_str(&format!(
        "bolt_networks_total {}\n",
        runtime_metrics.networks_total
    ));
    output.push('\n');

    // QUIC connections
    output.push_str("# HELP bolt_quic_connections Active QUIC connections\n");
    output.push_str("# TYPE bolt_quic_connections gauge\n");
    output.push_str(&format!(
        "bolt_quic_connections {}\n",
        runtime_metrics.quic_connections
    ));
    output.push('\n');

    // eBPF programs
    output.push_str("# HELP bolt_ebpf_programs_loaded Loaded eBPF programs\n");
    output.push_str("# TYPE bolt_ebpf_programs_loaded gauge\n");
    output.push_str(&format!(
        "bolt_ebpf_programs_loaded {}\n",
        runtime_metrics.ebpf_programs_loaded
    ));
    output.push('\n');

    output
}

/// Format custom metrics for Prometheus
async fn format_custom_metrics(
    custom_metrics: &Arc<RwLock<HashMap<String, CustomMetric>>>,
) -> String {
    let mut output = String::new();
    let metrics = custom_metrics.read().await;

    for (_, metric) in metrics.iter() {
        // Add help and type
        output.push_str(&format!("# HELP {} {}\n", metric.name, metric.help));
        output.push_str(&format!(
            "# TYPE {} {}\n",
            metric.name,
            match metric.metric_type {
                MetricType::Counter => "counter",
                MetricType::Gauge => "gauge",
                MetricType::Histogram => "histogram",
                MetricType::Summary => "summary",
            }
        ));

        // Add metric with labels
        let labels = if metric.labels.is_empty() {
            String::new()
        } else {
            let label_pairs: Vec<String> = metric
                .labels
                .iter()
                .map(|(k, v)| format!("{}=\"{}\"", k, v))
                .collect();
            format!("{{{}}}", label_pairs.join(","))
        };

        output.push_str(&format!("{}{} {}\n", metric.name, labels, metric.value));
        output.push('\n');
    }

    output
}

#[derive(Debug)]
struct MetricsError;

impl warp::reject::Reject for MetricsError {}
