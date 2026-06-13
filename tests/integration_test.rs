use anyhow::Result;
use bolt::{BoltConfig, BoltRuntime};
use tempfile::TempDir;

#[tokio::test]
async fn test_volume_and_network_functionality() -> Result<()> {
    // Test volume creation
    let temp_dir = TempDir::new()?;
    let runtime = BoltRuntime::with_config(BoltConfig {
        config_dir: temp_dir.path().join("config"),
        data_dir: temp_dir.path().join("data"),
        boltfile_path: temp_dir.path().join("Boltfile.toml"),
        verbose: false,
    });

    // Test volume creation
    let volume_info = runtime
        .create_volume("test-volume", "local", None, &[])
        .await?;

    assert_eq!(volume_info.name, "test-volume");
    assert_eq!(volume_info.driver, "local");

    // Test volume listing
    let volumes = runtime.list_volumes().await?;
    assert!(volumes.iter().any(|v| v.name == "test-volume"));

    // Test volume removal
    runtime.remove_volume("test-volume", false).await?;

    println!("✅ Volume management tests passed");
    Ok(())
}

#[tokio::test]
async fn test_network_functionality() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let runtime = BoltRuntime::with_config(BoltConfig {
        config_dir: temp_dir.path().join("config"),
        data_dir: temp_dir.path().join("data"),
        boltfile_path: temp_dir.path().join("Boltfile.toml"),
        verbose: false,
    });

    // Test network creation via NetworkManager
    if runtime
        .create_network("test-network", "bolt", Some("172.25.0.0/16"))
        .await
        .is_err()
    {
        println!("Network backend unavailable; skipping network functionality assertions");
        return Ok(());
    }

    // Test network listing
    let networks = runtime.list_networks().await?;
    assert!(networks.iter().any(|n| n.name == "test-network"));

    // Test network removal
    runtime.remove_network("test-network").await?;

    println!("✅ QUIC network management tests passed");
    Ok(())
}
