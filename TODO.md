# TODO

Project roadmap for **Bolt** (runtime) and **Surge** (orchestration).

---

## Phase 1 – Core Runtime (Bolt)
- [ ] Implement OCI image support
  - [ ] Pull images from registries
  - [ ] Build images from Boltfiles
  - [ ] Cache layers locally
- [ ] Implement Bolt **Capsules**
  - [ ] Namespaces (pid, net, mount, ipc)
  - [ ] Cgroups v2 resource limits
  - [ ] Filesystem overlay (aufs/btrfs/zfs)
  - [ ] Rootless execution mode
- [ ] Networking primitives
  - [ ] Bridge networking
  - [ ] Host networking
  - [ ] QUIC overlay foundation (`zquic`) - zig fetch --save https://github.com/ghostkellz/zquic
- [ ] Storage primitives
  - [ ] Volumes
  - [ ] Snapshots (ZFS/Btrfs)
  - [ ] Capsule persistence

---

## Phase 2 – Surge Orchestration
- [ ] Boltfile (TOML) parser
  - [ ] Schema validation
  - [ ] Strict typing (ports, env, volumes)
- [ ] Service orchestration
  - [ ] `bolt surge up` / `down`
  - [ ] Service dependencies & ordering
  - [ ] Health checks
  - [ ] Logs & monitoring
- [ ] Networking & discovery
  - [ ] Internal DNS (`zdns`)
  - [ ] Service-to-service encryption (`zcrypto`) - zig fetch --save https://github.com/ghostkellz/zcrypto
- [ ] Storage integration
  - [ ] Declarative volumes
  - [ ] Ephemeral vs persistent modes

---

## Phase 3 – Advanced Platform Features
- [ ] Declarative builds (Nix-inspired)
  - [ ] Deterministic builds
  - [ ] Content-addressed store
  - [ ] Reproducible environments
- [ ] Security
  - [ ] Signed manifests
  - [ ] Capsule attestation
  - [ ] User/role-based auth via `zauth`
- [ ] Distributed orchestration
  - [ ] Multi-node Surge clusters
  - [ ] QUIC fabric overlay
  - [ ] Scheduling & placement logic
- [ ] Capsule management
  - [ ] Live migration
  - [ ] Rollbacks
  - [ ] Resource quotas

---

## Phase 4 – Tooling & Ecosystem
- [ ] CLI UX polish
  - [ ] `bolt run` single capsules
  - [ ] `bolt surge` multi-service stacks
  - [ ] Logs, exec, attach, top
- [ ] Developer tooling
  - [ ] Dev mode (`bolt dev`)
  - [ ] Hot reload for services
  - [ ] Integration with IDEs / editors
- [ ] Web UI
  - [ ] Capsule + service dashboard
  - [ ] Resource graphs
  - [ ] Cluster management
- [ ] API
  - [ ] gRPC + REST control plane
  - [ ] SDKs in Zig, Rust, Go
  - [ ] WebSocket/QUIC event streams

---

## Phase 5 – Production Hardening
- [ ] CI/CD integration
  - [ ] GitHub Actions Bolt runner
  - [ ] GitLab CI Bolt runner
- [ ] Monitoring & observability
  - [ ] Metrics endpoint (Prometheus)
  - [ ] Tracing hooks
  - [ ] Centralized logs
- [ ] Packaging & distribution
  - [ ] Prebuilt binaries (Linux distros)
  - [ ] Containerized installer
  - [ ] Proxmox integration (Capsules as VMs/CTs)

---

## Stretch Goals
- [ ] Linux + Windows + macOS runtime support
- [ ] WASM capsule type
- [ ] GPU-accelerated capsules
- [ ] Mobile/edge device mode (IoT deployments)

