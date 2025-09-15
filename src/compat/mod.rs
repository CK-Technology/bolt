pub mod compose;
pub mod docker;
pub mod podman;

use crate::BoltRuntime;
use crate::error::{BoltError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Docker/Podman compatibility layer for seamless migration
pub trait ContainerCompatibility {
    /// Convert Docker/Podman command to Bolt equivalent
    fn translate_command(&self, args: &[String]) -> Result<Vec<String>>;

    /// Map container format to Bolt runtime
    fn convert_run_args(&self, args: &DockerRunArgs) -> Result<BoltRunArgs>;

    /// Convert compose file to Boltfile
    fn convert_compose(&self, compose: &str) -> Result<String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerRunArgs {
    pub image: String,
    pub name: Option<String>,
    pub ports: Vec<String>,
    pub volumes: Vec<String>,
    pub env: Vec<String>,
    pub network: Option<String>,
    pub detach: bool,
    pub interactive: bool,
    pub tty: bool,
    pub rm: bool,
    pub privileged: bool,
    pub user: Option<String>,
    pub workdir: Option<String>,
    pub entrypoint: Option<String>,
    pub cmd: Vec<String>,
    pub restart: Option<String>,
    pub memory: Option<String>,
    pub cpus: Option<String>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltRunArgs {
    pub image: String,
    pub name: Option<String>,
    pub ports: Vec<String>,
    pub volumes: Vec<String>,
    pub env: Vec<String>,
    pub network: Option<String>,
    pub detach: bool,
    pub capsule_config: Option<CapsuleConfig>,
    pub gaming_config: Option<super::config::GamingConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleConfig {
    pub memory_limit: Option<String>,
    pub cpu_limit: Option<f64>,
    pub privileged: bool,
    pub user: Option<String>,
    pub workdir: Option<String>,
    pub restart_policy: Option<String>,
}

/// Docker API compatibility endpoint mappings
pub struct DockerApiCompat {
    pub version: String,
    pub runtime: BoltRuntime,
}

impl Clone for DockerApiCompat {
    fn clone(&self) -> Self {
        Self {
            version: self.version.clone(),
            runtime: self.runtime.clone(),
        }
    }
}

impl DockerApiCompat {
    pub fn new(runtime: BoltRuntime) -> Self {
        Self {
            version: "1.43".to_string(), // Docker API version compatibility
            runtime,
        }
    }

    /// Handle Docker API requests and translate to Bolt operations
    pub async fn handle_request(&self, path: &str, method: &str, body: &str) -> Result<String> {
        match (method, path) {
            ("GET", "/version") => self.version_info(),
            ("GET", "/info") => self.system_info().await,
            ("GET", "/containers/json") => self.list_containers().await,
            ("POST", "/containers/create") => self.create_container(body).await,
            ("POST", path) if path.starts_with("/containers/") && path.ends_with("/start") => {
                let id = self.extract_container_id(path)?;
                self.start_container(&id).await
            }
            ("POST", path) if path.starts_with("/containers/") && path.ends_with("/stop") => {
                let id = self.extract_container_id(path)?;
                self.stop_container(&id).await
            }
            ("DELETE", path) if path.starts_with("/containers/") => {
                let id = self.extract_container_id(path)?;
                self.remove_container(&id).await
            }
            ("GET", "/images/json") => self.list_images().await,
            ("POST", "/images/create") => self.pull_image(body).await,
            ("POST", "/build") => self.build_image(body).await,
            _ => Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: format!("Unsupported API endpoint: {} {}", method, path),
            })),
        }
    }

    fn version_info(&self) -> Result<String> {
        let info = serde_json::json!({
            "Version": "24.0.0-bolt",
            "ApiVersion": self.version,
            "MinAPIVersion": "1.12",
            "GitCommit": "bolt-compat",
            "GoVersion": "go1.20",
            "Os": "linux",
            "Arch": "amd64",
            "KernelVersion": std::process::Command::new("uname")
                .arg("-r")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
            "Components": [
                {
                    "Name": "Bolt Engine",
                    "Version": "0.1.0",
                }
            ]
        });
        Ok(info.to_string())
    }

    async fn system_info(&self) -> Result<String> {
        let info = serde_json::json!({
            "ID": "bolt-system",
            "Containers": 0,
            "ContainersRunning": 0,
            "ContainersPaused": 0,
            "ContainersStopped": 0,
            "Images": 0,
            "Driver": "bolt",
            "DriverStatus": [
                ["Bolt Version", "0.1.0"],
                ["QUIC Networking", "Enabled"],
                ["Gaming Support", "Enabled"]
            ],
            "Plugins": {
                "Volume": ["local", "s3", "ghostbay"],
                "Network": ["bridge", "bolt", "quic"],
                "Authorization": null
            },
            "MemoryLimit": true,
            "SwapLimit": true,
            "KernelMemoryTCP": true,
            "CpuCfsPeriod": true,
            "CpuCfsQuota": true,
            "CPUShares": true,
            "CPUSet": true,
            "PidsLimit": true,
            "IPv4Forwarding": true,
            "BridgeNfIptables": true,
            "BridgeNfIp6tables": true,
            "Debug": false,
            "NFd": 24,
            "OomKillDisable": true,
            "NGoroutines": 42,
            "SystemTime": chrono::Utc::now().to_rfc3339(),
            "LoggingDriver": "json-file",
            "CgroupDriver": "systemd",
            "NEventsListener": 0,
            "KernelVersion": "6.5.0",
            "OperatingSystem": "Bolt OS",
            "OSVersion": "",
            "OSType": "linux",
            "Architecture": "x86_64",
            "IndexServerAddress": "https://index.docker.io/v1/",
            "NCPU": num_cpus::get(),
            "MemTotal": self.get_memory_total(),
            "DockerRootDir": "/var/lib/bolt",
            "HttpProxy": "",
            "HttpsProxy": "",
            "NoProxy": "",
            "Name": "bolt-host",
            "Labels": [],
            "ExperimentalBuild": false,
            "ServerVersion": "24.0.0-bolt",
            "ClusterStore": "",
            "ClusterAdvertise": "",
            "Runtimes": {
                "bolt": {
                    "path": "bolt-runtime"
                },
                "runc": {
                    "path": "runc"
                }
            },
            "DefaultRuntime": "bolt",
            "Swarm": {
                "NodeID": "",
                "NodeAddr": "",
                "LocalNodeState": "inactive",
                "ControlAvailable": false,
                "Error": "",
                "RemoteManagers": null
            },
            "LiveRestoreEnabled": false,
            "Isolation": "",
            "InitBinary": "",
            "ContainerdCommit": {
                "ID": "",
                "Expected": ""
            },
            "RuncCommit": {
                "ID": "",
                "Expected": ""
            },
            "InitCommit": {
                "ID": "",
                "Expected": ""
            },
            "SecurityOptions": [
                "name=seccomp,profile=default",
                "name=selinux"
            ],
            "ProductLicense": "MIT"
        });
        Ok(info.to_string())
    }

    async fn list_containers(&self) -> Result<String> {
        let containers = self.runtime.list_containers(false).await?;
        let docker_format: Vec<serde_json::Value> = containers
            .into_iter()
            .map(|c| {
                serde_json::json!({
                    "Id": c.id,
                    "Names": [format!("/{}", c.name)],
                    "Image": c.image,
                    "ImageID": format!("sha256:{}", "x".repeat(64)),
                    "Command": "",
                    "Created": chrono::Utc::now().timestamp(),
                    "Ports": c.ports.iter().map(|p| {
                        let parts: Vec<&str> = p.split(':').collect();
                        serde_json::json!({
                            "PublicPort": parts.get(0).unwrap_or(&"0").parse::<u32>().unwrap_or(0),
                            "PrivatePort": parts.get(1).unwrap_or(&"0").parse::<u32>().unwrap_or(0),
                            "Type": "tcp"
                        })
                    }).collect::<Vec<_>>(),
                    "Labels": {},
                    "State": c.status,
                    "Status": format!("Up 1 minute"),
                    "HostConfig": {
                        "NetworkMode": "default"
                    },
                    "NetworkSettings": {
                        "Networks": {
                            "bridge": {
                                "IPAMConfig": null,
                                "Links": null,
                                "Aliases": null,
                                "NetworkID": "bridge",
                                "EndpointID": "",
                                "Gateway": "172.17.0.1",
                                "IPAddress": "172.17.0.2",
                                "IPPrefixLen": 16,
                                "IPv6Gateway": "",
                                "GlobalIPv6Address": "",
                                "GlobalIPv6PrefixLen": 0,
                                "MacAddress": ""
                            }
                        }
                    },
                    "Mounts": []
                })
            })
            .collect();

        Ok(serde_json::to_string(&docker_format)?)
    }

    async fn create_container(&self, body: &str) -> Result<String> {
        let request: serde_json::Value = serde_json::from_str(body)?;

        let image = request["Image"].as_str().unwrap_or("").to_string();
        let name = request["Name"].as_str().map(|s| s.to_string());

        // Extract port mappings
        let mut ports = Vec::new();
        if let Some(port_bindings) = request["HostConfig"]["PortBindings"].as_object() {
            for (container_port, host_bindings) in port_bindings {
                if let Some(bindings) = host_bindings.as_array() {
                    for binding in bindings {
                        if let Some(host_port) = binding["HostPort"].as_str() {
                            let container_port_clean =
                                container_port.replace("/tcp", "").replace("/udp", "");
                            ports.push(format!("{}:{}", host_port, container_port_clean));
                        }
                    }
                }
            }
        }

        // Extract environment variables
        let mut env = Vec::new();
        if let Some(env_array) = request["Env"].as_array() {
            for env_var in env_array {
                if let Some(var) = env_var.as_str() {
                    env.push(var.to_string());
                }
            }
        }

        // Extract volumes
        let mut volumes = Vec::new();
        if let Some(binds) = request["HostConfig"]["Binds"].as_array() {
            for bind in binds {
                if let Some(volume) = bind.as_str() {
                    volumes.push(volume.to_string());
                }
            }
        }

        // Create container using Bolt runtime
        self.runtime
            .run_container(
                &image,
                name.as_deref(),
                &ports,
                &env,
                &volumes,
                false, // Don't start immediately
            )
            .await?;

        let container_id = format!(
            "bolt-{}",
            uuid::Uuid::new_v4().to_string().replace("-", "")[..12].to_string()
        );

        Ok(serde_json::json!({
            "Id": container_id,
            "Warnings": []
        })
        .to_string())
    }

    async fn start_container(&self, id: &str) -> Result<String> {
        // In a real implementation, we'd need to track container IDs
        // For now, assume container name matches ID
        self.runtime
            .run_container(
                "existing", // Would need to look up image
                Some(id),
                &[],
                &[],
                &[],
                true,
            )
            .await?;

        Ok("".to_string())
    }

    async fn stop_container(&self, id: &str) -> Result<String> {
        self.runtime.stop_container(id).await?;
        Ok("".to_string())
    }

    async fn remove_container(&self, id: &str) -> Result<String> {
        self.runtime.remove_container(id, false).await?;
        Ok("".to_string())
    }

    async fn list_images(&self) -> Result<String> {
        // Placeholder - would need actual image management
        let images = serde_json::json!([
            {
                "Id": "sha256:f7a7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7",
                "ParentId": "",
                "RepoTags": ["nginx:latest"],
                "RepoDigests": [],
                "Created": chrono::Utc::now().timestamp(),
                "Size": 142857216,
                "VirtualSize": 142857216,
                "SharedSize": -1,
                "Labels": null,
                "Containers": 1
            }
        ]);
        Ok(images.to_string())
    }

    async fn pull_image(&self, body: &str) -> Result<String> {
        let request: serde_json::Value = serde_json::from_str(body)
            .unwrap_or_else(|_| serde_json::json!({ "fromImage": "nginx:latest" }));

        let image = request["fromImage"].as_str().unwrap_or("nginx:latest");
        self.runtime.pull_image(image).await?;

        Ok(serde_json::json!({
            "status": "Downloaded newer image",
            "id": image
        })
        .to_string())
    }

    async fn build_image(&self, body: &str) -> Result<String> {
        // Extract build context and Dockerfile
        self.runtime
            .build_image(".", Some("bolt-built:latest"), "Dockerfile")
            .await?;

        Ok(serde_json::json!({
            "stream": "Successfully built bolt-built:latest\n"
        })
        .to_string())
    }

    fn extract_container_id(&self, path: &str) -> Result<String> {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() >= 3 {
            Ok(parts[2].to_string())
        } else {
            Err(BoltError::Runtime(crate::error::RuntimeError::OciError {
                message: "Invalid container path".to_string(),
            }))
        }
    }

    fn get_memory_total(&self) -> u64 {
        // Get system memory in bytes
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|contents| {
                contents
                    .lines()
                    .find(|line| line.starts_with("MemTotal:"))
                    .and_then(|line| {
                        line.split_whitespace()
                            .nth(1)
                            .and_then(|s| s.parse::<u64>().ok())
                    })
            })
            .map(|kb| kb * 1024) // Convert KB to bytes
            .unwrap_or(0)
    }
}
