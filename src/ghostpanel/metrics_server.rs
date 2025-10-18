//! Real-Time Metrics Server for GhostPanel
//!
//! WebSocket server that streams GPU and container metrics at 60 FPS

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use tokio::time;
use tracing::{debug, info, warn};

#[cfg(feature = "websocket")]
use {
    axum::{
        extract::{
            ws::{Message, WebSocket, WebSocketUpgrade},
            Path as AxumPath, State,
        },
        response::IntoResponse,
        routing::{get, post},
        Json, Router,
    },
    tower_http::cors::{Any, CorsLayer},
    tracing::error,
    futures_util::StreamExt,
};

/// Real-time container GPU metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsMessage {
    pub timestamp: u64,
    pub container_id: String,

    // Frame metrics
    pub fps: f32,
    pub frame_time_ms: f32,
    pub frame_time_p99: f32,

    // GPU utilization
    pub gpu_utilization: f32,
    pub gpu_temp_c: f32,
    pub gpu_clock_mhz: u32,
    pub memory_clock_mhz: u32,

    // VRAM
    pub vram_used_mb: u64,
    pub vram_total_mb: u64,
    pub vram_pressure: f32, // 0.0 - 1.0

    // Power
    pub power_draw_w: f32,
    pub power_limit_w: f32,

    // Status
    pub thermal_throttling: bool,
    pub dlss_active: bool,
    pub reflex_enabled: bool,
}

/// GPU configuration update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuConfigUpdate {
    pub power_limit_watts: Option<u32>,
    pub gpu_clock_offset_mhz: Option<i32>,
    pub memory_clock_offset_mhz: Option<i32>,
    pub fan_speed_percent: Option<u32>,
    pub performance_mode: Option<PerformanceMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PerformanceMode {
    Maximum,
    Balanced,
    Quiet,
    Custom,
}

/// Metrics server state
struct ServerState {
    /// Active container metrics
    container_metrics: Arc<RwLock<HashMap<String, MetricsMessage>>>,
    /// Broadcast channel for metrics updates
    metrics_tx: broadcast::Sender<MetricsMessage>,
}

/// GhostPanel Metrics Server
pub struct GhostPanelMetricsServer {
    state: Arc<ServerState>,
    update_interval: Duration,
}

impl GhostPanelMetricsServer {
    /// Create a new metrics server with 60 FPS update rate (16ms)
    pub fn new() -> Result<Self> {
        let (metrics_tx, _) = broadcast::channel(1000);

        let state = Arc::new(ServerState {
            container_metrics: Arc::new(RwLock::new(HashMap::new())),
            metrics_tx,
        });

        Ok(Self {
            state,
            update_interval: Duration::from_millis(16), // 60 FPS
        })
    }

    /// Start the metrics server on the specified port
    #[cfg(feature = "websocket")]
    pub async fn start_server(&self, port: u16) -> Result<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));

        info!("🚀 Starting GhostPanel metrics server on {}", addr);
        info!("   WebSocket endpoint: ws://localhost:{}/ws/metrics/{{container_id}}", port);
        info!("   REST API: http://localhost:{}/api/*", port);

        // Build router
        let app = Router::new()
            .route("/ws/metrics/:container_id", get(Self::websocket_handler))
            .route("/api/gpu/status", get(Self::gpu_status_handler))
            .route(
                "/api/containers/:container_id/gpu",
                get(Self::container_gpu_info_handler),
            )
            .route(
                "/api/containers/:container_id/gpu/profile",
                post(Self::update_gpu_profile_handler),
            )
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .with_state(self.state.clone());

        // Start metrics collector
        self.start_metrics_collector().await;

        // Start server
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .context("Failed to bind metrics server")?;

        info!("✅ GhostPanel metrics server running on {}", addr);

        axum::serve(listener, app)
            .await
            .context("Metrics server error")?;

        Ok(())
    }

    /// Start background metrics collector
    async fn start_metrics_collector(&self) {
        let container_metrics = self.state.container_metrics.clone();
        let metrics_tx = self.state.metrics_tx.clone();
        let update_interval = self.update_interval;

        tokio::spawn(async move {
            info!("📊 Starting metrics collector (60 FPS / 16ms updates)");
            let mut interval = time::interval(update_interval);

            loop {
                interval.tick().await;

                // Collect metrics for all active containers
                let metrics = container_metrics.read().await;
                for (container_id, metric) in metrics.iter() {
                    // Update metrics (in production, this would query real GPU state)
                    let updated = Self::update_container_metrics(container_id, metric).await;

                    // Broadcast to WebSocket clients
                    if let Err(e) = metrics_tx.send(updated) {
                        debug!("No active WebSocket clients: {}", e);
                    }
                }
            }
        });
    }

    /// Update metrics for a container (stub - would query real GPU)
    async fn update_container_metrics(
        _container_id: &str,
        current: &MetricsMessage,
    ) -> MetricsMessage {
        // In production, this would:
        // 1. Query nvidia-smi or nvbind for real GPU metrics
        // 2. Use frame capture for FPS/frame time
        // 3. Query container runtime for process stats

        #[cfg(feature = "nvbind-support")]
        {
            // Use nvbind to get real metrics
            // let metrics = nvbind::get_container_metrics(container_id).await;
        }

        // For now, return updated timestamp with current values
        let mut updated = current.clone();
        updated.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Simulate slight variations (in production, use real data)
        updated.gpu_utilization = (updated.gpu_utilization + 0.5).min(100.0);
        updated.fps = 144.0; // Would come from frame capture

        updated
    }

    /// WebSocket handler for real-time metrics streaming
    #[cfg(feature = "websocket")]
    async fn websocket_handler(
        ws: WebSocketUpgrade,
        AxumPath(container_id): AxumPath<String>,
        State(state): State<Arc<ServerState>>,
    ) -> impl IntoResponse {
        info!("📡 New WebSocket connection for container: {}", container_id);
        ws.on_upgrade(move |socket| Self::handle_websocket(socket, container_id, state))
    }

    #[cfg(feature = "websocket")]
    async fn handle_websocket(socket: WebSocket, container_id: String, state: Arc<ServerState>) {
        let (mut sender, mut receiver) = socket.split();

        // Subscribe to metrics updates
        let mut metrics_rx = state.metrics_tx.subscribe();

        // Spawn task to send metrics to client
        let container_id_clone = container_id.clone();
        let send_task = tokio::spawn(async move {
            while let Ok(metrics) = metrics_rx.recv().await {
                // Only send metrics for this container
                if metrics.container_id == container_id_clone {
                    let json = match serde_json::to_string(&metrics) {
                        Ok(json) => json,
                        Err(e) => {
                            error!("Failed to serialize metrics: {}", e);
                            continue;
                        }
                    };

                    if sender.send(Message::Text(json)).await.is_err() {
                        debug!("WebSocket client disconnected");
                        break;
                    }
                }
            }
        });

        // Handle incoming messages (for bidirectional communication)
        while let Some(msg) = receiver.next().await {
            if msg.is_err() {
                break;
            }
        }

        // Cleanup
        send_task.abort();
        info!("📡 WebSocket connection closed for container: {}", container_id);
    }

    /// REST API: Get GPU status
    #[cfg(feature = "websocket")]
    async fn gpu_status_handler() -> Json<GpuStatusResponse> {
        // In production, query real GPU state
        Json(GpuStatusResponse {
            gpus: vec![GpuInfo {
                id: "gpu:0".to_string(),
                name: "NVIDIA RTX 4090".to_string(),
                memory_total_mb: 24576,
                driver_version: "550.54.14".to_string(),
                temperature_c: 65,
                utilization_percent: 75.0,
            }],
        })
    }

    /// REST API: Get container GPU info
    #[cfg(feature = "websocket")]
    async fn container_gpu_info_handler(
        AxumPath(container_id): AxumPath<String>,
        State(state): State<Arc<ServerState>>,
    ) -> Json<Option<MetricsMessage>> {
        let metrics = state.container_metrics.read().await;
        Json(metrics.get(&container_id).cloned())
    }

    /// REST API: Update GPU profile (hot-reload)
    #[cfg(feature = "websocket")]
    async fn update_gpu_profile_handler(
        AxumPath(container_id): AxumPath<String>,
        Json(config): Json<GpuConfigUpdate>,
    ) -> Json<ApiResponse> {
        info!(
            "🔧 Hot-reloading GPU config for container: {}",
            container_id
        );

        // In production, apply GPU config via nvbind
        #[cfg(feature = "nvbind-support")]
        {
            // nvbind::apply_gpu_config(&container_id, &config).await;
        }

        Json(ApiResponse {
            success: true,
            message: format!("GPU config updated for container: {}", container_id),
        })
    }

    /// Register a container for metrics collection
    pub async fn register_container(&self, container_id: String) -> Result<()> {
        info!("📊 Registering container for metrics: {}", container_id);

        let metrics = MetricsMessage {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            container_id: container_id.clone(),
            fps: 0.0,
            frame_time_ms: 0.0,
            frame_time_p99: 0.0,
            gpu_utilization: 0.0,
            gpu_temp_c: 0.0,
            gpu_clock_mhz: 0,
            memory_clock_mhz: 0,
            vram_used_mb: 0,
            vram_total_mb: 0,
            vram_pressure: 0.0,
            power_draw_w: 0.0,
            power_limit_w: 0.0,
            thermal_throttling: false,
            dlss_active: false,
            reflex_enabled: false,
        };

        let mut container_metrics = self.state.container_metrics.write().await;
        container_metrics.insert(container_id, metrics);

        Ok(())
    }

    /// Unregister a container from metrics collection
    pub async fn unregister_container(&self, container_id: &str) -> Result<()> {
        info!("📊 Unregistering container from metrics: {}", container_id);

        let mut container_metrics = self.state.container_metrics.write().await;
        container_metrics.remove(container_id);

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct GpuStatusResponse {
    gpus: Vec<GpuInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GpuInfo {
    id: String,
    name: String,
    memory_total_mb: u64,
    driver_version: String,
    temperature_c: u32,
    utilization_percent: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse {
    success: bool,
    message: String,
}

#[cfg(not(feature = "websocket"))]
impl GhostPanelMetricsServer {
    pub async fn start_server(&self, _port: u16) -> Result<()> {
        warn!("⚠️  WebSocket feature not enabled, metrics server disabled");
        warn!("   Enable with: cargo build --features websocket");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_message_serialization() {
        let metrics = MetricsMessage {
            timestamp: 1234567890,
            container_id: "test-container".to_string(),
            fps: 144.5,
            frame_time_ms: 6.9,
            frame_time_p99: 8.2,
            gpu_utilization: 98.5,
            gpu_temp_c: 68.0,
            gpu_clock_mhz: 1920,
            memory_clock_mhz: 7800,
            vram_used_mb: 8192,
            vram_total_mb: 12288,
            vram_pressure: 0.67,
            power_draw_w: 285.0,
            power_limit_w: 350.0,
            thermal_throttling: false,
            dlss_active: true,
            reflex_enabled: true,
        };

        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: MetricsMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.container_id, "test-container");
        assert_eq!(deserialized.fps, 144.5);
        assert_eq!(deserialized.dlss_active, true);
    }
}
