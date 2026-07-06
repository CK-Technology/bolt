use crate::config::{BoltConfig, BoltFile, Network, Service, Volume};
use crate::{BoltRuntime, ContainerInfo, Result};
use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use tokio::net::UdpSocket;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
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

pub type ValidationReport = DoctorReport;

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
    #[serde(default = "localhost_ip")]
    pub address: Ipv4Addr,
    #[serde(default)]
    pub address_source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
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
    pub gpu: ServiceGpuInspection,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceGpuInspection {
    pub requested: bool,
    pub vendor: Option<String>,
    pub runtime: Option<String>,
    pub devices: Vec<String>,
    pub profile: Option<String>,
    pub notes: Vec<String>,
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
    let ordered_services = selective_apply_services(&boltfile, &before, services, force_recreate)?;
    if !ordered_services.is_empty() {
        runtime
            .surge_up(&ordered_services, detach, force_recreate)
            .await
            .context("failed to apply Boltfile through Surge")?;
    }
    let containers = runtime.list_containers(true).await.unwrap_or_default();
    write_service_discovery(config, &boltfile, &containers)?;
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
    let lock = build_lock(config, runtime, &boltfile, true).await?;
    let path = lock_path(config);
    let json = serde_json::to_string_pretty(&lock)?;
    std::fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(lock)
}

pub async fn check_lock(config: &BoltConfig, runtime: &BoltRuntime) -> Result<()> {
    let existing = read_lock(config)?;
    let boltfile = config.load_boltfile()?;
    let current = build_lock(config, runtime, &boltfile, false).await?;
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
    let missing_digests = validate_locked_image_digests(config);
    if !missing_digests.is_empty() {
        return Err(anyhow!(
            "Boltfile.lock is missing required image digests: {}",
            missing_digests.join("; ")
        )
        .into());
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
    containers: &[ContainerInfo],
) -> Result<ServiceDiscoveryRegistry> {
    let registry = service_discovery_registry(boltfile, containers);
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

pub async fn validate(config: &BoltConfig, runtime: &BoltRuntime) -> ValidationReport {
    let mut checks = Vec::new();
    let boltfile = match config.load_boltfile() {
        Ok(boltfile) => boltfile,
        Err(err) => {
            checks.push(DoctorCheck {
                name: "Boltfile parse".to_string(),
                ok: false,
                message: err.to_string(),
            });
            return ValidationReport { ok: false, checks };
        }
    };

    checks.push(DoctorCheck {
        name: "Service graph".to_string(),
        ok: service_order(&boltfile).is_ok(),
        message: service_order(&boltfile)
            .map(|order| format!("{} service(s), dependency order valid", order.len()))
            .unwrap_or_else(|err| err.to_string()),
    });
    let service_refs = validate_service_references(&boltfile);
    checks.push(DoctorCheck {
        name: "Service references".to_string(),
        ok: service_refs.is_empty(),
        message: validation_message(&service_refs),
    });
    let ports = validate_duplicate_ports(&boltfile);
    checks.push(DoctorCheck {
        name: "Ports".to_string(),
        ok: ports.is_empty(),
        message: validation_message(&ports),
    });
    let mounts = validate_volume_mounts(&boltfile);
    checks.push(DoctorCheck {
        name: "Volume mounts".to_string(),
        ok: mounts.is_empty(),
        message: validation_message(&mounts),
    });
    let drivers = validate_network_drivers(&boltfile);
    checks.push(DoctorCheck {
        name: "Network drivers".to_string(),
        ok: drivers.is_empty(),
        message: validation_message(&drivers),
    });
    let gpu_requests = validate_gpu_requests(&boltfile);
    checks.push(DoctorCheck {
        name: "GPU requests".to_string(),
        ok: gpu_requests.is_empty(),
        message: validation_message(&gpu_requests),
    });
    let missing_digests = validate_locked_image_digests(config);
    checks.push(DoctorCheck {
        name: "Locked digests".to_string(),
        ok: missing_digests.is_empty(),
        message: validation_message(&missing_digests),
    });
    checks.push(DoctorCheck {
        name: "Lockfile".to_string(),
        ok: check_lock(config, runtime).await.is_ok(),
        message: check_lock(config, runtime)
            .await
            .map(|_| "Boltfile.lock is current".to_string())
            .unwrap_or_else(|err| err.to_string()),
    });
    checks.push(DoctorCheck {
        name: "Service discovery".to_string(),
        ok: !service_discovery_registry(&boltfile, &[])
            .services
            .is_empty()
            || boltfile.services.is_empty(),
        message: format!("{} service discovery entrie(s)", boltfile.services.len()),
    });

    ValidationReport {
        ok: checks.iter().all(|check| check.ok),
        checks,
    }
}

pub async fn self_test(config: &BoltConfig, runtime: &BoltRuntime) -> DoctorReport {
    let mut checks = doctor(config, runtime).await.checks;
    let validation = validate(config, runtime).await;
    checks.extend(validation.checks);
    let plan_check = match plan(config, runtime).await {
        Ok(plan) => DoctorCheck {
            name: "Plan".to_string(),
            ok: true,
            message: format!("{} action(s)", plan.actions.len()),
        },
        Err(err) => DoctorCheck {
            name: "Plan".to_string(),
            ok: false,
            message: err.to_string(),
        },
    };
    checks.push(plan_check);
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
                detail: gpu_action_detail(service),
            });
        } else {
            actions.push(ProjectAction {
                action: ActionKind::Create,
                resource_type: ResourceType::Service,
                name: name.clone(),
                reason: format!("container '{}' is missing", expected),
                detail: gpu_action_detail(service),
            });
        }
        if let Some(image) = &service.image {
            actions.push(ProjectAction {
                action: ActionKind::Create,
                resource_type: ResourceType::Image,
                name: image.clone(),
                reason: "image must be present or pulled during apply".to_string(),
                detail: None,
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
                detail: None,
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
                detail: None,
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
                detail: None,
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
            detail: None,
        })
        .collect())
}

async fn build_lock(
    config: &BoltConfig,
    runtime: &BoltRuntime,
    boltfile: &BoltFile,
    resolve_images: bool,
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
        let image_digest = if let Some(image) = service.image.as_deref() {
            resolve_image_digest(runtime, &image_digests, image, resolve_images).await?
        } else {
            None
        };
        services.insert(
            name.clone(),
            LockedService {
                image: service.image.clone(),
                build: service.build.clone(),
                capsule: service.capsule.clone(),
                image_digest,
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

async fn resolve_image_digest(
    runtime: &BoltRuntime,
    cached_digests: &BTreeMap<String, String>,
    image: &str,
    resolve_remote: bool,
) -> Result<Option<String>> {
    if let Some(digest) = digest_from_image_reference(image) {
        return Ok(Some(digest.to_string()));
    }
    if let Some(digest) = cached_digests.get(image) {
        return Ok(Some(digest.clone()));
    }
    if let Ok((_, metadata, _)) = runtime.inspect_image(image).await {
        return Ok(Some(metadata.digest));
    }
    if !resolve_remote || !is_tag_based_image_reference(image) {
        return Ok(None);
    }

    runtime
        .pull_image(image)
        .await
        .with_context(|| format!("failed to resolve image digest for '{}'", image))?;
    let (_, metadata, _) = runtime
        .inspect_image(image)
        .await
        .with_context(|| format!("failed to inspect resolved image '{}'", image))?;
    Ok(Some(metadata.digest))
}

fn digest_from_image_reference(image: &str) -> Option<&str> {
    image
        .split_once("@sha256:")
        .map(|(_, digest)| digest)
        .filter(|digest| digest.len() == 64 && digest.chars().all(|ch| ch.is_ascii_hexdigit()))
        .map(|digest| &image[image.len() - digest.len() - "sha256:".len()..])
}

fn is_tag_based_image_reference(image: &str) -> bool {
    image.contains(':') && !image.contains("@sha256:")
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

fn selective_apply_services(
    boltfile: &BoltFile,
    plan: &ProjectPlan,
    services: &[String],
    force_recreate: bool,
) -> Result<Vec<String>> {
    if force_recreate || !services.is_empty() {
        return ordered_target_services(boltfile, services);
    }

    let changed = plan
        .actions
        .iter()
        .filter(|action| action.resource_type == ResourceType::Service)
        .filter(|action| matches!(action.action, ActionKind::Create | ActionKind::Update))
        .map(|action| action.name.clone())
        .collect::<BTreeSet<_>>();

    let mut ordered = service_order(boltfile)?;
    ordered.retain(|service| changed.contains(service));
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

fn validate_service_references(boltfile: &BoltFile) -> Vec<String> {
    let mut issues = Vec::new();
    let networks = boltfile
        .networks
        .as_ref()
        .map(|networks| networks.keys().cloned().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    let volumes = boltfile
        .volumes
        .as_ref()
        .map(|volumes| volumes.keys().cloned().collect::<BTreeSet<_>>())
        .unwrap_or_default();

    for (service_name, service) in &boltfile.services {
        for network in service.networks.clone().unwrap_or_default() {
            if !networks.contains(&network)
                && !matches!(network.as_str(), "bridge" | "host" | "none" | "default")
            {
                issues.push(format!(
                    "service '{}' references unknown network '{}'",
                    service_name, network
                ));
            }
        }
        for mount in service.volumes.clone().unwrap_or_default() {
            if let Some((source, _)) = mount.split_once(':')
                && !source.starts_with('/')
                && !source.starts_with('.')
                && !source.starts_with('~')
                && !volumes.contains(source)
            {
                issues.push(format!(
                    "service '{}' references unknown named volume '{}'",
                    service_name, source
                ));
            }
        }
    }
    issues
}

fn validate_duplicate_ports(boltfile: &BoltFile) -> Vec<String> {
    let mut seen = BTreeMap::<String, String>::new();
    let mut issues = Vec::new();
    for (service_name, service) in &boltfile.services {
        for port in service.ports.clone().unwrap_or_default() {
            if let Some(host) = port.split(':').next()
                && !host.is_empty()
                && host.chars().all(|ch| ch.is_ascii_digit())
                && let Some(previous) = seen.insert(host.to_string(), service_name.clone())
            {
                issues.push(format!(
                    "host port '{}' is used by '{}' and '{}'",
                    host, previous, service_name
                ));
            }
        }
    }
    issues
}

fn validate_volume_mounts(boltfile: &BoltFile) -> Vec<String> {
    let mut issues = Vec::new();
    for (service_name, service) in &boltfile.services {
        for mount in service.volumes.clone().unwrap_or_default() {
            let parts = mount.split(':').collect::<Vec<_>>();
            if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
                issues.push(format!(
                    "service '{}' has invalid volume mount '{}'",
                    service_name, mount
                ));
            }
        }
    }
    issues
}

fn validate_network_drivers(boltfile: &BoltFile) -> Vec<String> {
    let mut issues = Vec::new();
    for (name, network) in boltfile.networks.as_ref().into_iter().flatten() {
        if !matches!(
            network.driver.as_str(),
            "bolt" | "gquic" | "bridge" | "host" | "none"
        ) {
            issues.push(format!(
                "network '{}' uses unsupported driver '{}'",
                name, network.driver
            ));
        }
    }
    issues
}

fn validate_gpu_requests(boltfile: &BoltFile) -> Vec<String> {
    let mut issues = Vec::new();
    for (service_name, service) in &boltfile.services {
        let Some(gaming) = service.gaming.as_ref() else {
            continue;
        };
        let Some(gpu) = gaming.gpu.as_ref() else {
            if gaming.gpu_passthrough {
                issues.push(format!(
                    "service '{}' enables gpu_passthrough without [gaming.gpu] settings",
                    service_name
                ));
            }
            continue;
        };

        if let Some(runtime) = gpu.runtime.as_deref()
            && !matches!(runtime, "nvbind" | "nvidia" | "amd" | "auto")
        {
            issues.push(format!(
                "service '{}' requests unsupported GPU runtime '{}'",
                service_name, runtime
            ));
        }
        if let Some(isolation) = gpu.isolation_level.as_deref()
            && !matches!(
                isolation,
                "shared" | "exclusive" | "virtual" | "time-sliced"
            )
        {
            issues.push(format!(
                "service '{}' requests unsupported GPU isolation '{}'",
                service_name, isolation
            ));
        }
        if gpu.nvidia.is_some() && gpu.amd.is_some() {
            issues.push(format!(
                "service '{}' requests both NVIDIA and AMD GPU configs",
                service_name
            ));
        }
        if let Some(nvbind) = gpu.nvbind.as_ref()
            && nvbind
                .devices
                .as_ref()
                .is_some_and(|devices| devices.is_empty())
        {
            issues.push(format!(
                "service '{}' has an empty nvbind device list",
                service_name
            ));
        }
    }
    issues
}

fn validate_locked_image_digests(config: &BoltConfig) -> Vec<String> {
    let Ok(lock) = read_lock(config) else {
        return vec!["Boltfile.lock is missing or unreadable".to_string()];
    };
    lock.services
        .iter()
        .filter_map(|(service, locked)| {
            locked
                .image
                .as_ref()
                .filter(|image| is_tag_based_image_reference(image))
                .and_then(|image| {
                    locked.image_digest.is_none().then(|| {
                        format!(
                            "service '{}' image '{}' is tag-based without a resolved digest",
                            service, image
                        )
                    })
                })
        })
        .collect()
}

fn gpu_action_detail(service: &Service) -> Option<String> {
    let gpu = service_gpu_inspection(service);
    if !gpu.requested {
        return None;
    }
    let mut parts = Vec::new();
    if let Some(vendor) = gpu.vendor {
        parts.push(format!("gpu={vendor}"));
    } else {
        parts.push("gpu=requested".to_string());
    }
    if let Some(runtime) = gpu.runtime {
        parts.push(format!("runtime={runtime}"));
    }
    if !gpu.devices.is_empty() {
        parts.push(format!("devices={}", gpu.devices.join(",")));
    }
    if let Some(profile) = gpu.profile {
        parts.push(format!("profile={profile}"));
    }
    Some(parts.join(" "))
}

fn service_gpu_inspection(service: &Service) -> ServiceGpuInspection {
    let mut inspection = ServiceGpuInspection::default();
    let Some(gaming) = service.gaming.as_ref() else {
        return inspection;
    };

    inspection.requested = gaming.gpu_passthrough || gaming.gpu.is_some();
    inspection.profile = gaming.performance_profile.clone();
    if gaming.gpu_passthrough && gaming.gpu.is_none() {
        inspection
            .notes
            .push("gpu_passthrough enabled without detailed GPU config".to_string());
    }

    let Some(gpu) = gaming.gpu.as_ref() else {
        return inspection;
    };
    inspection.runtime = gpu.runtime.clone();
    inspection.vendor = if gpu.nvidia.is_some() {
        Some("nvidia".to_string())
    } else if gpu.amd.is_some() {
        Some("amd".to_string())
    } else if gpu.nvbind.is_some() {
        Some("nvbind".to_string())
    } else {
        None
    };
    if let Some(nvbind) = gpu.nvbind.as_ref()
        && let Some(devices) = nvbind.devices.as_ref()
    {
        inspection.devices.extend(devices.clone());
    }
    if let Some(nvidia) = gpu.nvidia.as_ref()
        && let Some(device) = nvidia.device
    {
        inspection.devices.push(format!("nvidia:{device}"));
    }
    if let Some(amd) = gpu.amd.as_ref()
        && let Some(device) = amd.device
    {
        inspection.devices.push(format!("amd:{device}"));
    }
    if inspection.devices.is_empty() && inspection.requested {
        inspection.devices.push("all".to_string());
    }
    inspection
}

fn validation_message(issues: &[String]) -> String {
    if issues.is_empty() {
        "ok".to_string()
    } else {
        issues.join("; ")
    }
}

fn service_discovery_registry(
    boltfile: &BoltFile,
    containers: &[ContainerInfo],
) -> ServiceDiscoveryRegistry {
    let services = boltfile
        .services
        .iter()
        .map(|(name, service)| {
            let container_name = service
                .container_name
                .clone()
                .unwrap_or_else(|| service_container_name(&boltfile.project, name));
            let container = containers.iter().find(|container| {
                container.name == container_name
                    || container.names.iter().any(|alias| alias == &container_name)
            });
            let entry =
                ServiceDiscoveryEntry {
                    service: name.clone(),
                    container_name,
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
                    healthy: container
                        .map(|container| container.status.contains("running"))
                        .unwrap_or(false),
                    address: service_discovery_address(service, container),
                    address_source: service_discovery_address_source(service, container),
                    container_id: container.map(|container| container.id.clone()),
                    status: container.map(|container| container.status.clone()),
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

fn localhost_ip() -> Ipv4Addr {
    Ipv4Addr::new(127, 0, 0, 1)
}

fn service_discovery_address(_service: &Service, _container: Option<&ContainerInfo>) -> Ipv4Addr {
    localhost_ip()
}

fn service_discovery_address_source(
    service: &Service,
    container: Option<&ContainerInfo>,
) -> String {
    if service.network_mode.as_deref() == Some("host") {
        "host-network".to_string()
    } else if service
        .ports
        .as_ref()
        .is_some_and(|ports| !ports.is_empty())
    {
        "published-port-loopback".to_string()
    } else if container.is_some() {
        "container-state-no-address".to_string()
    } else {
        "configured-fallback".to_string()
    }
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
    let container = runtime
        .list_containers(true)
        .await
        .unwrap_or_default()
        .into_iter()
        .find(|container| {
            container.name == container_name
                || container.names.iter().any(|alias| alias == &container_name)
        });
    let fallback_containers = container.iter().cloned().collect::<Vec<_>>();
    let discovery = read_service_discovery(config)
        .and_then(|registry| registry.services.get(name).cloned())
        .or_else(|| {
            service_discovery_registry(&boltfile, &fallback_containers)
                .services
                .get(name)
                .cloned()
        });
    Ok(ServiceInspection {
        name: name.to_string(),
        container_name,
        gpu: service_gpu_inspection(&desired),
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
        if let Some(detail) = &action.detail {
            println!("    {}", detail);
        }
    }
}

pub fn hosts_entries(config: &BoltConfig) -> Result<Vec<String>> {
    let registry = read_service_discovery(config)
        .ok_or_else(|| anyhow!("service discovery registry not found; run bolt apply first"))?;
    Ok(registry
        .services
        .values()
        .map(|entry| format!("{} {}", entry.address, entry.dns_name))
        .collect())
}

pub fn resolve_dns_name(config: &BoltConfig, name: &str) -> Result<ServiceDiscoveryEntry> {
    let registry = read_service_discovery(config)
        .ok_or_else(|| anyhow!("service discovery registry not found; run bolt apply first"))?;
    registry
        .services
        .values()
        .find(|entry| entry.dns_name == name || entry.service == name)
        .cloned()
        .ok_or_else(|| anyhow!("service '{}' not found in discovery registry", name).into())
}

pub async fn serve_dns(config: BoltConfig, bind: SocketAddr) -> Result<()> {
    let socket = UdpSocket::bind(bind).await?;
    let mut buf = [0u8; 512];
    loop {
        let (len, peer) = socket.recv_from(&mut buf).await?;
        if let Some(response) = dns_response(&config, &buf[..len]) {
            socket.send_to(&response, peer).await?;
        }
    }
}

fn dns_response(config: &BoltConfig, query: &[u8]) -> Option<Vec<u8>> {
    if query.len() < 12 {
        return None;
    }
    let qdcount = u16::from_be_bytes([query[4], query[5]]);
    if qdcount == 0 {
        return None;
    }
    let mut cursor = 12usize;
    let mut labels = Vec::new();
    while cursor < query.len() {
        let len = *query.get(cursor)? as usize;
        cursor += 1;
        if len == 0 {
            break;
        }
        let end = cursor.checked_add(len)?;
        let label = std::str::from_utf8(query.get(cursor..end)?).ok()?;
        labels.push(label.to_string());
        cursor = end;
    }
    if cursor + 4 > query.len() {
        return None;
    }
    let qtype = u16::from_be_bytes([query[cursor], query[cursor + 1]]);
    let qclass = u16::from_be_bytes([query[cursor + 2], query[cursor + 3]]);
    let question_end = cursor + 4;
    let name = labels.join(".");
    let resolved = resolve_dns_name(config, &name).ok();
    let mut out = Vec::new();
    out.extend_from_slice(&query[0..2]);
    let flags = if resolved.is_some() && qtype == 1 && qclass == 1 {
        0x8180u16
    } else {
        0x8183u16
    };
    out.extend_from_slice(&flags.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&(if flags == 0x8180 { 1u16 } else { 0u16 }).to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&query[12..question_end]);
    if let Some(entry) = resolved
        && qtype == 1
        && qclass == 1
    {
        out.extend_from_slice(&0xC00Cu16.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&30u32.to_be_bytes());
        out.extend_from_slice(&4u16.to_be_bytes());
        out.extend_from_slice(&entry.address.octets());
    }
    Some(out)
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
        let registry = service_discovery_registry(&boltfile, &[]);
        assert_eq!(
            registry.services.get("web").unwrap().dns_name,
            "web.demo.bolt"
        );
        assert!(!registry.services.get("web").unwrap().healthy);
        assert_eq!(
            registry.services.get("web").unwrap().address_source,
            "published-port-loopback"
        );
    }

    #[test]
    fn validation_helpers_report_project_config_issues() {
        let mut services = std::collections::HashMap::new();
        services.insert(
            "web".to_string(),
            Service {
                image: Some("nginx:latest".to_string()),
                ports: Some(vec!["8080:80".to_string()]),
                volumes: Some(vec!["missing:/data".to_string(), "badmount".to_string()]),
                networks: Some(vec!["missing-net".to_string()]),
                ..Service::default()
            },
        );
        services.insert(
            "api".to_string(),
            Service {
                image: Some("api:latest".to_string()),
                ports: Some(vec!["8080:8080".to_string()]),
                ..Service::default()
            },
        );
        let mut networks = HashMap::new();
        networks.insert(
            "bad".to_string(),
            Network {
                driver: "weird".to_string(),
                driver_opts: None,
                attachable: None,
                enable_ipv6: None,
                internal: None,
                labels: None,
                ipam: None,
                external: None,
                name: None,
            },
        );
        let boltfile = BoltFile {
            project: "demo".to_string(),
            services,
            networks: Some(networks),
            volumes: None,
            snapshots: None,
        };

        assert!(!validate_service_references(&boltfile).is_empty());
        assert!(!validate_duplicate_ports(&boltfile).is_empty());
        assert!(!validate_volume_mounts(&boltfile).is_empty());
        assert!(!validate_network_drivers(&boltfile).is_empty());
    }

    #[test]
    fn digest_pinned_images_are_locked_without_registry_resolution() {
        let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let reference = format!("alpine@sha256:{digest}");
        assert_eq!(
            digest_from_image_reference(&reference),
            Some(reference[7..].as_ref())
        );
        assert!(!is_tag_based_image_reference(&reference));
        assert!(is_tag_based_image_reference("alpine:latest"));
    }

    #[test]
    fn gpu_inspection_summarizes_requested_devices() {
        let service = Service {
            gaming: Some(crate::config::GamingConfig {
                enabled: true,
                gpu_passthrough: true,
                performance_profile: Some("gaming".to_string()),
                gpu: Some(crate::config::GpuConfig {
                    runtime: Some("nvbind".to_string()),
                    nvbind: Some(crate::config::NvbindConfig {
                        devices: Some(vec!["gpu:0".to_string()]),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Service::default()
        };
        let gpu = service_gpu_inspection(&service);
        assert!(gpu.requested);
        assert_eq!(gpu.runtime.as_deref(), Some("nvbind"));
        assert_eq!(gpu.devices, vec!["gpu:0"]);
        assert!(gpu_action_detail(&service).unwrap().contains("gpu=nvbind"));
    }

    #[test]
    fn selective_apply_chooses_only_changed_services() {
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
        let plan = ProjectPlan {
            project: "demo".to_string(),
            boltfile: PathBuf::from("Boltfile.toml"),
            actions: vec![
                ProjectAction {
                    action: ActionKind::Noop,
                    resource_type: ResourceType::Service,
                    name: "db".to_string(),
                    reason: String::new(),
                    detail: None,
                },
                ProjectAction {
                    action: ActionKind::Update,
                    resource_type: ResourceType::Service,
                    name: "web".to_string(),
                    reason: String::new(),
                    detail: None,
                },
            ],
            summary: PlanSummary::default(),
        };
        assert_eq!(
            selective_apply_services(&boltfile, &plan, &[], false).unwrap(),
            vec!["web"]
        );
        assert_eq!(
            selective_apply_services(&boltfile, &plan, &[], true).unwrap(),
            vec!["db", "web"]
        );
    }
}
