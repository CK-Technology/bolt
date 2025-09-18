use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::runtime::storage::ghostbay::GhostbayClient;

/// Enhanced Drift registry integration for Bolt ecosystem
/// Provides seamless package management across Drift, Ghostbay, and GhostWire
#[derive(Debug, Clone)]
pub struct DriftRegistryClient {
    pub endpoint: String,
    pub client: Client,
    pub ghostbay_client: Option<GhostbayClient>,
    pub cache: Arc<RwLock<PackageCache>>,
    pub features: DriftFeatures,
    pub gaming_config: GamingPackageConfig,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    pub schema_version: u32,
    pub media_type: String,
    pub config: BlobDescriptor,
    pub layers: Vec<LayerDescriptor>,
    pub annotations: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobDescriptor {
    pub media_type: String,
    pub size: u64,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerDescriptor {
    pub media_type: String,
    pub size: u64,
    pub digest: String,
    pub urls: Option<Vec<String>>,
    pub annotations: Option<HashMap<String, String>>,

    // Bolt-specific layer metadata
    pub gaming_assets: bool,
    pub system_libraries: bool,
    pub user_data: bool,
    pub cacheable: bool,
}

impl DriftRegistryClient {
    /// Create a new Drift registry client with Ghostbay integration
    pub async fn new(endpoint: String, ghostbay_client: Option<GhostbayClient>) -> Result<Self> {
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
            ghostbay_client,
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
        })
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

        // Pull from registry with Ghostbay optimization
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

    /// Pull package from registry with Ghostbay optimization
    async fn pull_from_registry(&self, package_ref: &str) -> Result<String> {
        debug!("🌊 Pulling from registry: {}", package_ref);

        // Get package manifest
        let manifest = self.get_package_manifest(package_ref).await?;

        // If Ghostbay is available, use optimized download
        if let Some(ref ghostbay) = self.ghostbay_client {
            return self
                .pull_via_ghostbay(package_ref, &manifest, ghostbay)
                .await;
        }

        // Standard registry pull
        self.pull_standard(package_ref, &manifest).await
    }

    /// Optimized pull using Ghostbay storage
    async fn pull_via_ghostbay(
        &self,
        package_ref: &str,
        manifest: &PackageManifest,
        ghostbay: &GhostbayClient,
    ) -> Result<String> {
        debug!("👻 Using Ghostbay optimized pull");

        // Check if layers are already cached in Ghostbay
        let mut cached_layers = Vec::new();
        let mut missing_layers = Vec::new();

        for layer in &manifest.layers {
            if ghostbay.blob_exists(&layer.digest).await? {
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
            self.download_missing_layers(&missing_layers).await?;
        }

        // Assemble final image
        let image_path = self
            .assemble_image_from_layers(package_ref, &manifest.layers)
            .await?;

        Ok(image_path)
    }

    /// Standard registry pull without Ghostbay
    async fn pull_standard(&self, package_ref: &str, manifest: &PackageManifest) -> Result<String> {
        debug!("📦 Standard registry pull");

        // Download all layers
        for layer in &manifest.layers {
            self.download_layer(&layer.digest).await?;
        }

        // Assemble image
        let image_path = self
            .assemble_image_from_layers(package_ref, &manifest.layers)
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

        // Upload layers to registry (and Ghostbay if available)
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

    async fn get_package_manifest(&self, _package_ref: &str) -> Result<PackageManifest> {
        // Fetch manifest from registry
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

    async fn share_via_p2p(&self, _package_ref: &str, _path: &str) -> Result<()> {
        // Share package via P2P mesh
        Ok(())
    }

    async fn download_missing_layers(&self, _layers: &[&LayerDescriptor]) -> Result<()> {
        // Download layers that aren't in Ghostbay
        Ok(())
    }

    async fn assemble_image_from_layers(
        &self,
        _package_ref: &str,
        _layers: &[LayerDescriptor],
    ) -> Result<String> {
        // Assemble final container image
        Ok("/var/lib/bolt/images/package".to_string())
    }

    async fn download_layer(&self, _digest: &str) -> Result<()> {
        // Download individual layer
        Ok(())
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
        // Upload layers to registry and Ghostbay
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
