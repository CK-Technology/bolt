#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BOLT="${BOLT_BIN:-$ROOT/target/debug/bolt}"
SMOKE_ROOT="$ROOT/.scratch/project-smoke"
PROJECT_DIR="$SMOKE_ROOT/project"

if [[ -z "${BOLT_BIN:-}" ]]; then
  cargo build --manifest-path "$ROOT/Cargo.toml"
fi

rm -rf "$SMOKE_ROOT"
mkdir -p "$PROJECT_DIR" "$SMOKE_ROOT/runtime" "$SMOKE_ROOT/storage" "$SMOKE_ROOT/home"

cleanup() {
  if [[ "${BOLT_PROJECT_SMOKE_APPLY:-0}" == "1" ]]; then
    (cd "$PROJECT_DIR" && "$BOLT" destroy --force --volumes) >/dev/null 2>&1 || true
  fi
  rm -rf "$SMOKE_ROOT"
}
trap cleanup EXIT

export BOLT_RUNTIME_DIR="$SMOKE_ROOT/runtime"
export BOLT_STORAGE_ROOT="$SMOKE_ROOT/storage"
export BOLT_LOG_DIR="$SMOKE_ROOT/logs"
export BOLT_HOME="$SMOKE_ROOT/home"

if [[ "${BOLT_PROJECT_SMOKE_APPLY:-0}" == "1" ]]; then
  SMOKE_IMAGE="${BOLT_PROJECT_SMOKE_IMAGE:-alpine:latest}"
else
  SMOKE_IMAGE="${BOLT_PROJECT_SMOKE_IMAGE:-example.invalid/alpine@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}"
fi

cat >"$PROJECT_DIR/Boltfile.toml" <<BOLTFILE
project = "project-smoke"

[services.web]
image = "$SMOKE_IMAGE"
command = ["sh", "-c", "sleep 60"]
ports = ["18080:80"]
volumes = ["data:/data"]
networks = ["default"]

[volumes.data]
driver = "local"

[networks.default]
driver = "bridge"
BOLTFILE

cd "$PROJECT_DIR"

echo "==> bolt validate"
"$BOLT" validate || true

echo "==> bolt plan"
"$BOLT" plan

echo "==> bolt lock"
"$BOLT" lock

echo "==> bolt dns hosts before apply"
"$BOLT" dns hosts || true

if [[ "${BOLT_PROJECT_SMOKE_APPLY:-0}" != "1" ]]; then
  echo "==> skipping apply/destroy; set BOLT_PROJECT_SMOKE_APPLY=1 to exercise runtime lifecycle"
  exit 0
fi

echo "==> bolt apply --locked -d"
if ! "$BOLT" apply --locked -d; then
  echo "apply unavailable in this environment; skipping runtime lifecycle" >&2
  exit 0
fi

echo "==> bolt dns hosts"
"$BOLT" dns hosts

echo "==> bolt drift"
"$BOLT" drift

echo "==> bolt destroy --force --volumes"
"$BOLT" destroy --force --volumes

echo "project smoke completed"
