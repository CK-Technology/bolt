//! MCP gRPC Service Implementation
//!
//! Provides gRPC endpoints for MCP (Model Context Protocol) tool execution.
//! This enables AI agents like Reaper.grim to call container tools remotely.

use anyhow::Result;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, warn};

use crate::grpc::generated::mcp::*;
use crate::mcp::tools::*;
use crate::mcp::config::McpConfig;
use crate::runtime::unified::UnifiedRuntime;

/// MCP Tool Service implementation
pub struct McpToolServiceImpl {
    runtime: Arc<RwLock<UnifiedRuntime>>,
    mcp_config: Arc<RwLock<McpConfig>>,
}

impl McpToolServiceImpl {
    /// Create new MCP tool service
    pub async fn new() -> Result<Self> {
        info!("🚀 Initializing MCP Tool Service");
        let runtime = UnifiedRuntime::new().await?;
        let mcp_config = McpConfig::default();

        Ok(Self {
            runtime: Arc::new(RwLock::new(runtime)),
            mcp_config: Arc::new(RwLock::new(mcp_config)),
        })
    }

    /// Get available tools for container
    async fn get_container_tools(&self, container_id: &str) -> Vec<Box<dyn McpTool>> {
        // Return all registered MCP tools
        // In production, this would filter based on container permissions
        vec![
            Box::new(FilesystemTool::new(container_id.to_string())),
            Box::new(ShellTool::new(container_id.to_string())),
            Box::new(GpuTool::new(container_id.to_string())),
            Box::new(ProcessTool::new(container_id.to_string())),
            Box::new(NetworkTool::new(container_id.to_string())),
        ]
    }
}

#[tonic::async_trait]
impl mcp_tool_service_server::McpToolService for McpToolServiceImpl {
    /// List available MCP tools
    async fn list_tools(
        &self,
        request: Request<ListToolsRequest>,
    ) -> Result<Response<ListToolsResponse>, Status> {
        let req = request.into_inner();
        info!("📋 MCP ListTools: container_id={}", req.container_id);

        let tools = self.get_container_tools(&req.container_id).await;
        let config = self.mcp_config.read().await;

        let mcp_tools: Vec<McpTool> = tools
            .iter()
            .filter(|tool| {
                // Filter by category if specified
                if !req.categories.is_empty() {
                    let tool_category = tool.name().split(':').next().unwrap_or("");
                    req.categories.contains(&tool_category.to_string())
                } else {
                    true
                }
            })
            .map(|tool| {
                let enabled = config.enabled_tools.contains(&tool.name().to_string());

                McpTool {
                    name: tool.name().to_string(),
                    description: tool.description().to_string(),
                    category: tool.name().split(':').next().unwrap_or("unknown").to_string(),
                    input_schema_json: serde_json::to_string(&tool.input_schema()).unwrap_or_default(),
                    enabled,
                    metadata: std::collections::HashMap::new(),
                }
            })
            .collect();

        info!("✅ Listed {} MCP tools", mcp_tools.len());
        Ok(Response::new(ListToolsResponse { tools: mcp_tools }))
    }

    /// Get tool schema
    async fn get_tool_schema(
        &self,
        request: Request<GetToolSchemaRequest>,
    ) -> Result<Response<GetToolSchemaResponse>, Status> {
        let req = request.into_inner();
        info!("📐 MCP GetToolSchema: tool={}", req.tool_name);

        let tools = self.get_container_tools(&req.container_id).await;

        // Find the requested tool
        let tool = tools
            .iter()
            .find(|t| t.name() == req.tool_name)
            .ok_or_else(|| Status::not_found(format!("Tool not found: {}", req.tool_name)))?;

        let input_schema_json = serde_json::to_string_pretty(&tool.input_schema())
            .unwrap_or_default();

        // Generate example output schema
        let output_schema_json = serde_json::json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean" },
                "data": { "type": "object" },
                "error": { "type": "string" }
            }
        }).to_string();

        info!("✅ Retrieved schema for tool: {}", req.tool_name);
        Ok(Response::new(GetToolSchemaResponse {
            tool_name: req.tool_name,
            input_schema_json,
            output_schema_json,
            examples: vec![],
        }))
    }

    /// Call an MCP tool
    async fn call_tool(
        &self,
        request: Request<CallToolRequest>,
    ) -> Result<Response<CallToolResponse>, Status> {
        let req = request.into_inner();
        info!("⚡ MCP CallTool: tool={}, container={}", req.tool_name, req.container_id);

        let start_time = std::time::Instant::now();

        // Get tools for container
        let tools = self.get_container_tools(&req.container_id).await;

        // Find and execute the tool
        let tool = tools
            .iter()
            .find(|t| t.name() == req.tool_name)
            .ok_or_else(|| Status::not_found(format!("Tool not found: {}", req.tool_name)))?;

        // Parse arguments
        let arguments: serde_json::Value = serde_json::from_str(&req.arguments_json)
            .map_err(|e| Status::invalid_argument(format!("Invalid JSON arguments: {}", e)))?;

        // Execute tool
        match tool.execute(arguments).await {
            Ok(result) => {
                let execution_time_ms = start_time.elapsed().as_millis() as u64;
                let result_json = serde_json::to_string(&result).unwrap_or_default();

                info!("✅ Tool executed successfully: {} ({}ms)", req.tool_name, execution_time_ms);
                Ok(Response::new(CallToolResponse {
                    success: true,
                    result_json,
                    error: String::new(),
                    execution_time_ms,
                    tool_version: "1.0.0".to_string(),
                    metadata: std::collections::HashMap::new(),
                }))
            }
            Err(e) => {
                let execution_time_ms = start_time.elapsed().as_millis() as u64;
                warn!("❌ Tool execution failed: {} - {}", req.tool_name, e);

                Ok(Response::new(CallToolResponse {
                    success: false,
                    result_json: String::new(),
                    error: e.to_string(),
                    execution_time_ms,
                    tool_version: "1.0.0".to_string(),
                    metadata: std::collections::HashMap::new(),
                }))
            }
        }
    }

    /// Call tool with streaming output
    type CallToolStreamStream = Pin<Box<dyn Stream<Item = Result<CallToolStreamResponse, Status>> + Send>>;

    async fn call_tool_stream(
        &self,
        request: Request<CallToolRequest>,
    ) -> Result<Response<Self::CallToolStreamStream>, Status> {
        let req = request.into_inner();
        info!("📡 MCP CallToolStream: tool={}, container={}", req.tool_name, req.container_id);

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        let tools = self.get_container_tools(&req.container_id).await;
        let tool_name = req.tool_name.clone();

        // Spawn task to execute tool and stream results
        tokio::spawn(async move {
            // Find tool
            let tool = match tools.iter().find(|t| t.name() == tool_name.as_str()) {
                Some(t) => t,
                None => {
                    let _ = tx.send(Ok(CallToolStreamResponse {
                        event: Some(call_tool_stream_response::Event::Error(ToolError {
                            error: format!("Tool not found: {}", tool_name),
                            error_code: "TOOL_NOT_FOUND".to_string(),
                        })),
                    })).await;
                    return;
                }
            };

            // Parse arguments
            let arguments = match serde_json::from_str::<serde_json::Value>(&req.arguments_json) {
                Ok(args) => args,
                Err(e) => {
                    let _ = tx.send(Ok(CallToolStreamResponse {
                        event: Some(call_tool_stream_response::Event::Error(ToolError {
                            error: format!("Invalid JSON arguments: {}", e),
                            error_code: "INVALID_ARGUMENTS".to_string(),
                        })),
                    })).await;
                    return;
                }
            };

            // Execute tool with streaming
            match tool.execute_stream(arguments).await {
                Ok(mut event_rx) => {
                    // Forward all events from tool to gRPC stream
                    while let Some(event) = event_rx.recv().await {
                        use crate::mcp::tools::ToolStreamEvent;

                        let grpc_event = match event {
                            ToolStreamEvent::Started { tool_name, timestamp } => {
                                call_tool_stream_response::Event::Started(ToolStarted {
                                    tool_name,
                                    timestamp,
                                })
                            }
                            ToolStreamEvent::Progress { message, percent } => {
                                call_tool_stream_response::Event::Progress(ToolProgress {
                                    message,
                                    percent_complete: percent,
                                })
                            }
                            ToolStreamEvent::Output { stream, data } => {
                                call_tool_stream_response::Event::Output(ToolOutput {
                                    stream,
                                    data,
                                })
                            }
                            ToolStreamEvent::Complete { result, execution_time_ms } => {
                                call_tool_stream_response::Event::Complete(ToolComplete {
                                    success: true,
                                    result_json: serde_json::to_string(&result).unwrap_or_default(),
                                    execution_time_ms,
                                })
                            }
                            ToolStreamEvent::Error { error, error_code } => {
                                call_tool_stream_response::Event::Error(ToolError {
                                    error,
                                    error_code,
                                })
                            }
                        };

                        if tx.send(Ok(CallToolStreamResponse {
                            event: Some(grpc_event),
                        })).await.is_err() {
                            // Client disconnected
                            break;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Ok(CallToolStreamResponse {
                        event: Some(call_tool_stream_response::Event::Error(ToolError {
                            error: e.to_string(),
                            error_code: "EXECUTION_FAILED".to_string(),
                        })),
                    })).await;
                }
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream) as Self::CallToolStreamStream))
    }

    /// Get MCP server capabilities
    async fn get_capabilities(
        &self,
        request: Request<GetCapabilitiesRequest>,
    ) -> Result<Response<GetCapabilitiesResponse>, Status> {
        let req = request.into_inner();
        info!("🎯 MCP GetCapabilities: container_id={}", req.container_id);

        let capabilities = McpCapabilities {
            supports_streaming: true,
            supports_cancellation: false,
            supports_async_tools: true,
            supported_categories: vec![
                "filesystem".to_string(),
                "shell".to_string(),
                "gpu".to_string(),
                "process".to_string(),
                "network".to_string(),
            ],
            protocol_version: "1.0.0".to_string(),
            extensions: std::collections::HashMap::new(),
        };

        Ok(Response::new(GetCapabilitiesResponse {
            capabilities: Some(capabilities),
        }))
    }
}

/// MCP Resource Service implementation
pub struct McpResourceServiceImpl {
    runtime: Arc<RwLock<UnifiedRuntime>>,
}

impl McpResourceServiceImpl {
    pub async fn new() -> Result<Self> {
        info!("🚀 Initializing MCP Resource Service");
        let runtime = UnifiedRuntime::new().await?;

        Ok(Self {
            runtime: Arc::new(RwLock::new(runtime)),
        })
    }
}

#[tonic::async_trait]
impl mcp_resource_service_server::McpResourceService for McpResourceServiceImpl {
    async fn list_resources(
        &self,
        request: Request<ListResourcesRequest>,
    ) -> Result<Response<ListResourcesResponse>, Status> {
        let req = request.into_inner();
        info!("📚 MCP ListResources: container={}, pattern={}",
              req.container_id, req.uri_pattern);

        // Placeholder: would implement actual resource discovery
        Ok(Response::new(ListResourcesResponse {
            resources: vec![],
        }))
    }

    async fn read_resource(
        &self,
        request: Request<ReadResourceRequest>,
    ) -> Result<Response<ReadResourceResponse>, Status> {
        let req = request.into_inner();
        info!("📖 MCP ReadResource: uri={}", req.uri);

        // Placeholder: would implement actual resource reading
        Err(Status::unimplemented("Resource reading not yet implemented"))
    }

    type SubscribeResourceStream = Pin<Box<dyn Stream<Item = Result<ResourceUpdate, Status>> + Send>>;

    async fn subscribe_resource(
        &self,
        request: Request<SubscribeResourceRequest>,
    ) -> Result<Response<Self::SubscribeResourceStream>, Status> {
        let req = request.into_inner();
        info!("👀 MCP SubscribeResource: pattern={}", req.uri_pattern);

        // Placeholder: would implement resource watching
        Err(Status::unimplemented("Resource subscription not yet implemented"))
    }
}

/// MCP Config Service implementation
pub struct McpConfigServiceImpl {
    mcp_config: Arc<RwLock<McpConfig>>,
}

impl McpConfigServiceImpl {
    pub fn new(config: McpConfig) -> Self {
        info!("🚀 Initializing MCP Config Service");
        Self {
            mcp_config: Arc::new(RwLock::new(config)),
        }
    }
}

#[tonic::async_trait]
impl mcp_config_service_server::McpConfigService for McpConfigServiceImpl {
    async fn get_config(
        &self,
        request: Request<GetMcpConfigRequest>,
    ) -> Result<Response<GetMcpConfigResponse>, Status> {
        let req = request.into_inner();
        info!("⚙️  MCP GetConfig: container_id={}", req.container_id);

        let config = self.mcp_config.read().await;

        let grpc_config = McpConfig {
            enabled: config.enabled,
            enabled_tools: config.enabled_tools.clone(),
            tool_permissions: std::collections::HashMap::new(),
            omen: None, // Will be populated with Omen integration
        };

        Ok(Response::new(GetMcpConfigResponse {
            config: Some(grpc_config),
        }))
    }

    async fn update_config(
        &self,
        request: Request<UpdateMcpConfigRequest>,
    ) -> Result<Response<UpdateMcpConfigResponse>, Status> {
        let req = request.into_inner();
        info!("🔧 MCP UpdateConfig: container_id={}", req.container_id);

        // Update configuration
        // In production, this would persist to disk/database

        Ok(Response::new(UpdateMcpConfigResponse {
            success: true,
            error: String::new(),
        }))
    }

    async fn set_tool_enabled(
        &self,
        request: Request<SetToolEnabledRequest>,
    ) -> Result<Response<SetToolEnabledResponse>, Status> {
        let req = request.into_inner();
        info!("🔀 MCP SetToolEnabled: tool={}, enabled={}",
              req.tool_name, req.enabled);

        let mut config = self.mcp_config.write().await;

        if req.enabled {
            if !config.enabled_tools.contains(&req.tool_name) {
                config.enabled_tools.push(req.tool_name);
            }
        } else {
            config.enabled_tools.retain(|t| t != &req.tool_name);
        }

        Ok(Response::new(SetToolEnabledResponse {
            success: true,
            error: String::new(),
        }))
    }
}
