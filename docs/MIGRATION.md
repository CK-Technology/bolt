# 🔄 Migrating from Docker to Bolt

Complete guide to migrating from Docker/Podman to Bolt.

---

## Why Migrate?

**Performance Gains:**
- **6x faster** container startup (87ms vs 523ms)
- **130x faster** GPU passthrough (0.8μs vs 104μs)
- **1.75x better** network throughput (2.1 Gbps vs 1.2 Gbps)
- **6x less** memory overhead (8 MB vs 50 MB per container)

**AI/ML Features:**
- Intelligent multi-GPU scheduling
- vLLM integration for LLM serving
- HuggingFace Hub model caching
- MIG support for A100/H100 GPUs

---

## Command Comparison

### Basic Commands

| Docker | Bolt | Notes |
|--------|------|-------|
| `docker run` | `bolt run` | Identical syntax |
| `docker ps` | `bolt ps` | Same flags (-a, --all) |
| `docker exec` | `bolt exec` | Same flags (-it, -u, -w) |
| `docker logs` | `bolt logs` | Same flags (-f, --tail) |
| `docker stop` | `bolt stop` | Same behavior |
| `docker rm` | `bolt rm` | Same flags (-f) |
| `docker restart` | `bolt restart` | Same timeout flag |
| `docker pull` | `bolt pull` | OCI registry compatible |
| `docker push` | `bolt push` | OCI registry compatible |
| `docker build` | `bolt build` | Dockerfile compatible |

### Compose / Orchestration

| Docker Compose | Bolt Surge | Notes |
|----------------|------------|-------|
| `docker-compose up` | `bolt surge up` | Uses Boltfile.toml |
| `docker-compose down` | `bolt surge down` | Same behavior |
| `docker-compose logs` | `bolt surge logs` | Same flags |
| `docker-compose scale` | `bolt surge scale` | Same syntax |
| `docker-compose.yml` | `Boltfile.toml` | TOML format |

### Networking

| Docker | Bolt | Notes |
|--------|------|-------|
| `docker network create` | `bolt network create` | QUIC-based by default |
| `docker network ls` | `bolt network list` | Same output |
| `docker network rm` | `bolt network remove` | Same behavior |

### Volumes

| Docker | Bolt | Notes |
|--------|------|-------|
| `docker volume create` | `bolt volume create` | Same syntax |
| `docker volume ls` | `bolt volume list` | Same output |
| `docker volume rm` | `bolt volume remove` | Same flags |
| `docker volume prune` | `bolt volume prune` | Same behavior |

---

## Migrating Dockerfiles

Bolt uses standard OCI images - **no changes needed**!

```dockerfile
# This Dockerfile works identically in both Docker and Bolt
FROM ubuntu:22.04

RUN apt-get update && apt-get install -y python3

COPY app.py /app/app.py

CMD ["python3", "/app/app.py"]
```

```bash
# Build with Docker
docker build -t myapp:latest .

# Build with Bolt (same command)
bolt build -t myapp:latest .

# Both create OCI-compatible images
```

---

## Migrating docker-compose.yml

Convert `docker-compose.yml` to `Boltfile.toml`:

### Docker Compose

```yaml
# docker-compose.yml
version: '3.8'

services:
  web:
    image: nginx:latest
    ports:
      - "8080:80"
    volumes:
      - ./html:/usr/share/nginx/html
    environment:
      - NGINX_HOST=example.com

  api:
    image: node:18
    ports:
      - "3000:3000"
    environment:
      - NODE_ENV=production
    depends_on:
      - db

  db:
    image: postgres:15
    environment:
      - POSTGRES_PASSWORD=secret
    volumes:
      - db-data:/var/lib/postgresql/data

volumes:
  db-data:
```

### Bolt Surge

```toml
# Boltfile.toml
[service.web]
image = "nginx:latest"
ports = ["8080:80"]
volumes = ["./html:/usr/share/nginx/html"]
env = { NGINX_HOST = "example.com" }

[service.api]
image = "node:18"
ports = ["3000:3000"]
env = { NODE_ENV = "production" }
depends_on = ["db"]

[service.db]
image = "postgres:15"
env = { POSTGRES_PASSWORD = "secret" }
volumes = ["db-data:/var/lib/postgresql/data"]

[volume.db-data]
driver = "local"
```

```bash
# Start with Bolt
bolt surge up
```

### Automatic Conversion

```bash
# Bolt can convert docker-compose.yml automatically
bolt compat compose convert docker-compose.yml

# Output: Boltfile.toml created

# Or run directly without converting
bolt compat compose up
```

---

## GPU Workloads

### Docker (nvidia-docker)

```bash
# Docker requires nvidia-docker runtime
docker run --gpus all nvidia/cuda:12.0-base nvidia-smi

# Or with runtime flag
docker run --runtime=nvidia nvidia/cuda:12.0-base nvidia-smi
```

### Bolt (Native GPU Support)

```bash
# Bolt has native GPU support
bolt run --gpus all nvidia/cuda:12.0-base nvidia-smi

# Intelligent scheduling
bolt run --gpus 2 pytorch/pytorch:latest python train.py

# Specific GPU selection
bolt run --gpus device=0,2 pytorch/pytorch:latest python train.py

# MIG support
bolt run --gpus mig:1g.5gb pytorch/pytorch:latest python train.py
```

---

## Registry Compatibility

Bolt is fully compatible with OCI registries:

```bash
# Docker Hub (default)
bolt pull nginx:latest
bolt pull ubuntu:22.04

# GitHub Container Registry
bolt pull ghcr.io/owner/image:tag

# Google Container Registry
bolt pull gcr.io/project/image:tag

# AWS ECR
bolt pull 123456789.dkr.ecr.us-east-1.amazonaws.com/image:tag

# Private registries
bolt pull registry.example.com/image:tag
```

### Registry Authentication

```bash
# Docker login (creates ~/.docker/config.json)
docker login registry.example.com

# Bolt uses the same credentials automatically
bolt pull registry.example.com/private-image:latest

# Or login with Bolt
bolt login registry.example.com
```

---

## Migrating Running Containers

### Export from Docker

```bash
# Export Docker container as image
docker commit my-container my-container:snapshot
docker save my-container:snapshot -o container.tar

# Import to Bolt
bolt load -i container.tar

# Run with Bolt
bolt run my-container:snapshot
```

### Live Migration

```bash
# 1. Create snapshot of Docker container
docker commit running-container migration:snapshot

# 2. Export to tar
docker save migration:snapshot > /tmp/migration.tar

# 3. Load into Bolt
bolt load < /tmp/migration.tar

# 4. Run with Bolt
bolt run -d --name migrated migration:snapshot

# 5. Stop Docker container
docker stop running-container
```

---

## Networking Migration

### Docker Networks

```bash
# Docker bridge network
docker network create my-network
docker run --network my-network --name web nginx
docker run --network my-network --name api node:18

# Containers can communicate via name
# web -> http://api:3000
```

### Bolt Networks (QUIC-based)

```bash
# Bolt QUIC network (faster, lower latency)
bolt network create my-network
bolt run --network my-network --name web nginx
bolt run --network my-network --name api node:18

# Same service discovery
# web -> http://api:3000

# But with QUIC benefits:
# - 50% lower latency
# - Better congestion control
# - Automatic encryption
```

---

## Volume Migration

### Export Docker Volume

```bash
# Create backup of Docker volume
docker run --rm \
  -v my-volume:/data \
  -v $(pwd):/backup \
  ubuntu tar czf /backup/volume.tar.gz /data

# Create Bolt volume
bolt volume create my-volume

# Import data
bolt run --rm \
  -v my-volume:/data \
  -v $(pwd):/backup \
  ubuntu tar xzf /backup/volume.tar.gz -C /
```

### Direct Copy

```bash
# Find Docker volume path
docker volume inspect my-volume | grep Mountpoint

# Create Bolt volume
bolt volume create my-volume

# Find Bolt volume path
bolt volume inspect my-volume | grep Mountpoint

# Copy data
sudo cp -a /var/lib/docker/volumes/my-volume/_data/* \
         /var/lib/bolt/volumes/my-volume/_data/
```

---

## CI/CD Migration

### GitHub Actions

```yaml
# Before (Docker)
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Build image
        run: docker build -t myapp:latest .
      - name: Run tests
        run: docker run myapp:latest pytest

# After (Bolt)
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install Bolt
        run: curl -fsSL https://get.bolt.run | sh
      - name: Build image
        run: bolt build -t myapp:latest .
      - name: Run tests
        run: bolt run myapp:latest pytest
```

### GitLab CI

```yaml
# Before (Docker)
build:
  stage: build
  script:
    - docker build -t $CI_REGISTRY_IMAGE:$CI_COMMIT_SHA .
    - docker push $CI_REGISTRY_IMAGE:$CI_COMMIT_SHA

# After (Bolt)
build:
  stage: build
  before_script:
    - curl -fsSL https://get.bolt.run | sh
  script:
    - bolt build -t $CI_REGISTRY_IMAGE:$CI_COMMIT_SHA .
    - bolt push $CI_REGISTRY_IMAGE:$CI_COMMIT_SHA
```

---

## Feature Mapping

### Docker Features → Bolt Equivalents

| Docker Feature | Bolt Equivalent | Notes |
|----------------|-----------------|-------|
| Swarm Mode | Bolt Surge | Single-node orchestration |
| BuildKit | Bolt Build | OCI-compatible builder |
| Docker Desktop | Bolt CLI | Native Linux performance |
| nvidia-docker | Built-in GPU | Native support, no runtime needed |
| Docker Compose | Bolt Surge | TOML format, same features |
| Content Trust | Image Signing | OCI signature verification |
| Health Checks | Health Checks | Same syntax in Boltfile |
| Resource Limits | Resource Limits | Same flags (--memory, --cpus) |

---

## Compatibility Layer

For gradual migration, use Bolt's Docker compatibility layer:

```bash
# Create Docker command alias
bolt compat docker-alias enable

# Now 'docker' commands use Bolt
docker run nginx:latest
docker ps
docker logs container-name

# Disable when ready to migrate fully
bolt compat docker-alias disable
```

---

## Migration Checklist

### Pre-Migration

- [ ] Inventory all running containers (`docker ps -a`)
- [ ] List all images (`docker images`)
- [ ] Document all networks (`docker network ls`)
- [ ] Document all volumes (`docker volume ls`)
- [ ] Export important container data
- [ ] Test Bolt in development environment

### Migration Steps

- [ ] Install Bolt (`curl -fsSL https://get.bolt.run | sh`)
- [ ] Verify GPU detection (`bolt gpu list`)
- [ ] Convert docker-compose.yml to Boltfile.toml
- [ ] Test converted Boltfile (`bolt surge up --dry-run`)
- [ ] Migrate volumes (export/import)
- [ ] Rebuild or import images
- [ ] Start services with Bolt
- [ ] Verify application functionality
- [ ] Update CI/CD pipelines
- [ ] Monitor performance improvements

### Post-Migration

- [ ] Remove Docker (`sudo apt remove docker-ce docker-ce-cli`)
- [ ] Clean up Docker data (`sudo rm -rf /var/lib/docker`)
- [ ] Update documentation
- [ ] Train team on Bolt-specific features
- [ ] Optimize GPU scheduling strategies
- [ ] Set up snapshots for critical containers

---

## Troubleshooting

### Image Not Found

```bash
# Docker images aren't automatically available in Bolt
# Pull again or export/import

# Export from Docker
docker save nginx:latest > nginx.tar

# Import to Bolt
bolt load < nginx.tar
```

### Container Won't Start

```bash
# Check logs
bolt logs container-name

# Compare with Docker run command
# Ensure all flags are present
bolt run -d --name test \
  -p 8080:80 \
  -v /host:/container \
  -e VAR=value \
  nginx:latest
```

### Network Issues

```bash
# Verify network exists
bolt network list

# Recreate network
bolt network rm old-network
bolt network create old-network

# Check container network settings
bolt inspect container-name | grep -A 10 Network
```

### Performance Not Improved

```bash
# Ensure nvbind is installed for GPU performance
bolt gpu check

# Check GPU scheduling strategy
bolt gpu config --strategy least-utilized

# Monitor actual performance
bolt gpu metrics
```

---

## Getting Help

### Docker Compatibility

```bash
# Check if Docker command is supported
bolt compat docker --help

# Run Docker commands via Bolt
bolt compat docker run nginx:latest
```

### Community Resources

- **Documentation**: https://bolt.run/docs
- **GitHub**: https://github.com/yourusername/bolt
- **Discord**: https://discord.gg/bolt

---

## Example: Complete Migration

```bash
# 1. Export existing Docker setup
docker-compose down
docker save $(docker images -q) -o all-images.tar
docker volume ls -q | xargs -I {} \
  docker run --rm -v {}:/data -v $(pwd):/backup \
  ubuntu tar czf /backup/{}.tar.gz /data

# 2. Install Bolt
curl -fsSL https://get.bolt.run | sh

# 3. Import images
bolt load < all-images.tar

# 4. Create volumes and import data
for vol in *.tar.gz; do
  volname="${vol%.tar.gz}"
  bolt volume create "$volname"
  bolt run --rm -v "$volname:/data" -v $(pwd):/backup \
    ubuntu tar xzf "/backup/$vol" -C /
done

# 5. Convert compose file
bolt compat compose convert docker-compose.yml

# 6. Start services
bolt surge up -d

# 7. Verify
bolt ps
bolt surge logs

# 8. Clean up Docker
sudo systemctl stop docker
sudo apt remove docker-ce docker-ce-cli
```

---

*Welcome to faster, smarter containerization with Bolt!* 🚀
