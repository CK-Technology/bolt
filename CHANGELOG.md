# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-06-13

### Added
- **Native GPU Support via built-in nvbind** - No external GPU tooling crate
  - NVIDIA: detection, driver info, architecture detection (Maxwell through Blackwell),
    CUDA version detection, device-node passthrough, and CDI v0.6.0 spec generation
  - AMD (experimental): ROCm detection, AMDGPU driver support, architecture detection
    (GCN, RDNA, CDNA) — detection and environment setup only
  - Intel (experimental): Arc GPU detection, i915/Xe drivers, oneAPI/Level Zero
    detection — detection and environment setup only

- **GPU Profile System** - Pre-configured profiles for gaming and AI/ML
  - Gaming profiles: cyberpunk 2077, doom eternal, hogwarts legacy, fortnite, etc.
  - AI/ML profiles: ollama-small/medium/large, training-single/multi, inference-batch
  - `bolt nv profile list/show/apply` commands
  - `--gpu-profile` flag on `bolt run` command

- **Vendor-Specific CLI Commands**
  - `bolt nv info/doctor/driver/arch/cdi/profile` - NVIDIA GPU management
  - `bolt amd info/doctor/rocm/cdi` - AMD GPU management
  - `bolt arc info/doctor/oneapi/cdi` - Intel Arc GPU management

- **Documentation Overhaul**
  - Reorganized docs/ with flat structure
  - New: orchestration.md, gaming.md, ai.md, networking.md, snapshots.md, api.md
  - Added CONTRIBUTING.md

### Changed
- Merged nvbind GPU runtime directly into bolt (no external dependency)
- Consolidated GPU code from src/runtime/gpu/ into unified architecture
- Removed dead dependencies (glyph, omen, MCP modules)

### Fixed
- Integration tests updated for current API
- `surge status` now reports only the services from the last deployment instead
  of every service defined in the Boltfile (selective `surge up` is respected)

## [0.1.0] - Initial Release

### Added
- Core OCI container runtime
- Bolt Capsules (LXC-like isolation)
- Rootless namespaces & cgroups integration
- Basic Surge orchestration
- TOML-based Boltfile configuration
- Network and volume management
- QUIC networking support
- Basic GPU support