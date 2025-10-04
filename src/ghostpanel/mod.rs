//! GhostPanel WebSocket Backend
//!
//! Provides real-time GPU and container metrics via WebSocket for the GhostPanel UI

pub mod metrics_server;

pub use metrics_server::{GhostPanelMetricsServer, MetricsMessage};
