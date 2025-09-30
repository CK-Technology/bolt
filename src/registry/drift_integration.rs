use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::{Client, StatusCode, header::ACCEPT};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::{fs, io::AsyncWriteExt};
use tracing::{debug, info, warn};

use sha2::{Digest, Sha256};

use crate::runtime::storage::object_store::ObjectStore;

/// Enhanced Drift registry integration for Bolt ecosystem
/// Provides seamless package management across Drift, Ghostbay, and GhostWire
#[derive(Clone)]
pub struct DriftRegistryClient {
    pub endpoint: String,
    pub client: Client,
    pub object_store: Option<Arc<dyn ObjectStore>>,
    pub cache: Arc<RwLock<PackageCache>>,
    pub features: DriftFeatures,
    pub gaming_config: GamingPackageConfig,
    pub credentials: Option<(String, String)>,
}

impl std::fmt::Debug for DriftRegistryClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DriftRegistryClient")
            .field("endpoint", &self.endpoint)
            .field("has_object_store", &self.object_store.is_some())
            .field("features", &self.features)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftFeatures {
    pub package_signing: bool,
    pub vulnerability_scanning: bool,
    pub gaming_optimization: bool,
    pub p2p_distribution: bool,
    pub ghostwire_integration: bool,
    pub multi_arch_support: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingPackageConfig {
    pub enable_proton_metadata: bool,
    pub gpu_compatibility_checking: bool,
    pub steam_integration: bool,
    pub performance_profiling: bool,
    pub auto_optimization: bool,
    pub ghostforge_sync: bool,
}

/// Package metadata with gaming-specific information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub registry: String,
    pub manifest_digest: String,
    pub size_bytes: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,

    // Gaming-specific metadata
    pub gaming: Option<GamingMetadata>,

    // Security information
    pub security: SecurityMetadata,

    // Performance optimization
    pub optimization: OptimizationMetadata,

    // Ecosystem integration
    pub ecosystem: EcosystemMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingMetadata {
    pub is_game: bool,
    pub proton_compatible: bool,
    pub proton_versions: Vec<String>,
    pub gpu_requirements: GpuRequirements,
    pub steam_app_id: Option<u32>,
    pub wine_version: Option<String>,
    pub dxvk_version: Option<String>,
    pub performance_tier: PerformanceTier,
    pub anti_cheat: AntiCheatCompatibility,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuRequirements {
    pub nvidia: Option<GpuSpec>,
    pub amd: Option<GpuSpec>,
    pub intel: Option<GpuSpec>,
    pub vulkan_required: bool,
    pub directx_version: Option<String>,
    pub opengl_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuSpec {
    pub min_vram_mb: u32,
    pub min_compute_capability: Option<String>,
    pub driver_version: Option<String>,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceTier {
    Competitive, // Low latency, high FPS
    Balanced,    // Good balance
    Quality,     // High quality, may sacrifice FPS
    Streaming,   // Optimized for game streaming
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiCheatCompatibility {
    pub battleye: bool,
    pub eac: bool, // Easy Anti-Cheat
    pub vac: bool, // Valve Anti-Cheat
    pub denuvo: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityMetadata {
    pub signed: bool,
    pub signature_algorithm: Option<String>,
    pub public_key_fingerprint: Option<String>,
    pub vulnerability_scan: Option<VulnerabilityScan>,
    pub attestation: Option<BuildAttestation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnerabilityScan {
    pub scanned_at: chrono::DateTime<chrono::Utc>,
    pub scanner: String,
    pub critical_count: u32,
    pub high_count: u32,
    pub medium_count: u32,
    pub low_count: u32,
    pub total_count: u32,
    pub cves: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildAttestation {
    pub build_system: String,
    pub source_repo: String,
    pub source_commit: String,
    pub build_timestamp: chrono::DateTime<chrono::Utc>,
    pub reproducible: bool,
    pub build_environment: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationMetadata {
    pub cpu_architecture: Vec<String>,
    pub optimized_for: Vec<String>,
    pub size_optimized: bool,
    pub performance_optimized: bool,
    pub startup_time_ms: Option<u32>,
    pub memory_usage_mb: Option<u32>,
    pub benchmarks: Vec<BenchmarkResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub test_name: String,
    pub value: f64,
    pub unit: String,
    pub better_direction: String, // "higher" or "lower"
    pub measured_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemMetadata {
    pub ghostforge_compatible: bool,
    pub ghostwire_routing: bool,
    pub ghostbay_optimized: bool,
    pub cluster_ready: bool,
    pub mesh_networking: bool,
}

/// Local package cache for performance
#[derive(Debug, Default)]
pub struct PackageCache {
    pub packages: HashMap<String, BoltPackage>,
    pub manifests: HashMap<String, PackageManifest>,
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct ResolvedManifest {
    pub repository: String,
    pub reference: String,
    pub manifest: PackageManifest,
    pub registry_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub config: BlobDescriptor,
    #[serde(default)]
    pub layers: Vec<LayerDescriptor>,
    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestList {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub manifests: Vec<ManifestDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestDescriptor {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub digest: String,
    pub size: u64,
    pub platform: Platform,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    pub architecture: String,
    pub os: String,
    #[serde(default)]
    pub variant: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobDescriptor {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub size: u64,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerDescriptor {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub size: u64,
    pub digest: String,
    pub urls: Option<Vec<String>>,
    pub annotations: Option<HashMap<String, String>>,

    // Bolt-specific layer metadata
    #[serde(default)]
    pub gaming_assets: bool,
    #[serde(default)]
    pub system_libraries: bool,
    #[serde(default)]
    pub user_data: bool,
    #[serde(default)]
    pub cacheable: bool,
}

impl DriftRegistryClient {
    /// Create a new Drift registry client with optional object store integration
    pub async fn new(
        endpoint: String,
        object_store: Option<Arc<dyn ObjectStore>>,
        credentials: Option<(String, String)>,
    ) -> Result<Self> {
        info!("🌊 Initializing Drift Registry Client");
        info!("  Registry: {}", endpoint);

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Bolt/1.0 (Drift-Registry-Client)")
            .build()?;

        let features = Self::detect_registry_features(&client, &endpoint).await?;

        info!("  Features detected:");
        info!(
            "    📦 Package Signing: {}",
            if features.package_signing {
                "✅"
            } else {
                "❌"
            }
        );
        info!(
            "    🛡️ Vulnerability Scanning: {}",
            if features.vulnerability_scanning {
                "✅"
            } else {
                "❌"
            }
        );
        info!(
            "    🎮 Gaming Optimization: {}",
            if features.gaming_optimization {
                "✅"
            } else {
                "❌"
            }
        );
        info!(
            "    🌐 P2P Distribution: {}",
            if features.p2p_distribution {
                "✅"
            } else {
                "❌"
            }
        );
        info!(
            "    🕸️ GhostWire Integration: {}",
            if features.ghostwire_integration {
                "✅"
            } else {
                "❌"
            }
        );

        Ok(Self {
            endpoint,
            client,
            object_store,
            cache: Arc::new(RwLock::new(PackageCache::default())),
            features,
            gaming_config: GamingPackageConfig {
                enable_proton_metadata: true,
                gpu_compatibility_checking: true,
                steam_integration: true,
                performance_profiling: true,
                auto_optimization: true,
                ghostforge_sync: true,
            },
            credentials,
        })
    }

    #[cfg(test)]
    pub fn new_test(object_store: Option<Arc<dyn ObjectStore>>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Bolt/1.0 (Drift-Registry-Client-Test)")
            .build()
            .expect("failed to build reqwest client");

        Self {
            endpoint: "https://registry.test".to_string(),
            client,
            object_store,
            cache: Arc::new(RwLock::new(PackageCache::default())),
            features: DriftFeatures {
                package_signing: false,
                vulnerability_scanning: false,
                gaming_optimization: false,
                p2p_distribution: false,
                ghostwire_integration: false,
                multi_arch_support: true,
            },
            gaming_config: GamingPackageConfig {
                enable_proton_metadata: true,
                gpu_compatibility_checking: true,
                steam_integration: true,
                performance_profiling: true,
                auto_optimization: true,
                ghostforge_sync: true,
            },
            credentials: None,
        }
    }

    fn with_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some((ref user, ref pass)) = self.credentials {
            builder.basic_auth(user, Some(pass))
        } else {
            builder
        }
    }

    fn parse_reference(image: &str) -> Result<(String, String)> {
        if let Some((repository, digest)) = image.split_once('@') {
            let normalized_repo = Self::normalize_repository(repository);
            return Ok((normalized_repo, digest.to_string()));
        }

        if let Some((repository, tag)) = image.rsplit_once(':') {
            if tag.contains('/') {
                let normalized_repo = Self::normalize_repository(image);
                Ok((normalized_repo, "latest".to_string()))
            } else {
                let normalized_repo = Self::normalize_repository(repository);
                Ok((normalized_repo, tag.to_string()))
            }
        } else {
            let normalized_repo = Self::normalize_repository(image);
            Ok((normalized_repo, "latest".to_string()))
        }
    }

    /// Normalize repository name for Docker Hub (add library/ prefix for official images)
    fn normalize_repository(repository: &str) -> String {
        // If no slash, it's an official Docker Hub image that needs library/ prefix
        if !repository.contains('/') {
            format!("library/{}", repository)
        } else {
            repository.to_string()
        }
    }

    fn manifest_cache_key(repository: &str, reference: &str) -> String {
        format!("{}@{}", repository, reference)
    }

    pub async fn resolve_manifest(&self, image: &str) -> Result<ResolvedManifest> {
        let (repository, reference) = Self::parse_reference(image)?;

        // For Docker Hub, get bearer token first
        let token = if self.endpoint.contains("docker.io") || self.endpoint.contains("registry-1.docker.io") {
            Some(self.get_docker_hub_token(&repository).await?)
        } else {
            None
        };

        if let Some(ref object_store) = self.object_store {
            match object_store.fetch_manifest(&repository, &reference).await {
                Ok(Some(bytes)) => {
                    debug!(
                        "Resolved manifest {}@{} from object store cache",
                        repository, reference
                    );

                    let manifest: PackageManifest = serde_json::from_slice(&bytes)
                        .context("Failed to deserialize cached manifest payload")?;

                    let registry_digest = Some(format!("sha256:{:x}", Sha256::digest(&bytes)));

                    {
                        let mut cache = self.cache.write().await;
                        cache.manifests.insert(
                            Self::manifest_cache_key(&repository, &reference),
                            manifest.clone(),
                        );
                        cache.last_updated = Some(chrono::Utc::now());
                    }

                    return Ok(ResolvedManifest {
                        repository,
                        reference,
                        manifest,
                        registry_digest,
                    });
                }
                Ok(None) => {}
                Err(err) => {
                    warn!(
                        "Failed to read manifest {}@{} from object store: {}",
                        repository, reference, err
                    );
                }
            }
        }

        // First check if we need to authenticate
        self.ensure_authenticated().await?;

        let url = format!(
            "{}/v2/{}/manifests/{}",
            self.endpoint, repository, reference
        );

        debug!("Fetching manifest from {}", url);

        let accept_header = "application/vnd.oci.image.manifest.v1+json,application/vnd.docker.distribution.manifest.v2+json";
        let mut request = self.client.get(&url).header(ACCEPT, accept_header);

        if let Some(ref token_value) = token {
            request = request.bearer_auth(token_value);
        } else {
            request = self.with_auth(request);
        }

        let response = request
            .send()
            .await
            .with_context(|| format!("Failed to request manifest for {}", image))?;

        match response.status() {
            StatusCode::OK => {}
            StatusCode::NOT_FOUND => {
                return Err(anyhow::anyhow!("Image not found in registry: {}", image));
            }
            status => {
                return Err(anyhow::anyhow!(
                    "Failed to fetch manifest {} (status: {})",
                    image,
                    status
                ));
            }
        }

        let registry_digest = response
            .headers()
            .get("Docker-Content-Digest")
            .and_then(|header| header.to_str().ok())
            .map(|s| s.to_string());

        let bytes = response
            .bytes()
            .await
            .context("Failed to read manifest body from registry")?;

        if let Some(ref object_store) = self.object_store {
            if let Err(err) = object_store
                .store_manifest(&repository, &reference, &bytes)
                .await
            {
                warn!(
                    "Failed to persist manifest {}@{} to object store: {}",
                    repository, reference, err
                );
            }
        }

        // Check if this is a manifest list (multi-arch)
        let manifest = if let Ok(manifest_list) = serde_json::from_slice::<ManifestList>(&bytes) {
            // It's a manifest list - select the right platform
            debug!("Manifest list detected, selecting platform manifest");
            self.select_platform_manifest(&repository, &reference, &manifest_list, token.as_deref())
                .await?
        } else {
            // Try to parse as regular manifest
            serde_json::from_slice::<PackageManifest>(&bytes)
                .context("Failed to deserialize manifest payload (not a manifest list or valid manifest)")?
        };

        {
            let mut cache = self.cache.write().await;
            cache.manifests.insert(
                Self::manifest_cache_key(&repository, &reference),
                manifest.clone(),
            );
            cache.last_updated = Some(chrono::Utc::now());
        }

        Ok(ResolvedManifest {
            repository,
            reference,
            manifest,
            registry_digest,
        })
    }

    pub async fn download_blob_to(
        &self,
        repository: &str,
        digest: &str,
        destination: &Path,
    ) -> Result<()> {
        if destination.exists() {
            return Ok(());
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to prepare directory {}", parent.display()))?;
        }

        // Get Docker Hub token if needed
        let token = if self.endpoint.contains("docker.io") || self.endpoint.contains("registry-1.docker.io") {
            Some(self.get_docker_hub_token(repository).await?)
        } else {
            None
        };

        let url = format!("{}/v2/{}/blobs/{}", self.endpoint, repository, digest);
        debug!("Downloading blob {} from {}", digest, url);

        let mut request = self.client.get(&url);
        if let Some(ref token_value) = token {
            request = request.bearer_auth(token_value);
        } else {
            request = self.with_auth(request);
        }

        let response = request
            .send()
            .await
            .with_context(|| format!("Failed to download blob {}", digest))?;

        match response.status() {
            StatusCode::OK => {}
            StatusCode::ACCEPTED => {}
            StatusCode::NOT_FOUND => {
                return Err(anyhow::anyhow!(
                    "Layer {} not found in repository {}",
                    digest,
                    repository
                ));
            }
            status => {
                return Err(anyhow::anyhow!(
                    "Failed to download blob {} (status: {})",
                    digest,
                    status
                ));
            }
        }

        let temp_path = destination.with_extension("download");
        let mut file = fs::File::create(&temp_path)
            .await
            .with_context(|| format!("Failed to create temp file at {}", temp_path.display()))?;

        let mut hasher = Sha256::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to read blob stream")?;
            hasher.update(&chunk);
            file.write_all(&chunk)
                .await
                .context("Failed to write blob to disk")?;
        }

        file.flush().await.context("Failed to flush blob to disk")?;
        drop(file);

        let computed_digest = format!("sha256:{:x}", hasher.finalize());
        if let Some((algo, _)) = digest.split_once(':') {
            if algo.eq_ignore_ascii_case("sha256") && computed_digest != digest {
                fs::remove_file(&temp_path).await.ok();
                return Err(anyhow::anyhow!(
                    "Digest mismatch for {} (expected {}, got {})",
                    destination.display(),
                    digest,
                    computed_digest
                ));
            }
        }

        fs::rename(&temp_path, destination)
            .await
            .with_context(|| format!("Failed to finalize blob at {}", destination.display()))?;

        Ok(())
    }

    pub async fn download_config_to(
        &self,
        repository: &str,
        digest: &str,
        destination: &Path,
    ) -> Result<()> {
        if destination.exists() {
            return Ok(());
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to prepare directory {}", parent.display()))?;
        }

        if let Some(ref object_store) = self.object_store {
            match object_store
                .download_cached_config(repository, digest, destination)
                .await
            {
                Ok(true) => {
                    debug!(
                        "Config {} fetched from object store cache for {}",
                        digest, repository
                    );
                    return Ok(());
                }
                Ok(false) => {}
                Err(err) => {
                    warn!(
                        "Object store config cache check failed for {} ({}): {}",
                        digest, repository, err
                    );
                }
            }
        }

        self.download_blob_to(repository, digest, destination)
            .await?;

        if let Some(ref object_store) = self.object_store {
            if let Err(err) = object_store
                .upload_config(repository, digest, destination)
                .await
            {
                warn!(
                    "Failed to upload config {} for {} to object store: {}",
                    digest, repository, err
                );
            }
        }

        Ok(())
    }

    /// Detect registry features by querying the API
    async fn detect_registry_features(client: &Client, endpoint: &str) -> Result<DriftFeatures> {
        debug!("🔍 Detecting registry features");

        // Query the registry's feature endpoint
        let features_url = format!("{}/v2/features", endpoint);

        match client.get(&features_url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let features: DriftFeatures = response
                        .json()
                        .await
                        .context("Failed to parse registry features")?;
                    Ok(features)
                } else {
                    warn!("Registry doesn't support feature detection, using defaults");
                    Ok(DriftFeatures {
                        package_signing: false,
                        vulnerability_scanning: false,
                        gaming_optimization: false,
                        p2p_distribution: false,
                        ghostwire_integration: false,
                        multi_arch_support: true,
                    })
                }
            }
            Err(_) => {
                warn!("Unable to detect registry features, using defaults");
                Ok(DriftFeatures {
                    package_signing: false,
                    vulnerability_scanning: false,
                    gaming_optimization: false,
                    p2p_distribution: false,
                    ghostwire_integration: false,
                    multi_arch_support: true,
                })
            }
        }
    }

    /// Search for packages with gaming-specific filters
    pub async fn search_packages(
        &self,
        query: &str,
        gaming_filter: Option<GamingSearchFilter>,
    ) -> Result<Vec<BoltPackage>> {
        info!("🔍 Searching packages: '{}'", query);

        let mut url = format!("{}/v2/search?q={}", self.endpoint, query);

        // Add gaming-specific filters
        if let Some(filter) = gaming_filter {
            if filter.games_only {
                url.push_str("&gaming=true");
            }
            if let Some(ref proton_version) = filter.proton_version {
                url.push_str(&format!("&proton={}", proton_version));
            }
            if let Some(ref gpu_vendor) = filter.gpu_vendor {
                url.push_str(&format!("&gpu={}", gpu_vendor));
            }
            if let Some(tier) = filter.performance_tier {
                url.push_str(&format!("&tier={:?}", tier));
            }
        }

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to search packages")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Search failed: {}", response.status()));
        }

        let packages: Vec<BoltPackage> = response
            .json()
            .await
            .context("Failed to parse search results")?;

        info!("✅ Found {} packages", packages.len());
        Ok(packages)
    }

    /// Pull package with intelligent source selection (registry vs P2P)
    pub async fn pull_package(
        &self,
        package_name: &str,
        version: Option<&str>,
        prefer_p2p: bool,
    ) -> Result<String> {
        info!(
            "📦 Pulling package: {} (version: {:?})",
            package_name, version
        );

        let package_ref = match version {
            Some(v) => format!("{}:{}", package_name, v),
            None => format!("{}:latest", package_name),
        };

        // Try P2P distribution first if enabled and preferred
        if prefer_p2p && self.features.p2p_distribution {
            if let Ok(path) = self.pull_via_p2p(&package_ref).await {
                info!("✅ Package pulled via P2P mesh network");
                return Ok(path);
            }
            warn!("P2P pull failed, falling back to registry");
        }

        // Pull from registry with object store optimization
        let path = self.pull_from_registry(&package_ref).await?;

        // Async background P2P sharing for future requests
        if self.features.p2p_distribution {
            let client = self.clone();
            let package_ref_clone = package_ref.clone();
            let path_clone = path.clone();

            tokio::spawn(async move {
                if let Err(e) = client.share_via_p2p(&package_ref_clone, &path_clone).await {
                    debug!("P2P sharing failed: {}", e);
                }
            });
        }

        info!("✅ Package pulled successfully: {}", path);
        Ok(path)
    }

    /// Pull package via P2P mesh network (GhostWire integration)
    async fn pull_via_p2p(&self, package_ref: &str) -> Result<String> {
        debug!("🌐 Attempting P2P pull for: {}", package_ref);

        // This would integrate with GhostWire's mesh networking
        // For now, simulate the P2P pull logic

        // Query mesh peers for the package
        let peers = self.discover_package_peers(package_ref).await?;

        if peers.is_empty() {
            return Err(anyhow::anyhow!("No peers found with package"));
        }

        // Select optimal peer based on latency and bandwidth
        let best_peer = self.select_optimal_peer(&peers).await?;

        // Download from peer using QUIC protocol
        let download_path = self.download_from_peer(&best_peer, package_ref).await?;

        debug!("✅ P2P download completed from peer: {}", best_peer.address);
        Ok(download_path)
    }

    /// Pull package from registry with object store optimization
    async fn pull_from_registry(&self, package_ref: &str) -> Result<String> {
        debug!("🌊 Pulling from registry: {}", package_ref);

        // Get package manifest
        let resolved = self.resolve_manifest(package_ref).await?;
        let manifest = &resolved.manifest;

        // If an object store cache is available, use optimized download
        if let Some(ref object_store) = self.object_store {
            return self
                .pull_via_object_store(
                    package_ref,
                    &resolved.repository,
                    manifest,
                    object_store.as_ref(),
                )
                .await;
        }

        // Standard registry pull
        self.pull_standard(package_ref, &resolved.repository, manifest)
            .await
    }

    /// Optimized pull using configured object store cache
    async fn pull_via_object_store(
        &self,
        package_ref: &str,
        repository: &str,
        manifest: &PackageManifest,
        object_store: &(dyn ObjectStore),
    ) -> Result<String> {
        debug!("🪣 Using object store optimized pull");

        // Check if layers are already cached in the object store
        let mut cached_layers = Vec::new();
        let mut missing_layers = Vec::new();

        for layer in &manifest.layers {
            if object_store.blob_exists(repository, &layer.digest).await? {
                cached_layers.push(layer);
            } else {
                missing_layers.push(layer);
            }
        }

        info!(
            "📊 Layer cache status: {} cached, {} missing",
            cached_layers.len(),
            missing_layers.len()
        );

        // Download missing layers in parallel
        if !missing_layers.is_empty() {
            self.download_missing_layers(repository, &missing_layers)
                .await?;
        }

        // Assemble final image
        let image_path = self
            .assemble_image_from_layers(package_ref, repository, &manifest.layers)
            .await?;

        Ok(image_path)
    }

    /// Standard registry pull without object store assistance
    async fn pull_standard(
        &self,
        package_ref: &str,
        repository: &str,
        manifest: &PackageManifest,
    ) -> Result<String> {
        debug!("📦 Standard registry pull");

        // Download all layers
        for layer in &manifest.layers {
            self.download_layer(repository, &layer.digest).await?;
        }

        // Assemble image
        let image_path = self
            .assemble_image_from_layers(package_ref, repository, &manifest.layers)
            .await?;

        Ok(image_path)
    }

    /// Get gaming compatibility information for a package
    pub async fn get_gaming_compatibility(
        &self,
        package_name: &str,
    ) -> Result<Option<GamingMetadata>> {
        if !self.features.gaming_optimization {
            return Ok(None);
        }

        let url = format!("{}/v2/gaming/{}/compatibility", self.endpoint, package_name);

        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            let gaming_data: GamingMetadata = response.json().await?;
            Ok(Some(gaming_data))
        } else {
            Ok(None)
        }
    }

    /// Push package to registry with gaming metadata
    pub async fn push_package(
        &self,
        package_path: &str,
        metadata: BoltPackage,
        gaming_metadata: Option<GamingMetadata>,
    ) -> Result<()> {
        info!("📤 Pushing package: {}", metadata.name);

        // Create enhanced manifest with gaming metadata
        let manifest = self
            .create_enhanced_manifest(&metadata, gaming_metadata)
            .await?;

        // Upload layers to registry (and object store if available)
        self.upload_package_layers(package_path, &manifest).await?;

        // Upload manifest
        self.upload_manifest(&metadata.name, &manifest).await?;

        // Update local cache
        self.update_cache(metadata).await;

        info!("✅ Package pushed successfully");
        Ok(())
    }

    // Implementation stubs for various helper methods
    async fn discover_package_peers(&self, _package_ref: &str) -> Result<Vec<MeshPeer>> {
        // Integrate with GhostWire mesh networking
        Ok(vec![])
    }

    async fn select_optimal_peer(&self, peers: &[MeshPeer]) -> Result<MeshPeer> {
        peers
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No peers available"))
    }

    async fn download_from_peer(&self, _peer: &MeshPeer, _package_ref: &str) -> Result<String> {
        // QUIC-based P2P download
        Ok("/tmp/package".to_string())
    }

    async fn share_via_p2p(&self, _package_ref: &str, _path: &str) -> Result<()> {
        // Share package via P2P mesh
        Ok(())
    }

    async fn download_missing_layers(
        &self,
        repository: &str,
        layers: &[&LayerDescriptor],
    ) -> Result<()> {
        for layer in layers {
            self.download_layer(repository, &layer.digest).await?;
        }
        Ok(())
    }

    async fn assemble_image_from_layers(
        &self,
        _package_ref: &str,
        repository: &str,
        _layers: &[LayerDescriptor],
    ) -> Result<String> {
        let cache_dir = Self::layer_cache_dir(repository);
        fs::create_dir_all(&cache_dir)
            .await
            .with_context(|| format!("Failed to ensure cache directory {}", cache_dir.display()))?;
        Ok(cache_dir.to_string_lossy().to_string())
    }

    async fn download_layer(&self, repository: &str, digest: &str) -> Result<()> {
        let cache_dir = Self::layer_cache_dir(repository);
        let destination = cache_dir.join(Self::blob_filename(digest));
        self.download_blob_to(repository, digest, &destination)
            .await
    }

    fn layer_cache_dir(repository: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push("bolt");
        path.push("layers");
        path.push(repository.replace('/', "_"));
        path
    }

    fn blob_filename(digest: &str) -> String {
        digest.replace(':', "_")
    }

    async fn create_enhanced_manifest(
        &self,
        _metadata: &BoltPackage,
        _gaming_metadata: Option<GamingMetadata>,
    ) -> Result<PackageManifest> {
        // Create manifest with gaming enhancements
        Ok(PackageManifest {
            schema_version: 2,
            media_type: "application/vnd.oci.image.manifest.v1+json".to_string(),
            config: BlobDescriptor {
                media_type: "application/vnd.oci.image.config.v1+json".to_string(),
                size: 1024,
                digest: "sha256:abcd1234".to_string(),
            },
            layers: vec![],
            annotations: HashMap::new(),
        })
    }

    async fn upload_package_layers(
        &self,
        _package_path: &str,
        _manifest: &PackageManifest,
    ) -> Result<()> {
        // Upload layers to registry and object store
        Ok(())
    }

    async fn upload_manifest(
        &self,
        _package_name: &str,
        _manifest: &PackageManifest,
    ) -> Result<()> {
        // Upload manifest to registry
        Ok(())
    }

    async fn update_cache(&self, package: BoltPackage) {
        let mut cache = self.cache.write().await;
        cache.packages.insert(package.name.clone(), package);
        cache.last_updated = Some(chrono::Utc::now());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingSearchFilter {
    pub games_only: bool,
    pub proton_version: Option<String>,
    pub gpu_vendor: Option<String>,
    pub performance_tier: Option<PerformanceTier>,
    pub anti_cheat_compatible: bool,
    pub steam_deck_verified: bool,
}

#[derive(Debug, Clone)]
pub struct MeshPeer {
    pub address: String,
    pub latency_ms: u32,
    pub bandwidth_mbps: u32,
    pub reliability_score: f64,
}

impl Default for DriftFeatures {
    fn default() -> Self {
        Self {
            package_signing: true,
            vulnerability_scanning: true,
            gaming_optimization: true,
            p2p_distribution: true,
            ghostwire_integration: true,
            multi_arch_support: true,
        }
    }
}

impl Default for GamingPackageConfig {
    fn default() -> Self {
        Self {
            enable_proton_metadata: true,
            gpu_compatibility_checking: true,
            steam_integration: true,
            performance_profiling: true,
            auto_optimization: true,
            ghostforge_sync: true,
        }
    }
}

// Enhanced registry client methods for better OCI support
impl DriftRegistryClient {
    /// Ensure the client is authenticated with the registry
    async fn ensure_authenticated(&self) -> Result<()> {
        // Docker Hub requires token-based auth even for public images
        if self.endpoint.contains("docker.io") || self.endpoint.contains("registry-1.docker.io") {
            // If we have credentials, they'll be used via with_auth()
            // For anonymous access, Docker Hub uses bearer tokens
            debug!("Docker Hub authentication will use bearer tokens from registry");
            return Ok(());
        }

        debug!("Registry authentication check for: {}", self.endpoint);
        Ok(())
    }

    /// Get Docker Hub bearer token for anonymous pulls
    async fn get_docker_hub_token(&self, repository: &str) -> Result<String> {
        let auth_url = format!(
            "https://auth.docker.io/token?service=registry.docker.io&scope=repository:{}:pull",
            repository
        );

        debug!("Fetching Docker Hub token for repository: {}", repository);

        let response = self
            .client
            .get(&auth_url)
            .send()
            .await
            .context("Failed to request Docker Hub token")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to get Docker Hub token: {}",
                response.status()
            ));
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            token: String,
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .context("Failed to parse Docker Hub token response")?;

        Ok(token_response.token)
    }

    /// Select the appropriate platform manifest from a manifest list
    async fn select_platform_manifest(
        &self,
        repository: &str,
        _reference: &str,
        manifest_list: &ManifestList,
        token: Option<&str>,
    ) -> Result<PackageManifest> {
        // Detect current platform
        let target_os = std::env::consts::OS;
        let target_arch = std::env::consts::ARCH;

        debug!("Selecting manifest for platform: {}/{}", target_os, target_arch);

        // Find matching platform
        let selected = manifest_list
            .manifests
            .iter()
            .find(|m| {
                m.platform.os == target_os
                    && (m.platform.architecture == target_arch ||
                        (target_arch == "x86_64" && m.platform.architecture == "amd64"))
            })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No manifest found for platform {}/{}",
                    target_os,
                    target_arch
                )
            })?;

        info!("Selected manifest digest: {} for {}/{}", selected.digest, target_os, target_arch);

        // Fetch the specific platform manifest by digest
        let url = format!(
            "{}/v2/{}/manifests/{}",
            self.endpoint, repository, selected.digest
        );

        let mut request = self.client.get(&url).header(
            ACCEPT,
            "application/vnd.oci.image.manifest.v1+json,application/vnd.docker.distribution.manifest.v2+json",
        );

        if let Some(token_value) = token {
            request = request.bearer_auth(token_value);
        } else {
            request = self.with_auth(request);
        }

        let response = request
            .send()
            .await
            .context("Failed to fetch platform-specific manifest")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to fetch manifest by digest {} (status: {})",
                selected.digest,
                response.status()
            ));
        }

        let bytes = response.bytes().await?;
        let manifest: PackageManifest = serde_json::from_slice(&bytes)
            .context("Failed to deserialize platform-specific manifest")?;

        Ok(manifest)
    }

    /// Calculate SHA256 digest of a file
    async fn calculate_file_digest(&self, path: &Path) -> Result<String> {
        use sha2::{Digest, Sha256};
        use tokio::io::AsyncReadExt;

        let mut file = fs::File::open(path).await?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192];

        loop {
            let bytes_read = file.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(hex::encode(hasher.finalize()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BoltError;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use tempfile::tempdir;

    struct TestObjectStore {
        manifest: Mutex<Option<Vec<u8>>>,
        config: Mutex<Option<Vec<u8>>>,
    }

    impl TestObjectStore {
        fn new(manifest: Option<Vec<u8>>, config: Option<Vec<u8>>) -> Self {
            Self {
                manifest: Mutex::new(manifest),
                config: Mutex::new(config),
            }
        }
    }

    type BoltResult<T> = crate::Result<T>;

    #[async_trait]
    impl ObjectStore for TestObjectStore {
        async fn blob_exists(&self, _repository: &str, _digest: &str) -> BoltResult<bool> {
            Ok(false)
        }

        async fn download_cached_layer(
            &self,
            _repository: &str,
            _digest: &str,
            _destination: &Path,
        ) -> BoltResult<bool> {
            Ok(false)
        }

        async fn upload_layer(
            &self,
            _repository: &str,
            _digest: &str,
            _source: &Path,
        ) -> BoltResult<()> {
            Ok(())
        }

        async fn download_cached_config(
            &self,
            _repository: &str,
            _digest: &str,
            destination: &Path,
        ) -> BoltResult<bool> {
            let maybe_bytes = self.config.lock().unwrap().clone();
            if let Some(bytes) = maybe_bytes {
                if let Some(parent) = destination.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(BoltError::from)?;
                }
                tokio::fs::write(destination, bytes)
                    .await
                    .map_err(BoltError::from)?;
                Ok(true)
            } else {
                Ok(false)
            }
        }

        async fn download_config(
            &self,
            _repository: &str,
            _digest: &str,
            _destination: &Path,
        ) -> BoltResult<()> {
            panic!("download_config should not be called in this test");
        }

        async fn upload_config(
            &self,
            _repository: &str,
            _digest: &str,
            _source: &Path,
        ) -> BoltResult<()> {
            panic!("upload_config should not be called in this test");
        }

        async fn fetch_manifest(
            &self,
            _repository: &str,
            _reference: &str,
        ) -> BoltResult<Option<Vec<u8>>> {
            Ok(self.manifest.lock().unwrap().clone())
        }

        async fn store_manifest(
            &self,
            _repository: &str,
            _reference: &str,
            _data: &[u8],
        ) -> BoltResult<()> {
            panic!("store_manifest should not be called in this test");
        }
    }

    fn test_registry_client(object_store: Option<Arc<dyn ObjectStore>>) -> DriftRegistryClient {
        DriftRegistryClient {
            endpoint: "https://example.registry".to_string(),
            client: Client::builder().build().expect("failed to build client"),
            object_store,
            cache: Arc::new(RwLock::new(PackageCache::default())),
            features: DriftFeatures::default(),
            gaming_config: GamingPackageConfig::default(),
            credentials: None,
        }
    }

    #[tokio::test]
    async fn resolve_manifest_uses_object_store_cache() {
        let manifest = PackageManifest {
            schema_version: 2,
            media_type: "application/vnd.oci.image.manifest.v1+json".into(),
            config: BlobDescriptor {
                media_type: "application/vnd.oci.image.config.v1+json".into(),
                size: 512,
                digest: "sha256:deadbeef".into(),
            },
            layers: vec![],
            annotations: HashMap::new(),
        };
        let manifest_bytes = serde_json::to_vec(&manifest).expect("serialize manifest");

        let client = test_registry_client(Some(Arc::new(TestObjectStore::new(
            Some(manifest_bytes.clone()),
            None,
        ))));

        let resolved = client
            .resolve_manifest("library/bolt:latest")
            .await
            .expect("resolve manifest");

        assert_eq!(resolved.manifest.config.digest, "sha256:deadbeef");
        assert!(
            resolved
                .registry_digest
                .expect("digest")
                .starts_with("sha256:")
        );

        let cache = client.cache.read().await;
        assert!(cache.manifests.contains_key("library/bolt@latest"));
    }

    #[tokio::test]
    async fn download_config_prefers_cached_object_store() {
        let client = test_registry_client(Some(Arc::new(TestObjectStore::new(
            None,
            Some(b"cached-config".to_vec()),
        ))));
        let temp = tempdir().expect("tempdir");
        let dest = temp.path().join("config.json");

        client
            .download_config_to("library/bolt", "sha256:deadbeef", &dest)
            .await
            .expect("download config");

        let contents = tokio::fs::read(&dest).await.expect("read cached config");
        assert_eq!(contents, b"cached-config");
    }
}
