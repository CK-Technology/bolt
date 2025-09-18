use anyhow::Result;
use async_trait::async_trait;
use std::any::Any;

use super::{GpuVendor, OptimizationContext, PluginType};

#[async_trait]
pub trait Plugin: Send + Sync {
    async fn initialize(&self) -> Result<()>;
    async fn enable(&self) -> Result<()>;
    async fn disable(&self) -> Result<()>;
    async fn is_enabled(&self) -> bool;
    fn plugin_type(&self) -> PluginType;
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}

#[async_trait]
pub trait GpuPlugin: Plugin {
    async fn setup_gpu_passthrough(&self, container_id: &str, device_id: u32) -> Result<()>;
    async fn enable_dlss(&self, container_id: &str, enabled: bool) -> Result<()>;
    async fn enable_reflex(&self, container_id: &str, enabled: bool) -> Result<()>;
    async fn set_power_limit(&self, watts: u32) -> Result<()>;
    async fn set_memory_clock(&self, mhz: u32) -> Result<()>;
    async fn set_core_clock(&self, mhz: u32) -> Result<()>;
    async fn get_gpu_metrics(&self) -> Result<GpuMetrics>;
    fn supported_vendors(&self) -> Vec<GpuVendor>;
}

#[async_trait]
pub trait OptimizationPlugin: Plugin {
    async fn apply_optimization(&self, context: &OptimizationContext) -> Result<()>;
    async fn remove_optimization(&self, context: &OptimizationContext) -> Result<()>;
    fn supports_optimization(&self, optimization_type: &str) -> bool;
    fn optimization_priority(&self) -> u32;
}

#[async_trait]
pub trait AudioPlugin: Plugin {
    async fn setup_audio_passthrough(&self, container_id: &str) -> Result<()>;
    async fn set_audio_latency(&self, latency_ms: u32) -> Result<()>;
    async fn enable_spatial_audio(&self, enabled: bool) -> Result<()>;
    fn supported_audio_systems(&self) -> Vec<AudioSystem>;
}

#[async_trait]
pub trait NetworkPlugin: Plugin {
    async fn optimize_for_gaming(&self, container_id: &str) -> Result<()>;
    async fn set_qos_priority(&self, priority: NetworkPriority) -> Result<()>;
    async fn enable_packet_batching(&self, enabled: bool) -> Result<()>;
    async fn get_network_metrics(&self) -> Result<NetworkMetrics>;
}

#[derive(Debug, Clone)]
pub struct GpuMetrics {
    pub utilization_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub temperature_celsius: f32,
    pub power_draw_watts: f32,
    pub core_clock_mhz: u32,
    pub memory_clock_mhz: u32,
}

#[derive(Debug, Clone)]
pub struct NetworkMetrics {
    pub latency_ms: f32,
    pub packet_loss_percent: f32,
    pub bandwidth_mbps: f32,
    pub jitter_ms: f32,
}

#[derive(Debug, Clone)]
pub enum AudioSystem {
    PipeWire,
    PulseAudio,
    Alsa,
    Jack,
}

#[derive(Debug, Clone)]
pub enum NetworkPriority {
    Gaming,
    Streaming,
    Background,
    Critical,
}

pub trait GameSpecificPlugin: Plugin {
    fn supported_games(&self) -> Vec<String>;
    async fn optimize_for_game(&self, game_title: &str, container_id: &str) -> Result<()>;
}
