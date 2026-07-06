# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Version is held at 0.1.0 (no release tags yet). Entries below are dated.

## 2026-07-06

### Added
- Project control-plane commands:
  - `bolt plan [--json]`
  - `bolt apply [-d|--detach] [--force-recreate] [--locked] [SERVICE...]`
  - `bolt destroy [--force] [--volumes] [SERVICE...]`
  - `bolt lock [--check]`
  - `bolt drift [--json]`
  - `bolt doctor [--json]`
  - `bolt validate [--json]`
  - `bolt self-test [--json]`
  - `bolt dns resolve|hosts|serve`
  - `bolt import compose|container|image`
  - `bolt inspect service|container|image|volume|network`
  - `bolt completions <bash|zsh|fish>` and `bolt manpage`
- Native image inspection and GC pins via `bolt image inspect|pin|unpin`.
- `Boltfile.lock` with Boltfile hashes, service config hashes, cached image
  digests, build context hashes, and volume/network config hashes.
- Snapshot-backed generations via `bolt generations list [--verbose]`.
- Service discovery registry foundation with `<service>.<project>.bolt` names.
- Bridge networking preflight via `bolt network preflight`.
- Volume metadata attach/detach APIs and local volume `mode`/`uid`/`gid`
  creation options.
- QUIC service registry state for endpoint, protocol, health, reconnect, and
  backpressure tracking.
- New operations and internals docs, including volumes, generations, networking,
  garbage collection, and architecture diagrams.

### Changed
- `bolt apply` now prepares declared volumes and networks before invoking Surge,
  writes service discovery metadata, honors `depends_on` ordering, selectively
  applies changed services, and updates `Boltfile.lock`.
- `bolt destroy` now plans destructive actions and uses reverse dependency
  ordering.
- Image garbage collection protects digests and container roots referenced by
  snapshot generations and image pins.
- Native volume resolution now uses the correct Bolt storage root, and volume
  usage metadata updates on container create/remove instead of relying only on
  reconciliation.
- Bridge network creation now performs real host preflight and gated `ip`
  command execution instead of silently logging placeholder success.
- Active scratch/cache code paths were moved to repo-local `.scratch` patterns
  where scratch space is unavoidable.

### Fixed
- Volume usage reconciliation no longer clears `used_by` metadata when no
  container state directory exists.
- Snapshot metadata now records Boltfile provenance and remains backward
  compatible with older `snapshot.json` files.
- Several native runtime, networking, GPU, Wayland, registry, and storage paths
  were hardened with focused tests and clearer failure behavior.

## 2026-06-13

### Added
- Native GPU support via built-in nvbind (no external GPU crate):
  - NVIDIA: detection, driver info, architecture detection (Maxwell through
    Blackwell), CUDA version detection, device-node passthrough, CDI v0.6.0 specs
  - AMD (experimental): ROCm/AMDGPU detection and environment setup only
  - Intel (experimental): Arc/i915/Xe, oneAPI/Level Zero detection only
- GPU profile system for gaming and AI/ML, with `--gpu-profile` on `bolt run`
  and `bolt nv profile list/show/apply`
- Vendor CLI commands: `bolt nv`, `bolt amd`, `bolt arc` (info/doctor/cdi/…)

### Changed
- Dependency and security overhaul: all advisories resolved, crates updated,
  MessagePack serialization, tonic/prost 0.13, axum 0.8, nix 0.30, rcgen 0.13
- Merged nvbind GPU runtime directly into bolt; consolidated `src/runtime/gpu/`
- Removed dead dependencies (glyph, omen, MCP modules)
- Reorganized packaging: `packaging/arch/` (PKGBUILD, .SRCINFO) and
  `packaging/rpm/bolt.spec`; package sources track the `main` branch
- Moved Node deploy scripts to `deploy/`; documentation reorganized under `docs/`
- GPU/AI/gaming env vars now flow through a per-container environment map that
  the native runtime merges into the OCI process env, replacing process-global
  `std::env::set_var` mutation
- Object-store credentials are passed via the AWS SDK credential provider
  instead of mutating `AWS_*` in the process environment

### Fixed
- Integration tests updated for current API
- `surge status` reports only services from the last deployment (selective
  `surge up` is respected)
- Container GPU env vars no longer leak host-wide or race across concurrent
  container starts
- nvbind GPU monitoring recovers from poisoned mutexes instead of panicking the
  runtime
- Per-container environment is cleared on container removal

### Removed
- Stale build artifacts and corrupted local `.bolt/` cache; release tags
  (v0.1.0/v0.1.1/v0.1.2) pending a real tagging strategy
