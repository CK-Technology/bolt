use crate::config::{BoltFile, GamingConfig, NetworkConfig, Service, VolumeConfig};
use crate::error::{BoltError, Result};
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::collections::HashMap;
use std::path::Path;

/// Complete Docker Compose specification support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerCompose {
    pub version: Option<String>,
    pub services: HashMap<String, DockerComposeService>,
    pub networks: Option<HashMap<String, DockerComposeNetwork>>,
    pub volumes: Option<HashMap<String, DockerComposeVolumeSpec>>,
    pub configs: Option<HashMap<String, DockerComposeConfig>>,
    pub secrets: Option<HashMap<String, DockerComposeSecret>>,
    #[serde(rename = "x-bolt-gaming")]
    pub bolt_gaming: Option<DockerComposeBoltGaming>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeService {
    pub image: Option<String>,
    pub build: Option<DockerComposeBuild>,
    pub command: Option<StringOrArray>,
    pub entrypoint: Option<StringOrArray>,
    pub environment: Option<Environment>,
    pub env_file: Option<StringOrArray>,
    pub ports: Option<Vec<DockerComposePort>>,
    pub expose: Option<Vec<String>>,
    pub volumes: Option<Vec<DockerComposeVolume>>,
    pub networks: Option<DockerComposeServiceNetworks>,
    pub depends_on: Option<Vec<String>>,
    pub external_links: Option<Vec<String>>,
    pub restart: Option<String>,
    pub container_name: Option<String>,
    pub hostname: Option<String>,
    pub domainname: Option<String>,
    pub user: Option<String>,
    pub working_dir: Option<String>,
    pub cap_add: Option<Vec<String>>,
    pub cap_drop: Option<Vec<String>>,
    pub cgroup_parent: Option<String>,
    pub devices: Option<Vec<String>>,
    pub device_cgroup_rules: Option<Vec<String>>,
    pub dns: Option<StringOrArray>,
    pub dns_search: Option<Vec<String>>,
    pub tmpfs: Option<StringOrArray>,
    pub extra_hosts: Option<Vec<String>>,
    pub group_add: Option<Vec<String>>,
    pub ipc: Option<String>,
    pub isolation: Option<String>,
    pub labels: Option<HashMap<String, String>>,
    pub logging: Option<DockerComposeLogging>,
    pub network_mode: Option<String>,
    pub pid: Option<String>,
    pub platform: Option<String>,
    pub privileged: Option<bool>,
    pub read_only: Option<bool>,
    pub security_opt: Option<Vec<String>>,
    pub shm_size: Option<String>,
    pub stdin_open: Option<bool>,
    pub stop_grace_period: Option<String>,
    pub stop_signal: Option<String>,
    pub sysctls: Option<HashMap<String, String>>,
    pub tty: Option<bool>,
    pub ulimits: Option<HashMap<String, DockerComposeUlimit>>,
    pub userns_mode: Option<String>,
    pub volumes_from: Option<Vec<String>>,
    pub cpu_count: Option<u32>,
    pub cpu_percent: Option<u32>,
    pub cpu_shares: Option<u64>,
    pub cpu_period: Option<u64>,
    pub cpu_quota: Option<u64>,
    pub cpu_rt_period: Option<u64>,
    pub cpu_rt_runtime: Option<u64>,
    pub cpuset: Option<String>,
    pub cpus: Option<String>,
    pub mem_limit: Option<String>,
    pub mem_reservation: Option<String>,
    pub mem_swappiness: Option<u64>,
    pub memswap_limit: Option<String>,
    pub oom_kill_disable: Option<bool>,
    pub oom_score_adj: Option<i32>,
    pub pids_limit: Option<u64>,
    pub blkio_config: Option<DockerComposeBlkioConfig>,
    pub deploy: Option<DockerComposeDeploy>,
    pub healthcheck: Option<DockerComposeHealthcheck>,
    #[serde(rename = "x-bolt-gaming")]
    pub bolt_gaming: Option<DockerComposeBoltGaming>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrArray {
    String(String),
    Array(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Environment {
    Map(HashMap<String, String>),
    Array(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeBuild {
    pub context: Option<String>,
    pub dockerfile: Option<String>,
    pub args: Option<HashMap<String, String>>,
    pub cache_from: Option<Vec<String>>,
    pub labels: Option<HashMap<String, String>>,
    pub network: Option<String>,
    pub shm_size: Option<String>,
    pub target: Option<String>,
    pub extra_hosts: Option<Vec<String>>,
    pub isolation: Option<String>,
    pub privileged: Option<bool>,
    pub pull: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DockerComposePort {
    String(String),
    Long {
        target: u16,
        published: Option<u16>,
        protocol: Option<String>,
        mode: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DockerComposeVolume {
    String(String),
    Long {
        #[serde(rename = "type")]
        volume_type: String,
        source: Option<String>,
        target: String,
        read_only: Option<bool>,
        bind: Option<DockerComposeBindOptions>,
        volume: Option<DockerComposeVolumeOptions>,
        tmpfs: Option<DockerComposeTmpfsOptions>,
        consistency: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeBindOptions {
    pub propagation: Option<String>,
    pub create_host_path: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeVolumeOptions {
    pub nocopy: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeTmpfsOptions {
    pub size: Option<String>,
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DockerComposeServiceNetworks {
    List(Vec<String>),
    Map(HashMap<String, DockerComposeServiceNetwork>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeServiceNetwork {
    pub aliases: Option<Vec<String>>,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
    pub link_local_ips: Option<Vec<String>>,
    pub priority: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeLogging {
    pub driver: Option<String>,
    pub options: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DockerComposeUlimit {
    Single(u64),
    Detailed { soft: u64, hard: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeBlkioConfig {
    pub weight: Option<u16>,
    pub weight_device: Option<Vec<DockerComposeWeightDevice>>,
    pub device_read_bps: Option<Vec<DockerComposeThrottleDevice>>,
    pub device_write_bps: Option<Vec<DockerComposeThrottleDevice>>,
    pub device_read_iops: Option<Vec<DockerComposeThrottleDevice>>,
    pub device_write_iops: Option<Vec<DockerComposeThrottleDevice>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeWeightDevice {
    pub path: String,
    pub weight: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeThrottleDevice {
    pub path: String,
    pub rate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeDeploy {
    pub mode: Option<String>,
    pub replicas: Option<u32>,
    pub labels: Option<HashMap<String, String>>,
    pub update_config: Option<DockerComposeUpdateConfig>,
    pub rollback_config: Option<DockerComposeRollbackConfig>,
    pub restart_policy: Option<DockerComposeRestartPolicy>,
    pub placement: Option<DockerComposePlacement>,
    pub endpoint_mode: Option<String>,
    pub resources: Option<DockerComposeResources>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeUpdateConfig {
    pub parallelism: Option<u32>,
    pub delay: Option<String>,
    pub failure_action: Option<String>,
    pub monitor: Option<String>,
    pub max_failure_ratio: Option<f64>,
    pub order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeRollbackConfig {
    pub parallelism: Option<u32>,
    pub delay: Option<String>,
    pub failure_action: Option<String>,
    pub monitor: Option<String>,
    pub max_failure_ratio: Option<f64>,
    pub order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeRestartPolicy {
    pub condition: Option<String>,
    pub delay: Option<String>,
    pub max_attempts: Option<u32>,
    pub window: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposePlacement {
    pub constraints: Option<Vec<String>>,
    pub preferences: Option<Vec<DockerComposePlacementPreference>>,
    pub max_replicas_per_node: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposePlacementPreference {
    pub spread: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeResources {
    pub limits: Option<DockerComposeResourceLimits>,
    pub reservations: Option<DockerComposeResourceReservations>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeResourceLimits {
    pub cpus: Option<String>,
    pub memory: Option<String>,
    pub pids: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeResourceReservations {
    pub cpus: Option<String>,
    pub memory: Option<String>,
    pub generic_resources: Option<Vec<DockerComposeGenericResource>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeGenericResource {
    pub discrete_resource_spec: Option<DockerComposeDiscreteResourceSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeDiscreteResourceSpec {
    pub kind: String,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeHealthcheck {
    pub test: Option<StringOrArray>,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    pub start_period: Option<String>,
    pub disable: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeNetwork {
    pub driver: Option<String>,
    pub driver_opts: Option<HashMap<String, String>>,
    pub attachable: Option<bool>,
    pub enable_ipv6: Option<bool>,
    pub ipam: Option<DockerComposeIpam>,
    pub internal: Option<bool>,
    pub labels: Option<HashMap<String, String>>,
    pub external: Option<DockerComposeExternal>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeIpam {
    pub driver: Option<String>,
    pub config: Option<Vec<DockerComposeIpamConfig>>,
    pub options: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeIpamConfig {
    pub subnet: Option<String>,
    pub ip_range: Option<String>,
    pub gateway: Option<String>,
    pub aux_addresses: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DockerComposeExternal {
    Bool(bool),
    Named { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DockerComposeVolumeSpec {
    Simple(HashMap<String, serde_yaml::Value>),
    External(DockerComposeExternal),
    Named {
        driver: Option<String>,
        driver_opts: Option<HashMap<String, String>>,
        external: Option<DockerComposeExternal>,
        labels: Option<HashMap<String, String>>,
        name: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeConfig {
    pub file: Option<String>,
    pub external: Option<DockerComposeExternal>,
    pub labels: Option<HashMap<String, String>>,
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<HashMap<String, String>>,
    pub template_driver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeSecret {
    pub file: Option<String>,
    pub external: Option<DockerComposeExternal>,
    pub labels: Option<HashMap<String, String>>,
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<HashMap<String, String>>,
    pub template_driver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeBoltGaming {
    pub enabled: Option<bool>,
    pub gpu_passthrough: Option<bool>,
    pub nvidia_runtime: Option<bool>,
    pub amd_runtime: Option<bool>,
    pub audio_passthrough: Option<bool>,
    pub real_time_priority: Option<bool>,
    pub wine_config: Option<DockerComposeBoltWineConfig>,
    pub performance_profile: Option<String>,
    pub input_devices: Option<Vec<String>>,
    pub display_config: Option<DockerComposeBoltDisplayConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeBoltWineConfig {
    pub prefix: Option<String>,
    pub version: Option<String>,
    pub proton: Option<bool>,
    pub proton_version: Option<String>,
    pub dxvk: Option<bool>,
    pub vkd3d: Option<bool>,
    pub esync: Option<bool>,
    pub fsync: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeBoltDisplayConfig {
    pub driver: Option<String>,
    pub resolution: Option<String>,
    pub refresh_rate: Option<u32>,
    pub full_screen: Option<bool>,
    pub vsync: Option<bool>,
}

/// Docker Compose parser and converter
pub struct DockerComposeParser;

impl DockerComposeParser {
    /// Parse Docker Compose file and convert to Bolt format
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<BoltFile> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            BoltError::Config(crate::error::ConfigError::InvalidFormat {
                reason: format!("Failed to read compose file: {}", e),
            })
        })?;

        Self::parse_yaml(&content)
    }

    /// Parse Docker Compose YAML content
    pub fn parse_yaml(content: &str) -> Result<BoltFile> {
        let compose: DockerCompose = serde_yaml::from_str(content).map_err(|e| {
            BoltError::Config(crate::error::ConfigError::InvalidFormat {
                reason: format!("Failed to parse compose YAML: {}", e),
            })
        })?;

        Self::convert_to_bolt_file(compose)
    }

    /// Convert Docker Compose to BoltFile
    pub fn convert_to_bolt_file(compose: DockerCompose) -> Result<BoltFile> {
        let project = "docker-compose-project".to_string();
        let mut services = HashMap::new();

        // Convert services
        for (name, docker_service) in compose.services {
            let bolt_service = Self::convert_service(docker_service)?;
            services.insert(name, bolt_service);
        }

        // Convert networks
        let networks = compose.networks.map(|nets| {
            nets.into_iter()
                .map(|(name, net)| (name, Self::convert_network(net)))
                .collect()
        });

        // Convert volumes
        let volumes = compose.volumes.map(|vols| {
            vols.into_iter()
                .map(|(name, vol)| (name, Self::convert_volume(vol)))
                .collect()
        });

        Ok(BoltFile {
            project,
            services,
            networks,
            volumes,
            snapshots: None,
        })
    }

    /// Convert Docker Compose service to Bolt service
    fn convert_service(docker_service: DockerComposeService) -> Result<Service> {
        let mut service = Service::default();

        // Basic properties
        service.image = docker_service.image;
        service.build = docker_service
            .build
            .map(|b| b.context.unwrap_or(".".to_string()));
        service.command = Self::convert_string_or_array(docker_service.command);
        service.entrypoint = Self::convert_string_or_array(docker_service.entrypoint);
        service.working_dir = docker_service.working_dir;
        service.user = docker_service.user;
        service.hostname = docker_service.hostname;
        service.container_name = docker_service.container_name;
        service.restart = docker_service.restart;
        service.privileged = docker_service.privileged;
        service.read_only = docker_service.read_only;
        service.stdin_open = docker_service.stdin_open;
        service.tty = docker_service.tty;
        service.network_mode = docker_service.network_mode;
        service.pid = docker_service.pid;
        service.ipc = docker_service.ipc;
        service.platform = docker_service.platform;
        service.labels = docker_service.labels;

        // Environment variables
        service.environment = match docker_service.environment {
            Some(Environment::Map(map)) => Some(map),
            Some(Environment::Array(arr)) => {
                let mut env_map = HashMap::new();
                for env_var in arr {
                    if let Some((key, value)) = env_var.split_once('=') {
                        env_map.insert(key.to_string(), value.to_string());
                    } else {
                        env_map.insert(env_var, "".to_string());
                    }
                }
                Some(env_map)
            }
            None => None,
        };

        // Ports
        if let Some(ports) = docker_service.ports {
            service.ports = Some(
                ports
                    .into_iter()
                    .map(|port| match port {
                        DockerComposePort::String(s) => s,
                        DockerComposePort::Long {
                            target,
                            published,
                            protocol,
                            ..
                        } => {
                            let proto = protocol.as_deref().unwrap_or("tcp");
                            if let Some(pub_port) = published {
                                format!("{}:{}/{}", pub_port, target, proto)
                            } else {
                                format!("{}/{}", target, proto)
                            }
                        }
                    })
                    .collect(),
            );
        }

        // Volumes
        if let Some(volumes) = docker_service.volumes {
            service.volumes = Some(
                volumes
                    .into_iter()
                    .map(|vol| match vol {
                        DockerComposeVolume::String(s) => s,
                        DockerComposeVolume::Long {
                            source,
                            target,
                            read_only,
                            ..
                        } => {
                            let mut volume_str =
                                format!("{}:{}", source.unwrap_or_default(), target);
                            if read_only == Some(true) {
                                volume_str.push_str(":ro");
                            }
                            volume_str
                        }
                    })
                    .collect(),
            );
        }

        // Networks
        service.networks = match docker_service.networks {
            Some(DockerComposeServiceNetworks::List(list)) => Some(list),
            Some(DockerComposeServiceNetworks::Map(map)) => Some(map.keys().cloned().collect()),
            None => None,
        };

        // Dependencies
        service.depends_on = docker_service.depends_on;

        // Resource limits
        if let Some(cpus) = docker_service.cpus {
            service.cpu_limit = Some(cpus);
        }
        if let Some(mem) = docker_service.mem_limit {
            service.memory_limit = Some(mem);
        }

        // Gaming configuration from Bolt-specific extensions
        if let Some(gaming_config) = docker_service.bolt_gaming {
            service.gaming = Some(GamingConfig {
                enabled: gaming_config.enabled.unwrap_or(false),
                gpu_passthrough: gaming_config.gpu_passthrough.unwrap_or(false),
                nvidia_runtime: gaming_config.nvidia_runtime.unwrap_or(false),
                amd_runtime: gaming_config.amd_runtime.unwrap_or(false),
                audio_passthrough: gaming_config.audio_passthrough.unwrap_or(false),
                real_time_priority: gaming_config.real_time_priority.unwrap_or(false),
                wine_prefix: gaming_config
                    .wine_config
                    .as_ref()
                    .and_then(|w| w.prefix.clone()),
                proton_version: gaming_config
                    .wine_config
                    .as_ref()
                    .and_then(|w| w.proton_version.clone()),
                dxvk_enabled: gaming_config.wine_config.as_ref().and_then(|w| w.dxvk),
                esync_enabled: gaming_config.wine_config.as_ref().and_then(|w| w.esync),
                fsync_enabled: gaming_config.wine_config.as_ref().and_then(|w| w.fsync),
                performance_profile: gaming_config.performance_profile,
                input_devices: gaming_config.input_devices,
                display_driver: gaming_config
                    .display_config
                    .as_ref()
                    .and_then(|d| d.driver.clone()),
                resolution: gaming_config
                    .display_config
                    .as_ref()
                    .and_then(|d| d.resolution.clone()),
                refresh_rate: gaming_config
                    .display_config
                    .as_ref()
                    .and_then(|d| d.refresh_rate),
                vsync: gaming_config.display_config.as_ref().and_then(|d| d.vsync),
                gpu: None,
                audio: None,
                wine: None,
                performance: None,
            });
        }

        // Device mappings (for GPU passthrough)
        if let Some(devices) = docker_service.devices {
            service.devices = Some(devices);
        }

        // Capabilities
        service.cap_add = docker_service.cap_add;
        service.cap_drop = docker_service.cap_drop;

        // Security options
        service.security_opt = docker_service.security_opt;

        // Sysctls
        service.sysctls = docker_service.sysctls;

        // Tmpfs
        if let Some(tmpfs) = docker_service.tmpfs {
            service.tmpfs = Some(match tmpfs {
                StringOrArray::String(s) => vec![s],
                StringOrArray::Array(arr) => arr,
            });
        }

        // DNS configuration
        if let Some(dns) = docker_service.dns {
            service.dns = Some(match dns {
                StringOrArray::String(s) => vec![s],
                StringOrArray::Array(arr) => arr,
            });
        }
        service.dns_search = docker_service.dns_search;

        // Extra hosts
        service.extra_hosts = docker_service.extra_hosts;

        // Group add
        service.group_add = docker_service.group_add;

        // Volumes from
        service.volumes_from = docker_service.volumes_from;

        // Links (deprecated but supported)
        service.links = docker_service.external_links;

        // Logging configuration
        if let Some(logging) = docker_service.logging {
            service.logging = Some(crate::config::LoggingConfig {
                driver: logging.driver.unwrap_or_default(),
                options: logging.options.unwrap_or_default(),
            });
        }

        // Health check
        if let Some(healthcheck) = docker_service.healthcheck {
            service.healthcheck = Some(crate::config::HealthcheckConfig {
                test: Self::convert_string_or_array(healthcheck.test).unwrap_or_default(),
                interval: healthcheck.interval,
                timeout: healthcheck.timeout,
                retries: healthcheck.retries,
                start_period: healthcheck.start_period,
                disable: healthcheck.disable.unwrap_or(false),
            });
        }

        Ok(service)
    }

    /// Convert network configuration
    fn convert_network(docker_network: DockerComposeNetwork) -> NetworkConfig {
        NetworkConfig {
            driver: docker_network.driver.unwrap_or("bridge".to_string()),
            driver_opts: docker_network.driver_opts,
            attachable: docker_network.attachable,
            enable_ipv6: docker_network.enable_ipv6,
            internal: docker_network.internal,
            labels: docker_network.labels,
            ipam: docker_network.ipam.map(|ipam| crate::config::IpamConfig {
                driver: ipam.driver,
                config: ipam.config.map(|configs| {
                    configs
                        .into_iter()
                        .map(|config| crate::config::IpamSubnetConfig {
                            subnet: config.subnet,
                            ip_range: config.ip_range,
                            gateway: config.gateway,
                            aux_addresses: config.aux_addresses,
                        })
                        .collect()
                }),
                options: ipam.options,
            }),
            external: docker_network.external.map(|ext| match ext {
                DockerComposeExternal::Bool(b) => b,
                DockerComposeExternal::Named { .. } => true,
            }),
            name: docker_network.name,
        }
    }

    /// Convert volume configuration
    fn convert_volume(docker_volume: DockerComposeVolumeSpec) -> VolumeConfig {
        match docker_volume {
            DockerComposeVolumeSpec::Simple(_) => VolumeConfig::default(),
            DockerComposeVolumeSpec::External(ext) => VolumeConfig {
                external: Some(match ext {
                    DockerComposeExternal::Bool(b) => b,
                    DockerComposeExternal::Named { .. } => true,
                }),
                ..Default::default()
            },
            DockerComposeVolumeSpec::Named {
                driver,
                driver_opts,
                external,
                labels,
                name,
            } => VolumeConfig {
                driver,
                driver_opts,
                external: external.map(|ext| match ext {
                    DockerComposeExternal::Bool(b) => b,
                    DockerComposeExternal::Named { .. } => true,
                }),
                labels,
                name,
            },
        }
    }

    /// Convert StringOrArray to Option<Vec<String>>
    fn convert_string_or_array(input: Option<StringOrArray>) -> Option<Vec<String>> {
        match input {
            Some(StringOrArray::String(s)) => Some(vec![s]),
            Some(StringOrArray::Array(arr)) => Some(arr),
            None => None,
        }
    }

    /// Convert Bolt service back to Docker Compose service
    pub fn convert_to_compose_service(bolt_service: &Service) -> DockerComposeService {
        DockerComposeService {
            image: bolt_service.image.clone(),
            build: bolt_service
                .build
                .as_ref()
                .map(|build_context| DockerComposeBuild {
                    context: Some(build_context.clone()),
                    dockerfile: None,
                    args: None,
                    cache_from: None,
                    labels: None,
                    network: None,
                    shm_size: None,
                    target: None,
                    extra_hosts: None,
                    isolation: None,
                    privileged: None,
                    pull: None,
                }),
            command: bolt_service
                .command
                .as_ref()
                .map(|cmd| StringOrArray::Array(cmd.clone())),
            entrypoint: bolt_service
                .entrypoint
                .as_ref()
                .map(|ep| StringOrArray::Array(ep.clone())),
            environment: bolt_service
                .environment
                .as_ref()
                .map(|env| Environment::Map(env.clone())),
            env_file: None,
            ports: bolt_service.ports.as_ref().map(|ports| {
                ports
                    .iter()
                    .map(|port| DockerComposePort::String(port.clone()))
                    .collect()
            }),
            expose: None,
            volumes: bolt_service.volumes.as_ref().map(|volumes| {
                volumes
                    .iter()
                    .map(|vol| DockerComposeVolume::String(vol.clone()))
                    .collect()
            }),
            networks: bolt_service
                .networks
                .as_ref()
                .map(|nets| DockerComposeServiceNetworks::List(nets.clone())),
            depends_on: bolt_service.depends_on.clone(),
            external_links: None,
            restart: bolt_service.restart.clone(),
            container_name: bolt_service.container_name.clone(),
            hostname: bolt_service.hostname.clone(),
            domainname: None,
            user: bolt_service.user.clone(),
            working_dir: bolt_service.working_dir.clone(),
            cap_add: bolt_service.cap_add.clone(),
            cap_drop: bolt_service.cap_drop.clone(),
            cgroup_parent: None,
            devices: bolt_service.devices.clone(),
            device_cgroup_rules: None,
            dns: bolt_service
                .dns
                .as_ref()
                .map(|dns| StringOrArray::Array(dns.clone())),
            dns_search: bolt_service.dns_search.clone(),
            tmpfs: bolt_service
                .tmpfs
                .as_ref()
                .map(|tmpfs| StringOrArray::Array(tmpfs.clone())),
            extra_hosts: bolt_service.extra_hosts.clone(),
            group_add: bolt_service.group_add.clone(),
            ipc: bolt_service.ipc.clone(),
            isolation: None,
            labels: bolt_service.labels.clone(),
            logging: bolt_service
                .logging
                .as_ref()
                .map(|log| DockerComposeLogging {
                    driver: Some(log.driver.clone()),
                    options: Some(log.options.clone()),
                }),
            network_mode: bolt_service.network_mode.clone(),
            pid: bolt_service.pid.clone(),
            platform: bolt_service.platform.clone(),
            privileged: bolt_service.privileged,
            read_only: bolt_service.read_only,
            security_opt: bolt_service.security_opt.clone(),
            shm_size: None,
            stdin_open: bolt_service.stdin_open,
            stop_grace_period: None,
            stop_signal: None,
            sysctls: bolt_service.sysctls.clone(),
            tty: bolt_service.tty,
            ulimits: None,
            userns_mode: None,
            volumes_from: bolt_service.volumes_from.clone(),
            cpu_count: None,
            cpu_percent: None,
            cpu_shares: None,
            cpu_period: None,
            cpu_quota: None,
            cpu_rt_period: None,
            cpu_rt_runtime: None,
            cpuset: None,
            cpus: bolt_service.cpu_limit.clone(),
            mem_limit: bolt_service.memory_limit.clone(),
            mem_reservation: None,
            mem_swappiness: None,
            memswap_limit: None,
            oom_kill_disable: None,
            oom_score_adj: None,
            pids_limit: None,
            blkio_config: None,
            deploy: None,
            healthcheck: bolt_service
                .healthcheck
                .as_ref()
                .map(|hc| DockerComposeHealthcheck {
                    test: Some(StringOrArray::Array(hc.test.clone())),
                    interval: hc.interval.clone(),
                    timeout: hc.timeout.clone(),
                    retries: hc.retries,
                    start_period: hc.start_period.clone(),
                    disable: Some(hc.disable),
                }),
            bolt_gaming: bolt_service
                .gaming
                .as_ref()
                .map(|gaming| DockerComposeBoltGaming {
                    enabled: Some(gaming.enabled),
                    gpu_passthrough: Some(gaming.gpu_passthrough),
                    nvidia_runtime: Some(gaming.nvidia_runtime),
                    amd_runtime: Some(gaming.amd_runtime),
                    audio_passthrough: Some(gaming.audio_passthrough),
                    real_time_priority: Some(gaming.real_time_priority),
                    wine_config: Some(DockerComposeBoltWineConfig {
                        prefix: gaming.wine_prefix.clone(),
                        version: None,
                        proton: Some(gaming.proton_version.is_some()),
                        proton_version: gaming.proton_version.clone(),
                        dxvk: gaming.dxvk_enabled,
                        vkd3d: None,
                        esync: gaming.esync_enabled,
                        fsync: gaming.fsync_enabled,
                    }),
                    performance_profile: gaming.performance_profile.clone(),
                    input_devices: gaming.input_devices.clone(),
                    display_config: Some(DockerComposeBoltDisplayConfig {
                        driver: gaming.display_driver.clone(),
                        resolution: gaming.resolution.clone(),
                        refresh_rate: gaming.refresh_rate,
                        full_screen: None,
                        vsync: gaming.vsync,
                    }),
                }),
        }
    }

    /// Convert BoltFile back to Docker Compose format
    pub fn convert_to_compose(bolt_file: &BoltFile) -> DockerCompose {
        let services = bolt_file
            .services
            .iter()
            .map(|(name, service)| (name.clone(), Self::convert_to_compose_service(service)))
            .collect();

        let networks = bolt_file.networks.as_ref().map(|networks| {
            networks
                .iter()
                .map(|(name, network)| {
                    (
                        name.clone(),
                        DockerComposeNetwork {
                            driver: Some(network.driver.clone()),
                            driver_opts: network.driver_opts.clone(),
                            attachable: network.attachable,
                            enable_ipv6: network.enable_ipv6,
                            ipam: network.ipam.as_ref().map(|ipam| DockerComposeIpam {
                                driver: ipam.driver.clone(),
                                config: ipam.config.as_ref().map(|configs| {
                                    configs
                                        .iter()
                                        .map(|config| DockerComposeIpamConfig {
                                            subnet: config.subnet.clone(),
                                            ip_range: config.ip_range.clone(),
                                            gateway: config.gateway.clone(),
                                            aux_addresses: config.aux_addresses.clone(),
                                        })
                                        .collect()
                                }),
                                options: ipam.options.clone(),
                            }),
                            internal: network.internal,
                            labels: network.labels.clone(),
                            external: network.external.map(|ext| DockerComposeExternal::Bool(ext)),
                            name: network.name.clone(),
                        },
                    )
                })
                .collect()
        });

        let volumes = bolt_file.volumes.as_ref().map(|volumes| {
            volumes
                .iter()
                .map(|(name, volume)| {
                    (
                        name.clone(),
                        DockerComposeVolumeSpec::Named {
                            driver: volume.driver.clone(),
                            driver_opts: volume.driver_opts.clone(),
                            external: volume.external.map(|ext| DockerComposeExternal::Bool(ext)),
                            labels: volume.labels.clone(),
                            name: volume.name.clone(),
                        },
                    )
                })
                .collect()
        });

        DockerCompose {
            version: Some("3.8".to_string()),
            services,
            networks,
            volumes,
            configs: None,
            secrets: None,
            bolt_gaming: None,
        }
    }

    /// Write Docker Compose to YAML
    pub fn write_compose_file<P: AsRef<Path>>(compose: &DockerCompose, path: P) -> Result<()> {
        let yaml_content = serde_yaml::to_string(compose).map_err(|e| {
            BoltError::Config(crate::error::ConfigError::InvalidFormat {
                reason: format!("Failed to serialize compose to YAML: {}", e),
            })
        })?;

        std::fs::write(path, yaml_content).map_err(|e| {
            BoltError::Config(crate::error::ConfigError::InvalidFormat {
                reason: format!("Failed to write compose file: {}", e),
            })
        })?;

        Ok(())
    }

    /// Validate Docker Compose file
    pub fn validate_compose(compose: &DockerCompose) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Check for deprecated features
        for (service_name, service) in &compose.services {
            if service.external_links.is_some() {
                warnings.push(format!(
                    "Service '{}': 'links' is deprecated, use networks instead",
                    service_name
                ));
            }

            if service.external_links.is_some() {
                warnings.push(format!(
                    "Service '{}': 'external_links' is deprecated",
                    service_name
                ));
            }

            if service.volumes_from.is_some() {
                warnings.push(format!(
                    "Service '{}': 'volumes_from' is deprecated, use named volumes instead",
                    service_name
                ));
            }
        }

        // Check for missing required fields
        for (service_name, service) in &compose.services {
            if service.image.is_none() && service.build.is_none() {
                return Err(BoltError::Config(
                    crate::error::ConfigError::InvalidFormat {
                        reason: format!(
                            "Service '{}' must have either 'image' or 'build' specified",
                            service_name
                        ),
                    },
                ));
            }
        }

        Ok(warnings)
    }
}
