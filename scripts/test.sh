#!/usr/bin/env bash
# Run the full verification suite: cargo test, clippy (warnings = error),
# and rustfmt check. Individual stages can be skipped via flags.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_lib.sh
source "$SCRIPT_DIR/_lib.sh"

run_tests=1
run_clippy=1
run_fmt=1
run_content=1
for a in "$@"; do
    case "$a" in
        --no-test)    run_tests=0 ;;
        --no-clippy)  run_clippy=0 ;;
        --no-fmt)     run_fmt=0 ;;
        --no-content) run_content=0 ;;
        --only-test)  run_clippy=0; run_fmt=0; run_content=0 ;;
        -h|--help)
            cat <<EOF
Usage: $(basename "$0") [--no-test|--no-clippy|--no-fmt|--no-content|--only-test]

Runs the full verification pipeline:
  1. cargo test --workspace
  2. content-test (Lua quest walkthrough specs)
  3. cargo clippy --workspace --all-targets -- -D warnings
  4. cargo fmt --all -- --check
EOF
            exit 0
            ;;
        *)
            err "unknown arg: $a"
            exit 2
            ;;
    esac
done

ensure_env

cd "$REPO_ROOT"

if [ "$run_tests" -eq 1 ]; then
    say "cargo test --workspace"
    cargo test --workspace
    ok "tests passed"
fi

if [ "$run_content" -eq 1 ]; then
    say "content-test (Lua quest walkthroughs)"
    cargo run -q -p content-test
    ok "content walkthroughs passed"
fi

if [ "$run_clippy" -eq 1 ]; then
    say "cargo clippy (warnings = errors)"
    cargo clippy --workspace --all-targets -- -D warnings
    ok "clippy clean"
fi

if [ "$run_fmt" -eq 1 ]; then
    say "cargo fmt --check"
    cargo fmt --all -- --check
    ok "formatting clean"
fi

ok "all verification stages passed"
