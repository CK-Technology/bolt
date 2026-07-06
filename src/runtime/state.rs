//! On-disk container state store.
//!
//! Container lifecycle state is persisted as JSON so that `bolt ps`, `stop`,
//! `restart`, `rm`, `logs`, and `exec` survive a restart of the Bolt process.
//! Both the runtime and the CLI read through this module so they agree on the
//! same location (derived from `BOLT_STORAGE_ROOT`).

use crate::Result;
use crate::runtime::oci::{ContainerState, ContainerStatus};
use crate::runtime::storage::storage_root;
use anyhow::Context;
use nix::sys::signal;
use nix::unistd::Pid;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Directory holding per-container state directories.
pub fn containers_dir() -> PathBuf {
    storage_root().join("containers")
}

/// Path to a single container's persisted state file.
pub fn state_path(id: &str) -> PathBuf {
    containers_dir().join(id).join("state.json")
}

/// Persist a container's state atomically (temp file + rename).
pub fn save(state: &ContainerState) -> Result<()> {
    let path = state_path(&state.id);
    let dir = path
        .parent()
        .expect("state_path always has a parent directory");
    std::fs::create_dir_all(dir)
        .with_context(|| format!("Failed to create state directory for {}", state.id))?;

    let json =
        serde_json::to_string_pretty(state).context("Failed to serialize container state")?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)
        .with_context(|| format!("Failed to write container state for {}", state.id))?;
    std::fs::rename(&tmp, &path)
        .with_context(|| format!("Failed to commit container state for {}", state.id))?;
    Ok(())
}

/// Load a single container's state by id. Returns `Ok(None)` when absent.
pub fn load(id: &str) -> Result<Option<ContainerState>> {
    read_state_file(&state_path(id))
}

/// Load every persisted container state, skipping (with a warning) any that
/// fail to parse so one corrupt file cannot break the whole runtime.
pub fn load_all() -> Result<Vec<ContainerState>> {
    let dir = containers_dir();
    let mut states = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(states),
        Err(err) => {
            return Err(anyhow::Error::from(err)
                .context(format!(
                    "Failed to read container state dir {}",
                    dir.display()
                ))
                .into());
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warn!("Skipping unreadable container state entry: {}", err);
                continue;
            }
        };
        let path = entry.path().join("state.json");
        match read_state_file(&path) {
            Ok(Some(state)) => states.push(state),
            Ok(None) => {}
            Err(err) => warn!(
                "Skipping corrupt container state {}: {}",
                path.display(),
                err
            ),
        }
    }

    Ok(states)
}

/// Remove a container's persisted state directory.
pub fn remove(id: &str) -> Result<()> {
    let dir = containers_dir().join(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .with_context(|| format!("Failed to remove container state for {}", id))?;
    }
    Ok(())
}

/// Resolve a user-supplied reference (id or `--name`) to a persisted state.
/// Exact id match wins; otherwise the first state whose configured name
/// matches is returned.
pub fn resolve_ref(name_or_id: &str) -> Result<Option<ContainerState>> {
    if let Some(state) = load(name_or_id)? {
        return Ok(Some(state));
    }
    Ok(load_all()?
        .into_iter()
        .find(|s| s.config.name.as_deref() == Some(name_or_id)))
}

/// If a state claims to be running but its PID is no longer alive, demote it to
/// `Exited`/`Stopped` so `ps`/`logs`/`exec` report the truth after a restart.
/// Returns `true` when the state was changed.
pub fn reconcile_liveness(state: &mut ContainerState) -> bool {
    if !matches!(state.status, ContainerStatus::Running) {
        return false;
    }
    let alive = state.pid.map(pid_is_alive).unwrap_or(false);
    if alive {
        return false;
    }
    state.status = match state.exit_code {
        Some(code) => ContainerStatus::Exited(code),
        None => ContainerStatus::Stopped,
    };
    state.pid = None;
    if state.finished.is_none() {
        state.finished = Some(std::time::SystemTime::now());
    }
    true
}

/// Whether a PID currently exists (via `kill(pid, 0)`).
pub fn pid_is_alive(pid: u32) -> bool {
    signal::kill(Pid::from_raw(pid as i32), None).is_ok()
}

fn read_state_file(path: &Path) -> Result<Option<ContainerState>> {
    match std::fs::read_to_string(path) {
        Ok(json) => {
            let state = serde_json::from_str(&json)
                .with_context(|| format!("Failed to parse container state {}", path.display()))?;
            Ok(Some(state))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(anyhow::Error::from(err)
            .context(format!("Failed to read container state {}", path.display()))
            .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::oci::{ContainerConfig, ContainerState, ContainerStatus};
    use std::collections::HashMap;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    // `BOLT_STORAGE_ROOT` is process-global; serialize tests that mutate it.
    fn env_guard() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn sample_state(id: &str, name: Option<&str>) -> ContainerState {
        ContainerState {
            id: id.to_string(),
            status: ContainerStatus::Running,
            pid: Some(1),
            bundle_path: PathBuf::from("/nonexistent").join(id),
            config: ContainerConfig {
                id: id.to_string(),
                name: name.map(str::to_string),
                image: "alpine:latest".to_string(),
                command: vec![],
                args: vec![],
                env: HashMap::new(),
                working_dir: None,
                user: None,
                hostname: None,
                network_mode: "bridge".to_string(),
                ports: vec![],
                volumes: vec![],
                capabilities: vec![],
                resource_limits: None,
                gaming_config: None,
                detach: true,
                privileged: false,
                tty: false,
                readonly_rootfs: false,
                seccomp: None,
            },
            created: std::time::SystemTime::now(),
            started: Some(std::time::SystemTime::now()),
            finished: None,
            exit_code: None,
            image_digest: Some("sha256:abc".to_string()),
            log_path: None,
            gpu_allocation: None,
        }
    }

    #[test]
    fn save_load_resolve_round_trip() {
        let _guard = env_guard();
        let tmp = scratch_tempdir();
        unsafe {
            std::env::set_var("BOLT_STORAGE_ROOT", tmp.path());
        }

        let a = sample_state("bolt-aaaa1111", Some("web"));
        let b = sample_state("bolt-bbbb2222", None);
        save(&a).unwrap();
        save(&b).unwrap();

        assert_eq!(load("bolt-aaaa1111").unwrap().unwrap().id, a.id);
        assert_eq!(load_all().unwrap().len(), 2);
        // Resolve by name and by id.
        assert_eq!(resolve_ref("web").unwrap().unwrap().id, "bolt-aaaa1111");
        assert_eq!(resolve_ref("bolt-bbbb2222").unwrap().unwrap().id, b.id);
        assert!(resolve_ref("missing").unwrap().is_none());

        remove("bolt-aaaa1111").unwrap();
        assert!(load("bolt-aaaa1111").unwrap().is_none());
        assert_eq!(load_all().unwrap().len(), 1);

        unsafe {
            std::env::remove_var("BOLT_STORAGE_ROOT");
        }
    }

    fn scratch_tempdir() -> tempfile::TempDir {
        std::fs::create_dir_all(".scratch").expect("create repo-local scratch directory");
        tempfile::tempdir_in(".scratch").expect("create repo-local scratch tempdir")
    }

    #[test]
    fn reconcile_liveness_demotes_dead_running_pid() {
        // PID 0 is not a real process from a normal user; kill(0,0) targets the
        // process group, so use a PID that is essentially never alive.
        let mut state = sample_state("bolt-dead0000", None);
        state.pid = Some(u32::MAX - 1);
        let changed = reconcile_liveness(&mut state);
        assert!(changed);
        assert!(matches!(state.status, ContainerStatus::Stopped));
        assert!(state.pid.is_none());
        assert!(state.finished.is_some());
    }
}
