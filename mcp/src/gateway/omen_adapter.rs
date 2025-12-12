use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// OMEN Provider Adapter
///
/// Presents OMEN AI providers as virtual MCP servers in the gateway catalog.
/// Each AI provider (OpenAI, Anthropic, Google, Azure, XAI, Ollama) appears
/// as a separate MCP server with its own tools.
#[cfg(feature = "omen")]
pub struct OmenProviderAdapter {
    router: Arc<RwLock<Option<omen::router::OmenRouter>>>,
    providers: Vec<String>,
}

#[cfg(feature = "omen")]
impl OmenProviderAdapter {
    /// Create a new OMEN provider adapter
    pub fn new() -> Self {
        Self {
            router: Arc::new(RwLock::new(None)),
            providers: Vec::new(),
        }
    }

    /// Initialize the OMEN router with configuration
    pub async fn initialize(&mut self, config: omen::config::Config) -> Result<()> {
        let router = omen::router::OmenRouter::new(config)
            .await
            .map_err(|e| anyhow!("Failed to initialize OMEN router: {}", e))?;

        // Get list of available providers
        self.providers = router
            .list_providers()
            .await
            .iter()
            .map(|p| p.id().to_string())
            .collect();

        let mut guard = self.router.write().await;
        *guard = Some(router);

        info!(
            "✅ OMEN provider adapter initialized with {} providers",
            self.providers.len()
        );
        Ok(())
    }

    /// Get list of available providers
    pub fn list_providers(&self) -> &[String] {
        &self.providers
    }

    /// Check if a specific provider is available
    pub fn has_provider(&self, provider_id: &str) -> bool {
        self.providers.contains(&provider_id.to_string())
    }

    /// Generate virtual MCP server definitions for catalog
    pub fn generate_server_definitions(&self) -> Vec<VirtualServerDefinition> {
        self.providers
            .iter()
            .map(|provider_id| VirtualServerDefinition {
                name: format!("omen-{}", provider_id),
                server_type: "omen-provider".to_string(),
                description: format!("OMEN AI provider: {}", provider_id),
                provider_id: provider_id.clone(),
                tools: vec![
                    ToolDefinition {
                        name: format!("{}_chat", provider_id),
                        description: format!("Chat completion via {}", provider_id),
                        enabled: true,
                    },
                    ToolDefinition {
                        name: format!("{}_embeddings", provider_id),
                        description: format!("Generate embeddings via {}", provider_id),
                        enabled: true,
                    },
                ],
                enabled: true,
            })
            .collect()
    }

    /// Execute a chat completion request
    pub async fn chat_completion(
        &self,
        provider_id: &str,
        messages: Vec<OmenChatMessage>,
        options: ChatOptions,
    ) -> Result<ChatResponse> {
        let router_guard = self.router.read().await;
        let router = router_guard
            .as_ref()
            .ok_or_else(|| anyhow!("OMEN router not initialized"))?;

        // Convert messages to OMEN format
        let omen_messages: Vec<omen::types::ChatMessage> = messages
            .into_iter()
            .map(|msg| omen::types::ChatMessage {
                role: msg.role,
                content: omen::types::MessageContent::Text(msg.content),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            })
            .collect();

        // Build request
        let request = omen::types::ChatCompletionRequest {
            model: options.model.unwrap_or_else(|| "auto".to_string()),
            messages: omen_messages,
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            stream: false,
            top_p: options.top_p,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            tools: None,
            tool_choice: None,
            tags: None,
            omen: Some(omen::types::OmenConfig {
                strategy: Some("single".to_string()),
                k: None,
                providers: Some(vec![provider_id.to_string()]),
                budget_usd: options.budget_usd,
                max_latency_ms: options.max_latency_ms,
                stickiness: None,
                priority_weights: None,
                min_useful_tokens: None,
            }),
        };

        // Build context
        let context = omen::types::RequestContext {
            request_id: uuid::Uuid::new_v4(),
            user_id: options.user_id.clone(),
            api_key: None,
            intent: options.intent.clone(),
            tags: std::collections::HashMap::new(),
        };

        info!("🎯 Routing to OMEN provider: {}", provider_id);

        // Execute request
        let response = router
            .chat_completion(request, context)
            .await
            .map_err(|e| {
                error!("OMEN request failed: {}", e);
                anyhow!("Chat completion failed: {}", e)
            })?;

        // Convert response
        Ok(ChatResponse {
            id: response.id,
            model: response.model,
            created: response.created,
            choices: response
                .choices
                .into_iter()
                .map(|choice| ChatChoice {
                    index: choice.index,
                    message: OmenChatMessage {
                        role: choice.message.role,
                        content: choice.message.content.text(),
                    },
                    finish_reason: choice.finish_reason,
                })
                .collect(),
            usage: ChatUsage {
                prompt_tokens: response.usage.prompt_tokens,
                completion_tokens: response.usage.completion_tokens,
                total_tokens: response.usage.total_tokens,
            },
        })
    }

    /// Get provider health status
    pub async fn get_provider_health(&self, provider_id: &str) -> Result<ProviderHealth> {
        let router_guard = self.router.read().await;
        let router = router_guard
            .as_ref()
            .ok_or_else(|| anyhow!("OMEN router not initialized"))?;

        let healthy = router
            .check_provider_health(provider_id)
            .await
            .unwrap_or(false);

        Ok(ProviderHealth {
            provider_id: provider_id.to_string(),
            healthy,
            latency_ms: None,
            last_check: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Get all provider health statuses
    pub async fn get_all_provider_health(&self) -> Result<Vec<ProviderHealth>> {
        let router_guard = self.router.read().await;
        let router = router_guard
            .as_ref()
            .ok_or_else(|| anyhow!("OMEN router not initialized"))?;

        let health_status = router
            .get_provider_health()
            .await
            .map_err(|e| anyhow!("Failed to get provider health: {}", e))?;

        Ok(health_status
            .into_iter()
            .map(|status| ProviderHealth {
                provider_id: status.id,
                healthy: status.healthy,
                latency_ms: status.latency_ms,
                last_check: chrono::Utc::now().to_rfc3339(),
            })
            .collect())
    }
}

#[cfg(feature = "omen")]
impl Default for OmenProviderAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Virtual server definition for catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualServerDefinition {
    pub name: String,
    pub server_type: String,
    pub description: String,
    pub provider_id: String,
    pub tools: Vec<ToolDefinition>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub enabled: bool,
}

/// Chat message (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmenChatMessage {
    pub role: String,
    pub content: String,
}

/// Chat options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatOptions {
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub intent: Option<String>,
    pub user_id: Option<String>,
    pub budget_usd: Option<f64>,
    pub max_latency_ms: Option<u32>,
}

/// Chat response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub created: i64,
    pub choices: Vec<ChatChoice>,
    pub usage: ChatUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: u32,
    pub message: OmenChatMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Provider health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub provider_id: String,
    pub healthy: bool,
    pub latency_ms: Option<u64>,
    pub last_check: String,
}

#[cfg(not(feature = "omen"))]
pub struct OmenProviderAdapter;

#[cfg(not(feature = "omen"))]
impl OmenProviderAdapter {
    pub fn new() -> Self {
        Self
    }

    pub fn list_providers(&self) -> &[String] {
        &[]
    }
}

#[cfg(not(feature = "omen"))]
impl Default for OmenProviderAdapter {
    fn default() -> Self {
        Self::new()
    }
}
