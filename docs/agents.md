# Working an issue with an AI coding agent

Garlemald Server is built to be contributed to with **AI coding agents** —
[Claude Code](https://claude.com/claude-code) / the Claude Agent SDK, and
[OpenAI Codex](https://openai.com/codex/) / Codex CLI. This page explains how to
point one at an issue and get a green PR out the other end.

The agent's in-repo instructions live in two files at the repo root:

- **[`CLAUDE.md`](../CLAUDE.md)** — read by Claude Code and the Claude Agent SDK.
- **[`AGENTS.md`](../AGENTS.md)** — read by OpenAI Codex and other `AGENTS.md`-aware
  tools.

Both files capture the same essentials — how to build/test/run, where the
subsystems live, the AGPL header rule, the `develop` → PR flow, and the
`fmt`/`clippy`/`build`/`test` gates — so whichever agent you use already knows the
house rules. **Keep the two files in sync** when you change one.

---

## Prerequisites

You need a working dev environment (see [`dev-environment.md`](dev-environment.md))
plus one agent toolchain:

### Claude

- **Claude Code (CLI):** install per the
  [Claude Code docs](https://docs.claude.com/en/docs/claude-code), then run
  `claude` inside your checkout. Authenticate with a Claude.ai (Pro/Max) login or
  an Anthropic API key (`ANTHROPIC_API_KEY`).
- **claude.ai/code (web):** point it at your fork/branch.
- **Claude Agent SDK:** for scripted runs — needs `ANTHROPIC_API_KEY`.

### OpenAI

- **Codex CLI / Codex:** install per the
  [Codex docs](https://developers.openai.com/codex/), then run `codex` inside your
  checkout. Authenticate with a ChatGPT login or `OPENAI_API_KEY`.

The agent reads `CLAUDE.md` / `AGENTS.md` automatically on startup — you don't
paste them in.

---

## The loop

1. **Fork & clone.** Fork `swstegall/Garlemald-Server`, clone your fork, and add
   the upstream remote:

   ```sh
   git clone git@github.com:<you>/Garlemald-Server.git
   cd Garlemald-Server
   git remote add upstream https://github.com/swstegall/Garlemald-Server.git
   ```

2. **Branch off `develop`.** Never work on `develop`/`main` directly:

   ```sh
   git fetch upstream
   git switch -c fix/<short-slug> upstream/develop
   ```

3. **Hand the agent the issue.** Start the agent in the checkout and give it the
   issue as its task — a link or the pasted issue body is enough. It reads
   `CLAUDE.md` / `AGENTS.md` for build/test/layout/conventions and gets to work.
   A good first instruction:

   > Implement issue #NN. Read CLAUDE.md (or AGENTS.md) first. Build and test
   > before you finish, and make sure `cargo fmt`, `cargo clippy -- -D warnings`,
   > and `cargo test` all pass.

4. **Let it build + test.** The agent should iterate until the six gates pass
   (see below). Review its diff yourself — you are responsible for the PR.

5. **Open a PR into upstream `develop`.** Push your branch to your fork and open a
   PR targeting `swstegall/Garlemald-Server:develop`, linking the issue. CI runs
   the gates; keep them green.

See [`../CONTRIBUTING.md`](../CONTRIBUTING.md) for the full contributor workflow
(collaborator/board access, issue selection, PR etiquette).

---

## The gates the agent must satisfy

A PR cannot merge unless CI is green. These are the exact commands (also in
`.github/workflows/ci.yml`); have the agent run them before finishing:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --all-targets --locked
cargo test --workspace --locked
cargo run --locked -p content-test        # Lua quest walkthroughs (SEQ_000/005/010)
PROFILE=debug scripts/smoke-local.sh      # boot-smoke all four services
```

`--locked` means a dependency change must be reflected in a committed `Cargo.lock`
update.

---

## Guardrails

These are non-negotiable; they're spelled out in `CLAUDE.md` / `AGENTS.md` so the
agent already knows them, but you should verify the agent honoured them:

- **AGPL license header on every new Rust source file.** Every `.rs` file (and
  `build.rs`) opens with the project's GNU AGPL v3 boilerplate and the
  `// SPDX-License-Identifier: AGPL-3.0-or-later` trailer. Copy the header
  verbatim from an existing sibling file. Markdown docs do **not** carry the
  header. If the change ports code from a new upstream, extend `NOTICE.md`.
- **Never cross-commit between repos.** Garlemald Server lives in a multi-repo
  workspace alongside `Garlemald-Client` and others. Each is an independent git
  repository — an agent must only commit to this one.
- **Keep CI green before raising a PR.** No PR with a failing gate.
- **Don't invent engine bindings.** When touching the Lua runtime, confirm a
  binding really exists in the engine before relying on it — the upstream C#
  server has server-side conveniences that are not real 1.x engine bindings (see
  [`lua-runtime.md`](lua-runtime.md)).
- **Respect the branching model.** Branch off `develop`; PR into `develop`. See
  [`RELEASING.md`](RELEASING.md).

---

## Where to read next

- [`../CLAUDE.md`](../CLAUDE.md) / [`../AGENTS.md`](../AGENTS.md) — what the agent reads.
- [`dev-environment.md`](dev-environment.md) — build, run, log, reset state.
- [`architecture.md`](architecture.md) and [`lua-runtime.md`](lua-runtime.md) — the codebase map.
- [`../CONTRIBUTING.md`](../CONTRIBUTING.md) — the human contribution workflow.
