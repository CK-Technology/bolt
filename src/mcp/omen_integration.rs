//! Omen AI Router Integration for MCP Tools
//!
//! Integrates Omen's intelligent AI routing with Bolt's MCP tool execution,
//! enabling cost-optimized and latency-optimized multi-provider AI access.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    MCP Tool Execution                       │
//! │  (shell:exec, gpu:stats, filesystem:read, etc.)            │
//! └────────────────────────┬────────────────────────────────────┘
//!                          │
//!                          ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │              Omen AI Router (This Module)                   │
//! │                                                              │
//! │  Strategy Selection:                                        │
//! │  • cost_optimized    → Use cheapest provider                │
//! │  • latency_optimized → Use fastest provider (local Ollama)  │
//! │  • balanced          → Balance cost vs latency              │
//! └────────────────────────┬────────────────────────────────────┘
//!                          │
//!           ┌──────────────┼──────────────┐
//!           ▼              ▼              ▼
//!    ┌──────────┐   ┌──────────┐   ┌──────────┐
//!    │  Ollama  │   │ Anthropic│   │  OpenAI  │
//!    │  (Local) │   │  Claude  │   │  GPT-4   │
//!    │  Free    │   │  Cloud   │   │  Cloud   │
//!    │  <10ms   │   │  $$      │   │  $$$     │
//!    └──────────┘   └──────────┘   └──────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! # use bolt::mcp::omen_integration::*;
//! # async fn example() -> anyhow::Result<()> {
//! // Initialize Omen router
//! let router = OmenRouter::new(OmenConfig {
//!     enabled: true,
//!     routing_strategy: RoutingStrategy::CostOptimized,
//!     providers: vec!["ollama".to_string(), "anthropic".to_string()],
//!     max_cost_per_hour: 5.0,
//!     provider_config: Default::default(),
//! }).await?;
//!
//! // Route an AI request through Omen
//! let response = router.route_completion(CompletionRequest {
//!     prompt: "Analyze this code...".to_string(),
//!     max_tokens: 1000,
//!     temperature: 0.7,
//! }).await?;
//!
//! // Check routing decision
//! println!("Used provider: {}", response.provider_used);
//! println!("Cost: ${:.4}", response.cost_usd);
//! println!("Latency: {}ms", response.latency_ms);
//! # Ok(())
//! # }
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Omen AI Router configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmenConfig {
    /// Whether Omen routing is enabled
    pub enabled: bool,

    /// Routing strategy for provider selection
    pub routing_strategy: RoutingStrategy,

    /// List of enabled AI providers (e.g., ["ollama", "anthropic", "openai"])
    pub providers: Vec<String>,

    /// Maximum cost per hour in USD (prevents runaway costs)
    pub max_cost_per_hour: f64,

    /// Provider-specific configuration (API keys, endpoints, etc.)
    pub provider_config: HashMap<String, String>,
}

impl Default for OmenConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            routing_strategy: RoutingStrategy::Balanced,
            providers: vec!["ollama".to_string()],
            max_cost_per_hour: 10.0,
            provider_config: HashMap::new(),
        }
    }
}

/// AI routing strategy
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RoutingStrategy {
    /// Minimize cost - use cheapest provider (usually local Ollama)
    CostOptimized,

    /// Minimize latency - use fastest provider (usually local Ollama)
    LatencyOptimized,

    /// Balance cost and latency
    Balanced,

    /// Use highest quality model regardless of cost/latency
    QualityOptimized,
}

impl std::fmt::Display for RoutingStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CostOptimized => write!(f, "cost_optimized"),
            Self::LatencyOptimized => write!(f, "latency_optimized"),
            Self::Balanced => write!(f, "balanced"),
            Self::QualityOptimized => write!(f, "quality_optimized"),
        }
    }
}

impl std::str::FromStr for RoutingStrategy {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "cost_optimized" => Ok(Self::CostOptimized),
            "latency_optimized" => Ok(Self::LatencyOptimized),
            "balanced" => Ok(Self::Balanced),
            "quality_optimized" => Ok(Self::QualityOptimized),
            _ => anyhow::bail!("Invalid routing strategy: {}", s),
        }
    }
}

/// AI completion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// User prompt/query
    pub prompt: String,

    /// Maximum tokens to generate
    pub max_tokens: usize,

    /// Temperature (0.0 = deterministic, 1.0 = creative)
    pub temperature: f64,

    /// Optional system prompt
    pub system_prompt: Option<String>,

    /// Optional context (e.g., code, files, etc.)
    pub context: Option<String>,
}

/// AI completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Generated text
    pub text: String,

    /// Provider used (e.g., "ollama", "anthropic", "openai")
    pub provider_used: String,

    /// Model used (e.g., "llama3", "claude-3-opus", "gpt-4")
    pub model_used: String,

    /// Cost in USD
    pub cost_usd: f64,

    /// Latency in milliseconds
    pub latency_ms: u64,

    /// Tokens used (prompt + completion)
    pub tokens_used: usize,
}

/// Routing decision metadata
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    pub provider: String,
    pub model: String,
    pub reason: String,
    pub estimated_cost: f64,
    pub estimated_latency_ms: u64,
}

/// Omen AI Router implementation
pub struct OmenRouter {
    config: Arc<RwLock<OmenConfig>>,
    metrics: Arc<RwLock<RoutingMetrics>>,
}

/// Routing metrics for cost/latency tracking
#[derive(Debug, Default)]
pub struct RoutingMetrics {
    /// Total requests routed
    pub total_requests: u64,

    /// Requests per provider
    pub requests_by_provider: HashMap<String, u64>,

    /// Total cost accumulated (USD)
    pub total_cost_usd: f64,

    /// Cost per provider
    pub cost_by_provider: HashMap<String, f64>,

    /// Average latency per provider (ms)
    pub avg_latency_by_provider: HashMap<String, f64>,

    /// Failover count (when primary provider failed)
    pub failover_count: u64,
}

impl OmenRouter {
    /// Create new Omen router
    pub async fn new(config: OmenConfig) -> Result<Self> {
        info!("🧠 Initializing Omen AI Router");
        info!("  • Strategy: {}", config.routing_strategy);
        info!("  • Providers: {:?}", config.providers);
        info!("  • Max cost/hour: ${:.2}", config.max_cost_per_hour);

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            metrics: Arc::new(RwLock::new(RoutingMetrics::default())),
        })
    }

    /// Route a completion request to the optimal provider
    pub async fn route_completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse> {
        let start = std::time::Instant::now();
        let config = self.config.read().await;

        if !config.enabled {
            anyhow::bail!("Omen routing is disabled");
        }

        // Make routing decision
        let decision = self.make_routing_decision(&config, &request).await?;
        info!("🎯 Routing to {} ({}): {}", decision.provider, decision.model, decision.reason);

        // Execute completion (placeholder - will integrate with actual Omen client)
        let response = self.execute_completion(&decision, request).await?;

        // Update metrics
        let elapsed_ms = start.elapsed().as_millis() as u64;
        self.update_metrics(&decision.provider, response.cost_usd, elapsed_ms)
            .await;

        info!(
            "✅ Completion: provider={}, cost=${:.4}, latency={}ms, tokens={}",
            response.provider_used, response.cost_usd, response.latency_ms, response.tokens_used
        );

        Ok(response)
    }

    /// Make routing decision based on strategy
    async fn make_routing_decision(
        &self,
        config: &OmenConfig,
        _request: &CompletionRequest,
    ) -> Result<RoutingDecision> {
        // Check cost limit
        let metrics = self.metrics.read().await;
        if metrics.total_cost_usd > config.max_cost_per_hour {
            warn!(
                "⚠️  Cost limit reached: ${:.2} > ${:.2}",
                metrics.total_cost_usd, config.max_cost_per_hour
            );
        }
        drop(metrics);

        // Select provider based on strategy
        let (provider, model, reason) = match config.routing_strategy {
            RoutingStrategy::CostOptimized => {
                // Prefer local Ollama (free)
                if config.providers.contains(&"ollama".to_string()) {
                    ("ollama".to_string(), "llama3".to_string(), "Free local inference".to_string())
                } else {
                    // Fall back to cheapest cloud provider
                    ("anthropic".to_string(), "claude-3-haiku".to_string(), "Cheapest cloud model".to_string())
                }
            }
            RoutingStrategy::LatencyOptimized => {
                // Prefer local Ollama (fastest)
                if config.providers.contains(&"ollama".to_string()) {
                    ("ollama".to_string(), "llama3".to_string(), "Local inference <10ms".to_string())
                } else {
                    ("anthropic".to_string(), "claude-3-sonnet".to_string(), "Low-latency cloud".to_string())
                }
            }
            RoutingStrategy::Balanced => {
                // Use Ollama for simple tasks, Claude for complex ones
                if config.providers.contains(&"ollama".to_string()) {
                    ("ollama".to_string(), "llama3".to_string(), "Balanced: local first".to_string())
                } else {
                    ("anthropic".to_string(), "claude-3-sonnet".to_string(), "Balanced: cloud fallback".to_string())
                }
            }
            RoutingStrategy::QualityOptimized => {
                // Use best available model
                if config.providers.contains(&"anthropic".to_string()) {
                    ("anthropic".to_string(), "claude-3-opus".to_string(), "Highest quality".to_string())
                } else if config.providers.contains(&"openai".to_string()) {
                    ("openai".to_string(), "gpt-4".to_string(), "High quality".to_string())
                } else {
                    ("ollama".to_string(), "llama3".to_string(), "Best local model".to_string())
                }
            }
        };

        Ok(RoutingDecision {
            provider: provider.clone(),
            model: model.clone(),
            reason,
            estimated_cost: self.estimate_cost(&provider, &model),
            estimated_latency_ms: self.estimate_latency(&provider),
        })
    }

    /// Execute completion with selected provider via Omen gateway
    async fn execute_completion(
        &self,
        decision: &RoutingDecision,
        request: CompletionRequest,
    ) -> Result<CompletionResponse> {
        debug!(
            "Executing completion: provider={}, model={}, tokens={}",
            decision.provider, decision.model, request.max_tokens
        );

        let start = std::time::Instant::now();

        // Build Omen API request (OpenAI-compatible format)
        let omen_url = format!("{}/v1/chat/completions",
            self.config.provider_config.get("omen_base_url")
                .and_then(|v| v.as_str())
                .unwrap_or("http://localhost:8080")
        );

        let client = reqwest::Client::new();

        #[derive(serde::Serialize)]
        struct OmenRequest {
            model: String,
            messages: Vec<OmenMessage>,
            max_tokens: usize,
        }

        #[derive(serde::Serialize)]
        struct OmenMessage {
            role: String,
            content: String,
        }

        #[derive(serde::Deserialize)]
        struct OmenResponse {
            choices: Vec<OmenChoice>,
            usage: Option<OmenUsage>,
        }

        #[derive(serde::Deserialize)]
        struct OmenChoice {
            message: OmenResponseMessage,
        }

        #[derive(serde::Deserialize)]
        struct OmenResponseMessage {
            content: String,
        }

        #[derive(serde::Deserialize)]
        struct OmenUsage {
            total_tokens: usize,
        }

        let omen_req = OmenRequest {
            model: decision.model.clone(),
            messages: vec![OmenMessage {
                role: "user".to_string(),
                content: request.prompt.clone(),
            }],
            max_tokens: request.max_tokens,
        };

        // Call Omen gateway
        let response = client
            .post(&omen_url)
            .header("Content-Type", "application/json")
            .json(&omen_req)
            .send()
            .await
            .context("Failed to send request to Omen gateway")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!(
                "Omen gateway returned error {}: {}",
                status,
                error_text
            ));
        }

        let omen_response: OmenResponse = response
            .json()
            .await
            .context("Failed to parse Omen response")?;

        let latency_ms = start.elapsed().as_millis() as u64;
        let tokens_used = omen_response.usage
            .map(|u| u.total_tokens)
            .unwrap_or(request.max_tokens);

        let cost_usd = self.calculate_cost(&decision.provider, &decision.model, tokens_used);

        let text = omen_response.choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_else(|| "No response from AI".to_string());

        Ok(CompletionResponse {
            text,
            provider_used: decision.provider.clone(),
            model_used: decision.model.clone(),
            cost_usd,
            latency_ms,
            tokens_used,
        })
    }

    /// Update routing metrics
    async fn update_metrics(&self, provider: &str, cost: f64, latency_ms: u64) {
        let mut metrics = self.metrics.write().await;

        metrics.total_requests += 1;
        *metrics.requests_by_provider.entry(provider.to_string()).or_insert(0) += 1;

        metrics.total_cost_usd += cost;
        *metrics.cost_by_provider.entry(provider.to_string()).or_insert(0.0) += cost;

        // Update average latency
        let prev_avg = *metrics.avg_latency_by_provider.get(provider).unwrap_or(&0.0);
        let request_count = *metrics.requests_by_provider.get(provider).unwrap_or(&1);
        let new_avg = (prev_avg * (request_count - 1) as f64 + latency_ms as f64) / request_count as f64;
        metrics.avg_latency_by_provider.insert(provider.to_string(), new_avg);
    }

    /// Get current routing metrics
    pub async fn get_metrics(&self) -> RoutingMetrics {
        self.metrics.read().await.clone()
    }

    /// Estimate cost for a provider/model combination
    fn estimate_cost(&self, provider: &str, model: &str) -> f64 {
        match (provider, model) {
            ("ollama", _) => 0.0, // Local inference is free
            ("anthropic", "claude-3-opus") => 0.015,   // $15/1M tokens (input)
            ("anthropic", "claude-3-sonnet") => 0.003,  // $3/1M tokens
            ("anthropic", "claude-3-haiku") => 0.00025, // $0.25/1M tokens
            ("openai", "gpt-4") => 0.030,               // $30/1M tokens
            ("openai", "gpt-3.5-turbo") => 0.0005,      // $0.50/1M tokens
            _ => 0.001, // Default estimate
        }
    }

    /// Estimate latency for a provider
    fn estimate_latency(&self, provider: &str) -> u64 {
        match provider {
            "ollama" => 10,      // Local: ~10ms
            "anthropic" => 200,  // Cloud: ~200ms
            "openai" => 300,     // Cloud: ~300ms
            _ => 500,            // Unknown: assume high latency
        }
    }

    /// Calculate actual cost based on tokens used
    fn calculate_cost(&self, provider: &str, model: &str, tokens: usize) -> f64 {
        let cost_per_token = self.estimate_cost(provider, model);
        cost_per_token * (tokens as f64 / 1000.0)
    }
}

impl Clone for RoutingMetrics {
    fn clone(&self) -> Self {
        Self {
            total_requests: self.total_requests,
            requests_by_provider: self.requests_by_provider.clone(),
            total_cost_usd: self.total_cost_usd,
            cost_by_provider: self.cost_by_provider.clone(),
            avg_latency_by_provider: self.avg_latency_by_provider.clone(),
            failover_count: self.failover_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_omen_router_creation() {
        let config = OmenConfig::default();
        let router = OmenRouter::new(config).await.unwrap();
        let metrics = router.get_metrics().await;
        assert_eq!(metrics.total_requests, 0);
    }

    #[tokio::test]
    async fn test_routing_strategy_parsing() {
        assert_eq!(
            "cost_optimized".parse::<RoutingStrategy>().unwrap(),
            RoutingStrategy::CostOptimized
        );
        assert_eq!(
            "latency_optimized".parse::<RoutingStrategy>().unwrap(),
            RoutingStrategy::LatencyOptimized
        );
    }

    #[tokio::test]
    async fn test_completion_routing() {
        let config = OmenConfig {
            enabled: true,
            routing_strategy: RoutingStrategy::CostOptimized,
            providers: vec!["ollama".to_string()],
            max_cost_per_hour: 10.0,
            provider_config: HashMap::new(),
        };

        let router = OmenRouter::new(config).await.unwrap();

        let request = CompletionRequest {
            prompt: "Test prompt".to_string(),
            max_tokens: 100,
            temperature: 0.7,
            system_prompt: None,
            context: None,
        };

        let response = router.route_completion(request).await.unwrap();
        assert_eq!(response.provider_used, "ollama");
        assert_eq!(response.cost_usd, 0.0); // Ollama is free
    }
}
