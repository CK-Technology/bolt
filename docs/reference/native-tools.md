# Native Service Tools

Bolt has built-in service tools for automation workflows that would otherwise require a separate docker-mcp sidecar.

Tools are configured directly in `Boltfile.toml`:

```toml
[services.dev]
image = "rust:latest"
volumes = ["./:/workspace"]
working_dir = "/workspace"

[services.dev.tools]
enabled = true
allow = ["fs.read", "fs.write", "shell.exec", "gpu.stats", "process.list"]

[services.dev.tools.permissions]
filesystem_roots = ["/workspace"]
shell_commands = ["cargo", "git", "npm"]
gpu_access = "read_only"
network_access = "read_only"
process_access = "read_only"
```

## Commands

```bash
bolt tools list
bolt tools inspect dev
```

## Built-In Tools

| Tool | Scope | Description |
|------|-------|-------------|
| `fs.read` | `filesystem_roots` | Read files from allowed roots |
| `fs.write` | `filesystem_roots` | Write files under allowed roots |
| `fs.list` | `filesystem_roots` | List files under allowed roots |
| `fs.watch` | `filesystem_roots` | Watch allowed roots for changes |
| `shell.exec` | `shell_commands` | Execute allow-listed commands |
| `gpu.stats` | `gpu_access` | Read GPU utilization and memory metrics |
| `gpu.info` | `gpu_access` | Read GPU inventory and driver information |
| `process.list` | `process_access` | List service processes |
| `process.kill` | `process_access` | Terminate service processes when explicitly allowed |
| `network.stats` | `network_access` | Read service network counters |
| `network.connections` | `network_access` | List service network connections |
