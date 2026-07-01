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

//! Coroutine scheduler. Ported from the `mSleepingOnTime` / `mSleepingOnSignal`
//! / `mSleepingOnPlayerEvent` dictionaries in `LuaEngine.cs`.
//!
//! Scripts yield via `coroutine.yield("_WAIT_TIME", s)`,
//! `coroutine.yield("_WAIT_SIGNAL", name)`, or
//! `coroutine.yield("_WAIT_EVENT", player)`. The scheduler records the
//! pending thread and resumes it when the condition fires.
//!
//! Because `mlua::Thread` is tied to its source `Lua` runtime, each parked
//! coroutine stashes `(Arc<Lua>, Thread)`. This matches the C# shape of
//! holding a Coroutine reference alongside its script.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mlua::{Lua, Thread, Value};

use super::command::{CommandQueue, LuaCommandArg};

/// One parked coroutine, waiting for a condition.
///
/// The `queue` handle is the same `Arc<Mutex<CommandQueue>>` bound to
/// the script's userdata at spawn time — resumes may push commands
/// (e.g. `director:EndGuildleve(true)`), and the tick-driven resume
/// path drains those commands into the game loop's
/// `apply_runtime_lua_commands` pipeline.
pub struct ParkedCoroutine {
    pub lua: Arc<Lua>,
    pub thread: Thread,
    pub queue: Arc<Mutex<CommandQueue>>,
    /// Actor id of the player this coroutine acts on behalf of (the
    /// `onEventStarted` trigger player for director/command scripts),
    /// or 0 when unknown (e.g. a director `main` with no player in
    /// scope). The ticker uses this to route the commands a resumed
    /// coroutine queues through the EVENT bridge for that player —
    /// without it, event-flavoured commands (`RunEventFunction` /
    /// `EndEvent` / `KickEvent`) drained from a timer resume would fall
    /// through the runtime drain's catch-all and never reach the wire
    /// (e.g. the SEQ_005 director's post-`wait(1)` `kickEventContinue`
    /// + `processTtrBtl002`). (Garlemald-Server #28.)
    pub owner_player_id: u32,
}

impl std::fmt::Debug for ParkedCoroutine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParkedCoroutine").finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
pub struct CoroutineScheduler {
    /// Coroutines sleeping on a deadline (millis since UNIX epoch).
    sleeping_on_time: Vec<(u64, ParkedCoroutine)>,
    /// Coroutines sleeping on a named signal.
    sleeping_on_signal: HashMap<String, Vec<ParkedCoroutine>>,
    /// Coroutines sleeping on the next event update for a player.
    sleeping_on_player_event: HashMap<u32, ParkedCoroutine>,
}

impl CoroutineScheduler {
    pub fn shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::default()))
    }

    pub fn park_time(&mut self, seconds: f32, coroutine: ParkedCoroutine) {
        let now = common::utils::millis_unix_timestamp();
        let wake_at = now + (seconds.max(0.0) * 1000.0) as u64;
        self.sleeping_on_time.push((wake_at, coroutine));
    }

    pub fn park_signal(&mut self, signal: impl Into<String>, coroutine: ParkedCoroutine) {
        self.sleeping_on_signal
            .entry(signal.into())
            .or_default()
            .push(coroutine);
    }

    pub fn park_event(&mut self, player_id: u32, coroutine: ParkedCoroutine) {
        // If the player already has a parked coroutine, overwrite it; the
        // C# version explicitly dropped the old one on `_WAIT_EVENT`.
        //
        // DIAGNOSTIC (Garlemald-Server #46 — man0l1 SEQ_007 IncCounter drop):
        // a re-park while one is ALREADY pending DROPS the displaced
        // coroutine's not-yet-run continuation — including any `data:`/`quest:`
        // mutation it would queue on resume (e.g. Isandorel's
        // `data:IncCounter(CNTR_SEQ7_MSK)` queued AFTER the processEvent035
        // park). If the MSK counter never advancing is caused by such a
        // double-park, this warning fires during the Isandorel talk; if it
        // never fires, the counter is lost elsewhere. Temporary triage.
        if self.sleeping_on_player_event.contains_key(&player_id) {
            tracing::warn!(
                player = player_id,
                "park_event DISPLACING an already-parked coroutine — its pending \
                 continuation/mutations are dropped (Garlemald-Server #46 diagnostic)",
            );
        }
        self.sleeping_on_player_event.insert(player_id, coroutine);
    }

    /// Wake every time-parked coroutine whose deadline has passed. Returns
    /// them so the caller can `Thread::resume()` on each.
    pub fn drain_due_time(&mut self) -> Vec<ParkedCoroutine> {
        let now = common::utils::millis_unix_timestamp();
        let (due, pending): (Vec<_>, Vec<_>) = std::mem::take(&mut self.sleeping_on_time)
            .into_iter()
            .partition(|(t, _)| *t <= now);
        self.sleeping_on_time = pending;
        due.into_iter().map(|(_, c)| c).collect()
    }

    /// Wake every coroutine parked on `signal`.
    pub fn drain_signal(&mut self, signal: &str) -> Vec<ParkedCoroutine> {
        self.sleeping_on_signal.remove(signal).unwrap_or_default()
    }

    /// Pop the coroutine parked against a specific player's event channel.
    pub fn take_event(&mut self, player_id: u32) -> Option<ParkedCoroutine> {
        self.sleeping_on_player_event.remove(&player_id)
    }

    /// Drop every parked coroutine owned by `player_id`, across all
    /// three park kinds. The `ContentFinished` teardown calls this so
    /// a stale park (e.g. a director `_WAIT_EVENT` left behind by an
    /// abandoned tutorial) can never resume into a torn-down instance.
    /// Owner-0 (ownerless) parks are kept — they're not attributable
    /// to the leaving player. Returns the number of coroutines dropped.
    /// (#28 S1.3.)
    pub fn purge_owner(&mut self, player_id: u32) -> usize {
        if player_id == 0 {
            return 0;
        }
        let before =
            self.pending_time_count() + self.pending_signal_count() + self.pending_event_count();
        self.sleeping_on_time
            .retain(|(_, c)| c.owner_player_id != player_id);
        for parked in self.sleeping_on_signal.values_mut() {
            parked.retain(|c| c.owner_player_id != player_id);
        }
        self.sleeping_on_signal.retain(|_, v| !v.is_empty());
        // Event parks key by the yield's player id, which can differ
        // from the owning player (the historical player_id=0 fallback)
        // — purge by either identity.
        self.sleeping_on_player_event
            .retain(|k, v| *k != player_id && v.owner_player_id != player_id);
        before
            - (self.pending_time_count() + self.pending_signal_count() + self.pending_event_count())
    }

    pub fn pending_time_count(&self) -> usize {
        self.sleeping_on_time.len()
    }

    /// The earliest pending time-park deadline (millis since UNIX epoch,
    /// the same clock `drain_due_time` compares against), or `None` when
    /// nothing is parked on the clock. Used by the headless test harness
    /// (`crate::testkit`) to sleep just past the next `wait(n)` instead of
    /// guessing a fixed duration.
    #[cfg(feature = "testkit")]
    pub(crate) fn next_time_deadline_ms(&self) -> Option<u64> {
        self.sleeping_on_time.iter().map(|(t, _)| *t).min()
    }

    pub fn pending_signal_count(&self) -> usize {
        self.sleeping_on_signal.values().map(|v| v.len()).sum()
    }

    pub fn pending_event_count(&self) -> usize {
        self.sleeping_on_player_event.len()
    }
}

/// Inspect a `(status, value)` tuple returned by `coroutine.resume(thread)`
/// and decide how to re-park the thread. Returns the "what are you waiting
/// for" verdict, matching the C# `ResolveResume` helper.
#[derive(Debug)]
pub enum YieldDirective {
    /// Coroutine finished; don't re-park.
    Finished,
    /// Script returned `coroutine.yield("_WAIT_TIME", n)`.
    WaitTime(f32),
    /// Script returned `coroutine.yield("_WAIT_SIGNAL", name)`.
    WaitSignal(String),
    /// Script returned `coroutine.yield("_WAIT_EVENT", player)`.
    WaitEvent(u32),
    /// Neither — let caller handle it.
    Other,
}

pub fn classify_yield(value: &Value) -> YieldDirective {
    match value {
        Value::Nil => YieldDirective::Finished,
        Value::Table(tbl) => {
            let tag: Option<String> = tbl.get(1).ok();
            match tag.as_deref() {
                Some("_WAIT_TIME") => YieldDirective::WaitTime(tbl.get::<f32>(2).unwrap_or(0.0)),
                Some("_WAIT_SIGNAL") => {
                    YieldDirective::WaitSignal(tbl.get::<String>(2).unwrap_or_default())
                }
                Some("_WAIT_EVENT") => YieldDirective::WaitEvent(tbl.get::<u32>(2).unwrap_or(0)),
                _ => YieldDirective::Other,
            }
        }
        Value::String(s) if s.to_str().map(|c| c == "_WAIT_EVENT").unwrap_or(false) => {
            // The C# bare-string variant defers the player id to the
            // surrounding call context (see LuaEngine.ResolveResume).
            YieldDirective::WaitEvent(0)
        }
        _ => YieldDirective::Other,
    }
}

/// Multi-value-aware variant of [`classify_yield`]. `global.lua`'s wait
/// helpers yield BARE value pairs — `coroutine.yield("_WAIT_SIGNAL",
/// signal)` / `("_WAIT_TIME", seconds)` / `("_WAIT_EVENT", player)` —
/// so a resume captured as a single `Value` keeps only the tag string
/// and DISCARDS the argument. `classify_yield` then has no bare-string
/// arm for `_WAIT_SIGNAL`/`_WAIT_TIME` and returns `Other`, which the
/// repark path treats as "drop the coroutine": the SEQ_005 director
/// resumed off the cinematic's EventUpdate yielded
/// `("_WAIT_SIGNAL", "playerActive")`, got classified `Other`, and was
/// silently dropped — so the F press's `sendSignal("playerActive")`
/// found nothing parked and the tutorial soft-locked. Every resume site
/// must capture `mlua::MultiValue` and classify through this function.
/// (Garlemald-Server #28.)
pub fn classify_yield_mv(values: &mlua::MultiValue) -> YieldDirective {
    let mut it = values.iter();
    let Some(first) = it.next() else {
        return YieldDirective::Finished;
    };
    let second = it.next();
    match first {
        Value::String(s) => {
            let tag = s.to_str().map(|c| c.to_string()).unwrap_or_default();
            match tag.as_str() {
                "_WAIT_TIME" => YieldDirective::WaitTime(match second {
                    Some(Value::Number(n)) => *n as f32,
                    Some(Value::Integer(i)) => *i as f32,
                    _ => 0.0,
                }),
                "_WAIT_SIGNAL" => YieldDirective::WaitSignal(match second {
                    Some(Value::String(name)) => {
                        name.to_str().map(|c| c.to_string()).unwrap_or_default()
                    }
                    _ => String::new(),
                }),
                "_WAIT_EVENT" => YieldDirective::WaitEvent(match second {
                    // `waitForEvent(player)` passes the LuaPlayer
                    // userdata; coerce it to the actor id so the park
                    // lands under the real player (with the historical
                    // player_id=0 fallback still honoured by
                    // `fire_player_event_and_drain`).
                    Some(v) => match value_to_command_arg(v) {
                        LuaCommandArg::ActorId(id) => id,
                        LuaCommandArg::Int(i) => i as u32,
                        LuaCommandArg::UInt(u) => u as u32,
                        _ => 0,
                    },
                    None => 0,
                }),
                _ => YieldDirective::Other,
            }
        }
        // Table-shaped yields (and Nil = finished) keep the original
        // single-value semantics.
        other => classify_yield(other),
    }
}

/// Adapter: turn a Lua value into the matching `LuaCommandArg` so scripts can
/// return structured values that the game loop consumes. UserData values
/// (LuaPlayer / LuaActor / LuaNpc / LuaDirectorHandle / LuaQuestHandle)
/// are coerced to `ActorId` so cutscene and event RPCs that pass `player`
/// or `quest` as Lua-param entries (e.g. `callClientFunction(player,
/// "delegateEvent", player, quest, "processTtrNomal001withHQ")`) end up
/// with type-byte 0x06 on the wire instead of being silently flattened
/// to `Nil`.
pub fn value_to_command_arg(value: &Value) -> LuaCommandArg {
    match value {
        Value::Nil => LuaCommandArg::Nil,
        Value::Boolean(b) => LuaCommandArg::Bool(*b),
        Value::Integer(i) => LuaCommandArg::Int(*i),
        Value::Number(n) => LuaCommandArg::Float(*n),
        Value::String(s) => {
            LuaCommandArg::String(s.to_str().map(|c| c.to_string()).unwrap_or_default())
        }
        Value::UserData(ud) => {
            use super::userdata::{LuaActor, LuaDirectorHandle, LuaNpc, LuaPlayer, LuaQuestHandle};
            // Use `borrow_scoped` rather than `borrow`: the latter conflicts
            // with the mlua method binding's outer borrow when a script
            // passes `self` back into the call as a vararg
            // (`player:RunEventFunction("delegateEvent", player, …)`),
            // which silently dropped the player slot to Nil before this
            // change. `borrow_scoped` releases its handle as soon as the
            // closure returns, so it composes safely with the binding's
            // immutable borrow of `this`.
            if let Ok(id) = ud.borrow_scoped::<LuaPlayer, _>(|p| p.snapshot.actor_id) {
                LuaCommandArg::ActorId(id)
            } else if let Ok(id) = ud.borrow_scoped::<LuaActor, _>(|a| a.actor_id) {
                LuaCommandArg::ActorId(id)
            } else if let Ok(id) = ud.borrow_scoped::<LuaNpc, _>(|n| n.base.actor_id) {
                LuaCommandArg::ActorId(id)
            } else if let Ok(id) = ud.borrow_scoped::<LuaDirectorHandle, _>(|d| d.actor_id) {
                LuaCommandArg::ActorId(id)
            } else if let Ok(id) =
                ud.borrow_scoped::<LuaQuestHandle, _>(|q| 0xA0F0_0000 | q.quest_id)
            {
                // Meteor's CreateLuaParamList encodes a quest as
                // `0xA0F00000 | quest.GetQuestId()` (the same masking
                // StaticActors uses), then writes it as an Actor
                // LuaParam. Mirror that so the client recognises the
                // quest reference inside the cutscene RPC payload.
                LuaCommandArg::ActorId(id)
            } else {
                LuaCommandArg::Nil
            }
        }
        _ => LuaCommandArg::Nil,
    }
}

#[cfg(test)]
mod classify_yield_tests {
    use super::*;
    use mlua::{Lua, MultiValue};

    fn mv(lua: &Lua, vals: Vec<Value>) -> MultiValue {
        let mut m = MultiValue::new();
        for v in vals {
            m.push_back(v);
        }
        let _ = lua; // values already constructed against this Lua
        m
    }

    /// `global.lua`'s wait helpers yield BARE value pairs — the resume
    /// capture must not lose the second value. A single-`Value` capture
    /// classified `("_WAIT_SIGNAL", name)` as `Other` and dropped the
    /// SEQ_005 director at the `waitForSignal("playerActive")` re-park
    /// (the F-press softlock). (Garlemald-Server #28.)
    #[test]
    fn bare_pair_yields_classify_correctly() {
        let lua = Lua::new();

        let sig = mv(
            &lua,
            vec![
                Value::String(lua.create_string("_WAIT_SIGNAL").unwrap()),
                Value::String(lua.create_string("playerActive").unwrap()),
            ],
        );
        match classify_yield_mv(&sig) {
            YieldDirective::WaitSignal(name) => assert_eq!(name, "playerActive"),
            other => panic!("expected WaitSignal, got {other:?}"),
        }

        let time = mv(
            &lua,
            vec![
                Value::String(lua.create_string("_WAIT_TIME").unwrap()),
                Value::Integer(3),
            ],
        );
        match classify_yield_mv(&time) {
            YieldDirective::WaitTime(s) => assert_eq!(s, 3.0),
            other => panic!("expected WaitTime, got {other:?}"),
        }

        let event = mv(
            &lua,
            vec![
                Value::String(lua.create_string("_WAIT_EVENT").unwrap()),
                Value::Integer(42),
            ],
        );
        match classify_yield_mv(&event) {
            YieldDirective::WaitEvent(id) => assert_eq!(id, 42),
            other => panic!("expected WaitEvent, got {other:?}"),
        }
    }

    #[test]
    fn table_and_terminal_forms_still_classify() {
        let lua = Lua::new();

        let tbl = lua.create_table().unwrap();
        tbl.set(1, "_WAIT_SIGNAL").unwrap();
        tbl.set(2, "battleComplete").unwrap();
        let table_form = mv(&lua, vec![Value::Table(tbl)]);
        match classify_yield_mv(&table_form) {
            YieldDirective::WaitSignal(name) => assert_eq!(name, "battleComplete"),
            other => panic!("expected WaitSignal, got {other:?}"),
        }

        // Coroutine returned nothing → finished.
        match classify_yield_mv(&MultiValue::new()) {
            YieldDirective::Finished => {}
            other => panic!("expected Finished, got {other:?}"),
        }

        // Unknown tag → Other (caller drops).
        let unk = mv(
            &lua,
            vec![Value::String(lua.create_string("_SOMETHING").unwrap())],
        );
        assert!(matches!(classify_yield_mv(&unk), YieldDirective::Other));
    }
}
