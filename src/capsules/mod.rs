use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, debug, warn, error};
use uuid::Uuid;

pub mod vm;
pub mod snapshots;
pub mod templates;

use crate::runtime::oci::ContainerConfig;

/// Bolt Capsules - Our revolutionary container-VM hybrid
///
/// Capsules provide:
/// 1. VM-like isolation but container speed
/// 2. Live migration capabilities
/// 3. Instant snapshots for game saves
/// 4. Gaming-optimized resource allocation
/// 5. Template system for common environments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleManager {
    pub root_path: PathBuf,
    pub capsules: HashMap<String, CapsuleState>,
    pub templates: HashMap<String, CapsuleTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleState {
    pub id: String,
    pub name: String,
    pub capsule_type: CapsuleType,
    pub status: CapsuleStatus,
    pub config: CapsuleConfig,
    pub runtime_info: CapsuleRuntimeInfo,
    pub snapshots: Vec<SnapshotMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CapsuleType {
    /// Standard lightweight container-like capsule
    Standard,
    /// Gaming-optimized capsule with GPU/audio passthrough
    Gaming,
    /// Development capsule with toolchain and debugging
    Development,
    /// Database capsule with persistent storage optimizations
    Database,
    /// Network service capsule with advanced networking
    NetworkService,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CapsuleStatus {
    Created,
    Starting,
    Running,
    Paused,
    Migrating,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleConfig {
    pub template: Option<String>,
    pub image: String,
    pub resources: CapsuleResources,
    pub networking: CapsuleNetworking,
    pub storage: CapsuleStorage,
    pub security: CapsuleSecurity,
    pub gaming: Option<GamingCapsuleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleResources {
    pub memory_mb: u64,
    pub vcpus: u32,
    pub cpu_shares: u32,
    pub memory_balloon: bool,    // Dynamic memory allocation
    pub cpu_hotplug: bool,       // Hot-add/remove CPUs
    pub numa_topology: Option<NumaConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumaConfig {
    pub nodes: u32,
    pub memory_per_node_mb: u64,
    pub cpu_affinity: Vec<Vec<u32>>, // CPU sets per NUMA node
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleNetworking {
    pub network_type: NetworkType,
    pub interfaces: Vec<NetworkInterface>,
    pub dns_config: DnsConfig,
    pub firewall_rules: Vec<FirewallRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkType {
    Bridge,
    Host,
    QuicFabric,     // Our advanced QUIC-based networking
    IsolatedVPN,    // Completely isolated network
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub mac_address: String,
    pub ip_address: Option<String>,
    pub bandwidth_limit: Option<u64>, // Mbps
    pub latency_priority: bool,       // Gaming mode
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    pub servers: Vec<String>,
    pub search_domains: Vec<String>,
    pub bolt_dns_enabled: bool, // Use our service discovery
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallRule {
    pub action: String,  // ALLOW, DENY, DROP
    pub protocol: String, // TCP, UDP, QUIC
    pub port_range: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleStorage {
    pub root_disk: DiskConfig,
    pub data_disks: Vec<DiskConfig>,
    pub shared_folders: Vec<SharedFolder>,
    pub snapshot_policy: SnapshotPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskConfig {
    pub name: String,
    pub size_gb: u64,
    pub disk_type: DiskType,
    pub encryption: bool,
    pub compression: bool,
    pub cache_policy: CachePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiskType {
    SSD,
    NVMe,
    HDD,
    Memory,      // RAM disk for ultra-fast I/O
    Network,     // Network-attached storage
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CachePolicy {
    WriteThrough,
    WriteBack,
    DirectSync,
    Gaming,      // Optimized for gaming workloads
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFolder {
    pub host_path: String,
    pub capsule_path: String,
    pub readonly: bool,
    pub auto_mount: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotPolicy {
    pub auto_snapshot: bool,
    pub interval_minutes: u32,
    pub max_snapshots: u32,
    pub compress_snapshots: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleSecurity {
    pub isolation_level: IsolationLevel,
    pub privilege_mode: PrivilegeMode,
    pub allowed_syscalls: Vec<String>,
    pub device_permissions: Vec<DevicePermission>,
    pub mandatory_access_control: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IsolationLevel {
    Container,   // Container-like isolation
    LightVM,     // Light virtual machine isolation
    FullVM,      // Full virtual machine isolation
    Gaming,      // Gaming-optimized isolation
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrivilegeMode {
    Unprivileged,
    Privileged,
    Gaming,      // Special gaming privileges
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicePermission {
    pub device_path: String,
    pub permissions: String, // r, w, x
    pub device_type: String, // gpu, audio, input, storage
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingCapsuleConfig {
    pub gpu_passthrough: bool,
    pub audio_passthrough: bool,
    pub input_devices: Vec<String>,
    pub display_server: DisplayServer,
    pub performance_mode: PerformanceMode,
    pub anti_cheat_compat: bool,
    pub steam_integration: bool,
    pub wine_config: Option<WineConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisplayServer {
    X11,
    Wayland,
    Both,
    Headless,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceMode {
    PowerSaver,
    Balanced,
    Performance,
    Gaming,
    Competitive, // Ultra-low latency for esports
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WineConfig {
    pub version: String,
    pub prefix_path: String,
    pub windows_version: String,
    pub dxvk_enabled: bool,
    pub esync_enabled: bool,
    pub fsync_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleRuntimeInfo {
    pub pid: Option<u32>,
    pub vm_id: Option<u32>,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub uptime_seconds: u64,
    pub memory_usage_mb: u64,
    pub cpu_usage_percent: f64,
    pub network_stats: NetworkStats,
    pub migration_state: Option<MigrationState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationState {
    pub target_host: String,
    pub progress_percent: f32,
    pub estimated_completion: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub id: String,
    pub name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub size_bytes: u64,
    pub description: String,
    pub capsule_state: CapsuleStatus,
    pub memory_included: bool,
    pub parent_snapshot: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleTemplate {
    pub name: String,
    pub description: String,
    pub capsule_type: CapsuleType,
    pub base_config: CapsuleConfig,
    pub initialization_scripts: Vec<String>,
    pub required_capabilities: Vec<String>,
}

impl CapsuleManager {
    pub fn new(root_path: PathBuf) -> Result<Self> {
        info!("🔧 Initializing Bolt Capsule Manager at: {:?}", root_path);

        std::fs::create_dir_all(&root_path)
            .context("Failed to create capsules root directory")?;

        // Create capsule subdirectories
        let dirs = ["instances", "templates", "snapshots", "images", "networks"];
        for dir in &dirs {
            std::fs::create_dir_all(root_path.join(dir))
                .with_context(|| format!("Failed to create capsules/{} directory", dir))?;
        }

        let mut manager = Self {
            root_path,
            capsules: HashMap::new(),
            templates: HashMap::new(),
        };

        // Load built-in templates
        manager.initialize_builtin_templates()?;

        Ok(manager)
    }

    pub async fn create_capsule(
        &mut self,
        capsule_name: &str,
        container_config: &ContainerConfig,
    ) -> Result<String> {
        info!("🔧 Creating Bolt Capsule: {}", capsule_name);

        let capsule_id = Uuid::new_v4().to_string();

        // Determine capsule type based on configuration
        let capsule_type = self.determine_capsule_type(container_config);

        info!("Capsule type determined: {:?}", capsule_type);

        // Convert container config to capsule config
        let capsule_config = self.convert_to_capsule_config(container_config, &capsule_type)?;

        // Create capsule state
        let capsule_state = CapsuleState {
            id: capsule_id.clone(),
            name: capsule_name.to_string(),
            capsule_type,
            status: CapsuleStatus::Created,
            config: capsule_config,
            runtime_info: CapsuleRuntimeInfo {
                pid: None,
                vm_id: None,
                start_time: chrono::Utc::now(),
                uptime_seconds: 0,
                memory_usage_mb: 0,
                cpu_usage_percent: 0.0,
                network_stats: NetworkStats {
                    bytes_sent: 0,
                    bytes_received: 0,
                    packets_sent: 0,
                    packets_received: 0,
                    latency_ms: 0.0,
                },
                migration_state: None,
            },
            snapshots: Vec::new(),
        };

        // Start the capsule
        self.start_capsule(&capsule_state).await?;

        // Store capsule state
        self.capsules.insert(capsule_id.clone(), capsule_state);

        info!("✅ Bolt Capsule {} created and started", capsule_id);
        Ok(capsule_id)
    }

    fn determine_capsule_type(&self, config: &ContainerConfig) -> CapsuleType {
        if config.gaming_config.is_some() {
            return CapsuleType::Gaming;
        }

        // Check for development tools
        for env_var in &config.args {
            if env_var.contains("rust") || env_var.contains("cargo") ||
               env_var.contains("gcc") || env_var.contains("npm") {
                return CapsuleType::Development;
            }
        }

        // Check for database
        if config.image.contains("postgres") || config.image.contains("mysql") ||
           config.image.contains("redis") || config.image.contains("mongo") {
            return CapsuleType::Database;
        }

        // Check for network services
        if config.image.contains("nginx") || config.image.contains("haproxy") ||
           config.image.contains("envoy") {
            return CapsuleType::NetworkService;
        }

        CapsuleType::Standard
    }

    fn convert_to_capsule_config(
        &self,
        container_config: &ContainerConfig,
        capsule_type: &CapsuleType,
    ) -> Result<CapsuleConfig> {
        let resources = match capsule_type {
            CapsuleType::Gaming => CapsuleResources {
                memory_mb: 8192,  // 8GB for gaming
                vcpus: 4,
                cpu_shares: 2048, // High priority
                memory_balloon: false, // Stable memory for gaming
                cpu_hotplug: false,
                numa_topology: None,
            },
            CapsuleType::Development => CapsuleResources {
                memory_mb: 4096,  // 4GB for development
                vcpus: 2,
                cpu_shares: 1024,
                memory_balloon: true, // Dynamic memory
                cpu_hotplug: true,
                numa_topology: None,
            },
            CapsuleType::Database => CapsuleResources {
                memory_mb: 2048,  // 2GB for database
                vcpus: 2,
                cpu_shares: 1536, // Higher priority for DB
                memory_balloon: false, // Stable memory for DB
                cpu_hotplug: false,
                numa_topology: None,
            },
            _ => CapsuleResources {
                memory_mb: 1024,  // 1GB default
                vcpus: 1,
                cpu_shares: 1024,
                memory_balloon: true,
                cpu_hotplug: false,
                numa_topology: None,
            },
        };

        let networking = CapsuleNetworking {
            network_type: if capsule_type == &CapsuleType::Gaming {
                NetworkType::QuicFabric // Gaming gets QUIC
            } else {
                NetworkType::Bridge
            },
            interfaces: vec![NetworkInterface {
                name: "eth0".to_string(),
                mac_address: self.generate_mac_address(),
                ip_address: None,
                bandwidth_limit: None,
                latency_priority: matches!(capsule_type, CapsuleType::Gaming),
            }],
            dns_config: DnsConfig {
                servers: vec!["8.8.8.8".to_string(), "1.1.1.1".to_string()],
                search_domains: vec!["bolt.local".to_string()],
                bolt_dns_enabled: true,
            },
            firewall_rules: Vec::new(),
        };

        let storage = CapsuleStorage {
            root_disk: DiskConfig {
                name: "root".to_string(),
                size_gb: match capsule_type {
                    CapsuleType::Gaming => 100,     // 100GB for games
                    CapsuleType::Development => 50,  // 50GB for dev tools
                    CapsuleType::Database => 20,     // 20GB for database
                    _ => 10,                          // 10GB default
                },
                disk_type: if matches!(capsule_type, CapsuleType::Gaming) {
                    DiskType::NVMe  // Fast storage for gaming
                } else {
                    DiskType::SSD
                },
                encryption: true,
                compression: !matches!(capsule_type, CapsuleType::Gaming), // No compression for gaming
                cache_policy: match capsule_type {
                    CapsuleType::Gaming => CachePolicy::Gaming,
                    _ => CachePolicy::WriteBack,
                },
            },
            data_disks: Vec::new(),
            shared_folders: Vec::new(),
            snapshot_policy: SnapshotPolicy {
                auto_snapshot: matches!(capsule_type, CapsuleType::Gaming | CapsuleType::Database),
                interval_minutes: if matches!(capsule_type, CapsuleType::Gaming) { 30 } else { 60 },
                max_snapshots: 10,
                compress_snapshots: !matches!(capsule_type, CapsuleType::Gaming),
            },
        };

        let security = CapsuleSecurity {
            isolation_level: match capsule_type {
                CapsuleType::Gaming => IsolationLevel::Gaming,
                CapsuleType::Database => IsolationLevel::LightVM,
                _ => IsolationLevel::Container,
            },
            privilege_mode: if matches!(capsule_type, CapsuleType::Gaming) {
                PrivilegeMode::Gaming
            } else {
                PrivilegeMode::Unprivileged
            },
            allowed_syscalls: Vec::new(),
            device_permissions: if matches!(capsule_type, CapsuleType::Gaming) {
                vec![
                    DevicePermission {
                        device_path: "/dev/dri".to_string(),
                        permissions: "rw".to_string(),
                        device_type: "gpu".to_string(),
                    },
                    DevicePermission {
                        device_path: "/dev/snd".to_string(),
                        permissions: "rw".to_string(),
                        device_type: "audio".to_string(),
                    },
                ]
            } else {
                Vec::new()
            },
            mandatory_access_control: false,
        };

        let gaming_config = container_config.gaming_config.as_ref().map(|gc| {
            GamingCapsuleConfig {
                gpu_passthrough: gc.gpu.is_some(),
                audio_passthrough: gc.audio.is_some(),
                input_devices: vec!["/dev/input".to_string()],
                display_server: DisplayServer::Both,
                performance_mode: PerformanceMode::Gaming,
                anti_cheat_compat: true,
                steam_integration: true,
                wine_config: gc.wine.as_ref().map(|w| WineConfig {
                    version: w.version.clone().unwrap_or_else(|| "latest".to_string()),
                    prefix_path: w.prefix.clone().unwrap_or_else(|| "/wine".to_string()),
                    windows_version: w.winver.clone().unwrap_or_else(|| "win10".to_string()),
                    dxvk_enabled: true,
                    esync_enabled: true,
                    fsync_enabled: true,
                }),
            }
        });

        Ok(CapsuleConfig {
            template: None,
            image: container_config.image.clone(),
            resources,
            networking,
            storage,
            security,
            gaming: gaming_config,
        })
    }

    async fn start_capsule(&self, capsule_state: &CapsuleState) -> Result<()> {
        info!("🚀 Starting Bolt Capsule: {}", capsule_state.name);

        match capsule_state.capsule_type {
            CapsuleType::Gaming => {
                self.start_gaming_capsule(capsule_state).await?;
            }
            CapsuleType::Development => {
                self.start_development_capsule(capsule_state).await?;
            }
            CapsuleType::Database => {
                self.start_database_capsule(capsule_state).await?;
            }
            _ => {
                self.start_standard_capsule(capsule_state).await?;
            }
        }

        Ok(())
    }

    async fn start_gaming_capsule(&self, capsule_state: &CapsuleState) -> Result<()> {
        info!("🎮 Starting Gaming Capsule with optimizations");

        // Gaming capsules get special treatment:
        // 1. GPU passthrough setup
        // 2. Audio passthrough
        // 3. Ultra-low latency networking
        // 4. Performance CPU scheduling
        // 5. Anti-cheat compatibility

        if let Some(ref gaming) = capsule_state.config.gaming {
            if gaming.gpu_passthrough {
                self.setup_gpu_passthrough(&capsule_state.id).await?;
            }

            if gaming.audio_passthrough {
                self.setup_audio_passthrough(&capsule_state.id).await?;
            }

            self.setup_gaming_performance(&capsule_state.id).await?;
        }

        warn!("Gaming capsule implementation pending");
        Ok(())
    }

    async fn start_development_capsule(&self, _capsule_state: &CapsuleState) -> Result<()> {
        info!("💻 Starting Development Capsule");
        // TODO: Setup development environment
        Ok(())
    }

    async fn start_database_capsule(&self, _capsule_state: &CapsuleState) -> Result<()> {
        info!("🗄️  Starting Database Capsule");
        // TODO: Setup database optimizations
        Ok(())
    }

    async fn start_standard_capsule(&self, _capsule_state: &CapsuleState) -> Result<()> {
        info!("📦 Starting Standard Capsule");
        // TODO: Setup standard capsule
        Ok(())
    }

    async fn setup_gpu_passthrough(&self, capsule_id: &str) -> Result<()> {
        info!("🖥️  Setting up GPU passthrough for capsule: {}", capsule_id);
        // TODO: Implement GPU device passthrough
        Ok(())
    }

    async fn setup_audio_passthrough(&self, capsule_id: &str) -> Result<()> {
        info!("🔊 Setting up audio passthrough for capsule: {}", capsule_id);
        // TODO: Implement audio device passthrough
        Ok(())
    }

    async fn setup_gaming_performance(&self, capsule_id: &str) -> Result<()> {
        info!("⚡ Setting up gaming performance for capsule: {}", capsule_id);
        // TODO: Implement gaming performance optimizations
        Ok(())
    }

    fn initialize_builtin_templates(&mut self) -> Result<()> {
        info!("📋 Initializing built-in capsule templates");

        // Gaming template
        let gaming_template = CapsuleTemplate {
            name: "gaming".to_string(),
            description: "Gaming-optimized capsule with GPU/audio passthrough".to_string(),
            capsule_type: CapsuleType::Gaming,
            base_config: CapsuleConfig {
                template: None,
                image: "bolt://gaming-base:latest".to_string(),
                resources: CapsuleResources {
                    memory_mb: 8192,
                    vcpus: 4,
                    cpu_shares: 2048,
                    memory_balloon: false,
                    cpu_hotplug: false,
                    numa_topology: None,
                },
                networking: CapsuleNetworking {
                    network_type: NetworkType::QuicFabric,
                    interfaces: vec![],
                    dns_config: DnsConfig {
                        servers: vec!["1.1.1.1".to_string()],
                        search_domains: vec![],
                        bolt_dns_enabled: true,
                    },
                    firewall_rules: vec![],
                },
                storage: CapsuleStorage {
                    root_disk: DiskConfig {
                        name: "root".to_string(),
                        size_gb: 100,
                        disk_type: DiskType::NVMe,
                        encryption: true,
                        compression: false,
                        cache_policy: CachePolicy::Gaming,
                    },
                    data_disks: vec![],
                    shared_folders: vec![],
                    snapshot_policy: SnapshotPolicy {
                        auto_snapshot: true,
                        interval_minutes: 30,
                        max_snapshots: 10,
                        compress_snapshots: false,
                    },
                },
                security: CapsuleSecurity {
                    isolation_level: IsolationLevel::Gaming,
                    privilege_mode: PrivilegeMode::Gaming,
                    allowed_syscalls: vec![],
                    device_permissions: vec![],
                    mandatory_access_control: false,
                },
                gaming: Some(GamingCapsuleConfig {
                    gpu_passthrough: true,
                    audio_passthrough: true,
                    input_devices: vec!["/dev/input".to_string()],
                    display_server: DisplayServer::Both,
                    performance_mode: PerformanceMode::Gaming,
                    anti_cheat_compat: true,
                    steam_integration: true,
                    wine_config: None,
                }),
            },
            initialization_scripts: vec![],
            required_capabilities: vec!["GPU".to_string(), "AUDIO".to_string()],
        };

        self.templates.insert("gaming".to_string(), gaming_template);

        info!("✅ Built-in templates loaded");
        Ok(())
    }

    fn generate_mac_address(&self) -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        format!(
            "02:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            rng.r#gen::<u8>(),
            rng.r#gen::<u8>(),
            rng.r#gen::<u8>(),
            rng.r#gen::<u8>(),
            rng.r#gen::<u8>()
        )
    }

    pub fn list_capsules(&self) -> Vec<&CapsuleState> {
        self.capsules.values().collect()
    }

    pub fn get_capsule(&self, capsule_id: &str) -> Option<&CapsuleState> {
        self.capsules.get(capsule_id)
    }
}