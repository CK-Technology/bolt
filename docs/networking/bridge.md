# Bridge Networking

Bolt bridge networking keeps the familiar Linux bridge/veth model, but tracks
container attachments and IP allocation in runtime state so cleanup and stats can
reason about the actual bridge a container joined.

```mermaid
flowchart LR
    CLI["bolt run --network bridge"] --> Native["native runtime"]
    Native --> Spec["OCI spec<br/>network namespace"]
    Native --> Bridge["BridgeManager"]
    Bridge --> IPAM["IP allocation<br/>per network"]
    Bridge --> Veth["veth pair<br/>host ↔ container"]
    Bridge --> Stats["bridge stats<br/>per bridge"]
    Veth --> Container["container eth0"]
```

## Current Behavior

- `bridge` and `bolt` keep a private network namespace in the OCI spec.
- `host` omits the OCI network namespace.
- `none` configures loopback-only networking.
- `container:<id>` is rejected until namespace sharing is implemented.
- Bridge stats count interfaces attached to the requested bridge only.

## Safety Model

Bridge cleanup should be idempotent. The runtime records interface allocation so
disconnect and delete can remove only interfaces attached to the target bridge.
Future netlink work should keep that state model and replace logging fallbacks
with capability-gated netlink operations.

```mermaid
stateDiagram-v2
    [*] --> Planned
    Planned --> BridgeReady: create bridge
    BridgeReady --> Attached: connect container
    Attached --> Detached: stop/rm container
    Detached --> BridgeReady: release IP/veth
    BridgeReady --> Deleted: remove network
    Deleted --> [*]
```

## Verification

```bash
cargo test --lib networking::bridge
bolt run --network bridge -p 8080:80 nginx:latest
bolt ps -a
```
