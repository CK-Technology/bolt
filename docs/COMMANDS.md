# Bolt Command Reference

## Quick Reference

| Command Category | Commands | Description |
|-----------------|----------|-------------|
| **Container** | `run`, `ps`, `stop`, `restart`, `rm` | Container lifecycle management |
| **Image** | `build`, `pull`, `push` | Image operations |
| **Gaming** | `gaming gpu`, `gaming wine`, `gaming launch` | Gaming and GPU management |
| **Snapshot** | `snapshot create`, `snapshot list`, `snapshot rollback` | Snapshot automation |
| **Surge** | `surge up`, `surge down`, `surge status` | Multi-service orchestration |
| **Network** | `network create`, `network ls`, `network rm` | Network management |
| **Volume** | `volume create`, `volume ls`, `volume rm` | Volume management |

## Container Commands

### `bolt run [OPTIONS] IMAGE [COMMAND] [ARG...]`
Run a new container from an image.

**Options:**
- `-n, --name NAME` - Container name
- `-p, --ports HOST:CONTAINER` - Port mappings
- `-e, --env KEY=VALUE` - Environment variables
- `-v, --volumes HOST:CONTAINER` - Volume mounts
- `-d, --detach` - Run in background
- `--runtime RUNTIME` - GPU runtime (nvbind, docker, nvidia, amd)
- `--gpu DEVICES` - GPU devices (all, 0, 1,2)

**Examples:**
```bash
bolt run ubuntu:latest
bolt run --name web --ports 80:80 nginx:latest
bolt run --runtime nvbind --gpu all nvidia/cuda:latest
```

### `bolt ps [OPTIONS]`
List containers.

**Options:**
- `-a, --all` - Show all containers (default: running only)
- `--format FORMAT` - Output format (table, json)
- `-q, --quiet` - Only show container IDs

**Examples:**
```bash
bolt ps
bolt ps --all
bolt ps --format json
```

### `bolt stop CONTAINER [CONTAINER...]`
Stop running containers.

**Examples:**
```bash
bolt stop web
bolt stop web database redis
```

### `bolt restart [OPTIONS] CONTAINER [CONTAINER...]`
Restart containers.

**Options:**
- `-t, --timeout SECONDS` - Timeout before force kill (default: 10)

**Examples:**
```bash
bolt restart web
bolt restart web --timeout 30
```

### `bolt rm [OPTIONS] CONTAINER [CONTAINER...]`
Remove containers.

**Options:**
- `-f, --force` - Force remove running containers

**Examples:**
```bash
bolt rm web
bolt rm --force running-container
```

## Image Commands

### `bolt build [OPTIONS] PATH`
Build an image from a Dockerfile.

**Options:**
- `-t, --tag NAME:TAG` - Image name and tag
- `-f, --file DOCKERFILE` - Dockerfile path (default: Dockerfile)

**Examples:**
```bash
bolt build .
bolt build --tag myapp:v1.0 .
bolt build --file Dockerfile.prod --tag myapp:prod
```

### `bolt pull IMAGE[:TAG]`
Pull an image from registry.

**Examples:**
```bash
bolt pull ubuntu:latest
bolt pull nvidia/cuda:12.0-runtime-ubuntu22.04
```

### `bolt push IMAGE[:TAG]`
Push an image to registry.

**Examples:**
```bash
bolt push myapp:v1.0
bolt push registry.example.com/myapp:latest
```

## Gaming Commands

### `bolt gaming gpu SUBCOMMAND`
GPU management for gaming workloads.

#### `bolt gaming gpu list`
List available GPU devices.

#### `bolt gaming gpu nvidia [OPTIONS]`
Configure NVIDIA GPU.

**Options:**
- `--device INDEX` - GPU device index
- `--dlss` - Enable DLSS
- `--raytracing` - Enable ray tracing

#### `bolt gaming gpu amd [OPTIONS]`
Configure AMD GPU.

**Options:**
- `--device INDEX` - GPU device index

#### `bolt gaming gpu nvbind [OPTIONS]`
Configure nvbind GPU runtime.

**Options:**
- `--devices DEVICES` - GPU devices (all, 0, 1,2)
- `--driver DRIVER` - Driver type (auto, nvidia-open, proprietary, nouveau)
- `--performance MODE` - Performance mode (ultra, high, balanced, efficient)
- `--wsl2` - Enable WSL2 optimizations

**Examples:**
```bash
bolt gaming gpu list
bolt gaming gpu nvbind --devices all --performance ultra --wsl2
bolt gaming gpu nvidia --device 0 --dlss --raytracing
```

#### `bolt gaming gpu check`
Check GPU runtime compatibility.

#### `bolt gaming gpu benchmark`
Run GPU performance benchmark.

### `bolt gaming wine [OPTIONS]`
Configure Wine/Proton for Windows gaming.

**Options:**
- `--proton VERSION` - Proton version
- `--winver VERSION` - Windows version to emulate

**Examples:**
```bash
bolt gaming wine --proton 8.0 --winver win10
```

### `bolt gaming audio [OPTIONS]`
Configure audio for gaming.

**Options:**
- `--system SYSTEM` - Audio system (pipewire, pulseaudio)

**Examples:**
```bash
bolt gaming audio --system pipewire
```

### `bolt gaming launch GAME [ARG...]`
Launch a game or gaming application.

**Examples:**
```bash
bolt gaming launch steam
bolt gaming launch /path/to/game.exe --windowed
```

### `bolt gaming wayland`
Start Wayland gaming session.

### `bolt gaming realtime [OPTIONS]`
Configure real-time gaming optimizations.

**Options:**
- `--enable` - Enable optimizations

**Examples:**
```bash
bolt gaming realtime --enable
```

### `bolt gaming optimize --pid PID`
Optimize a running game process.

**Examples:**
```bash
bolt gaming optimize --pid 1234
```

### `bolt gaming performance`
Show gaming performance report.

## Snapshot Commands

### `bolt snapshot create [OPTIONS]`
Create a snapshot.

**Options:**
- `-n, --name NAME` - Snapshot name
- `-d, --description DESC` - Snapshot description
- `--type TYPE` - Snapshot type (manual, auto)

**Examples:**
```bash
bolt snapshot create
bolt snapshot create --name "stable-config" --description "Working configuration"
```

### `bolt snapshot list [OPTIONS]`
List snapshots.

**Options:**
- `-v, --verbose` - Show detailed information
- `--filter-type TYPE` - Filter by type

**Examples:**
```bash
bolt snapshot list
bolt snapshot list --verbose
bolt snapshot list --filter-type daily
```

### `bolt snapshot show SNAPSHOT`
Show snapshot details.

**Examples:**
```bash
bolt snapshot show stable-config
bolt snapshot show bolt-20231201-120000
```

### `bolt snapshot rollback [OPTIONS] SNAPSHOT`
Rollback to a snapshot.

**Options:**
- `-f, --force` - Force rollback without confirmation

**Examples:**
```bash
bolt snapshot rollback stable-config
bolt snapshot rollback --force bolt-20231201-120000
```

### `bolt snapshot delete [OPTIONS] SNAPSHOT`
Delete a snapshot.

**Options:**
- `-f, --force` - Force deletion without confirmation

**Examples:**
```bash
bolt snapshot delete old-snapshot
bolt snapshot delete --force bolt-20231201-120000
```

### `bolt snapshot cleanup [OPTIONS]`
Apply retention policy and cleanup old snapshots.

**Options:**
- `--dry-run` - Show what would be deleted
- `-f, --force` - Force cleanup without confirmation

**Examples:**
```bash
bolt snapshot cleanup --dry-run
bolt snapshot cleanup --force
```

### `bolt snapshot config [OPTIONS]`
Show snapshot configuration.

**Options:**
- `-v, --verbose` - Show detailed configuration

**Examples:**
```bash
bolt snapshot config
bolt snapshot config --verbose
```

### `bolt snapshot auto ACTION`
Control automatic snapshots.

**Actions:**
- `enable` - Enable automatic snapshots
- `disable` - Disable automatic snapshots
- `status` - Show automatic snapshot status

**Examples:**
```bash
bolt snapshot auto enable
bolt snapshot auto status
```

## Surge Commands

### `bolt surge up [OPTIONS] [SERVICE...]`
Start services from Boltfile.

**Options:**
- `-d, --detach` - Run in background
- `--force-recreate` - Recreate containers

**Examples:**
```bash
bolt surge up
bolt surge up web database
bolt surge up --detach --force-recreate
```

### `bolt surge down [OPTIONS] [SERVICE...]`
Stop and remove services.

**Options:**
- `-v, --volumes` - Remove volumes

**Examples:**
```bash
bolt surge down
bolt surge down web database
bolt surge down --volumes
```

### `bolt surge status`
Show service status.

### `bolt surge logs [OPTIONS] [SERVICE]`
Show service logs.

**Options:**
- `-f, --follow` - Follow logs in real-time
- `-t, --tail LINES` - Number of lines to show

**Examples:**
```bash
bolt surge logs
bolt surge logs web
bolt surge logs --follow --tail 100
```

### `bolt surge scale SERVICE=COUNT [SERVICE=COUNT...]`
Scale services.

**Examples:**
```bash
bolt surge scale web=3
bolt surge scale web=3 worker=5
```

## Network Commands

### `bolt network create [OPTIONS] NAME`
Create a network.

**Options:**
- `--driver DRIVER` - Network driver (bolt, gquic, bridge, host, none)
- `--subnet CIDR` - Subnet CIDR

**Examples:**
```bash
bolt network create gaming-net
bolt network create --driver bolt --subnet 172.20.0.0/16 gaming-net
```

### `bolt network list`
List networks.

**Aliases:** `bolt network ls`

### `bolt network remove NAME`
Remove a network.

**Aliases:** `bolt network rm`

**Examples:**
```bash
bolt network remove gaming-net
bolt network rm old-network
```

### `bolt network inspect NAME`
Inspect network details.

**Examples:**
```bash
bolt network inspect gaming-net
```

## Volume Commands

### `bolt volume create [OPTIONS] NAME`
Create a volume.

**Options:**
- `--driver DRIVER` - Volume driver (default: local)
- `--size SIZE` - Volume size
- `-o, --opt KEY=VALUE` - Driver options

**Examples:**
```bash
bolt volume create game-data
bolt volume create --size 100GB --driver local game-data
bolt volume create --opt type=nfs --opt device=nas:/share remote-data
```

### `bolt volume list`
List volumes.

**Aliases:** `bolt volume ls`

### `bolt volume remove [OPTIONS] NAME`
Remove a volume.

**Aliases:** `bolt volume rm`

**Options:**
- `-f, --force` - Force removal

**Examples:**
```bash
bolt volume remove game-data
bolt volume rm --force old-volume
```

### `bolt volume inspect NAME`
Inspect volume details.

**Examples:**
```bash
bolt volume inspect game-data
```

### `bolt volume prune [OPTIONS]`
Remove unused volumes.

**Options:**
- `-f, --force` - Don't prompt for confirmation

**Examples:**
```bash
bolt volume prune
bolt volume prune --force
```

## Global Options

### Common Flags
Available for all commands:

- `-v, --verbose` - Enable verbose logging
- `-c, --config PATH` - Configuration file path (default: Boltfile.toml)
- `-h, --help` - Show help

**Examples:**
```bash
bolt --verbose ps
bolt --config /path/to/Boltfile.toml surge up
bolt --help
```

## Environment Variables

### Configuration
- `BOLT_CONFIG` - Default configuration file path
- `BOLT_LOG_LEVEL` - Logging level (trace, debug, info, warn, error)
- `BOLT_DATA_DIR` - Data directory (default: /var/lib/bolt)

### GPU Runtime
- `BOLT_NVBIND_PATH` - Path to nvbind binary
- `BOLT_GPU_RUNTIME` - Default GPU runtime
- `NVIDIA_VISIBLE_DEVICES` - NVIDIA GPU devices

### Snapshots
- `BOLT_SNAPSHOT_DIR` - Snapshot directory
- `BOLT_SNAPSHOT_RETENTION` - Default retention policy

**Examples:**
```bash
export BOLT_LOG_LEVEL=debug
export BOLT_GPU_RUNTIME=nvbind
export NVIDIA_VISIBLE_DEVICES=0,1
bolt run --gpu all nvidia/cuda:latest
```

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | General error |
| 2 | Misuse of shell builtins |
| 125 | Container run error |
| 126 | Container command not executable |
| 127 | Container command not found |
| 128 | Invalid exit code |

## Configuration File

### Boltfile.toml Location
Bolt looks for configuration files in this order:
1. `--config` flag path
2. `BOLT_CONFIG` environment variable
3. `./Boltfile.toml` (current directory)
4. `~/.config/bolt/Boltfile.toml` (user config)
5. `/etc/bolt/Boltfile.toml` (system config)

### Basic Boltfile Structure
```toml
project = "my-project"

[services.web]
image = "nginx:latest"
ports = ["80:80"]

[services.database]
image = "postgres:latest"
environment = { POSTGRES_DB = "myapp" }

[snapshots]
enabled = true
filesystem = "auto"

[snapshots.retention]
keep_daily = 7
keep_weekly = 4
```

## Aliases & Shortcuts

### Command Aliases
```bash
# Container management
bolt rm = bolt remove
bolt ps = bolt ls (for containers)

# Volume management
bolt volume ls = bolt volume list
bolt volume rm = bolt volume remove

# Network management
bolt network ls = bolt network list
bolt network rm = bolt network remove

# Snapshot management
bolt snapshot ls = bolt snapshot list
bolt snapshot rm = bolt snapshot delete
```

### Useful Shortcuts
```bash
# Stop all containers
bolt stop $(bolt ps -q)

# Remove all stopped containers
bolt rm $(bolt ps -aq --filter status=exited)

# Show container IDs only
bolt ps -q

# Follow all service logs
bolt surge logs --follow

# Create snapshot before deployment
bolt snapshot create --name "before-deploy-$(date +%Y%m%d-%H%M%S)"
```

## Tab Completion

### Install Completion
```bash
# Bash
bolt completion bash > /etc/bash_completion.d/bolt

# Zsh
bolt completion zsh > "${fpath[1]}/_bolt"

# Fish
bolt completion fish > ~/.config/fish/completions/bolt.fish
```

### Completion Features
- Command completion
- Option completion
- Container name completion
- Image name completion
- Network name completion
- Volume name completion
- Snapshot name completion

## Integration Examples

### Complete Workflow
```bash
# 1. Create project snapshot
bolt snapshot create --name "project-start"

# 2. Setup infrastructure
bolt network create --driver bolt app-net
bolt volume create --size 50GB app-data

# 3. Start services
bolt surge up --detach

# 4. Monitor services
bolt ps
bolt surge status
bolt surge logs --follow

# 5. Gaming setup with GPU
bolt gaming gpu nvbind --devices all --performance ultra
bolt run --runtime nvbind --gpu all --name gaming steam:latest

# 6. Create stable snapshot
bolt snapshot create --name "stable-deployment"

# 7. Cleanup if needed
bolt surge down --volumes
bolt snapshot rollback project-start
```

### CI/CD Integration
```bash
#!/bin/bash
# Deploy script with snapshots

# Create pre-deployment snapshot
bolt snapshot create --name "pre-deploy-$(date +%Y%m%d-%H%M%S)"

# Deploy application
bolt surge up --force-recreate

# Health check
if bolt surge status | grep -q "healthy"; then
    echo "Deployment successful"
    bolt snapshot create --name "deploy-success-$(date +%Y%m%d-%H%M%S)"
else
    echo "Deployment failed, rolling back"
    bolt snapshot rollback pre-deploy-*
    exit 1
fi

# Cleanup old snapshots
bolt snapshot cleanup --force
```