# ⚡ Advanced Bolt Features

Deep dive into Bolt's advanced capabilities for power users.

---

## Table of Contents

- [QUIC Networking](#quic-networking)
- [Snapshot System](#snapshot-system)
- [GPU Scheduling](#gpu-scheduling)
- [Performance Tuning](#performance-tuning)
- [Security](#security)
- [Hardware Optimization](#hardware-optimization)
- [Custom Runtimes](#custom-runtimes)
- [Monitoring & Metrics](#monitoring--metrics)

---

## QUIC Networking

Bolt uses QUIC for container networking, providing better performance than traditional TCP/bridge networking.

### Benefits

- **50% Lower Latency**: UDP-based transport with 0-RTT connection establishment
- **Better Congestion Control**: Modern algorithms vs TCP
- **Automatic Encryption**: TLS 1.3 built-in
- **Connection Migration**: Seamless network changes
- **Multiplexing**: Multiple streams without head-of-line blocking

### Create QUIC Network

```bash
# Create Bolt QUIC network (default)
bolt network create my-network --driver bolt

# With custom subnet
bolt network create my-network \
  --driver bolt \
  --subnet 172.20.0.0/16

# Traditional bridge network (fallback)
bolt network create legacy-network --driver bridge
```

### Network Drivers

| Driver | Protocol | Use Case | Performance |
|--------|----------|----------|-------------|
| `bolt` | QUIC/UDP | Default, best performance | ⚡ Fastest |
| `gquic` | Google QUIC variant | Legacy compatibility | ⚡ Fast |
| `bridge` | Traditional Linux bridge | Compatibility | 🐢 Slower |
| `host` | Host networking | Maximum performance | ⚡⚡ Ultra-fast |
| `none` | No networking | Isolation | N/A |

### Network Configuration

```toml
# Boltfile.toml
[network.my-network]
driver = "bolt"
subnet = "172.20.0.0/16"
gateway = "172.20.0.1"
ipv6 = true
encryption = true
congestion_control = "bbr2"  # bbr, bbr2, cubic, reno

[service.web]
networks = ["my-network"]
ipv4_address = "172.20.0.10"  # Static IP
```

### Advanced Network Features

```bash
# Enable IPv6
bolt network create my-network --ipv6 --subnet-ipv6 fd00::/64

# Custom DNS
bolt network create my-network --dns 1.1.1.1 --dns 8.8.8.8

# Network isolation
bolt network create isolated --internal  # No external access

# Connect running container to network
bolt network connect my-network container-name
```

---

## Snapshot System

Bolt supports filesystem snapshots with GPU state capture using BTRFS or ZFS.

### BTRFS Snapshots

```bash
# Enable BTRFS backend
# Ensure /var/lib/bolt is on BTRFS filesystem

# Create snapshot
bolt snapshot create --name before-upgrade

# With GPU state (clocks, temps, power, driver)
bolt snapshot create \
  --name gaming-session \
  --with-gpu-state \
  --description "CS2 optimal settings"

# List snapshots
bolt snapshot list

# Output:
# ╔══════════════════╦═══════════════════╦═══════════╦═════════╗
# ║ Snapshot ID      ║      Created      ║   Size    ║  Type   ║
# ╠══════════════════╬═══════════════════╬═══════════╬═════════╣
# ║ before-upgrade   ║ 2025-01-15 14:30  ║  2.3 GB   ║ Manual  ║
# ║ gaming-session   ║ 2025-01-15 18:45  ║  1.8 GB   ║ Manual  ║
# ║ auto-daily-20250115 ║ 2025-01-15 00:00 ║ 3.1 GB  ║ Auto    ║
# ╚══════════════════╩═══════════════════╩═══════════╩═════════╝

# Show snapshot details
bolt snapshot show gaming-session

# Rollback to snapshot
bolt snapshot rollback gaming-session

# With GPU state restoration
bolt snapshot rollback gaming-session --restore-gpu-state
```

### ZFS Snapshots

```bash
# Enable ZFS backend
# Ensure /var/lib/bolt is on ZFS dataset

# Same commands work with ZFS
bolt snapshot create --name checkpoint-1

# ZFS provides additional features:
# - Compression
# - Deduplication
# - Send/receive for backup
```

### Automatic Snapshots

```bash
# Enable automatic snapshots
bolt snapshot auto enable

# Configure retention policy
bolt snapshot config

# Output:
# Automatic Snapshots: Enabled
# Retention Policy:
#   - Keep last 7 daily snapshots
#   - Keep last 4 weekly snapshots
#   - Keep last 6 monthly snapshots
# Filesystem: BTRFS
# GPU State Capture: Enabled

# Cleanup old snapshots
bolt snapshot cleanup --dry-run  # Preview deletions
bolt snapshot cleanup --force    # Actually delete
```

### Snapshot Types

```toml
# Config: ~/.config/bolt/snapshots.toml
[retention]
keep_hourly = 24
keep_daily = 7
keep_weekly = 4
keep_monthly = 6

[schedule]
auto_snapshot = true
hourly = true
daily = true
weekly = true
monthly = true

[options]
capture_gpu_state = true
compression = "zstd"
dedup = true
```

---

## GPU Scheduling

Advanced GPU allocation and scheduling strategies.

### Scheduling Strategies

```bash
# Configure scheduling strategy
bolt gpu config --strategy <STRATEGY>

# Available strategies:
# - round-robin: Distribute evenly across GPUs
# - least-utilized: Choose GPU with lowest utilization
# - most-memory: Choose GPU with most available VRAM
# - exclusive: Each container gets dedicated GPU(s)
```

### Strategy Details

#### Round-Robin
```bash
bolt gpu config --strategy round-robin

# Container 1 → GPU 0
# Container 2 → GPU 1
# Container 3 → GPU 2
# Container 4 → GPU 0 (wraps around)
```

#### Least-Utilized
```bash
bolt gpu config --strategy least-utilized

# Dynamically assigns to GPU with lowest utilization
# Good for mixed workloads (training + inference)
```

#### Most-Memory
```bash
bolt gpu config --strategy most-memory

# Prioritizes GPUs with most available VRAM
# Good for memory-intensive workloads
```

#### Exclusive
```bash
bolt gpu config --strategy exclusive

# One container per GPU, no sharing
# Best for gaming or critical workloads
```

### VRAM Quotas

```bash
# Limit container VRAM usage
bolt run --gpus 1 --gpu-memory 16GB pytorch train.py

# Multiple containers sharing GPU
bolt run --gpus 1 --gpu-memory 8GB --name job1 pytorch train1.py
bolt run --gpus 1 --gpu-memory 8GB --name job2 pytorch train2.py

# Bolt enforces memory limits via cgroups
```

### MIG (Multi-Instance GPU)

For NVIDIA A100/H100 GPUs:

```bash
# Enable MIG mode on GPU 0
sudo nvidia-smi -i 0 -mig 1

# Create MIG instances
bolt gpu mig --gpu 0 --profile 3g.40gb

# Available profiles:
# A100-80GB:
#   1g.10gb, 2g.20gb, 3g.40gb, 7g.80gb
# A100-40GB:
#   1g.5gb, 2g.10gb, 3g.20gb, 7g.40gb
# H100:
#   1g.12gb, 2g.24gb, 3g.47gb, 7g.94gb

# Run container on MIG instance
bolt run --gpus mig:3g.40gb pytorch train.py

# List MIG instances
bolt gpu mig list
```

### GPU Metrics API

```bash
# Real-time metrics
bolt gpu metrics

# JSON output for scripting
bolt gpu metrics --format json

# Filter by container
bolt gpu metrics --container training-job

# Custom interval
bolt gpu metrics --interval 0.5  # Update every 500ms
```

---

## Performance Tuning

### Container Startup Optimization

```bash
# Pre-load frequently used images
bolt pull pytorch/pytorch:latest
bolt pull nvidia/cuda:12.0-base

# Use image layer caching
bolt build --cache-from pytorch/pytorch:latest -t myimage .

# Measure startup time
time bolt run --rm myimage echo "ready"
```

### CPU Affinity

```bash
# Pin to specific CPU cores
bolt run --cpuset-cpus 0-7 myapp

# Performance cores only (P-cores on hybrid CPUs)
bolt hardware affinity gaming
# Output: Recommended CPU mask: 0-7 (P-cores)

bolt run --cpuset-cpus 0-7 game.exe
```

### CPU Governor

```bash
# Set CPU governor for maximum performance
bolt hardware governor performance

# Governors:
# - performance: Maximum frequency, no power saving
# - powersave: Minimum frequency
# - ondemand: Scale based on load
# - schedutil: Modern scheduler-based scaling

# Reset to default
bolt hardware governor schedutil
```

### Memory Optimization

```bash
# Limit memory usage
bolt run --memory 16g pytorch train.py

# Memory + swap limit
bolt run --memory 16g --memory-swap 20g pytorch train.py

# Disable swap for container
bolt run --memory 16g --memory-swap 16g pytorch train.py

# Reserve memory (no overcommit)
bolt run --memory-reservation 12g pytorch train.py
```

### I/O Priority

```bash
# Real-time I/O priority
bolt run --io-priority realtime database

# Best-effort (default)
bolt run --io-priority besteffort app

# Idle
bolt run --io-priority idle background-job
```

---

## Security

### Namespace Isolation

Bolt uses Linux namespaces for isolation:

```bash
# Full isolation (PID, network, mount, UTS, IPC)
bolt run --isolation full myapp

# Shared namespaces (less secure, better performance)
bolt run --network host --pid host myapp
```

### Capabilities

```bash
# Drop all capabilities
bolt run --cap-drop ALL nginx

# Add specific capability
bolt run --cap-add NET_ADMIN network-tool

# Common capabilities:
# - NET_ADMIN: Network configuration
# - SYS_ADMIN: System administration
# - SYS_PTRACE: Process tracing
# - NET_RAW: Raw sockets
```

### Seccomp Profiles

```bash
# Default seccomp profile (blocks dangerous syscalls)
bolt run myapp

# Custom seccomp profile
bolt run --seccomp-profile ./custom-seccomp.json myapp

# Disable seccomp (not recommended)
bolt run --seccomp unconfined myapp
```

### Read-Only Containers

```bash
# Read-only root filesystem
bolt run --read-only nginx

# With tmpfs for /tmp
bolt run --read-only --tmpfs /tmp nginx
```

### User Namespaces

```bash
# Run as non-root inside container
bolt run --user 1000:1000 myapp

# Map UID/GID ranges
bolt run --userns-remap myapp
```

---

## Hardware Optimization

### CPU Detection

```bash
# Detect CPU capabilities
bolt hardware cpu

# Output:
# CPU: AMD Ryzen 9 7950X (16 cores, 32 threads)
# Architecture: Zen 4
# Features:
#   ✓ AVX2
#   ✓ AVX-512
#   ✓ FMA3
#   ✓ AES-NI
# Topology:
#   L1d Cache: 512 KB
#   L1i Cache: 512 KB
#   L2 Cache: 16 MB
#   L3 Cache: 64 MB
# Performance Cores: 0-15 (all cores)
# Efficiency Cores: None (not hybrid)

# Optimize for detected CPU
bolt run --cpu-optimization auto pytorch train.py
```

### GPU Detection

```bash
# Detect GPUs with full details
bolt hardware gpu --verbose

# Output:
# GPU 0: NVIDIA GeForce RTX 4090
#   Architecture: Ada Lovelace
#   Compute Capability: 8.9
#   CUDA Cores: 16384
#   Tensor Cores: 512 (4th gen)
#   RT Cores: 128 (3rd gen)
#   Memory: 24 GB GDDR6X
#   Memory Bandwidth: 1008 GB/s
#   Base Clock: 2235 MHz
#   Boost Clock: 2520 MHz
#   TDP: 450W
#   Features:
#     ✓ DLSS 3.5
#     ✓ Ray Tracing
#     ✓ NVENC (8th gen)
#     ✓ NVDEC (5th gen)
```

### Memory Detection

```bash
# Detect memory configuration
bolt hardware memory

# Output:
# Total Memory: 64 GB
# Available: 48 GB
# Type: DDR5-6000
# Channels: 2
# NUMA Nodes: 1
# Huge Pages: Enabled (2MB pages)
```

### Topology-Aware Scheduling

```bash
# Bind to NUMA node with GPU
bolt run --gpus 0 --cpuset-mems 0 --cpuset-cpus 0-15 pytorch train.py

# Auto-detect and bind
bolt run --gpus 0 --numa-aware pytorch train.py
```

---

## Custom Runtimes

### OCI Runtime Configuration

```bash
# Use custom OCI runtime
bolt run --runtime runc myapp

# Available runtimes:
# - crun (default): Fast C-based runtime
# - runc: Reference Go implementation
# - youki: Rust-based runtime
# - kata-runtime: VM-based isolation

# Configure default runtime
bolt config set runtime.default crun
```

### nvbind Runtime

```bash
# Use nvbind for ultra-fast GPU passthrough
bolt run --runtime nvbind --gpus 1 pytorch train.py

# nvbind configuration
bolt gaming gpu nvbind \
  --devices all \
  --driver auto \
  --performance ultra \
  --wsl2 false

# Check compatibility
bolt gaming gpu check
```

---

## Monitoring & Metrics

### GhostPanel Metrics Server

```bash
# Start WebSocket metrics server (60 FPS)
bolt metrics serve --port 8080

# Connect from GhostPanel frontend
# ws://localhost:8080/ws/metrics/<container_id>

# REST API endpoints:
# - GET  /api/gpu/status
# - GET  /api/containers
# - GET  /api/metrics/:container_id
# - POST /api/gpu/config
```

### Prometheus Integration

```bash
# Export metrics in Prometheus format
bolt metrics export --format prometheus --port 9090

# Metrics available:
# - bolt_container_cpu_usage
# - bolt_container_memory_usage
# - bolt_gpu_utilization
# - bolt_gpu_memory_used
# - bolt_gpu_temperature
# - bolt_gpu_power_draw
# - bolt_network_rx_bytes
# - bolt_network_tx_bytes
```

### Custom Metrics

```bash
# Query metrics programmatically
bolt metrics query \
  --container training-job \
  --metric gpu_utilization \
  --from "1 hour ago" \
  --format json

# Output:
# {
#   "container": "training-job",
#   "metric": "gpu_utilization",
#   "data": [
#     {"timestamp": 1705334400, "value": 85.2},
#     {"timestamp": 1705334401, "value": 87.1},
#     ...
#   ]
# }
```

---

## Advanced Boltfile

Complete example with all features:

```toml
# Boltfile.toml

# Global configuration
[config]
runtime = "crun"
network_driver = "bolt"
log_level = "info"

# Networks
[network.frontend]
driver = "bolt"
subnet = "172.20.0.0/24"
ipv6 = true
encryption = true

[network.backend]
driver = "bolt"
subnet = "172.21.0.0/24"
internal = true  # No external access

# Volumes
[volume.db-data]
driver = "local"
options = { type = "btrfs", compression = "zstd" }

[volume.model-cache]
driver = "local"
size = "500GB"
dedup = true

# Services
[service.web]
image = "nginx:latest"
ports = ["8080:80", "8443:443"]
networks = ["frontend"]
volumes = ["./html:/usr/share/nginx/html:ro"]
memory = "2g"
cpus = "2"
healthcheck = { cmd = "curl -f http://localhost/health", interval = "30s" }
restart = "always"

[service.api]
image = "node:18"
networks = ["frontend", "backend"]
env = { NODE_ENV = "production", DB_HOST = "database" }
depends_on = ["database"]
memory = "4g"
cpus = "4"

[service.ai-inference]
image = "vllm/vllm-openai:latest"
gpus = 2
gpu_memory = "20GB"
gpu_strategy = "least-utilized"
ports = ["8000:8000"]
volumes = ["model-cache:/models:ro"]
env = {
    MODEL = "meta-llama/Llama-3-70B",
    TENSOR_PARALLEL_SIZE = "2"
}
healthcheck = { enabled = true, endpoint = "/health" }
auto_restart = true

[service.database]
image = "postgres:15"
networks = ["backend"]
volumes = ["db-data:/var/lib/postgresql/data"]
env = { POSTGRES_PASSWORD_FILE = "/run/secrets/db_password" }
memory = "8g"
shm_size = "1g"

[service.monitoring]
image = "prometheus:latest"
ports = ["9090:9090"]
volumes = ["./prometheus.yml:/etc/prometheus/prometheus.yml:ro"]
command = ["--config.file=/etc/prometheus/prometheus.yml"]

# Secrets
[secret.db_password]
file = "./secrets/db_password.txt"

# Snapshots
[snapshot]
auto = true
retention = { daily = 7, weekly = 4, monthly = 6 }
capture_gpu_state = true
compression = "zstd"
```

```bash
# Deploy entire stack
bolt surge up

# Scale specific service
bolt surge scale ai-inference=4

# Update service
bolt surge up ai-inference --force-recreate
```

---

## Benchmarking

### Run Benchmarks

```bash
# Complete benchmark suite
bolt benchmark run

# Specific benchmarks
bolt benchmark run --test container-startup
bolt benchmark run --test gpu-passthrough
bolt benchmark run --test network-throughput

# Compare with Docker
bolt benchmark compare-docker

# Save results
bolt benchmark run --output results.json
```

### Custom Benchmarks

```bash
# Benchmark custom workload
bolt benchmark custom \
  --image pytorch/pytorch:latest \
  --command "python train.py" \
  --iterations 100 \
  --gpus 1
```

---

## Debugging

### Container Inspection

```bash
# Full container details
bolt inspect container-name

# Specific field
bolt inspect --format '{{.State.Pid}}' container-name

# JSON output
bolt inspect --format json container-name | jq '.NetworkSettings'
```

### Process Debugging

```bash
# Attach to container process
bolt attach container-name

# Execute debug shell
bolt exec -it container-name /bin/bash

# View process tree
bolt top container-name

# Export container filesystem
bolt export container-name > container.tar
```

### Network Debugging

```bash
# Test connectivity
bolt exec container-name ping other-container

# DNS resolution
bolt exec container-name nslookup service-name

# Network statistics
bolt stats container-name --no-stream

# Packet capture
bolt exec container-name tcpdump -i eth0
```

---

## Best Practices

### 1. Use Named Volumes for Persistent Data

```bash
bolt volume create db-data
bolt run -v db-data:/var/lib/postgresql/data postgres
```

### 2. Enable Health Checks

```toml
[service.api]
healthcheck = { cmd = "curl -f http://localhost/health", interval = "30s" }
```

### 3. Set Resource Limits

```bash
bolt run --memory 4g --cpus 2 --gpu-memory 8GB myapp
```

### 4. Use Snapshots Before Upgrades

```bash
bolt snapshot create --name before-v2-upgrade
bolt surge up --force-recreate
# If issues: bolt snapshot rollback before-v2-upgrade
```

### 5. Monitor GPU Usage

```bash
bolt gpu metrics --interval 1
```

---

## Next Steps

- **Quickstart Guide**: [QUICKSTART.md](./QUICKSTART.md)
- **AI Workloads**: [AI_WORKLOADS.md](./AI_WORKLOADS.md)
- **Migration Guide**: [MIGRATION.md](./MIGRATION.md)
- **Gaming Setup**: [GAMING.md](./GAMING.md)

---

*Master Bolt's full potential!* ⚡
