use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod client;
pub mod types;

pub use client::DriftClient;
pub use types::*;

/// Drift Registry API specification for Bolt integration
/// Compatible with Docker Registry v2 + custom extensions for Bolt profiles/plugins

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftRegistry {
    pub name: String,
    pub url: String,
    pub auth: Option<RegistryAuth>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryAuth {
    pub username: String,
    pub password: String,
    pub token: Option<String>,
}

/// API Response Types for Drift Registry

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileListResponse {
    pub profiles: Vec<ProfileSummary>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub downloads: u64,
    pub rating: f32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub tags: Vec<String>,
    pub compatible_games: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDetail {
    pub profile: crate::optimizations::OptimizationProfile,
    pub metadata: ProfileMetadata,
    pub manifest: ProfileManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetadata {
    pub author: String,
    pub author_email: String,
    pub version: String,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub downloads: u64,
    pub rating: f32,
    pub rating_count: u32,
    pub verified: bool,
    pub tags: Vec<String>,
    pub compatible_games: Vec<String>,
    pub system_requirements: SystemRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemRequirements {
    pub min_cpu_cores: Option<u32>,
    pub min_memory_gb: Option<u32>,
    pub required_gpu_vendor: Option<String>,
    pub min_gpu_memory_gb: Option<u32>,
    pub supported_os: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileManifest {
    pub schema_version: String,
    pub media_type: String,
    pub config: ManifestConfig,
    pub layers: Vec<ManifestLayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestConfig {
    pub media_type: String,
    pub size: u64,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestLayer {
    pub media_type: String,
    pub size: u64,
    pub digest: String,
}

/// Plugin API Types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginListResponse {
    pub plugins: Vec<PluginSummary>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSummary {
    pub name: String,
    pub version: String,
    pub plugin_type: String,
    pub author: String,
    pub description: String,
    pub downloads: u64,
    pub rating: f32,
    pub verified: bool,
    pub supported_platforms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDetail {
    pub manifest: crate::plugins::PluginManifest,
    pub metadata: PluginMetadata,
    pub binary_manifest: PluginBinaryManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub author: String,
    pub author_email: String,
    pub license: String,
    pub repository: String,
    pub documentation: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub downloads: u64,
    pub rating: f32,
    pub rating_count: u32,
    pub verified: bool,
    pub supported_platforms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginBinaryManifest {
    pub platform: String,
    pub architecture: String,
    pub size: u64,
    pub digest: String,
    pub download_url: String,
}

/// Upload Types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileUploadRequest {
    pub profile: crate::optimizations::OptimizationProfile,
    pub metadata: ProfileUploadMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileUploadMetadata {
    pub author_email: String,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub tags: Vec<String>,
    pub compatible_games: Vec<String>,
    pub system_requirements: SystemRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginUploadRequest {
    pub manifest: crate::plugins::PluginManifest,
    pub metadata: PluginUploadMetadata,
    pub binary_data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginUploadMetadata {
    pub author_email: String,
    pub license: String,
    pub repository: String,
    pub documentation: Option<String>,
    pub supported_platforms: Vec<String>,
}

/// Metrics Types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryMetrics {
    pub total_profiles: u64,
    pub total_plugins: u64,
    pub total_downloads: u64,
    pub daily_downloads: HashMap<String, u64>,
    pub popular_profiles: Vec<ProfileSummary>,
    pub popular_plugins: Vec<PluginSummary>,
    pub storage_usage_bytes: u64,
    pub active_users_30d: u64,
}

/// Search Types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: Option<String>,
    pub tags: Option<Vec<String>>,
    pub author: Option<String>,
    pub game: Option<String>,
    pub gpu_vendor: Option<String>,
    pub sort_by: Option<String>, // "downloads", "rating", "created_at", "updated_at"
    pub sort_order: Option<String>, // "asc", "desc"
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse<T> {
    pub results: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
}

/// Error Types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.error, self.message)
    }
}

impl std::error::Error for ApiError {}

/// API Endpoints for Drift Registry

pub const API_VERSION: &str = "v1";

pub mod endpoints {
    pub const PROFILES_LIST: &str = "/v1/profiles";
    pub const PROFILES_SEARCH: &str = "/v1/profiles/search";
    pub const PROFILES_GET: &str = "/v1/profiles/{name}";
    pub const PROFILES_DOWNLOAD: &str = "/v1/profiles/{name}/download";
    pub const PROFILES_UPLOAD: &str = "/v1/profiles/upload";
    pub const PROFILES_DELETE: &str = "/v1/profiles/{name}";

    pub const PLUGINS_LIST: &str = "/v1/plugins";
    pub const PLUGINS_SEARCH: &str = "/v1/plugins/search";
    pub const PLUGINS_GET: &str = "/v1/plugins/{name}";
    pub const PLUGINS_DOWNLOAD: &str = "/v1/plugins/{name}/download";
    pub const PLUGINS_UPLOAD: &str = "/v1/plugins/upload";
    pub const PLUGINS_DELETE: &str = "/v1/plugins/{name}";

    pub const AUTH_LOGIN: &str = "/v1/auth/login";
    pub const AUTH_REGISTER: &str = "/v1/auth/register";
    pub const AUTH_REFRESH: &str = "/v1/auth/refresh";
    pub const AUTH_LOGOUT: &str = "/v1/auth/logout";

    pub const METRICS_OVERVIEW: &str = "/v1/metrics";
    pub const METRICS_PROFILES: &str = "/v1/metrics/profiles";
    pub const METRICS_PLUGINS: &str = "/v1/metrics/plugins";

    pub const HEALTH: &str = "/health";
    pub const VERSION: &str = "/version";
}

/// Media Types for Registry Content

pub mod media_types {
    pub const BOLT_PROFILE: &str = "application/vnd.bolt.profile.v1+toml";
    pub const BOLT_PLUGIN_MANIFEST: &str = "application/vnd.bolt.plugin.manifest.v1+json";
    pub const BOLT_PLUGIN_BINARY: &str = "application/vnd.bolt.plugin.binary.v1+octet-stream";
    pub const DOCKER_MANIFEST: &str = "application/vnd.docker.distribution.manifest.v2+json";
    pub const OCI_MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
}