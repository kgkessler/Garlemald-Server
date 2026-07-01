# AGENTS.md — agent guide for Garlemald Server

Instructions for AI coding agents (OpenAI Codex and other `AGENTS.md`-aware tools)
working in this repository. **Claude Code / Claude Agent SDK: see
[`CLAUDE.md`](CLAUDE.md)** — it is the twin of this file. Keep the two in sync
when you change one.

Read this before you write code. The deeper reference docs are in `docs/`
(linked throughout).

---

## What this is

Garlemald Server is a **Rust port of a FINAL FANTASY XIV v1.23b server emulator**
(the original 2010 1.0 release, not A Realm Reborn) — a from-scratch port of the
C# [Project Meteor Server](https://bitbucket.org/Ioncannon/project-meteor-server/).
It is a single Cargo workspace (edition 2024, Rust 1.95) of four service binaries
plus a shared library, backed by SQLite, hosting ~1,142 Lua content scripts.

---

## Build, test, run

```sh
# Build everything
cargo build --workspace

# Run the full stack (build + launch all four binaries; logs in ./logs/)
./scripts/run-all.sh            # PROFILE=debug for a debug build

# Run one service
./scripts/run-{web,lobby,world,map}.sh

# Tests
cargo test --workspace
```

Each binary also runs standalone: `./target/release/<name> --config configs/<name>.toml`.
Ports (localhost defaults): web **54993**, lobby **54994**, world **54992**, map
**1989**. Full detail: [`docs/dev-environment.md`](docs/dev-environment.md).

First boot creates `./data/garlemald.db` from the bundled schema; no external DB
or web server is needed (SQLite is bundled, Lua 5.4 is vendored — a C toolchain
is required to compile them).

### Logging & packet capture

- `RUST_LOG=map_server=debug` (or `trace`, or per-crate filters) — `tracing` via
  `EnvFilter` (`common/src/logging.rs`). Default: our crates at `debug`,
  dependencies at `info`.
- `GARLEMALD_PACKET_LOG_DIR=<dir>` — hex-dump every packet to `<tag>-packets.log`;
  add `GARLEMALD_PACKET_LOG_PLAINTEXT=1` for pre-encryption bytes
  (`common/src/packet_log.rs`).

---

## Where the subsystems live

| Crate          | Owns                                                                        |
|----------------|-----------------------------------------------------------------------------|
| `common`       | Wire primitives (`packet.rs`, `subpacket.rs`), `blowfish.rs`, `bitstream.rs`, `packet_log.rs`, `logging.rs`, `db.rs` |
| `web-server`   | axum HTTP signup/login; session tokens (`server.rs`, `handlers.rs`, `session.rs`) |
| `lobby-server` | Character list/creation, world handoff (`processor.rs`, `database.rs`, `character_creator.rs`, `hardcoded.rs`) |
| `world-server` | Session ownership, party/group hierarchy, zone routing (`server.rs`, `world_master.rs`, `group.rs`, `database.rs`) |
| `map-server`   | Zone sim: `actor/`, `battle/`, `processor.rs`, `command_processor.rs`, `world_manager.rs`, and the Lua runtime in `lua/` + `runtime/` |

Architecture and the wire protocol: [`docs/architecture.md`](docs/architecture.md).
The Lua content system (engine, scheduler, hooks, the `LuaCommand` →
packet pipeline): [`docs/lua-runtime.md`](docs/lua-runtime.md).

> **Lua bindings:** before relying on a Lua engine binding, confirm it actually
> exists in `map-server/src/lua/`. The upstream C# server has server-side
> conveniences that are **not** real 1.x engine bindings — don't assume parity.

---

## The license header (REQUIRED on every new Rust file)

Every `.rs` file (including `build.rs`, examples, and tests) **must** open with the
AGPL header below, verbatim. Copy it from a sibling file rather than retyping it.
Markdown / TOML / SQL files do **not** carry it.

```rust
// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later
```

If you port code from a new upstream, also credit it in [`NOTICE.md`](NOTICE.md).
Verbatim translations from the GPL-3.0 LandSandBoat reference trigger the
combined-work rule — read `NOTICE.md` and prefer "read, re-derive, cite".

---

## Branching & PR flow

- Branch off **`develop`** (the default/integration branch); never commit to
  `develop` or `main` directly. PR **into `develop`**.
- `develop` → `main` is the release path; releases are tag-driven (see
  [`docs/RELEASING.md`](docs/RELEASING.md)). Don't bump versions by hand — the
  release workflow owns `Cargo.toml`'s version.
- Link the issue you're working from in the PR.

---

## The gates your work MUST pass (CI runs all six)

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --all-targets --locked
cargo test --workspace --locked
cargo run --locked -p content-test        # Lua quest walkthroughs (SEQ_000/005/010)
PROFILE=debug scripts/smoke-local.sh      # boot-smoke all four services
```

`--locked` means: if you change dependencies, commit the updated `Cargo.lock`.
`scripts/test.sh` bundles fmt + clippy + test + content-test; `scripts/smoke-local.sh`
is the boot smoke. **Do not open a PR until all six are green.**

---

## Hard rules

1. **AGPL header on every new `.rs` file** (above).
2. **Never cross-commit between repos.** This repo is one of several independent
   git repositories in a shared workspace (e.g. `Garlemald-Client`). Only ever
   commit here.
3. **Keep CI green** before raising a PR.
4. **Branch off `develop`, PR into `develop`.**
5. **Don't invent Lua engine bindings** — verify against `map-server/src/lua/`.
6. Keep this file and [`CLAUDE.md`](CLAUDE.md) **in sync**.

---

## Reference docs

- [`docs/architecture.md`](docs/architecture.md) — topology, per-binary roles, wire protocol.
- [`docs/lua-runtime.md`](docs/lua-runtime.md) — Lua engine, scheduler, hooks, command pipeline.
- [`docs/dev-environment.md`](docs/dev-environment.md) — build/run, logging, packet capture, save state.
- [`docs/agents.md`](docs/agents.md) — running an agent on an issue (human-facing).
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — the contribution workflow.
- [`docs/RELEASING.md`](docs/RELEASING.md) — branching & release automation.
- [`NOTICE.md`](NOTICE.md) — upstream attribution & licensing rules.
