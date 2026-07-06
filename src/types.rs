use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Container information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub names: Vec<String>, // Docker API compatibility
    pub image: String,
    pub image_id: String, // Docker API compatibility
    pub command: String,
    pub created: String,
    pub status: String,
    pub ports: Vec<String>,
    pub labels: HashMap<String, String>, // Docker API compatibility
    pub uptime: Option<String>,          // Docker API compatibility
    pub runtime: Option<String>,         // nvbind, docker, etc.
}

/// Surge orchestration status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurgeStatus {
    pub services: Vec<ServiceInfo>,
    pub networks: Vec<NetworkInfo>,
}

/// Service information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub status: String,
    pub replicas: u32,
}

/// Network information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub id: String, // Docker API compatibility
    pub name: String,
    pub driver: String,
    pub subnet: Option<String>,
    pub created: Option<String>, // Docker API compatibility
}

/// Image information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub created: Option<String>,
}
