#!/usr/bin/env bash
# Run the Lua content walkthrough tests (the `content-test` runner). Extra args
# are forwarded to the CLI, e.g. `--filter Man0l0`. Run from anywhere; the
# binary resolves ./scripts/lua + ./scripts/tests relative to the repo root.
#
# Note: the SEQ_005 combat legs are slow because they drive the director's real
# wait(N) beats on a wall clock; the SEQ_000 tutorial and SEQ_010 handoff legs
# are fast and timing independent. Run only the fast legs with --filter SEQ_000.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_lib.sh
source "$SCRIPT_DIR/_lib.sh"

profile="${PROFILE:-release}"
if [ "$profile" = "release" ]; then
    flag="--release"
    bin="$REPO_ROOT/target/release/content-test"
else
    flag=""
    bin="$REPO_ROOT/target/debug/content-test"
fi

ensure_env

cd "$REPO_ROOT"
say "Building content-test ($profile)"
# shellcheck disable=SC2086
cargo build -p content-test $flag

say "Running content-test"
if [ ! -x "$bin" ]; then
    err "built binary not found at $bin"
    exit 1
fi
exec "$bin" "$@"
