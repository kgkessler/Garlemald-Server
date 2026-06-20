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

//! Headless opener-combat test harness facade.
//!
//! This is the reusable generalization of the in-crate proof-of-concept
//! `man0g0_seq005_force_kill_fires_battlecomplete_and_advances` integration
//! test. It builds the full map-server substrate (a `WorldManager`, an
//! `ActorRegistry`, a temp `Database` with the quest catalog installed, the
//! content + destination `Zone`s, a `MapSession` carrying the active content
//! script and the transient director roster, the player `Character` seeded at
//! the pre-fight quest sequence, and the wolf + ally actors), drives the
//! content script `onCreate` spawn-intent, then exposes a small driver API
//! that walks the QuestDirector tutorial to its `waitForSignal("battleComplete")`
//! park, force-kills the hostiles through the REAL death path, and drives the
//! post-kill win tail to completion.
//!
//! It is gated behind the `testkit` cargo feature so it never ships in the
//! normal server build. The out-of-crate `content-test` runner links against
//! it; the recipe is exercised in-crate by the feature-gated Gridania
//! integration test.
//!
//! Everything is driven on the DoM branch (`current_class = 22 = THM`), which
//! is the simplest director path: a single `battleComplete` signal wait, no
//! milestone tooltip chain.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use crate::actor::Character;
use crate::actor::quest::{Quest, quest_actor_id};
use crate::battle::outbox::BattleEvent;
use crate::data::{ActiveContentScript, ClientHandle, Session as MapSession};
use crate::lua::LuaEngine;
use crate::lua::command::{CommandQueue, LuaCommand};
use crate::lua::userdata::{LuaContentArea, LuaDirectorHandle, PlayerSnapshot, QuestStateSnapshot};
use crate::runtime::actor_registry::{ActorHandle, ActorKindTag, ActorRegistry};
use crate::runtime::dispatcher::dispatch_battle_event;
use crate::world_manager::WorldManager;
use crate::zone::area::{ActorKind, StoredActor};
use crate::zone::navmesh::StubNavmeshLoader;
use crate::zone::outbox::AreaOutbox;
use crate::zone::zone::Zone;

/// THM. `IsDiscipleOfMagic()` reads `current_class` in `22..=23`, picking the
/// simpler DoM director branch (a single `battleComplete` wait).
const CLASS_THM: u8 = 22;

/// Pre-fight quest sequence the opener directors open from; the win path
/// `StartSequence(10)` advances past it.
const PRE_FIGHT_SEQUENCE: u32 = 5;

/// Safety bound on every park-pump loop so a stuck director can never wedge a
/// test indefinitely.
const MAX_PUMP_ITERS: usize = 64;

/// Per-opener static differences. Everything that varies between the Gridania /
/// Limsa / Ul'dah SEQ_005 combat tutorials lives here; the driver logic is
/// otherwise identical (all three directors share the DoM
/// single-`battleComplete`-wait branch).
struct OpenerConfig {
    /// e.g. "Man0g0".
    quest_name: &'static str,
    /// Numeric quest id (e.g. 110_005).
    quest_id: u32,
    /// Content script under `scripts/lua/content/` (no extension).
    content_script: &'static str,
    /// Director name as the engine sees it (e.g. "Quest/QuestDirectorMan0g001").
    director_name: &'static str,
    /// Private-area name (e.g. "man0g01").
    area_name: &'static str,
    /// Hostile bnpc ids the content `onCreate` spawns; these gate
    /// `battleComplete` and are force-killed.
    wolves: &'static [u32],
    /// Ally bnpc ids the content `onCreate` spawns (roster only, never killed).
    allies: &'static [u32],
    /// Synthetic content-arena zone id; distinct per opener so concurrent runs
    /// don't collide.
    content_zone_id: u32,
    /// Warp destination after the win (the director's `DoZoneChange` target).
    destination_zone_id: u32,
}

impl OpenerConfig {
    /// Resolve a config from a `quest_code` like "Man0g0" (case-insensitive).
    fn resolve(quest_code: &str) -> Result<&'static OpenerConfig> {
        OPENERS
            .iter()
            .find(|c| c.quest_name.eq_ignore_ascii_case(quest_code))
            .copied()
            .with_context(|| format!("unknown opener quest_code {quest_code:?}"))
    }
}

/// Gridania (Man0g0 "Sundered Skies"): 3 wolves + Yda/Papalymo, warp to 155.
/// Limsa (Man0l0 "Shapeless Melody"): 3 aurelias + Sthalmann/Y'shtola, warp to
/// 230. Ul'dah (Man0u0 "Flowers for All"): 1 goobbue + Thancred/Niellefresne,
/// warp to 175. All three share the DoM single-`battleComplete` director branch.
static OPENERS: &[&OpenerConfig] = &[
    &OpenerConfig {
        quest_name: "Man0g0",
        quest_id: 110_005,
        content_script: "SimpleContent30010",
        director_name: "Quest/QuestDirectorMan0g001",
        area_name: "man0g01",
        wolves: &[3, 4, 5],
        allies: &[6, 7],
        content_zone_id: 166,
        destination_zone_id: 155,
    },
    &OpenerConfig {
        quest_name: "Man0l0",
        quest_id: 110_001,
        content_script: "SimpleContent30002",
        director_name: "Quest/QuestDirectorMan0l001",
        area_name: "man0l01",
        wolves: &[8, 9, 10],
        allies: &[11, 12],
        content_zone_id: 167,
        destination_zone_id: 230,
    },
    &OpenerConfig {
        quest_name: "Man0u0",
        quest_id: 110_009,
        content_script: "SimpleContent30079",
        director_name: "Quest/QuestDirectorMan0u001",
        area_name: "man0u01",
        wolves: &[13],
        allies: &[14, 15],
        content_zone_id: 168,
        destination_zone_id: 175,
    },
];

/// Synthetic actor id for a content-roster bnpc, keyed off its bnpc id (mirrors
/// the proof-of-concept's `0x4534_00NN` scheme).
fn content_actor_id(bnpc_id: u32) -> u32 {
    0x4534_0000 | (bnpc_id & 0xFF)
}

/// The headless opener-combat walkthrough driver. See the module docs.
pub struct OpenerCombat {
    cfg: &'static OpenerConfig,
    script_root: std::path::PathBuf,
    lua: Arc<LuaEngine>,
    world: Arc<WorldManager>,
    registry: Arc<ActorRegistry>,
    db: Arc<crate::database::Database>,
    player_id: u32,
    director_id: u32,
    session_id: u32,
    /// Synthetic content-zone actor ids of the hostile wolves (force-kill set).
    wolf_actor_ids: Vec<u32>,
    /// The bnpc ids the content `onCreate` queued via `SpawnBattleNpcById`.
    spawn_intent: Vec<u32>,
}

impl OpenerCombat {
    /// Build the full substrate and drive the content script `onCreate` hook,
    /// capturing the `SpawnBattleNpcById` intent. Leaves everything ready for
    /// [`Self::start_director`]. `quest_code` is e.g. "Man0g0".
    pub async fn start(script_root: &Path, quest_code: &str) -> Result<Self> {
        let cfg = OpenerConfig::resolve(quest_code)?;

        let lua = Arc::new(LuaEngine::new(script_root));
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let db = Arc::new(
            crate::database::Database::open(tempdb())
                .await
                .context("open temp database")?,
        );

        // The director's onEventStarted opens with `player:GetQuest(...)` and
        // the win path ends in QuestStartSequence, so install the quest catalog
        // so the GetQuest binding + apply path are wired.
        lua.catalogs().install_quests(
            db.get_quest_gamedata()
                .await
                .context("load quest gamedata")?,
        );

        let player_id = 0x0040_0001u32;
        let director_id = 0x6608_0003u32;
        let session_id = 7u32;
        let content_zone_id = cfg.content_zone_id;

        let wolf_actor_ids: Vec<u32> = cfg.wolves.iter().map(|&b| content_actor_id(b)).collect();
        let ally_actor_ids: Vec<u32> = cfg.allies.iter().map(|&b| content_actor_id(b)).collect();

        // --- content onCreate: capture the spawn intent. -------------------
        let oncreate_player = PlayerSnapshot {
            actor_id: player_id,
            name: "TestkitHero".to_string(),
            zone_id: content_zone_id,
            current_class: CLASS_THM,
            active_quests: vec![cfg.quest_id],
            ..Default::default()
        };
        let oncreate_area = LuaContentArea {
            parent_zone_id: content_zone_id,
            area_name: cfg.area_name.to_string(),
            area_class_path: "/Area/PrivateArea/ContentArea".to_string(),
            director_name: cfg.director_name.to_string(),
            director_actor_id: director_id,
            queue: CommandQueue::new(),
            players: Vec::new(),
            allies: Vec::new(),
            monsters: Vec::new(),
        };
        let oncreate_director = LuaDirectorHandle {
            name: cfg.director_name.to_string(),
            actor_id: director_id,
            class_path: format!("/Director/{}", cfg.director_name),
            queue: CommandQueue::new(),
        };
        let content_script_path = script_root.join(format!("content/{}.lua", cfg.content_script));
        let oncreate = lua.call_content_hook(
            &content_script_path,
            "onCreate",
            oncreate_player,
            oncreate_area,
            oncreate_director,
        );
        if let Some(e) = oncreate.error {
            bail!("content onCreate errored: {e}");
        }
        let spawn_intent: Vec<u32> = oncreate
            .commands
            .iter()
            .filter_map(|c| match c {
                LuaCommand::SpawnBattleNpcById { bnpc_id, .. } => Some(*bnpc_id),
                _ => None,
            })
            .collect();

        // --- live substrate: content arena + destination zones. ------------
        let mut zone = Zone::new(
            content_zone_id,
            "openerArena",
            106,
            "/Area/Zone/ZoneDefault",
            0,
            0,
            0,
            false,
            false,
            false,
            false,
            false,
            Some(&StubNavmeshLoader),
        );
        // Player + allies + wolves. Positions are placeholders; combat is
        // never simulated here (we force-kill the wolves directly).
        let mut placed: Vec<(u32, ActorKind)> = vec![(player_id, ActorKind::Player)];
        for &id in &ally_actor_ids {
            placed.push((id, ActorKind::Ally));
        }
        for &id in &wolf_actor_ids {
            placed.push((id, ActorKind::BattleNpc));
        }
        let mut ob = AreaOutbox::new();
        for (i, (id, kind)) in placed.iter().enumerate() {
            zone.core.add_actor(
                StoredActor {
                    actor_id: *id,
                    kind: *kind,
                    position: common::Vector3::new(370.0 + i as f32, 4.2, -700.0 - i as f32),
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );
        }
        let _ = ob.drain();
        world.register_zone(zone).await;

        // Destination zone for the post-win DoZoneChange. Without it
        // registered, do_zone_change_with_private_area logs "destination not
        // on this map server" and returns early WITHOUT updating the session
        // zone, so the move would silently no-op.
        let destination = Zone::new(
            cfg.destination_zone_id,
            "openerDest",
            101,
            "/Area/Zone/ZoneDefault",
            0,
            0,
            0,
            false,
            false,
            false,
            false,
            false,
            Some(&StubNavmeshLoader),
        );
        world.register_zone(destination).await;

        // Player Character: seed the quest journal at the pre-fight sequence
        // so the director's StartSequence(10) has a slot to mutate (the apply
        // path skips quests absent from the journal).
        let mut player = Character::new(player_id);
        player.base.zone_id = content_zone_id;
        player.base.position_x = 370.0;
        player.base.position_y = 4.2;
        player.base.position_z = -700.0;
        player.chara.hp = 1000;
        player.chara.max_hp = 1000;
        player.quest_journal.add(Quest::from_db_row(
            quest_actor_id(cfg.quest_id),
            cfg.quest_name,
            PRE_FIGHT_SEQUENCE,
            0,
            0,
            0,
            0,
            0,
        ));
        registry
            .insert(ActorHandle::new(
                player_id,
                ActorKindTag::Player,
                content_zone_id,
                session_id,
                player,
            ))
            .await;

        // Allies (Ally tag) + wolves (BattleNpc tag): the roster the kill
        // gate iterates. Only the BattleNpc wolves gate battleComplete.
        for (id, kind) in placed.iter().skip(1) {
            let mut c = Character::new(*id);
            c.base.zone_id = content_zone_id;
            c.chara.hp = 1000;
            c.chara.max_hp = 1000;
            let tag = if *kind == ActorKind::Ally {
                ActorKindTag::Ally
            } else {
                ActorKindTag::BattleNpc
            };
            registry
                .insert(ActorHandle::new(*id, tag, content_zone_id, 0, c))
                .await;
        }

        // Session: active content script + transient director roster.
        let (tx, _rx) = tokio::sync::mpsc::channel::<Vec<u8>>(512);
        world
            .register_client(session_id, ClientHandle::new(session_id, tx))
            .await;
        let mut session = MapSession {
            id: session_id,
            current_zone_id: content_zone_id,
            content_warp_acked: true,
            active_content_script: Some(ActiveContentScript {
                parent_zone_id: content_zone_id,
                area_name: cfg.area_name.to_string(),
                area_class_path: "/Area/PrivateArea/ContentArea".to_string(),
                director_name: cfg.director_name.to_string(),
                director_actor_id: director_id,
                content_area_actor_id: 0x6534_0002,
                content_script: cfg.content_script.to_string(),
                warp_complete: true,
                spawned_actor_ids: Vec::new(),
            }),
            ..MapSession::default()
        };
        let mut roster = vec![player_id, director_id];
        roster.extend(ally_actor_ids.iter().copied());
        roster.extend(wolf_actor_ids.iter().copied());
        session
            .transient_director_members
            .insert(director_id, roster);
        world.upsert_session(session).await;

        Ok(Self {
            cfg,
            script_root: script_root.to_path_buf(),
            lua,
            world,
            registry,
            db,
            player_id,
            director_id,
            session_id,
            wolf_actor_ids,
            spawn_intent,
        })
    }

    /// The bnpc ids the content `onCreate` queued via `SpawnBattleNpcById`
    /// (e.g. `[3,4,5,6,7]` for Gridania). For spec-side `:expectSpawn`
    /// assertions.
    pub fn spawn_intent(&self) -> Vec<u32> {
        self.spawn_intent.clone()
    }

    /// Route a resumed/initial command batch through the production event
    /// applier exactly as the live ticker / F-press dispatch do.
    async fn apply(&self, cmds: Vec<LuaCommand>) {
        if cmds.is_empty() {
            return;
        }
        let Some(handle) = self.registry.get(self.player_id).await else {
            return;
        };
        crate::runtime::quest_apply::apply_event_script_commands(
            &handle,
            cmds,
            &self.registry,
            &self.db,
            &self.world,
            Some(&self.lua),
        )
        .await;
    }

    fn pending_signal_count(&self) -> usize {
        self.lua.scheduler().lock().unwrap().pending_signal_count()
    }

    fn pending_event_count(&self) -> usize {
        self.lua.scheduler().lock().unwrap().pending_event_count()
    }

    fn pending_time_count(&self) -> usize {
        self.lua.scheduler().lock().unwrap().pending_time_count()
    }

    /// Sleep just past the earliest pending time-park deadline, then tick the
    /// scheduler and apply every resumed batch. Returns `false` if there was
    /// nothing parked on the clock.
    async fn advance_one_time_park(&self) -> bool {
        let deadline = self.lua.scheduler().lock().unwrap().next_time_deadline_ms();
        let Some(deadline) = deadline else {
            return false;
        };
        // The scheduler's deadlines live in the epoch-millis domain
        // (`drain_due_time` compares against `millis_unix_timestamp()`), so
        // sleep relative to that same clock, not the process-relative
        // `server_now_ms()` anchor, which would compute a multi-year wait.
        let now = common::utils::millis_unix_timestamp();
        let sleep_ms = deadline.saturating_sub(now) + 50;
        tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
        // `server_now_ms` is part of the live clock contract this harness
        // mirrors; touch it so the anchor is initialised the same way the
        // ticker would (keeps AI-state deadline math in one domain).
        let _ = crate::runtime::clock::server_now_ms();
        for (_owner, cmds) in self.lua.tick() {
            self.apply(cmds).await;
        }
        true
    }

    /// Drive the director `onEventStarted` forward until it is PARKED awaiting
    /// the `battleComplete` signal. Errors if it goes idle first or exceeds a
    /// safety bound.
    pub async fn start_director(&mut self) -> Result<()> {
        let player = PlayerSnapshot {
            actor_id: self.player_id,
            name: "TestkitHero".to_string(),
            zone_id: self.cfg.content_zone_id,
            current_class: CLASS_THM,
            current_event_owner: self.director_id,
            active_quests: vec![self.cfg.quest_id],
            active_quest_states: vec![QuestStateSnapshot {
                quest_id: self.cfg.quest_id,
                sequence: PRE_FIGHT_SEQUENCE,
                flags: 0,
                counters: [0; 4],
                npc_ls_from: 0,
                npc_ls_msg_step: 0,
            }],
            ..Default::default()
        };
        let director = LuaDirectorHandle {
            name: self.cfg.director_name.to_string(),
            actor_id: self.director_id,
            class_path: format!("/Director/{}", self.cfg.director_name),
            queue: CommandQueue::new(),
        };
        let director_script_path = self
            .script_root
            .join(format!("directors/{}.lua", self.cfg.director_name));
        let started = self.lua.call_director_on_event_started(
            &director_script_path,
            player,
            director,
            "noticeEvent".to_string(),
            PRE_FIGHT_SEQUENCE as u8,
            vec![],
        );
        if let Some(e) = started.error {
            bail!("director onEventStarted errored: {e}");
        }
        self.apply(started.commands).await;

        // Generic park-pump to the battleComplete signal wait. The DoM branch
        // walks: event park (cinematic) -> time park (wait(1)) -> event park
        // (kick reply) -> SIGNAL park (battleComplete).
        self.pump_to_battle_complete().await
    }

    /// Loop firing/advancing parks until a `battleComplete` signal wait exists
    /// (`pending_signal_count() >= 1`). Errors on going idle or exceeding the
    /// safety bound.
    async fn pump_to_battle_complete(&self) -> Result<()> {
        for _ in 0..MAX_PUMP_ITERS {
            if self.pending_signal_count() >= 1 {
                return Ok(());
            }
            if self.pending_event_count() > 0 {
                if let Some(cmds) = self.lua.fire_player_event_and_drain(self.player_id, &[]) {
                    self.apply(cmds).await;
                }
            } else if self.pending_time_count() > 0 {
                self.advance_one_time_park().await;
            } else {
                bail!("director went idle before parking on battleComplete");
            }
        }
        bail!("director did not reach the battleComplete park within the safety bound")
    }

    /// Force-kill: dispatch `BattleEvent::Die` (with lua + db) for
    /// each hostile wolf through the real death path; the last death fires
    /// `battleComplete` organically and resumes the director. Then drive the
    /// post-kill win tail to completion so StartSequence / EndEvent /
    /// ContentFinished / DoZoneChange land.
    pub async fn kill_monsters(&mut self) -> Result<()> {
        let zone_arc = self
            .world
            .zone(self.cfg.content_zone_id)
            .await
            .context("content arena zone vanished")?;
        for &wolf in &self.wolf_actor_ids {
            dispatch_battle_event(
                &BattleEvent::Die {
                    owner_actor_id: wolf,
                },
                &self.registry,
                &self.world,
                &zone_arc,
                Some(&self.lua),
                Some(&self.db),
            )
            .await;
        }

        // Post-kill win tail: drain every remaining time + event park until the
        // director is fully done (all park kinds drained to zero). The last
        // death already consumed the signal park organically.
        for _ in 0..MAX_PUMP_ITERS {
            if self.pending_event_count() == 0
                && self.pending_time_count() == 0
                && self.pending_signal_count() == 0
            {
                return Ok(());
            }
            if self.pending_event_count() > 0 {
                if let Some(cmds) = self.lua.fire_player_event_and_drain(self.player_id, &[]) {
                    self.apply(cmds).await;
                }
            } else if self.pending_time_count() > 0 {
                self.advance_one_time_park().await;
            } else {
                // Only a leftover signal park remains and nothing will fire it:
                // the director is wedged.
                bail!("director left a dangling signal park after the win tail");
            }
        }
        bail!("director win tail did not drain within the safety bound")
    }

    /// The player's tracked quest sequence (read from the Character
    /// quest_journal).
    pub async fn quest_sequence(&self) -> u32 {
        let Some(handle) = self.registry.get(self.player_id).await else {
            return 0;
        };
        let c = handle.character.read().await;
        c.quest_journal
            .get(self.cfg.quest_id)
            .map(|q| q.get_sequence())
            .unwrap_or(0)
    }

    /// Whether the session still has an active content script (false after the
    /// win tears it down).
    pub async fn content_active(&self) -> bool {
        self.world
            .session(self.session_id)
            .await
            .map(|s| s.active_content_script.is_some())
            .unwrap_or(false)
    }

    /// The player's current zone id (the warp destination after the win).
    pub async fn player_zone(&self) -> u32 {
        self.world
            .session(self.session_id)
            .await
            .map(|s| s.current_zone_id)
            .unwrap_or(0)
    }
}

/// A unique temp-db path per call (mirrors the integration-suite helper).
fn tempdb() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("garlemald-testkit-{nanos}-{seq}.db"))
}
