# Rust API

Bolt provides a Rust API for programmatic container management.

## Installation

```toml
[dependencies]
bolt = { git = "https://github.com/CK-Technology/bolt" }
tokio = { version = "1.0", features = ["full"] }
```

## Basic Usage

```rust
use bolt::api::*;

#[tokio::main]
async fn main() -> bolt::Result<()> {
    let runtime = BoltRuntime::new()?;

    // Run a container
    runtime.run_container(
        "nginx:latest",
        Some("web"),
        &["8080:80"],
        &[],  // volumes
        &[],  // env
        false // detach
    ).await?;

    Ok(())
}
```

## Container Management

### Run Container
```rust
runtime.run_container(
    "ubuntu:latest",      // image
    Some("my-container"), // name (optional)
    &["8080:80"],         // ports
    &["./data:/data"],    // volumes
    &["KEY=value"],       // environment
    true                  // detach
).await?;
```

### List Containers
```rust
let containers = runtime.list_containers(true).await?; // true = show all
for c in containers {
    println!("{}: {}", c.name, c.status);
}
```

### Stop/Remove
```rust
runtime.stop_container("my-container").await?;
runtime.remove_container("my-container", true).await?; // true = force
```

## GPU Support

```rust
use bolt::gpu::*;

// Detect GPUs
let gpus = detect_gpus()?;
for gpu in gpus {
    println!("{}: {}", gpu.name, gpu.driver);
}

// Run with GPU
runtime.run_container_with_gpu(
    "nvidia/cuda:12.0-base",
    GpuConfig {
        devices: GpuDevices::All,
        profile: Some("ollama-medium".to_string()),
    }
).await?;
```

## Orchestration

### Surge Up/Down
```rust
// Load Boltfile.toml and start services
runtime.surge_up(&[], false, false).await?;

// Stop all services
runtime.surge_down().await?;
```

### Programmatic Boltfile
```rust
let boltfile = BoltFileBuilder::new("my-project")
    .add_service("web", ServiceConfig {
        image: "nginx:latest".to_string(),
        ports: vec!["8080:80".to_string()],
        ..Default::default()
    })
    .add_service("api", ServiceConfig {
        image: "myapi:latest".to_string(),
        depends_on: vec!["db".to_string()],
        ..Default::default()
    })
    .build();

runtime.surge_up_with_config(&boltfile).await?;
```

## Networking

```rust
// Create network
runtime.create_network(
    "my-network",
    "bolt",
    Some("172.20.0.0/16")
).await?;

// List networks
let networks = runtime.list_networks().await?;

// Remove network
runtime.remove_network("my-network").await?;
```

## Volumes

```rust
// Create volume
runtime.create_volume("my-volume", None).await?;

// List volumes
let volumes = runtime.list_volumes().await?;

// Remove volume
runtime.remove_volume("my-volume").await?;
```

## Snapshots

```rust
use bolt::snapshots::*;

// Create snapshot
create_snapshot("before-update", Some("Pre-update snapshot"))?;

// List snapshots
let snapshots = list_snapshots()?;

// Rollback
rollback_snapshot("before-update")?;
```

## Feature Flags

```toml
[dependencies.bolt]
git = "https://github.com/CK-Technology/bolt"
features = ["gaming", "nvidia-support"]
```

| Feature | Description |
|---------|-------------|
| `gaming` | Gaming optimizations, Wine/Proton |
| `nvidia-support` | NVIDIA GPU passthrough |
| `amd-support` | AMD GPU support |
| `quic-networking` | QUIC protocol networking |
| `oci-runtime` | Full OCI container support |

## Error Handling

```rust
use bolt::error::BoltError;

match runtime.run_container("invalid:image", None, &[], &[], &[], false).await {
    Ok(_) => println!("Started"),
    Err(BoltError::ImageNotFound(name)) => println!("Image not found: {}", name),
    Err(BoltError::GpuNotAvailable) => println!("No GPU available"),
    Err(e) => println!("Error: {}", e),
}
```

## Examples

### Gaming Container
```rust
let gaming_config = GamingConfig {
    gpu: Some(GpuConfig {
        devices: GpuDevices::All,
        profile: Some("cyberpunk 2077".to_string()),
    }),
    wine: Some(WineConfig {
        version: "8.0".to_string(),
        prefix: "/home/user/.wine".to_string(),
    }),
    audio: Some(AudioConfig {
        system: AudioSystem::PipeWire,
        latency: Latency::Low,
    }),
};

runtime.run_gaming_container("steam:latest", gaming_config).await?;
```

### ML Inference
```rust
let ml_config = GpuConfig {
    devices: GpuDevices::All,
    profile: Some("ollama-medium".to_string()),
};

runtime.run_container_with_gpu(
    "ollama/ollama",
    ml_config
).await?;
```
