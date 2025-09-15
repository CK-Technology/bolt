use bolt::capsules::{Capsule, CapsuleTemplate, SnapshotManager};
use bolt::runtime::BoltRuntime;
use tempfile::TempDir;
use tokio;

#[tokio::test]
async fn test_capsule_creation() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    // Create a capsule
    let capsule = Capsule::new(
        "test-capsule",
        temp_dir.path().to_str().unwrap()
    );

    assert!(capsule.is_ok());
    let capsule = capsule.unwrap();
    assert_eq!(capsule.name(), "test-capsule");
    assert!(capsule.is_running() == false);
}

#[tokio::test]
async fn test_capsule_lifecycle() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    let mut capsule = Capsule::new(
        "lifecycle-test",
        temp_dir.path().to_str().unwrap()
    ).unwrap();

    // Start capsule
    let start_result = capsule.start().await;
    assert!(start_result.is_ok());
    assert!(capsule.is_running());

    // Stop capsule
    let stop_result = capsule.stop().await;
    assert!(stop_result.is_ok());
    assert!(!capsule.is_running());

    // Destroy capsule
    let destroy_result = capsule.destroy().await;
    assert!(destroy_result.is_ok());
}

#[tokio::test]
async fn test_capsule_with_resources() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    let mut capsule = Capsule::new(
        "resource-test",
        temp_dir.path().to_str().unwrap()
    ).unwrap();

    // Set resource limits
    capsule.set_memory_limit("512M");
    capsule.set_cpu_limit(1.5);
    capsule.set_storage_limit("10G");

    assert_eq!(capsule.memory_limit(), Some("512M".to_string()));
    assert_eq!(capsule.cpu_limit(), Some(1.5));
    assert_eq!(capsule.storage_limit(), Some("10G".to_string()));

    // Start with resource limits
    let start_result = capsule.start().await;
    assert!(start_result.is_ok());

    // Cleanup
    capsule.stop().await.ok();
    capsule.destroy().await.ok();
}

#[tokio::test]
async fn test_capsule_snapshots() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    let mut capsule = Capsule::new(
        "snapshot-test",
        temp_dir.path().to_str().unwrap()
    ).unwrap();

    // Start capsule
    capsule.start().await.unwrap();

    // Create snapshot
    let snapshot_manager = SnapshotManager::new(temp_dir.path().to_str().unwrap());
    let snapshot_result = snapshot_manager.create_snapshot(&capsule, "test-snapshot").await;
    assert!(snapshot_result.is_ok());

    // List snapshots
    let snapshots = snapshot_manager.list_snapshots(&capsule).await;
    assert!(snapshots.is_ok());
    let snapshots = snapshots.unwrap();
    assert!(snapshots.iter().any(|s| s.name == "test-snapshot"));

    // Restore snapshot
    let restore_result = snapshot_manager.restore_snapshot(&mut capsule, "test-snapshot").await;
    assert!(restore_result.is_ok());

    // Delete snapshot
    let delete_result = snapshot_manager.delete_snapshot(&capsule, "test-snapshot").await;
    assert!(delete_result.is_ok());

    // Cleanup
    capsule.stop().await.ok();
    capsule.destroy().await.ok();
}

#[tokio::test]
async fn test_capsule_templates() {
    let temp_dir = TempDir::new().unwrap();

    // Create template
    let template = CapsuleTemplate::new("postgres-template")
        .with_base_image("postgres:15")
        .with_env("POSTGRES_USER", "admin")
        .with_env("POSTGRES_PASSWORD", "secret")
        .with_port(5432)
        .with_volume("/var/lib/postgresql/data")
        .build();

    assert!(template.is_ok());
    let template = template.unwrap();
    assert_eq!(template.name(), "postgres-template");

    // Create capsule from template
    let capsule = template.instantiate(
        "postgres-instance",
        temp_dir.path().to_str().unwrap()
    );

    assert!(capsule.is_ok());
    let mut capsule = capsule.unwrap();
    assert_eq!(capsule.name(), "postgres-instance");

    // Verify template settings applied
    assert!(capsule.env_vars().contains_key("POSTGRES_USER"));
    assert!(capsule.ports().contains(&5432));

    // Cleanup
    capsule.destroy().await.ok();
}

#[tokio::test]
async fn test_capsule_networking() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    // Create network
    runtime.create_network("capsule-net", "bridge", Some("172.31.0.0/16")).await.unwrap();

    let mut capsule = Capsule::new(
        "net-test",
        temp_dir.path().to_str().unwrap()
    ).unwrap();

    // Attach to network
    capsule.attach_network("capsule-net", Some("172.31.0.10"));

    // Start capsule with network
    let start_result = capsule.start().await;
    assert!(start_result.is_ok());

    // Verify network attachment
    assert!(capsule.networks().contains(&"capsule-net".to_string()));
    assert_eq!(capsule.ip_address("capsule-net"), Some("172.31.0.10".to_string()));

    // Cleanup
    capsule.stop().await.ok();
    capsule.destroy().await.ok();
    runtime.remove_network("capsule-net").await.ok();
}

#[tokio::test]
async fn test_capsule_isolation() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    // Create two isolated capsules
    let mut capsule1 = Capsule::new(
        "isolated-1",
        temp_dir.path().join("capsule1").to_str().unwrap()
    ).unwrap();

    let mut capsule2 = Capsule::new(
        "isolated-2",
        temp_dir.path().join("capsule2").to_str().unwrap()
    ).unwrap();

    // Start both capsules
    capsule1.start().await.unwrap();
    capsule2.start().await.unwrap();

    // Verify isolation
    assert_ne!(capsule1.pid(), capsule2.pid());
    assert_ne!(capsule1.namespace(), capsule2.namespace());

    // Cleanup
    capsule1.stop().await.ok();
    capsule2.stop().await.ok();
    capsule1.destroy().await.ok();
    capsule2.destroy().await.ok();
}

#[tokio::test]
async fn test_capsule_migration() {
    let runtime = BoltRuntime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();

    let source_path = temp_dir.path().join("source");
    let target_path = temp_dir.path().join("target");

    let mut capsule = Capsule::new(
        "migration-test",
        source_path.to_str().unwrap()
    ).unwrap();

    // Start capsule
    capsule.start().await.unwrap();

    // Create checkpoint
    let checkpoint_result = capsule.checkpoint("pre-migration").await;
    assert!(checkpoint_result.is_ok());

    // Stop capsule
    capsule.stop().await.unwrap();

    // Migrate to new location
    let migrate_result = capsule.migrate(target_path.to_str().unwrap()).await;
    assert!(migrate_result.is_ok());

    // Restore from checkpoint
    let restore_result = capsule.restore("pre-migration").await;
    assert!(restore_result.is_ok());

    // Start at new location
    capsule.start().await.unwrap();
    assert!(capsule.is_running());

    // Cleanup
    capsule.stop().await.ok();
    capsule.destroy().await.ok();
}