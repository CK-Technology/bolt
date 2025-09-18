# BTRFS/ZFS Snapshot System

## Overview

Bolt provides comprehensive snapshot automation with **snapper-like functionality** for both BTRFS and ZFS filesystems. The system offers "set and forget" snapshot management with intelligent retention policies, multiple trigger types, and automated cleanup.

## Key Features

### 📸 Automated Snapshots
- **Time-based triggers**: Hourly, daily, weekly, monthly schedules
- **Operation-based triggers**: Before builds, surge operations, system updates
- **Change-based triggers**: File monitoring with configurable thresholds
- **Named snapshots**: Specific configurations you want to preserve

### 🗃️ Intelligent Retention
- Configurable retention policies (hourly/daily/weekly/monthly/yearly)
- Maximum snapshot limits with automatic cleanup
- Keep-forever snapshots for critical configurations
- Dry-run cleanup to preview changes

### 🔧 Filesystem Support
- **BTRFS**: Native subvolume snapshots
- **ZFS**: ZFS dataset snapshots
- **Auto-detection**: Automatically detects filesystem type
- **Unified API**: Same interface regardless of underlying filesystem

## Installation & Setup

### Prerequisites
```bash
# For BTRFS
sudo apt-get install btrfs-progs

# For ZFS
sudo apt-get install zfsutils-linux

# Verify filesystem
df -T /
```

### Enable Snapshots
Add to your `Boltfile.toml`:
```toml
[snapshots]
enabled = true
filesystem = "auto"  # auto, btrfs, zfs
```

## Configuration

### Basic Configuration
```toml
[snapshots]
enabled = true
filesystem = "auto"                # Auto-detect BTRFS or ZFS
root_path = "/"                    # Root filesystem to snapshot
snapshot_path = "/.snapshots"      # Where to store snapshots
```

### Retention Policies
```toml
[snapshots.retention]
keep_hourly = 24        # Keep 24 hourly snapshots (1 day)
keep_daily = 7          # Keep 7 daily snapshots (1 week)
keep_weekly = 4         # Keep 4 weekly snapshots (1 month)
keep_monthly = 6        # Keep 6 monthly snapshots (6 months)
keep_yearly = 2         # Keep 2 yearly snapshots
max_total = 50          # Maximum total snapshots
cleanup_frequency = "daily"  # Run cleanup daily
```

### Time-based Triggers
```toml
[snapshots.triggers]
# Time-based snapshots
hourly = true                      # Every hour
daily = "02:00"                    # Daily at 2 AM
weekly = "sunday@03:00"            # Weekly on Sunday at 3 AM
monthly = "1@04:00"                # Monthly on 1st at 4 AM
yearly = "jan-1@05:00"             # Yearly on January 1st at 5 AM
```

### Operation-based Triggers
```toml
[snapshots.triggers]
# Operation-based snapshots
before_container_run = false       # Before every container run (usually false)
before_build = true                # Before image builds
before_surge_up = true             # Before surge operations
before_system_update = true        # Before system updates
before_gaming_setup = true         # Before gaming environment setup
before_nvidia_update = true        # Before NVIDIA driver updates
```

### Change-based Triggers
```toml
[snapshots.triggers]
# Change-based snapshots
min_change_threshold = "100MB"     # Only snapshot if >100MB changed
change_detection_interval = "30m"  # Check for changes every 30 minutes

[snapshots.triggers.on_file_changes]
enabled = true
watch_paths = [
    "/etc",                        # System configuration
    "/home",                       # User data
    "/var/lib/bolt",              # Bolt data
    "/opt/games",                 # Game installations
]
exclude_paths = [
    "/tmp",
    "/var/tmp",
    "/var/log",
    "/var/cache",
    "/home/*/.cache",
    "/home/*/.local/share/Steam/steamapps/shadercache",
]
file_patterns = [
    "*.toml", "*.yaml", "*.yml", "*.json",
    "*.conf", "*.cfg", "*.ini"
]
exclude_patterns = [
    "*.tmp", "*.log", "*.cache", ".git/*", "*.swp", "*~"
]
change_types = ["modify", "create"]  # Don't trigger on deletions
```

### Named Snapshots
```toml
# Critical system states
[[snapshots.named_snapshots]]
name = "fresh-install"
description = "Clean system after fresh Bolt installation"
trigger = "manual"
auto_create = false
keep_forever = true

[[snapshots.named_snapshots]]
name = "stable-config"
description = "Known working configuration"
trigger = "manual"
auto_create = false
keep_forever = true

# Automatic named snapshots
[[snapshots.named_snapshots]]
name = "before-gaming-setup"
description = "Before setting up gaming environment"
trigger = "before_gaming_setup"
auto_create = true
keep_forever = false

[[snapshots.named_snapshots]]
name = "before-nvidia-driver-update"
description = "Before updating NVIDIA drivers"
trigger = "before_nvidia_update"
auto_create = true
keep_forever = false
```

## CLI Commands

### Creating Snapshots
```bash
# Create manual snapshot
bolt snapshot create --name "before-update" --description "Before system update"

# Create named snapshot
bolt snapshot create --name "stable-gaming" --description "Working gaming setup"

# Create automatic snapshot
bolt snapshot create --type "manual"
```

### Listing Snapshots
```bash
# List all snapshots
bolt snapshot list

# Verbose listing with details
bolt snapshot list --verbose

# Filter by type
bolt snapshot list --filter-type "daily"

# Show only named snapshots
bolt snapshot list --filter-type "named"
```

### Managing Snapshots
```bash
# Show snapshot details
bolt snapshot show stable-config

# Rollback to snapshot
bolt snapshot rollback stable-config

# Force rollback without confirmation
bolt snapshot rollback stable-config --force

# Delete snapshot
bolt snapshot delete old-snapshot

# Force delete without confirmation
bolt snapshot delete old-snapshot --force
```

### Cleanup Operations
```bash
# Show what would be cleaned up (dry run)
bolt snapshot cleanup --dry-run

# Apply retention policy
bolt snapshot cleanup

# Force cleanup without confirmation
bolt snapshot cleanup --force
```

### Configuration Management
```bash
# Show snapshot configuration
bolt snapshot config

# Show detailed configuration
bolt snapshot config --verbose

# Enable/disable automatic snapshots
bolt snapshot auto enable
bolt snapshot auto disable
bolt snapshot auto status
```

## Usage Examples

### Gaming Setup Protection
```toml
project = "gaming-protection"

[snapshots]
enabled = true
filesystem = "auto"

[snapshots.retention]
keep_daily = 7
keep_weekly = 4
max_total = 30

[snapshots.triggers]
daily = "02:00"
before_surge_up = true
before_gaming_setup = true

[[snapshots.named_snapshots]]
name = "fresh-gaming-install"
description = "Clean gaming environment"
keep_forever = true

[[snapshots.named_snapshots]]
name = "before-steam-setup"
trigger = "before_gaming_setup"
auto_create = true
```

### Development Environment
```toml
[snapshots]
enabled = true

[snapshots.retention]
keep_hourly = 12         # Keep 12 hours of work
keep_daily = 7           # Keep 1 week
keep_weekly = 4          # Keep 1 month

[snapshots.triggers]
hourly = true
before_build = true
min_change_threshold = "50MB"

[snapshots.triggers.on_file_changes]
enabled = true
watch_paths = ["/home/dev", "/var/lib/bolt"]
file_patterns = ["*.rs", "*.toml", "*.md", "*.yml"]
```

### Server Production
```toml
[snapshots]
enabled = true

[snapshots.retention]
keep_daily = 30          # Keep 1 month
keep_weekly = 12         # Keep 3 months
keep_monthly = 12        # Keep 1 year
keep_yearly = 5          # Keep 5 years

[snapshots.triggers]
daily = "03:00"
before_surge_up = true
before_system_update = true

[[snapshots.named_snapshots]]
name = "production-stable"
description = "Known working production state"
keep_forever = true
```

## Filesystem-Specific Details

### BTRFS Implementation
```bash
# Create subvolume snapshot
btrfs subvolume snapshot -r / /.snapshots/bolt-20231201-120000

# List snapshots
btrfs subvolume list -s /.snapshots

# Delete snapshot
btrfs subvolume delete /.snapshots/bolt-20231201-120000
```

### ZFS Implementation
```bash
# Create dataset snapshot
zfs snapshot rpool/ROOT/ubuntu@bolt-20231201-120000

# List snapshots
zfs list -t snapshot

# Delete snapshot
zfs destroy rpool/ROOT/ubuntu@bolt-20231201-120000
```

## Advanced Features

### Snapshot Metadata
Each snapshot includes:
- **ID**: Unique identifier
- **Name**: Optional human-readable name
- **Description**: Optional description
- **Timestamp**: Creation time
- **Type**: manual, automatic, named
- **Size**: Snapshot size in bytes
- **Parent**: Parent snapshot relationship

### Integration with Surge
```bash
# Automatic snapshot before surge operations
bolt surge up  # Creates snapshot automatically

# Manual snapshot before surge
bolt snapshot create --name "before-deployment"
bolt surge up
```

### Rollback Safety
```bash
# Automatic backup before rollback
bolt snapshot rollback stable-config
# Creates backup snapshot: bolt-backup-20231201-120000
```

## Monitoring & Alerts

### Snapshot Status
```bash
# Check snapshot system status
bolt snapshot auto status

# Monitor snapshot creation
bolt snapshot list --follow

# Check retention policy status
bolt snapshot cleanup --dry-run
```

### Integration with System Monitoring
```bash
# Export snapshot metrics
bolt snapshot list --format json > snapshots.json

# Check snapshot health
bolt snapshot config --health
```

## Troubleshooting

### Common Issues

#### Insufficient Space
```bash
# Check available space
df -h /.snapshots

# Clean up old snapshots
bolt snapshot cleanup --force

# Adjust retention policy
# Edit retention settings in Boltfile.toml
```

#### Permission Issues
```bash
# Check snapshot directory permissions
ls -la /.snapshots

# Fix permissions
sudo chown -R bolt:bolt /.snapshots
```

#### Filesystem Not Supported
```bash
# Check filesystem type
df -T /

# Verify BTRFS/ZFS tools
which btrfs
which zfs
```

### Performance Optimization

#### Reduce Snapshot Frequency
```toml
[snapshots.triggers]
hourly = false           # Disable hourly snapshots
daily = "02:00"         # Keep daily only
min_change_threshold = "1GB"  # Higher threshold
```

#### Optimize Retention
```toml
[snapshots.retention]
keep_hourly = 0         # No hourly retention
keep_daily = 3          # Reduce daily retention
max_total = 20          # Lower total limit
```

## Security Considerations

### Snapshot Access Control
- Snapshots are read-only by default
- Access controlled through filesystem permissions
- Metadata stored securely in `/var/lib/bolt/snapshots/`

### Rollback Safety
- Automatic backup before rollback operations
- Confirmation required for destructive operations
- Dry-run mode for testing changes

## Integration Examples

### Complete Gaming Setup with Snapshots
```toml
project = "gaming-with-snapshots"

# Snapshot configuration
[snapshots]
enabled = true
filesystem = "auto"

[snapshots.retention]
keep_daily = 7
keep_weekly = 4
max_total = 30

[snapshots.triggers]
daily = "02:00"
before_surge_up = true

[[snapshots.named_snapshots]]
name = "stable-gaming"
description = "Working gaming configuration"
keep_forever = true

# Gaming service
[services.steam]
image = "ghcr.io/games-on-whales/steam:latest"
ports = ["8080:8080"]

[services.steam.gaming.gpu]
runtime = "nvbind"
isolation_level = "exclusive"
```

Usage:
```bash
# Create stable snapshot
bolt snapshot create --name "stable-gaming"

# Deploy gaming environment (automatic snapshot)
bolt surge up

# If issues occur, rollback
bolt snapshot rollback stable-gaming
```