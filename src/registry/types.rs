use serde::{Deserialize, Serialize};

/// Additional type definitions for Drift Registry integration

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    pub name: String,
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub token: Option<String>,
    pub insecure: bool,
    pub timeout_seconds: u64,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            name: "drift".to_string(),
            url: "https://registry.bolt.dev".to_string(),
            username: None,
            password: None,
            token: None,
            insecure: false,
            timeout_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadProgress {
    pub bytes_uploaded: u64,
    pub total_bytes: u64,
    pub percentage: f32,
    pub status: UploadStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UploadStatus {
    Preparing,
    Uploading,
    Processing,
    Complete,
    Failed { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub percentage: Option<f32>,
    pub status: DownloadStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DownloadStatus {
    Starting,
    Downloading,
    Complete,
    Failed { error: String },
}

/// Registry compatibility types for Docker Registry v2 API

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerManifest {
    pub schema_version: u32,
    pub media_type: String,
    pub config: DockerConfig,
    pub layers: Vec<DockerLayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerConfig {
    pub media_type: String,
    pub size: u64,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerLayer {
    pub media_type: String,
    pub size: u64,
    pub digest: String,
    pub urls: Option<Vec<String>>,
}

/// Bolt-specific extensions to Registry API

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltRegistryManifest {
    pub schema_version: String,
    pub media_type: String,
    pub bolt_version: String,
    pub content_type: BoltContentType,
    pub config: super::ManifestConfig,
    pub layers: Vec<super::ManifestLayer>,
    pub annotations: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BoltContentType {
    Profile,
    Plugin,
    Capsule,
    Template,
}

/// Rate limiting and quota types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub requests_per_hour: u32,
    pub downloads_per_day: u32,
    pub upload_size_mb: u32,
    pub storage_quota_gb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaUsage {
    pub requests_used: u32,
    pub downloads_used: u32,
    pub storage_used_gb: f32,
    pub reset_time: chrono::DateTime<chrono::Utc>,
}

/// Registry statistics and analytics

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryStats {
    pub total_repositories: u64,
    pub total_profiles: u64,
    pub total_plugins: u64,
    pub total_downloads: u64,
    pub storage_used_gb: f32,
    pub bandwidth_used_gb: f32,
    pub active_users: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopularityMetrics {
    pub downloads_24h: u64,
    pub downloads_7d: u64,
    pub downloads_30d: u64,
    pub unique_users_30d: u64,
    pub rating_average: f32,
    pub rating_count: u32,
}

/// Error handling types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedError {
    pub code: String,
    pub message: String,
    pub details: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCode {
    AuthenticationFailed,
    AuthorizationFailed,
    ResourceNotFound,
    ResourceAlreadyExists,
    InvalidRequest,
    QuotaExceeded,
    RateLimitExceeded,
    UploadFailed,
    DownloadFailed,
    SystemError,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCode::AuthenticationFailed => write!(f, "AUTHENTICATION_FAILED"),
            ErrorCode::AuthorizationFailed => write!(f, "AUTHORIZATION_FAILED"),
            ErrorCode::ResourceNotFound => write!(f, "RESOURCE_NOT_FOUND"),
            ErrorCode::ResourceAlreadyExists => write!(f, "RESOURCE_ALREADY_EXISTS"),
            ErrorCode::InvalidRequest => write!(f, "INVALID_REQUEST"),
            ErrorCode::QuotaExceeded => write!(f, "QUOTA_EXCEEDED"),
            ErrorCode::RateLimitExceeded => write!(f, "RATE_LIMIT_EXCEEDED"),
            ErrorCode::UploadFailed => write!(f, "UPLOAD_FAILED"),
            ErrorCode::DownloadFailed => write!(f, "DOWNLOAD_FAILED"),
            ErrorCode::SystemError => write!(f, "SYSTEM_ERROR"),
        }
    }
}
