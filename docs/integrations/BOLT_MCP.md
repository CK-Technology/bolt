# Bolt MCP Integration

## Overview

Bolt is integrating **Glyph** (MCP Protocol Core) and **Omen** (AI Router) to provide native Model Context Protocol (MCP) support in containers. This makes Bolt the first container runtime with intelligent, built-in MCP capabilities.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Bolt Container                     │
│  ┌────────────────────────────────────────────┐    │
│  │         Application (Gaming/Dev/AI)         │    │
│  └────────────────┬───────────────────────────┘    │
│                   │                                  │
│  ┌────────────────▼───────────────────────────┐    │
│  │      Glyph MCP Server (embedded/sidecar)    │    │
│  │  • Exposes tools/resources                  │    │
│  │  • Policy enforcement                       │    │
│  │  • Audit/consent hooks                      │    │
│  └────────────────┬───────────────────────────┘    │
└───────────────────┼──────────────────────────────────┘
                    │ (MCP Protocol)
                    ▼
       ┌────────────────────────────────┐
       │     Bolt MCP Gateway           │
       │  • Catalog management          │
       │  • Client connections          │
       │  • Tool registry               │
       │  • Secret management           │
       └────────────┬───────────────────┘
                    │
                    ▼
       ┌────────────────────────────────┐
       │     Omen AI Router             │
       │  • Smart provider routing      │
       │  • Cost/latency optimization   │
       │  • Usage controls & quotas     │
       │  • Multi-provider support      │
       └────────────────────────────────┘
```

## Why MCP in Bolt?

### Key Benefits

1. **Container-Native AI Tools**
   - Containers can expose capabilities as MCP tools
   - Gaming containers: GPU stats, game state, mod tools
   - Dev containers: filesystem, shell, build tools
   - AI containers: model inference, embeddings, vector search

2. **Intelligent AI Routing**
   - Omen provides cost-aware, latency-optimized routing
   - Local Ollama for fast, cheap inference
   - Cloud providers (Claude, GPT, Gemini) for complex tasks
   - Automatic failover and load balancing

3. **Ghost Stack Integration**
   - Seamless integration with Glyph, Omen, Zeke, Jarvis, GhostFlow
   - Container-native AI agent deployment
   - MCP tool federation across the entire Ghost Stack

4. **Superior to Docker**
   - Rust performance vs Go implementation
   - Smart routing vs static configuration
   - Native GPU tool exposure
   - Modern async/await architecture

## Configuration

### Boltfile MCP Configuration

Enable MCP in your container services:

```toml
project = "ai-workload"

# Service with embedded MCP server
[services.dev-env]
image = "ubuntu:latest"

[services.dev-env.mcp]
enabled = true
transport = "websocket"          # websocket, stdio, or http2
port = 7331
tools = ["filesystem", "shell", "git"]
policy = "consent-required"      # audit, consent-required, or permissive

[services.dev-env.mcp.policy]
require_consent = ["shell", "filesystem.write"]
audit_all = true
audit_log = "/var/log/mcp/audit.log"

# AI routing via Omen
[services.dev-env.omen]
enabled = true
router_strategy = "cost-optimized"    # cost-optimized, latency-first, or balanced
prefer_local = true                   # Prefer Ollama when possible
budget_limit = "10.00"                # USD per day
allowed_providers = ["ollama", "anthropic", "openai"]

[services.dev-env.omen.quotas]
max_requests_per_hour = 1000
max_tokens_per_day = 500000
```

### Gaming Container with MCP

Expose gaming tools via MCP:

```toml
[services.steam]
image = "ghcr.io/games-on-whales/steam:latest"

[services.steam.gaming.gpu]
runtime = "nvbind"
isolation_level = "exclusive"

[services.steam.mcp]
enabled = true
transport = "websocket"
tools = ["gpu_stats", "game_state", "performance_metrics"]

# Custom gaming tools
[[services.steam.mcp.custom_tools]]
name = "gpu_stats"
description = "Get real-time GPU statistics"
schema = "tools/gpu_stats.json"

[[services.steam.mcp.custom_tools]]
name = "game_state"
description = "Query current game state"
schema = "tools/game_state.json"
```

## CLI Commands

### MCP Gateway Management

```bash
# Start MCP gateway
bolt mcp gateway run
bolt mcp gateway run --port 8080 --transport streaming

# With specific servers
bolt mcp gateway run --servers dev-env,steam --tools dev-env:*

# With secret management
bolt mcp gateway run --secrets=bolt-vault:./.env

# Watch mode (auto-reload on config changes)
bolt mcp gateway run --watch --verbose
```

### MCP Server Management

```bash
# List MCP servers in containers
bolt mcp server ls
bolt mcp server list --verbose

# Inspect server capabilities
bolt mcp server inspect dev-env

# Enable/disable servers
bolt mcp server enable dev-env
bolt mcp server disable steam

# Reset server state
bolt mcp server reset dev-env
```

### MCP Tools

```bash
# List available tools
bolt mcp tools ls
bolt mcp tools list --server dev-env

# Inspect tool schema
bolt mcp tools inspect filesystem

# Call a tool directly
bolt mcp tools call dev-env:shell --input '{"command": "ls -la"}'

# Enable/disable specific tools
bolt mcp tools enable dev-env:shell
bolt mcp tools disable dev-env:shell
```

### MCP Client Management

```bash
# List connected clients
bolt mcp client ls

# Connect a new client
bolt mcp client connect --name claude --transport websocket

# Disconnect client
bolt mcp client disconnect claude
```

## Implementation Phases

### Phase 1: Glyph Integration (Current)
- [x] Add glyph as optional dependency
- [ ] Create `src/mcp/` module structure
- [ ] Implement embedded MCP server in containers
- [ ] Basic tool registry (filesystem, shell)
- [ ] Configuration parsing for Boltfile MCP section

### Phase 2: Gateway Layer (Next)
- [ ] Implement `bolt mcp gateway run` command
- [ ] MCP catalog management
- [ ] Client connection handling
- [ ] Secret/credential management
- [ ] Tool discovery across containers

### Phase 3: Omen Integration (Planned)
- [ ] Connect MCP gateway to Omen
- [ ] AI tool call routing
- [ ] Smart provider selection
- [ ] Usage tracking and quotas
- [ ] Cost optimization

### Phase 4: Ghost Stack Integration (Future)
- [ ] Seamless Zeke/Jarvis/GhostFlow integration
- [ ] Container-native AI agents
- [ ] MCP tool federation
- [ ] Advanced policy engine

## Deployment Modes

### Embedded Mode

Glyph MCP server runs inside the container process:

```toml
[services.app.mcp]
mode = "embedded"          # Lowest overhead
enabled = true
```

**Pros:** Minimal overhead, direct access to container resources
**Cons:** Shares process lifecycle with application

### Sidecar Mode

Glyph MCP server runs as separate container:

```toml
[services.app.mcp]
mode = "sidecar"           # Isolated but connected
enabled = true
```

**Pros:** Isolation, independent lifecycle, easier debugging
**Cons:** Slight overhead for IPC

### Gateway Mode

Central gateway manages all MCP servers:

```bash
bolt mcp gateway run --servers app1,app2,app3
```

**Pros:** Centralized management, client simplicity
**Cons:** Single point of coordination

## Security Model

### Policy Enforcement

```toml
[services.app.mcp.policy]
mode = "consent-required"              # audit, consent-required, permissive
require_consent = ["shell", "fs.write", "network"]
audit_all = true
audit_log = "/var/log/mcp/audit.log"
redact_secrets = true
```

### Container Boundaries

- MCP server inherits container's namespace isolation
- Filesystem access limited to container's rootfs
- Network access controlled by container network config
- GPU access via Bolt's existing GPU isolation

### Omen Quotas

```toml
[services.app.omen.quotas]
max_requests_per_hour = 1000
max_tokens_per_day = 500000
max_cost_per_day = "10.00"           # USD
blocked_providers = ["expensive-model"]
```

## Use Cases

### 1. AI-Enhanced Development Container

```toml
[services.dev]
image = "bolt://dev-env:latest"

[services.dev.mcp]
enabled = true
tools = ["filesystem", "shell", "git", "build"]

[services.dev.omen]
enabled = true
prefer_local = true
router_strategy = "latency-first"
```

**Use:** Claude Code, Cursor, or Zeke can connect to container and execute tools

### 2. Gaming Container with AI Assist

```toml
[services.game]
image = "bolt://gaming:latest"

[services.game.gaming.gpu]
runtime = "nvbind"

[services.game.mcp]
enabled = true
tools = ["gpu_stats", "game_state", "mod_loader"]

[services.game.omen]
enabled = true
router_strategy = "cost-optimized"
allowed_providers = ["ollama"]      # Local only for low latency
```

**Use:** In-game AI assistant, mod recommendations, performance optimization

### 3. AI Model Serving

```toml
[services.inference]
image = "bolt://ollama:latest"

[services.inference.mcp]
enabled = true
tools = ["model_inference", "embeddings", "context_window"]

[services.inference.omen]
enabled = true
expose_as_provider = true           # Make available to Omen
model_class = "local"
cost_per_token = 0.0
```

**Use:** Expose local AI models as Omen provider

### 4. Multi-Container AI Workflow

```toml
# Web scraper
[services.scraper]
image = "bolt://scraper:latest"

[services.scraper.mcp]
enabled = true
tools = ["http_fetch", "html_parse"]

# Data processor
[services.processor]
image = "bolt://processor:latest"

[services.processor.mcp]
enabled = true
tools = ["transform", "filter", "aggregate"]

# AI analyzer (with Omen routing)
[services.analyzer]
image = "bolt://analyzer:latest"

[services.analyzer.mcp]
enabled = true
tools = ["analyze", "classify", "summarize"]

[services.analyzer.omen]
enabled = true
router_strategy = "balanced"
budget_limit = "50.00"
```

**Use:** Orchestrated AI pipeline with tool chaining

## Integration with Ghost Stack

### Zeke (Local AI Assistant)

Zeke can connect to Bolt containers via MCP:

```bash
# In container
bolt mcp gateway run

# In Zeke
zeke connect bolt://dev-env
zeke tool filesystem read /app/src/main.rs
```

### Jarvis (Agent Runtime)

Jarvis orchestrates multi-container MCP tools:

```bash
jarvis deploy --boltfile workflow.toml
jarvis task "analyze logs from all containers"
```

### GhostFlow (Workflow Engine)

GhostFlow nodes can call Bolt container tools:

```yaml
# GhostFlow workflow
nodes:
  - type: bolt_mcp_tool
    container: dev-env
    tool: shell
    input: { command: "make build" }
```

## Performance Characteristics

### Overhead Benchmarks (Estimated)

| Mode     | Latency Overhead | Memory Overhead | CPU Overhead |
|----------|------------------|-----------------|--------------|
| Embedded | <50μs            | ~5MB            | <1%          |
| Sidecar  | <500μs           | ~20MB           | <2%          |
| Gateway  | <1ms             | ~50MB           | <3%          |

### Throughput

- **Tool calls/sec:** 10,000+ (embedded), 5,000+ (sidecar), 2,000+ (gateway)
- **Concurrent clients:** 1000+ per gateway
- **Max tools per container:** 100+

## Advantages Over Docker MCP

| Feature                | Docker MCP (Go) | Bolt MCP (Rust + Glyph + Omen) |
|------------------------|-----------------|--------------------------------|
| Performance            | Good            | **Excellent** (Rust)           |
| AI Routing             | Static config   | **Smart routing** (Omen)       |
| Gaming Support         | No              | **Native** (nvbind)            |
| GPU Tool Exposure      | Limited         | **Full** (real-time stats)     |
| Transport Options      | stdio, SSE      | **stdio, WS, HTTP/2, h3**      |
| Policy Engine          | Basic           | **Advanced** (Glyph)           |
| Observability          | Logs            | **Tracing + Metrics**          |
| Ecosystem Integration  | Docker-only     | **Ghost Stack**                |

## Roadmap

### v0.1 (Q1 2025) - Foundation
- Basic Glyph integration
- Embedded MCP servers
- Core tools (filesystem, shell)
- Simple gateway mode

### v0.2 (Q2 2025) - Gateway
- Full MCP gateway implementation
- Catalog management
- Client connection handling
- Secret management

### v0.3 (Q3 2025) - Omen Integration
- AI routing via Omen
- Smart provider selection
- Usage tracking and quotas
- Cost optimization

### v1.0 (Q4 2025) - Production Ready
- Ghost Stack integration
- Advanced policy engine
- Performance optimizations
- Comprehensive documentation

## Getting Started

### Installation

```bash
# Install Bolt with MCP support
curl -fsSL https://bolt.cktech.org | bash

# Or build from source with MCP feature
cargo build --release --features mcp-gateway
```

### Quick Start

1. **Create a Boltfile with MCP enabled:**

```toml
project = "mcp-demo"

[services.dev]
image = "ubuntu:latest"

[services.dev.mcp]
enabled = true
transport = "websocket"
tools = ["filesystem", "shell"]
```

2. **Launch the stack:**

```bash
bolt surge up
```

3. **Connect a client:**

```bash
# Start gateway
bolt mcp gateway run

# In another terminal, connect Claude Code or Zeke
```

## Resources

- **Glyph Repository:** `/data/projects/glyph`
- **Omen Repository:** `/data/projects/omen`
- **Docker MCP Reference:** `archive/mcp-gateway/docs/mcp-gateway.md`
- **MCP Specification:** https://modelcontextprotocol.io

## Support

For issues, questions, or contributions:
- GitHub Issues: https://github.com/CK-Technology/bolt/issues
- Discord: https://discord.gg/ghoststack
- Email: ghostkellz@proton.me

---

**Status:** Planning & Early Development
**Target:** v0.1 in Q1 2025
**Maintainer:** @ghostkellz
