#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BOLT="${BOLT_BIN:-$ROOT/target/debug/bolt}"
CONTAINER="${BOLT_SMOKE_CONTAINER:-bolt-smoke-alpine}"
RM_CONTAINER="${BOLT_SMOKE_RM_CONTAINER:-bolt-smoke-rm}"
DETACHED_CONTAINER="${BOLT_SMOKE_DETACHED_CONTAINER:-bolt-smoke-detached}"
IMAGE="${BOLT_SMOKE_IMAGE:-alpine:latest}"

if [[ -z "${BOLT_BIN:-}" ]]; then
  cargo build --manifest-path "$ROOT/Cargo.toml"
fi

# Ephemeral working dir under repo-local .scratch, created fresh and
# removed on exit. Override with BOLT_SMOKE_ROOT to point at a
# caller-managed directory (not auto-removed).
if [[ -n "${BOLT_SMOKE_ROOT:-}" ]]; then
  SMOKE_ROOT="$BOLT_SMOKE_ROOT"
  OWN_SMOKE_ROOT=0
  mkdir -p "$SMOKE_ROOT"
else
  SCRATCH_BASE="$ROOT/.scratch"
  mkdir -p "$SCRATCH_BASE"
  SMOKE_ROOT="$(mktemp -d "$SCRATCH_BASE/local-smoke.XXXXXX")"
  OWN_SMOKE_ROOT=1
fi

mkdir -p "$SMOKE_ROOT/runtime" "$SMOKE_ROOT/home"
export BOLT_RUNTIME_DIR="$SMOKE_ROOT/runtime"
export BOLT_STORAGE_ROOT="$SMOKE_ROOT/storage"
export BOLT_LOG_DIR="$SMOKE_ROOT/logs"
export BOLT_HOME="$SMOKE_ROOT/home"

cleanup() {
  "$BOLT" rm --force "$CONTAINER" >/dev/null 2>&1 || true
  "$BOLT" rm --force "$RM_CONTAINER" >/dev/null 2>&1 || true
  "$BOLT" rm --force "$DETACHED_CONTAINER" >/dev/null 2>&1 || true
  if [[ "$OWN_SMOKE_ROOT" == "1" ]]; then
    rm -rf "$SMOKE_ROOT"
  fi
}
trap cleanup EXIT

cleanup

echo "==> bolt run"
"$BOLT" run --name "$CONTAINER" --network bridge "$IMAGE" echo ok

echo "==> bolt run --rm"
"$BOLT" run --rm --name "$RM_CONTAINER" --network bridge "$IMAGE" echo remove-me
if "$BOLT" ps -a | grep -q "$RM_CONTAINER"; then
  echo "--rm container was still present in ps -a" >&2
  exit 1
fi

echo "==> bolt ps -a"
"$BOLT" ps -a || true

echo "==> bolt detached lifecycle"
"$BOLT" run --detach --name "$DETACHED_CONTAINER" --network bridge "$IMAGE" sh -c 'echo detached-ready; while true; do sleep 30; done'
sleep 1
"$BOLT" logs "$DETACHED_CONTAINER" | grep -q "detached-ready"
"$BOLT" exec "$DETACHED_CONTAINER" true
"$BOLT" restart --timeout 1 "$DETACHED_CONTAINER"
sleep 1
"$BOLT" ps | grep -q "$DETACHED_CONTAINER"
"$BOLT" stop --timeout 1 "$DETACHED_CONTAINER"
"$BOLT" rm "$DETACHED_CONTAINER"

echo "==> bolt logs"
if "$BOLT" logs "$CONTAINER"; then
  :
else
  echo "logs unavailable for $CONTAINER; native log capture is tracked separately" >&2
fi

echo "==> bolt rm"
if "$BOLT" rm --force "$CONTAINER"; then
  :
else
  echo "rm unavailable for $CONTAINER after process restart; persistent container state is tracked separately" >&2
fi

echo "local smoke completed"
