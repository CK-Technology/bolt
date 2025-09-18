# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **nvbind GPU Runtime Integration** - Sub-microsecond GPU passthrough (100x faster than Docker)
  - Added nvbind dependency with feature flag `nvbind-support`
  - Created `NvbindManager` for GPU runtime management
  - Support for gaming, AI/ML, and compute workloads with nvbind optimizations
  - GPU device selection and isolation with exclusive/shared modes
  - Gaming-specific GPU configurations (DLSS, Ray Tracing, Wine optimizations)

- **BTRFS/ZFS Snapshot Automation** - Snapper-like functionality with comprehensive automation
  - Time-based triggers (hourly, daily, weekly, monthly)
  - Operation-based triggers (before builds, surge operations, system updates)
  - Change-based triggers with file monitoring and thresholds
  - Retention policies with automatic cleanup
  - Named snapshots for specific configurations
  - Support for both BTRFS and ZFS with auto-detection

- **Enhanced Container Management**
  - `bolt restart` command with configurable timeout
  - Enhanced `bolt ps` output with modern Docker-like formatting
  - Command aliases: `rm`/`remove`, `ls`/`list`
  - Container lifecycle management improvements
  - Enhanced container information display (command, created time, runtime)

- **Gaming & GPU CLI Commands**
  - `bolt gaming gpu nvbind` - Configure nvbind GPU runtime
  - `bolt gaming gpu check` - Check GPU runtime compatibility
  - `bolt gaming gpu list` - List available GPUs
  - `bolt gaming launch` - Launch gaming workloads
  - `bolt gaming wayland` - Start Wayland gaming session

- **Volume Management**
  - `bolt volume create` - Create volumes with size specification
  - `bolt volume ls` - List volumes
  - `bolt volume rm` - Remove volumes with force option
  - `bolt volume inspect` - Inspect volume details
  - `bolt volume prune` - Clean up unused volumes

- **Snapshot Management CLI**
  - `bolt snapshot create` - Create manual snapshots with names and descriptions
  - `bolt snapshot list` - List snapshots with filtering options
  - `bolt snapshot rollback` - Rollback to specific snapshots
  - `bolt snapshot delete` - Remove snapshots
  - `bolt snapshot cleanup` - Apply retention policies
  - `bolt snapshot config` - Show snapshot configuration
  - `bolt snapshot auto` - Enable/disable automatic snapshots

- **Comprehensive Boltfile Configuration**
  - Gaming and GPU configuration with nvbind settings
  - Snapshot configuration with retention policies and triggers
  - Named snapshots for specific setups
  - File monitoring and change detection settings
  - Network configuration for QUIC networking

### Enhanced
- **Surge Orchestration** - Docker Compose-like multi-service stacks
  - Enhanced service orchestration with snapshot integration
  - Improved dependency management and health checks
  - Better error handling and rollback capabilities

- **CLI User Experience**
  - Modern output formatting across all commands
  - Better error messages and user feedback
  - Consistent command structure and aliases
  - Enhanced help and documentation

### Changed
- Restructured GPU runtime architecture to support multiple backends
- Enhanced configuration schema to support snapshot and gaming settings
- Improved container information tracking and display
- Updated project roadmap to reflect completed features

### Technical
- Added comprehensive snapshot management system in `src/snapshots/`
- Created BTRFS-specific operations in `src/snapshots/btrfs.rs`
- Enhanced GPU management in `src/runtime/gpu/`
- Extended CLI structure in `src/cli/mod.rs`
- Updated configuration structures in `src/config/mod.rs`
- Added comprehensive examples in `examples/Boltfile-with-snapshots.toml`

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