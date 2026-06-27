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

//! `content-test`: a standalone runner for **Lua-authored** quest-walkthrough
//! tests: a dedicated exe that drives the real content engine in-process so
//! quest/opener walkthroughs can be authored in Lua next to the content.
//!
//! It hosts a spec Lua VM that loads `describe`/`it` test files from
//! `scripts/tests/`, exposes a `walkthrough` bridge bound to the real
//! map-server [`map_server::lua::LuaEngine`] (a separate per-script VM), runs
//! every registered case, and reports pass/fail with a non-zero exit code on
//! any failure, so CI can gate on it.
//!
//! ```text
//! content-test [--script-root DIR] [--specs DIR] [--filter SUBSTR]
//! ```
//! Defaults resolve `./scripts/lua` and `./scripts/tests` (run from the repo
//! root, as the other `run-*.sh` scripts do), falling back to the workspace
//! paths baked in at build time so `cargo run -p content-test` works too.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use mlua::{Function, Lua, Table};

mod combat;
mod walkthrough;
use combat::CombatWalkthrough;
use walkthrough::Walkthrough;

#[derive(Parser)]
#[command(
    name = "content-test",
    about = "Garlemald Lua content walkthrough test runner"
)]
struct Args {
    /// Lua content scripts root (default: ./scripts/lua).
    #[arg(long)]
    script_root: Option<PathBuf>,
    /// Spec files root (default: ./scripts/tests).
    #[arg(long)]
    specs: Option<PathBuf>,
    /// Only run tests whose full name contains this substring.
    #[arg(long)]
    filter: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let script_root = resolve_root(args.script_root, "scripts/lua");
    let specs_root = resolve_root(args.specs, "scripts/tests");

    anyhow::ensure!(
        script_root.join("quests").is_dir(),
        "no quests/ under script root {}: pass --script-root",
        script_root.display(),
    );
    let framework = specs_root.join("framework").join("spec.lua");
    anyhow::ensure!(
        framework.is_file(),
        "spec framework not found at {}: pass --specs",
        framework.display(),
    );

    // One spec VM hosts the describe/it registry + both bridges. The Tier-2
    // combat bridge drives an async facade, so it blocks on a shared runtime.
    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .context("building the combat runtime")?,
    );
    let lua = Lua::new();
    register_walkthrough(&lua, &script_root)?;
    register_combat(&lua, rt, &script_root)?;
    load_chunk(&lua, &framework).context("loading the spec framework")?;

    let specs = discover_specs(&specs_root);
    for spec in &specs {
        load_chunk(&lua, spec).with_context(|| format!("loading spec {}", spec.display()))?;
    }

    let cases = collect_cases(&lua).context("reading the registered test cases")?;
    println!(
        "content-test: {} spec file(s), {} case(s)\n",
        specs.len(),
        cases.len()
    );

    let mut passed = 0usize;
    let mut failed = 0usize;
    for (name, body) in &cases {
        if let Some(filter) = &args.filter
            && !name.contains(filter.as_str())
        {
            continue;
        }
        match body.call::<()>(()) {
            Ok(()) => {
                passed += 1;
                println!("  ok    {name}");
            }
            Err(e) => {
                failed += 1;
                println!("  FAIL  {name}");
                for line in clean_error(&e.to_string()).lines() {
                    println!("          {line}");
                }
            }
        }
    }

    println!(
        "\n{passed} passed, {failed} failed, {} total",
        passed + failed
    );
    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// CLI override wins; otherwise prefer `./REL` (run from repo root), then the
/// workspace path baked in at build time (so `cargo run` works anywhere).
fn resolve_root(cli: Option<PathBuf>, rel: &str) -> PathBuf {
    if let Some(p) = cli {
        return p;
    }
    let cwd_rel = PathBuf::from(rel);
    if cwd_rel.exists() {
        return cwd_rel;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|workspace| workspace.join(rel))
        .unwrap_or(cwd_rel)
}

/// Install the `walkthrough` global, bound to the content scripts root so each
/// call spins up its own isolated engine. `walkthrough.start("Man0l0")` starts
/// at SEQ_000. An optional second arg seeds the sequence, e.g.
/// `walkthrough.start("Man0l0", 10)` to walk the post-combat hand-off leg.
fn register_walkthrough(lua: &Lua, script_root: &Path) -> anyhow::Result<()> {
    let table = lua.create_table()?;
    let root = script_root.to_path_buf();
    let start = lua.create_function(move |lua, (code, seq): (String, Option<u32>)| {
        let w =
            Walkthrough::new(&root, &code, seq.unwrap_or(0)).map_err(mlua::Error::RuntimeError)?;
        lua.create_userdata(w)
    })?;
    table.set("start", start)?;
    lua.globals().set("walkthrough", table)?;
    Ok(())
}

/// Install the `combat` global (`combat.start("Man0g0")`): the Tier-2 SEQ_005
/// force-kill bridge. Each `start` stands up a fresh facade (its own world +
/// registry + db), blocking on the shared runtime.
fn register_combat(
    lua: &Lua,
    rt: Arc<tokio::runtime::Runtime>,
    script_root: &Path,
) -> anyhow::Result<()> {
    let table = lua.create_table()?;
    let root = script_root.to_path_buf();
    let start = lua.create_function(move |lua, code: String| {
        let w = CombatWalkthrough::start(rt.clone(), &root, &code)
            .map_err(mlua::Error::RuntimeError)?;
        lua.create_userdata(w)
    })?;
    table.set("start", start)?;
    lua.globals().set("combat", table)?;
    Ok(())
}

fn load_chunk(lua: &Lua, path: &Path) -> anyhow::Result<()> {
    let src =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let name = format!("@{}", path.display());
    lua.load(&src).set_name(name.as_str()).exec()?;
    Ok(())
}

/// Pull `__SPEC.tests` (a `{ name, body }` array the framework builds) into a
/// Rust vector of `(name, function)`.
fn collect_cases(lua: &Lua) -> anyhow::Result<Vec<(String, Function)>> {
    let spec: Table = lua.globals().get("__SPEC")?;
    let tests: Table = spec.get("tests")?;
    let mut cases = Vec::new();
    for case in tests.sequence_values::<Table>() {
        let case = case?;
        let name: String = case.get("name")?;
        let body: Function = case.get("body")?;
        cases.push((name, body));
    }
    Ok(cases)
}

/// Recursively collect `*.lua` spec files, skipping the `framework/` dir
/// (loaded explicitly). Sorted for deterministic run order.
fn discover_specs(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            if p.is_dir() {
                if p.file_name().and_then(|n| n.to_str()) == Some("framework") {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().and_then(|s| s.to_str()) == Some("lua") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out
}

/// Trim mlua's `runtime error:` prefix and the trailing Lua stack traceback so
/// the failure line shows just the assertion message.
fn clean_error(message: &str) -> String {
    let body = message.strip_prefix("runtime error: ").unwrap_or(message);
    match body.find("stack traceback:") {
        Some(i) => body[..i].trim_end().to_string(),
        None => body.trim_end().to_string(),
    }
}
