/// Container information
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct SurgeStatus {
    pub services: Vec<ServiceInfo>,
    pub networks: Vec<NetworkInfo>,
}

/// Service information
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub status: String,
    pub replicas: u32,
}

/// Network information
#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub id: String, // Docker API compatibility
    pub name: String,
    pub driver: String,
    pub subnet: Option<String>,
    pub created: Option<String>, // Docker API compatibility
}

use std::collections::HashMap;
