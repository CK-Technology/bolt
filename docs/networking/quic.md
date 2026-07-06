# QUIC Networking

Bolt has two QUIC-facing layers:

- `networking::quic_real` uses Quinn for real QUIC endpoints, pooled
  connections, RTT reporting, and deterministic container/network IP allocation.
- `networking::quic_proxy` handles socket proxy rules, health checks, and
  traffic shaping metadata.

```mermaid
flowchart TD
    Service["container service"] --> Proxy["QUICSocketProxy"]
    Proxy --> Rule["ProxyRule<br/>listen → target"]
    Rule --> Health["health check"]
    Rule --> Stats["proxy stats"]
    Service --> Real["RealQUICServer"]
    Real --> Pool["connection pool"]
    Pool --> RTT["real RTT from Quinn connection"]
```

## Health Checks

TCP, UDP, and HTTP checks are implemented. Unsupported ICMP and custom checks
fail closed: they return unhealthy instead of assuming the target is healthy.

```mermaid
flowchart LR
    Check["health check"] --> Kind{method}
    Kind --> TCP["TCP connect"]
    Kind --> UDP["UDP send"]
    Kind --> HTTP["HTTP status"]
    Kind --> ICMP["ICMP unsupported<br/>unhealthy"]
    Kind --> Custom["custom unsupported<br/>unhealthy"]
```

## Current Limits

- Service discovery is still local runtime state, not a distributed catalog.
- QUIC port forwarding exists as tracked rules, but full multi-host routing is
  not a Phase D exit criterion yet.
- Backpressure and reconnect policy should be promoted before QUIC is described
  as production multi-host networking.
