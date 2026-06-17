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

//! SQLite connection helper shared by every Garlemald server.
//!
//! `open_or_create` is the canonical entry point: it creates the parent
//! directory, opens a `tokio_rusqlite::Connection`, applies the bundled
//! schema when the file is fresh, applies any new seed migrations, and
//! sets WAL + foreign-key pragmas.
//!
//! `ConnCallExt::call_db` is the shape every `database.rs` queue uses to
//! dispatch blocking rusqlite work — it pins `E = rusqlite::Error` so the
//! inline closures don't have to annotate the error type on every `Ok(..)`.

use std::future::Future;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rusqlite::{TransactionBehavior, named_params};
use tokio_rusqlite::Connection;

use crate::migrations;

/// Embedded schema (the `CREATE TABLE IF NOT EXISTS` set needed by all
/// servers). Applied once when the database file is first created.
pub const SCHEMA_SQL: &str = include_str!("../sql/schema.sql");

/// Ported MySQL -> SQLite data dumps from `project-meteor-mirror` are
/// tracked here. Each migration is keyed by filename; the runner records
/// applied names in `schema_migrations` so upgrading existing databases
/// picks up only the new files on next boot.
const SCHEMA_MIGRATIONS_DDL: &str = r#"
    CREATE TABLE IF NOT EXISTS schema_migrations (
        name       TEXT PRIMARY KEY,
        applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
"#;

/// Open (and initialise, if fresh) the SQLite database at `path`.
///
/// Steps:
/// 1. Ensure the parent directory exists.
/// 2. Open an async rusqlite connection and set WAL + foreign keys.
/// 3. If the file was just created, execute `SCHEMA_SQL` to lay down
///    every table the server code expects.
/// 4. Apply any bundled seed migrations that haven't run yet (tracked
///    in `schema_migrations`).
pub async fn open_or_create(path: impl AsRef<Path>) -> Result<Connection> {
    let path = path.as_ref().to_path_buf();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating db dir {}", parent.display()))?;
    }

    let fresh = !path.exists();
    let conn = Connection::open(path.clone())
        .await
        .with_context(|| format!("opening sqlite {}", path.display()))?;

    conn.call(|c| {
        c.pragma_update(None, "journal_mode", "WAL")?;
        c.pragma_update(None, "foreign_keys", "ON")?;
        c.pragma_update(None, "synchronous", "NORMAL")?;
        // 60s busy window lets concurrent server processes wait on each
        // other during the startup migration pass instead of failing fast.
        c.busy_timeout(Duration::from_secs(60))?;
        Ok::<(), rusqlite::Error>(())
    })
    .await
    .context("setting sqlite pragmas")?;

    if fresh {
        tracing::info!(path = %path.display(), "initialising fresh sqlite database");
        conn.call(|c| {
            c.execute_batch(SCHEMA_SQL)?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .context("applying schema.sql")?;
    }

    apply_migrations(&conn).await?;

    Ok(conn)
}

/// Apply every bundled migration that isn't already recorded in
/// `schema_migrations`. The whole pass runs under one `BEGIN IMMEDIATE`
/// transaction so that the 4 Garlemald servers racing on the same SQLite
/// file at boot serialise on the writer lock instead of double-applying a
/// migration (which produced `UNIQUE constraint failed` on
/// `schema_migrations.name`) or failing fast with `database is locked`.
/// The applied-set is re-read *inside* the lock, so once one process
/// finishes the others observe the new rows and no-op the pass.
pub async fn apply_migrations(conn: &Connection) -> Result<()> {
    conn.call(|c| {
        c.execute_batch(SCHEMA_MIGRATIONS_DDL)?;
        Ok::<(), rusqlite::Error>(())
    })
    .await
    .context("creating schema_migrations table")?;

    let total = migrations::count();
    let migrations: Vec<(String, String)> = migrations::iter()
        .map(|m| (m.name.to_string(), m.sql))
        .collect();

    let applied_now = conn
        .call(move |c| {
            let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;

            let applied: std::collections::HashSet<String> = {
                let mut stmt = tx.prepare("SELECT name FROM schema_migrations")?;
                stmt.query_map([], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<_>>()?
            };

            let mut count = 0usize;
            for (name, sql) in &migrations {
                if applied.contains(name) {
                    continue;
                }
                let started = Instant::now();
                match tx.execute_batch(sql) {
                    Ok(()) => {}
                    // The migration's schema change is already present — e.g. an
                    // `ALTER TABLE ... ADD COLUMN` whose column exists because a
                    // prior run applied it but the process died (or it was added
                    // out-of-band) before the name was recorded in
                    // `schema_migrations`. SQLite `ALTER` has no `IF NOT EXISTS`,
                    // so a naive re-run aborts the whole batch and bricks startup
                    // (every server that opens the shared DB dies ~1s in). Treat
                    // "duplicate column name" as success: record the migration
                    // and continue so a partially-migrated DB self-heals. Any
                    // OTHER error still propagates and fails the boot loudly.
                    Err(e) if is_already_satisfied(&e) => {
                        tracing::warn!(
                            migration = %name,
                            error = %e,
                            "migration's schema change already present; recording as applied and continuing",
                        );
                    }
                    Err(e) => return Err(e),
                }
                tx.execute(
                    "INSERT INTO schema_migrations(name) VALUES(:n)",
                    named_params! { ":n": name },
                )?;
                tracing::info!(
                    migration = %name,
                    took_ms = started.elapsed().as_millis() as u64,
                    "migration applied",
                );
                count += 1;
            }

            tx.commit()?;
            Ok::<usize, rusqlite::Error>(count)
        })
        .await
        .context("applying migration batch")?;

    tracing::info!(
        total_bundled = total,
        newly_applied = applied_now,
        "migration pass complete",
    );
    Ok(())
}

/// True when a migration error means the change it makes is *already present*,
/// so re-running it is a safe no-op rather than a failure. Currently this is
/// SQLite's "duplicate column name" from an `ALTER TABLE ADD COLUMN` whose
/// column already exists (ALTER has no `IF NOT EXISTS`). Scoped narrowly: any
/// other error still aborts the migration pass.
fn is_already_satisfied(e: &rusqlite::Error) -> bool {
    e.to_string()
        .to_ascii_lowercase()
        .contains("duplicate column name")
}

/// Extension trait that pins `E = rusqlite::Error` on the async `.call()`
/// helper provided by `tokio_rusqlite::Connection`. Without it every closure
/// body needs `Ok::<_, rusqlite::Error>(..)` annotations to satisfy the
/// generic error parameter.
pub trait ConnCallExt {
    fn call_db<F, R>(&self, f: F) -> impl Future<Output = Result<R>> + Send
    where
        F: FnOnce(&mut rusqlite::Connection) -> rusqlite::Result<R> + Send + 'static,
        R: Send + 'static;
}

impl ConnCallExt for Connection {
    // The explicit `-> impl Future + Send` return (rather than `async fn`) is
    // deliberate: it pins the `Send` bound the trait promises, which `async fn`
    // in a trait impl cannot express. Hence the `manual_async_fn` allow.
    #[allow(clippy::manual_async_fn)]
    fn call_db<F, R>(&self, f: F) -> impl Future<Output = Result<R>> + Send
    where
        F: FnOnce(&mut rusqlite::Connection) -> rusqlite::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        async move { self.call(f).await.map_err(anyhow::Error::from) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Re-applying an `ALTER TABLE ADD COLUMN` whose column already exists must
    /// be tolerated (this is the exact failure a re-run of an already-applied
    /// migration hits — SQLite ALTER has no `IF NOT EXISTS`). Guards the boot
    /// self-heal in `apply_migrations`. (Garlemald-Server #46.)
    #[test]
    fn duplicate_column_error_is_treated_as_already_satisfied() {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE t (a INTEGER);").unwrap();
        c.execute_batch("ALTER TABLE t ADD COLUMN b INTEGER;")
            .unwrap();
        let err = c
            .execute_batch("ALTER TABLE t ADD COLUMN b INTEGER;")
            .unwrap_err();
        assert!(
            is_already_satisfied(&err),
            "duplicate column must be tolerated; got: {err}"
        );
    }

    /// A genuine error (not a benign already-applied one) must NOT be swallowed
    /// — the migration pass should still fail loudly.
    #[test]
    fn other_errors_are_not_tolerated() {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        let err = c
            .execute_batch("SELECT * FROM nonexistent_table;")
            .unwrap_err();
        assert!(
            !is_already_satisfied(&err),
            "a real error must not be swallowed; got: {err}"
        );
    }
}
