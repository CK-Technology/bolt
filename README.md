# Bolt

<div align="center">
  <img src="assets/icons/bolt.png" alt="bolt icon" width="128" height="128">

**Next-Generation Container Runtime & Orchestration**  
*Fast. Secure. Declarative.*

</div>

---

## Badges

![Zig](https://img.shields.io/badge/Zig-v0.16-yellow?logo=zig)  
![Runtime](https://img.shields.io/badge/Runtime-Containers-blue?logo=docker)  
![Orchestration](https://img.shields.io/badge/Orchestration-Surge-orange?logo=kubernetes)  
![Declarative](https://img.shields.io/badge/Config-TOML-green?logo=toml)  
![Virtualization](https://img.shields.io/badge/Virtualization-LXC--like-purple?logo=proxmox)  

---

## Overview

**Bolt** is a **Zig-native container runtime** that redefines how services are built, shipped, and deployed.  
It unifies:  

- **Docker’s runtime simplicity**  
- **Compose’s orchestration model**  
- **Nix’s declarative reproducibility**  
- **Proxmox/LXC’s lightweight virtualization**  

**Surge** is the orchestration layer that powers Bolt.  
It replaces brittle YAML with **TOML Boltfiles**, providing a modern, readable, and deterministic way to define environments.  

Together, they deliver a **fast, secure, and declarative alternative** to Docker + Compose.  

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
  Zig runtime with predictable memory and low overhead.  

- 🧩 **Declarative Configs**  
  TOML Boltfiles for clarity, reproducibility, and version control.  

- 🔐 **Security by Default**  
  Signed manifests, encrypted networking (`zcrypto`), rootless containers.  

- 🌐 **Protocol-Native Networking**  
  QUIC (`zquic`), DNS (`zdns`), and Auth (`zauth`) integrated from day one.  

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

Bring the stack online:
```bash
bolt surge up
```
--- 
## Roadmap

### Phase 1 – Core Runtime
- [ ] OCI image support (pull, build, run)  
- [ ] Bolt **Capsules** (LXC-like isolation)  
- [ ] Rootless namespaces & cgroups integration  
- [ ] Snapshot/restore functionality  

### Phase 2 – Surge Orchestration
- [ ] Boltfile (TOML) parser & schema validation  
- [ ] Multi-service orchestration (`bolt surge up`)  
- [ ] Networking & DNS via `zdns`  
- [ ] Persistent storage & volume support  
- [ ] Health checks & service dependencies  

### Phase 3 – Advanced Platform
- [ ] Secure service auth with `zauth`  
- [ ] QUIC fabric (`zquic`) for distributed services  
- [ ] Declarative builds (Nix-like reproducibility)  
- [ ] Web UI (Proxmox-style for capsules & clusters)  
- [ ] Remote orchestration across multiple nodes  

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

## Ecosystem Integration

Bolt is part of the **GhostStack protocol ecosystem**:  

- [`zcrypto`](https://github.com/ghostkellz/zcrypto) → Cryptographic primitives  
- [`zquic`](https://github.com/ghostkellz/zquic) → QUIC transport layer  
- [`zdns`](https://github.com/ghostkellz/zdns) → DNS & service discovery  
- [`zauth`](https://github.com/ghostkellz/zauth) → Authentication & SSO  
- [`zsync`](https://github.com/ghostkellz/zsync) → Async runtime  

---

## Vision

Bolt is not just a container runtime.  
It’s a **new foundation for reproducible, secure, and distributed systems**.  

By combining runtime, orchestration, declarative configs, and security into one stack, Bolt removes the need for Docker + Compose + Kubernetes + Nix + LXC as separate layers.  

Bolt is **the next step in container infrastructure**.  

---

<div align="center">

⚡ *Bolt your infrastructure together. Surge your services into life.* ⚡

</div>

