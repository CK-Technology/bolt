# Enhanced CLI Commands

## Overview

Bolt provides a **Docker-compatible command-line interface** with modern enhancements, better output formatting, and additional functionality for gaming, snapshots, and orchestration.

## Container Management

### `bolt run` - Run Containers
Run containers with enhanced GPU and gaming support.

```bash
# Basic container execution
bolt run ubuntu:latest
bolt run --name web nginx:latest

# Port mapping
bolt run --ports 8080:80 --name webserver nginx:latest

# Environment variables
bolt run --env NODE_ENV=production --env PORT=3000 node:latest

# Volume mounts
bolt run --volumes /host/data:/container/data ubuntu:latest

# Detached mode
bolt run --detach --name background-service redis:latest

# GPU runtime with nvbind
bolt run --runtime nvbind --gpu all nvidia/cuda:latest
bolt run --runtime nvbind --gpu 0,1 tensorflow/tensorflow:latest-gpu

# Complete example
bolt run \
  --name gaming-container \
  --runtime nvbind \
  --gpu all \
  --ports 8080:8080 \
  --volumes /home/games:/games \
  --env DISPLAY=:0 \
  --detach \
  ghcr.io/games-on-whales/steam:latest
```

### `bolt ps` - List Containers
Enhanced container listing with modern output formatting.

```bash
# List running containers
bolt ps

# List all containers (including stopped)
bolt ps --all

# Example output:
# CONTAINER ID   NAME      IMAGE           COMMAND         CREATED        STATUS       PORTS                    RUNTIME
# a1b2c3d4e5f6   web       nginx:latest    "nginx -g..."   2 hours ago    Up 2 hours   0.0.0.0:8080->80/tcp    docker
# f6e5d4c3b2a1   gaming    steam:latest    "/entrypoint"   1 hour ago     Up 1 hour    0.0.0.0:8080->8080/tcp  nvbind
```

### `bolt restart` - Restart Containers
Restart containers with configurable timeout.

```bash
# Restart single container
bolt restart web

# Restart multiple containers
bolt restart web gaming database

# Custom timeout (default: 10 seconds)
bolt restart web --timeout 30

# Restart all containers
bolt restart $(bolt ps -q)
```

### `bolt stop` - Stop Containers
Stop running containers gracefully.

```bash
# Stop single container
bolt stop web

# Stop multiple containers
bolt stop web gaming database

# Stop all running containers
bolt stop $(bolt ps -q)
```

### `bolt rm` / `bolt remove` - Remove Containers
Remove containers with force option.

```bash
# Remove stopped container
bolt rm web

# Remove multiple containers
bolt rm web gaming database

# Force remove running container
bolt rm --force gaming

# Alternative command alias
bolt remove web --force
```

## Image Management

### `bolt build` - Build Images
Build container images from Dockerfile.

```bash
# Build from current directory
bolt build

# Build with custom tag
bolt build --tag myapp:v1.0

# Build from specific path
bolt build /path/to/context --tag myapp:latest

# Custom Dockerfile
bolt build --file Dockerfile.prod --tag myapp:production
```

### `bolt pull` - Pull Images
Pull images from registry.

```bash
# Pull latest image
bolt pull ubuntu:latest

# Pull specific tag
bolt pull nvidia/cuda:12.0-runtime-ubuntu22.04

# Pull gaming images
bolt pull ghcr.io/games-on-whales/steam:latest
```

### `bolt push` - Push Images
Push images to registry.

```bash
# Push to registry
bolt push myapp:v1.0

# Push to custom registry
bolt push registry.example.com/myapp:latest
```

## Gaming Commands

### `bolt gaming gpu` - GPU Management
Configure and manage GPU resources for gaming.

```bash
# List available GPUs
bolt gaming gpu list

# Configure NVIDIA GPU
bolt gaming gpu nvidia --device 0 --dlss --raytracing

# Configure AMD GPU
bolt gaming gpu amd --device 0

# Configure nvbind runtime
bolt gaming gpu nvbind \
  --devices all \
  --driver auto \
  --performance ultra \
  --wsl2

# Check GPU compatibility
bolt gaming gpu check

# Benchmark GPU performance
bolt gaming gpu benchmark
```

### `bolt gaming` - Gaming Operations
Gaming-specific operations and optimizations.

```bash
# Setup Wine/Proton
bolt gaming wine --proton 8.0 --winver win10

# Configure audio
bolt gaming audio --system pipewire

# Launch game
bolt gaming launch steam
bolt gaming launch /path/to/game.exe --args arg1 arg2

# Start Wayland gaming session
bolt gaming wayland

# Enable real-time optimizations
bolt gaming realtime --enable

# Optimize running game process
bolt gaming optimize --pid 1234

# Show performance report
bolt gaming performance
```

## Snapshot Commands

### `bolt snapshot create` - Create Snapshots
Create manual or automatic snapshots.

```bash
# Create manual snapshot
bolt snapshot create

# Create named snapshot
bolt snapshot create --name "stable-config" --description "Working configuration"

# Create specific type
bolt snapshot create --type "manual" --name "before-update"
```

### `bolt snapshot list` / `bolt snapshot ls` - List Snapshots
List snapshots with filtering options.

```bash
# List all snapshots
bolt snapshot list

# Verbose output with details
bolt snapshot list --verbose

# Filter by type
bolt snapshot list --filter-type "daily"
bolt snapshot list --filter-type "manual"
bolt snapshot list --filter-type "named"

# Example output:
# ID                  NAME           DESCRIPTION              CREATED               SIZE     TYPE
# bolt-20231201-1200  stable-config  Working configuration    2023-12-01 12:00:00   2.5GB    named
# bolt-20231201-0200  -              Daily snapshot           2023-12-01 02:00:00   1.8GB    daily
```

### `bolt snapshot` - Snapshot Management
Manage snapshots and rollback operations.

```bash
# Show snapshot details
bolt snapshot show stable-config

# Rollback to snapshot
bolt snapshot rollback stable-config

# Force rollback without confirmation
bolt snapshot rollback stable-config --force

# Delete snapshot
bolt snapshot delete old-snapshot
bolt snapshot rm old-snapshot  # alias

# Force delete without confirmation
bolt snapshot delete old-snapshot --force
```

### `bolt snapshot cleanup` - Cleanup Operations
Apply retention policies and cleanup old snapshots.

```bash
# Dry run - show what would be deleted
bolt snapshot cleanup --dry-run

# Apply retention policy
bolt snapshot cleanup

# Force cleanup without confirmation
bolt snapshot cleanup --force
```

### `bolt snapshot config` - Configuration
Show and manage snapshot configuration.

```bash
# Show configuration
bolt snapshot config

# Verbose configuration details
bolt snapshot config --verbose

# Show configuration health
bolt snapshot config --health
```

### `bolt snapshot auto` - Automatic Snapshots
Control automatic snapshot behavior.

```bash
# Enable automatic snapshots
bolt snapshot auto enable

# Disable automatic snapshots
bolt snapshot auto disable

# Check automatic snapshot status
bolt snapshot auto status
```

## Surge Orchestration

### `bolt surge up` - Start Services
Start multi-service stacks from Boltfile.

```bash
# Start all services
bolt surge up

# Start specific services
bolt surge up web database

# Detached mode
bolt surge up --detach

# Force recreate containers
bolt surge up --force-recreate

# Start with automatic snapshot
bolt surge up  # Automatically creates snapshot if configured
```

### `bolt surge down` - Stop Services
Stop services and clean up resources.

```bash
# Stop all services
bolt surge down

# Stop specific services
bolt surge down web database

# Remove volumes
bolt surge down --volumes
```

### `bolt surge` - Service Management
Manage surge services and operations.

```bash
# Show service status
bolt surge status

# Show service logs
bolt surge logs

# Follow logs in real-time
bolt surge logs --follow

# Show logs for specific service
bolt surge logs web --tail 100

# Scale services
bolt surge scale web=3 worker=5
```

## Network Management

### `bolt network create` - Create Networks
Create container networks with various drivers.

```bash
# Create basic network
bolt network create mynetwork

# Create with custom driver
bolt network create --driver bolt gaming-net

# Create with subnet
bolt network create --driver bolt --subnet 172.20.0.0/16 gaming-net

# QUIC networking
bolt network create --driver gquic low-latency-net
```

### `bolt network` - Network Operations
Manage container networks.

```bash
# List networks
bolt network list
bolt network ls  # alias

# Remove network
bolt network remove mynetwork
bolt network rm mynetwork  # alias

# Inspect network
bolt network inspect gaming-net
```

## Volume Management

### `bolt volume create` - Create Volumes
Create persistent volumes for containers.

```bash
# Create basic volume
bolt volume create myvolume

# Create with specific driver
bolt volume create --driver local myvolume

# Create with size limit
bolt volume create --size 100GB game-data

# Create with driver options
bolt volume create --opt type=nfs --opt device=nas:/share remote-data
```

### `bolt volume` - Volume Operations
Manage persistent volumes.

```bash
# List volumes
bolt volume list
bolt volume ls  # alias

# Remove volume
bolt volume remove myvolume
bolt volume rm myvolume  # alias

# Force remove
bolt volume rm --force myvolume

# Inspect volume
bolt volume inspect game-data

# Prune unused volumes
bolt volume prune
bolt volume prune --force  # No confirmation
```

## Global Options

### Common Flags
Available across most commands:

```bash
# Verbose output
bolt --verbose ps
bolt -v ps

# Custom config file
bolt --config /path/to/Boltfile.toml surge up
bolt -c custom.toml surge up

# Help
bolt --help
bolt <command> --help
```

## Output Formatting

### Modern Table Output
Enhanced table formatting with proper alignment:

```bash
# Container listing
bolt ps
# CONTAINER ID   NAME      IMAGE           COMMAND         CREATED        STATUS       PORTS                    RUNTIME
# a1b2c3d4e5f6   web       nginx:latest    "nginx -g..."   2 hours ago    Up 2 hours   0.0.0.0:8080->80/tcp    docker

# Volume listing
bolt volume ls
# VOLUME NAME    DRIVER    SIZE      CREATED         MOUNTPOINT
# game-data      local     100GB     2 hours ago     /var/lib/bolt/volumes/game-data
# web-data       local     50GB      1 day ago       /var/lib/bolt/volumes/web-data

# Network listing
bolt network ls
# NETWORK ID     NAME           DRIVER    SCOPE    CREATED
# net-a1b2c3d4   gaming-net     bolt      local    2 hours ago
# net-f6e5d4c3   bridge         bridge    local    1 day ago
```

### JSON Output
Export data in JSON format for scripting:

```bash
# Container information as JSON
bolt ps --format json

# Snapshot information as JSON
bolt snapshot list --format json

# Network information as JSON
bolt network ls --format json
```

## Command Aliases

### Container Management
```bash
# Remove commands
bolt rm container-name      # Primary
bolt remove container-name  # Alias

# List commands
bolt ps                     # Primary
bolt ls                     # Alias (for containers)
```

### Volume Management
```bash
# List volumes
bolt volume list           # Primary
bolt volume ls             # Alias

# Remove volumes
bolt volume remove volume-name  # Primary
bolt volume rm volume-name      # Alias
```

### Network Management
```bash
# List networks
bolt network list          # Primary
bolt network ls            # Alias

# Remove networks
bolt network remove net-name    # Primary
bolt network rm net-name        # Alias
```

### Snapshot Management
```bash
# List snapshots
bolt snapshot list         # Primary
bolt snapshot ls           # Alias

# Remove snapshots
bolt snapshot delete snap-id    # Primary
bolt snapshot rm snap-id        # Alias
```

## Tab Completion

### Bash Completion
```bash
# Install completion
bolt completion bash > /etc/bash_completion.d/bolt
source /etc/bash_completion.d/bolt

# Or for current session
source <(bolt completion bash)
```

### Zsh Completion
```bash
# Install completion
bolt completion zsh > "${fpath[1]}/_bolt"
compinit

# Or for current session
source <(bolt completion zsh)
```

### Fish Completion
```bash
# Install completion
bolt completion fish > ~/.config/fish/completions/bolt.fish

# Or for current session
bolt completion fish | source
```

## Examples & Workflows

### Complete Gaming Setup
```bash
# 1. Create gaming network
bolt network create --driver bolt --subnet 172.20.0.0/16 gaming-net

# 2. Create game data volume
bolt volume create --size 100GB game-data

# 3. Create stable snapshot
bolt snapshot create --name "before-gaming-setup"

# 4. Run gaming container with nvbind
bolt run \
  --name steam \
  --runtime nvbind \
  --gpu all \
  --network gaming-net \
  --volumes game-data:/games \
  --ports 8080:8080 \
  --detach \
  ghcr.io/games-on-whales/steam:latest

# 5. Check status
bolt ps
bolt gaming gpu check
bolt snapshot list
```

### Development Workflow
```bash
# 1. Create development snapshot
bolt snapshot create --name "clean-dev-environment"

# 2. Start development stack
bolt surge up

# 3. Work on project...

# 4. Create progress snapshot
bolt snapshot create --name "feature-complete"

# 5. If something breaks, rollback
bolt snapshot rollback clean-dev-environment
```

### Container Lifecycle Management
```bash
# Start containers
bolt run --name web nginx:latest
bolt run --name db postgres:latest

# Monitor containers
bolt ps

# Restart if needed
bolt restart web --timeout 30

# Stop and remove
bolt stop web db
bolt rm web db --force

# Clean up resources
bolt volume prune
bolt network prune
```