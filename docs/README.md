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

A single `bolt` CLI drives every subsystem. The runtime executes containers
(natively where possible, delegating to podman/docker otherwise); GPU, networking,
orchestration, and snapshots are layered services it composes. Dashed edges mark
paths that are planned or still rolling out.

```mermaid
flowchart TB
    CLI["bolt CLI<br/>run · surge · nv / amd / arc · snapshot"]

    subgraph RT["Runtime"]
        UR["UnifiedRuntime<br/>native or delegate"]
        OCI["OCI execution<br/>podman / docker fallback"]
        CAP["Bolt capsule (bolt://)<br/>LXC-like · planned"]
    end

    subgraph GPU["GPU — built-in nvbind engine"]
        NV["NVIDIA<br/>detect · passthrough · CDI"]
        AMD["AMD<br/>detect only · experimental"]
        INT["Intel<br/>detect only · experimental"]
    end

    subgraph NET["Networking"]
        BR["Bridge / veth · NAT<br/>port forwarding"]
        QUIC["QUIC transport<br/>encrypted · multiplexed"]
    end

    subgraph ORCH["Surge orchestration"]
        BF["Boltfile.toml parser"]
        DEP["dependency ordering"]
    end

    SNAP["Snapshots<br/>BTRFS / ZFS automation"]

    CLI --> UR
    UR --> OCI
    UR -.-> CAP
    CLI --> NV
    CLI --> AMD
    CLI --> INT
    UR --> BR
    BR --> QUIC
    CLI --> BF
    BF --> DEP
    DEP --> UR
    CLI --> SNAP
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
