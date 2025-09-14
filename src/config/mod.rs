use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use anyhow::{Context, Result, anyhow};
use tracing::{info, warn, debug};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoltFile {
    pub project: String,
    pub services: HashMap<String, Service>,
    pub networks: Option<HashMap<String, Network>>,
    pub volumes: Option<HashMap<String, Volume>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Service {
    pub image: Option<String>,
    pub build: Option<String>,
    pub capsule: Option<String>,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub environment: Option<HashMap<String, String>>,
    pub env: Option<HashMap<String, String>>,
    pub depends_on: Option<Vec<String>>,
    pub restart: Option<RestartPolicy>,
    pub networks: Option<Vec<String>>,
    pub storage: Option<Storage>,
    pub auth: Option<Auth>,
    pub gaming: Option<GamingConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Network {
    pub driver: Option<String>,
    pub subnet: Option<String>,
    pub gateway: Option<String>,
    pub ipam: Option<Ipam>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Ipam {
    pub driver: String,
    pub config: Vec<IpamConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IpamConfig {
    pub subnet: String,
    pub gateway: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Volume {
    pub driver: Option<String>,
    pub driver_opts: Option<HashMap<String, String>>,
    pub external: Option<bool>,
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
    pub gpu: Option<GpuConfig>,
    pub audio: Option<AudioConfig>,
    pub wine: Option<WineConfig>,
    pub performance: Option<PerformanceConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GpuConfig {
    pub nvidia: Option<NvidiaConfig>,
    pub amd: Option<AmdConfig>,
    pub passthrough: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NvidiaConfig {
    pub device: Option<u32>,
    pub dlss: Option<bool>,
    pub raytracing: Option<bool>,
    pub cuda: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AmdConfig {
    pub device: Option<u32>,
    pub rocm: Option<bool>,
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

impl BoltFile {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read Boltfile at {:?}", path.as_ref()))?;

        let config: BoltFile = toml::from_str(&content)
            .with_context(|| "Failed to parse Boltfile")?;

        // Validate the configuration
        config.validate()?;

        Ok(config)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        // Validate before saving
        self.validate()?;

        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize Boltfile")?;

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
            return Err(anyhow!("Project name cannot contain spaces: '{}'", self.project));
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
                return Err(anyhow!("Service '{}' must specify either 'image', 'build', or 'capsule'", name));
            }

            // Validate mutually exclusive options
            let options_count = [service.image.is_some(), service.build.is_some(), service.capsule.is_some()]
                .iter()
                .filter(|&&x| x)
                .count();

            if options_count > 1 {
                return Err(anyhow!("Service '{}' can only specify one of 'image', 'build', or 'capsule'", name));
            }

            // Validate gaming configuration
            if let Some(ref gaming) = service.gaming {
                self.validate_gaming_config(name, gaming)?;
            }

            // Validate storage configuration
            if let Some(ref storage) = service.storage {
                self.validate_storage_config(name, storage)?;
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
                        return Err(anyhow!("Service '{}' depends on non-existent service '{}'", name, dep));
                    }
                }
            }
        }

        debug!("✅ Dependencies validation passed");
        Ok(())
    }

    fn check_circular_dependencies(&self, service: &str, deps: &[String], visited: &mut HashSet<String>) -> Result<()> {
        if visited.contains(service) {
            return Err(anyhow!("Circular dependency detected involving service '{}'", service));
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

        for (name, service) in &self.services {
            if let Some(ref ports) = service.ports {
                for port_mapping in ports {
                    let host_port = self.extract_host_port(port_mapping)?;

                    if used_host_ports.contains(&host_port) {
                        return Err(anyhow!("Port conflict: host port {} is used by multiple services", host_port));
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
        host_port_str.parse::<u16>()
            .with_context(|| format!("Invalid host port number: '{}'", host_port_str))
    }

    fn validate_service_ports(&self, service_name: &str, ports: &[String]) -> Result<()> {
        for port in ports {
            // Validate port mapping format (host:container or just port)
            if port.contains(':') {
                let parts: Vec<&str> = port.split(':').collect();
                if parts.len() != 2 {
                    return Err(anyhow!("Service '{}': invalid port mapping format '{}'", service_name, port));
                }

                // Validate host port
                parts[0].parse::<u16>()
                    .with_context(|| format!("Service '{}': invalid host port '{}'", service_name, parts[0]))?;

                // Validate container port
                parts[1].parse::<u16>()
                    .with_context(|| format!("Service '{}': invalid container port '{}'", service_name, parts[1]))?;
            } else {
                // Single port number
                port.parse::<u16>()
                    .with_context(|| format!("Service '{}': invalid port number '{}'", service_name, port))?;
            }
        }

        Ok(())
    }

    fn validate_service_volumes(&self, service_name: &str, volumes: &[String]) -> Result<()> {
        for volume in volumes {
            if volume.contains(':') {
                let parts: Vec<&str> = volume.split(':').collect();
                if parts.len() < 2 || parts.len() > 3 {
                    return Err(anyhow!("Service '{}': invalid volume mapping format '{}'", service_name, volume));
                }

                // Validate host path (first part)
                let host_path = parts[0];
                if host_path.is_empty() {
                    return Err(anyhow!("Service '{}': empty host path in volume mapping '{}'", service_name, volume));
                }

                // Validate container path (second part)
                let container_path = parts[1];
                if container_path.is_empty() {
                    return Err(anyhow!("Service '{}': empty container path in volume mapping '{}'", service_name, volume));
                }

                // Validate options (third part, if present)
                if parts.len() == 3 {
                    let options = parts[2];
                    for option in options.split(',') {
                        match option {
                            "ro" | "rw" | "z" | "Z" => {},  // Valid options
                            _ => warn!("Service '{}': unknown volume option '{}' in '{}'", service_name, option, volume),
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
                    warn!("Service '{}': NVIDIA device ID {} is unusually high", service_name, nvidia.device.unwrap());
                }
            }

            if let Some(ref amd) = gpu.amd {
                if amd.device.is_some() && amd.device.unwrap() > 7 {
                    warn!("Service '{}': AMD device ID {} is unusually high", service_name, amd.device.unwrap());
                }
            }
        }

        if let Some(ref audio) = gaming.audio {
            match audio.system.as_str() {
                "pipewire" | "pulseaudio" => {},
                _ => return Err(anyhow!("Service '{}': unsupported audio system '{}'", service_name, audio.system)),
            }
        }

        if let Some(ref perf) = gaming.performance {
            if let Some(nice) = perf.nice_level {
                if nice < -20 || nice > 19 {
                    return Err(anyhow!("Service '{}': nice level {} out of range (-20 to 19)", service_name, nice));
                }
            }

            if let Some(rt_prio) = perf.rt_priority {
                if rt_prio > 99 {
                    return Err(anyhow!("Service '{}': RT priority {} out of range (0 to 99)", service_name, rt_prio));
                }
            }
        }

        Ok(())
    }

    fn validate_storage_config(&self, service_name: &str, storage: &Storage) -> Result<()> {
        // Validate storage size format
        let size = &storage.size;
        if !size.ends_with("Gi") && !size.ends_with("Mi") && !size.ends_with("Ki") && !size.ends_with("G") && !size.ends_with("M") && !size.ends_with("K") {
            return Err(anyhow!("Service '{}': invalid storage size format '{}' (use formats like '5Gi', '500Mi')", service_name, size));
        }

        // Extract numeric part
        let numeric_part = size.trim_end_matches(|c: char| c.is_alphabetic());
        numeric_part.parse::<f64>()
            .with_context(|| format!("Service '{}': invalid storage size number in '{}'", service_name, size))?;

        Ok(())
    }

    fn validate_networks(&self, networks: &HashMap<String, Network>) -> Result<()> {
        debug!("Validating network definitions");

        for (name, network) in networks {
            if let Some(ref subnet) = network.subnet {
                if !subnet.contains('/') {
                    return Err(anyhow!("Network '{}': subnet '{}' must be in CIDR notation", name, subnet));
                }
            }

            if let Some(ref ipam) = network.ipam {
                for config in &ipam.config {
                    if !config.subnet.contains('/') {
                        return Err(anyhow!("Network '{}': IPAM subnet '{}' must be in CIDR notation", name, config.subnet));
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
                    "local" | "nfs" | "cifs" => {},
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
                suggestions.push(format!("Service '{}': Consider adding a restart policy (e.g., 'always', 'on-failure')", name));
            }

            // Suggest health checks for long-running services
            if service.restart == Some(RestartPolicy::Always) {
                suggestions.push(format!("Service '{}': Consider adding health checks for better reliability", name));
            }

            // Gaming-specific suggestions
            if let Some(ref gaming) = service.gaming {
                if gaming.gpu.is_some() && gaming.performance.is_none() {
                    suggestions.push(format!("Service '{}': Gaming service with GPU should consider performance optimizations", name));
                }

                if gaming.gpu.is_some() && gaming.audio.is_none() {
                    suggestions.push(format!("Service '{}': Gaming service should configure audio system", name));
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
            suggestions.push("Consider defining custom networks for better service isolation".to_string());
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

[networks.<name>]                # Optional custom networks
driver = "bolt"                  # Network driver: bolt, bridge, host (optional)
subnet = "10.0.0.0/16"          # Network subnet in CIDR notation (optional)

[volumes.<name>]                 # Optional named volumes
driver = "local"                 # Volume driver (optional)
external = false                 # Use external volume (optional)
"#.to_string()
    }
}

/// Bolt configuration for runtime operations
#[derive(Debug, Clone, Default)]
pub struct BoltConfig {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub boltfile_path: PathBuf,
    pub verbose: bool,
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
    services.insert("web".to_string(), Service {
        image: Some("bolt://nginx:latest".to_string()),
        build: None,
        capsule: None,
        ports: Some(vec!["80:80".to_string()]),
        volumes: Some(vec!["./site:/usr/share/nginx/html".to_string()]),
        environment: None,
        env: None,
        depends_on: Some(vec!["api".to_string()]),
        restart: Some(RestartPolicy::Always),
        networks: None,
        storage: None,
        auth: None,
        gaming: None,
    });

    // API service
    services.insert("api".to_string(), Service {
        image: None,
        build: Some("./api".to_string()),
        capsule: None,
        ports: Some(vec!["3000:3000".to_string()]),
        volumes: None,
        environment: None,
        env: {
            let mut env = HashMap::new();
            env.insert("DATABASE_URL".to_string(), "bolt://db".to_string());
            Some(env)
        },
        depends_on: Some(vec!["db".to_string()]),
        restart: Some(RestartPolicy::Always),
        networks: None,
        storage: None,
        auth: None,
        gaming: None,
    });

    // Database service
    services.insert("db".to_string(), Service {
        image: None,
        build: None,
        capsule: Some("postgres".to_string()),
        ports: None,
        volumes: None,
        environment: None,
        env: None,
        depends_on: None,
        restart: Some(RestartPolicy::Always),
        networks: None,
        storage: Some(Storage {
            size: "5Gi".to_string(),
            driver: None,
        }),
        auth: Some(Auth {
            user: "demo".to_string(),
            password: "secret".to_string(),
        }),
        gaming: None,
    });

    // Gaming service example
    services.insert("game".to_string(), Service {
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
        restart: Some(RestartPolicy::No),
        networks: None,
        storage: Some(Storage {
            size: "100Gi".to_string(),
            driver: None,
        }),
        auth: None,
        gaming: Some(GamingConfig {
            gpu: Some(GpuConfig {
                nvidia: Some(NvidiaConfig {
                    device: Some(0),
                    dlss: Some(true),
                    raytracing: Some(true),
                    cuda: Some(false),
                }),
                amd: None,
                passthrough: Some(true),
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
    });

    BoltFile {
        project: "demo".to_string(),
        services,
        networks: None,
        volumes: None,
    }
}