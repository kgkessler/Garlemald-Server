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

//! The `combat` bridge: a spec-side userdata that drives the map-server
//! [`OpenerCombat`] facade (Tier-2, full battle substrate) through a starting
//! opener's SEQ_005 combat tutorial and lets a Lua spec assert the outcome.
//!
//! Unlike the [`crate::walkthrough`] (Tier-1) bridge, which drives quest hooks
//! synchronously on the bare `LuaEngine`, this leg needs the real world +
//! actor registry + battle ticker, which the facade stands up. The facade API
//! is async, so each spec call blocks on a shared [`tokio::runtime::Runtime`].
//!
//! The combat is a **force-kill**: `:killMonsters()` deals the wolves
//! their deaths through the genuine `BattleEvent::Die` path, so the engine's own
//! kill-gate fires `battleComplete` organically, the director resumes, and the
//! quest advances to SEQ_010, exactly as it would for a live client, minus the
//! turn-by-turn fight.
//!
//! Spec shape:
//! ```lua
//! local c = combat.start("Man0g0")
//! c:expectSpawn(3):expectSpawn(4):expectSpawn(5):expectSpawn(6):expectSpawn(7)
//! c:startDirector()
//! c:killMonsters()
//! c:expectSequence(10):expectContentCleared():expectZone(155)
//! ```

use std::path::Path;
use std::sync::Arc;

use mlua::{AnyUserData, UserData, UserDataMethods};
use tokio::runtime::Runtime;

use map_server::testkit::OpenerCombat;

fn lua_err(message: String) -> mlua::Error {
    mlua::Error::RuntimeError(message)
}

/// One opener's combat-tutorial playthrough, bound to the runtime the spec
/// runner blocks on.
pub struct CombatWalkthrough {
    combat: OpenerCombat,
    rt: Arc<Runtime>,
}

impl CombatWalkthrough {
    /// Stand up the full substrate and drive the content `onCreate` hook
    /// (capturing the spawn intent), leaving the director ready to start.
    pub fn start(rt: Arc<Runtime>, script_root: &Path, quest_code: &str) -> Result<Self, String> {
        let combat = rt
            .block_on(OpenerCombat::start(script_root, quest_code))
            .map_err(|e| format!("combat.start(\"{quest_code}\"): {e}"))?;
        Ok(Self { combat, rt })
    }
}

impl UserData for CombatWalkthrough {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // Assert the content onCreate queued a spawn for this bnpc id.
        methods.add_function("expectSpawn", |_, (this, bnpc): (AnyUserData, u32)| {
            {
                let w = this.borrow::<CombatWalkthrough>()?;
                let intent = w.combat.spawn_intent();
                if !intent.contains(&bnpc) {
                    return Err(lua_err(format!(
                        "expected onCreate to spawn bnpc {bnpc}; got {intent:?}"
                    )));
                }
            }
            Ok(this)
        });

        // Drive the director onEventStarted to the battleComplete park.
        methods.add_function("startDirector", |_, this: AnyUserData| {
            {
                let mut w = this.borrow_mut::<CombatWalkthrough>()?;
                let rt = w.rt.clone();
                rt.block_on(w.combat.start_director())
                    .map_err(|e| lua_err(format!("startDirector: {e}")))?;
            }
            Ok(this)
        });

        // Force-kill + drive the win sequence to completion.
        methods.add_function("killMonsters", |_, this: AnyUserData| {
            {
                let mut w = this.borrow_mut::<CombatWalkthrough>()?;
                let rt = w.rt.clone();
                rt.block_on(w.combat.kill_monsters())
                    .map_err(|e| lua_err(format!("killMonsters: {e}")))?;
            }
            Ok(this)
        });

        methods.add_function("expectSequence", |_, (this, seq): (AnyUserData, u32)| {
            {
                let w = this.borrow::<CombatWalkthrough>()?;
                let actual = w.rt.block_on(w.combat.quest_sequence());
                if actual != seq {
                    return Err(lua_err(format!(
                        "expected quest sequence {seq}, got {actual}"
                    )));
                }
            }
            Ok(this)
        });

        methods.add_function("expectContentCleared", |_, this: AnyUserData| {
            {
                let w = this.borrow::<CombatWalkthrough>()?;
                if w.rt.block_on(w.combat.content_active()) {
                    return Err(lua_err(
                        "expected the content script to be torn down, but it is still active"
                            .to_string(),
                    ));
                }
            }
            Ok(this)
        });

        methods.add_function("expectZone", |_, (this, zone): (AnyUserData, u32)| {
            {
                let w = this.borrow::<CombatWalkthrough>()?;
                let actual = w.rt.block_on(w.combat.player_zone());
                if actual != zone {
                    return Err(lua_err(format!(
                        "expected the player in zone {zone}, got {actual}"
                    )));
                }
            }
            Ok(this)
        });
    }
}
