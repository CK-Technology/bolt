# Bolt Documentation

## Quick Links

| Document | Description |
|----------|-------------|
| [Quickstart](getting-started/quickstart.md) | Installation and first container |
| [CLI Reference](reference/cli.md) | Commands and options |
| [Rust API](reference/rust-api.md) | Programmatic container management |
| [Native Service Tools](reference/native-tools.md) | Built-in docker-mcp-like service tooling |
| [GPU Overview](gpu/overview.md) | Native NVIDIA, AMD, and Intel GPU passthrough |
| [Gaming Workloads](workloads/gaming.md) | Gaming profiles and optimization |
| [AI/ML Workloads](workloads/ai-ml.md) | Ollama, training, and inference |
| [Surge Orchestration](operations/orchestration.md) | Multi-service Boltfile stacks |
| [Networking](operations/networking.md) | QUIC networks, DNS, and ports |
| [Snapshots](operations/snapshots.md) | BTRFS/ZFS automation |

## Overview

Bolt is a container runtime with native GPU support. The NVIDIA nvbind path is built into Bolt, so it does not require the external nvbind crate or nvidia-container-toolkit.

**Core features:**
- Multi-vendor GPU passthrough (NVIDIA, AMD, Intel)
- CDI v0.6.0 specification generation
- Gaming and AI/ML profile system
- Surge orchestration (docker-compose alternative)
- BTRFS/ZFS snapshot automation
- QUIC networking
- Docker-compatible CLI

## Architecture

```
bolt
├── Runtime        # OCI container execution
├── GPU Manager    # Multi-vendor GPU detection
│   ├── NVIDIA     # Built-in nvbind support
│   ├── AMD        # ROCm integration
│   └── Intel      # oneAPI/Level Zero
├── Profiles       # Gaming & AI optimizations
├── CDI            # Container Device Interface
├── Surge          # Orchestration (Boltfile.toml)
└── Snapshots      # BTRFS/ZFS automation
```

## Structure

```
docs/
├── getting-started/
├── gpu/
├── operations/
├── reference/
└── workloads/
```
