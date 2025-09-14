use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

        Ok(config)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize Boltfile")?;

        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write Boltfile at {:?}", path.as_ref()))?;

        Ok(())
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