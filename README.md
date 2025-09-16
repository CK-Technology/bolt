# Bolt

<div align="center">
  <img src="assets/icons/sizes/bolt-icon-128.png" alt="bolt icon" width="128" height="128">

**Next-Generation Container Runtime & Orchestration**  
*Fast. Secure. Declarative.*

</div>

---

## Badges

![Rust](https://img.shields.io/badge/Rust%201.85+-red?logo=rust)  
![Runtime](https://img.shields.io/badge/Runtime-Containers-blue?logo=docker)  
![Orchestration](https://img.shields.io/badge/Orchestration-Surge-orange?logo=kubernetes)  
![Declarative](https://img.shields.io/badge/Config-TOML-green?logo=toml)  
![Virtualization](https://img.shields.io/badge/Virtualization-LXC--like-purple?logo=proxmox)  

---

## Overview

**Bolt** is a **Rust-native container runtime** that redefines how services are built, shipped, and deployed.  
It unifies:  

- **Docker’s runtime simplicity**  
- **Compose’s orchestration model**  
- **Nix’s declarative reproducibility**  
- **Proxmox/LXC’s lightweight virtualization**  

**Surge** is the orchestration layer that powers Bolt.  
It replaces brittle YAML with **TOML Boltfiles**, providing a modern, readable, and deterministic way to define environments.  

Together, they deliver a **fast, secure, and declarative alternative** to Docker + Compose.  

---

## Install

```bash
curl -fsSL https://bolt.cktech.org | bash
```

---

## Why Bolt?

Containers today are fragmented:  
- Docker is runtime-focused but aging.  
- Kubernetes is powerful but bloated.  
- Nix provides reproducibility but not simple orchestration.  
- Proxmox offers lightweight virtualization, but not modern runtime portability.  

**Bolt unifies these ideas into one coherent stack**.  
It’s designed to be **developer-friendly**, **secure by default**, and **scalable across bare metal, cloud, and homelabs**.  

---

## Design Goals

- ⚡ **Performance First**
  Rust runtime with zero-cost abstractions, memory safety, and low overhead.  

- 🧩 **Declarative Configs**  
  TOML Boltfiles for clarity, reproducibility, and version control.  

- 🔐 **Security by Default**
  Signed manifests, encrypted networking, rootless containers, and Rust's memory safety.

- 🌐 **Protocol-Native Networking**
  QUIC networking, DNS resolution, and authentication integrated from day one.  

- 📦 **Capsules**  
  Bolt’s native, LXC-like container: lightweight, snapshot-ready, and resource-efficient.  

- 🛠 **Unified Orchestration**  
  Surge integrates service orchestration directly into Bolt — no external tooling required.  

---

## Configuration: Boltfile (TOML)

Bolt uses **TOML Boltfiles** for defining projects.  
This eliminates YAML indentation issues and provides strict typing.  

Example:

```toml
project = "demo"

[services.web]
image = "bolt://nginx:latest"
ports = ["80:80"]
volumes = ["./site:/usr/share/nginx/html"]

[services.api]
build = "./api"
env.DATABASE_URL = "bolt://db"

[services.db]
capsule = "postgres"

[services.db.storage]
size = "5Gi"

[services.db.auth]
user = "demo"
password = "secret"
```

Bring the stack online:
```bash
bolt surge up
```
--- 
## Roadmap

### Phase 1 – Core Runtime ✅ **COMPLETE**
- [x] OCI image support (pull, build, run)
- [x] Bolt **Capsules** (LXC-like isolation)
- [x] Rootless namespaces & cgroups integration
- [x] Snapshot/restore functionality  

### Phase 2 – Surge Orchestration ✅ **COMPLETE**
- [x] Boltfile (TOML) parser & schema validation
- [x] Multi-service orchestration (`bolt surge up`)
- [x] Networking & DNS resolution
- [x] Persistent storage & volume support
- [x] Health checks & service dependencies

### Phase 3 – Advanced Platform ✅ **COMPLETE**
- [x] Secure service authentication
- [x] QUIC networking for distributed services
- [x] Declarative builds (Nix-like reproducibility)
- [x] Web UI (Proxmox-style for capsules & clusters)
- [x] Remote orchestration across multiple nodes  

---

## Comparisons

| Feature              | Docker + Compose | Kubernetes | NixOS | Proxmox/LXC | **Bolt + Surge** |
|----------------------|------------------|------------|-------|-------------|------------------|
| Runtime              | ✅               | ✅         | ❌    | ✅          | ✅ (OCI + Capsules) |
| Orchestration        | ✅ (basic)       | ✅ (complex)| ❌    | ❌          | ✅ (Surge built-in) |
| Config Format        | YAML             | YAML/JSON  | Nix   | Conf files  | TOML (clean) |
| Reproducibility      | ❌               | Partial    | ✅    | ❌          | ✅ |
| Virtualization       | ❌               | ❌         | ❌    | ✅          | ✅ |
| Secure by Default    | ❌               | Limited    | ✅    | ❌          | ✅ |
| Learning Curve       | Low              | High       | Medium| Medium      | Low |

---

## Requirements

- **Rust 1.85+** (Required for latest async/await optimizations and performance improvements)
- **Linux Kernel 5.4+** (For container namespaces and cgroups v2 support)
- **Tokio 1.0+** (Async runtime integration)

---

## 🚀 Rust API Integration

Bolt provides a **production-ready Rust API** for programmatic container management:

### **Quick Start**

Add to your `Cargo.toml`:
```toml
[dependencies]
bolt = { git = "https://github.com/CK-Technology/bolt" }
tokio = { version = "1.0", features = ["full"] }
```

### **Basic Usage**
```rust
use bolt::api::*;

#[tokio::main]
async fn main() -> bolt::Result<()> {
    let runtime = BoltRuntime::new()?;

    // Run containers
    runtime.run_container("nginx:latest", Some("web"), &["8080:80"], &[], &[], false).await?;

    // Gaming containers with GPU
    let gaming_config = GamingConfig {
        gpu: Some(GpuConfig { nvidia: Some(NvidiaConfig { dlss: Some(true), .. }), .. }),
        wine: Some(WineConfig { proton: Some("8.0"), .. }),
        ..
    };

    runtime.add_gaming_service("steam", "bolt://steam:latest", gaming_config);
    runtime.surge_up(&[], false, false).await?;

    Ok(())
}
```

### **Integration Examples**

**🎮 Ghostforge (Gaming Container Management):**
```rust
// Create gaming-optimized containers with GPU passthrough
let runtime = BoltRuntime::new()?;
runtime.setup_gaming(Some("8.0"), Some("win10")).await?;
runtime.launch_game("steam://run/123456", &[]).await?;
```

**🖥️ nvcontrol (GPU Management):**
```rust
// Allocate GPU resources to Bolt containers
runtime.create_network("gpu-net", "bolt", Some("10.2.0.0/16")).await?;
runtime.run_container("bolt://gpu-workload", None, &[], &[], &[], false).await?;
```

**📦 Programmatic Boltfiles:**
```rust
let boltfile = BoltFileBuilder::new("my-project")
    .add_gaming_service("game", "bolt://steam:latest", gaming_config)
    .build();

config.save_boltfile(&boltfile)?;
```

### **Feature Flags**
```toml
bolt = { git = "https://github.com/CK-Technology/bolt", features = ["gaming", "quic-networking"] }
```

- `gaming` - Gaming optimizations, GPU support, Wine/Proton
- `quic-networking` - Ultra-low latency QUIC networking
- `oci-runtime` - Full OCI container support
- `nvidia-support` - NVIDIA GPU passthrough
- `amd-support` - AMD GPU support

---

## Ecosystem Integration

Bolt integrates with modern Rust ecosystem libraries for enhanced functionality:

- **Cryptography** → Secure networking and container signing
- **QUIC Networking** → Ultra-low latency transport
- **DNS Resolution** → Service discovery and networking
- **Authentication** → Secure service-to-service communication
- **Async Runtime** → Powered by Tokio for high-performance I/O

---

## Vision

Bolt is not just a container runtime.  
It’s a **new foundation for reproducible, secure, and distributed systems**.  

By combining runtime, orchestration, declarative configs, and security into one cohesive Rust-powered stack, Bolt removes the need for Docker + Compose + Kubernetes + Nix + LXC as separate layers.  

Bolt is **the next step in container infrastructure**.  

---

<div align="center">

⚡ *Bolt your infrastructure together. Surge your services into life.* ⚡

</div>

