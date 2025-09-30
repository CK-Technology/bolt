use bolt::volume::VolumeCreateOptions;
use std::time::Instant;

#[tokio::test]
async fn test_bolt_startup_time() {
    let start = Instant::now();

    // Test that Bolt config loads quickly
    let config = bolt::config::BoltConfig::load();

    let elapsed = start.elapsed();
    println!("Config load time: {:?}", elapsed);

    assert!(config.is_ok(), "Config should load successfully");
    assert!(elapsed.as_millis() < 100, "Config load should be < 100ms");
}

#[tokio::test]
#[ignore] // Requires write permissions to /var/lib/bolt/volumes
async fn test_volume_creation_performance() {
    let start = Instant::now();

    let mut volume_manager = bolt::volume::VolumeManager::new().unwrap();
    let volume_name = format!("perf-test-{}", uuid::Uuid::new_v4());

    // Create volume
    let create_start = Instant::now();
    let options = VolumeCreateOptions::default();
    let result = volume_manager.create_volume(&volume_name, options);
    let create_time = create_start.elapsed();

    println!("Volume creation time: {:?}", create_time);

    // Cleanup
    if result.is_ok() {
        let _ = volume_manager.remove_volume(&volume_name, false);
    }

    assert!(create_time.as_millis() < 500, "Volume creation should be < 500ms");
}

#[tokio::test]
#[ignore] // Requires write permissions to /var/lib/bolt/snapshots
async fn test_snapshot_list_performance() {
    let start = Instant::now();

    let snapshot_manager = bolt::capsules::snapshots::SnapshotManager::new()
        .await
        .unwrap();

    let result = snapshot_manager.list_snapshots().await;
    let elapsed = start.elapsed();

    println!("Snapshot list time: {:?}", elapsed);

    assert!(result.is_ok(), "Snapshot list should succeed");
    assert!(elapsed.as_millis() < 200, "Snapshot list should be < 200ms");
}

#[tokio::test]
async fn test_user_config_load_performance() {
    let start = Instant::now();

    let config = bolt::config::UserConfig::load();
    let elapsed = start.elapsed();

    println!("User config load time: {:?}", elapsed);

    assert!(config.is_ok(), "User config should load");
    assert!(elapsed.as_millis() < 50, "User config load should be < 50ms");
}

#[test]
fn test_hardware_detection_performance() {
    let start = Instant::now();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let result = runtime.block_on(async {
        bolt::runtime::hardware_detection::HardwareProfile::detect().await
    });

    let elapsed = start.elapsed();

    println!("Hardware detection time: {:?}", elapsed);

    assert!(result.is_ok(), "Hardware detection should succeed");
    assert!(elapsed.as_millis() < 1000, "Hardware detection should be < 1s");
}
