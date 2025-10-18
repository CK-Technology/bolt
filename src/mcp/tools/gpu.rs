//! GPU statistics tool
//!
//! Exposes GPU metrics for gaming containers via NVIDIA Management Library (NVML)

use crate::mcp::{tools::McpTool, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// GPU statistics tool
///
/// Provides real-time GPU metrics including:
/// - Utilization percentage
/// - Memory usage
/// - Temperature
/// - Power draw
/// - Clock speeds
pub struct GpuStatsTool {
    #[cfg(feature = "nvidia-support")]
    nvml: Option<nvml_wrapper::Nvml>,
}

impl GpuStatsTool {
    /// Create a new GPU stats tool
    pub fn new() -> Result<Self> {
        #[cfg(feature = "nvidia-support")]
        {
            let nvml = match nvml_wrapper::Nvml::init() {
                Ok(n) => {
                    tracing::info!("NVML initialized successfully");
                    Some(n)
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize NVML: {}. GPU stats will be unavailable.", e);
                    None
                }
            };
            Ok(Self { nvml })
        }

        #[cfg(not(feature = "nvidia-support"))]
        {
            tracing::warn!("GPU stats tool created without nvidia-support feature");
            Ok(Self {})
        }
    }

    #[cfg(feature = "nvidia-support")]
    async fn get_nvidia_stats(&self, device_id: u32) -> Result<GpuStats> {
        use crate::mcp::McpError;

        let nvml = self.nvml.as_ref().ok_or_else(|| {
            McpError::ToolExecution("NVML not initialized".to_string())
        })?;

        let device = nvml.device_by_index(device_id).map_err(|e| {
            McpError::ToolExecution(format!("Failed to get GPU device {}: {}", device_id, e))
        })?;

        let utilization = device
            .utilization_rates()
            .map(|u| u.gpu as f32)
            .unwrap_or(0.0);

        let memory_info = device.memory_info().ok();
        let memory_used = memory_info.as_ref().map(|m| m.used).unwrap_or(0);
        let memory_total = memory_info.as_ref().map(|m| m.total).unwrap_or(0);

        let temperature = device
            .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
            .unwrap_or(0);

        let power_draw = device.power_usage().unwrap_or(0) as f32 / 1000.0; // Convert mW to W

        let clock_speed = device
            .clock_info(nvml_wrapper::enum_wrappers::device::Clock::Graphics)
            .unwrap_or(0);

        let name = device.name().unwrap_or_else(|_| "Unknown GPU".to_string());

        Ok(GpuStats {
            device_id,
            name,
            utilization_percent: utilization,
            memory_used_mb: memory_used / 1024 / 1024,
            memory_total_mb: memory_total / 1024 / 1024,
            temperature_celsius: temperature,
            power_draw_watts: power_draw,
            clock_speed_mhz: clock_speed,
        })
    }
}

impl Default for GpuStatsTool {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            #[cfg(feature = "nvidia-support")]
            return Self { nvml: None };
            #[cfg(not(feature = "nvidia-support"))]
            return Self {};
        })
    }
}

#[derive(Debug, Deserialize)]
struct GpuStatsInput {
    #[serde(default)]
    device_id: u32,
}

#[derive(Debug, Serialize)]
struct GpuStats {
    device_id: u32,
    name: String,
    utilization_percent: f32,
    memory_used_mb: u64,
    memory_total_mb: u64,
    temperature_celsius: u32,
    power_draw_watts: f32,
    clock_speed_mhz: u32,
}

impl McpTool for GpuStatsTool {
    fn name(&self) -> &str {
        "bolt_gpu_stats"
    }

    fn description(&self) -> &str {
        "Get real-time GPU statistics including utilization, memory, temperature, and power draw"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device_id": {
                    "type": "integer",
                    "description": "GPU device ID (default: 0 for primary GPU)",
                    "default": 0,
                    "minimum": 0
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use crate::mcp::McpError;

        let args: GpuStatsInput = serde_json::from_value(input)
            .map_err(|e| McpError::Json(e))?;

        tracing::info!("Querying GPU stats for device {}", args.device_id);

        #[cfg(feature = "nvidia-support")]
        {
            let stats = self.get_nvidia_stats(args.device_id).await?;
            Ok(serde_json::to_value(stats)?)
        }

        #[cfg(not(feature = "nvidia-support"))]
        {
            Err(McpError::ToolExecution(
                "GPU stats unavailable: nvidia-support feature not enabled".to_string()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_creation() {
        let tool = GpuStatsTool::default();
        assert_eq!(tool.name(), "bolt_gpu_stats");
    }

    #[test]
    fn test_input_schema() {
        let tool = GpuStatsTool::default();
        let schema = tool.input_schema();
        assert!(schema.is_object());
        assert!(schema["properties"]["device_id"].is_object());
    }
}
