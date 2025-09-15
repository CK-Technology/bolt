/// Container information
#[derive(Debug, Clone)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
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
    pub name: String,
    pub driver: String,
    pub subnet: Option<String>,
}