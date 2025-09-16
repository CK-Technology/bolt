# Drift - Self-Hosted Bolt Registry

**Drift** is a self-hosted registry for Bolt profiles, plugins, and gaming configurations. Built as an extension to Docker Registry v2 with Rust, it provides a complete solution for managing and distributing Bolt gaming optimizations across teams and organizations.

## Overview

Drift combines the reliability of Docker Registry v2 with specialized support for Bolt's gaming ecosystem:

- **Profiles**: Gaming optimization profiles (Steam, competitive, AI/ML workloads)
- **Plugins**: Third-party GPU and gaming optimization plugins
- **Configurations**: Complete Boltfile environments and Surge orchestration configs
- **Authentication**: Secure user management and access control
- **Analytics**: Usage metrics and download statistics

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Bolt Client   │───▶│  Drift Registry │───▶│  Storage Layer  │
│                 │    │                 │    │                 │
│ - Profile Mgmt  │    │ - REST API      │    │ - File System   │
│ - Plugin Mgmt   │    │ - Auth Service  │    │ - S3 Compatible │
│ - Config Sync   │    │ - Metrics       │    │ - Docker Layers │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## Tech Stack

- **Language**: Rust 2024 Edition
- **Web Framework**: Axum (async, high-performance)
- **Storage**: Compatible with Docker Registry v2 + Bolt extensions
- **Database**: SQLite (embedded) or PostgreSQL (production)
- **Authentication**: JWT tokens with refresh
- **Metrics**: Built-in analytics and usage tracking

## Integration with Bolt

### Bolt as Dependency

Add Bolt to your `Cargo.toml`:

```toml
[dependencies]
bolt = { git = "https://github.com/CK-Technology/bolt", features = ["registry"] }
```

### Using Bolt Registry Client

```rust
use bolt::registry::{DriftClient, DriftRegistry, RegistryConfig};
use bolt::optimizations::OptimizationProfile;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure registry connection
    let registry = DriftRegistry {
        name: "company-drift".to_string(),
        url: "https://drift.company.com".to_string(),
        auth: None,
        version: "v1".to_string(),
    };

    // Create client
    let mut client = DriftClient::new(&registry);

    // Authenticate
    client.authenticate("username", "password").await?;

    // List available profiles
    let profiles = client.list_profiles(Some(1), Some(20)).await?;
    println!("Available profiles: {}", profiles.total);

    // Install a gaming profile
    let install_path = std::path::Path::new("./profiles");
    client.install_profile("steam-gaming", install_path).await?;

    Ok(())
}
```

## API Specification

### Profile Management

```
GET    /v1/profiles              # List profiles (paginated)
POST   /v1/profiles/search       # Search profiles with filters
GET    /v1/profiles/{name}       # Get profile details
GET    /v1/profiles/{name}/download # Download profile
POST   /v1/profiles/upload       # Upload new profile
DELETE /v1/profiles/{name}       # Delete profile (auth required)
```

### Plugin Management

```
GET    /v1/plugins               # List plugins
POST   /v1/plugins/search        # Search plugins
GET    /v1/plugins/{name}        # Get plugin details
GET    /v1/plugins/{name}/download # Download plugin binary
POST   /v1/plugins/upload        # Upload plugin
DELETE /v1/plugins/{name}        # Delete plugin
```

### Authentication

```
POST   /v1/auth/login           # Login with username/password
POST   /v1/auth/register        # Register new user
POST   /v1/auth/refresh         # Refresh JWT token
POST   /v1/auth/logout          # Logout and invalidate token
```

### Metrics & Analytics

```
GET    /v1/metrics              # Overall registry metrics
GET    /v1/metrics/profiles     # Profile-specific metrics
GET    /v1/metrics/plugins      # Plugin-specific metrics
```

### Health & System

```
GET    /health                  # Health check
GET    /version                 # Registry version info
```

## Content Types

Drift extends Docker Registry v2 with Bolt-specific media types:

```
application/vnd.bolt.profile.v1+toml              # Optimization profiles
application/vnd.bolt.plugin.manifest.v1+json      # Plugin manifests
application/vnd.bolt.plugin.binary.v1+octet-stream # Plugin binaries
application/vnd.docker.distribution.manifest.v2+json # Docker compatibility
```

## Example Profile Upload

```rust
use bolt::registry::{ProfileUploadRequest, ProfileUploadMetadata, SystemRequirements};
use bolt::optimizations::OptimizationProfile;
use bolt::plugins::GpuVendor;

// Create a gaming profile
let profile = OptimizationProfile {
    name: "my-gaming-profile".to_string(),
    description: "Custom gaming profile for RTX 4090".to_string(),
    priority: 100,
    // ... profile configuration
};

// Upload metadata
let metadata = ProfileUploadMetadata {
    author_email: "user@company.com".to_string(),
    license: Some("MIT".to_string()),
    repository: Some("https://github.com/company/gaming-profiles".to_string()),
    tags: vec!["gaming".to_string(), "nvidia".to_string(), "rtx4090".to_string()],
    compatible_games: vec!["Counter-Strike 2".to_string(), "Valorant".to_string()],
    system_requirements: SystemRequirements {
        min_cpu_cores: Some(8),
        min_memory_gb: Some(16),
        required_gpu_vendor: Some("nvidia".to_string()),
        min_gpu_memory_gb: Some(12),
        supported_os: vec!["linux".to_string()],
    },
};

// Upload to registry
let upload_request = ProfileUploadRequest {
    profile,
    metadata,
};

client.upload_profile(&upload_request).await?;
```

## Example Plugin Upload

```rust
use bolt::plugins::PluginManifest;
use bolt::registry::{PluginUploadRequest, PluginUploadMetadata};

// Plugin manifest
let manifest = PluginManifest {
    name: "nvidia-dlss-optimizer".to_string(),
    version: "1.0.0".to_string(),
    author: "Company Gaming Team".to_string(),
    description: "Advanced DLSS optimization plugin".to_string(),
    plugin_type: bolt::plugins::PluginType::GpuOptimization,
    entry_point: "libnvidia_dlss_optimizer.so".to_string(),
    dependencies: vec![],
    permissions: vec![bolt::plugins::Permission::GpuAccess],
    supported_gpus: vec![bolt::plugins::GpuVendor::Nvidia],
};

// Plugin binary data (would be loaded from file)
let binary_data = std::fs::read("./plugins/libnvidia_dlss_optimizer.so")?;

// Upload metadata
let metadata = PluginUploadMetadata {
    author_email: "gaming-team@company.com".to_string(),
    license: "Proprietary".to_string(),
    repository: "https://github.com/company/bolt-plugins".to_string(),
    documentation: Some("https://wiki.company.com/gaming/dlss-optimizer".to_string()),
    supported_platforms: vec!["linux-x86_64".to_string()],
};

let upload_request = PluginUploadRequest {
    manifest,
    metadata,
    binary_data,
};

client.upload_plugin(&upload_request).await?;
```

## Search and Discovery

```rust
use bolt::registry::SearchRequest;

// Search for Steam gaming profiles
let search_request = SearchRequest {
    query: Some("steam".to_string()),
    tags: Some(vec!["gaming".to_string(), "nvidia".to_string()]),
    game: Some("Counter-Strike".to_string()),
    gpu_vendor: Some("nvidia".to_string()),
    sort_by: Some("downloads".to_string()),
    sort_order: Some("desc".to_string()),
    page: Some(1),
    per_page: Some(10),
};

let results = client.search_profiles(&search_request).await?;

for profile in results.results {
    println!("📦 {} - {} downloads", profile.name, profile.downloads);
    println!("   ⭐ {:.1}/5.0 rating", profile.rating);
    println!("   🎮 Games: {:?}", profile.compatible_games);
}
```

## Development Setup

### 1. Clone and Setup

```bash
git clone https://github.com/CK-Technology/Drift.git
cd Drift

# Add Bolt dependency
cargo add bolt --git https://github.com/CK-Technology/bolt --features registry
```

### 2. Database Setup

```bash
# Development (SQLite)
cargo run --bin drift -- migrate

# Production (PostgreSQL)
export DATABASE_URL="postgres://user:pass@localhost/drift"
cargo run --bin drift -- migrate
```

### 3. Configuration

Create `drift.toml`:

```toml
[server]
bind = "0.0.0.0:5000"
workers = 4

[storage]
type = "filesystem"  # or "s3"
path = "./data"

[auth]
jwt_secret = "your-secret-key"
token_expiry_hours = 24

[registry]
max_upload_size_mb = 100
rate_limit_per_hour = 1000

[bolt]
# Integration with Bolt optimizations
enable_profile_validation = true
enable_plugin_sandbox = true
auto_update_profiles = false
```

### 4. Run Development Server

```bash
cargo run --bin drift-server
```

### 5. Test with Bolt Client

```bash
# Configure Bolt to use local Drift registry
bolt config set registry.url http://localhost:5000
bolt config set registry.username admin
bolt config set registry.password admin

# List available profiles
bolt profiles list

# Install a gaming profile
bolt profiles install steam-gaming
```

## Production Deployment

### Docker Compose

```yaml
version: '3.8'
services:
  drift:
    image: drift:latest
    ports:
      - "5000:5000"
    environment:
      - DATABASE_URL=postgres://drift:password@db:5432/drift
      - STORAGE_TYPE=s3
      - S3_BUCKET=company-drift-registry
      - JWT_SECRET=production-secret
    volumes:
      - ./config:/app/config
    depends_on:
      - db
      - minio

  db:
    image: postgres:15
    environment:
      - POSTGRES_DB=drift
      - POSTGRES_USER=drift
      - POSTGRES_PASSWORD=password
    volumes:
      - postgres_data:/var/lib/postgresql/data

  minio:
    image: minio/minio
    command: server /data --console-address ":9001"
    ports:
      - "9000:9000"
      - "9001:9001"
    environment:
      - MINIO_ROOT_USER=admin
      - MINIO_ROOT_PASSWORD=password
    volumes:
      - minio_data:/data

volumes:
  postgres_data:
  minio_data:
```

### Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: drift-registry
spec:
  replicas: 3
  selector:
    matchLabels:
      app: drift-registry
  template:
    metadata:
      labels:
        app: drift-registry
    spec:
      containers:
      - name: drift
        image: drift:latest
        ports:
        - containerPort: 5000
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: drift-secrets
              key: database-url
        - name: JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: drift-secrets
              key: jwt-secret
        volumeMounts:
        - name: config
          mountPath: /app/config
      volumes:
      - name: config
        configMap:
          name: drift-config
```

## Gaming Organization Use Cases

### 1. Esports Team Profile Management

```rust
// Team manager uploads optimized profiles for different games
let csgo_profile = create_csgo_competitive_profile();
let valorant_profile = create_valorant_tournament_profile();

client.upload_profile(&csgo_profile).await?;
client.upload_profile(&valorant_profile).await?;

// Players sync profiles before matches
bolt_sync_team_profiles("esports-team-registry.company.com").await?;
```

### 2. Gaming Cafe Chain

```rust
// Central profile management for gaming cafes
let gaming_cafe_profiles = vec![
    "steam-general",
    "battle-net-optimized",
    "epic-games-performance",
    "local-tournament-competitive"
];

for profile in gaming_cafe_profiles {
    client.install_profile(profile, &cafe_profiles_path).await?;
}
```

### 3. Game Development Studio

```rust
// Developers share optimized testing profiles
let dev_profiles = vec![
    "unity-development",
    "unreal-engine-testing",
    "performance-profiling",
    "graphics-debugging"
];

// QA team uses standardized testing configurations
let qa_configs = client.search_profiles(&SearchRequest {
    tags: Some(vec!["qa".to_string(), "testing".to_string()]),
    ..Default::default()
}).await?;
```

## Security Features

- **JWT Authentication**: Secure token-based auth with refresh
- **Rate Limiting**: Prevent abuse with configurable limits
- **Content Validation**: Automatic validation of profiles and plugins
- **Access Control**: Role-based permissions (read, write, admin)
- **Audit Logging**: Track all registry operations
- **Sandboxed Plugins**: Safe plugin execution environment

## Monitoring & Analytics

```rust
// Get registry metrics
let metrics = client.get_metrics().await?;
println!("📊 Registry Stats:");
println!("   Profiles: {}", metrics.total_profiles);
println!("   Plugins: {}", metrics.total_plugins);
println!("   Downloads: {}", metrics.total_downloads);
println!("   Storage: {:.2} GB", metrics.storage_usage_bytes as f64 / 1e9);

// Popular profiles this month
for profile in metrics.popular_profiles {
    println!("🔥 {} - {} downloads", profile.name, profile.downloads);
}
```

## Contributing

1. **Bolt Integration**: Use Bolt crate for all gaming/optimization logic
2. **API Compatibility**: Maintain Docker Registry v2 compatibility
3. **Testing**: Comprehensive tests for registry operations
4. **Documentation**: API docs and integration examples
5. **Security**: Follow security best practices for registry software

## Getting Started

```bash
# 1. Set up Drift registry
git clone https://github.com/CK-Technology/Drift.git
cd Drift && cargo run

# 2. Configure Bolt client
bolt config set registry.url http://localhost:5000
bolt auth login

# 3. Start using profiles
bolt profiles search gaming
bolt profiles install steam-gaming
bolt surge up --profile steam-gaming

# 4. Share your optimizations
bolt profiles create my-custom-profile
bolt profiles upload my-custom-profile
```

This creates a complete ecosystem where Bolt provides the runtime and optimization engine, while Drift handles the distribution and management of gaming configurations across teams and organizations.