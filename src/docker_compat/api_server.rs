use crate::BoltRuntime;
use crate::error::{BoltError, Result};
use crate::types::{ContainerInfo, NetworkInfo};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use warp::{Filter, Rejection, Reply};

// Convert BoltError to warp::Rejection
impl warp::reject::Reject for BoltError {}

/// Docker API server implementation for complete compatibility
pub struct DockerAPIServer {
    runtime: Arc<BoltRuntime>,
    bind_address: String,
    port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DockerVersion {
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "ApiVersion")]
    pub api_version: String,
    #[serde(rename = "MinAPIVersion")]
    pub min_api_version: String,
    #[serde(rename = "GitCommit")]
    pub git_commit: String,
    #[serde(rename = "GoVersion")]
    pub go_version: String,
    #[serde(rename = "Os")]
    pub os: String,
    #[serde(rename = "Arch")]
    pub arch: String,
    #[serde(rename = "BuildTime")]
    pub build_time: String,
    #[serde(rename = "Experimental")]
    pub experimental: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DockerInfo {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Containers")]
    pub containers: u32,
    #[serde(rename = "ContainersRunning")]
    pub containers_running: u32,
    #[serde(rename = "ContainersPaused")]
    pub containers_paused: u32,
    #[serde(rename = "ContainersStopped")]
    pub containers_stopped: u32,
    #[serde(rename = "Images")]
    pub images: u32,
    #[serde(rename = "Driver")]
    pub driver: String,
    #[serde(rename = "DriverStatus")]
    pub driver_status: Vec<Vec<String>>,
    #[serde(rename = "SystemStatus")]
    pub system_status: Option<Value>,
    #[serde(rename = "Plugins")]
    pub plugins: DockerPlugins,
    #[serde(rename = "MemoryLimit")]
    pub memory_limit: bool,
    #[serde(rename = "SwapLimit")]
    pub swap_limit: bool,
    #[serde(rename = "CpuCfsPeriod")]
    pub cpu_cfs_period: bool,
    #[serde(rename = "CpuCfsQuota")]
    pub cpu_cfs_quota: bool,
    #[serde(rename = "CPUShares")]
    pub cpu_shares: bool,
    #[serde(rename = "CPUSet")]
    pub cpu_set: bool,
    #[serde(rename = "PidsLimit")]
    pub pids_limit: bool,
    #[serde(rename = "OomKillDisable")]
    pub oom_kill_disable: bool,
    #[serde(rename = "IPv4Forwarding")]
    pub ipv4_forwarding: bool,
    #[serde(rename = "BridgeNfIptables")]
    pub bridge_nf_iptables: bool,
    #[serde(rename = "BridgeNfIp6tables")]
    pub bridge_nf_ip6tables: bool,
    #[serde(rename = "Debug")]
    pub debug: bool,
    #[serde(rename = "NFd")]
    pub nfd: u32,
    #[serde(rename = "NGoroutines")]
    pub ngoroutines: u32,
    #[serde(rename = "SystemTime")]
    pub system_time: String,
    #[serde(rename = "LoggingDriver")]
    pub logging_driver: String,
    #[serde(rename = "CgroupDriver")]
    pub cgroup_driver: String,
    #[serde(rename = "NEventsListener")]
    pub nevents_listener: u32,
    #[serde(rename = "KernelVersion")]
    pub kernel_version: String,
    #[serde(rename = "OperatingSystem")]
    pub operating_system: String,
    #[serde(rename = "OSType")]
    pub os_type: String,
    #[serde(rename = "Architecture")]
    pub architecture: String,
    #[serde(rename = "NCPU")]
    pub ncpu: u32,
    #[serde(rename = "MemTotal")]
    pub mem_total: u64,
    #[serde(rename = "DockerRootDir")]
    pub docker_root_dir: String,
    #[serde(rename = "HttpProxy")]
    pub http_proxy: String,
    #[serde(rename = "HttpsProxy")]
    pub https_proxy: String,
    #[serde(rename = "NoProxy")]
    pub no_proxy: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Labels")]
    pub labels: Vec<String>,
    #[serde(rename = "ExperimentalBuild")]
    pub experimental_build: bool,
    #[serde(rename = "ServerVersion")]
    pub server_version: String,
    #[serde(rename = "Runtimes")]
    pub runtimes: HashMap<String, DockerRuntime>,
    #[serde(rename = "DefaultRuntime")]
    pub default_runtime: String,
    #[serde(rename = "Swarm")]
    pub swarm: DockerSwarmInfo,
    #[serde(rename = "LiveRestoreEnabled")]
    pub live_restore_enabled: bool,
    #[serde(rename = "Isolation")]
    pub isolation: String,
    #[serde(rename = "InitBinary")]
    pub init_binary: String,
    #[serde(rename = "ContainerdCommit")]
    pub containerd_commit: DockerCommit,
    #[serde(rename = "RuncCommit")]
    pub runc_commit: DockerCommit,
    #[serde(rename = "InitCommit")]
    pub init_commit: DockerCommit,
    #[serde(rename = "SecurityOptions")]
    pub security_options: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DockerPlugins {
    #[serde(rename = "Volume")]
    pub volume: Vec<String>,
    #[serde(rename = "Network")]
    pub network: Vec<String>,
    #[serde(rename = "Authorization")]
    pub authorization: Option<Vec<String>>,
    #[serde(rename = "Log")]
    pub log: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DockerRuntime {
    pub path: String,
    #[serde(rename = "runtimeArgs")]
    pub runtime_args: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DockerSwarmInfo {
    #[serde(rename = "NodeID")]
    pub node_id: String,
    #[serde(rename = "NodeAddr")]
    pub node_addr: String,
    #[serde(rename = "LocalNodeState")]
    pub local_node_state: String,
    #[serde(rename = "ControlAvailable")]
    pub control_available: bool,
    #[serde(rename = "Error")]
    pub error: String,
    #[serde(rename = "RemoteManagers")]
    pub remote_managers: Option<Vec<DockerPeer>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DockerPeer {
    #[serde(rename = "NodeID")]
    pub node_id: String,
    #[serde(rename = "Addr")]
    pub addr: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DockerCommit {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Expected")]
    pub expected: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerCreateRequest {
    #[serde(rename = "Image")]
    pub image: String,
    #[serde(rename = "Cmd")]
    pub cmd: Option<Vec<String>>,
    #[serde(rename = "Env")]
    pub env: Option<Vec<String>>,
    #[serde(rename = "ExposedPorts")]
    pub exposed_ports: Option<HashMap<String, Value>>,
    #[serde(rename = "HostConfig")]
    pub host_config: Option<HostConfig>,
    #[serde(rename = "NetworkingConfig")]
    pub networking_config: Option<NetworkingConfig>,
    #[serde(rename = "Hostname")]
    pub hostname: Option<String>,
    #[serde(rename = "Domainname")]
    pub domainname: Option<String>,
    #[serde(rename = "User")]
    pub user: Option<String>,
    #[serde(rename = "AttachStdin")]
    pub attach_stdin: Option<bool>,
    #[serde(rename = "AttachStdout")]
    pub attach_stdout: Option<bool>,
    #[serde(rename = "AttachStderr")]
    pub attach_stderr: Option<bool>,
    #[serde(rename = "Tty")]
    pub tty: Option<bool>,
    #[serde(rename = "OpenStdin")]
    pub open_stdin: Option<bool>,
    #[serde(rename = "StdinOnce")]
    pub stdin_once: Option<bool>,
    #[serde(rename = "WorkingDir")]
    pub working_dir: Option<String>,
    #[serde(rename = "Entrypoint")]
    pub entrypoint: Option<Vec<String>>,
    #[serde(rename = "Labels")]
    pub labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HostConfig {
    #[serde(rename = "Binds")]
    pub binds: Option<Vec<String>>,
    #[serde(rename = "ContainerIDFile")]
    pub container_id_file: Option<String>,
    #[serde(rename = "LogConfig")]
    pub log_config: Option<LogConfig>,
    #[serde(rename = "NetworkMode")]
    pub network_mode: Option<String>,
    #[serde(rename = "PortBindings")]
    pub port_bindings: Option<HashMap<String, Vec<PortBinding>>>,
    #[serde(rename = "RestartPolicy")]
    pub restart_policy: Option<RestartPolicy>,
    #[serde(rename = "AutoRemove")]
    pub auto_remove: Option<bool>,
    #[serde(rename = "VolumeDriver")]
    pub volume_driver: Option<String>,
    #[serde(rename = "VolumesFrom")]
    pub volumes_from: Option<Vec<String>>,
    #[serde(rename = "Mounts")]
    pub mounts: Option<Vec<Mount>>,
    #[serde(rename = "CapAdd")]
    pub cap_add: Option<Vec<String>>,
    #[serde(rename = "CapDrop")]
    pub cap_drop: Option<Vec<String>>,
    #[serde(rename = "CgroupnsMode")]
    pub cgroupns_mode: Option<String>,
    #[serde(rename = "Dns")]
    pub dns: Option<Vec<String>>,
    #[serde(rename = "DnsOptions")]
    pub dns_options: Option<Vec<String>>,
    #[serde(rename = "DnsSearch")]
    pub dns_search: Option<Vec<String>>,
    #[serde(rename = "ExtraHosts")]
    pub extra_hosts: Option<Vec<String>>,
    #[serde(rename = "GroupAdd")]
    pub group_add: Option<Vec<String>>,
    #[serde(rename = "IpcMode")]
    pub ipc_mode: Option<String>,
    #[serde(rename = "Cgroup")]
    pub cgroup: Option<String>,
    #[serde(rename = "Links")]
    pub links: Option<Vec<String>>,
    #[serde(rename = "OomScoreAdj")]
    pub oom_score_adj: Option<i32>,
    #[serde(rename = "PidMode")]
    pub pid_mode: Option<String>,
    #[serde(rename = "Privileged")]
    pub privileged: Option<bool>,
    #[serde(rename = "PublishAllPorts")]
    pub publish_all_ports: Option<bool>,
    #[serde(rename = "ReadonlyRootfs")]
    pub readonly_rootfs: Option<bool>,
    #[serde(rename = "SecurityOpt")]
    pub security_opt: Option<Vec<String>>,
    #[serde(rename = "StorageOpt")]
    pub storage_opt: Option<HashMap<String, String>>,
    #[serde(rename = "Tmpfs")]
    pub tmpfs: Option<HashMap<String, String>>,
    #[serde(rename = "UTSMode")]
    pub uts_mode: Option<String>,
    #[serde(rename = "UsernsMode")]
    pub userns_mode: Option<String>,
    #[serde(rename = "ShmSize")]
    pub shm_size: Option<u64>,
    #[serde(rename = "Sysctls")]
    pub sysctls: Option<HashMap<String, String>>,
    #[serde(rename = "Runtime")]
    pub runtime: Option<String>,
    #[serde(rename = "ConsoleSize")]
    pub console_size: Option<Vec<u32>>,
    #[serde(rename = "Isolation")]
    pub isolation: Option<String>,
    #[serde(rename = "CpuShares")]
    pub cpu_shares: Option<u64>,
    #[serde(rename = "Memory")]
    pub memory: Option<u64>,
    #[serde(rename = "NanoCpus")]
    pub nano_cpus: Option<u64>,
    #[serde(rename = "CgroupParent")]
    pub cgroup_parent: Option<String>,
    #[serde(rename = "BlkioWeight")]
    pub blkio_weight: Option<u16>,
    #[serde(rename = "BlkioWeightDevice")]
    pub blkio_weight_device: Option<Vec<WeightDevice>>,
    #[serde(rename = "BlkioDeviceReadBps")]
    pub blkio_device_read_bps: Option<Vec<ThrottleDevice>>,
    #[serde(rename = "BlkioDeviceWriteBps")]
    pub blkio_device_write_bps: Option<Vec<ThrottleDevice>>,
    #[serde(rename = "BlkioDeviceReadIOps")]
    pub blkio_device_read_iops: Option<Vec<ThrottleDevice>>,
    #[serde(rename = "BlkioDeviceWriteIOps")]
    pub blkio_device_write_iops: Option<Vec<ThrottleDevice>>,
    #[serde(rename = "CpuPeriod")]
    pub cpu_period: Option<u64>,
    #[serde(rename = "CpuQuota")]
    pub cpu_quota: Option<u64>,
    #[serde(rename = "CpuRealtimePeriod")]
    pub cpu_realtime_period: Option<u64>,
    #[serde(rename = "CpuRealtimeRuntime")]
    pub cpu_realtime_runtime: Option<u64>,
    #[serde(rename = "CpusetCpus")]
    pub cpuset_cpus: Option<String>,
    #[serde(rename = "CpusetMems")]
    pub cpuset_mems: Option<String>,
    #[serde(rename = "Devices")]
    pub devices: Option<Vec<DeviceMapping>>,
    #[serde(rename = "DeviceCgroupRules")]
    pub device_cgroup_rules: Option<Vec<String>>,
    #[serde(rename = "DeviceRequests")]
    pub device_requests: Option<Vec<DeviceRequest>>,
    #[serde(rename = "KernelMemory")]
    pub kernel_memory: Option<u64>,
    #[serde(rename = "KernelMemoryTCP")]
    pub kernel_memory_tcp: Option<u64>,
    #[serde(rename = "MemoryReservation")]
    pub memory_reservation: Option<u64>,
    #[serde(rename = "MemorySwap")]
    pub memory_swap: Option<u64>,
    #[serde(rename = "MemorySwappiness")]
    pub memory_swappiness: Option<u64>,
    #[serde(rename = "OomKillDisable")]
    pub oom_kill_disable: Option<bool>,
    #[serde(rename = "PidsLimit")]
    pub pids_limit: Option<u64>,
    #[serde(rename = "Ulimits")]
    pub ulimits: Option<Vec<Ulimit>>,
    #[serde(rename = "CpuCount")]
    pub cpu_count: Option<u64>,
    #[serde(rename = "CpuPercent")]
    pub cpu_percent: Option<u64>,
    #[serde(rename = "IOMaximumIOps")]
    pub io_maximum_iops: Option<u64>,
    #[serde(rename = "IOMaximumBandwidth")]
    pub io_maximum_bandwidth: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogConfig {
    #[serde(rename = "Type")]
    pub log_type: String,
    #[serde(rename = "Config")]
    pub config: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PortBinding {
    #[serde(rename = "HostIp")]
    pub host_ip: Option<String>,
    #[serde(rename = "HostPort")]
    pub host_port: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RestartPolicy {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "MaximumRetryCount")]
    pub maximum_retry_count: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Mount {
    #[serde(rename = "Target")]
    pub target: String,
    #[serde(rename = "Source")]
    pub source: String,
    #[serde(rename = "Type")]
    pub mount_type: String,
    #[serde(rename = "ReadOnly")]
    pub read_only: Option<bool>,
    #[serde(rename = "Consistency")]
    pub consistency: Option<String>,
    #[serde(rename = "BindOptions")]
    pub bind_options: Option<BindOptions>,
    #[serde(rename = "VolumeOptions")]
    pub volume_options: Option<VolumeOptions>,
    #[serde(rename = "TmpfsOptions")]
    pub tmpfs_options: Option<TmpfsOptions>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BindOptions {
    #[serde(rename = "Propagation")]
    pub propagation: Option<String>,
    #[serde(rename = "NonRecursive")]
    pub non_recursive: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VolumeOptions {
    #[serde(rename = "NoCopy")]
    pub no_copy: Option<bool>,
    #[serde(rename = "Labels")]
    pub labels: Option<HashMap<String, String>>,
    #[serde(rename = "DriverConfig")]
    pub driver_config: Option<Driver>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Driver {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Options")]
    pub options: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TmpfsOptions {
    #[serde(rename = "SizeBytes")]
    pub size_bytes: Option<u64>,
    #[serde(rename = "Mode")]
    pub mode: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WeightDevice {
    #[serde(rename = "Path")]
    pub path: String,
    #[serde(rename = "Weight")]
    pub weight: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThrottleDevice {
    #[serde(rename = "Path")]
    pub path: String,
    #[serde(rename = "Rate")]
    pub rate: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceMapping {
    #[serde(rename = "PathOnHost")]
    pub path_on_host: String,
    #[serde(rename = "PathInContainer")]
    pub path_in_container: String,
    #[serde(rename = "CgroupPermissions")]
    pub cgroup_permissions: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceRequest {
    #[serde(rename = "Driver")]
    pub driver: Option<String>,
    #[serde(rename = "Count")]
    pub count: Option<i32>,
    #[serde(rename = "DeviceIDs")]
    pub device_ids: Option<Vec<String>>,
    #[serde(rename = "Capabilities")]
    pub capabilities: Option<Vec<Vec<String>>>,
    #[serde(rename = "Options")]
    pub options: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ulimit {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Soft")]
    pub soft: u64,
    #[serde(rename = "Hard")]
    pub hard: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkingConfig {
    #[serde(rename = "EndpointsConfig")]
    pub endpoints_config: HashMap<String, EndpointSettings>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EndpointSettings {
    #[serde(rename = "IPAMConfig")]
    pub ipam_config: Option<EndpointIPAMConfig>,
    #[serde(rename = "Links")]
    pub links: Option<Vec<String>>,
    #[serde(rename = "Aliases")]
    pub aliases: Option<Vec<String>>,
    #[serde(rename = "NetworkID")]
    pub network_id: Option<String>,
    #[serde(rename = "EndpointID")]
    pub endpoint_id: Option<String>,
    #[serde(rename = "Gateway")]
    pub gateway: Option<String>,
    #[serde(rename = "IPAddress")]
    pub ip_address: Option<String>,
    #[serde(rename = "IPPrefixLen")]
    pub ip_prefix_len: Option<u32>,
    #[serde(rename = "IPv6Gateway")]
    pub ipv6_gateway: Option<String>,
    #[serde(rename = "GlobalIPv6Address")]
    pub global_ipv6_address: Option<String>,
    #[serde(rename = "GlobalIPv6PrefixLen")]
    pub global_ipv6_prefix_len: Option<u32>,
    #[serde(rename = "MacAddress")]
    pub mac_address: Option<String>,
    #[serde(rename = "DriverOpts")]
    pub driver_opts: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EndpointIPAMConfig {
    #[serde(rename = "IPv4Address")]
    pub ipv4_address: Option<String>,
    #[serde(rename = "IPv6Address")]
    pub ipv6_address: Option<String>,
    #[serde(rename = "LinkLocalIPs")]
    pub link_local_ips: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerCreateResponse {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Warnings")]
    pub warnings: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerStartRequest {
    #[serde(rename = "DetachKeys")]
    pub detach_keys: Option<String>,
}

impl DockerAPIServer {
    pub fn new(runtime: Arc<BoltRuntime>) -> Self {
        Self {
            runtime,
            bind_address: "127.0.0.1".to_string(),
            port: 2375,
        }
    }

    pub fn with_address(mut self, address: String, port: u16) -> Self {
        self.bind_address = address;
        self.port = port;
        self
    }

    pub async fn start(&self) -> Result<()> {
        tracing::info!(
            "🐳 Starting Docker API server on {}:{}",
            self.bind_address,
            self.port
        );

        let runtime = self.runtime.clone();

        // Version endpoint
        let version = warp::path("version").and(warp::get()).and_then(move || {
            let rt = runtime.clone();
            async move { Self::version_handler(rt).await }
        });

        // Info endpoint
        let runtime_clone = self.runtime.clone();
        let info = warp::path("info").and(warp::get()).and_then(move || {
            let rt = runtime_clone.clone();
            async move {
                Self::info_handler(rt)
                    .await
                    .map_err(|e| warp::reject::custom(e))
            }
        });

        // Container endpoints
        let runtime_clone = self.runtime.clone();
        let containers_list = warp::path!("containers" / "json")
            .and(warp::get())
            .and(warp::query::<HashMap<String, String>>())
            .and_then(move |params: HashMap<String, String>| {
                let rt = runtime_clone.clone();
                async move { Self::containers_list_handler(rt, params).await }
            });

        let runtime_clone = self.runtime.clone();
        let containers_create = warp::path!("containers" / "create")
            .and(warp::post())
            .and(warp::query::<HashMap<String, String>>())
            .and(warp::body::json())
            .and_then(
                move |params: HashMap<String, String>, body: ContainerCreateRequest| {
                    let rt = runtime_clone.clone();
                    async move { Self::containers_create_handler(rt, params, body).await }
                },
            );

        let runtime_clone = self.runtime.clone();
        let containers_start = warp::path!("containers" / String / "start")
            .and(warp::post())
            .and(warp::query::<HashMap<String, String>>())
            .and(warp::body::json())
            .and_then(
                move |id: String, params: HashMap<String, String>, body: ContainerStartRequest| {
                    let rt = runtime_clone.clone();
                    async move { Self::containers_start_handler(rt, id, params, body).await }
                },
            );

        let runtime_clone = self.runtime.clone();
        let containers_stop = warp::path!("containers" / String / "stop")
            .and(warp::post())
            .and(warp::query::<HashMap<String, String>>())
            .and_then(move |id: String, params: HashMap<String, String>| {
                let rt = runtime_clone.clone();
                async move { Self::containers_stop_handler(rt, id, params).await }
            });

        let runtime_clone = self.runtime.clone();
        let containers_remove = warp::path!("containers" / String)
            .and(warp::delete())
            .and(warp::query::<HashMap<String, String>>())
            .and_then(move |id: String, params: HashMap<String, String>| {
                let rt = runtime_clone.clone();
                async move { Self::containers_remove_handler(rt, id, params).await }
            });

        let runtime_clone = self.runtime.clone();
        let containers_inspect = warp::path!("containers" / String / "json")
            .and(warp::get())
            .and_then(move |id: String| {
                let rt = runtime_clone.clone();
                async move { Self::containers_inspect_handler(rt, id).await }
            });

        // Image endpoints
        let runtime_clone = self.runtime.clone();
        let images_list = warp::path!("images" / "json")
            .and(warp::get())
            .and(warp::query::<HashMap<String, String>>())
            .and_then(move |params: HashMap<String, String>| {
                let rt = runtime_clone.clone();
                async move { Self::images_list_handler(rt, params).await }
            });

        let runtime_clone = self.runtime.clone();
        let images_pull = warp::path!("images" / "create")
            .and(warp::post())
            .and(warp::query::<HashMap<String, String>>())
            .and_then(move |params: HashMap<String, String>| {
                let rt = runtime_clone.clone();
                async move { Self::images_pull_handler(rt, params).await }
            });

        let runtime_clone = self.runtime.clone();
        let images_push = warp::path!("images" / String / "push")
            .and(warp::post())
            .and(warp::query::<HashMap<String, String>>())
            .and_then(move |name: String, params: HashMap<String, String>| {
                let rt = runtime_clone.clone();
                async move { Self::images_push_handler(rt, name, params).await }
            });

        // Network endpoints
        let runtime_clone = self.runtime.clone();
        let networks_list = warp::path!("networks")
            .and(warp::get())
            .and(warp::query::<HashMap<String, String>>())
            .and_then(move |params: HashMap<String, String>| {
                let rt = runtime_clone.clone();
                async move { Self::networks_list_handler(rt, params).await }
            });

        // Combine all routes
        let api_routes = version
            .or(info)
            .or(containers_list)
            .or(containers_create)
            .or(containers_start)
            .or(containers_stop)
            .or(containers_remove)
            .or(containers_inspect)
            .or(images_list)
            .or(images_pull)
            .or(images_push)
            .or(networks_list);

        // Add CORS and logging
        let routes = api_routes
            .with(warp::cors().allow_any_origin())
            .with(warp::log("docker_api"));

        // Start server
        let addr = format!("{}:{}", self.bind_address, self.port);
        let socket_addr: std::net::SocketAddr = addr.parse().map_err(|e| {
            BoltError::Runtime(crate::error::RuntimeError::StartFailed {
                reason: format!("Invalid bind address: {}", e),
            })
        })?;

        tracing::info!("✅ Docker API server listening on http://{}", addr);
        warp::serve(routes).run(socket_addr).await;

        Ok(())
    }

    async fn version_handler(_runtime: Arc<BoltRuntime>) -> Result<impl Reply, Rejection> {
        let version = DockerVersion {
            version: "24.0.7".to_string(),
            api_version: "1.43".to_string(),
            min_api_version: "1.12".to_string(),
            git_commit: "bolt-runtime".to_string(),
            go_version: "go1.21.0".to_string(),
            os: "linux".to_string(),
            arch: "amd64".to_string(),
            build_time: chrono::Utc::now().to_rfc3339(),
            experimental: true,
        };

        Ok(warp::reply::json(&version))
    }

    async fn info_handler(_runtime: Arc<BoltRuntime>) -> Result<impl Reply> {
        let info = DockerInfo {
            id: "bolt-runtime".to_string(),
            containers: 0,
            containers_running: 0,
            containers_paused: 0,
            containers_stopped: 0,
            images: 0,
            driver: "bolt".to_string(),
            driver_status: vec![
                vec!["Backing Filesystem".to_string(), "extfs".to_string()],
                vec!["Supports d_type".to_string(), "true".to_string()],
                vec!["Native Overlay Diff".to_string(), "true".to_string()],
                vec!["userxattr".to_string(), "false".to_string()],
            ],
            system_status: None,
            plugins: DockerPlugins {
                volume: vec![
                    "local".to_string(),
                    "nfs".to_string(),
                    "overlay".to_string(),
                ],
                network: vec![
                    "bridge".to_string(),
                    "host".to_string(),
                    "overlay".to_string(),
                ],
                authorization: None,
                log: vec!["json-file".to_string(), "journald".to_string()],
            },
            memory_limit: true,
            swap_limit: true,
            cpu_cfs_period: true,
            cpu_cfs_quota: true,
            cpu_shares: true,
            cpu_set: true,
            pids_limit: true,
            oom_kill_disable: true,
            ipv4_forwarding: true,
            bridge_nf_iptables: true,
            bridge_nf_ip6tables: true,
            debug: false,
            nfd: 24,
            ngoroutines: 48,
            system_time: chrono::Utc::now().to_rfc3339(),
            logging_driver: "json-file".to_string(),
            cgroup_driver: "systemd".to_string(),
            nevents_listener: 0,
            kernel_version: "6.5.0".to_string(),
            operating_system: "Bolt Linux".to_string(),
            os_type: "linux".to_string(),
            architecture: "x86_64".to_string(),
            ncpu: num_cpus::get() as u32,
            mem_total: 8589934592, // 8GB
            docker_root_dir: "/var/lib/bolt".to_string(),
            http_proxy: "".to_string(),
            https_proxy: "".to_string(),
            no_proxy: "".to_string(),
            name: hostname::get()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            labels: vec![],
            experimental_build: true,
            server_version: "24.0.7".to_string(),
            runtimes: {
                let mut runtimes = HashMap::new();
                runtimes.insert(
                    "bolt".to_string(),
                    DockerRuntime {
                        path: "bolt-runtime".to_string(),
                        runtime_args: Some(vec!["--systemd-cgroup".to_string()]),
                    },
                );
                runtimes.insert(
                    "runc".to_string(),
                    DockerRuntime {
                        path: "runc".to_string(),
                        runtime_args: Some(vec!["--systemd-cgroup".to_string()]),
                    },
                );
                runtimes
            },
            default_runtime: "bolt".to_string(),
            swarm: DockerSwarmInfo {
                node_id: "".to_string(),
                node_addr: "".to_string(),
                local_node_state: "inactive".to_string(),
                control_available: false,
                error: "".to_string(),
                remote_managers: None,
            },
            live_restore_enabled: false,
            isolation: "".to_string(),
            init_binary: "bolt-init".to_string(),
            containerd_commit: DockerCommit {
                id: "bolt-containerd".to_string(),
                expected: "bolt-containerd".to_string(),
            },
            runc_commit: DockerCommit {
                id: "bolt-runc".to_string(),
                expected: "bolt-runc".to_string(),
            },
            init_commit: DockerCommit {
                id: "bolt-init".to_string(),
                expected: "bolt-init".to_string(),
            },
            security_options: vec![
                "name=seccomp,profile=default".to_string(),
                "name=cgroupns".to_string(),
                "name=selinux".to_string(),
                "name=userns".to_string(),
            ],
        };

        Ok(warp::reply::json(&info))
    }

    async fn containers_list_handler(
        runtime: Arc<BoltRuntime>,
        params: HashMap<String, String>,
    ) -> Result<impl Reply, Rejection> {
        let all = params
            .get("all")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        match runtime.list_containers(all).await {
            Ok(containers) => {
                // Convert Bolt containers to Docker API format
                let docker_containers: Vec<Value> = containers
                    .into_iter()
                    .map(|c| {
                        serde_json::json!({
                            "Id": c.id,
                            "Names": [format!("/{}", c.name)],
                            "Image": c.image,
                            "ImageID": c.image_id,
                            "Command": c.command,
                            "Created": c.created,
                            "Ports": [],
                            "Labels": c.labels,
                            "State": c.status,
                            "Status": format!("{} {}", c.status, c.uptime.as_deref().unwrap_or("Unknown")),
                            "HostConfig": {
                                "NetworkMode": "default"
                            },
                            "NetworkSettings": {
                                "Networks": {}
                            },
                            "Mounts": []
                        })
                    })
                    .collect();

                Ok(warp::reply::json(&docker_containers))
            }
            Err(e) => {
                tracing::error!("Failed to list containers: {}", e);
                Err(warp::reject::custom(e))
            }
        }
    }

    async fn containers_create_handler(
        runtime: Arc<BoltRuntime>,
        params: HashMap<String, String>,
        body: ContainerCreateRequest,
    ) -> Result<impl Reply, Rejection> {
        let name = params.get("name").cloned();

        // Convert Docker create request to Bolt format
        let ports = body
            .host_config
            .as_ref()
            .and_then(|hc| hc.port_bindings.as_ref())
            .map(|pb| {
                pb.iter()
                    .map(|(container_port, host_bindings)| {
                        if let Some(binding) = host_bindings.first() {
                            if let Some(host_port) = &binding.host_port {
                                format!(
                                    "{}:{}",
                                    host_port,
                                    container_port.split('/').next().unwrap_or(container_port)
                                )
                            } else {
                                container_port.clone()
                            }
                        } else {
                            container_port.clone()
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let env = body.env.unwrap_or_default();

        let volumes = body
            .host_config
            .as_ref()
            .and_then(|hc| hc.binds.as_ref())
            .cloned()
            .unwrap_or_default();

        // For create, we just prepare but don't start
        match runtime
            .run_container(
                &body.image,
                name.as_deref(),
                &ports,
                &env,
                &volumes,
                true, // Always detached for create
            )
            .await
        {
            Ok(_) => {
                // Generate a container ID (in real implementation, this would come from the runtime)
                let container_id = uuid::Uuid::new_v4().to_string().replace("-", "");

                let response = ContainerCreateResponse {
                    id: container_id,
                    warnings: None,
                };

                Ok(warp::reply::json(&response))
            }
            Err(e) => {
                tracing::error!("Failed to create container: {}", e);
                Err(warp::reject::custom(e))
            }
        }
    }

    async fn containers_start_handler(
        _runtime: Arc<BoltRuntime>,
        id: String,
        _params: HashMap<String, String>,
        _body: ContainerStartRequest,
    ) -> Result<impl Reply, Rejection> {
        tracing::info!("Starting container: {}", id);

        // In a real implementation, this would start the already created container
        // For now, return success
        Ok(warp::reply::with_status(
            "",
            warp::http::StatusCode::NO_CONTENT,
        ))
    }

    async fn containers_stop_handler(
        runtime: Arc<BoltRuntime>,
        id: String,
        params: HashMap<String, String>,
    ) -> Result<impl Reply, Rejection> {
        let _timeout = params
            .get("t")
            .and_then(|t| t.parse::<u64>().ok())
            .unwrap_or(10);

        match runtime.stop_container(&id).await {
            Ok(_) => Ok(warp::reply::with_status(
                "",
                warp::http::StatusCode::NO_CONTENT,
            )),
            Err(e) => {
                tracing::error!("Failed to stop container {}: {}", id, e);
                Err(warp::reject::custom(e))
            }
        }
    }

    async fn containers_remove_handler(
        runtime: Arc<BoltRuntime>,
        id: String,
        params: HashMap<String, String>,
    ) -> Result<impl Reply, Rejection> {
        let force = params
            .get("force")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        match runtime.remove_container(&id, force).await {
            Ok(_) => Ok(warp::reply::with_status(
                "",
                warp::http::StatusCode::NO_CONTENT,
            )),
            Err(e) => {
                tracing::error!("Failed to remove container {}: {}", id, e);
                Err(warp::reject::custom(e))
            }
        }
    }

    async fn containers_inspect_handler(
        _runtime: Arc<BoltRuntime>,
        id: String,
    ) -> Result<impl Reply, Rejection> {
        // Return a basic container inspection response
        let response = serde_json::json!({
            "Id": id,
            "Created": chrono::Utc::now().to_rfc3339(),
            "Path": "/bin/sh",
            "Args": [],
            "State": {
                "Status": "running",
                "Running": true,
                "Paused": false,
                "Restarting": false,
                "OOMKilled": false,
                "Dead": false,
                "Pid": 1234,
                "ExitCode": 0,
                "Error": "",
                "StartedAt": chrono::Utc::now().to_rfc3339(),
                "FinishedAt": "0001-01-01T00:00:00Z"
            },
            "Image": "sha256:example",
            "ResolvConfPath": "/var/lib/bolt/containers/resolv.conf",
            "HostnamePath": "/var/lib/bolt/containers/hostname",
            "HostsPath": "/var/lib/bolt/containers/hosts",
            "LogPath": "/var/lib/bolt/containers/container.log",
            "Name": format!("/{}", id),
            "RestartCount": 0,
            "Driver": "bolt",
            "Platform": "linux",
            "MountLabel": "",
            "ProcessLabel": "",
            "AppArmorProfile": "",
            "ExecIDs": null,
            "HostConfig": {
                "Binds": [],
                "ContainerIDFile": "",
                "LogConfig": {
                    "Type": "json-file",
                    "Config": {}
                },
                "NetworkMode": "default",
                "PortBindings": {},
                "RestartPolicy": {
                    "Name": "no",
                    "MaximumRetryCount": 0
                },
                "AutoRemove": false,
                "VolumeDriver": "",
                "VolumesFrom": null,
                "Mounts": [],
                "CapAdd": null,
                "CapDrop": null,
                "CgroupnsMode": "host",
                "Dns": [],
                "DnsOptions": [],
                "DnsSearch": [],
                "ExtraHosts": null,
                "GroupAdd": null,
                "IpcMode": "private",
                "Cgroup": "",
                "Links": null,
                "OomScoreAdj": 0,
                "PidMode": "",
                "Privileged": false,
                "PublishAllPorts": false,
                "ReadonlyRootfs": false,
                "SecurityOpt": null,
                "UTSMode": "",
                "UsernsMode": "",
                "ShmSize": 67108864,
                "Runtime": "bolt",
                "ConsoleSize": [0, 0],
                "Isolation": "",
                "CpuShares": 0,
                "Memory": 0,
                "NanoCpus": 0,
                "CgroupParent": "",
                "BlkioWeight": 0,
                "BlkioWeightDevice": [],
                "BlkioDeviceReadBps": null,
                "BlkioDeviceWriteBps": null,
                "BlkioDeviceReadIOps": null,
                "BlkioDeviceWriteIOps": null,
                "CpuPeriod": 0,
                "CpuQuota": 0,
                "CpuRealtimePeriod": 0,
                "CpuRealtimeRuntime": 0,
                "CpusetCpus": "",
                "CpusetMems": "",
                "Devices": [],
                "DeviceCgroupRules": null,
                "DeviceRequests": null,
                "KernelMemory": 0,
                "KernelMemoryTCP": 0,
                "MemoryReservation": 0,
                "MemorySwap": 0,
                "MemorySwappiness": null,
                "OomKillDisable": false,
                "PidsLimit": null,
                "Ulimits": null,
                "CpuCount": 0,
                "CpuPercent": 0,
                "IOMaximumIOps": 0,
                "IOMaximumBandwidth": 0
            },
            "GraphDriver": {
                "Data": {
                    "LowerDir": "",
                    "MergedDir": "",
                    "UpperDir": "",
                    "WorkDir": ""
                },
                "Name": "overlay2"
            },
            "Mounts": [],
            "Config": {
                "Hostname": id[..12].to_string(),
                "Domainname": "",
                "User": "",
                "AttachStdin": false,
                "AttachStdout": true,
                "AttachStderr": true,
                "Tty": false,
                "OpenStdin": false,
                "StdinOnce": false,
                "Env": [
                    "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
                ],
                "Cmd": ["/bin/sh"],
                "Image": "example:latest",
                "Volumes": null,
                "WorkingDir": "",
                "Entrypoint": null,
                "OnBuild": null,
                "Labels": {}
            },
            "NetworkSettings": {
                "Bridge": "",
                "SandboxID": "",
                "HairpinMode": false,
                "LinkLocalIPv6Address": "",
                "LinkLocalIPv6PrefixLen": 0,
                "Ports": {},
                "SandboxKey": "",
                "SecondaryIPAddresses": null,
                "SecondaryIPv6Addresses": null,
                "EndpointID": "",
                "Gateway": "",
                "GlobalIPv6Address": "",
                "GlobalIPv6PrefixLen": 0,
                "IPAddress": "",
                "IPPrefixLen": 0,
                "IPv6Gateway": "",
                "MacAddress": "",
                "Networks": {
                    "bridge": {
                        "IPAMConfig": null,
                        "Links": null,
                        "Aliases": null,
                        "NetworkID": "",
                        "EndpointID": "",
                        "Gateway": "",
                        "IPAddress": "",
                        "IPPrefixLen": 0,
                        "IPv6Gateway": "",
                        "GlobalIPv6Address": "",
                        "GlobalIPv6PrefixLen": 0,
                        "MacAddress": "",
                        "DriverOpts": null
                    }
                }
            }
        });

        Ok(warp::reply::json(&response))
    }

    async fn images_list_handler(
        _runtime: Arc<BoltRuntime>,
        _params: HashMap<String, String>,
    ) -> Result<impl Reply, Rejection> {
        // Return empty list for now
        let images: Vec<Value> = vec![];
        Ok(warp::reply::json(&images))
    }

    async fn images_pull_handler(
        runtime: Arc<BoltRuntime>,
        params: HashMap<String, String>,
    ) -> Result<impl Reply, Rejection> {
        if let Some(from_image) = params.get("fromImage") {
            match runtime.pull_image(from_image).await {
                Ok(_) => {
                    let response = serde_json::json!({
                        "status": "Pull complete",
                        "id": from_image
                    });
                    Ok(warp::reply::json(&response))
                }
                Err(e) => {
                    tracing::error!("Failed to pull image {}: {}", from_image, e);
                    Err(warp::reject::custom(DockerAPIError::Internal(
                        e.to_string(),
                    )))
                }
            }
        } else {
            Err(warp::reject::custom(DockerAPIError::BadRequest(
                "Missing fromImage parameter".to_string(),
            )))
        }
    }

    async fn images_push_handler(
        runtime: Arc<BoltRuntime>,
        name: String,
        _params: HashMap<String, String>,
    ) -> Result<impl Reply, Rejection> {
        match runtime.push_image(&name).await {
            Ok(_) => {
                let response = serde_json::json!({
                    "status": "Push complete",
                    "id": name
                });
                Ok(warp::reply::json(&response))
            }
            Err(e) => {
                tracing::error!("Failed to push image {}: {}", name, e);
                Err(warp::reject::custom(e))
            }
        }
    }

    async fn networks_list_handler(
        runtime: Arc<BoltRuntime>,
        _params: HashMap<String, String>,
    ) -> Result<impl Reply, Rejection> {
        match runtime.list_networks().await {
            Ok(networks) => {
                let docker_networks: Vec<Value> = networks
                    .into_iter()
                    .map(|n| {
                        serde_json::json!({
                            "Name": n.name,
                            "Id": n.id,
                            "Created": n.created,
                            "Scope": "local",
                            "Driver": n.driver,
                            "EnableIPv6": false,
                            "IPAM": {
                                "Driver": "default",
                                "Options": null,
                                "Config": []
                            },
                            "Internal": false,
                            "Attachable": false,
                            "Ingress": false,
                            "ConfigFrom": {
                                "Network": ""
                            },
                            "ConfigOnly": false,
                            "Containers": {},
                            "Options": {},
                            "Labels": {}
                        })
                    })
                    .collect();

                Ok(warp::reply::json(&docker_networks))
            }
            Err(e) => {
                tracing::error!("Failed to list networks: {}", e);
                Err(warp::reject::custom(e))
            }
        }
    }
}

#[derive(Debug)]
pub enum DockerAPIError {
    BadRequest(String),
    NotFound(String),
    Internal(String),
}

impl warp::reject::Reject for DockerAPIError {}

impl std::fmt::Display for DockerAPIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DockerAPIError::BadRequest(msg) => write!(f, "Bad Request: {}", msg),
            DockerAPIError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            DockerAPIError::Internal(msg) => write!(f, "Internal Error: {}", msg),
        }
    }
}

impl std::error::Error for DockerAPIError {}
