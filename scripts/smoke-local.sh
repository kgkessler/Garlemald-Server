#!/usr/bin/env bash
# smoke-local.sh — CI/dev fail-fast boot check.
#
# Builds the workspace, then boots each garlemald service in `--smoke` mode
# against a throwaway SQLite database and asserts it prints a matching SMOKE_OK
# line and exits 0. Any non-zero exit or missing marker fails the whole run.
#
# Usage:   scripts/smoke-local.sh
# Profile: PROFILE=debug scripts/smoke-local.sh   (default: release)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

PROFILE="${PROFILE:-release}"
if [ "$PROFILE" = "release" ]; then
    BUILD_FLAGS=(--release)
    BIN_DIR="$REPO_ROOT/target/release"
else
    BUILD_FLAGS=()
    BIN_DIR="$REPO_ROOT/target/debug"
fi

echo "==> building workspace ($PROFILE)"
cargo build --workspace "${BUILD_FLAGS[@]}"

TMP_DIR="$(mktemp -d)"
cleanup() { rm -rf "$TMP_DIR"; }
trap cleanup EXIT

# Each service boots against its own fresh DB so a failure is isolated.
SERVICES=(web lobby world map)
FAILURES=0

for name in "${SERVICES[@]}"; do
    bin="$BIN_DIR/${name}-server"
    cfg="$REPO_ROOT/configs/${name}.toml"
    db="$TMP_DIR/${name}.db"

    if [ ! -x "$bin" ]; then
        echo "SMOKE_FAIL ${name} driver: binary not found at ${bin}"
        FAILURES=$((FAILURES + 1))
        continue
    fi

    echo "==> smoke ${name} (${bin})"
    set +e
    output="$("$bin" --smoke --config "$cfg" --db-path "$db" 2>&1)"
    code=$?
    set -e
    echo "$output"

    if [ "$code" -eq 0 ] && printf '%s\n' "$output" | grep -q '^SMOKE_OK '; then
        echo "==> ${name} OK (exit 0, SMOKE_OK seen)"
    else
        echo "SMOKE_FAIL ${name} driver: exit=${code} (expected 0 + a SMOKE_OK line)"
        FAILURES=$((FAILURES + 1))
    fi
done

if [ "$FAILURES" -ne 0 ]; then
    echo "SMOKE_FAIL ${FAILURES} service(s) failed"
    exit 1
fi

echo "SMOKE_OK all services"
