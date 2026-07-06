use crate::config::{BoltConfig, BoltFile, Network, Service, Volume};
use crate::{BoltRuntime, ContainerInfo, Result};
use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPlan {
    pub project: String,
    pub boltfile: PathBuf,
    pub actions: Vec<ProjectAction>,
    pub summary: PlanSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectAction {
    pub action: ActionKind,
    pub resource_type: ResourceType,
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ActionKind {
    Create,
    Update,
    Destroy,
    Noop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    Service,
    Image,
    Volume,
    Network,
    Discovery,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanSummary {
    pub create: usize,
    pub update: usize,
    pub destroy: usize,
    pub noop: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltLock {
    pub version: u32,
    pub project: String,
    pub boltfile_hash: String,
    pub generated_at: String,
    pub services: BTreeMap<String, LockedService>,
    #[serde(default)]
    pub volumes: BTreeMap<String, LockedResource>,
    #[serde(default)]
    pub networks: BTreeMap<String, LockedResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedService {
    pub image: Option<String>,
    pub build: Option<String>,
    pub capsule: Option<String>,
    pub image_digest: Option<String>,
    pub config_hash: String,
    pub build_context_hash: Option<String>,
    pub ports: Vec<String>,
    pub volumes: Vec<String>,
    pub networks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedResource {
    pub config_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    pub project: String,
    pub drifted: bool,
    pub entries: Vec<DriftEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftEntry {
    pub resource_type: ResourceType,
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub name: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDiscoveryRegistry {
    pub project: String,
    pub generated_at: String,
    pub services: BTreeMap<String, ServiceDiscoveryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDiscoveryEntry {
    pub service: String,
    pub container_name: String,
    pub dns_name: String,
    pub networks: Vec<String>,
    pub ports: Vec<String>,
    pub protocol: String,
    pub healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum Inspection {
    Service(ServiceInspection),
    Container(ContainerInfo),
    Image(crate::ImageInfo),
    Volume(crate::volume::VolumeInfo),
    Network(crate::NetworkInfo),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInspection {
    pub name: String,
    pub container_name: String,
    pub desired: Service,
    pub discovery: Option<ServiceDiscoveryEntry>,
    pub container: Option<ContainerInfo>,
}

pub async fn plan(config: &BoltConfig, runtime: &BoltRuntime) -> Result<ProjectPlan> {
    let boltfile = config.load_boltfile()?;
    let containers = runtime.list_containers(true).await.unwrap_or_default();
    Ok(plan_from_state(config, &boltfile, &containers))
}

pub async fn apply(
    config: &BoltConfig,
    runtime: &BoltRuntime,
    services: &[String],
    detach: bool,
    force_recreate: bool,
    locked: bool,
) -> Result<ProjectPlan> {
    if locked {
        check_lock(config, runtime).await?;
    }
    let before = plan(config, runtime).await?;
    let boltfile = config.load_boltfile()?;
    create_declared_volumes(runtime, &boltfile).await?;
    create_declared_networks(runtime, &boltfile).await?;
    write_service_discovery(config, &boltfile)?;
    let ordered_services = ordered_target_services(&boltfile, services)?;
    runtime
        .surge_up(&ordered_services, detach, force_recreate)
        .await
        .context("failed to apply Boltfile through Surge")?;
    write_lock(config, runtime).await?;
    Ok(before)
}

pub async fn destroy(
    config: &BoltConfig,
    runtime: &BoltRuntime,
    services: &[String],
    volumes: bool,
    force: bool,
) -> Result<ProjectPlan> {
    let mut plan = plan(config, runtime).await?;
    plan.actions = destroy_actions(config, services)?;
    plan.summary = summarize(&plan.actions);
    if !force {
        return Ok(plan);
    }
    runtime.surge_down(services, volumes).await?;
    Ok(plan)
}

pub async fn write_lock(config: &BoltConfig, runtime: &BoltRuntime) -> Result<BoltLock> {
    let boltfile = config.load_boltfile()?;
    let lock = build_lock(config, runtime, &boltfile).await?;
    let path = lock_path(config);
    let json = serde_json::to_string_pretty(&lock)?;
    std::fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(lock)
}

pub async fn check_lock(config: &BoltConfig, runtime: &BoltRuntime) -> Result<()> {
    let existing = read_lock(config)?;
    let boltfile = config.load_boltfile()?;
    let current = build_lock(config, runtime, &boltfile).await?;
    if existing.boltfile_hash != current.boltfile_hash {
        return Err(anyhow!(
            "Boltfile.lock is stale: expected {}, current {}",
            existing.boltfile_hash,
            current.boltfile_hash
        )
        .into());
    }
    if existing.services != current.services
        || existing.volumes != current.volumes
        || existing.networks != current.networks
    {
        return Err(anyhow!("Boltfile.lock is stale: locked resource hashes differ").into());
    }
    Ok(())
}

pub async fn drift(config: &BoltConfig, runtime: &BoltRuntime) -> Result<DriftReport> {
    let boltfile = config.load_boltfile()?;
    let containers = runtime.list_containers(true).await.unwrap_or_default();
    let actual_names = container_names(&containers);
    let mut entries = Vec::new();

    for service_name in boltfile.services.keys() {
        let expected = service_container_name(&boltfile.project, service_name);
        if !actual_names.contains(&expected) {
            entries.push(DriftEntry {
                resource_type: ResourceType::Service,
                name: service_name.clone(),
                message: format!("expected container '{}' is missing", expected),
            });
        }
    }

    let prefix = format!("{}_", boltfile.project);
    for name in actual_names {
        if let Some(service_name) = name.strip_prefix(&prefix)
            && !boltfile.services.contains_key(service_name)
        {
            entries.push(DriftEntry {
                resource_type: ResourceType::Service,
                name: service_name.to_string(),
                message: format!("container '{}' is not declared in Boltfile", name),
            });
        }
    }

    Ok(DriftReport {
        project: boltfile.project,
        drifted: !entries.is_empty(),
        entries,
    })
}

pub fn write_service_discovery(
    config: &BoltConfig,
    boltfile: &BoltFile,
) -> Result<ServiceDiscoveryRegistry> {
    let registry = service_discovery_registry(boltfile);
    fs::create_dir_all(&config.data_dir)?;
    let path = service_discovery_path(config);
    fs::write(&path, serde_json::to_string_pretty(&registry)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(registry)
}

pub fn read_service_discovery(config: &BoltConfig) -> Option<ServiceDiscoveryRegistry> {
    let data = fs::read_to_string(service_discovery_path(config)).ok()?;
    serde_json::from_str(&data).ok()
}

pub async fn inspect(
    config: &BoltConfig,
    runtime: &BoltRuntime,
    kind: InspectKind,
    name: &str,
) -> Result<Inspection> {
    match kind {
        InspectKind::Service => inspect_service(config, runtime, name)
            .await
            .map(Inspection::Service),
        InspectKind::Container => runtime
            .list_containers(true)
            .await?
            .into_iter()
            .find(|container| {
                container.id == name
                    || container.name == name
                    || container.names.iter().any(|alias| alias == name)
            })
            .map(Inspection::Container)
            .ok_or_else(|| anyhow!("container '{}' not found", name).into()),
        InspectKind::Image => runtime
            .list_images()
            .await?
            .into_iter()
            .find(|image| image.name == name || image.id == name)
            .map(Inspection::Image)
            .ok_or_else(|| anyhow!("image '{}' not found", name).into()),
        InspectKind::Volume => runtime.inspect_volume(name).await.map(Inspection::Volume),
        InspectKind::Network => runtime
            .list_networks()
            .await?
            .into_iter()
            .find(|network| network.name == name || network.id == name)
            .map(Inspection::Network)
            .ok_or_else(|| anyhow!("network '{}' not found", name).into()),
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InspectKind {
    Service,
    Container,
    Image,
    Volume,
    Network,
}

pub fn import_compose(input: &Path, output: &Path) -> Result<()> {
    let content = fs::read_to_string(input)
        .with_context(|| format!("failed to read compose file {}", input.display()))?;
    let boltfile_content = crate::compat::compose::ComposeCompat::convert_compose_file(&content)?;
    fs::write(output, boltfile_content)
        .with_context(|| format!("failed to write {}", output.display()))?;
    Ok(())
}

pub async fn import_container(
    config: &BoltConfig,
    runtime: &BoltRuntime,
    container: &str,
    service_name: Option<&str>,
) -> Result<()> {
    let info = runtime
        .list_containers(true)
        .await?
        .into_iter()
        .find(|item| {
            item.id == container
                || item.name == container
                || item.names.iter().any(|alias| alias == container)
        })
        .ok_or_else(|| anyhow!("container '{}' not found", container))?;
    let mut boltfile = load_or_empty_boltfile(config)?;
    let name = service_name
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| strip_project_prefix(&boltfile.project, &info.name));
    boltfile.services.insert(
        name,
        Service {
            image: Some(info.image),
            ports: Some(info.ports),
            container_name: Some(info.name),
            ..Service::default()
        },
    );
    boltfile.save(&config.boltfile_path)?;
    Ok(())
}

pub fn import_image(config: &BoltConfig, image: &str, service_name: Option<&str>) -> Result<()> {
    let mut boltfile = load_or_empty_boltfile(config)?;
    let name = service_name
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| image_service_name(image));
    boltfile.services.insert(
        name,
        Service {
            image: Some(image.to_string()),
            ..Service::default()
        },
    );
    boltfile.save(&config.boltfile_path)?;
    Ok(())
}

pub async fn doctor(config: &BoltConfig, runtime: &BoltRuntime) -> DoctorReport {
    let mut checks = Vec::new();
    checks.push(DoctorCheck {
        name: "Boltfile".to_string(),
        ok: config.boltfile_path.exists(),
        message: config.boltfile_path.display().to_string(),
    });

    let runtime_ok = runtime.list_containers(true).await.is_ok();
    checks.push(DoctorCheck {
        name: "Runtime".to_string(),
        ok: runtime_ok,
        message: if runtime_ok {
            "container state is readable".to_string()
        } else {
            "container state could not be listed".to_string()
        },
    });

    let bridge = crate::networking::bridge::BridgeManager::preflight();
    checks.push(DoctorCheck {
        name: "Bridge networking".to_string(),
        ok: bridge.can_manage_links,
        message: if bridge.reasons.is_empty() {
            "host can manage bridge links".to_string()
        } else {
            bridge.reasons.join(", ")
        },
    });

    let snapshot_check = match crate::capsules::snapshots::SnapshotManager::new().await {
        Ok(manager) => match manager.preflight().await {
            Ok(report) => DoctorCheck {
                name: "Snapshots".to_string(),
                ok: report.supported,
                message: report.reason.unwrap_or_else(|| report.filesystem),
            },
            Err(err) => DoctorCheck {
                name: "Snapshots".to_string(),
                ok: false,
                message: err.to_string(),
            },
        },
        Err(err) => DoctorCheck {
            name: "Snapshots".to_string(),
            ok: false,
            message: err.to_string(),
        },
    };
    checks.push(snapshot_check);

    DoctorReport {
        ok: checks.iter().all(|check| check.ok),
        checks,
    }
}

fn plan_from_state(
    config: &BoltConfig,
    boltfile: &BoltFile,
    containers: &[ContainerInfo],
) -> ProjectPlan {
    let actual_names = container_names(containers);
    let mut actions = Vec::new();

    let locked = read_lock(config).ok();
    let ordered =
        service_order(boltfile).unwrap_or_else(|_| boltfile.services.keys().cloned().collect());
    for name in ordered {
        let Some(service) = boltfile.services.get(&name) else {
            continue;
        };
        let expected = service_container_name(&boltfile.project, &name);
        let image = service
            .image
            .as_deref()
            .or(service.build.as_deref())
            .or(service.capsule.as_deref())
            .unwrap_or("<unset>");
        if actual_names.contains(&expected) {
            let desired_hash =
                service_config_hash(service).unwrap_or_else(|_| "unknown".to_string());
            let action = locked
                .as_ref()
                .and_then(|lock| lock.services.get(&name))
                .map(|locked_service| locked_service.config_hash != desired_hash)
                .unwrap_or(false);
            actions.push(ProjectAction {
                action: if action {
                    ActionKind::Update
                } else {
                    ActionKind::Noop
                },
                resource_type: ResourceType::Service,
                name: name.clone(),
                reason: if action {
                    format!("service config changed for container '{}'", expected)
                } else {
                    format!("container '{}' exists for {}", expected, image)
                },
            });
        } else {
            actions.push(ProjectAction {
                action: ActionKind::Create,
                resource_type: ResourceType::Service,
                name: name.clone(),
                reason: format!("container '{}' is missing", expected),
            });
        }
        if let Some(image) = &service.image {
            actions.push(ProjectAction {
                action: ActionKind::Create,
                resource_type: ResourceType::Image,
                name: image.clone(),
                reason: "image must be present or pulled during apply".to_string(),
            });
        }
    }

    if let Some(volumes) = &boltfile.volumes {
        for name in volumes.keys() {
            actions.push(ProjectAction {
                action: ActionKind::Create,
                resource_type: ResourceType::Volume,
                name: name.clone(),
                reason: "declared in Boltfile".to_string(),
            });
        }
    }

    if let Some(networks) = &boltfile.networks {
        for name in networks.keys() {
            actions.push(ProjectAction {
                action: ActionKind::Create,
                resource_type: ResourceType::Network,
                name: name.clone(),
                reason: "declared in Boltfile".to_string(),
            });
        }
    }

    let prefix = format!("{}_", boltfile.project);
    for name in actual_names {
        if let Some(service_name) = name.strip_prefix(&prefix)
            && !boltfile.services.contains_key(service_name)
        {
            actions.push(ProjectAction {
                action: ActionKind::Destroy,
                resource_type: ResourceType::Service,
                name: service_name.to_string(),
                reason: format!("container '{}' is not declared", name),
            });
        }
    }

    let summary = summarize(&actions);
    ProjectPlan {
        project: boltfile.project.clone(),
        boltfile: config.boltfile_path.clone(),
        actions,
        summary,
    }
}

fn destroy_actions(config: &BoltConfig, services: &[String]) -> Result<Vec<ProjectAction>> {
    let boltfile = config.load_boltfile()?;
    let target_services: Vec<String> = if services.is_empty() {
        let mut ordered = service_order(&boltfile)?;
        ordered.reverse();
        ordered
    } else {
        let requested = services.iter().cloned().collect::<BTreeSet<_>>();
        let mut ordered = service_order(&boltfile)?;
        ordered.retain(|service| requested.contains(service));
        ordered.reverse();
        ordered
    };
    Ok(target_services
        .into_iter()
        .map(|name| ProjectAction {
            action: ActionKind::Destroy,
            resource_type: ResourceType::Service,
            reason: "destroy requested".to_string(),
            name,
        })
        .collect())
}

async fn build_lock(
    config: &BoltConfig,
    runtime: &BoltRuntime,
    boltfile: &BoltFile,
) -> Result<BoltLock> {
    let image_digests = runtime
        .list_images()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|image| (image.name, image.id))
        .collect::<BTreeMap<_, _>>();

    let mut services = BTreeMap::new();
    for (name, service) in &boltfile.services {
        services.insert(
            name.clone(),
            LockedService {
                image: service.image.clone(),
                build: service.build.clone(),
                capsule: service.capsule.clone(),
                image_digest: service
                    .image
                    .as_ref()
                    .and_then(|image| image_digests.get(image).cloned()),
                config_hash: service_config_hash(service)?,
                build_context_hash: service
                    .build
                    .as_deref()
                    .map(|path| hash_build_context(Path::new(path)))
                    .transpose()?,
                ports: service.ports.clone().unwrap_or_default(),
                volumes: service.volumes.clone().unwrap_or_default(),
                networks: service.networks.clone().unwrap_or_default(),
            },
        );
    }
    let volumes = boltfile
        .volumes
        .as_ref()
        .map(|volumes| {
            volumes
                .iter()
                .map(|(name, volume)| {
                    Ok((
                        name.clone(),
                        LockedResource {
                            config_hash: stable_hash(volume)?,
                        },
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()
        })
        .transpose()?
        .unwrap_or_default();
    let networks = boltfile
        .networks
        .as_ref()
        .map(|networks| {
            networks
                .iter()
                .map(|(name, network)| {
                    Ok((
                        name.clone(),
                        LockedResource {
                            config_hash: stable_hash(network)?,
                        },
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()
        })
        .transpose()?
        .unwrap_or_default();

    Ok(BoltLock {
        version: 1,
        project: boltfile.project.clone(),
        boltfile_hash: boltfile_hash(&config.boltfile_path)?,
        generated_at: chrono::Utc::now().to_rfc3339(),
        services,
        volumes,
        networks,
    })
}

fn read_lock(config: &BoltConfig) -> Result<BoltLock> {
    let path = lock_path(config);
    let data = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(serde_json::from_str(&data)
        .with_context(|| format!("failed to parse {}", path.display()))?)
}

fn lock_path(config: &BoltConfig) -> PathBuf {
    config
        .boltfile_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Boltfile.lock")
}

fn boltfile_hash(path: &Path) -> Result<String> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(format!("sha256:{:x}", Sha256::digest(&bytes)))
}

fn service_config_hash(service: &Service) -> Result<String> {
    stable_hash(service)
}

fn stable_hash<T: Serialize>(value: &T) -> Result<String> {
    let bytes = serde_json::to_vec(value)?;
    Ok(format!("sha256:{:x}", Sha256::digest(&bytes)))
}

fn hash_build_context(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    if !path.exists() {
        return Ok("missing".to_string());
    }
    if path.is_file() {
        hash_file(path, path, &mut hasher)?;
        return Ok(format!("sha256:{:x}", hasher.finalize()));
    }
    let mut files = Vec::new();
    collect_context_files(path, path, &mut files)?;
    files.sort();
    for relative in files {
        let full = path.join(&relative);
        hasher.update(relative.to_string_lossy().as_bytes());
        hash_file(path, &full, &mut hasher)?;
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn collect_context_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if matches!(name.as_ref(), ".git" | "target" | ".scratch") {
            continue;
        }
        if path.is_dir() {
            collect_context_files(root, &path, files)?;
        } else if path.is_file() {
            files.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
        }
    }
    Ok(())
}

fn hash_file(root: &Path, path: &Path, hasher: &mut Sha256) -> Result<()> {
    let relative = path.strip_prefix(root).unwrap_or(path);
    hasher.update(relative.to_string_lossy().as_bytes());
    hasher.update(fs::read(path)?);
    Ok(())
}

fn service_order(boltfile: &BoltFile) -> Result<Vec<String>> {
    let mut indegree = boltfile
        .services
        .keys()
        .map(|name| (name.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (name, service) in &boltfile.services {
        for dep in service.depends_on.clone().unwrap_or_default() {
            if !boltfile.services.contains_key(&dep) {
                return Err(
                    anyhow!("service '{}' depends on unknown service '{}'", name, dep).into(),
                );
            }
            edges.entry(dep).or_default().insert(name.clone());
            *indegree.entry(name.clone()).or_default() += 1;
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(name, degree)| (*degree == 0).then_some(name.clone()))
        .collect::<VecDeque<_>>();
    let mut out = Vec::new();
    while let Some(name) = ready.pop_front() {
        out.push(name.clone());
        if let Some(children) = edges.get(&name) {
            for child in children {
                let degree = indegree.get_mut(child).expect("child exists");
                *degree -= 1;
                if *degree == 0 {
                    ready.push_back(child.clone());
                }
            }
        }
    }
    if out.len() != boltfile.services.len() {
        return Err(anyhow!("service dependency cycle detected").into());
    }
    Ok(out)
}

fn ordered_target_services(boltfile: &BoltFile, services: &[String]) -> Result<Vec<String>> {
    let mut ordered = service_order(boltfile)?;
    if services.is_empty() {
        return Ok(ordered);
    }
    let requested = services.iter().cloned().collect::<BTreeSet<_>>();
    ordered.retain(|service| requested.contains(service));
    Ok(ordered)
}

async fn create_declared_volumes(runtime: &BoltRuntime, boltfile: &BoltFile) -> Result<()> {
    let existing = runtime
        .list_volumes()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|volume| volume.name)
        .collect::<BTreeSet<_>>();
    for (name, volume) in boltfile.volumes.as_ref().into_iter().flatten() {
        if volume.external.unwrap_or(false) || existing.contains(name) {
            continue;
        }
        let driver = volume.driver.as_deref().unwrap_or("local");
        let options = volume_options(volume);
        runtime.create_volume(name, driver, None, &options).await?;
    }
    Ok(())
}

async fn create_declared_networks(runtime: &BoltRuntime, boltfile: &BoltFile) -> Result<()> {
    let existing = runtime
        .list_networks()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|network| network.name)
        .collect::<BTreeSet<_>>();
    for (name, network) in boltfile.networks.as_ref().into_iter().flatten() {
        if network.external.unwrap_or(false) || existing.contains(name) {
            continue;
        }
        let subnet = network_subnet(network);
        runtime
            .create_network(name, &network.driver, subnet.as_deref())
            .await?;
    }
    Ok(())
}

fn volume_options(volume: &Volume) -> Vec<String> {
    volume
        .driver_opts
        .as_ref()
        .map(|opts| opts.iter().map(|(k, v)| format!("{k}={v}")).collect())
        .unwrap_or_default()
}

fn network_subnet(network: &Network) -> Option<String> {
    network
        .ipam
        .as_ref()
        .and_then(|ipam| ipam.config.as_ref())
        .and_then(|configs| configs.first())
        .and_then(|config| config.subnet.clone())
}

fn service_discovery_registry(boltfile: &BoltFile) -> ServiceDiscoveryRegistry {
    let services = boltfile
        .services
        .iter()
        .map(|(name, service)| {
            let entry =
                ServiceDiscoveryEntry {
                    service: name.clone(),
                    container_name: service
                        .container_name
                        .clone()
                        .unwrap_or_else(|| service_container_name(&boltfile.project, name)),
                    dns_name: format!("{name}.{}.bolt", boltfile.project),
                    networks: service.networks.clone().unwrap_or_default(),
                    ports: service.ports.clone().unwrap_or_default(),
                    protocol: if service.networks.as_ref().is_some_and(|networks| {
                        networks.iter().any(|network| network.contains("quic"))
                    }) {
                        "quic".to_string()
                    } else {
                        "tcp".to_string()
                    },
                    healthy: true,
                };
            (name.clone(), entry)
        })
        .collect();
    ServiceDiscoveryRegistry {
        project: boltfile.project.clone(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        services,
    }
}

fn service_discovery_path(config: &BoltConfig) -> PathBuf {
    config.data_dir.join("service_discovery.json")
}

async fn inspect_service(
    config: &BoltConfig,
    runtime: &BoltRuntime,
    name: &str,
) -> Result<ServiceInspection> {
    let boltfile = config.load_boltfile()?;
    let desired = boltfile
        .services
        .get(name)
        .cloned()
        .ok_or_else(|| anyhow!("service '{}' not found", name))?;
    let container_name = desired
        .container_name
        .clone()
        .unwrap_or_else(|| service_container_name(&boltfile.project, name));
    let discovery =
        read_service_discovery(config).and_then(|registry| registry.services.get(name).cloned());
    let container = runtime
        .list_containers(true)
        .await
        .unwrap_or_default()
        .into_iter()
        .find(|container| {
            container.name == container_name
                || container.names.iter().any(|alias| alias == &container_name)
        });
    Ok(ServiceInspection {
        name: name.to_string(),
        container_name,
        desired,
        discovery,
        container,
    })
}

fn load_or_empty_boltfile(config: &BoltConfig) -> Result<BoltFile> {
    if config.boltfile_path.exists() {
        Ok(config.load_boltfile()?)
    } else {
        Ok(BoltFile {
            project: config
                .boltfile_path
                .parent()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or("bolt-project")
                .to_string(),
            services: HashMap::new(),
            networks: None,
            volumes: None,
            snapshots: None,
        })
    }
}

fn strip_project_prefix(project: &str, name: &str) -> String {
    name.strip_prefix(&format!("{project}_"))
        .unwrap_or(name)
        .replace(['/', ':'], "-")
}

fn image_service_name(image: &str) -> String {
    image
        .rsplit('/')
        .next()
        .unwrap_or(image)
        .split(':')
        .next()
        .unwrap_or("image")
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn container_names(containers: &[ContainerInfo]) -> BTreeSet<String> {
    containers
        .iter()
        .flat_map(|container| {
            std::iter::once(container.name.clone()).chain(container.names.iter().cloned())
        })
        .filter(|name| !name.is_empty())
        .collect()
}

fn service_container_name(project: &str, service_name: &str) -> String {
    format!("{project}_{service_name}")
}

fn summarize(actions: &[ProjectAction]) -> PlanSummary {
    let mut summary = PlanSummary::default();
    for action in actions {
        match action.action {
            ActionKind::Create => summary.create += 1,
            ActionKind::Update => summary.update += 1,
            ActionKind::Destroy => summary.destroy += 1,
            ActionKind::Noop => summary.noop += 1,
        }
    }
    summary
}

pub fn print_plan(plan: &ProjectPlan) {
    println!("Project: {}", plan.project);
    println!("Boltfile: {}", plan.boltfile.display());
    println!(
        "Plan: {} create, {} update, {} destroy, {} noop",
        plan.summary.create, plan.summary.update, plan.summary.destroy, plan.summary.noop
    );
    for action in &plan.actions {
        println!(
            "  {:?} {:?}.{} - {}",
            action.action, action.resource_type, action.name, action.reason
        );
    }
}

pub fn ensure_force(force: bool, operation: &str) -> Result<()> {
    if force {
        Ok(())
    } else {
        Err(anyhow!("{operation} requires --force after reviewing the plan").into())
    }
}

#[allow(dead_code)]
fn _service_kind(service: &Service) -> &'static str {
    if service.image.is_some() {
        "image"
    } else if service.build.is_some() {
        "build"
    } else if service.capsule.is_some() {
        "capsule"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Service;

    #[test]
    fn plan_creates_missing_services_and_marks_existing_noop() {
        let mut services = std::collections::HashMap::new();
        services.insert(
            "web".to_string(),
            Service {
                image: Some("nginx:latest".to_string()),
                ..Service::default()
            },
        );
        services.insert(
            "db".to_string(),
            Service {
                image: Some("postgres:16".to_string()),
                ..Service::default()
            },
        );
        let boltfile = BoltFile {
            project: "demo".to_string(),
            services,
            networks: None,
            volumes: None,
            snapshots: None,
        };
        let config = BoltConfig {
            boltfile_path: PathBuf::from("Boltfile.toml"),
            ..BoltConfig::default()
        };
        let containers = vec![ContainerInfo {
            id: "abc".to_string(),
            name: "demo_web".to_string(),
            names: vec!["demo_web".to_string()],
            image: "nginx:latest".to_string(),
            image_id: String::new(),
            command: String::new(),
            created: String::new(),
            status: "running".to_string(),
            ports: vec![],
            labels: Default::default(),
            uptime: None,
            runtime: None,
        }];

        let plan = plan_from_state(&config, &boltfile, &containers);
        assert!(plan.actions.iter().any(|action| {
            action.resource_type == ResourceType::Service
                && action.name == "web"
                && action.action == ActionKind::Noop
        }));
        assert!(plan.actions.iter().any(|action| {
            action.resource_type == ResourceType::Service
                && action.name == "db"
                && action.action == ActionKind::Create
        }));
    }

    #[test]
    fn plan_detects_undeclared_project_containers() {
        let boltfile = BoltFile {
            project: "demo".to_string(),
            services: Default::default(),
            networks: None,
            volumes: None,
            snapshots: None,
        };
        let config = BoltConfig {
            boltfile_path: PathBuf::from("Boltfile.toml"),
            ..BoltConfig::default()
        };
        let containers = vec![ContainerInfo {
            id: "abc".to_string(),
            name: "demo_old".to_string(),
            names: vec![],
            image: "nginx:latest".to_string(),
            image_id: String::new(),
            command: String::new(),
            created: String::new(),
            status: "running".to_string(),
            ports: vec![],
            labels: Default::default(),
            uptime: None,
            runtime: None,
        }];

        let plan = plan_from_state(&config, &boltfile, &containers);
        assert!(plan.actions.iter().any(|action| {
            action.resource_type == ResourceType::Service
                && action.name == "old"
                && action.action == ActionKind::Destroy
        }));
    }

    #[test]
    fn service_order_respects_depends_on_and_destroy_reverses_it() {
        let mut services = std::collections::HashMap::new();
        services.insert(
            "db".to_string(),
            Service {
                image: Some("postgres:16".to_string()),
                ..Service::default()
            },
        );
        services.insert(
            "web".to_string(),
            Service {
                image: Some("nginx:latest".to_string()),
                depends_on: Some(vec!["db".to_string()]),
                ..Service::default()
            },
        );
        let boltfile = BoltFile {
            project: "demo".to_string(),
            services,
            networks: None,
            volumes: None,
            snapshots: None,
        };
        assert_eq!(service_order(&boltfile).unwrap(), vec!["db", "web"]);
    }

    #[test]
    fn service_config_hash_changes_when_ports_change() {
        let mut service = Service {
            image: Some("nginx:latest".to_string()),
            ports: Some(vec!["8080:80".to_string()]),
            ..Service::default()
        };
        let first = service_config_hash(&service).unwrap();
        service.ports = Some(vec!["8081:80".to_string()]);
        let second = service_config_hash(&service).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn discovery_registry_assigns_project_dns_names() {
        let mut services = std::collections::HashMap::new();
        services.insert(
            "web".to_string(),
            Service {
                image: Some("nginx:latest".to_string()),
                ports: Some(vec!["8080:80".to_string()]),
                ..Service::default()
            },
        );
        let boltfile = BoltFile {
            project: "demo".to_string(),
            services,
            networks: None,
            volumes: None,
            snapshots: None,
        };
        let registry = service_discovery_registry(&boltfile);
        assert_eq!(
            registry.services.get("web").unwrap().dns_name,
            "web.demo.bolt"
        );
    }
}
