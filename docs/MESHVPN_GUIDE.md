# Bolt MeshVPN Guide - GhostWire Integration

> **Zero-Configuration Mesh VPN for Container Networking**

This guide covers Bolt's revolutionary MeshVPN capabilities through GhostWire integration, providing Tailscale-like functionality optimized for containers and gaming.

## 🌐 What is MeshVPN?

MeshVPN creates a secure, peer-to-peer network overlay that allows containers and nodes to communicate directly, regardless of their physical network location. Unlike traditional VPNs that require a central server, mesh VPNs create direct encrypted connections between peers.

### Key Benefits

- **Zero Configuration** - Automatic peer discovery and connection
- **NAT Traversal** - Works behind firewalls and NAT devices
- **Direct P2P** - No central bottleneck, optimal performance
- **Gaming Optimized** - Ultra-low latency for gaming workloads
- **Self-Healing** - Automatic route optimization and failover
- **Secure by Default** - End-to-end encryption with modern cryptography

## 🏗️ Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    GhostWire MeshVPN                            │
├─────────────────┬─────────────────┬─────────────────┬──────────┤
│ Control Plane   │   Data Plane    │  Gaming Engine  │Security  │
│                 │                 │                 │          │
│ • Peer Discovery│ • QUIC Protocol │ • Packet Pacing │• WireGuar│
│ • Route Mgmt    │ • NAT Traversal │ • Jitter Buffer │• ChaCha20│
│ • Mesh Topology │ • Load Balance  │ • Frame Sync    │• Poly1305│
│ • Health Check  │ • Path Selection│ • Anti-Cheat    │• Perfect │
│ • Magic DNS     │ • Traffic Shape │ • Lag Predict   │ Forward  │
└─────────────────┴─────────────────┴─────────────────┴──────────┘
```

## 🚀 Quick Start

### 1. Install GhostWire Control Server

```bash
# Option 1: Self-hosted control server
docker run -d \
  --name ghostwire-control \
  -p 443:443 \
  -p 80:80 \
  -v ghostwire-data:/var/lib/ghostwire \
  ghostkellz/ghostwire:latest

# Option 2: Use managed control server
# Sign up at https://ghostwire.dev for hosted control plane
```

### 2. Configure Bolt with GhostWire

```toml
# ~/.config/bolt/network.toml
[meshvpn]
provider = "ghostwire"
control_server = "https://ghostwire.your-domain.com"
auth_key = "tskey-auth-your-key-here"

[meshvpn.features]
magic_dns = true
gaming_optimizations = true
nat_traversal = true
subnet_routing = true
exit_nodes = true
```

### 3. Initialize MeshVPN

```bash
# Authenticate with control server
bolt meshvpn login --auth-key tskey-auth-your-key

# Join mesh network
bolt meshvpn up

# Verify connection
bolt meshvpn status
```

## 🎮 Gaming Optimization Features

### Ultra-Low Latency Mode

GhostWire includes specialized optimizations for gaming workloads:

```rust
use bolt::networking::meshvpn::{GamingConfig, LatencyTier};

let gaming_config = GamingConfig {
    latency_tier: LatencyTier::Competitive, // < 1ms additional latency
    packet_pacing: true,                    // Smooth packet delivery
    jitter_buffer: Duration::from_micros(500), // Minimize jitter
    congestion_control: CongestionControl::Gaming, // Gaming-aware CC
    priority_queues: true,                  // Separate queues for game traffic
    frame_synchronization: true,            // Sync with game frame rate
    anti_cheat_integration: true,           // Verified connection metadata
};

networking.meshvpn.configure_gaming(gaming_config).await?;
```

### Game Server Discovery

Automatic discovery of game servers across the mesh:

```rust
// Find game servers in the mesh
let game_servers = networking.meshvpn.discover_game_servers().await?;

for server in game_servers {
    println!("Game: {} - Server: {} - Ping: {}ms - Players: {}/{}",
        server.game_name,
        server.server_name,
        server.latency_ms,
        server.current_players,
        server.max_players
    );
}

// Auto-connect to best server
let best_server = networking.meshvpn.find_best_game_server("counter-strike").await?;
networking.meshvpn.optimize_route_to(best_server.node_id).await?;
```

### Gaming Traffic Prioritization

```bash
# Enable gaming mode for specific containers
bolt run --name game-server \
  --meshvpn-gaming \
  --meshvpn-priority=high \
  --meshvpn-latency-target=5ms \
  valheim-server:latest

# Configure gaming traffic rules
bolt meshvpn gaming add-rule \
  --port=7777-7784 \
  --protocol=udp \
  --priority=critical \
  --latency-target=1ms \
  --jitter-limit=0.5ms
```

## 🕸️ Mesh Network Management

### Node Discovery and Connection

```rust
use bolt::networking::meshvpn::{MeshNode, NodeCapabilities};

// Get mesh topology
let mesh_status = networking.meshvpn.get_mesh_status().await?;

println!("Mesh Network Status:");
println!("- Total nodes: {}", mesh_status.total_nodes);
println!("- Direct connections: {}", mesh_status.direct_connections);
println!("- Relay connections: {}", mesh_status.relay_connections);

for node in mesh_status.nodes {
    println!("Node: {} ({})", node.hostname, node.ip);
    println!("  Capabilities: {:?}", node.capabilities);
    println!("  Latency: {:.1}ms", node.latency_ms);
    println!("  Last seen: {} ago", node.last_seen);
}
```

### Automatic Route Optimization

```rust
// Enable intelligent routing
networking.meshvpn.enable_smart_routing(SmartRoutingConfig {
    latency_optimization: true,
    bandwidth_optimization: false, // Prioritize latency over bandwidth
    cost_optimization: false,      // Don't consider cost for gaming

    // Route selection criteria
    max_acceptable_latency: Duration::from_millis(20),
    min_bandwidth_mbps: 10.0,
    max_hops: 3,

    // Gaming-specific routing
    avoid_congested_paths: true,
    prefer_direct_connections: true,
    enable_multipath: false, // Single path for consistent latency
}).await?;
```

### Subnet Routing

Enable access to remote networks through mesh nodes:

```bash
# Advertise local subnets to mesh
bolt meshvpn advertise-routes 192.168.1.0/24,10.0.0.0/8

# Accept routes from specific nodes
bolt meshvpn accept-routes --from=node-gaming-rig-01

# Use specific node as exit node for internet traffic
bolt meshvpn set-exit-node node-gaming-rig-01
```

## 🔒 Security Features

### End-to-End Encryption

All mesh traffic is encrypted using modern cryptography:

```rust
use bolt::networking::meshvpn::{EncryptionConfig, CipherSuite};

let encryption_config = EncryptionConfig {
    cipher_suite: CipherSuite::ChaCha20Poly1305,
    key_rotation_interval: Duration::from_hours(24),
    forward_secrecy: true,
    post_quantum_ready: true, // Future-proof against quantum attacks

    // Gaming optimizations
    hardware_acceleration: true, // Use CPU crypto instructions
    zero_copy_encryption: true,  // Minimize memory copies
};

networking.meshvpn.configure_encryption(encryption_config).await?;
```

### Access Control Lists (ACLs)

Fine-grained access control for mesh resources:

```json
{
  "acls": [
    {
      "action": "accept",
      "src": ["tag:gaming-nodes"],
      "dst": ["tag:game-servers:27015"],
      "proto": "udp"
    },
    {
      "action": "accept",
      "src": ["tag:admin"],
      "dst": ["*:22", "*:3389"],
      "proto": "tcp"
    },
    {
      "action": "drop",
      "src": ["*"],
      "dst": ["tag:production:*"],
      "comment": "Default deny to production"
    }
  ],
  "nodeAttrs": [
    {
      "target": ["192.168.1.100"],
      "attr": ["tag:gaming-nodes", "tag:admin"]
    }
  ]
}
```

### Device Trust and Verification

```bash
# Trust a new device
bolt meshvpn trust-device --device-id=abcd1234 --name="gaming-laptop"

# Revoke device access
bolt meshvpn revoke-device --device-id=abcd1234

# List trusted devices
bolt meshvpn list-devices

# Enable device verification
bolt meshvpn enable-device-verification \
  --require-os-verification \
  --require-secure-boot \
  --check-antivirus
```

## 🔧 Advanced Configuration

### Self-Hosted Control Server

Deploy your own GhostWire control server for maximum privacy:

```yaml
# docker-compose.yml
version: '3.8'
services:
  ghostwire-control:
    image: ghostkellz/ghostwire-control:latest
    ports:
      - "443:443"
      - "3478:3478/udp" # STUN server
    volumes:
      - ghostwire-data:/var/lib/ghostwire
      - ./certs:/etc/ssl/certs
    environment:
      GHOSTWIRE_DOMAIN: ghostwire.your-domain.com
      GHOSTWIRE_DB_URL: postgres://ghostwire:password@db:5432/ghostwire
      GHOSTWIRE_OIDC_ISSUER: https://auth.your-domain.com
      GHOSTWIRE_GAMING_FEATURES: "true"
    depends_on:
      - db

  db:
    image: postgres:15
    environment:
      POSTGRES_DB: ghostwire
      POSTGRES_USER: ghostwire
      POSTGRES_PASSWORD: secure_password
    volumes:
      - postgres-data:/var/lib/postgresql/data

  stun-server:
    image: ghostkellz/ghostwire-stun:latest
    ports:
      - "3478:3478/udp"
      - "3479:3479/udp"
    environment:
      STUN_EXTERNAL_IP: your.public.ip.address

volumes:
  ghostwire-data:
  postgres-data:
```

### Multi-Region Setup

Deploy control servers in multiple regions for optimal performance:

```bash
# Primary control server (US East)
export GHOSTWIRE_REGION=us-east
export GHOSTWIRE_PRIMARY=true
docker-compose up -d

# Secondary control server (EU West)
export GHOSTWIRE_REGION=eu-west
export GHOSTWIRE_PRIMARY_URL=https://us-east.ghostwire.com
docker-compose up -d

# Gaming region (Asia Pacific)
export GHOSTWIRE_REGION=ap-southeast
export GHOSTWIRE_GAMING_OPTIMIZED=true
docker-compose up -d
```

### Custom DERP Relays

Deploy custom relay servers for regions where direct connections aren't possible:

```bash
# Deploy DERP relay server
docker run -d \
  --name ghostwire-derp \
  -p 443:443 \
  -p 3478:3478/udp \
  -e DERP_HOSTNAME=derp-singapore.your-domain.com \
  -e DERP_CERTMODE=letsencrypt \
  ghostkellz/ghostwire-derp:latest

# Register relay with control server
curl -X POST https://control.ghostwire.com/api/v1/derp/register \
  -H "Authorization: Bearer $CONTROL_API_KEY" \
  -d '{
    "hostname": "derp-singapore.your-domain.com",
    "region": "ap-southeast-1",
    "ipv4": "203.0.113.1",
    "ipv6": "2001:db8::1",
    "stunPort": 3478,
    "insecureForTests": false
  }'
```

## 🎯 Use Cases

### 1. Gaming Server Network

Connect game servers across multiple data centers:

```bash
# Node 1: US West game server
bolt run -d \
  --name cs-server-usw \
  --meshvpn-advertise \
  --meshvpn-gaming \
  counter-strike-server:latest

# Node 2: EU West game server
bolt run -d \
  --name cs-server-euw \
  --meshvpn-advertise \
  --meshvpn-gaming \
  counter-strike-server:latest

# Load balancer automatically routes players to closest server
bolt run -d \
  --name cs-loadbalancer \
  --meshvpn-discover-backends \
  --publish 27015:27015 \
  gaming-loadbalancer:latest
```

### 2. Development Environment

Connect dev environments across different locations:

```yaml
# docker-compose.yml
version: '3.8'
services:
  web-dev:
    image: nodejs:18
    networks:
      - meshvpn
    labels:
      - "meshvpn.expose=3000"
      - "meshvpn.hostname=web-dev.mesh"

  db-dev:
    image: postgres:15
    networks:
      - meshvpn
    labels:
      - "meshvpn.expose=5432"
      - "meshvpn.hostname=db-dev.mesh"
      - "meshvpn.acl=tag:developers"

networks:
  meshvpn:
    driver: bolt-meshvpn
    driver_opts:
      meshvpn.magic_dns: "true"
      meshvpn.subnet_routing: "true"
```

### 3. Home Lab Integration

Connect home lab services with cloud infrastructure:

```bash
# Home lab node - advertise local services
bolt meshvpn up --advertise-routes=192.168.1.0/24 --hostname=homelab

# Cloud node - access home lab services
bolt meshvpn up --accept-routes --hostname=cloud-1

# Access home lab services from cloud
docker run --rm -it --network=bolt-meshvpn \
  alpine ping homelab.mesh
```

### 4. Secure Remote Access

Provide secure access to internal services without traditional VPN:

```bash
# Internal services node
bolt meshvpn up \
  --advertise-tags=internal-services \
  --advertise-routes=10.0.0.0/8

# Remote worker access
bolt meshvpn up \
  --accept-routes \
  --hostname=remote-worker-laptop \
  --tags=remote-workers

# Services are accessible via Magic DNS
curl http://gitlab.internal.mesh
ssh admin@jenkins.internal.mesh
```

## 📊 Monitoring and Troubleshooting

### Network Diagnostics

```bash
# Comprehensive network status
bolt meshvpn status --verbose

# Test connectivity to specific node
bolt meshvpn ping node-gaming-rig-01

# Trace route through mesh
bolt meshvpn traceroute 100.64.0.5

# Check NAT traversal capabilities
bolt meshvpn check-nat

# Bandwidth test between nodes
bolt meshvpn speedtest node-gaming-rig-01
```

### Performance Monitoring

```rust
// Real-time mesh performance metrics
let metrics = networking.meshvpn.get_performance_metrics().await?;

println!("MeshVPN Performance:");
println!("- Avg latency: {:.1}ms", metrics.avg_latency_ms);
println!("- P99 latency: {:.1}ms", metrics.p99_latency_ms);
println!("- Packet loss: {:.3}%", metrics.packet_loss_percent);
println!("- Throughput: {:.1} Mbps", metrics.throughput_mbps);
println!("- Direct connections: {}/{}",
         metrics.direct_connections,
         metrics.total_connections);

// Gaming-specific metrics
let gaming_metrics = networking.meshvpn.get_gaming_metrics().await?;
println!("Gaming Performance:");
println!("- Jitter: {:.2}ms", gaming_metrics.jitter_ms);
println!("- Frame sync accuracy: {:.2}%", gaming_metrics.frame_sync_accuracy);
println!("- Input lag: {:.1}ms", gaming_metrics.input_lag_ms);
```

### Log Analysis

```bash
# View mesh logs
bolt logs meshvpn

# Debug connection issues
bolt meshvpn debug --node=problematic-node

# Export network diagnostics
bolt meshvpn export-diagnostics --output=diagnostics.zip

# Real-time traffic analysis
bolt meshvpn monitor --real-time --filter=gaming-traffic
```

## 🛠️ Integration with Other Services

### Kubernetes Integration

```yaml
# ghostwire-operator.yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: ghostwire-operator
  namespace: kube-system
spec:
  selector:
    matchLabels:
      app: ghostwire-operator
  template:
    metadata:
      labels:
        app: ghostwire-operator
    spec:
      hostNetwork: true
      containers:
      - name: ghostwire
        image: ghostkellz/ghostwire-k8s:latest
        env:
        - name: GHOSTWIRE_AUTH_KEY
          valueFrom:
            secretKeyRef:
              name: ghostwire-auth
              key: auth-key
        - name: NODE_NAME
          valueFrom:
            fieldRef:
              fieldPath: spec.nodeName
        securityContext:
          capabilities:
            add: ["NET_ADMIN"]
```

### Docker Swarm Integration

```bash
# Create overlay network with mesh capabilities
docker network create \
  --driver=bolt-meshvpn \
  --opt=meshvpn.control_server=https://ghostwire.com \
  --opt=meshvpn.auth_key=${GHOSTWIRE_AUTH_KEY} \
  --opt=meshvpn.gaming_optimizations=true \
  swarm-mesh

# Deploy service across swarm with mesh networking
docker service create \
  --name game-server-cluster \
  --network swarm-mesh \
  --constraint 'node.labels.gpu==nvidia' \
  --replicas 3 \
  game-server:latest
```

### Terraform Provider

```hcl
terraform {
  required_providers {
    ghostwire = {
      source  = "ghostkellz/ghostwire"
      version = "~> 1.0"
    }
  }
}

provider "ghostwire" {
  control_server = "https://ghostwire.your-domain.com"
  api_key       = var.ghostwire_api_key
}

resource "ghostwire_device" "gaming_nodes" {
  count    = 3
  hostname = "gaming-node-${count.index + 1}"
  tags     = ["gaming", "gpu-enabled"]

  routes = [
    "192.168.1.0/24"  # Home network
  ]

  gaming_optimizations = true
  exit_node           = false
}

resource "ghostwire_acl" "gaming_policy" {
  rules = [
    {
      action = "accept"
      src    = ["tag:gaming"]
      dst    = ["tag:gaming:27015"]
      proto  = "udp"
    }
  ]
}
```

## 🚨 Security Best Practices

### Device Management

```bash
# Generate device-specific auth keys
bolt meshvpn generate-auth-key \
  --device="gaming-laptop" \
  --expires="30d" \
  --tags="gaming,personal" \
  --reusable=false

# Enable device approval workflow
bolt meshvpn configure \
  --require-device-approval \
  --approval-timeout=24h \
  --auto-approve-tags="trusted"

# Set up device compliance checking
bolt meshvpn compliance enable \
  --check-os-version \
  --check-security-patches \
  --check-antivirus \
  --quarantine-non-compliant
```

### Network Segmentation

```bash
# Create isolated gaming segment
bolt meshvpn create-segment gaming \
  --tags="gaming-nodes" \
  --isolated-from="production,corporate" \
  --allow-internet

# Production segment with strict controls
bolt meshvpn create-segment production \
  --tags="prod-servers" \
  --deny-internet \
  --require-approval-for="ssh,rdp" \
  --log-all-connections
```

### Audit and Compliance

```bash
# Enable comprehensive logging
bolt meshvpn configure logging \
  --log-level=info \
  --log-connections \
  --log-denied-access \
  --export-to-siem

# Generate compliance reports
bolt meshvpn audit generate-report \
  --period=monthly \
  --include-access-logs \
  --include-device-compliance \
  --format=json > compliance-report.json
```

---

## 🎯 Performance Optimization

### Gaming Workload Tuning

```bash
# Optimize for competitive gaming (minimize latency)
bolt meshvpn tune gaming-competitive \
  --target-latency=1ms \
  --disable-compression \
  --enable-fast-retransmit \
  --cpu-priority=realtime

# Optimize for streaming (maximize quality)
bolt meshvpn tune gaming-streaming \
  --target-bandwidth=100mbps \
  --enable-adaptive-bitrate \
  --buffer-size=large \
  --error-correction=enhanced
```

### Resource Management

```bash
# Allocate dedicated CPU cores to mesh networking
bolt meshvpn configure resources \
  --cpu-cores=2,3 \
  --memory-limit=2GB \
  --network-buffers=large \
  --interrupt-affinity=optimal

# Enable hardware acceleration where available
bolt meshvpn enable hardware-accel \
  --crypto-offload \
  --network-offload \
  --compression-offload
```

---

This MeshVPN implementation through GhostWire integration provides enterprise-grade networking with gaming-first optimizations. The combination of zero-configuration setup, ultra-low latency, and robust security makes it ideal for gaming servers, development environments, and secure remote access scenarios.

The mesh topology ensures optimal performance by creating direct peer-to-peer connections while maintaining the simplicity of traditional VPN solutions. With built-in NAT traversal, automatic route optimization, and comprehensive monitoring, GhostWire represents the next evolution of VPN technology optimized for the container era. 🚀