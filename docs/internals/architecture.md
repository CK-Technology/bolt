# Architecture

Bolt is split into runtime orchestration, storage, networking, GPU integration, and operational metadata. The native runtime owns container lifecycle and delegates focused work to subsystem managers.

```mermaid
flowchart LR
    CLI[CLI] --> UR[UnifiedRuntime]
    API[Rust API] --> UR
    UR --> NR[Native Runtime]
    UR --> DELEGATE[Docker/Podman delegation]
    NR --> OCI[OCI bundle/spec]
    NR --> STORAGE[Storage Manager]
    NR --> NET[Network Manager]
    NR --> GPU[GPU Integration]
    NR --> VOL[Volume Manager]
    NR --> STATE[Persistent State]
    STORAGE --> REG[OCI Registry Client]
    STORAGE --> ROOTFS[Rootfs Assembly]
    NET --> BRIDGE[Bridge/Host/None]
    NET --> QUIC[QUIC Fabric]
```

## Lifecycle

```mermaid
sequenceDiagram
    participant User
    participant CLI
    participant Runtime
    participant Storage
    participant Network
    participant OCI

    User->>CLI: bolt run image
    CLI->>Runtime: run_container
    Runtime->>Storage: pull or hydrate image
    Runtime->>Storage: create rootfs
    Runtime->>Network: allocate network mode
    Runtime->>OCI: write spec and start
    Runtime->>Runtime: persist state
    Runtime-->>CLI: container id
```

## Operational State

```mermaid
flowchart TB
    STATE[containers/*/state.json]
    SNAP[snapshots/*/snapshot.json]
    VOL[volumes/*/metadata.json]
    IMG[images/*/metadata.json]
    GC[GC]
    GEN[Generations]

    STATE --> VOL
    STATE --> GC
    SNAP --> GEN
    SNAP --> GC
    IMG --> SNAP
    IMG --> GC
```

Persistent metadata is intentionally plain JSON so operators can inspect state directly during recovery.
