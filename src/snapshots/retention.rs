//! Snapshot Retention Policy Management

use super::{RetentionPolicy, Snapshot, SnapshotType};
use anyhow::Result;
use std::collections::HashSet;
use tracing::{debug, info};

/// Apply retention policy to snapshots
pub async fn apply_retention_policy(
    snapshots: &[Snapshot],
    policy: &RetentionPolicy,
) -> Result<Vec<String>> {
    info!("📋 Applying retention policy");
    debug!("  • Keep hourly: {}", policy.keep_hourly);
    debug!("  • Keep daily: {}", policy.keep_daily);
    debug!("  • Keep weekly: {}", policy.keep_weekly);
    debug!("  • Keep monthly: {}", policy.keep_monthly);
    debug!("  • Keep yearly: {}", policy.keep_yearly);

    let snapshots_to_delete = calculate_snapshots_to_delete(snapshots, policy);

    info!(
        "✅ Retention policy applied: {} snapshots marked for deletion",
        snapshots_to_delete.len()
    );
    Ok(snapshots_to_delete)
}

/// Calculate which snapshots should be deleted based on retention policy
pub fn calculate_snapshots_to_delete(
    snapshots: &[Snapshot],
    policy: &RetentionPolicy,
) -> Vec<String> {
    let mut keep = HashSet::new();

    keep_newest_matching(
        snapshots,
        policy.keep_hourly as usize,
        |snapshot| matches!(snapshot.snapshot_type, SnapshotType::Hourly),
        &mut keep,
    );
    keep_newest_matching(
        snapshots,
        policy.keep_daily as usize,
        |snapshot| {
            matches!(
                snapshot.snapshot_type,
                SnapshotType::Daily | SnapshotType::Auto
            )
        },
        &mut keep,
    );
    keep_newest_matching(
        snapshots,
        policy.keep_weekly as usize,
        |snapshot| matches!(snapshot.snapshot_type, SnapshotType::Weekly),
        &mut keep,
    );
    keep_newest_matching(
        snapshots,
        policy.keep_monthly as usize,
        |snapshot| matches!(snapshot.snapshot_type, SnapshotType::Monthly),
        &mut keep,
    );

    let mut to_delete: Vec<_> = snapshots
        .iter()
        .filter(|snapshot| {
            !matches!(
                snapshot.snapshot_type,
                SnapshotType::Manual | SnapshotType::Named(_)
            ) && !keep.contains(&snapshot.id)
        })
        .map(|snapshot| snapshot.id.clone())
        .collect();
    to_delete.sort();
    to_delete
}

fn keep_newest_matching<F>(
    snapshots: &[Snapshot],
    limit: usize,
    matches_type: F,
    keep: &mut HashSet<String>,
) where
    F: Fn(&Snapshot) -> bool,
{
    let mut matching: Vec<_> = snapshots
        .iter()
        .filter(|snapshot| matches_type(snapshot))
        .collect();
    matching.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    for snapshot in matching.into_iter().take(limit) {
        keep.insert(snapshot.id.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshots::FilesystemType;
    use chrono::{Duration, TimeZone, Utc};
    use std::path::PathBuf;

    fn snapshot(id: &str, age_days: i64, snapshot_type: SnapshotType) -> Snapshot {
        Snapshot {
            id: id.to_string(),
            name: None,
            description: None,
            timestamp: Utc.with_ymd_and_hms(2026, 7, 5, 12, 0, 0).unwrap()
                - Duration::days(age_days),
            snapshot_type,
            filesystem_type: FilesystemType::BTRFS,
            path: PathBuf::from(id),
            size_bytes: None,
            parent: None,
            gpu_state: None,
        }
    }

    #[test]
    fn retention_keeps_newest_per_bucket_and_preserves_manual() {
        let snapshots = vec![
            snapshot("daily-new", 0, SnapshotType::Daily),
            snapshot("daily-old", 2, SnapshotType::Daily),
            snapshot("hourly-new", 0, SnapshotType::Hourly),
            snapshot("hourly-old", 1, SnapshotType::Hourly),
            snapshot("manual-old", 99, SnapshotType::Manual),
        ];
        let policy = RetentionPolicy {
            keep_hourly: 1,
            keep_daily: 1,
            keep_weekly: 0,
            keep_monthly: 0,
            keep_yearly: 0,
        };

        let delete = calculate_snapshots_to_delete(&snapshots, &policy);
        assert_eq!(delete, vec!["daily-old", "hourly-old"]);
    }
}
