use anyhow::Result;
use bolt::BoltRuntime;

#[tokio::test]
async fn test_volume_and_network_functionality() -> Result<()> {
    // Test volume creation
    let runtime = BoltRuntime::new()?;

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
    let runtime = BoltRuntime::new()?;

    // Test network creation via NetworkManager
    runtime
        .create_network("test-network", "bolt", Some("172.25.0.0/16"))
        .await?;

    // Test network listing
    let networks = runtime.list_networks().await?;
    assert!(networks.iter().any(|n| n.name == "test-network"));

    // Test network removal
    runtime.remove_network("test-network").await?;

    println!("✅ QUIC network management tests passed");
    Ok(())
}
