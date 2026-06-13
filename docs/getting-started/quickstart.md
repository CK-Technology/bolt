# Getting Started

## Installation

```bash
# Install from script
curl -fsSL https://bolt.cktech.sh | bash

# Or build from source
git clone https://github.com/CK-Technology/bolt
cd bolt
cargo build --release
```

## Requirements

- Linux kernel 5.4+
- Rust 1.96+ (for building)
- BTRFS or ZFS (for snapshots)
- GPU drivers (for GPU passthrough)

## First Container

```bash
# Run a container
bolt run ubuntu:latest

# Run with GPU
bolt run --gpu all nvidia/cuda:12.0-base nvidia-smi

# Run with port mapping
bolt run -p 8080:80 --name web nginx:latest
```

## Check GPU Support

```bash
# NVIDIA
bolt nv info
bolt nv doctor

# AMD
bolt amd info

# Intel
bolt arc info
```

## Configuration

Create a `Boltfile.toml` for multi-service setups:

```toml
project = "my-app"

[services.web]
image = "nginx:latest"
ports = ["8080:80"]

[services.api]
image = "node:20"
ports = ["3000:3000"]
volumes = ["./app:/app"]
```

Launch with:
```bash
bolt surge up
```

## Next Steps

- [CLI Reference](../reference/cli.md) - Full command documentation
- [GPU Support](../gpu/overview.md) - Configure GPU passthrough
- [Gaming](../workloads/gaming.md) - Gaming container setup
