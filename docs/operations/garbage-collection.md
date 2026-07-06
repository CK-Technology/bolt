# Garbage Collection

Native GC is rooted in persisted runtime state. `bolt image prune` now reports
unused image cache entries and stale runtime roots, with dry-run as the default
unless `--force` is passed.

```mermaid
flowchart TD
    Prune["bolt image prune"] --> Runtime["UnifiedRuntime"]
    Runtime --> Native["native runtime"]
    Native --> Protect["protect live container image refs<br/>and container IDs"]
    Native --> Storage["StorageManager.prune_images"]
    Storage --> Images["unused image dirs"]
    Storage --> StaleImages["stale image dirs without metadata"]
    Storage --> Roots["stale container bundles"]
    Images --> Report["dry-run/report"]
    StaleImages --> Report
    Roots --> Report
```

## Commands

```bash
bolt image inspect alpine:latest
bolt image pin alpine:latest
bolt image unpin alpine:latest

# Preview only
bolt image prune --dry-run

# Also preview, because --force is absent
bolt image prune

# Remove unreferenced native image/runtime roots
bolt image prune --force
```

## Protected Roots

- Images referenced by persisted containers are protected.
- Images pinned with `bolt image pin` are protected.
- Image digests referenced by snapshot generations are protected.
- Container bundle directories whose IDs are present in runtime state are
  protected.
- Container IDs referenced by snapshot generations are protected.

## Output Shape

The prune report separates image candidates from runtime-root candidates and
includes byte counts for both categories. It also explains image references
protected by pins, live containers, or generation digests.
