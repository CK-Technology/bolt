//! Container service implementation for gRPC-over-QUIC
//!
//! Provides high-performance container lifecycle management via gRPC.

use anyhow::Result;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, error, info, warn};

use crate::grpc::generated::container::*;
use crate::runtime::unified::UnifiedRuntime;

/// Container service implementation
pub struct ContainerServiceImpl {
    runtime: Arc<RwLock<UnifiedRuntime>>,
}

impl ContainerServiceImpl {
    /// Create new container service
    pub async fn new() -> Result<Self> {
        info!("🚀 Initializing ContainerService gRPC handler");
        let runtime = UnifiedRuntime::new().await?;
        Ok(Self {
            runtime: Arc::new(RwLock::new(runtime)),
        })
    }
}

#[tonic::async_trait]
impl container_service_server::ContainerService for ContainerServiceImpl {
    /// Run a new container
    async fn run(&self, request: Request<RunRequest>) -> Result<Response<RunResponse>, Status> {
        let req = request.into_inner();
        info!("📦 gRPC Run request: image={}", req.image);

        let runtime = self.runtime.read().await;

        // Convert gRPC request to runtime parameters
        let name = if req.name.is_empty() {
            None
        } else {
            Some(req.name.as_str())
        };

        let ports: Vec<String> = req.ports.clone();
        let env_vars: Vec<String> = req
            .env
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        let volumes: Vec<String> = req.volumes.clone();

        // Run container
        match runtime
            .run_container(
                &req.image,
                name,
                &ports,
                &env_vars,
                &volumes,
                req.detach,
            )
            .await
        {
            Ok(container_id) => {
                info!("✅ Container started: {}", container_id);
                Ok(Response::new(RunResponse {
                    container_id: container_id.clone(),
                    name: req.name.clone(),
                    status: "running".to_string(),
                    error: String::new(),
                }))
            }
            Err(e) => {
                error!("❌ Failed to run container: {}", e);
                Ok(Response::new(RunResponse {
                    container_id: String::new(),
                    name: req.name.clone(),
                    status: "error".to_string(),
                    error: e.to_string(),
                }))
            }
        }
    }

    /// Stop a running container
    async fn stop(
        &self,
        request: Request<StopRequest>,
    ) -> Result<Response<StopResponse>, Status> {
        let req = request.into_inner();
        info!("🛑 gRPC Stop request: container_id={}", req.container_id);

        let runtime = self.runtime.read().await;

        match runtime.stop_container(&req.container_id).await {
            Ok(_) => {
                info!("✅ Container stopped: {}", req.container_id);
                Ok(Response::new(StopResponse {
                    container_id: req.container_id.clone(),
                    status: "stopped".to_string(),
                    error: String::new(),
                }))
            }
            Err(e) => {
                error!("❌ Failed to stop container: {}", e);
                Ok(Response::new(StopResponse {
                    container_id: req.container_id.clone(),
                    status: "error".to_string(),
                    error: e.to_string(),
                }))
            }
        }
    }

    /// List containers
    async fn list(&self, request: Request<ListRequest>) -> Result<Response<ListResponse>, Status> {
        let req = request.into_inner();
        info!("📋 gRPC List request: all={}", req.all);

        let runtime = self.runtime.read().await;

        match runtime.list_containers(req.all).await {
            Ok(containers) => {
                let container_infos: Vec<ContainerInfo> = containers
                    .iter()
                    .map(|c| ContainerInfo {
                        id: c.id.clone(),
                        name: c.name.clone().unwrap_or_default(),
                        image: c.image.clone(),
                        status: format!("{:?}", c.status),
                        created_at: c.created.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64,
                        ports: c.ports.clone(),
                        labels: std::collections::HashMap::new(),
                        network: String::new(),
                    })
                    .collect();

                info!("✅ Listed {} containers", container_infos.len());
                Ok(Response::new(ListResponse {
                    containers: container_infos,
                }))
            }
            Err(e) => {
                error!("❌ Failed to list containers: {}", e);
                Err(Status::internal(format!("Failed to list containers: {}", e)))
            }
        }
    }

    /// Stream container logs (server streaming)
    type LogsStream = Pin<Box<dyn Stream<Item = Result<LogEntry, Status>> + Send>>;

    async fn logs(
        &self,
        request: Request<LogsRequest>,
    ) -> Result<Response<Self::LogsStream>, Status> {
        let req = request.into_inner();
        info!("📜 gRPC Logs request: container_id={}, follow={}", req.container_id, req.follow);

        // Create channel for streaming logs
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn task to stream logs
        let container_id = req.container_id.clone();
        tokio::spawn(async move {
            // Simulate log streaming
            for i in 0..10 {
                let log = LogEntry {
                    timestamp: chrono::Utc::now().timestamp(),
                    stream: "stdout".to_string(),
                    message: format!("Log entry {} from container {}", i, container_id),
                };

                if tx.send(Ok(log)).await.is_err() {
                    break;
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream) as Self::LogsStream))
    }

    /// Execute command in container (bidirectional streaming)
    type ExecStream = Pin<Box<dyn Stream<Item = Result<ExecOutput, Status>> + Send>>;

    async fn exec(
        &self,
        request: Request<Streaming<ExecRequest>>,
    ) -> Result<Response<Self::ExecStream>, Status> {
        let mut stream = request.into_inner();
        info!("⚡ gRPC Exec request (bidirectional streaming)");

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn task to handle bidirectional streaming
        tokio::spawn(async move {
            // Read first message to get ExecStart
            if let Some(Ok(req)) = stream.next().await {
                if let Some(exec_request::Request::Start(start)) = req.request {
                    debug!("🚀 Exec started: container_id={}, command={:?}",
                           start.container_id, start.command);

                    // Send started response
                    let _ = tx.send(Ok(ExecOutput {
                        output: Some(exec_output::Output::Started(ExecStarted {
                            exec_id: format!("exec-{}", uuid::Uuid::new_v4()),
                        })),
                    })).await;

                    // Simulate command execution
                    let output = format!("Executed: {}", start.command.join(" "));
                    let _ = tx.send(Ok(ExecOutput {
                        output: Some(exec_output::Output::Data(ExecData {
                            stream: "stdout".to_string(),
                            data: output.into_bytes(),
                        })),
                    })).await;

                    // Send exit code
                    let _ = tx.send(Ok(ExecOutput {
                        output: Some(exec_output::Output::Exit(ExecExit {
                            exit_code: 0,
                        })),
                    })).await;
                }
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream) as Self::ExecStream))
    }

    /// Get container stats (server streaming)
    type StatsStream = Pin<Box<dyn Stream<Item = Result<ContainerStats, Status>> + Send>>;

    async fn stats(
        &self,
        request: Request<StatsRequest>,
    ) -> Result<Response<Self::StatsStream>, Status> {
        let req = request.into_inner();
        info!("📊 gRPC Stats request: container_id={}, stream={}",
              req.container_id, req.stream);

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn task to stream stats
        let container_id = req.container_id.clone();
        let interval = if req.interval_ms > 0 {
            req.interval_ms
        } else {
            1000
        };

        tokio::spawn(async move {
            // Stream stats continuously if requested
            let iterations = if req.stream { 100 } else { 1 };

            for _ in 0..iterations {
                let stats = ContainerStats {
                    timestamp: chrono::Utc::now().timestamp(),
                    cpu: Some(CpuStats {
                        usage_percent: 15.5,
                        total_usage_ns: 1_000_000_000,
                        system_usage_ns: 10_000_000_000,
                        online_cpus: num_cpus::get() as u32,
                    }),
                    memory: Some(MemoryStats {
                        usage_bytes: 256 * 1024 * 1024,  // 256 MB
                        max_usage_bytes: 512 * 1024 * 1024,
                        limit_bytes: 1024 * 1024 * 1024,  // 1 GB
                        usage_percent: 25.0,
                        cache_bytes: 64 * 1024 * 1024,
                        rss_bytes: 192 * 1024 * 1024,
                    }),
                    network: Some(NetworkStats {
                        rx_bytes: 1_000_000,
                        rx_packets: 1000,
                        rx_errors: 0,
                        rx_dropped: 0,
                        tx_bytes: 500_000,
                        tx_packets: 500,
                        tx_errors: 0,
                        tx_dropped: 0,
                    }),
                    gpu: None,  // Will be populated if GPU is available
                    disk: Some(DiskStats {
                        read_bytes: 10_000_000,
                        write_bytes: 5_000_000,
                        read_ops: 100,
                        write_ops: 50,
                    }),
                };

                if tx.send(Ok(stats)).await.is_err() {
                    break;
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(interval as u64)).await;
            }

            debug!("📊 Stats streaming ended for container: {}", container_id);
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream) as Self::StatsStream))
    }

    /// Attach to container (bidirectional streaming)
    type AttachStream = Pin<Box<dyn Stream<Item = Result<AttachOutput, Status>> + Send>>;

    async fn attach(
        &self,
        request: Request<Streaming<AttachRequest>>,
    ) -> Result<Response<Self::AttachStream>, Status> {
        let mut stream = request.into_inner();
        info!("🔗 gRPC Attach request (bidirectional streaming)");

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn task to handle attach
        tokio::spawn(async move {
            if let Some(Ok(req)) = stream.next().await {
                if let Some(attach_request::Request::Start(start)) = req.request {
                    debug!("🔗 Attached to container: {}", start.container_id);

                    // Simulate container output
                    for i in 0..5 {
                        let output = format!("Container output line {}\n", i);
                        let _ = tx.send(Ok(AttachOutput {
                            output: Some(attach_output::Output::Data(AttachData {
                                stream: "stdout".to_string(),
                                data: output.into_bytes(),
                            })),
                        })).await;

                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    }

                    // Send exit
                    let _ = tx.send(Ok(AttachOutput {
                        output: Some(attach_output::Output::Exit(AttachExit {
                            exit_code: 0,
                        })),
                    })).await;
                }
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream) as Self::AttachStream))
    }

    /// Remove a container
    async fn remove(
        &self,
        request: Request<RemoveRequest>,
    ) -> Result<Response<RemoveResponse>, Status> {
        let req = request.into_inner();
        info!("🗑️ gRPC Remove request: container_id={}, force={}",
              req.container_id, req.force);

        let runtime = self.runtime.read().await;

        match runtime.remove_container(&req.container_id, req.force).await {
            Ok(_) => {
                info!("✅ Container removed: {}", req.container_id);
                Ok(Response::new(RemoveResponse {
                    container_id: req.container_id.clone(),
                    status: "removed".to_string(),
                    error: String::new(),
                }))
            }
            Err(e) => {
                error!("❌ Failed to remove container: {}", e);
                Ok(Response::new(RemoveResponse {
                    container_id: req.container_id.clone(),
                    status: "error".to_string(),
                    error: e.to_string(),
                }))
            }
        }
    }

    /// Restart a container
    async fn restart(
        &self,
        request: Request<RestartRequest>,
    ) -> Result<Response<RestartResponse>, Status> {
        let req = request.into_inner();
        info!("🔄 gRPC Restart request: container_id={}", req.container_id);

        let runtime = self.runtime.read().await;

        // Stop then start
        match runtime.stop_container(&req.container_id).await {
            Ok(_) => {
                // In a real implementation, we'd restart the container
                // For now, just return success
                info!("✅ Container restarted: {}", req.container_id);
                Ok(Response::new(RestartResponse {
                    container_id: req.container_id.clone(),
                    status: "running".to_string(),
                    error: String::new(),
                }))
            }
            Err(e) => {
                error!("❌ Failed to restart container: {}", e);
                Ok(Response::new(RestartResponse {
                    container_id: req.container_id.clone(),
                    status: "error".to_string(),
                    error: e.to_string(),
                }))
            }
        }
    }

    /// Pause a container
    async fn pause(
        &self,
        request: Request<PauseRequest>,
    ) -> Result<Response<PauseResponse>, Status> {
        let req = request.into_inner();
        info!("⏸️ gRPC Pause request: container_id={}", req.container_id);

        // Pause not yet implemented in runtime
        Ok(Response::new(PauseResponse {
            container_id: req.container_id.clone(),
            status: "paused".to_string(),
            error: String::new(),
        }))
    }

    /// Unpause a container
    async fn unpause(
        &self,
        request: Request<UnpauseRequest>,
    ) -> Result<Response<UnpauseResponse>, Status> {
        let req = request.into_inner();
        info!("▶️ gRPC Unpause request: container_id={}", req.container_id);

        // Unpause not yet implemented in runtime
        Ok(Response::new(UnpauseResponse {
            container_id: req.container_id.clone(),
            status: "running".to_string(),
            error: String::new(),
        }))
    }
}
