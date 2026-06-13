# Snapshots

Bolt provides automated BTRFS/ZFS snapshot management.

## Requirements

- BTRFS or ZFS filesystem
- Root or appropriate permissions

## Quick Start

```bash
# Create snapshot
bolt snapshot create --name "before-update"

# List snapshots
bolt snapshot list

# Rollback
bolt snapshot rollback before-update

# Cleanup old snapshots
bolt snapshot cleanup --dry-run
bolt snapshot cleanup
```

## Commands

### Create
```bash
bolt snapshot create --name "my-snapshot" --description "Before major change"
```

### List
```bash
bolt snapshot list
bolt snapshot list --verbose
```

### Rollback
```bash
bolt snapshot rollback my-snapshot
```

### Delete
```bash
bolt snapshot delete my-snapshot
```

### Cleanup
```bash
# Preview what would be deleted
bolt snapshot cleanup --dry-run

# Execute cleanup
bolt snapshot cleanup
```

## Boltfile Configuration

```toml
[snapshots]
enabled = true
filesystem = "auto"  # auto, btrfs, or zfs

[snapshots.retention]
keep_daily = 7
keep_weekly = 4
keep_monthly = 6
max_total = 50

[snapshots.triggers]
daily = "02:00"           # Daily at 2 AM
before_build = true       # Before image builds
before_surge_up = true    # Before surge operations
min_change_threshold = "100MB"

[[snapshots.named_snapshots]]
name = "stable-config"
description = "Known working configuration"
keep_forever = true

[[snapshots.named_snapshots]]
name = "pre-update"
description = "Before system updates"
```

## Retention Policies

### Time-Based
```toml
[snapshots.retention]
keep_daily = 7      # Keep 7 daily snapshots
keep_weekly = 4     # Keep 4 weekly snapshots
keep_monthly = 6    # Keep 6 monthly snapshots
```

### Count-Based
```toml
[snapshots.retention]
max_total = 50      # Maximum snapshots to keep
```

### Named Snapshots
```toml
[[snapshots.named_snapshots]]
name = "stable"
keep_forever = true  # Never auto-delete
```

## Triggers

### Scheduled
```toml
[snapshots.triggers]
daily = "02:00"     # Every day at 2 AM
weekly = "sun:03:00" # Sundays at 3 AM
```

### Operation-Based
```toml
[snapshots.triggers]
before_build = true      # Before bolt build
before_surge_up = true   # Before bolt surge up
```

### Change-Based
```toml
[snapshots.triggers]
min_change_threshold = "100MB"  # Only if >100MB changed
```

## Filesystem Support

### BTRFS
```bash
# Check if BTRFS
df -T /

# Manual snapshot
btrfs subvolume snapshot /data /data/.snapshots/manual
```

### ZFS
```bash
# Check if ZFS
zfs list

# Manual snapshot
zfs snapshot tank/data@manual
```

## Troubleshooting

### Snapshot Failed
```bash
# Check filesystem type
df -T /path/to/data

# Check permissions
sudo bolt snapshot create --name test

# Check disk space
df -h
```

### Rollback Failed
```bash
# List available snapshots
bolt snapshot list --verbose

# Check snapshot exists
bolt snapshot list | grep my-snapshot
```

### Cleanup Not Working
```bash
# Preview first
bolt snapshot cleanup --dry-run

# Check retention settings in Boltfile.toml
```
