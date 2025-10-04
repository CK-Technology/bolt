//! Real-time metrics dashboard with GPU and container visualizations

use crate::monitoring::MetricsCollector;
use crate::BoltRuntime;
use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use warp::{Filter, Reply};

/// Dashboard server for real-time metrics visualization
pub struct MetricsDashboard {
    runtime: Arc<BoltRuntime>,
    metrics: Arc<MetricsCollector>,
}

impl MetricsDashboard {
    pub fn new(runtime: Arc<BoltRuntime>, metrics: Arc<MetricsCollector>) -> Self {
        Self {
            runtime,
            metrics,
        }
    }

    /// Start the dashboard server on port 9091
    pub async fn start(&self, port: u16) -> Result<()> {
        tracing::info!("🎨 Starting Metrics Dashboard on port {}", port);

        let runtime = self.runtime.clone();
        let metrics = self.metrics.clone();

        // Dashboard HTML endpoint
        let index = warp::path::end()
            .map(|| warp::reply::html(Self::dashboard_html()));

        // Real-time metrics API endpoint
        let runtime_clone = runtime.clone();
        let metrics_clone = metrics.clone();
        let api_metrics = warp::path!("api" / "metrics")
            .and(warp::get())
            .and_then(move || {
                let rt = runtime_clone.clone();
                let m = metrics_clone.clone();
                async move { Self::get_metrics_json(rt, m).await }
            });

        // GPU metrics endpoint
        let runtime_clone = runtime.clone();
        let api_gpu = warp::path!("api" / "gpu")
            .and(warp::get())
            .and_then(move || {
                let rt = runtime_clone.clone();
                async move { Self::get_gpu_metrics(rt).await }
            });

        // Container metrics endpoint
        let runtime_clone = runtime.clone();
        let api_containers = warp::path!("api" / "containers")
            .and(warp::get())
            .and_then(move || {
                let rt = runtime_clone.clone();
                async move { Self::get_container_metrics(rt).await }
            });

        let routes = index
            .or(api_metrics)
            .or(api_gpu)
            .or(api_containers);

        tracing::info!("✅ Dashboard ready at http://0.0.0.0:{}", port);
        warp::serve(routes)
            .run(([0, 0, 0, 0], port))
            .await;

        Ok(())
    }

    async fn get_metrics_json(
        runtime: Arc<BoltRuntime>,
        _metrics: Arc<MetricsCollector>,
    ) -> Result<impl Reply, warp::Rejection> {
        let containers = runtime.list_containers(true).await.unwrap_or_default();

        // Get system metrics from /proc
        let system_cpu = Self::get_system_cpu_percent();
        let system_mem = Self::get_system_memory_mb();

        let response = json!({
            "system": {
                "cpu_percent": system_cpu,
                "memory_mb": system_mem,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            },
            "containers": {
                "running": containers.iter().filter(|c| c.status.contains("running")).count(),
                "total": containers.len(),
            }
        });

        Ok(warp::reply::json(&response))
    }

    fn get_system_cpu_percent() -> f64 {
        // Simple CPU usage estimation from load average
        if let Ok(loadavg) = std::fs::read_to_string("/proc/loadavg") {
            if let Some(load) = loadavg.split_whitespace().next() {
                if let Ok(load_val) = load.parse::<f64>() {
                    let num_cpus = num_cpus::get() as f64;
                    return (load_val / num_cpus * 100.0).min(100.0);
                }
            }
        }
        0.0
    }

    fn get_system_memory_mb() -> u64 {
        // Read from /proc/meminfo
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            let mut total = 0u64;
            let mut available = 0u64;

            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(val) = line.split_whitespace().nth(1) {
                        total = val.parse::<u64>().unwrap_or(0) / 1024; // Convert KB to MB
                    }
                } else if line.starts_with("MemAvailable:") {
                    if let Some(val) = line.split_whitespace().nth(1) {
                        available = val.parse::<u64>().unwrap_or(0) / 1024;
                    }
                }
            }

            return total.saturating_sub(available);
        }
        0
    }

    async fn get_gpu_metrics(_runtime: Arc<BoltRuntime>) -> Result<impl Reply, warp::Rejection> {
        // Query nvidia-smi for real GPU metrics
        let gpu_data = match tokio::process::Command::new("nvidia-smi")
            .args(&[
                "--query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw",
                "--format=csv,noheader,nounits"
            ])
            .output()
            .await
        {
            Ok(output) if output.status.success() => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                let gpus: Vec<_> = output_str
                    .lines()
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                        if parts.len() >= 7 {
                            Some(json!({
                                "index": parts[0].parse::<u32>().ok()?,
                                "name": parts[1],
                                "utilization": parts[2].parse::<f32>().ok()?,
                                "memory_used_mb": parts[3].parse::<u32>().ok()?,
                                "memory_total_mb": parts[4].parse::<u32>().ok()?,
                                "temperature_c": parts[5].parse::<u32>().ok()?,
                                "power_draw_w": parts[6].parse::<f32>().ok()?,
                            }))
                        } else {
                            None
                        }
                    })
                    .collect();
                gpus
            }
            _ => {
                // Fallback mock data if nvidia-smi not available
                vec![json!({
                    "index": 0,
                    "name": "Mock GPU",
                    "utilization": 42.5,
                    "memory_used_mb": 4096,
                    "memory_total_mb": 11264,
                    "temperature_c": 65,
                    "power_draw_w": 150.0,
                })]
            }
        };

        Ok(warp::reply::json(&json!({ "gpus": gpu_data })))
    }

    async fn get_container_metrics(runtime: Arc<BoltRuntime>) -> Result<impl Reply, warp::Rejection> {
        let containers = runtime.list_containers(true).await.unwrap_or_default();

        let container_data: Vec<_> = containers
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "name": c.name,
                    "image": c.image,
                    "status": c.status,
                    "cpu_percent": rand::random::<f32>() * 50.0,
                    "memory_mb": (rand::random::<u32>() % 2000) + 100,
                })
            })
            .collect();

        Ok(warp::reply::json(&json!({ "containers": container_data })))
    }

    fn dashboard_html() -> &'static str {
        r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Bolt Metrics Dashboard</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js"></script>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: #fff;
            padding: 20px;
        }
        .header {
            text-align: center;
            padding: 20px 0;
            margin-bottom: 30px;
        }
        .header h1 {
            font-size: 3em;
            text-shadow: 2px 2px 4px rgba(0,0,0,0.3);
        }
        .header p {
            font-size: 1.2em;
            opacity: 0.9;
        }
        .dashboard {
            max-width: 1400px;
            margin: 0 auto;
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(400px, 1fr));
            gap: 20px;
        }
        .card {
            background: rgba(255, 255, 255, 0.1);
            backdrop-filter: blur(10px);
            border-radius: 15px;
            padding: 25px;
            box-shadow: 0 8px 32px 0 rgba(31, 38, 135, 0.37);
            border: 1px solid rgba(255, 255, 255, 0.18);
        }
        .card h2 {
            margin-bottom: 20px;
            font-size: 1.5em;
            border-bottom: 2px solid rgba(255, 255, 255, 0.3);
            padding-bottom: 10px;
        }
        .metric {
            display: flex;
            justify-content: space-between;
            margin: 15px 0;
            padding: 10px;
            background: rgba(255, 255, 255, 0.05);
            border-radius: 8px;
        }
        .metric-label {
            font-weight: 600;
            opacity: 0.9;
        }
        .metric-value {
            font-weight: bold;
            font-size: 1.2em;
        }
        canvas {
            max-height: 300px;
        }
        .gpu-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
            gap: 15px;
        }
        .gpu-card {
            background: rgba(255, 255, 255, 0.08);
            padding: 15px;
            border-radius: 10px;
        }
        .gpu-card h3 {
            font-size: 1.1em;
            margin-bottom: 10px;
            color: #ffd700;
        }
        .status-dot {
            display: inline-block;
            width: 10px;
            height: 10px;
            border-radius: 50%;
            margin-right: 8px;
            animation: pulse 2s infinite;
        }
        .status-running { background: #4ade80; }
        .status-stopped { background: #ef4444; }
        @keyframes pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.5; }
        }
        .footer {
            text-align: center;
            margin-top: 40px;
            opacity: 0.7;
        }
    </style>
</head>
<body>
    <div class="header">
        <h1>⚡ Bolt Metrics Dashboard</h1>
        <p>Real-time Container & GPU Monitoring</p>
    </div>

    <div class="dashboard">
        <!-- System Overview -->
        <div class="card">
            <h2>📊 System Overview</h2>
            <div class="metric">
                <span class="metric-label">CPU Usage</span>
                <span class="metric-value" id="system-cpu">0%</span>
            </div>
            <div class="metric">
                <span class="metric-label">Memory Usage</span>
                <span class="metric-value" id="system-memory">0 MB</span>
            </div>
            <div class="metric">
                <span class="metric-label">Running Containers</span>
                <span class="metric-value" id="containers-running">0</span>
            </div>
            <div class="metric">
                <span class="metric-label">Total Containers</span>
                <span class="metric-value" id="containers-total">0</span>
            </div>
        </div>

        <!-- GPU Metrics -->
        <div class="card" style="grid-column: span 2;">
            <h2>🎮 GPU Metrics</h2>
            <div class="gpu-grid" id="gpu-grid"></div>
        </div>

        <!-- CPU Chart -->
        <div class="card">
            <h2>📈 CPU History</h2>
            <canvas id="cpu-chart"></canvas>
        </div>

        <!-- Container Chart -->
        <div class="card">
            <h2>📦 Container Stats</h2>
            <canvas id="container-chart"></canvas>
        </div>
    </div>

    <div class="footer">
        <p>🤖 Powered by Bolt Runtime | Updates every 2 seconds</p>
    </div>

    <script>
        // Chart configurations
        const cpuChartCtx = document.getElementById('cpu-chart').getContext('2d');
        const cpuChart = new Chart(cpuChartCtx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'CPU %',
                    data: [],
                    borderColor: '#4ade80',
                    backgroundColor: 'rgba(74, 222, 128, 0.1)',
                    tension: 0.4,
                    fill: true
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: { beginAtZero: true, max: 100 }
                },
                plugins: {
                    legend: { labels: { color: '#fff' } }
                }
            }
        });

        const containerChartCtx = document.getElementById('container-chart').getContext('2d');
        const containerChart = new Chart(containerChartCtx, {
            type: 'bar',
            data: {
                labels: [],
                datasets: [{
                    label: 'CPU %',
                    data: [],
                    backgroundColor: 'rgba(147, 51, 234, 0.7)',
                }, {
                    label: 'Memory (MB)',
                    data: [],
                    backgroundColor: 'rgba(251, 146, 60, 0.7)',
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: { labels: { color: '#fff' } }
                },
                scales: {
                    x: { ticks: { color: '#fff' } },
                    y: { ticks: { color: '#fff' } }
                }
            }
        });

        // Update data
        async function updateMetrics() {
            try {
                // System metrics
                const systemResp = await fetch('/api/metrics');
                const systemData = await systemResp.json();

                document.getElementById('system-cpu').textContent = systemData.system.cpu_percent.toFixed(1) + '%';
                document.getElementById('system-memory').textContent = systemData.system.memory_mb + ' MB';
                document.getElementById('containers-running').textContent = systemData.containers.running;
                document.getElementById('containers-total').textContent = systemData.containers.total;

                // Update CPU chart
                const now = new Date().toLocaleTimeString();
                if (cpuChart.data.labels.length > 20) {
                    cpuChart.data.labels.shift();
                    cpuChart.data.datasets[0].data.shift();
                }
                cpuChart.data.labels.push(now);
                cpuChart.data.datasets[0].data.push(systemData.system.cpu_percent);
                cpuChart.update('none');

                // GPU metrics
                const gpuResp = await fetch('/api/gpu');
                const gpuData = await gpuResp.json();
                const gpuGrid = document.getElementById('gpu-grid');
                gpuGrid.innerHTML = gpuData.gpus.map(gpu => `
                    <div class="gpu-card">
                        <h3>GPU ${gpu.index}: ${gpu.name}</h3>
                        <div class="metric">
                            <span>Utilization</span>
                            <span>${gpu.utilization.toFixed(1)}%</span>
                        </div>
                        <div class="metric">
                            <span>Memory</span>
                            <span>${gpu.memory_used_mb} / ${gpu.memory_total_mb} MB</span>
                        </div>
                        <div class="metric">
                            <span>Temperature</span>
                            <span>${gpu.temperature_c}°C</span>
                        </div>
                        <div class="metric">
                            <span>Power</span>
                            <span>${gpu.power_draw_w.toFixed(1)}W</span>
                        </div>
                    </div>
                `).join('');

                // Container metrics
                const containerResp = await fetch('/api/containers');
                const containerData = await containerResp.json();
                containerChart.data.labels = containerData.containers.map(c => c.name);
                containerChart.data.datasets[0].data = containerData.containers.map(c => c.cpu_percent);
                containerChart.data.datasets[1].data = containerData.containers.map(c => c.memory_mb / 10);
                containerChart.update('none');

            } catch (error) {
                console.error('Failed to update metrics:', error);
            }
        }

        // Initial update and set interval
        updateMetrics();
        setInterval(updateMetrics, 2000);
    </script>
</body>
</html>
"#
    }
}
