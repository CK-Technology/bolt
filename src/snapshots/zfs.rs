//! ZFS snapshot support.

use super::{FilesystemType, Snapshot, SnapshotConfig, SnapshotType};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use tokio::process::Command;

pub async fn create_snapshot(
    config: &SnapshotConfig,
    snapshot_id: &str,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<Snapshot> {
    let dataset = resolve_dataset(&config.root_path).await?;
    let snapshot_name = format!("{dataset}@{snapshot_id}");

    let output = Command::new("zfs")
        .arg("snapshot")
        .arg("-o")
        .arg("bolt:type=manual")
        .arg("-o")
        .arg(format!("bolt:name={}", name.unwrap_or(snapshot_id)))
        .arg("-o")
        .arg(format!("bolt:description={}", description.unwrap_or("")))
        .arg(&snapshot_name)
        .output()
        .await
        .context("Failed to execute zfs snapshot")?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "failed to create ZFS snapshot {}: {}",
            snapshot_name,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(Snapshot {
        id: snapshot_id.to_string(),
        name: name.map(String::from),
        description: description.map(String::from),
        timestamp: chrono::Utc::now(),
        snapshot_type: SnapshotType::Manual,
        filesystem_type: FilesystemType::ZFS,
        path: config.snapshot_path.join(snapshot_id),
        size_bytes: None,
        parent: Some(dataset),
        gpu_state: None,
    })
}

pub async fn list_snapshots(config: &SnapshotConfig) -> Result<Vec<Snapshot>> {
    let dataset = resolve_dataset(&config.root_path).await?;
    let output = Command::new("zfs")
        .args([
            "list",
            "-H",
            "-t",
            "snapshot",
            "-o",
            "name,creation,used,bolt:name,bolt:description,bolt:type",
            "-r",
            &dataset,
        ])
        .output()
        .await
        .context("Failed to execute zfs list")?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "failed to list ZFS snapshots for {}: {}",
            dataset,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter_map(|line| parse_zfs_snapshot_line(line, &dataset, &config.snapshot_path))
        .collect())
}

pub async fn rollback_snapshot(config: &SnapshotConfig, snapshot_id: &str) -> Result<()> {
    let dataset = resolve_dataset(&config.root_path).await?;
    let rescue_id = format!(
        "bolt-pre-rollback-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let rescue_snapshot = format!("{dataset}@{rescue_id}");
    let rescue = Command::new("zfs")
        .arg("snapshot")
        .arg("-o")
        .arg("bolt:type=auto")
        .arg("-o")
        .arg("bolt:description=Pre-rollback rescue snapshot")
        .arg(&rescue_snapshot)
        .output()
        .await
        .context("Failed to execute zfs rescue snapshot")?;
    if !rescue.status.success() {
        return Err(anyhow::anyhow!(
            "failed to create ZFS rescue snapshot {}: {}",
            rescue_snapshot,
            String::from_utf8_lossy(&rescue.stderr).trim()
        ));
    }

    let snapshot_name = format!("{dataset}@{snapshot_id}");
    let output = Command::new("zfs")
        .args(["rollback", "-r", &snapshot_name])
        .output()
        .await
        .context("Failed to execute zfs rollback")?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "failed to rollback ZFS snapshot {}: {}",
            snapshot_name,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

pub async fn delete_snapshot(config: &SnapshotConfig, snapshot_id: &str) -> Result<()> {
    let dataset = resolve_dataset(&config.root_path).await?;
    let snapshot_name = format!("{dataset}@{snapshot_id}");
    let holds = list_holds(&snapshot_name).await?;
    if !holds.is_empty() {
        return Err(anyhow::anyhow!(
            "cannot delete ZFS snapshot {}; holds are present: {}",
            snapshot_name,
            holds.join(", ")
        ));
    }
    let output = Command::new("zfs")
        .args(["destroy", &snapshot_name])
        .output()
        .await
        .context("Failed to execute zfs destroy")?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "failed to delete ZFS snapshot {}: {}",
            snapshot_name,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

async fn list_holds(snapshot_name: &str) -> Result<Vec<String>> {
    let output = Command::new("zfs")
        .args(["holds", "-H", snapshot_name])
        .output()
        .await
        .context("Failed to execute zfs holds")?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "failed to list ZFS holds for {}: {}",
            snapshot_name,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(parse_zfs_holds(&String::from_utf8_lossy(&output.stdout)))
}

async fn resolve_dataset(root_path: &PathBuf) -> Result<String> {
    let output = Command::new("findmnt")
        .arg("-n")
        .arg("-o")
        .arg("SOURCE")
        .arg("--target")
        .arg(root_path)
        .output()
        .await
        .context("Failed to resolve ZFS dataset with findmnt")?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "failed to resolve filesystem source for {}: {}",
            root_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let source = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if source.is_empty() || source.starts_with('/') {
        return Err(anyhow::anyhow!(
            "filesystem source '{}' is not a ZFS dataset",
            source
        ));
    }
    Ok(source)
}

fn parse_zfs_snapshot_line(line: &str, dataset: &str, snapshot_path: &PathBuf) -> Option<Snapshot> {
    let mut parts = line.split('\t');
    let name = parts.next()?.trim();
    let creation = parts.next().unwrap_or("").trim();
    let used = parts.next().unwrap_or("").trim();
    let bolt_name = empty_dash_to_none(parts.next().unwrap_or("").trim());
    let description = empty_dash_to_none(parts.next().unwrap_or("").trim());
    let snapshot_type = parse_snapshot_type(parts.next().unwrap_or("").trim());
    let prefix = format!("{dataset}@");
    let snapshot_id = name.strip_prefix(&prefix)?;
    if !snapshot_id.starts_with("bolt-") {
        return None;
    }

    Some(Snapshot {
        id: snapshot_id.to_string(),
        name: bolt_name,
        description,
        timestamp: parse_zfs_creation(creation).unwrap_or_else(Utc::now),
        snapshot_type,
        filesystem_type: FilesystemType::ZFS,
        path: snapshot_path.join(snapshot_id),
        size_bytes: parse_zfs_size(used),
        parent: Some(dataset.to_string()),
        gpu_state: None,
    })
}

fn parse_zfs_creation(value: &str) -> Option<DateTime<Utc>> {
    if let Ok(timestamp) = value.parse::<i64>() {
        return DateTime::<Utc>::from_timestamp(timestamp, 0);
    }

    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

fn parse_snapshot_type(value: &str) -> SnapshotType {
    match value.to_ascii_lowercase().as_str() {
        "auto" => SnapshotType::Auto,
        "hourly" => SnapshotType::Hourly,
        "daily" => SnapshotType::Daily,
        "weekly" => SnapshotType::Weekly,
        "monthly" => SnapshotType::Monthly,
        other if !other.is_empty() && other != "-" => SnapshotType::Named(other.to_string()),
        _ => SnapshotType::Manual,
    }
}

fn empty_dash_to_none(value: &str) -> Option<String> {
    match value {
        "" | "-" => None,
        other => Some(other.to_string()),
    }
}

fn parse_zfs_holds(output: &str) -> Vec<String> {
    let mut holds: Vec<_> = output
        .lines()
        .filter_map(|line| line.split('\t').nth(1))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    holds.sort();
    holds.dedup();
    holds
}

fn parse_zfs_size(value: &str) -> Option<u64> {
    let value = value.trim();
    if value.is_empty() || value == "-" {
        return None;
    }
    let split = value
        .find(|ch: char| !ch.is_ascii_digit() && ch != '.')
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let number = number.parse::<f64>().ok()?;
    let multiplier = match unit.trim().to_ascii_uppercase().as_str() {
        "" | "B" => 1.0,
        "K" => 1024.0,
        "M" => 1024.0 * 1024.0,
        "G" => 1024.0 * 1024.0 * 1024.0,
        "T" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((number * multiplier) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zfs_size_parser_handles_units() {
        assert_eq!(parse_zfs_size("512B"), Some(512));
        assert_eq!(parse_zfs_size("1.5K"), Some(1536));
        assert_eq!(parse_zfs_size("2M"), Some(2 * 1024 * 1024));
        assert_eq!(parse_zfs_size("-"), None);
    }

    #[test]
    fn zfs_snapshot_line_parser_filters_to_bolt_snapshots() {
        let root = PathBuf::from("/.snapshots");
        let snapshot = parse_zfs_snapshot_line(
            "tank/bolt@bolt-20260705-120000\t1783267200\t1M\tprod\tbefore deploy\tdaily",
            "tank/bolt",
            &root,
        )
        .expect("bolt snapshot line");
        assert_eq!(snapshot.id, "bolt-20260705-120000");
        assert_eq!(snapshot.name.as_deref(), Some("prod"));
        assert_eq!(snapshot.description.as_deref(), Some("before deploy"));
        assert!(matches!(snapshot.snapshot_type, SnapshotType::Daily));
        assert_eq!(snapshot.size_bytes, Some(1024 * 1024));
        assert!(
            parse_zfs_snapshot_line("tank/bolt@manual\tdate\t1M", "tank/bolt", &root).is_none()
        );
    }

    #[test]
    fn zfs_holds_parser_returns_hold_names() {
        assert_eq!(
            parse_zfs_holds("tank/bolt@snap\tkeep\t2026\n tank/bolt@snap\tbackup\t2026\n"),
            vec!["backup", "keep"]
        );
    }
}
