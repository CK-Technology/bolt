# Bolt Integration Guide for Rust Projects

This guide shows how to integrate external Rust projects with Bolt container runtime and orchestration.

## Overview

Bolt provides a next-generation container runtime written in Rust that can replace Docker/Podman workflows with superior performance, security, and declarative configuration via TOML Boltfiles.

## Integration Methods

### 1. Direct Dependency Integration

Add Bolt as a dependency to leverage its runtime capabilities:

```toml
[dependencies]
bolt = { git = "https://github.com/CK-Technology/bolt", branch = "main" }
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
```

### 2. CLI Binary Integration

Use Bolt's CLI for container orchestration:

```bash
# Install Bolt
cargo install --git https://github.com/CK-Technology/bolt

# Use in your project scripts
bolt surge up
```

### 3. Library Integration Pattern

```rust
use bolt::runtime::oci::Container;
use bolt::surge::Orchestrator;
use bolt::config::Boltfile;

// Example integration
async fn integrate_with_bolt() -> anyhow::Result<()> {
    let boltfile = Boltfile::from_path("Boltfile.toml")?;
    let orchestrator = Orchestrator::new(boltfile);
    orchestrator.surge_up().await?;
    Ok(())
}
```

## Boltfile Configuration

Create a `Boltfile.toml` in your project root:

```toml
project = "your-project"

[services.app]
build = "."
ports = ["8080:8080"]
env.RUST_LOG = "info"

[services.database]
capsule = "postgres:15"
env.POSTGRES_DB = "myapp"

[services.redis]
capsule = "redis:7"
```

## Gaming Project Integration

For gaming-related projects, leverage Bolt's specialized gaming infrastructure:

```toml
[services.game-runtime]
capsule = "gaming/proton"
gpu.nvidia = true
volumes = [
    "./games:/games",
    "/tmp/.X11-unix:/tmp/.X11-unix"
]
env.DISPLAY = ":0"

[services.game-runtime.gaming]
wine_prefix = "/games/prefix"
proton_version = "8.0"
graphics_drivers = ["nvidia"]
```

## Network Integration

Bolt provides QUIC-based networking for distributed services:

```rust
use bolt::network::quic::QuicTransport;

async fn setup_networking() -> anyhow::Result<()> {
    let transport = QuicTransport::new().await?;
    transport.bind("0.0.0.0:4433").await?;
    Ok(())
}
```

## GPU Integration

For projects requiring GPU access (gaming, ML, etc.):

```toml
[services.gpu-service]
build = "."
gpu.nvidia = true
gpu.amd = false
devices = ["/dev/dri"]
```

## Storage and Snapshots

Leverage Bolt's capsule snapshot functionality:

```bash
# Create snapshot before major changes
bolt capsule snapshot create myapp-backup

# Restore if needed
bolt capsule snapshot restore myapp-backup
```

## CI/CD Integration

Example GitHub Actions workflow:

```yaml
name: Build with Bolt
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    - name: Install Bolt
      run: cargo install --git https://github.com/CK-Technology/bolt
    - name: Build containers
      run: bolt surge build
    - name: Run tests
      run: bolt surge test
```

## Best Practices

1. **Use Capsules for Isolation**: Prefer Bolt capsules over traditional containers for better resource management
2. **Leverage TOML Configuration**: Keep all orchestration in `Boltfile.toml` for reproducibility
3. **GPU Resource Management**: Explicitly declare GPU requirements in service definitions
4. **Snapshot Workflows**: Create snapshots before deployments for easy rollbacks
5. **QUIC Networking**: Use Bolt's native QUIC transport for inter-service communication

## Migration from Docker/Podman

Replace existing Docker Compose files:

```yaml
# Old docker-compose.yml
services:
  app:
    build: .
    ports:
      - "8080:8080"
```

```toml
# New Boltfile.toml
[services.app]
build = "."
ports = ["8080:8080"]
```

## Troubleshooting

- **GPU Access Issues**: Ensure proper device permissions and driver compatibility
- **Network Conflicts**: Use Bolt's native QUIC instead of bridge networks
- **Build Failures**: Check Rust edition compatibility (Bolt uses 2024 edition)
- **Performance Issues**: Enable release builds with `bolt surge build --release`

## Example Projects

See these integration examples:
- [NVControl](./NVCONTROL.md) - NVIDIA control panel with containerized CLI
- [GhostForge](./GHOSTFORGE.md) - Gaming runtime management