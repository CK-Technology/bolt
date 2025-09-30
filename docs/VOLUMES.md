# Bolt Volume Management Guide

> **Advanced Persistent Storage - Better Than Docker Volumes**

This guide covers Bolt's advanced volume management system, including named volumes, host path mounting, and integration with QUIC networking.

## 🗃️ Overview

Bolt's volume system provides Docker-compatible persistent storage with significant enhancements:

- **OCI 7.0 Compliance** - Standards-compliant volume mounting
- **Named Volume Management** - Centralized storage in `/var/lib/bolt/volumes`
- **Host Path Validation** - Ensures source paths exist before mounting
- **Multiple Volume Drivers** - Local, BTRFS, ZFS support
- **Size Limits** - Prevent runaway storage usage
- **Metadata Persistence** - JSON-based volume configuration
- **QUIC Network Integration** - Optimized storage access over QUIC networks

### Key Advantages Over Docker

| Feature | Docker | Bolt |
|---------|--------|------|
| **Volume Location** | `/var/lib/docker/volumes` | `/var/lib/bolt/volumes` |
| **Size Limits** | None | Configurable per volume |
| **Metadata** | Basic | Rich JSON metadata |
| **Validation** | Limited | Host path validation |
| **Performance** | Standard | QUIC-optimized access |
| **Drivers** | Limited | Local, BTRFS, ZFS |

## 📦 Volume Types

### 1. Named Volumes

Managed volumes stored in `/var/lib/bolt/volumes/` with centralized management.

```bash
# Create named volume
bolt volume create mydata --driver local --size 10GB

# Use in container
bolt run --volume mydata:/app/data nginx:alpine

# Volume resolves to: /var/lib/bolt/volumes/mydata/_data:/app/data
```

### 2. Host Path Mounts

Direct mounting of host directories into containers with validation.

```bash
# Mount host directory (with validation)
bolt run --volume /home/user/data:/app/data nginx:alpine

# Mount read-only
bolt run --volume /home/user/config:/app/config:ro nginx:alpine
```

### 3. Temporary Volumes

In-memory volumes for temporary data.

```bash
# Create temporary volume
bolt volume create temp-data --driver tmpfs --size 1GB

# Use for temporary storage
bolt run --volume temp-data:/tmp/cache redis:alpine
```

## 🔧 Volume Management Commands

### Creating Volumes

```bash
# Basic volume creation
bolt volume create myvolume

# Volume with specific driver
bolt volume create --driver local myvolume

# Volume with size limit
bolt volume create --size 50GB large-data

# Volume with driver options
bolt volume create --opt type=btrfs --opt compress=zstd btrfs-vol

# Volume with labels
bolt volume create \
  --opt purpose=database \
  --opt backup=daily \
  database-vol
```

### Listing and Inspecting Volumes

```bash
# List all volumes
bolt volume ls
bolt volume list  # alias

# Detailed volume information
bolt volume inspect myvolume

# List volumes with custom format
bolt volume ls --format json
```

### Removing Volumes

```bash
# Remove volume (fails if in use)
bolt volume rm myvolume

# Force remove volume
bolt volume rm --force myvolume

# Remove multiple volumes
bolt volume rm vol1 vol2 vol3 --force

# Prune unused volumes
bolt volume prune
bolt volume prune --force  # No confirmation
```

## 🐳 Container Integration

### Volume Mounting Syntax

```bash
# Named volume mounting
bolt run --volume myvolume:/app/data nginx:alpine

# Host path mounting
bolt run --volume /host/path:/container/path nginx:alpine

# Read-only mounting
bolt run --volume myvolume:/app/data:ro nginx:alpine

# Multiple volumes
bolt run \
  --volume data-vol:/app/data \
  --volume config-vol:/app/config:ro \
  --volume /host/logs:/app/logs \
  nginx:alpine
```

### Volume Resolution

Bolt automatically resolves volume sources:

1. **Absolute paths** → Direct host mount with validation
2. **Named volumes** → `/var/lib/bolt/volumes/{name}/_data`
3. **Non-existent paths** → Error with clear message

```bash
# These are equivalent:
bolt run --volume mydata:/app/data nginx:alpine
bolt run --volume /var/lib/bolt/volumes/mydata/_data:/app/data nginx:alpine
```

## 🌐 QUIC Network Integration

Volumes work seamlessly with QUIC networks for optimized performance.

```bash
# Create QUIC network and volume
bolt network create app-net --driver bolt --subnet 172.20.0.0/16
bolt volume create app-data --size 20GB

# Run containers with both
bolt run \
  --name frontend \
  --network app-net \
  --volume app-data:/usr/share/nginx/html \
  --ports 8080:80 \
  nginx:alpine

bolt run \
  --name backend \
  --network app-net \
  --volume app-data:/app/shared \
  --ports 3000:3000 \
  node:alpine
```

## 📊 Volume Drivers

### Local Driver (Default)

Standard filesystem-based storage.

```bash
bolt volume create --driver local myvolume
```

**Features:**
- Direct filesystem access
- Standard performance
- No special requirements

### BTRFS Driver

Copy-on-write filesystem with snapshots.

```bash
bolt volume create --driver btrfs --opt compress=zstd btrfs-vol
```

**Features:**
- Built-in compression
- Snapshot support
- Efficient cloning

### ZFS Driver

Enterprise-grade filesystem with advanced features.

```bash
bolt volume create --driver zfs --opt compression=lz4 zfs-vol
```

**Features:**
- Data integrity checking
- Advanced snapshots
- Replication support

## 🔍 Practical Examples

### Web Application with Database

```bash
# Create volumes for different components
bolt volume create web-data --size 10GB
bolt volume create db-data --size 50GB
bolt volume create logs --size 5GB

# Create network for isolation
bolt network create webapp-net --driver bolt

# Run database with persistent storage
bolt run \
  --name database \
  --network webapp-net \
  --volume db-data:/var/lib/postgresql/data \
  --volume logs:/var/log \
  --env POSTGRES_DB=myapp \
  postgres:15

# Run web server with shared storage
bolt run \
  --name webserver \
  --network webapp-net \
  --volume web-data:/usr/share/nginx/html \
  --volume logs:/var/log/nginx \
  --ports 8080:80 \
  nginx:alpine
```

### Development Environment

```bash
# Create development volumes
bolt volume create node-modules --size 5GB
bolt volume create build-cache --size 2GB

# Mount source code and use named volumes for cache
bolt run \
  --name dev-env \
  --volume ./src:/app/src:ro \
  --volume node-modules:/app/node_modules \
  --volume build-cache:/app/.cache \
  --ports 3000:3000 \
  --env NODE_ENV=development \
  node:18-alpine
```

### Gaming Setup with Storage

```bash
# Create gaming network and storage
bolt network create gaming-net --driver bolt --subnet 172.30.0.0/16
bolt volume create game-saves --size 100GB
bolt volume create game-cache --size 50GB

# Run gaming container with GPU and storage
bolt run \
  --name gaming-rig \
  --network gaming-net \
  --runtime nvbind \
  --gpu all \
  --volume game-saves:/home/gamer/saves \
  --volume game-cache:/home/gamer/.cache \
  --volume /tmp/.X11-unix:/tmp/.X11-unix \
  --env DISPLAY=:0 \
  --ports 7777:7777 \
  ubuntu:22.04
```

## 🛠️ Advanced Configuration

### Volume Metadata

Bolt stores rich metadata for each volume:

```json
{
  "name": "myvolume",
  "driver": "local",
  "mountpoint": "/var/lib/bolt/volumes/myvolume/_data",
  "created": "2024-01-15T10:30:00Z",
  "labels": {
    "purpose": "webapp-storage",
    "backup": "daily"
  },
  "options": {
    "size": "50GB"
  },
  "scope": "Local",
  "size_limit": 53687091200,
  "used_by": ["webapp-frontend", "webapp-backend"]
}
```

### Backup Integration

Volumes integrate with Bolt's snapshot system:

```bash
# Create snapshot before major updates
bolt snapshot create --name before-migration

# Volume data is included in snapshots
bolt snapshot rollback before-migration
```

### Performance Tuning

Optimize volume performance for different workloads:

```bash
# High-performance database volume
bolt volume create \
  --driver btrfs \
  --opt compress=zstd \
  --opt ssd=true \
  --size 100GB \
  database-vol

# Gaming storage with low latency
bolt volume create \
  --driver local \
  --opt noatime=true \
  --size 500GB \
  games-storage
```

## 🚨 Troubleshooting

### Common Issues

**Volume not found:**
```bash
# Check if volume exists
bolt volume ls | grep myvolume

# Create if missing
bolt volume create myvolume
```

**Permission errors:**
```bash
# Check volume permissions
bolt volume inspect myvolume

# Fix ownership (if needed)
sudo chown -R 1000:1000 /var/lib/bolt/volumes/myvolume/_data
```

**Storage full:**
```bash
# Check volume usage
df -h /var/lib/bolt/volumes/

# Clean up unused volumes
bolt volume prune
```

**Mount failures:**
```bash
# Check if host path exists
ls -la /host/path

# Verify volume driver
bolt volume inspect myvolume | grep driver
```

## 📈 Monitoring and Metrics

Monitor volume usage and performance:

```bash
# Volume space usage
bolt volume ls --format "table {{.Name}}\t{{.Size}}\t{{.Used}}\t{{.Available}}"

# Container volume usage
bolt stats --volumes

# Volume I/O metrics
bolt volume inspect myvolume --metrics
```

## 🔐 Security Considerations

### Volume Security

- **Isolation**: Volumes are isolated between containers by default
- **Permissions**: Proper filesystem permissions are enforced
- **Validation**: Host paths are validated before mounting
- **Encryption**: Use encrypted filesystem drivers for sensitive data

### Best Practices

1. **Use named volumes** for persistent data
2. **Mount read-only** when possible
3. **Set size limits** to prevent storage exhaustion
4. **Regular backups** using snapshot integration
5. **Monitor usage** to prevent issues

---

## 🚀 Performance Benefits

Bolt volumes provide significant performance improvements:

- **QUIC Integration**: Reduced latency for network-attached storage
- **Zero-Copy**: Direct memory mapping where possible
- **Efficient Drivers**: BTRFS and ZFS optimization
- **Gaming Mode**: Specialized optimizations for low-latency workloads

Experience the next generation of container storage with Bolt! 🎯