# Bolt MCP Quick Start

Get up and running with Bolt MCP in 5 minutes.

## Prerequisites

- Rust 1.85+ (edition 2024)
- Bolt container runtime
- (Optional) NVIDIA GPU with drivers for GPU tools

## Installation

### Build from Source

```bash
cd /data/projects/bolt

# Build with MCP support
cargo build --features mcp --release

# With NVIDIA GPU support
cargo build --features "mcp,nvidia-support" --release

# Binary location
./target/x86_64-unknown-linux-gnu/release/bolt
```

### Verify Installation

```bash
bolt mcp --help
```

Expected output:
```
MCP (Model Context Protocol) server commands

Usage: bolt mcp <COMMAND>

Commands:
  serve    Start MCP server
  gateway  Run MCP gateway (centralized management)
  help     Print this message or the help of the given subcommand(s)
```

## Quick Start: Server Mode

### 1. Start the MCP Server

```bash
# WebSocket on default port 7331
bolt mcp serve
```

You should see:
```
🚀 Bolt starting up...
🤖 Starting Bolt MCP server
   Transport: websocket
   📡 WebSocket server: ws://0.0.0.0:7331

💡 Connect via WebSocket:
   ws://0.0.0.0:7331

🛠️  Available tools:
   • bolt_gpu_stats - GPU metrics and monitoring
   • bolt_filesystem - Container filesystem access
   • bolt_shell_exec - Execute shell commands
   • bolt_process - Process management
   • bolt_network_stats - Network statistics

   Press Ctrl+C to stop
```

### 2. Test with curl (WebSocket)

In another terminal:

```bash
# Install websocat if not already installed
cargo install websocat

# Connect and send a test message
echo '{"jsonrpc":"2.0","method":"tools/list","id":1}' | \
  websocat ws://localhost:7331
```

## Quick Start: Claude Desktop

### 1. Start Server with stdio Transport

```bash
bolt mcp serve --transport stdio
```

### 2. Configure Claude Desktop

**macOS:**
Edit `~/Library/Application Support/Claude/claude_desktop_config.json`:

**Linux:**
Edit `~/.config/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "bolt": {
      "command": "/data/projects/bolt/target/x86_64-unknown-linux-gnu/release/bolt",
      "args": ["mcp", "serve", "--transport", "stdio"]
    }
  }
}
```

**Note:** Use the full path to the bolt binary.

### 3. Restart Claude Desktop

Close and reopen Claude Desktop.

### 4. Test in Claude

In Claude Desktop, try:

> "Can you list the available MCP tools?"

Claude should respond with the list of Bolt MCP tools.

## Quick Start: Gateway Mode

### 1. Create a Catalog

Create `~/.config/bolt/mcp-catalog.toml`:

```toml
[metadata]
name = "My Bolt Catalog"
version = "1.0.0"

[[servers.bolt-runtime]]
name = "bolt-runtime"
server_type = "embedded"
description = "Bolt container runtime"
enabled = true

[[servers.bolt-runtime.tools]]
name = "bolt_gpu_stats"
enabled = true

[[servers.bolt-runtime.tools]]
name = "bolt_filesystem"
enabled = true
```

### 2. Run the Gateway

```bash
cargo run --package bolt-mcp --bin bolt-mcp-gateway \
  --catalog ~/.config/bolt/mcp-catalog.toml
```

## Next Steps

- [Full Documentation](./README.md)
- [Configuration Guide](./configuration.md)
- [Tool Reference](./tools.md)
- [Examples](./examples.md)

## Troubleshooting

### Port Already in Use

```bash
# Use a different port
bolt mcp serve --port 8080
```

### Permission Denied

```bash
# Check file permissions
ls -la ~/.config/bolt/

# Create directory if needed
mkdir -p ~/.config/bolt
```

### MCP Not Found

```bash
# Verify MCP feature was compiled
bolt --version

# Should show feature flags
```

## Example Session

```bash
# Terminal 1: Start MCP server
bolt mcp serve --verbose

# Terminal 2: Test connection
echo '{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}' | \
  websocat ws://localhost:7331

# Terminal 3: Monitor logs
tail -f ~/.bolt/logs/mcp.log
```

That's it! You're now running Bolt MCP. 🎉
