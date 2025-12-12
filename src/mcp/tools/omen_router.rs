use crate::mcp::{McpError, McpTool};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Omen AI Router MCP Tool
///
/// Provides smart AI routing capabilities through MCP, enabling:
/// - Intent-based model selection (code, tests, analysis, etc.)
/// - Cost/latency/quality-aware routing
/// - Multi-provider support (OpenAI, Anthropic, Google, Azure, XAI, Ollama)
/// - Advanced routing strategies (single, race, speculative, parallel_merge)
pub struct OmenRouterTool {
    #[cfg(feature = "omen")]
    router: Arc<RwLock<Option<omen::router::OmenRouter>>>,
    #[cfg(not(feature = "omen"))]
    _phantom: std::marker::PhantomData<()>,
}

impl OmenRouterTool {
    pub fn new() -> Self {
        #[cfg(feature = "omen")]
        {
            Self {
                router: Arc::new(RwLock::new(None)),
            }
        }
        #[cfg(not(feature = "omen"))]
        {
            Self {
                _phantom: std::marker::PhantomData,
            }
        }
    }

    #[cfg(feature = "omen")]
    pub async fn initialize(&self, config: omen::config::Config) -> Result<(), McpError> {
        let router = omen::router::OmenRouter::new(config).await.map_err(|e| {
            McpError::InternalError(format!("Failed to initialize Omen router: {}", e))
        })?;

        let mut guard = self.router.write().await;
        *guard = Some(router);

        info!("✅ Omen AI router initialized");
        Ok(())
    }
}

impl Default for OmenRouterTool {
    fn default() -> Self {
        Self::new()
    }
}

impl McpTool for OmenRouterTool {
    fn name(&self) -> &str {
        "bolt_omen_chat"
    }

    fn description(&self) -> &str {
        "Smart AI chat completion with cost/latency/quality-aware routing across multiple providers (OpenAI, Anthropic, Google, Azure, XAI, Ollama)"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "messages": {
                    "type": "array",
                    "description": "Array of chat messages with role and content",
                    "items": {
                        "type": "object",
                        "properties": {
                            "role": {
                                "type": "string",
                                "enum": ["system", "user", "assistant"],
                                "description": "Message role"
                            },
                            "content": {
                                "type": "string",
                                "description": "Message content"
                            }
                        },
                        "required": ["role", "content"]
                    }
                },
                "model": {
                    "type": "string",
                    "description": "Model to use ('auto' for smart routing, or specific model like 'gpt-4', 'claude-3-opus')",
                    "default": "auto"
                },
                "temperature": {
                    "type": "number",
                    "description": "Temperature (0.0-2.0)",
                    "default": 0.7,
                    "minimum": 0.0,
                    "maximum": 2.0
                },
                "max_tokens": {
                    "type": "integer",
                    "description": "Maximum tokens to generate",
                    "default": 500,
                    "minimum": 1
                },
                "stream": {
                    "type": "boolean",
                    "description": "Enable streaming",
                    "default": false
                },
                "intent": {
                    "type": "string",
                    "enum": ["code", "tests", "analysis", "explanation", "regex", "general"],
                    "description": "Intent hint for smart routing",
                    "default": "general"
                },
                "strategy": {
                    "type": "string",
                    "enum": ["single", "race", "speculate_k", "parallel_merge"],
                    "description": "Routing strategy for multi-provider requests",
                    "default": "single"
                },
                "providers": {
                    "type": "array",
                    "description": "Allowlist of providers to consider (optional)",
                    "items": {
                        "type": "string",
                        "enum": ["openai", "anthropic", "google", "azure", "xai", "ollama", "bedrock"]
                    }
                },
                "budget_usd": {
                    "type": "number",
                    "description": "Budget cap per request in USD",
                    "default": 0.10,
                    "minimum": 0.0
                },
                "max_latency_ms": {
                    "type": "integer",
                    "description": "Maximum latency in milliseconds",
                    "default": 5000,
                    "minimum": 100
                }
            },
            "required": ["messages"]
        })
    }

    fn execute(&self, input: Value) -> Result<Value, McpError> {
        #[cfg(feature = "omen")]
        {
            // Execute async operation in blocking context
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async { self.execute_async(input).await })
        }
        #[cfg(not(feature = "omen"))]
        {
            let _ = input;
            Err(McpError::InternalError(
                "OMEN integration not enabled. Rebuild with --features omen".to_string(),
            ))
        }
    }
}

#[cfg(feature = "omen")]
impl OmenRouterTool {
    async fn execute_async(&self, input: Value) -> Result<Value, McpError> {
        // Parse input
        let messages: Vec<omen::types::ChatMessage> = serde_json::from_value(
            input
                .get("messages")
                .ok_or_else(|| McpError::InvalidInput("Missing 'messages' field".to_string()))?
                .clone(),
        )
        .map_err(|e| McpError::InvalidInput(format!("Invalid messages format: {}", e)))?;

        let model = input
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("auto")
            .to_string();

        let temperature = input
            .get("temperature")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32);

        let max_tokens = input
            .get("max_tokens")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        let stream = input
            .get("stream")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let intent = input
            .get("intent")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Build OMEN config from input
        let mut omen_config = omen::types::OmenConfig::default();

        if let Some(strategy) = input.get("strategy").and_then(|v| v.as_str()) {
            omen_config.strategy = Some(strategy.to_string());
        }

        if let Some(providers_arr) = input.get("providers").and_then(|v| v.as_array()) {
            omen_config.providers = Some(
                providers_arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect(),
            );
        }

        if let Some(budget) = input.get("budget_usd").and_then(|v| v.as_f64()) {
            omen_config.budget_usd = Some(budget);
        }

        if let Some(max_latency) = input.get("max_latency_ms").and_then(|v| v.as_u64()) {
            omen_config.max_latency_ms = Some(max_latency as u32);
        }

        // Build chat completion request
        let request = omen::types::ChatCompletionRequest {
            model,
            messages,
            temperature,
            max_tokens,
            stream,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            tools: None,
            tool_choice: None,
            tags: None,
            omen: Some(omen_config),
        };

        // Build request context
        let mut tags = std::collections::HashMap::new();
        if let Some(intent_str) = intent {
            tags.insert("intent".to_string(), intent_str.clone());
        }

        let context = omen::types::RequestContext {
            request_id: uuid::Uuid::new_v4(),
            user_id: None,
            api_key: None,
            intent,
            tags,
        };

        // Get router instance
        let router_guard = self.router.read().await;
        let router = router_guard
            .as_ref()
            .ok_or_else(|| McpError::InternalError("Omen router not initialized".to_string()))?;

        // Execute request
        if stream {
            // For streaming, we'll return an error for now (streaming requires different handling)
            return Err(McpError::InternalError(
                "Streaming not yet supported via MCP. Use stream: false".to_string(),
            ));
        }

        info!(
            "🎯 Routing AI request to Omen (model: {}, intent: {:?})",
            request.model, context.intent
        );

        let response = router
            .chat_completion(request, context)
            .await
            .map_err(|e| {
                error!("Omen request failed: {}", e);
                McpError::InternalError(format!("AI request failed: {}", e))
            })?;

        // Convert response to MCP-friendly format
        let result = json!({
            "id": response.id,
            "model": response.model,
            "created": response.created,
            "choices": response.choices.iter().map(|choice| {
                json!({
                    "index": choice.index,
                    "message": {
                        "role": choice.message.role,
                        "content": choice.message.content.text()
                    },
                    "finish_reason": choice.finish_reason
                })
            }).collect::<Vec<_>>(),
            "usage": {
                "prompt_tokens": response.usage.prompt_tokens,
                "completion_tokens": response.usage.completion_tokens,
                "total_tokens": response.usage.total_tokens
            }
        });

        info!("✅ Omen request completed successfully");
        Ok(result)
    }
}
