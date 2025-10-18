//! Interceptor Middleware
//!
//! Pipeline for logging, filtering, and rate limiting MCP requests

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::{info, warn};

/// Interceptor trait
#[async_trait]
pub trait Interceptor: Send + Sync {
    /// Intercept a request before execution
    async fn before_request(&self, tool: &str, input: &Value) -> Result<(), String>;

    /// Intercept a response after execution
    async fn after_response(&self, tool: &str, result: &Value) -> Result<(), String>;
}

/// Logging interceptor
pub struct LoggingInterceptor;

#[async_trait]
impl Interceptor for LoggingInterceptor {
    async fn before_request(&self, tool: &str, input: &Value) -> Result<(), String> {
        info!("MCP Tool Call: {} with input: {}", tool, input);
        Ok(())
    }

    async fn after_response(&self, tool: &str, result: &Value) -> Result<(), String> {
        info!("MCP Tool Response: {} returned: {}", tool, result);
        Ok(())
    }
}

/// Rate limiting interceptor
pub struct RateLimitInterceptor {
    max_requests_per_minute: u32,
}

impl RateLimitInterceptor {
    pub fn new(max_requests_per_minute: u32) -> Self {
        Self {
            max_requests_per_minute,
        }
    }
}

#[async_trait]
impl Interceptor for RateLimitInterceptor {
    async fn before_request(&self, tool: &str, _input: &Value) -> Result<(), String> {
        // TODO: Implement actual rate limiting with token bucket or similar
        // For now, just a placeholder
        info!("Rate limit check for tool: {}", tool);
        Ok(())
    }

    async fn after_response(&self, _tool: &str, _result: &Value) -> Result<(), String> {
        Ok(())
    }
}

/// Secret redaction interceptor
pub struct SecretRedactionInterceptor;

#[async_trait]
impl Interceptor for SecretRedactionInterceptor {
    async fn before_request(&self, _tool: &str, input: &Value) -> Result<(), String> {
        // Redact secrets from logs
        if let Some(obj) = input.as_object() {
            for (key, value) in obj {
                let key_lower = key.to_lowercase();
                if key_lower.contains("secret")
                    || key_lower.contains("password")
                    || key_lower.contains("token")
                    || key_lower.contains("key")
                {
                    warn!("Redacting secret field: {}", key);
                }
            }
        }
        Ok(())
    }

    async fn after_response(&self, _tool: &str, _result: &Value) -> Result<(), String> {
        Ok(())
    }
}

/// Interceptor chain
pub struct InterceptorChain {
    interceptors: Vec<Arc<dyn Interceptor>>,
}

impl InterceptorChain {
    /// Create a new interceptor chain
    pub fn new() -> Self {
        Self {
            interceptors: vec![
                Arc::new(LoggingInterceptor),
                Arc::new(SecretRedactionInterceptor),
            ],
        }
    }

    /// Add an interceptor to the chain
    pub fn add_interceptor(&mut self, interceptor: Arc<dyn Interceptor>) {
        self.interceptors.push(interceptor);
    }

    /// Execute before_request for all interceptors
    pub async fn before_request(&self, tool: &str, input: &Value) -> Result<(), String> {
        for interceptor in &self.interceptors {
            interceptor.before_request(tool, input).await?;
        }
        Ok(())
    }

    /// Execute after_response for all interceptors
    pub async fn after_response(&self, tool: &str, result: &Value) -> Result<(), String> {
        for interceptor in &self.interceptors {
            interceptor.after_response(tool, result).await?;
        }
        Ok(())
    }
}

impl Default for InterceptorChain {
    fn default() -> Self {
        Self::new()
    }
}
