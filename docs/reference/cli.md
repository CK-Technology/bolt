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
bolt surge up [-d|--detach]   # Start services from Boltfile.toml
bolt surge down               # Stop services
bolt surge status             # Show service status
bolt surge logs [--follow]    # View logs
```

### Boltfile Control Plane
```bash
bolt plan [--json]
bolt apply [-d|--detach] [--force-recreate] [--locked] [SERVICE...]
bolt destroy [--force] [--volumes] [SERVICE...]
bolt lock [--check]
bolt drift [--json]
bolt doctor [--json]
bolt validate [--json]
bolt self-test [--json]
bolt dns resolve <service|name.project.bolt>
bolt dns hosts
bolt dns serve [--bind 127.0.0.1:8053]
bolt import compose [-i docker-compose.yml] [-o Boltfile.toml]
bolt import container <ID|NAME> [--service <NAME>]
bolt import image <REF> [--service <NAME>]
bolt inspect <service|container|image|volume|network> <NAME|ID> [--json]
bolt completions <bash|zsh|fish>
bolt manpage
```

`bolt apply` uses Surge as the executor, then writes `Boltfile.lock`.
`bolt plan`, `bolt drift`, and `bolt doctor` are inspection commands.
`bolt apply` creates declared volumes and networks before services, honors
`depends_on` order, and writes `.bolt` service-discovery metadata under the Bolt
data directory.

### Native Images

```bash
bolt image list
bolt image inspect <REF>
bolt image pin <REF>
bolt image unpin <REF>
bolt image prune [--dry-run] [--force]
```

Pinned images are protected from native image garbage collection.

## Snapshots

```bash
bolt snapshot create --name <NAME> [--description <DESC>]
bolt snapshot list [--verbose]
bolt snapshot show <NAME>
bolt snapshot preflight
bolt snapshot rollback <NAME> --force
bolt snapshot delete <NAME> [--dry-run] --force
bolt snapshot cleanup [--dry-run] --force
```

## Generations

```bash
bolt generations list [--verbose]
```

## Networking

```bash
bolt network create <NAME> [--driver bolt] [--subnet <CIDR>]
bolt network ls
bolt network preflight
bolt network rm <NAME>
```

## Volumes

```bash
bolt volume create <NAME> [--size <SIZE>] [--opt mode=0750] [--opt uid=1000] [--opt gid=1000]
bolt volume ls
bolt volume rm <NAME>
bolt volume prune [--dry-run] [--force]
```

## Images

```bash
bolt pull <IMAGE>
bolt push <IMAGE>
bolt build [--tag <TAG>] [--file <DOCKERFILE>] <PATH>
bolt images
bolt image prune [--dry-run] [--force]
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
bolt snapshot rollback pre-update --force
```
