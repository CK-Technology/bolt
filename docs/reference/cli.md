# CLI Reference

## Container Commands

### bolt run
```bash
bolt run [OPTIONS] <IMAGE> [COMMAND]

Options:
  -n, --name <NAME>       Container name
  -p, --ports <PORTS>     Port mappings (host:container)
  -e, --env <ENV>         Environment variables
  -v, --volumes <VOL>     Volume mounts (host:container)
  -d, --detach            Run in background
  --gpu <DEVICES>         GPU devices (all, 0, 1,2)
  --gpu-profile <NAME>    GPU profile (gaming or AI)
  --runtime <RUNTIME>     GPU runtime (nvbind, docker)
  -i, --interactive       Keep STDIN open
  -t, --tty               Allocate pseudo-TTY
  --rm                    Remove on exit
```

### bolt ps
```bash
bolt ps [OPTIONS]

Options:
  -a, --all      Show all containers
  -q, --quiet    Only show IDs
```

### bolt stop / rm / restart
```bash
bolt stop <CONTAINER>...
bolt rm <CONTAINER>... [--force]
bolt restart <CONTAINER>... [--timeout <SECS>]
```

## GPU Commands

### bolt nv (NVIDIA)
```bash
bolt nv info [--format json] [--detailed]
bolt nv doctor [--fix]
bolt nv driver
bolt nv arch [--gpu <N>]

# Profiles
bolt nv profile list [--profile-type gaming|ai|all]
bolt nv profile show <NAME>
bolt nv profile apply <NAME> [--output <FILE>]

# CDI
bolt nv cdi generate [--output <FILE>] [--profile gaming|aiml]
bolt nv cdi list
bolt nv cdi validate <FILE>
```

### bolt amd (AMD)
```bash
bolt amd info [--format json]
bolt amd doctor
bolt amd rocm status
bolt amd rocm info
bolt amd cdi generate [--profile gaming|aiml]
```

### bolt arc (Intel)
```bash
bolt arc info [--format json]
bolt arc doctor
bolt arc oneapi status
bolt arc oneapi level-zero
bolt arc cdi generate
```

## Native Service Tools

```bash
bolt tools list
bolt tools inspect <SERVICE>
```

## Orchestration

### bolt surge
```bash
bolt surge up [--detach]      # Start services from Boltfile.toml
bolt surge down               # Stop services
bolt surge status             # Show service status
bolt surge logs [--follow]    # View logs
```

## Snapshots

```bash
bolt snapshot create --name <NAME> [--description <DESC>]
bolt snapshot list [--verbose]
bolt snapshot rollback <NAME>
bolt snapshot delete <NAME>
bolt snapshot cleanup [--dry-run]
```

## Networking

```bash
bolt network create <NAME> [--driver bolt] [--subnet <CIDR>]
bolt network ls
bolt network rm <NAME>
```

## Volumes

```bash
bolt volume create <NAME> [--size <SIZE>]
bolt volume ls
bolt volume rm <NAME>
bolt volume prune
```

## Images

```bash
bolt pull <IMAGE>
bolt push <IMAGE>
bolt build [--tag <TAG>] [--file <DOCKERFILE>] <PATH>
bolt images
```

## Examples

```bash
# Gaming with profile
bolt run --gpu all --gpu-profile "cyberpunk 2077" steam-image

# AI inference
bolt run --gpu all --gpu-profile ollama-medium ollama/ollama

# Development stack
bolt surge up -d
bolt surge logs api --follow

# Inspect built-in tools enabled for a service
bolt tools inspect api

# Snapshot before update
bolt snapshot create --name "pre-update"
apt update && apt upgrade -y
# If something breaks:
bolt snapshot rollback pre-update
```
