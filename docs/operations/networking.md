# Networking

Bolt provides QUIC-based networking for container communication.

## Networks

### Create Network
```bash
bolt network create my-network --driver bolt --subnet 172.20.0.0/16
```

### List Networks
```bash
bolt network ls
```

### Remove Network
```bash
bolt network rm my-network
```

## Boltfile Configuration

```toml
[networks.frontend]
driver = "bolt"
subnet = "172.20.0.0/16"
gateway = "172.20.0.1"

[networks.backend]
driver = "bolt"
subnet = "172.21.0.0/16"
internal = true  # No external access

[services.web]
image = "nginx:latest"
networks = ["frontend"]

[services.api]
image = "myapi:latest"
networks = ["frontend", "backend"]

[services.db]
image = "postgres:16"
networks = ["backend"]
```

## Port Mapping

### CLI
```bash
bolt run -p 8080:80 nginx:latest
bolt run -p 127.0.0.1:3000:3000 myapp:latest
bolt run -p 8080:80 -p 8443:443 nginx:latest
```

### Boltfile
```toml
[services.web]
image = "nginx:latest"
ports = ["8080:80", "8443:443"]
```

## DNS Resolution

Services can reach each other by name within the same network:

```toml
[services.api]
environment = { DATABASE_URL = "postgres://db:5432/app" }

[services.db]
image = "postgres:16"
```

The `api` service can connect to `db` by hostname.

## QUIC Protocol

Bolt uses QUIC for container-to-container communication:

- Zero-RTT connection establishment
- Multiplexed streams
- Built-in encryption
- Connection migration

## Environment Variables

```bash
# Inside containers
BOLT_NETWORK=my-network
BOLT_HOSTNAME=my-service
BOLT_DNS_SERVER=172.20.0.1
```

## Troubleshooting

### Container Can't Reach Another
```bash
# Check both on same network
bolt network ls

# Verify DNS resolution
bolt exec container1 ping container2
```

### Port Already in Use
```bash
# Find process using port
lsof -i :8080

# Use different host port
bolt run -p 8081:80 nginx:latest
```
