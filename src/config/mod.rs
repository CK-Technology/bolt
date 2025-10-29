use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

pub mod user;
pub use user::UserConfig;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoltFile {
    pub project: String,
    pub services: HashMap<String, Service>,
    pub networks: Option<HashMap<String, Network>>,
    pub volumes: Option<HashMap<String, Volume>>,
    pub snapshots: Option<SnapshotConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Service {
    pub image: Option<String>,
    pub build: Option<String>,
    pub capsule: Option<String>,
    pub command: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub environment: Option<HashMap<String, String>>,
    pub env: Option<HashMap<String, String>>,
    pub depends_on: Option<Vec<String>>,
    pub restart: Option<String>,
    pub networks: Option<Vec<String>>,
    pub storage: Option<Storage>,
    pub auth: Option<Auth>,
    pub gaming: Option<GamingConfig>,
    pub working_dir: Option<String>,
    pub user: Option<String>,
    pub hostname: Option<String>,
    pub container_name: Option<String>,
    pub privileged: Option<bool>,
    pub read_only: Option<bool>,
    pub stdin_open: Option<bool>,
    pub tty: Option<bool>,
    pub network_mode: Option<String>,
    pub pid: Option<String>,
    pub ipc: Option<String>,
    pub platform: Option<String>,
    pub labels: Option<HashMap<String, String>>,
    pub devices: Option<Vec<String>>,
    pub cap_add: Option<Vec<String>>,
    pub cap_drop: Option<Vec<String>>,
    pub security_opt: Option<Vec<String>>,
    pub sysctls: Option<HashMap<String, String>>,
    pub tmpfs: Option<Vec<String>>,
    pub dns: Option<Vec<String>>,
    pub dns_search: Option<Vec<String>>,
    pub extra_hosts: Option<Vec<String>>,
    pub group_add: Option<Vec<String>>,
    pub volumes_from: Option<Vec<String>>,
    pub links: Option<Vec<String>>,
    pub logging: Option<LoggingConfig>,
    pub healthcheck: Option<HealthcheckConfig>,
    pub cpu_limit: Option<String>,
    pub memory_limit: Option<String>,
    pub mcp: Option<ServiceMcpConfig>,
}

pub type NetworkConfig = Network;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Network {
    pub driver: String,
    pub driver_opts: Option<HashMap<String, String>>,
    pub attachable: Option<bool>,
    pub enable_ipv6: Option<bool>,
    pub internal: Option<bool>,
    pub labels: Option<HashMap<String, String>>,
    pub ipam: Option<IpamConfig>,
    pub external: Option<bool>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IpamConfig {
    pub driver: Option<String>,
    pub config: Option<Vec<IpamSubnetConfig>>,
    pub options: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IpamSubnetConfig {
    pub subnet: Option<String>,
    pub ip_range: Option<String>,
    pub gateway: Option<String>,
    pub aux_addresses: Option<HashMap<String, String>>,
}

pub type VolumeConfig = Volume;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Volume {
    pub driver: Option<String>,
    pub driver_opts: Option<HashMap<String, String>>,
    pub external: Option<bool>,
    pub labels: Option<HashMap<String, String>>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Storage {
    pub size: String,
    pub driver: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Auth {
    pub user: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GamingConfig {
    pub enabled: bool,
    pub gpu_passthrough: bool,
    pub nvidia_runtime: bool,
    pub amd_runtime: bool,
    pub audio_passthrough: bool,
    pub real_time_priority: bool,
    pub wine_prefix: Option<String>,
    pub proton_version: Option<String>,
    pub dxvk_enabled: Option<bool>,
    pub esync_enabled: Option<bool>,
    pub fsync_enabled: Option<bool>,
    pub performance_profile: Option<String>,
    pub input_devices: Option<Vec<String>>,
    pub display_driver: Option<String>,
    pub resolution: Option<String>,
    pub refresh_rate: Option<u32>,
    pub vsync: Option<bool>,
    pub gpu: Option<GpuConfig>,
    pub audio: Option<AudioConfig>,
    pub wine: Option<WineConfig>,
    pub performance: Option<PerformanceConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    pub driver: String,
    pub options: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthcheckConfig {
    pub test: Vec<String>,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    pub start_period: Option<String>,
    pub disable: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GpuConfig {
    pub runtime: Option<String>, // "nvbind", "docker", "nvidia", "amd"
    pub nvidia: Option<NvidiaConfig>,
    pub amd: Option<AmdConfig>,
    pub nvbind: Option<NvbindConfig>,
    pub passthrough: Option<bool>,
    pub isolation_level: Option<String>, // "shared", "exclusive", "virtual"
    pub memory_limit: Option<String>,    // e.g., "8GB"
    pub gaming: Option<GpuGamingConfig>, // nvbind gaming optimizations
    pub aiml: Option<GpuAiMlConfig>,     // nvbind AI/ML optimizations
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NvidiaConfig {
    pub device: Option<u32>,
    pub dlss: Option<bool>,
    pub reflex: Option<bool>,
    pub raytracing: Option<bool>,
    pub cuda: Option<bool>,
    pub power_limit: Option<u32>,
    pub memory_clock_offset: Option<i32>,
    pub core_clock_offset: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AmdConfig {
    pub device: Option<u32>,
    pub rocm: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NvbindConfig {
    pub driver: Option<String>, // "auto", "nvidia-open", "proprietary", "nouveau"
    pub devices: Option<Vec<String>>, // e.g., ["gpu:0"], ["gpu:all"]
    pub wsl2_optimized: Option<bool>, // Enable WSL2 optimizations
    pub performance_mode: Option<String>, // "ultra", "high", "balanced", "efficient"
    pub preload_libraries: Option<bool>, // Preload GPU libraries
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GpuGamingConfig {
    pub profile: Option<String>, // "ultra-low-latency", "performance", "balanced"
    pub dlss_enabled: Option<bool>,
    pub rt_cores_enabled: Option<bool>,
    pub wine_optimizations: Option<bool>,
    pub vrs_enabled: Option<bool>, // Variable Rate Shading
    pub performance_profile: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GpuAiMlConfig {
    pub profile: Option<String>,   // "training", "inference", "development"
    pub mig_enabled: Option<bool>, // Multi-Instance GPU
    pub tensor_cores_enabled: Option<bool>,
    pub mixed_precision: Option<bool>,
    pub cuda_cache_size: Option<u32>,     // CUDA cache size in MB
    pub memory_pool_size: Option<String>, // e.g., "16GB"
}

// MCP (Model Context Protocol) Configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceMcpConfig {
    /// Enable MCP for this service
    #[serde(default)]
    pub enabled: bool,

    /// Enabled tools (e.g., ["filesystem:read", "shell:exec", "gpu:stats"])
    #[serde(default)]
    pub tools: Vec<String>,

    /// Tool-specific permissions
    #[serde(default)]
    pub permissions: Option<McpPermissions>,

    /// Omen AI Router configuration (optional)
    #[cfg(feature = "omen")]
    #[serde(default)]
    pub omen: Option<McpOmenConfig>,
}

impl ServiceMcpConfig {
    /// Valid MCP tool names
    const VALID_TOOLS: &'static [&'static str] = &[
        "filesystem:read",
        "filesystem:write",
        "filesystem:list",
        "filesystem:watch",
        "shell:exec",
        "gpu:stats",
        "gpu:info",
        "process:list",
        "process:kill",
        "network:stats",
        "network:connections",
    ];

    /// Validate the MCP configuration
    pub fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(()); // No validation needed if MCP is disabled
        }

        // Validate tool names
        for tool in &self.tools {
            if !Self::VALID_TOOLS.contains(&tool.as_str()) {
                return Err(anyhow::anyhow!(
                    "Invalid MCP tool '{}'. Valid tools: {:?}",
                    tool,
                    Self::VALID_TOOLS
                ));
            }
        }

        // Validate permissions if specified
        if let Some(ref perms) = self.permissions {
            perms.validate()?;
        }

        // Validate Omen config if specified
        #[cfg(feature = "omen")]
        if let Some(ref omen) = self.omen {
            omen.validate()?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpPermissions {
    /// Allowed filesystem paths for filesystem tools
    #[serde(default)]
    pub filesystem_paths: Vec<String>,

    /// Allowed shell commands for shell execution
    #[serde(default)]
    pub shell_commands: Vec<String>,

    /// GPU access level: "read_only", "read_write", "full"
    #[serde(default)]
    pub gpu_access: Option<String>,

    /// Network access level: "none", "read_only", "full"
    #[serde(default)]
    pub network_access: Option<String>,

    /// Process access level: "read_only", "read_write", "full"
    #[serde(default)]
    pub process_access: Option<String>,
}

impl McpPermissions {
    /// Valid access levels
    const VALID_ACCESS_LEVELS: &'static [&'static str] = &["none", "read_only", "read_write", "full"];

    /// Validate permissions configuration
    pub fn validate(&self) -> Result<()> {
        // Validate filesystem paths
        for path in &self.filesystem_paths {
            if path.is_empty() {
                return Err(anyhow::anyhow!("Filesystem path cannot be empty"));
            }
            // Ensure paths are absolute or relative
            if !path.starts_with('/') && !path.starts_with("./") && !path.starts_with("../") {
                return Err(anyhow::anyhow!(
                    "Filesystem path '{}' must be absolute or relative (start with /, ./, or ../)",
                    path
                ));
            }
        }

        // Validate shell commands
        for cmd in &self.shell_commands {
            if cmd.is_empty() {
                return Err(anyhow::anyhow!("Shell command cannot be empty"));
            }
            if cmd.contains("..") || cmd.contains("&&") || cmd.contains(";") {
                return Err(anyhow::anyhow!(
                    "Shell command '{}' contains potentially unsafe characters",
                    cmd
                ));
            }
        }

        // Validate access levels
        if let Some(ref level) = self.gpu_access {
            if !Self::VALID_ACCESS_LEVELS.contains(&level.as_str()) {
                return Err(anyhow::anyhow!(
                    "Invalid GPU access level '{}'. Valid levels: {:?}",
                    level,
                    Self::VALID_ACCESS_LEVELS
                ));
            }
        }

        if let Some(ref level) = self.network_access {
            if !Self::VALID_ACCESS_LEVELS.contains(&level.as_str()) {
                return Err(anyhow::anyhow!(
                    "Invalid network access level '{}'. Valid levels: {:?}",
                    level,
                    Self::VALID_ACCESS_LEVELS
                ));
            }
        }

        if let Some(ref level) = self.process_access {
            if !Self::VALID_ACCESS_LEVELS.contains(&level.as_str()) {
                return Err(anyhow::anyhow!(
                    "Invalid process access level '{}'. Valid levels: {:?}",
                    level,
                    Self::VALID_ACCESS_LEVELS
                ));
            }
        }

        Ok(())
    }
}

#[cfg(feature = "omen")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpOmenConfig {
    /// Enable Omen AI routing for this service
    #[serde(default)]
    pub enabled: bool,

    /// Routing strategy: "cost_optimized", "latency_optimized", "balanced", "quality_optimized"
    #[serde(default)]
    pub strategy: String,

    /// Maximum cost per hour in USD
    #[serde(default)]
    pub max_cost_per_hour: Option<f64>,

    /// Preferred providers (e.g., ["ollama", "anthropic"])
    #[serde(default)]
    pub providers: Vec<String>,

    /// Provider-specific configuration
    #[serde(default)]
    pub provider_config: HashMap<String, String>,
}

#[cfg(feature = "omen")]
impl McpOmenConfig {
    /// Valid routing strategies
    const VALID_STRATEGIES: &'static [&'static str] = &[
        "cost_optimized",
        "latency_optimized",
        "balanced",
        "quality_optimized",
    ];

    /// Valid AI providers
    const VALID_PROVIDERS: &'static [&'static str] = &[
        "ollama",
        "anthropic",
        "openai",
        "google",
        "xai",
        "azure",
        "bedrock",
        "vertexai",
    ];

    /// Validate Omen configuration
    pub fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(()); // No validation needed if Omen is disabled
        }

        // Validate routing strategy
        if !self.strategy.is_empty() && !Self::VALID_STRATEGIES.contains(&self.strategy.as_str()) {
            return Err(anyhow::anyhow!(
                "Invalid Omen routing strategy '{}'. Valid strategies: {:?}",
                self.strategy,
                Self::VALID_STRATEGIES
            ));
        }

        // Validate max_cost_per_hour
        if let Some(cost) = self.max_cost_per_hour {
            if cost < 0.0 {
                return Err(anyhow::anyhow!(
                    "max_cost_per_hour must be >= 0, got: {}",
                    cost
                ));
            }
            if cost > 1000.0 {
                return Err(anyhow::anyhow!(
                    "max_cost_per_hour seems unreasonably high: {}. Please check your configuration.",
                    cost
                ));
            }
        }

        // Validate providers
        for provider in &self.providers {
            if !Self::VALID_PROVIDERS.contains(&provider.as_str()) {
                return Err(anyhow::anyhow!(
                    "Invalid Omen provider '{}'. Valid providers: {:?}",
                    provider,
                    Self::VALID_PROVIDERS
                ));
            }
        }

        Ok(())
    }
}

// Snapshot Configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SnapshotConfig {
    pub enabled: Option<bool>,                       // Enable/disable snapshots
    pub filesystem: Option<String>,                  // "btrfs", "zfs", "auto"
    pub root_path: Option<String>,                   // Path to snapshot (default: "/")
    pub snapshot_path: Option<String>,               // Where to store snapshots
    pub retention: Option<RetentionPolicy>,          // Retention settings
    pub triggers: Option<SnapshotTriggers>,          // When to take snapshots
    pub named_snapshots: Option<Vec<NamedSnapshot>>, // Pre-defined snapshots
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetentionPolicy {
    pub keep_hourly: Option<u32>,          // Keep N hourly snapshots
    pub keep_daily: Option<u32>,           // Keep N daily snapshots
    pub keep_weekly: Option<u32>,          // Keep N weekly snapshots
    pub keep_monthly: Option<u32>,         // Keep N monthly snapshots
    pub keep_yearly: Option<u32>,          // Keep N yearly snapshots
    pub max_total: Option<u32>,            // Maximum total snapshots
    pub cleanup_frequency: Option<String>, // "daily", "weekly", "monthly"
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SnapshotTriggers {
    // Time-based triggers
    pub hourly: Option<bool>,    // Take hourly snapshots
    pub daily: Option<String>,   // Daily at specific time (e.g. "02:00")
    pub weekly: Option<String>,  // Weekly on day+time (e.g. "sunday@02:00")
    pub monthly: Option<String>, // Monthly on day+time (e.g. "1@02:00")

    // Operation-based triggers
    pub before_container_run: Option<bool>, // Before running containers
    pub before_build: Option<bool>,         // Before building images
    pub before_surge_up: Option<bool>,      // Before surge up
    pub before_system_update: Option<bool>, // Before system updates

    // Change-based triggers
    pub on_file_changes: Option<ChangeBasedConfig>, // Monitor file changes
    pub min_change_threshold: Option<String>, // Minimum changes to trigger (e.g. "100MB", "1000 files")
    pub change_detection_interval: Option<String>, // How often to check (e.g. "5m", "1h")
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChangeBasedConfig {
    pub enabled: Option<bool>,
    pub watch_paths: Option<Vec<String>>, // Paths to monitor for changes
    pub exclude_paths: Option<Vec<String>>, // Paths to exclude from monitoring
    pub file_patterns: Option<Vec<String>>, // File patterns to monitor (e.g. "*.toml", "*.rs")
    pub exclude_patterns: Option<Vec<String>>, // Patterns to exclude (e.g. "*.tmp", "*.log")
    pub change_types: Option<Vec<String>>, // "create", "modify", "delete"
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NamedSnapshot {
    pub name: String,                // User-friendly name
    pub description: Option<String>, // Description of the snapshot
    pub trigger: Option<String>,     // When to create ("manual", "before_gaming", etc.)
    pub auto_create: Option<bool>,   // Create automatically
    pub keep_forever: Option<bool>,  // Exclude from retention cleanup
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AudioConfig {
    pub system: String, // pipewire, pulseaudio
    pub latency: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WineConfig {
    pub version: Option<String>,
    pub proton: Option<String>,
    pub winver: Option<String>,
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    pub cpu_governor: Option<String>,
    pub nice_level: Option<i32>,
    pub rt_priority: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    No,
    Always,
    OnFailure,
    UnlessStopped,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum DriverType {
    Auto,
    Docker,
    Podman,
    Containerd,
    Crun,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum GamingProfile {
    Balanced,
    HighPerformance,
    PowerSaver,
    Competitive,
    Quality,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PerformanceMode {
    Gaming,
    Balanced,
    PowerSaver,
    HighPerformance,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum IsolationLevel {
    Shared,
    Exclusive,
    Container,
    Process,
}

impl BoltFile {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read Boltfile at {:?}", path.as_ref()))?;

        let config: BoltFile =
            toml::from_str(&content).with_context(|| "Failed to parse Boltfile")?;

        // Validate the configuration
        config.validate()?;

        Ok(config)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        // Validate before saving
        self.validate()?;

        let content =
            toml::to_string_pretty(self).with_context(|| "Failed to serialize Boltfile")?;

        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write Boltfile at {:?}", path.as_ref()))?;

        Ok(())
    }

    /// Comprehensive validation of the Boltfile configuration
    pub fn validate(&self) -> Result<()> {
        info!("🔍 Validating Boltfile configuration");

        // Basic validation
        self.validate_basic()?;

        // Service validation
        self.validate_services()?;

        // Dependency validation
        self.validate_dependencies()?;

        // Port validation
        self.validate_ports()?;

        // Network validation
        if let Some(ref networks) = self.networks {
            self.validate_networks(networks)?;
        }

        // Volume validation
        if let Some(ref volumes) = self.volumes {
            self.validate_volumes(volumes)?;
        }

        info!("✅ Boltfile validation passed");
        Ok(())
    }

    fn validate_basic(&self) -> Result<()> {
        debug!("Validating basic configuration");

        if self.project.is_empty() {
            return Err(anyhow!("Project name cannot be empty"));
        }

        if self.project.contains(' ') {
            return Err(anyhow!(
                "Project name cannot contain spaces: '{}'",
                self.project
            ));
        }

        if self.services.is_empty() {
            return Err(anyhow!("At least one service must be defined"));
        }

        debug!("✅ Basic validation passed");
        Ok(())
    }

    fn validate_services(&self) -> Result<()> {
        debug!("Validating services");

        for (name, service) in &self.services {
            debug!("Validating service: {}", name);

            if name.is_empty() {
                return Err(anyhow!("Service name cannot be empty"));
            }

            if name.contains(' ') {
                return Err(anyhow!("Service name cannot contain spaces: '{}'", name));
            }

            // Service must have at least one of: image, build, or capsule
            if service.image.is_none() && service.build.is_none() && service.capsule.is_none() {
                return Err(anyhow!(
                    "Service '{}' must specify either 'image', 'build', or 'capsule'",
                    name
                ));
            }

            // Validate mutually exclusive options
            let options_count = [
                service.image.is_some(),
                service.build.is_some(),
                service.capsule.is_some(),
            ]
            .iter()
            .filter(|&&x| x)
            .count();

            if options_count > 1 {
                return Err(anyhow!(
                    "Service '{}' can only specify one of 'image', 'build', or 'capsule'",
                    name
                ));
            }

            // Validate gaming configuration
            if let Some(ref gaming) = service.gaming {
                self.validate_gaming_config(name, gaming)?;
            }

            // Validate storage configuration
            if let Some(ref storage) = service.storage {
                self.validate_storage_config(name, storage)?;
            }

            // Validate MCP configuration
            if let Some(ref mcp) = service.mcp {
                debug!("Validating MCP configuration for service: {}", name);
                mcp.validate()
                    .with_context(|| format!("Invalid MCP configuration for service '{}'", name))?;
            }

            // Validate ports
            if let Some(ref ports) = service.ports {
                self.validate_service_ports(name, ports)?;
            }

            // Validate volumes
            if let Some(ref volumes) = service.volumes {
                self.validate_service_volumes(name, volumes)?;
            }
        }

        debug!("✅ Services validation passed");
        Ok(())
    }

    fn validate_dependencies(&self) -> Result<()> {
        debug!("Validating service dependencies");

        // Check for circular dependencies
        for (name, service) in &self.services {
            if let Some(ref deps) = service.depends_on {
                self.check_circular_dependencies(name, deps, &mut HashSet::new())?;

                // Validate that all dependencies exist
                for dep in deps {
                    if !self.services.contains_key(dep) {
                        return Err(anyhow!(
                            "Service '{}' depends on non-existent service '{}'",
                            name,
                            dep
                        ));
                    }
                }
            }
        }

        debug!("✅ Dependencies validation passed");
        Ok(())
    }

    fn check_circular_dependencies(
        &self,
        service: &str,
        deps: &[String],
        visited: &mut HashSet<String>,
    ) -> Result<()> {
        if visited.contains(service) {
            return Err(anyhow!(
                "Circular dependency detected involving service '{}'",
                service
            ));
        }

        visited.insert(service.to_string());

        for dep in deps {
            if let Some(dep_service) = self.services.get(dep) {
                if let Some(ref dep_deps) = dep_service.depends_on {
                    self.check_circular_dependencies(dep, dep_deps, visited)?;
                }
            }
        }

        visited.remove(service);
        Ok(())
    }

    fn validate_ports(&self) -> Result<()> {
        debug!("Validating port conflicts");

        let mut used_host_ports = HashSet::new();

        for (_name, service) in &self.services {
            if let Some(ref ports) = service.ports {
                for port_mapping in ports {
                    let host_port = self.extract_host_port(port_mapping)?;

                    if used_host_ports.contains(&host_port) {
                        return Err(anyhow!(
                            "Port conflict: host port {} is used by multiple services",
                            host_port
                        ));
                    }

                    used_host_ports.insert(host_port);
                }
            }
        }

        debug!("✅ Port validation passed");
        Ok(())
    }

    fn extract_host_port(&self, port_mapping: &str) -> Result<u16> {
        let parts: Vec<&str> = port_mapping.split(':').collect();

        if parts.is_empty() {
            return Err(anyhow!("Invalid port mapping format: '{}'", port_mapping));
        }

        let host_port_str = parts[0];
        host_port_str
            .parse::<u16>()
            .with_context(|| format!("Invalid host port number: '{}'", host_port_str))
    }

    fn validate_service_ports(&self, service_name: &str, ports: &[String]) -> Result<()> {
        for port in ports {
            // Validate port mapping format (host:container or just port)
            if port.contains(':') {
                let parts: Vec<&str> = port.split(':').collect();
                if parts.len() != 2 {
                    return Err(anyhow!(
                        "Service '{}': invalid port mapping format '{}'",
                        service_name,
                        port
                    ));
                }

                // Validate host port
                parts[0].parse::<u16>().with_context(|| {
                    format!(
                        "Service '{}': invalid host port '{}'",
                        service_name, parts[0]
                    )
                })?;

                // Validate container port
                parts[1].parse::<u16>().with_context(|| {
                    format!(
                        "Service '{}': invalid container port '{}'",
                        service_name, parts[1]
                    )
                })?;
            } else {
                // Single port number
                port.parse::<u16>().with_context(|| {
                    format!("Service '{}': invalid port number '{}'", service_name, port)
                })?;
            }
        }

        Ok(())
    }

    fn validate_service_volumes(&self, service_name: &str, volumes: &[String]) -> Result<()> {
        for volume in volumes {
            if volume.contains(':') {
                let parts: Vec<&str> = volume.split(':').collect();
                if parts.len() < 2 || parts.len() > 3 {
                    return Err(anyhow!(
                        "Service '{}': invalid volume mapping format '{}'",
                        service_name,
                        volume
                    ));
                }

                // Validate host path (first part)
                let host_path = parts[0];
                if host_path.is_empty() {
                    return Err(anyhow!(
                        "Service '{}': empty host path in volume mapping '{}'",
                        service_name,
                        volume
                    ));
                }

                // Validate container path (second part)
                let container_path = parts[1];
                if container_path.is_empty() {
                    return Err(anyhow!(
                        "Service '{}': empty container path in volume mapping '{}'",
                        service_name,
                        volume
                    ));
                }

                // Validate options (third part, if present)
                if parts.len() == 3 {
                    let options = parts[2];
                    for option in options.split(',') {
                        match option {
                            "ro" | "rw" | "z" | "Z" => {} // Valid options
                            _ => warn!(
                                "Service '{}': unknown volume option '{}' in '{}'",
                                service_name, option, volume
                            ),
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn validate_gaming_config(&self, service_name: &str, gaming: &GamingConfig) -> Result<()> {
        debug!("Validating gaming config for service: {}", service_name);

        if let Some(ref gpu) = gaming.gpu {
            if let Some(ref nvidia) = gpu.nvidia {
                if nvidia.device.is_some() && nvidia.device.unwrap() > 7 {
                    warn!(
                        "Service '{}': NVIDIA device ID {} is unusually high",
                        service_name,
                        nvidia.device.unwrap()
                    );
                }

                // Validate power limit
                if let Some(power_limit) = nvidia.power_limit {
                    if power_limit > 150 {
                        return Err(anyhow!(
                            "Service '{}': NVIDIA power limit {}% exceeds maximum (150%)",
                            service_name,
                            power_limit
                        ));
                    }
                }

                // Validate clock offsets
                if let Some(memory_offset) = nvidia.memory_clock_offset {
                    if memory_offset.abs() > 2000 {
                        return Err(anyhow!(
                            "Service '{}': NVIDIA memory clock offset {} MHz is too extreme (max ±2000 MHz)",
                            service_name,
                            memory_offset
                        ));
                    }
                }

                if let Some(core_offset) = nvidia.core_clock_offset {
                    if core_offset.abs() > 1000 {
                        return Err(anyhow!(
                            "Service '{}': NVIDIA core clock offset {} MHz is too extreme (max ±1000 MHz)",
                            service_name,
                            core_offset
                        ));
                    }
                }
            }

            if let Some(ref amd) = gpu.amd {
                if amd.device.is_some() && amd.device.unwrap() > 7 {
                    warn!(
                        "Service '{}': AMD device ID {} is unusually high",
                        service_name,
                        amd.device.unwrap()
                    );
                }
            }
        }

        if let Some(ref audio) = gaming.audio {
            match audio.system.as_str() {
                "pipewire" | "pulseaudio" => {}
                _ => {
                    return Err(anyhow!(
                        "Service '{}': unsupported audio system '{}'",
                        service_name,
                        audio.system
                    ));
                }
            }
        }

        if let Some(ref perf) = gaming.performance {
            if let Some(nice) = perf.nice_level {
                if !(-20..=19).contains(&nice) {
                    return Err(anyhow!(
                        "Service '{}': nice level {} out of range (-20 to 19)",
                        service_name,
                        nice
                    ));
                }
            }

            if let Some(rt_prio) = perf.rt_priority {
                if rt_prio > 99 {
                    return Err(anyhow!(
                        "Service '{}': RT priority {} out of range (0 to 99)",
                        service_name,
                        rt_prio
                    ));
                }
            }
        }

        Ok(())
    }

    fn validate_storage_config(&self, service_name: &str, storage: &Storage) -> Result<()> {
        // Validate storage size format
        let size = &storage.size;
        if !size.ends_with("Gi")
            && !size.ends_with("Mi")
            && !size.ends_with("Ki")
            && !size.ends_with("G")
            && !size.ends_with("M")
            && !size.ends_with("K")
        {
            return Err(anyhow!(
                "Service '{}': invalid storage size format '{}' (use formats like '5Gi', '500Mi')",
                service_name,
                size
            ));
        }

        // Extract numeric part
        let numeric_part = size.trim_end_matches(|c: char| c.is_alphabetic());
        numeric_part.parse::<f64>().with_context(|| {
            format!(
                "Service '{}': invalid storage size number in '{}'",
                service_name, size
            )
        })?;

        Ok(())
    }

    fn validate_networks(&self, networks: &HashMap<String, Network>) -> Result<()> {
        debug!("Validating network definitions");

        for (name, network) in networks {
            if let Some(ref ipam) = network.ipam {
                if let Some(ref configs) = ipam.config {
                    for config in configs {
                        if let Some(ref subnet) = config.subnet {
                            if !subnet.contains('/') {
                                return Err(anyhow!(
                                    "Network '{}': IPAM subnet '{}' must be in CIDR notation",
                                    name,
                                    subnet
                                ));
                            }
                        }
                    }
                }
            }
        }

        debug!("✅ Networks validation passed");
        Ok(())
    }

    fn validate_volumes(&self, volumes: &HashMap<String, Volume>) -> Result<()> {
        debug!("Validating volume definitions");

        for (name, volume) in volumes {
            if let Some(ref driver) = volume.driver {
                match driver.as_str() {
                    "local" | "nfs" | "cifs" => {}
                    _ => warn!("Volume '{}': unknown driver '{}'", name, driver),
                }
            }
        }

        debug!("✅ Volumes validation passed");
        Ok(())
    }

    /// Lint the Boltfile and provide suggestions for improvements
    pub fn lint(&self) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Check for common optimization opportunities
        for (name, service) in &self.services {
            // Suggest using Bolt-native images
            if let Some(ref image) = service.image {
                if image.starts_with("docker.io/") {
                    suggestions.push(format!("Service '{}': Consider using Bolt-native image 'bolt://{}' for better performance", name, image.strip_prefix("docker.io/").unwrap_or(image)));
                }
            }

            // Suggest restart policies
            if service.restart.is_none() {
                suggestions.push(format!(
                    "Service '{}': Consider adding a restart policy (e.g., 'always', 'on-failure')",
                    name
                ));
            }

            // Suggest health checks for long-running services
            if service.restart == Some("always".to_string()) {
                suggestions.push(format!(
                    "Service '{}': Consider adding health checks for better reliability",
                    name
                ));
            }

            // Gaming-specific suggestions
            if let Some(ref gaming) = service.gaming {
                if gaming.gpu.is_some() && gaming.performance.is_none() {
                    suggestions.push(format!("Service '{}': Gaming service with GPU should consider performance optimizations", name));
                }

                if gaming.gpu.is_some() && gaming.audio.is_none() {
                    suggestions.push(format!(
                        "Service '{}': Gaming service should configure audio system",
                        name
                    ));
                }
            }

            // Security suggestions
            if service.volumes.is_some() {
                let volumes = service.volumes.as_ref().unwrap();
                for volume in volumes {
                    if !volume.contains(":ro") && !volume.contains(":rw") {
                        suggestions.push(format!("Service '{}': Consider explicitly specifying read-only (:ro) or read-write (:rw) for volume '{}'", name, volume));
                    }
                }
            }
        }

        // Network suggestions
        if self.networks.is_none() {
            suggestions
                .push("Consider defining custom networks for better service isolation".to_string());
        }

        // Gaming network suggestion
        let has_gaming_services = self.services.values().any(|s| s.gaming.is_some());
        if has_gaming_services && self.networks.is_none() {
            suggestions.push("Gaming services detected: Consider creating a dedicated gaming network with QUIC optimizations".to_string());
        }

        suggestions
    }

    /// Get schema information for the Boltfile format
    pub fn schema_info() -> String {
        r#"
Bolt Configuration Schema (TOML)

[project]
project = "string"               # Required: Project name (no spaces)

[services.<name>]
image = "string"                 # Docker/OCI image (mutually exclusive with build/capsule)
build = "string"                 # Build context path (mutually exclusive with image/capsule)
capsule = "string"               # Bolt capsule name (mutually exclusive with image/build)
ports = ["host:container"]       # Port mappings (optional)
volumes = ["host:container:opts"] # Volume mounts (optional)
env = {KEY = "value"}           # Environment variables (optional)
depends_on = ["service1"]        # Service dependencies (optional)
restart = "always"               # Restart policy: no, always, on-failure, unless-stopped (optional)
networks = ["network1"]          # Custom networks (optional)

[services.<name>.storage]        # Optional storage configuration
size = "5Gi"                     # Storage size (required if storage block present)
driver = "local"                 # Storage driver (optional)

[services.<name>.auth]           # Optional authentication
user = "username"                # Username (required if auth block present)
password = "password"            # Password (required if auth block present)

[services.<name>.gaming]         # Optional gaming optimizations
[services.<name>.gaming.gpu]     # GPU configuration
[services.<name>.gaming.gpu.nvidia]
device = 0                       # GPU device ID (optional)
dlss = true                      # Enable DLSS (optional)
raytracing = true                # Enable ray tracing (optional)
cuda = false                     # Enable CUDA (optional)

[services.<name>.gaming.gpu.amd]
device = 0                       # GPU device ID (optional)
rocm = true                      # Enable ROCm (optional)

[services.<name>.gaming.audio]
system = "pipewire"              # Audio system: pipewire, pulseaudio
latency = "low"                  # Audio latency setting (optional)

[services.<name>.gaming.wine]
proton = "8.0"                   # Proton version (optional)
winver = "win10"                 # Windows version (optional)
prefix = "/path/to/prefix"       # Wine prefix path (optional)

[services.<name>.gaming.performance]
cpu_governor = "performance"     # CPU governor (optional)
nice_level = -10                 # Process nice level -20 to 19 (optional)
rt_priority = 50                 # Real-time priority 0 to 99 (optional)

[services.<name>.mcp]            # Optional MCP (Model Context Protocol) configuration
enabled = true                   # Enable MCP for this service
tools = [                        # Enabled MCP tools (optional)
  "filesystem:read",
  "filesystem:write",
  "shell:exec",
  "gpu:stats",
  "process:list",
]

[services.<name>.mcp.permissions] # Tool-specific permissions (optional)
filesystem_paths = ["/workspace", "/src"]  # Allowed filesystem paths
shell_commands = ["npm", "cargo", "zig"]    # Allowed shell commands
gpu_access = "read_only"         # GPU access level: read_only, read_write, full
network_access = "read_only"     # Network access level: none, read_only, full
process_access = "read_only"     # Process access level: read_only, read_write, full

[services.<name>.mcp.omen]       # Omen AI Router configuration (optional, requires omen feature)
enabled = true                   # Enable Omen routing
strategy = "balanced"            # Routing strategy: cost_optimized, latency_optimized, balanced, quality_optimized
max_cost_per_hour = 5.00        # Maximum cost per hour in USD
providers = ["ollama", "anthropic"]  # Preferred providers

[networks.<name>]                # Optional custom networks
driver = "bolt"                  # Network driver: bolt, bridge, host (optional)
subnet = "10.0.0.0/16"          # Network subnet in CIDR notation (optional)

[volumes.<name>]                 # Optional named volumes
driver = "local"                 # Volume driver (optional)
external = false                 # Use external volume (optional)
"#
        .to_string()
    }
}

/// Bolt configuration for runtime operations
#[derive(Debug, Clone)]
pub struct BoltConfig {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub boltfile_path: PathBuf,
    pub verbose: bool,
    #[cfg(feature = "mcp")]
    pub mcp: Option<crate::mcp::McpConfig>,
}

impl Default for BoltConfig {
    fn default() -> Self {
        Self {
            config_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            boltfile_path: PathBuf::new(),
            verbose: false,
            #[cfg(feature = "mcp")]
            mcp: None,
        }
    }
}

impl BoltConfig {
    /// Load configuration from default locations
    pub fn load() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("bolt");

        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("bolt");

        let boltfile_path = std::env::current_dir()
            .unwrap_or_default()
            .join("Boltfile.toml");

        Ok(Self {
            config_dir,
            data_dir,
            boltfile_path,
            verbose: false,
            #[cfg(feature = "mcp")]
            mcp: None,
        })
    }

    /// Load Boltfile from the configured path
    pub fn load_boltfile(&self) -> Result<BoltFile> {
        BoltFile::load(&self.boltfile_path)
    }

    /// Save Boltfile to the configured path
    pub fn save_boltfile(&self, boltfile: &BoltFile) -> Result<()> {
        boltfile.save(&self.boltfile_path)
    }
}

pub fn create_example_boltfile() -> BoltFile {
    let mut services = HashMap::new();

    // Web service
    services.insert(
        "web".to_string(),
        Service {
            image: Some("bolt://nginx:latest".to_string()),
            ports: Some(vec!["80:80".to_string()]),
            volumes: Some(vec!["./site:/usr/share/nginx/html".to_string()]),
            depends_on: Some(vec!["api".to_string()]),
            restart: Some("always".to_string()),
            ..Default::default()
        },
    );

    // API service
    services.insert(
        "api".to_string(),
        Service {
            image: None,
            build: Some("./api".to_string()),
            ports: Some(vec!["3000:3000".to_string()]),
            env: {
                let mut env = HashMap::new();
                env.insert("DATABASE_URL".to_string(), "bolt://db".to_string());
                Some(env)
            },
            depends_on: Some(vec!["db".to_string()]),
            restart: Some("always".to_string()),
            ..Default::default()
        },
    );

    // Database service
    services.insert(
        "db".to_string(),
        Service {
            capsule: Some("postgres".to_string()),
            restart: Some("always".to_string()),
            storage: Some(Storage {
                size: "5Gi".to_string(),
                driver: None,
            }),
            auth: Some(Auth {
                user: "demo".to_string(),
                password: "secret".to_string(),
            }),
            ..Default::default()
        },
    );

    // AI development service example with MCP
    services.insert(
        "ai-dev".to_string(),
        Service {
            image: Some("bolt://rust:latest".to_string()),
            build: None,
            capsule: None,
            command: None,
            entrypoint: None,
            ports: Some(vec!["8080:8080".to_string()]),
            volumes: Some(vec![
                "./workspace:/workspace".to_string(),
                "./src:/src".to_string(),
            ]),
            environment: Some({
                let mut env = HashMap::new();
                env.insert("RUST_LOG".to_string(), "info".to_string());
                env
            }),
            env: None,
            depends_on: None,
            restart: Some("unless-stopped".to_string()),
            networks: None,
            storage: Some(Storage {
                size: "20Gi".to_string(),
                driver: None,
            }),
            auth: None,
            gaming: None,
            working_dir: Some("/workspace".to_string()),
            user: None,
            hostname: Some("ai-dev".to_string()),
            container_name: Some("bolt-ai-dev".to_string()),
            privileged: None,
            read_only: None,
            stdin_open: Some(true),
            tty: Some(true),
            network_mode: None,
            pid: None,
            ipc: None,
            platform: None,
            labels: None,
            devices: None,
            cap_add: None,
            cap_drop: None,
            security_opt: None,
            sysctls: None,
            tmpfs: None,
            dns: None,
            dns_search: None,
            extra_hosts: None,
            group_add: None,
            volumes_from: None,
            links: None,
            logging: None,
            healthcheck: None,
            cpu_limit: Some("2".to_string()),
            memory_limit: Some("4Gi".to_string()),
            mcp: Some(ServiceMcpConfig {
                enabled: true,
                tools: vec![
                    "filesystem:read".to_string(),
                    "filesystem:write".to_string(),
                    "shell:exec".to_string(),
                    "gpu:stats".to_string(),
                    "process:list".to_string(),
                ],
                permissions: Some(McpPermissions {
                    filesystem_paths: vec![
                        "/workspace".to_string(),
                        "/src".to_string(),
                        "/tmp".to_string(),
                    ],
                    shell_commands: vec![
                        "cargo".to_string(),
                        "rustc".to_string(),
                        "git".to_string(),
                        "npm".to_string(),
                        "ls".to_string(),
                        "cat".to_string(),
                    ],
                    gpu_access: Some("read_only".to_string()),
                    network_access: Some("read_only".to_string()),
                    process_access: Some("read_only".to_string()),
                }),
                #[cfg(feature = "omen")]
                omen: Some(McpOmenConfig {
                    enabled: true,
                    strategy: "balanced".to_string(),
                    max_cost_per_hour: Some(5.0),
                    providers: vec![
                        "ollama".to_string(),
                        "anthropic".to_string(),
                    ],
                    provider_config: {
                        let mut config = HashMap::new();
                        config.insert("ollama_endpoint".to_string(), "http://localhost:11434".to_string());
                        config
                    },
                }),
            }),
        },
    );

    // Gaming service example
    services.insert(
        "game".to_string(),
        Service {
            image: Some("bolt://steam:latest".to_string()),
            build: None,
            capsule: None,
            ports: None,
            volumes: Some(vec![
                "./games:/games".to_string(),
                "/dev/dri:/dev/dri".to_string(),
            ]),
            environment: None,
            env: None,
            depends_on: None,
            restart: Some("no".to_string()),
            networks: None,
            storage: Some(Storage {
                size: "100Gi".to_string(),
                driver: None,
            }),
            auth: None,
            gaming: Some(GamingConfig {
                enabled: true,
                gpu_passthrough: true,
                nvidia_runtime: true,
                amd_runtime: false,
                audio_passthrough: true,
                real_time_priority: true,
                wine_prefix: Some("/games/wine-prefix".to_string()),
                proton_version: Some("8.0".to_string()),
                dxvk_enabled: Some(true),
                esync_enabled: Some(true),
                fsync_enabled: Some(true),
                performance_profile: Some("maximum".to_string()),
                input_devices: None,
                display_driver: None,
                resolution: None,
                refresh_rate: None,
                vsync: Some(true),
                gpu: Some(GpuConfig {
                    runtime: Some("nvbind".to_string()),
                    nvidia: Some(NvidiaConfig {
                        device: Some(0),
                        dlss: Some(true),
                        reflex: Some(false),
                        raytracing: Some(true),
                        cuda: Some(false),
                        power_limit: Some(100),
                        memory_clock_offset: Some(0),
                        core_clock_offset: Some(0),
                    }),
                    amd: None,
                    nvbind: Some(NvbindConfig {
                        driver: Some("auto".to_string()),
                        devices: Some(vec!["gpu:0".to_string()]),
                        wsl2_optimized: Some(true),
                        performance_mode: Some("ultra".to_string()),
                        preload_libraries: Some(true),
                    }),
                    passthrough: Some(true),
                    isolation_level: Some("exclusive".to_string()),
                    memory_limit: Some("8GB".to_string()),
                    gaming: Some(GpuGamingConfig {
                        profile: Some("ultra-low-latency".to_string()),
                        dlss_enabled: Some(true),
                        rt_cores_enabled: Some(true),
                        wine_optimizations: Some(true),
                        vrs_enabled: Some(true),
                        performance_profile: Some("maximum".to_string()),
                    }),
                    aiml: None,
                }),
                audio: Some(AudioConfig {
                    system: "pipewire".to_string(),
                    latency: Some("low".to_string()),
                }),
                wine: Some(WineConfig {
                    version: None,
                    proton: Some("8.0".to_string()),
                    winver: Some("win10".to_string()),
                    prefix: Some("/games/wine-prefix".to_string()),
                }),
                performance: Some(PerformanceConfig {
                    cpu_governor: Some("performance".to_string()),
                    nice_level: Some(-10),
                    rt_priority: Some(50),
                }),
            }),
            ..Default::default()
        },
    );

    BoltFile {
        project: "demo".to_string(),
        services,
        networks: None,
        volumes: None,
        snapshots: Some(SnapshotConfig {
            enabled: Some(true),
            filesystem: Some("auto".to_string()),
            root_path: Some("/".to_string()),
            snapshot_path: Some("/.snapshots".to_string()),
            retention: Some(RetentionPolicy {
                keep_hourly: Some(0),  // No hourly snapshots
                keep_daily: Some(7),   // Keep 7 daily snapshots
                keep_weekly: Some(4),  // Keep 4 weekly snapshots
                keep_monthly: Some(6), // Keep 6 monthly snapshots
                keep_yearly: Some(2),  // Keep 2 yearly snapshots
                max_total: Some(50),   // Maximum 50 total snapshots
                cleanup_frequency: Some("daily".to_string()),
            }),
            triggers: Some(SnapshotTriggers {
                // Time-based
                hourly: Some(false),
                daily: Some("02:00".to_string()), // Daily at 2 AM
                weekly: Some("sunday@03:00".to_string()), // Sunday at 3 AM
                monthly: Some("1@04:00".to_string()), // 1st of month at 4 AM

                // Operation-based
                before_container_run: Some(false), // Don't snapshot before every run
                before_build: Some(true),          // Snapshot before builds
                before_surge_up: Some(true),       // Snapshot before surge operations
                before_system_update: Some(true),  // Snapshot before system updates

                // Change-based
                on_file_changes: Some(ChangeBasedConfig {
                    enabled: Some(true),
                    watch_paths: Some(vec![
                        "/etc".to_string(),
                        "/home".to_string(),
                        "/var/lib/bolt".to_string(),
                    ]),
                    exclude_paths: Some(vec![
                        "/tmp".to_string(),
                        "/var/tmp".to_string(),
                        "/var/log".to_string(),
                        "/var/cache".to_string(),
                    ]),
                    file_patterns: Some(vec![
                        "*.toml".to_string(),
                        "*.yaml".to_string(),
                        "*.yml".to_string(),
                        "*.json".to_string(),
                        "*.conf".to_string(),
                    ]),
                    exclude_patterns: Some(vec![
                        "*.tmp".to_string(),
                        "*.log".to_string(),
                        "*.cache".to_string(),
                        ".git/*".to_string(),
                    ]),
                    change_types: Some(vec!["modify".to_string(), "create".to_string()]),
                }),
                min_change_threshold: Some("50MB".to_string()),
                change_detection_interval: Some("30m".to_string()),
            }),
            named_snapshots: Some(vec![
                NamedSnapshot {
                    name: "fresh-install".to_string(),
                    description: Some("Clean system after fresh Bolt installation".to_string()),
                    trigger: Some("manual".to_string()),
                    auto_create: Some(false),
                    keep_forever: Some(true),
                },
                NamedSnapshot {
                    name: "before-gaming".to_string(),
                    description: Some("Before setting up gaming environment".to_string()),
                    trigger: Some("before_gaming_setup".to_string()),
                    auto_create: Some(true),
                    keep_forever: Some(false),
                },
                NamedSnapshot {
                    name: "stable-config".to_string(),
                    description: Some("Known stable configuration".to_string()),
                    trigger: Some("manual".to_string()),
                    auto_create: Some(false),
                    keep_forever: Some(true),
                },
            ]),
        }),
    }
}


#[cfg(test)]
mod mcp_validation_tests {
    use super::*;

    #[test]
    fn test_valid_mcp_config() {
        let mcp = ServiceMcpConfig {
            enabled: true,
            tools: vec![
                "filesystem:read".to_string(),
                "shell:exec".to_string(),
                "gpu:stats".to_string(),
            ],
            permissions: Some(McpPermissions {
                filesystem_paths: vec!["/workspace".to_string()],
                shell_commands: vec!["cargo".to_string()],
                gpu_access: Some("read_only".to_string()),
                network_access: Some("none".to_string()),
                process_access: Some("read_only".to_string()),
            }),
            #[cfg(feature = "omen")]
            omen: None,
        };

        assert!(mcp.validate().is_ok());
    }

    #[test]
    fn test_invalid_tool_name() {
        let mcp = ServiceMcpConfig {
            enabled: true,
            tools: vec!["invalid:tool".to_string()],
            permissions: None,
            #[cfg(feature = "omen")]
            omen: None,
        };

        assert!(mcp.validate().is_err());
    }

    #[test]
    fn test_invalid_access_level() {
        let mcp = ServiceMcpConfig {
            enabled: true,
            tools: vec!["gpu:stats".to_string()],
            permissions: Some(McpPermissions {
                filesystem_paths: vec![],
                shell_commands: vec![],
                gpu_access: Some("invalid_level".to_string()),
                network_access: None,
                process_access: None,
            }),
            #[cfg(feature = "omen")]
            omen: None,
        };

        assert!(mcp.validate().is_err());
    }

    #[test]
    fn test_invalid_filesystem_path() {
        let perms = McpPermissions {
            filesystem_paths: vec!["not-absolute".to_string()],
            shell_commands: vec![],
            gpu_access: None,
            network_access: None,
            process_access: None,
        };

        assert!(perms.validate().is_err());
    }

    #[test]
    fn test_unsafe_shell_command() {
        let perms = McpPermissions {
            filesystem_paths: vec![],
            shell_commands: vec!["rm && rm -rf /".to_string()],
            gpu_access: None,
            network_access: None,
            process_access: None,
        };

        assert!(perms.validate().is_err());
    }

    #[cfg(feature = "omen")]
    #[test]
    fn test_valid_omen_config() {
        let omen = McpOmenConfig {
            enabled: true,
            strategy: "balanced".to_string(),
            max_cost_per_hour: Some(5.0),
            providers: vec!["ollama".to_string(), "anthropic".to_string()],
            provider_config: HashMap::new(),
        };

        assert!(omen.validate().is_ok());
    }

    #[cfg(feature = "omen")]
    #[test]
    fn test_invalid_omen_strategy() {
        let omen = McpOmenConfig {
            enabled: true,
            strategy: "invalid_strategy".to_string(),
            max_cost_per_hour: None,
            providers: vec![],
            provider_config: HashMap::new(),
        };

        assert!(omen.validate().is_err());
    }

    #[cfg(feature = "omen")]
    #[test]
    fn test_invalid_omen_provider() {
        let omen = McpOmenConfig {
            enabled: true,
            strategy: "balanced".to_string(),
            max_cost_per_hour: None,
            providers: vec!["invalid_provider".to_string()],
            provider_config: HashMap::new(),
        };

        assert!(omen.validate().is_err());
    }

    #[cfg(feature = "omen")]
    #[test]
    fn test_negative_max_cost() {
        let omen = McpOmenConfig {
            enabled: true,
            strategy: "cost_optimized".to_string(),
            max_cost_per_hour: Some(-5.0),
            providers: vec!["ollama".to_string()],
            provider_config: HashMap::new(),
        };

        assert!(omen.validate().is_err());
    }
}

