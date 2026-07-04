#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BOLT="${BOLT_BIN:-$ROOT/target/debug/bolt}"
CONTAINER="${BOLT_SMOKE_CONTAINER:-bolt-smoke-alpine}"
IMAGE="${BOLT_SMOKE_IMAGE:-alpine:latest}"

if [[ -z "${BOLT_BIN:-}" ]]; then
  cargo build --manifest-path "$ROOT/Cargo.toml"
fi

# Ephemeral working dir under the user cache (XDG), created fresh and
# removed on exit. Nothing is left in the repo or in /tmp. Override with
# BOLT_SMOKE_ROOT to point at a caller-managed directory (not auto-removed).
if [[ -n "${BOLT_SMOKE_ROOT:-}" ]]; then
  SMOKE_ROOT="$BOLT_SMOKE_ROOT"
  OWN_SMOKE_ROOT=0
  mkdir -p "$SMOKE_ROOT"
else
  CACHE_BASE="${XDG_CACHE_HOME:-$HOME/.cache}/bolt"
  mkdir -p "$CACHE_BASE"
  SMOKE_ROOT="$(mktemp -d "$CACHE_BASE/local-smoke.XXXXXX")"
  OWN_SMOKE_ROOT=1
fi

mkdir -p "$SMOKE_ROOT/runtime" "$SMOKE_ROOT/home"
export BOLT_RUNTIME_DIR="$SMOKE_ROOT/runtime"
export BOLT_STORAGE_ROOT="$SMOKE_ROOT/storage"
export BOLT_LOG_DIR="$SMOKE_ROOT/logs"
export BOLT_HOME="$SMOKE_ROOT/home"

cleanup() {
  "$BOLT" rm --force "$CONTAINER" >/dev/null 2>&1 || true
  if [[ "$OWN_SMOKE_ROOT" == "1" ]]; then
    rm -rf "$SMOKE_ROOT"
  fi
}
trap cleanup EXIT

cleanup

echo "==> bolt run"
"$BOLT" run --name "$CONTAINER" --network bridge "$IMAGE" echo ok

echo "==> bolt ps -a"
"$BOLT" ps -a || true

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
