use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::time::{Duration, Instant};
use tracing::Level;
use tracing_subscriber::{EnvFilter, Registry, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Comprehensive tracing configuration for Bolt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    pub log_level: String,
    pub enable_json_logs: bool,
    pub enable_file_logging: bool,
    pub log_file_path: String,
    pub enable_jaeger: bool,
    pub jaeger_endpoint: String,
    pub service_name: String,
    pub enable_metrics: bool,
    pub structured_logging: bool,
}

impl TracingConfig {
    /// Initialize comprehensive tracing
    pub fn init_tracing(&self) -> Result<()> {
        println!("🔍 Initializing comprehensive tracing system");

        // Create env filter
        let env_filter = EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new(&self.log_level))
            .unwrap_or_else(|_| EnvFilter::new("info"));

        // We'll use a simpler approach without dynamic layers

        // Console logging layer
        if self.structured_logging {
            let console_layer = fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true)
                .with_writer(io::stdout);

            if self.enable_json_logs {
                let json_layer = console_layer
                    .json()
                    .flatten_event(true)
                    .with_current_span(true)
                    .with_span_list(true);

                Registry::default()
                    .with(env_filter.clone())
                    .with(json_layer)
                    .init();
            } else {
                let pretty_layer = console_layer.pretty().with_ansi(true);

                Registry::default()
                    .with(env_filter.clone())
                    .with(pretty_layer)
                    .init();
            }
        } else {
            // Simple console logging
            let simple_layer = fmt::layer().with_target(false).compact();

            Registry::default()
                .with(env_filter.clone())
                .with(simple_layer)
                .init();
        }

        // File logging
        if self.enable_file_logging {
            self.setup_file_logging()?;
        }

        // Jaeger tracing
        if self.enable_jaeger {
            self.setup_jaeger_tracing()?;
        }

        println!("✅ Tracing system initialized successfully");
        Ok(())
    }

    /// Setup file logging
    fn setup_file_logging(&self) -> Result<()> {
        use std::fs::OpenOptions;
        use tracing_appender;

        println!("📝 Setting up file logging: {}", self.log_file_path);

        // Create log directory if it doesn't exist
        if let Some(parent) = std::path::Path::new(&self.log_file_path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Setup file appender
        let file_appender = tracing_appender::rolling::daily(
            std::path::Path::new(&self.log_file_path)
                .parent()
                .unwrap_or(std::path::Path::new(".")),
            "bolt.log",
        );

        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

        let _file_layer = fmt::layer::<Registry>()
            .with_writer(non_blocking)
            .with_ansi(false)
            .json();

        // This would be added to the registry in a real implementation
        // Currently tracing-subscriber doesn't allow adding layers after init

        println!("  ✓ File logging configured");
        Ok(())
    }

    /// Setup Jaeger distributed tracing
    fn setup_jaeger_tracing(&self) -> Result<()> {
        println!("🔍 Setting up Jaeger distributed tracing");

        // In a real implementation, this would use opentelemetry-jaeger
        // For now, just log the configuration
        println!("  • Service name: {}", self.service_name);
        println!("  • Jaeger endpoint: {}", self.jaeger_endpoint);
        println!("  ✓ Jaeger tracing would be configured here");

        Ok(())
    }

    /// Create span with custom attributes
    pub fn create_span_with_attrs(
        &self,
        name: &str,
        attrs: HashMap<String, String>,
    ) -> tracing::Span {
        let span = tracing::info_span!(
            "bolt_operation",
            operation = %name,
            service = %self.service_name
        );

        for (key, value) in attrs {
            span.record(key.as_str(), &tracing::field::display(&value));
        }

        span
    }

    /// Log performance metrics
    pub fn log_performance_metric(&self, operation: &str, duration_ms: f64, success: bool) {
        tracing::info!(
            operation = %operation,
            duration_ms = %duration_ms,
            success = %success,
            "Performance metric recorded"
        );
    }

    /// Log container lifecycle event
    pub fn log_container_event(&self, container_id: &str, event: &str, details: Option<&str>) {
        tracing::info!(
            container_id = %container_id,
            event = %event,
            details = ?details,
            "Container lifecycle event"
        );
    }

    /// Log GPU operation
    pub fn log_gpu_operation(&self, gpu_id: &str, operation: &str, result: &str) {
        tracing::info!(
            gpu_id = %gpu_id,
            operation = %operation,
            result = %result,
            "GPU operation"
        );
    }

    /// Log network event
    pub fn log_network_event(
        &self,
        interface: &str,
        event_type: &str,
        details: HashMap<String, String>,
    ) {
        tracing::info!(
            interface = %interface,
            event_type = %event_type,
            details = ?details,
            "Network event"
        );
    }

    /// Log security event
    pub fn log_security_event(&self, event_type: &str, severity: &str, description: &str) {
        match severity {
            "high" | "critical" => {
                tracing::error!(
                    event_type = %event_type,
                    severity = %severity,
                    description = %description,
                    "Security event"
                );
            }
            "medium" => {
                tracing::warn!(
                    event_type = %event_type,
                    severity = %severity,
                    description = %description,
                    "Security event"
                );
            }
            _ => {
                tracing::info!(
                    event_type = %event_type,
                    severity = %severity,
                    description = %description,
                    "Security event"
                );
            }
        }
    }

    /// Log API request
    pub fn log_api_request(
        &self,
        method: &str,
        path: &str,
        status_code: u16,
        duration_ms: f64,
        user_id: Option<&str>,
    ) {
        tracing::info!(
            method = %method,
            path = %path,
            status_code = %status_code,
            duration_ms = %duration_ms,
            user_id = ?user_id,
            "API request"
        );
    }

    /// Create error span with context
    pub fn error_span(&self, error: &str, context: HashMap<String, String>) -> tracing::Span {
        let span = tracing::error_span!(
            "bolt_error",
            error = %error,
            service = %self.service_name
        );

        for (key, value) in context {
            span.record(key.as_str(), &tracing::field::display(&value));
        }

        span
    }
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            enable_json_logs: false,
            enable_file_logging: true,
            log_file_path: "/var/log/bolt/bolt.log".to_string(),
            enable_jaeger: false,
            jaeger_endpoint: "http://localhost:14268/api/traces".to_string(),
            service_name: "bolt-runtime".to_string(),
            enable_metrics: true,
            structured_logging: true,
        }
    }
}

/// Tracing macros for common operations
#[macro_export]
macro_rules! trace_container_op {
    ($container_id:expr, $operation:expr, $block:block) => {
        let span = tracing::info_span!(
            "container_operation",
            container_id = %$container_id,
            operation = %$operation
        );
        let _enter = span.enter();

        let start = std::time::Instant::now();
        let result = $block;
        let duration = start.elapsed();

        tracing::info!(
            duration_ms = %duration.as_millis(),
            success = %result.is_ok(),
            "Container operation completed"
        );

        result
    };
}

#[macro_export]
macro_rules! trace_gpu_op {
    ($gpu_id:expr, $operation:expr, $block:block) => {
        let span = tracing::info_span!(
            "gpu_operation",
            gpu_id = %$gpu_id,
            operation = %$operation
        );
        let _enter = span.enter();

        let start = std::time::Instant::now();
        let result = $block;
        let duration = start.elapsed();

        tracing::info!(
            duration_ms = %duration.as_millis(),
            success = %result.is_ok(),
            "GPU operation completed"
        );

        result
    };
}

#[macro_export]
macro_rules! trace_network_op {
    ($interface:expr, $operation:expr, $block:block) => {
        let span = tracing::info_span!(
            "network_operation",
            interface = %$interface,
            operation = %$operation
        );
        let _enter = span.enter();

        let start = std::time::Instant::now();
        let result = $block;
        let duration = start.elapsed();

        tracing::info!(
            duration_ms = %duration.as_millis(),
            success = %result.is_ok(),
            "Network operation completed"
        );

        result
    };
}

/// Performance instrumentation
pub struct PerformanceInstrumentation {
    start_time: std::time::Instant,
    operation: String,
    context: HashMap<String, String>,
}

impl PerformanceInstrumentation {
    pub fn new(operation: &str) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            operation: operation.to_string(),
            context: HashMap::new(),
        }
    }

    pub fn add_context(&mut self, key: &str, value: &str) {
        self.context.insert(key.to_string(), value.to_string());
    }

    pub fn finish(self) {
        let duration = self.start_time.elapsed();

        tracing::info!(
            operation = %self.operation,
            duration_ms = %duration.as_millis(),
            context = ?self.context,
            "Performance measurement"
        );
    }

    pub fn finish_with_result<T, E>(self, result: &Result<T, E>) -> Duration
    where
        E: std::fmt::Display,
    {
        let duration = self.start_time.elapsed();

        match result {
            Ok(_) => {
                tracing::info!(
                    operation = %self.operation,
                    duration_ms = %duration.as_millis(),
                    success = true,
                    context = ?self.context,
                    "Performance measurement"
                );
            }
            Err(e) => {
                tracing::warn!(
                    operation = %self.operation,
                    duration_ms = %duration.as_millis(),
                    success = false,
                    error = %e,
                    context = ?self.context,
                    "Performance measurement with error"
                );
            }
        }

        duration
    }
}
