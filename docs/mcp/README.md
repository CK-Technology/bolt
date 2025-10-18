# Bolt MCP Integration

**Model Context Protocol (MCP) support for Bolt Container Runtime**

Bolt integrates [Glyph](https://github.com/ghostkellz/glyph) to provide native MCP support, enabling AI assistants to interact with containers through standardized tools.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Architecture](#architecture)
- [Features](#features)
- [Usage](#usage)
  - [Server Mode](#server-mode)
  - [Gateway Mode](#gateway-mode)
- [Configuration](#configuration)
- [Tools](#available-tools)
- [Examples](#examples)
- [Troubleshooting](#troubleshooting)

---

## Overview

Bolt's MCP integration provides two modes:

1. **Server Mode** - Embedded MCP server within Bolt runtime
2. **Gateway Mode** - Centralized gateway managing multiple MCP servers

Both modes expose Bolt's container management capabilities as MCP tools, allowing AI assistants like Claude to interact with containers.

---

## Quick Start

### Install Bolt with MCP Support

```bash
# Build from source
cargo build --features mcp --release

# The binary will be at
./target/x86_64-unknown-linux-gnu/release/bolt
```

### Start MCP Server

```bash
# WebSocket (default)
bolt mcp serve

# stdio (for Claude Desktop)
bolt mcp serve --transport stdio

# HTTP
bolt mcp serve --transport http --port 8080
```

### Claude Desktop Integration

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "bolt": {
      "command": "bolt",
      "args": ["mcp", "serve", "--transport", "stdio"]
    }
  }
}
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Bolt Container Runtime                        │
│                                                                   │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │            Bolt MCP Server (Embedded Glyph)               │  │
│  │  • GPU Stats Tool                                         │  │
│  │  • Container Filesystem Tool                              │  │
│  │  • Shell Execution Tool                                   │  │
│  │  • Process Management Tool                                │  │
│  │  • Network Stats Tool                                     │  │
│  └──────────────────────────────────────────────────────────┘  │
│                           ↕ MCP Protocol                         │
└─────────────────────────────────────────────────────────────────┘
                             ↓
                    ┌────────────────┐
                    │   AI Client    │
                    │  (Claude, etc) │
                    └────────────────┘
```

### Gateway Mode Architecture

```
┌────────────────────────────────────────────────────────────┐
│                   MCP Gateway                               │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  Catalog  │  Tool Registry  │  Client Manager       │ │
│  │  Secret Store  │  Interceptors                       │ │
│  └──────────────────────────────────────────────────────┘ │
└─────────────┬──────────────────────────────────────────────┘
              │
              ├─────► Container 1 (MCP Server)
              ├─────► Container 2 (MCP Server)
              └─────► Container 3 (MCP Server)
```

---

## Features

### ✨ Core Features

- **Zero overhead when disabled** - Feature-gated compilation
- **Multiple transports** - stdio, WebSocket, HTTP/2
- **GPU-aware tools** - Native NVIDIA GPU monitoring via nvml-wrapper
- **Container-native** - Direct access to container internals
- **Security first** - Path validation, command allowlists, audit logging
- **Production-ready** - Comprehensive error handling and observability

### 🔧 Available Tools

| Tool | Description | Requires Consent |
|------|-------------|------------------|
| `bolt_gpu_stats` | GPU metrics (utilization, memory, temp, power) | No |
| `bolt_filesystem` | Read/write container files | Yes (write) |
| `bolt_shell_exec` | Execute shell commands | Yes |
| `bolt_process` | List and manage processes | No |
| `bolt_network_stats` | Network interface statistics | No |

---

## Usage

### Server Mode

**Basic Usage:**

```bash
# Start WebSocket server on default port (7331)
bolt mcp serve

# Start on custom port
bolt mcp serve --port 8080

# Use stdio for Claude Desktop
bolt mcp serve --transport stdio

# Bind to specific address
bolt mcp serve --address 127.0.0.1 --port 7331
```

**Container-Specific Server:**

```bash
# Expose tools for specific container
bolt mcp serve --container my-app
```

### Gateway Mode

**Start the Gateway:**

```bash
# Use the standalone binary
cargo run --package bolt-mcp --bin bolt-mcp-gateway

# Or via bolt CLI (shows instructions)
bolt mcp gateway

# With specific servers enabled
bolt mcp gateway --servers bolt-runtime,dev-tools

# With specific tools enabled
bolt mcp gateway --tools "bolt-runtime:*,dev-tools:shell"

# Custom catalog
bolt mcp gateway --catalog ~/.config/bolt/my-catalog.toml
```

**Gateway Features:**

- **Multi-container federation** - Manage tools from multiple containers
- **Centralized policy enforcement** - Single point of control
- **Tool registry** - Enable/disable tools dynamically
- **Secret management** - Docker Desktop + .env fallback
- **Request interceptors** - Logging, filtering, rate limiting

---

## Configuration

### Boltfile MCP Configuration

```toml
# Boltfile.toml
project = "my-project"

[services.app]
image = "my-app:latest"

# Enable MCP for this service
[services.app.mcp]
enabled = true
transport = "websocket"
port = 7331

# Policy configuration
[services.app.mcp.policy]
mode = "consent-required"  # "allow-all", "deny-all", "consent-required"
require_consent = ["shell.execute", "fs.write"]
audit_all = true
audit_log = "/var/log/bolt/mcp-audit.jsonl"
redact_secrets = true

# Tool-specific configuration
[services.app.mcp.tools.gpu_stats]
enabled = true

[services.app.mcp.tools.filesystem]
enabled = true
root = "/app"  # Restrict access to /app directory

[services.app.mcp.tools.shell]
enabled = true
allowed_commands = ["ls", "ps", "nvidia-smi", "cat", "grep"]

[services.app.mcp.tools.process]
enabled = true

[services.app.mcp.tools.network]
enabled = true

# Observability
[services.app.mcp.observability]
enable_metrics = true
metrics_port = 9090
enable_tracing = false
```

### MCP Catalog (TOML)

Location: `~/.config/bolt/mcp-catalog.toml`

```toml
[metadata]
name = "My MCP Catalog"
version = "1.0.0"
description = "Custom MCP server definitions"
author = "Your Name"

# Define a server
[[servers.bolt-runtime]]
name = "bolt-runtime"
server_type = "embedded"
description = "Bolt container runtime MCP server"
policy_mode = "consent-required"
enabled = true

[[servers.bolt-runtime.tools]]
name = "bolt_gpu_stats"
description = "Get GPU statistics"
enabled = true

[[servers.bolt-runtime.tools]]
name = "bolt_filesystem"
description = "Container filesystem access"
enabled = true

# Container-based server
[[servers.dev-tools]]
name = "dev-tools"
server_type = "container"
description = "Development tools MCP server"
image = "ghcr.io/my-org/dev-tools-mcp:latest"
command = ["serve", "--transport", "websocket"]
network_mode = "host"
enabled = true

[servers.dev-tools.env]
MCP_PORT = "7332"
LOG_LEVEL = "debug"

[servers.dev-tools.resources]
cpus = 1.0
memory = "2Gb"
```

---

## Available Tools

### 1. GPU Stats Tool

**Tool Name:** `bolt_gpu_stats`

**Description:** Get real-time GPU statistics for NVIDIA GPUs

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "device_id": {
      "type": "integer",
      "description": "GPU device ID (default: 0)",
      "default": 0
    }
  }
}
```

**Output:**
```json
{
  "device_id": 0,
  "name": "NVIDIA GeForce RTX 4090",
  "utilization_percent": 75.5,
  "memory_used_mb": 18432,
  "memory_total_mb": 24576,
  "temperature_celsius": 68,
  "power_draw_watts": 380.5,
  "clock_speed_mhz": 2520
}
```

**Requirements:** `nvidia-support` feature enabled

---

### 2. Filesystem Tool

**Tool Name:** `bolt_filesystem`

**Description:** Access container filesystem (read/write/list)

**Operations:**

**Read File:**
```json
{
  "operation": "read",
  "path": "/app/config.json"
}
```

**Write File:**
```json
{
  "operation": "write",
  "path": "/app/output.txt",
  "contents": "Hello, World!"
}
```

**List Directory:**
```json
{
  "operation": "list",
  "path": "/app"
}
```

**Security:**
- Path traversal prevention (cannot access `../../../etc/passwd`)
- Restricted to configured root directory
- Write operations require consent

---

### 3. Shell Execution Tool

**Tool Name:** `bolt_shell_exec`

**Description:** Execute shell commands in the container

**Input:**
```json
{
  "command": "nvidia-smi"
}
```

**Output:**
```json
{
  "stdout": "...",
  "stderr": "",
  "exit_code": 0,
  "success": true
}
```

**Security:**
- Command allowlist enforcement
- Audit logging for all executions
- Requires explicit consent

**Default Allowed Commands:**
- `ls`, `ps`, `nvidia-smi`, `cat`, `grep`

---

### 4. Process Management Tool

**Tool Name:** `bolt_process`

**Description:** List and monitor container processes

**Operations:**

**List Processes:**
```json
{
  "operation": "list"
}
```

**Get Process Count:**
```json
{
  "operation": "count"
}
```

**Output:**
```json
{
  "operation": "list",
  "count": 12,
  "processes": [
    {
      "user": "root",
      "pid": 1,
      "cpu_percent": 0.5,
      "mem_percent": 2.3,
      "vsz": 18432,
      "rss": 4096,
      "command": "/app/server"
    }
  ]
}
```

---

### 5. Network Stats Tool

**Tool Name:** `bolt_network_stats`

**Description:** Get network interface statistics

**Operations:**

**Get Interface Stats:**
```json
{
  "operation": "stats",
  "interface": "eth0"
}
```

**Get Summary:**
```json
{
  "operation": "summary"
}
```

**Output:**
```json
{
  "operation": "stats",
  "interfaces": [
    {
      "interface": "eth0",
      "rx_bytes": 123456789,
      "rx_packets": 9876543,
      "rx_errors": 0,
      "rx_dropped": 0,
      "tx_bytes": 987654321,
      "tx_packets": 7654321,
      "tx_errors": 0,
      "tx_dropped": 0
    }
  ]
}
```

---

## Examples

### Example 1: AI-Enhanced Development Container

```toml
[services.dev]
image = "ubuntu:latest"

[services.dev.mcp]
enabled = true
transport = "websocket"
port = 7331
tools = ["filesystem", "shell", "process"]

[services.dev.mcp.policy]
mode = "consent-required"
require_consent = ["shell.execute", "fs.write"]
```

**Use Cases:**
- Claude can read code files
- Execute builds and tests
- Monitor process resources
- List directory contents

---

### Example 2: Gaming Container with GPU Monitoring

```toml
[services.game]
image = "nvidia/cuda:12.3.0-base"

[services.game.gaming.gpu]
runtime = "nvbind"
isolation_level = "exclusive"

[services.game.mcp]
enabled = true
transport = "websocket"
tools = ["gpu_stats", "process"]

[services.game.mcp.policy]
mode = "allow-all"  # No consent needed for monitoring
```

**Use Cases:**
- Real-time GPU metrics for performance tuning
- Process monitoring during gameplay
- Temperature and power tracking

---

### Example 3: Multi-Container Workflow with Gateway

```toml
# Boltfile.toml
project = "ai-workflow"

[services.scraper]
image = "my-scraper:latest"

[services.scraper.mcp]
enabled = true
tools = ["http_fetch", "filesystem"]

[services.processor]
image = "my-processor:latest"

[services.processor.mcp]
enabled = true
tools = ["filesystem", "process"]

[services.analyzer]
image = "my-analyzer:latest"

[services.analyzer.mcp]
enabled = true
tools = ["shell", "filesystem"]
```

**Gateway Command:**
```bash
bolt mcp gateway \
  --servers scraper,processor,analyzer \
  --tools "scraper:*,processor:*,analyzer:*"
```

**Use Cases:**
- AI orchestrates data pipeline across containers
- Scraper fetches data → Processor transforms → Analyzer summarizes
- Single MCP client coordinates all three services

---

## Troubleshooting

### MCP Server Won't Start

**Issue:** `bolt mcp serve` fails to start

**Solutions:**
1. Check if port is already in use:
   ```bash
   lsof -i :7331
   ```

2. Try a different port:
   ```bash
   bolt mcp serve --port 8080
   ```

3. Enable verbose logging:
   ```bash
   bolt mcp serve --verbose
   ```

### Claude Desktop Connection Failed

**Issue:** Claude Desktop can't connect to MCP server

**Solutions:**
1. Verify correct transport:
   ```bash
   bolt mcp serve --transport stdio
   ```

2. Check Claude Desktop config location:
   - macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
   - Linux: `~/.config/Claude/claude_desktop_config.json`

3. Restart Claude Desktop after config changes

### GPU Stats Tool Returns Error

**Issue:** `bolt_gpu_stats` tool unavailable

**Solutions:**
1. Ensure `nvidia-support` feature is enabled:
   ```bash
   cargo build --features "mcp,nvidia-support" --release
   ```

2. Check NVIDIA drivers:
   ```bash
   nvidia-smi
   ```

3. Verify nvml-wrapper can initialize:
   ```bash
   # Should show GPU info
   bolt mcp serve --verbose
   ```

### Permission Denied for Shell/Filesystem Tools

**Issue:** Tool execution fails with permission error

**Solutions:**
1. Check policy mode in Boltfile:
   ```toml
   [mcp.policy]
   mode = "consent-required"
   ```

2. Verify tool is enabled:
   ```toml
   [mcp.tools.shell]
   enabled = true
   ```

3. For shell, check allowlist:
   ```toml
   [mcp.tools.shell]
   allowed_commands = ["ls", "ps", "your-command"]
   ```

---

## Performance Characteristics

### Overhead Benchmarks

| Mode | Latency Overhead | Memory Overhead | CPU Overhead |
|------|------------------|-----------------|--------------|
| Embedded | <50μs | ~5MB | <1% |
| Gateway | <1ms | ~50MB | <3% |

### Throughput

- **Tool calls/sec:** 10,000+ (embedded), 2,000+ (gateway)
- **Concurrent clients:** 1000+ per gateway
- **Max tools per server:** 100+

---

## Next Steps

- [Quick Start Guide](./quickstart.md)
- [Architecture Deep Dive](./architecture.md)
- [Phase 3: OMEN AI Integration](./phase3-omen.md) ⭐ NEW
- [Gateway Setup Guide](./gateway.md)
- [Tool Development Guide](./tools.md)
- [Security Best Practices](./security.md)
- [Integration Examples](./examples/)

---

## Roadmap

### ✅ Phase 1: Embedded MCP Server (Complete)
- Glyph integration
- 5 core tools (GPU, filesystem, shell, process, network)
- Multiple transports (stdio, WebSocket, HTTP)

### ✅ Phase 2: Gateway Architecture (Complete)
- Multi-server federation
- TOML-based catalog system
- Tool registry with enable/disable
- Secret store (Docker Desktop + .env)
- Interceptor middleware pipeline

### 🚀 Phase 3: OMEN AI Integration (Complete)
- Smart AI routing across 8+ providers
- Cost/latency/quality-aware selection
- Intent-based routing (code, tests, analysis)
- MCP tool: `bolt_omen_chat`
- OMEN provider adapter

### 📋 Phase 4: Rune Integration (Planned)
- Zig FFI bindings for ultra-fast operations
- Sub-millisecond MCP tool invocation
- SIMD-accelerated prompt processing
- >3× Rust baseline performance

### 🔮 Phase 5: Ghost Stack Full Integration (Planned)
- Zeke.nvim native support
- Jarvis agent runtime orchestration
- GhostFlow workflow nodes
- Autonomous container management

---

**Questions or issues?**
- GitHub: https://github.com/CK-Technology/bolt/issues
- Discord: https://discord.gg/ghoststack
- Email: ghostkellz@proton.me
