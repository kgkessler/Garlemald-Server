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

//! Game-loop ticker.
//!
//! Spawned from `main.rs` alongside `server::run`. Ticks at a configurable
//! cadence (default 100ms); each tick:
//!
//! 1. Advances the global millisecond clock.
//! 2. Walks every zone and, within each zone, every actor whose `zone_id`
//!    matches. For each actor, drives:
//!    - `StatusEffectContainer::update` → emits `StatusEvent`s.
//!    - `AIContainer::update`           → emits `BattleEvent`s.
//! 3. Drains and dispatches all typed outboxes (status / battle / area /
//!    inventory). Events route through the existing dispatcher functions,
//!    which turn them into real packets on session queues, DB writes, and
//!    Lua calls.
//!
//! The ticker holds `Arc` references to the `Database`, `WorldManager`,
//! `ActorRegistry`, and (later) `LuaEngine` — shareable and cheap to
//! clone into spawned tasks.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::time::interval;

use crate::actor::Character;
use crate::actor::modifier::ModifierMap;
use crate::battle::controller::ControllerOwnerView;
use crate::battle::outbox::BattleOutbox;
use crate::battle::target_find::{ActorArena, ActorView};
use crate::database::Database;
use crate::lua::LuaEngine;
use crate::status::StatusOutbox;
use crate::world_manager::WorldManager;
use crate::zone::outbox::AreaOutbox;
use crate::zone::zone::Zone;

use super::actor_registry::{ActorKindTag, ActorRegistry};
use super::dispatcher::{dispatch_area_event, dispatch_battle_event, dispatch_status_event};

/// Inn rested-XP accrual rate: 1 percentage point per N seconds while
/// the player is parked in an `is_inn` zone. 60s gives a +100% rested
/// bonus across a full inn-night sleep without being so fast that an
/// AFK lunch break maxes the bar.
pub const INN_REST_INTERVAL_SECS: u32 = 60;
/// Cap for the rested-bonus pool — 100% is retail's effective max
/// (matches the `accruerest` GM-command clamp + `consume_rested_xp`'s
/// `rested_pct.min(100)` ceiling).
pub const INN_REST_BONUS_CAP: i32 = 100;

/// BattleNpc respawn delay — how many seconds after `apply_die`
/// elapses before the ticker brings the NPC back at its spawn
/// position. Retail tunes this per-spawner via
/// `server_battlenpc_spawn_locations.respawnTimeMs`; the value here
/// is the conservative default the spawner falls back to when the
/// per-row override is missing or zero. 30s is a reasonable
/// playtest value (Meteor's default is 60s).
pub const BNPC_DEFAULT_RESPAWN_SECS: u32 = 30;

/// Corpse linger time for one-shot (respawn-disabled) BattleNpcs — the
/// SEQ_005 tutorial wolves. Long enough for the collapse animation +
/// a beat, short enough that the field clears while the player is
/// still nearby. pmeteor's DeathState despawn timer is 10s.
pub const CORPSE_DESPAWN_SECS: u32 = 10;

/// Per-actor death-state tick decision. Returned from the read-lock
/// scan inside `tick_zone` so the follow-up write lock + dispatcher
/// call happens after the read lock is released.
enum DeathTickAction {
    None,
    /// `Modifier::Raise > 0` on a dead actor — bring them back where
    /// they fell.
    AutoRevive,
    /// BattleNpc whose respawn timer has elapsed — snap back to
    /// spawn position + revive.
    Respawn,
    /// One-shot (respawn-disabled) BattleNpc whose corpse-linger timer
    /// elapsed — broadcast RemoveActor + drop from registry/grid.
    Despawn,
}

#[derive(Debug, Clone, Copy)]
pub struct TickerConfig {
    /// Tick period. Retail runs the zone thread ~every 333 ms, but 100 ms
    /// keeps combat + regen crisper without adding much load.
    pub tick_interval: Duration,
}

impl Default for TickerConfig {
    fn default() -> Self {
        Self {
            tick_interval: Duration::from_millis(100),
        }
    }
}

pub struct GameTicker {
    pub config: TickerConfig,
    pub world: Arc<WorldManager>,
    pub registry: Arc<ActorRegistry>,
    pub db: Arc<Database>,
    /// Optional Lua engine — when present, combat-side hooks like
    /// `onKillBNpc` can fire from the ticker's `dispatch_battle_event`
    /// path. Test harnesses pass `None`; main.rs wires the real engine.
    pub lua: Option<Arc<LuaEngine>>,
    /// Shared gamedata catalogs (items, quests, recipes, passive GL).
    /// Pulled from `lua.catalogs()` when a real `LuaEngine` is present;
    /// otherwise an empty instance so test harnesses don't need to wire
    /// up Catalogs just to run the ticker. The dispatchers consume this
    /// (gear-paramBonus summer reads `items`).
    pub catalogs: Arc<crate::lua::Catalogs>,
    /// Last `now_ms` the per-content-area `onUpdate` driver fired —
    /// delta-gated (see `tick_once`).
    last_content_update_ms: tokio::sync::Mutex<u64>,
}

impl GameTicker {
    pub fn new(
        config: TickerConfig,
        world: Arc<WorldManager>,
        registry: Arc<ActorRegistry>,
        db: Arc<Database>,
    ) -> Self {
        Self::with_lua(config, world, registry, db, None)
    }

    pub fn with_lua(
        config: TickerConfig,
        world: Arc<WorldManager>,
        registry: Arc<ActorRegistry>,
        db: Arc<Database>,
        lua: Option<Arc<LuaEngine>>,
    ) -> Self {
        let catalogs = lua
            .as_ref()
            .map(|l| l.catalogs().clone())
            .unwrap_or_else(|| Arc::new(crate::lua::Catalogs::default()));
        // Anchor the shared battle clock now so AI deadlines armed
        // before the first frame (login-time engages) share the domain.
        let _ = crate::runtime::clock::server_now_ms();
        Self {
            config,
            world,
            registry,
            db,
            lua,
            catalogs,
            last_content_update_ms: tokio::sync::Mutex::new(0),
        }
    }

    /// Run forever — suitable for `tokio::spawn`. Returns only on error.
    pub async fn run(self) -> ! {
        tracing::info!(
            interval_ms = self.config.tick_interval.as_millis() as u64,
            has_lua = self.lua.is_some(),
            "game ticker started",
        );
        let mut int = interval(self.config.tick_interval);
        // The first tick fires immediately; we want the period to apply
        // between ticks.
        int.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut frames: u64 = 0;
        loop {
            int.tick().await;
            // Shared battle-clock anchor (`runtime/clock.rs`) — the same
            // domain `apply_actor_engage` / the set-target engage arm use
            // to arm `AttackState.next_swing_ms`. The loop arithmetic
            // below is unchanged; only the anchor is shared. (#28 S0.2.)
            let now_ms = crate::runtime::clock::server_now_ms();
            self.tick_once(now_ms).await;
            frames += 1;
            // Liveness heartbeat — a frozen/starved ticker (the SEQ_005
            // wait(1) continuation silently never firing) is otherwise
            // invisible in the logs. Scheduler depths included so a
            // parked-but-never-drained coroutine is directly visible.
            if frames.is_multiple_of(100) {
                let (t, s, e) = self
                    .lua
                    .as_ref()
                    .and_then(|l| {
                        l.scheduler().lock().ok().map(|sch| {
                            (
                                sch.pending_time_count(),
                                sch.pending_signal_count(),
                                sch.pending_event_count(),
                            )
                        })
                    })
                    .unwrap_or((usize::MAX, usize::MAX, usize::MAX));
                tracing::debug!(
                    frames,
                    now_ms,
                    parked_time = t,
                    parked_signal = s,
                    parked_event = e,
                    "game ticker heartbeat",
                );
            }
        }
    }

    /// One pass of the tick loop. Exposed separately so tests can call it
    /// without waiting on the interval.
    pub async fn tick_once(&self, now_ms: u64) {
        let zone_ids = self.world.zone_ids().await;
        for zone_id in zone_ids {
            let Some(zone_arc) = self.world.zone(zone_id).await else {
                continue;
            };
            self.tick_zone(now_ms, zone_id, &zone_arc).await;
        }
        // Lua coroutine scheduler — resume every parked-on-time
        // coroutine whose deadline passed this frame (director
        // `main` scripts that yielded on `wait(N)`, player-script
        // coroutines from the same pool). Commands emitted by the
        // resumed slices drain through the shared runtime pipeline
        // so a resumed `director:EndGuildleve(true)` fan-out hits
        // the dispatcher + seal accrual same as a fresh script call.
        if let Some(lua) = self.lua.as_ref() {
            let lua = lua.clone();
            let resumed = match tokio::task::spawn_blocking(move || lua.tick()).await {
                Ok(batches) => batches,
                Err(e) => {
                    // A panic inside the scheduler tick would otherwise
                    // vanish into `unwrap_or_default()` — and a panicked
                    // resume means a parked coroutine was lost.
                    tracing::warn!(error = %e, "lua scheduler tick panicked");
                    Vec::new()
                }
            };
            // Per-owner batches: a coroutine resumed off a `wait(n)` may
            // queue EVENT-flavoured commands (the SEQ_005 director's
            // post-`wait(1)` `kickEventContinue` + `processTtrBtl002`
            // RunEventFunction) which the plain runtime drain DROPS at
            // its catch-all. Route batches with a known owner player
            // through the event bridge — same path the F-press command
            // dispatch uses — and fall back to the runtime drain for
            // ownerless coroutines. (Garlemald-Server #28.)
            for (owner, cmds) in resumed {
                let handle = if owner != 0 {
                    self.registry.get(owner).await
                } else {
                    None
                };
                if let Some(handle) = handle {
                    crate::runtime::quest_apply::apply_event_script_commands(
                        &handle,
                        cmds,
                        &self.registry,
                        &self.db,
                        &self.world,
                        self.lua.as_ref(),
                    )
                    .await;
                } else {
                    crate::runtime::quest_apply::apply_runtime_lua_commands(
                        cmds,
                        &self.registry,
                        &self.db,
                        &self.world,
                        self.lua.as_ref(),
                    )
                    .await;
                }
            }
        }

        // B6 of the SEQ_005 unblock plan: per-active-content-area
        // `onUpdate(tick, area)` driver. The combat tutorial's
        // `SimpleContent30010.lua::onUpdate` polls for engagement
        // state every tick and engages allies when it detects the
        // player in combat. We fire it every CONTENT_UPDATE_PERIOD_MS
        // milliseconds (~500ms by default) — the C# `Director`
        // tick interval. Sessions without an active content script
        // are skipped.
        const CONTENT_UPDATE_PERIOD_MS: u64 = 500;
        // Delta-based gate — `now_ms.is_multiple_of(500)` almost never
        // fired in production because `start.elapsed().as_millis()`
        // jitters off the exact multiples (the live SEQ_005 sessions
        // show ZERO content onUpdate activity — the ally-engage loop
        // was never running). (Garlemald-Server #28.)
        let content_due = {
            let mut last = self.last_content_update_ms.lock().await;
            if now_ms.saturating_sub(*last) >= CONTENT_UPDATE_PERIOD_MS {
                *last = now_ms;
                true
            } else {
                false
            }
        };
        // Deferred zone-in bundles (retail same-region pacing — see
        // `apply_do_zone_change`). Fire once due; while parked, the
        // position handler holds stale client coordinate reports off
        // the warp destination.
        {
            let now_unix_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            for session in self.world.all_sessions().await {
                let Some(pending) = session.pending_zone_in.clone() else {
                    continue;
                };
                if now_unix_ms < pending.fire_at_unix_ms {
                    continue;
                }
                if let Some(mut snap) = self.world.session(session.id).await {
                    snap.pending_zone_in = None;
                    self.world.upsert_session(snap).await;
                }
                self.world
                    .send_zone_in_bundle(
                        &self.registry,
                        &self.db,
                        self.lua.as_ref(),
                        session.id,
                        pending.spawn_type,
                        pending.commit_keep_list,
                    )
                    .await;
                if pending.notify_private_area
                    && let Some(client) = self.world.client(session.id).await
                {
                    let mut msg = crate::packets::send::misc::build_text_sheet_no_source_x28(
                        crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                        crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
                        34108,
                        0x20,
                    );
                    msg.set_target_id(session.id);
                    client.send_bytes(msg.to_bytes()).await;
                }
                tracing::info!(
                    session = session.id,
                    "deferred zone-in bundle dispatched (retail same-region pacing)",
                );
            }
        }

        if content_due && let Some(lua) = self.lua.as_ref() {
            let sessions = self.world.all_sessions().await;
            for session in sessions {
                let Some(active) = session.active_content_script.clone() else {
                    continue;
                };
                // Park the content driver until the client has finished
                // loading into the instance (it echoed its post-warp zone-in,
                // setting `content_warp_acked`). Driving the escort/combat
                // before then fires actor packets at a still-loading client
                // and crashes it. (Garlemald-Server #46.)
                if !session.content_warp_acked {
                    continue;
                }
                let script_path = lua.resolver().content(&active.content_script);
                if !script_path.exists() {
                    continue;
                }
                let placeholder_queue = crate::lua::command::CommandQueue::new();
                let (players_vec, allies_vec, monsters_vec) = build_content_rosters(
                    &self.registry,
                    &session,
                    &active,
                    placeholder_queue.clone(),
                )
                .await;

                let area = crate::lua::userdata::LuaContentArea {
                    parent_zone_id: active.parent_zone_id,
                    area_name: active.area_name.clone(),
                    area_class_path: active.area_class_path.clone(),
                    director_name: active.director_name.clone(),
                    director_actor_id: active.director_actor_id,
                    queue: placeholder_queue,
                    players: players_vec,
                    allies: allies_vec,
                    monsters: monsters_vec,
                };
                let lua_clone = lua.clone();
                let tick = now_ms / CONTENT_UPDATE_PERIOD_MS;
                let result = tokio::task::spawn_blocking(move || {
                    lua_clone.call_content_on_update(&script_path, tick, area)
                })
                .await;
                let partial = match result {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                if let Some(e) = partial.error {
                    tracing::debug!(
                        session = session.id,
                        content_script = %active.content_script,
                        error = %e,
                        "content onUpdate errored (likely missing binding — Phase B6 expected)",
                    );
                }
                if !partial.commands.is_empty() {
                    // Route through the EVENT bridge with the owning
                    // player's handle so content scripts can `sendSignal`
                    // (resuming parked director beats — the S4.1 kill
                    // gate's `battleComplete` shape) and emit event-
                    // flavoured commands; both were silent drops on the
                    // plain runtime drain. The bridge falls back to the
                    // same runtime appliers for everything else, so this
                    // is a strict superset. (#28 S1.4.)
                    if let Some(handle) = self.registry.by_session(session.id).await {
                        crate::runtime::quest_apply::apply_event_script_commands(
                            &handle,
                            partial.commands,
                            &self.registry,
                            &self.db,
                            &self.world,
                            self.lua.as_ref(),
                        )
                        .await;
                    } else {
                        crate::runtime::quest_apply::apply_runtime_lua_commands(
                            partial.commands,
                            &self.registry,
                            &self.db,
                            &self.world,
                            self.lua.as_ref(),
                        )
                        .await;
                    }
                }
            }
        }
    }

    async fn tick_zone(&self, now_ms: u64, zone_id: u32, zone: &Arc<RwLock<Zone>>) {
        let actors = self.registry.actors_in_zone(zone_id).await;
        // Hoist the zone's `is_inn` flag once per tick so the per-actor
        // rest-bonus loop doesn't have to re-acquire the read lock.
        let zone_is_inn = { zone.read().await.core.is_inn };

        for handle in actors {
            let mut status_outbox = StatusOutbox::new();
            let mut battle_outbox = BattleOutbox::new();

            // Drive status effects + AI while holding the character write lock.
            let owner_view = {
                let mut chara = handle.character.write().await;
                tick_status(&mut chara, now_ms, &mut status_outbox);
                build_owner_view(&chara, handle.actor_id, zone_id)
            };

            // AIContainer::update needs an ActorArena — the zone itself.
            // The movement intent (#28 S2.2) is taken in the same lock
            // scope, gated on `can_change_state()` — a Magic state on
            // top suppresses the step (pmeteor's HasMoved cast-interrupt
            // rule, satisfied without implementing interrupts).
            //
            // Corpses don't think and don't walk: a DEAD actor's
            // controller could still hold a MoveTo from its final tick
            // (live SEQ_005: a killed wolf glided away in its death
            // pose) — skip the whole AI/movement pass while dead. The
            // death-state housekeeping below owns dead actors.
            let is_dead_actor = {
                let chara = handle.character.read().await;
                chara.base.current_main_state == crate::actor::MAIN_STATE_DEAD
            };
            let movement = if is_dead_actor {
                let mut chara = handle.character.write().await;
                chara.ai_container.pending_movement = None;
                None
            } else {
                let zone_read = zone.read().await;
                let mut chara = handle.character.write().await;
                chara
                    .ai_container
                    .update(now_ms, owner_view, &*zone_read, &mut battle_outbox);
                if chara.ai_container.can_change_state() {
                    chara.ai_container.pending_movement.take()
                } else {
                    chara.ai_container.pending_movement = None;
                    None
                }
            };
            if let Some(step) = movement {
                apply_npc_movement_step(&self.world, &self.registry, zone, &handle, step).await;
            }

            // Chocobo rental expiry — port of Meteor commit `8687e431`'s
            // `Player.Update` arm. If the actor has an active rental
            // whose expire timestamp has elapsed, dismount and
            // restore the zone's BGM; otherwise tick down the
            // UI-visible minutes-remaining counter.
            //
            // Inn auto-accrual — Tier 4 #17 follow-up. While the
            // player is in an `is_inn` zone, accumulate `restBonus`
            // toward the +100% cap. Tracked alongside the chocobo
            // tick because both touch CharaState under the same write
            // lock; sharing the lock keeps the per-tick cost flat.
            //
            // Accrual rate: 1% per `INN_REST_INTERVAL_SECS` (default
            // 60s) — coarse enough that long inn stays visibly fill
            // the bar without a cliff. Persistence is deferred to the
            // logout / explicit-flush path so each tick stays a
            // single in-memory increment; `Database::set_rest_bonus_exp_rate`
            // already exists when we choose to flush.
            {
                let tick_utc = (now_ms / 1000) as u32;
                let mut chara = handle.character.write().await;
                if chara.chara.rental_expire_time != 0 {
                    if chara.chara.rental_expire_time <= tick_utc {
                        chara.chara.rental_expire_time = 0;
                        chara.chara.rental_min_left = 0;
                        chara.chara.mount_state = 0;
                        chara.base.current_main_state = crate::actor::MAIN_STATE_PASSIVE;
                        chara.chara.new_main_state = crate::actor::MAIN_STATE_PASSIVE;
                    } else {
                        chara.chara.rental_min_left =
                            ((chara.chara.rental_expire_time - tick_utc) / 60) as u8;
                    }
                }

                if zone_is_inn && chara.chara.rest_bonus_exp_rate < INN_REST_BONUS_CAP {
                    if chara.chara.last_rest_accrual_utc == 0 {
                        // Fresh entry — anchor the accrual window
                        // without granting points. Subsequent ticks
                        // measure elapsed time from here.
                        chara.chara.last_rest_accrual_utc = tick_utc;
                    } else {
                        let elapsed_since_last =
                            tick_utc.saturating_sub(chara.chara.last_rest_accrual_utc);
                        let earned = (elapsed_since_last / INN_REST_INTERVAL_SECS) as i32;
                        if earned > 0 {
                            chara.chara.rest_bonus_exp_rate =
                                (chara.chara.rest_bonus_exp_rate + earned).min(INN_REST_BONUS_CAP);
                            // Advance the anchor by exactly the
                            // earned amount * interval so any
                            // sub-interval remainder carries to the
                            // next tick instead of being lost.
                            chara.chara.last_rest_accrual_utc = chara
                                .chara
                                .last_rest_accrual_utc
                                .saturating_add(earned as u32 * INN_REST_INTERVAL_SECS);
                        }
                    }
                } else if !zone_is_inn {
                    // Player left the inn (or this is a non-inn
                    // tick) — reset the anchor so a future inn entry
                    // starts a fresh accrual window.
                    chara.chara.last_rest_accrual_utc = 0;
                }
            }

            // ---- Death-state housekeeping (Tier 1 #7 follow-up) ----
            //   * `Modifier::Raise > 0` — auto-revive a dead actor on
            //     the next tick (Players + NPCs alike). Mirrors
            //     Meteor's `Spawn` tail when a Raise modifier catches
            //     the actor before the despawn timer fires. Modifier
            //     stays in place — the matching status effect
            //     (Medic raise, Phoenix down) owns its own
            //     decay/cleanup.
            //   * BattleNpc respawn timer — for non-Player actors,
            //     when `time_of_death_utc + BNPC_DEFAULT_RESPAWN_SECS
            //     <= now`, restore HP/MP to max + flip back to
            //     PASSIVE at the spawn position. Player home-point
            //     revive is *not* automatic; it waits on a future
            //     packet handler.
            let action = {
                let chara = handle.character.read().await;
                if chara.base.current_main_state != crate::actor::MAIN_STATE_DEAD {
                    DeathTickAction::None
                } else {
                    let raise = chara.chara.mods.get(crate::actor::Modifier::Raise);
                    if raise > 0.0 {
                        DeathTickAction::AutoRevive
                    } else if !matches!(handle.kind, ActorKindTag::Player)
                        && !chara.chara.respawn_disabled
                        && chara.chara.time_of_death_utc != 0
                        // `time_of_death_utc` is wall-clock (apply_die
                        // stamps `unix_timestamp()`), so the comparison
                        // must read wall-clock too — never the ticker's
                        // server-start-relative `now_ms`. (#28 S0.2:
                        // one clock domain per field, asserted in
                        // `respawn_timer_compares_wall_clock`.)
                        && common::utils::unix_timestamp() as u32
                            >= chara
                                .chara
                                .time_of_death_utc
                                .saturating_add(BNPC_DEFAULT_RESPAWN_SECS)
                    {
                        DeathTickAction::Respawn
                    } else if !matches!(handle.kind, ActorKindTag::Player)
                        && chara.chara.respawn_disabled
                        && chara.chara.time_of_death_utc != 0
                        && common::utils::unix_timestamp() as u32
                            >= chara
                                .chara
                                .time_of_death_utc
                                .saturating_add(CORPSE_DESPAWN_SECS)
                    {
                        // One-shot (respawn-disabled) BNpcs — the SEQ_005
                        // tutorial wolves — fade their corpse instead of
                        // respawning (pmeteor DeathState: despawn timer
                        // after the collapse). Registry removal inside
                        // the applier is the once-only latch. (#28.)
                        DeathTickAction::Despawn
                    } else {
                        DeathTickAction::None
                    }
                }
            };
            match action {
                DeathTickAction::None => {}
                DeathTickAction::Despawn => {
                    crate::runtime::quest_apply::apply_despawn_actor(
                        zone_id,
                        handle.actor_id,
                        &self.registry,
                        &self.world,
                    )
                    .await;
                }
                DeathTickAction::AutoRevive | DeathTickAction::Respawn => {
                    // For respawn, also snap back to the spawn
                    // position before reviving — matches Meteor's
                    // BattleNpc respawn flow which restores to the
                    // initial spawn coords. AutoRevive (raise) just
                    // brings the actor back where they fell.
                    if matches!(action, DeathTickAction::Respawn) {
                        let mut c = handle.character.write().await;
                        let (sx, sy, sz) = (c.chara.spawn_x, c.chara.spawn_y, c.chara.spawn_z);
                        c.base.position_x = sx;
                        c.base.position_y = sy;
                        c.base.position_z = sz;
                    }
                    crate::runtime::dispatcher::apply_revive(
                        handle.actor_id,
                        &self.registry,
                        &self.world,
                        zone,
                    )
                    .await;
                }
            }

            for e in status_outbox.drain() {
                dispatch_status_event(&e, &self.registry, &self.world, &self.db, &self.catalogs)
                    .await;
            }
            for e in battle_outbox.drain() {
                dispatch_battle_event(
                    &e,
                    &self.registry,
                    &self.world,
                    zone,
                    self.lua.as_ref(),
                    Some(&self.db),
                )
                .await;
            }
        }

        // Area-level events (weather sweeps, broadcasts queued by scripts).
        let mut area_outbox = AreaOutbox::new();
        {
            let mut zone_write = zone.write().await;
            zone_write.sweep_finished_content(&mut area_outbox);
        }
        for e in area_outbox.drain() {
            dispatch_area_event(&e, &self.registry, &self.world, zone).await;
        }
    }
}

/// Phase C2b — build the content-area roster snapshots for the 500 ms
/// `onUpdate(tick, area)` driver, so the script's `area:GetPlayers()` /
/// `:GetAllies()` / `:GetMonsters()` iterators yield real userdata
/// instead of empty tables. Sources the roster from
/// `session.transient_director_members[director_id]` (Phase B4 records
/// the director's member list there) + the live `ActorRegistry`,
/// classified by `ActorKindTag`. The session-owning player is always in
/// `players`.
///
/// #28 S0.5 — dead actors are filtered out of the ally/monster vectors:
/// "in the roster table ≡ alive", so the content script needs no
/// liveness checks and corpses leave `GetMonsters()` the tick they die.
pub(crate) async fn build_content_rosters(
    registry: &ActorRegistry,
    session: &crate::data::Session,
    active: &crate::data::ActiveContentScript,
    placeholder_queue: std::sync::Arc<std::sync::Mutex<crate::lua::command::CommandQueue>>,
) -> (
    Vec<crate::lua::userdata::PlayerSnapshot>,
    Vec<crate::lua::userdata::LuaActor>,
    Vec<crate::lua::userdata::LuaActor>,
) {
    let mut players_vec = Vec::new();
    let mut allies_vec = Vec::new();
    let mut monsters_vec = Vec::new();

    // Resolve the owning player → PlayerSnapshot.
    if let Some(player_handle) = registry.by_session(session.id).await {
        let snap = {
            let c = player_handle.character.read().await;
            crate::processor::build_player_snapshot_from_character(&c)
        };
        players_vec.push(snap);
    }

    // Resolve every director member → LuaActor / PlayerSnapshot,
    // classified by ActorKindTag.
    let member_ids = if active.director_actor_id.ne(&0) {
        session
            .transient_director_members
            .get(&active.director_actor_id)
            .cloned()
            .unwrap_or_default()
    } else {
        Default::default()
    };
    for mid in member_ids {
        let Some(handle) = registry.get(mid).await else {
            continue;
        };
        let c = handle.character.read().await;
        match handle.kind {
            crate::runtime::actor_registry::ActorKindTag::Player => {
                // Skip the session-owner — already in players_vec from
                // the by_session branch.
                if mid != players_vec.first().map(|p| p.actor_id).unwrap_or(0) {
                    players_vec.push(crate::processor::build_player_snapshot_from_character(&c));
                }
            }
            kind @ (crate::runtime::actor_registry::ActorKindTag::Ally
            | crate::runtime::actor_registry::ActorKindTag::BattleNpc
            | crate::runtime::actor_registry::ActorKindTag::Npc
            | crate::runtime::actor_registry::ActorKindTag::Pet) => {
                // S0.5 dead-filter (see fn doc).
                if c.base.current_main_state == crate::actor::MAIN_STATE_DEAD {
                    continue;
                }
                let actor = crate::lua::userdata::LuaActor {
                    actor_id: mid,
                    name: c.base.actor_name.clone(),
                    class_name: String::new(),
                    class_path: String::new(),
                    unique_id: String::new(),
                    zone_id: handle.zone_id,
                    zone_name: String::new(),
                    state: c.base.current_main_state,
                    pos: (c.base.position_x, c.base.position_y, c.base.position_z),
                    rotation: c.base.rotation,
                    queue: placeholder_queue.clone(),
                    // Engagement = an active AttackState (pmeteor
                    // `aiContainer.IsEngaged()`), NOT
                    // `Character::is_engaged()` which reads
                    // `current_target != INVALID_ACTORID` and
                    // defaults to `0 != 0xC0000000` = always-true
                    // for NPCs that never get a `current_target`.
                    // With the buggy value the content script's
                    // `if not allies[i]:IsEngaged()` guard never
                    // passes and allies never engage. (Garlemald #28.)
                    is_engaged: c.ai_container.is_engaged(),
                    speed: c.get_speed(),
                    target_actor_id: c
                        .ai_container
                        .current_state()
                        .map(|s| s.target_actor_id)
                        .filter(|id| *id != 0)
                        .unwrap_or(0),
                    // #46 escort R4a — live HP for `actor:GetHP()` /
                    // `GetMaxHP()` (escort onUpdate monitors Sisipu).
                    hp: c.chara.hp,
                    max_hp: c.chara.max_hp,
                };
                // Ally if the registered kind says so OR the BattleNpc
                // was spawned with allegiance==1 (we can't read that
                // from ActorKindTag::BattleNpc alone, but B1 tags ally
                // BNpcs as `Ally`).
                match kind {
                    crate::runtime::actor_registry::ActorKindTag::Ally => allies_vec.push(actor),
                    _ => monsters_vec.push(actor),
                }
            }
        }
    }
    (players_vec, allies_vec, monsters_vec)
}

/// pmeteor `Actor.LookAt` (Actor.cs:688-704): face `(x, z)` from
/// `(owner_x, owner_z)` — `rotation = π - atan2(dz, dx) + π/2` with the
/// deltas measured owner − target.
pub(crate) fn look_at_rotation(owner_x: f32, owner_z: f32, x: f32, z: f32) -> f32 {
    if owner_x == x && owner_z == z {
        return 0.0;
    }
    let d_x = owner_x - x;
    let d_z = owner_z - z;
    std::f32::consts::PI - d_z.atan2(d_x) + std::f32::consts::FRAC_PI_2
}

/// #28 S2.2 — apply one straight-line movement step (Option B: the
/// SEQ_005 arena is flat, ≤ 11 y across and obstacle-free; the `PathFind`
/// port stays untouched for future zones). Per tick:
///
///   1. advance `min(speed * 0.1, dist)` toward the stop-ring destination
///      (the controller already projected the ring point, so walking the
///      full remaining distance can never overshoot — pmeteor
///      `PathFind.StepTo` shape; LSB: mob_controller.cpp::Move (shape
///      re-derived, GPL-3 — no verbatim copy));
///   2. rotate toward the destination (same ray as the target);
///   3. write `Character.base.position_*` then re-insert into the zone's
///      spatial grid so `actors_around` (the AI arena + broadcast radii)
///      stays truthful — the S0.3 sibling for the AI path;
///   4. emit 0x00CF via `broadcast_around_actor` (per-recipient
///      target-stamping handled there) on the FIRST step, every 3rd step
///      after (~300 ms — pmeteor's observed 333-360 ms cadence; the
///      client interpolates between waypoints), and on arrival.
///      `move_state = 2` (running) while stepping, final packet at the
///      ring with `move_state = 0` — moveState 2-vs-0 is the one
///      empirical unknown (pmeteor ships 0; wiki says 2 = running); flip
///      the constant if the live client double-plays the run animation.
///
/// Concrete wire shape per emission (wolf bnpc 3 closing on the player):
///   SubPacket size=0x50 source=<wolf id> target=<stamped per recipient>
///   opcode=0x00CF, body +0x00..07 zero | +0x08 f32 x | +0x0C f32 y |
///   +0x10 f32 z | +0x14 f32 rot | +0x18 u16 moveState | +0x24
///   floatingHeight = 0 (ground mobs).
async fn apply_npc_movement_step(
    world: &Arc<WorldManager>,
    registry: &Arc<ActorRegistry>,
    zone: &Arc<RwLock<Zone>>,
    handle: &crate::runtime::actor_registry::ActorHandle,
    step: crate::battle::ai_container::MovementStep,
) {
    use crate::battle::ai_container::MovementStep;

    let (emit, x, y, z, rot, move_state) = {
        let mut chara = handle.character.write().await;
        let pos = chara.base.position();
        match step {
            MovementStep::Face { position } => {
                // In range — rotation only, no translation, no wire (the
                // arrival packet already carried the facing; combat
                // result packets keep the client's pose fresh).
                chara.base.rotation = look_at_rotation(pos.x, pos.z, position.x, position.z);
                chara.ai_container.movement_step_count = 0;
                return;
            }
            MovementStep::MoveTo { destination } => {
                let dx = destination.x - pos.x;
                let dz = destination.z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                // speed 5.0 → 0.5 y per 100 ms tick = the announced
                // 0x00D0 run band, so client animation ≈ server step.
                let step_len = chara.get_speed() * 0.1;
                let arrived = step_len >= dist;
                let advance = step_len.min(dist);
                let (nx, ny, nz) = if dist > f32::EPSILON {
                    let t = advance / dist;
                    (
                        pos.x + dx * t,
                        pos.y + (destination.y - pos.y) * t,
                        pos.z + dz * t,
                    )
                } else {
                    (pos.x, pos.y, pos.z)
                };
                let rot = look_at_rotation(pos.x, pos.z, destination.x, destination.z);
                chara.base.position_x = nx;
                chara.base.position_y = ny;
                chara.base.position_z = nz;
                chara.base.rotation = rot;
                chara.ai_container.movement_step_count += 1;
                let count = chara.ai_container.movement_step_count;
                let emit = arrived || count % 3 == 1;
                if arrived {
                    chara.ai_container.movement_step_count = 0;
                }
                let move_state = if arrived { 0u16 } else { 2u16 };
                (emit, nx, ny, nz, rot, move_state)
            }
        }
    };

    // Grid re-insert — keeps `actors_around` (AI arena, broadcast radii)
    // in sync with the authoritative position we just wrote.
    {
        let mut zone_w = zone.write().await;
        let mut ob = AreaOutbox::new();
        zone_w
            .core
            .update_actor_position(handle.actor_id, common::Vector3::new(x, y, z), &mut ob);
        // ActorMoved grid events are visibility bookkeeping; the player
        // position path drops them too (world_manager.rs:2280-2298).
        let _ = ob.drain();
    }

    if emit {
        let sub = crate::packets::send::actor::build_move_actor_to_position(
            handle.actor_id,
            x,
            y,
            z,
            rot,
            move_state,
        );
        crate::runtime::broadcast::broadcast_around_actor(
            world,
            registry,
            zone,
            handle.actor_id,
            sub.to_bytes(),
        )
        .await;
    }
}

fn tick_status(chara: &mut Character, now_ms: u64, outbox: &mut StatusOutbox) {
    // Clone the ModifierMap so we can hand it in without aliasing — the
    // underlying HashMap is small enough that this is essentially free.
    let mods_snapshot: ModifierMap = chara.chara.mods.clone();
    chara.status_effects.update(now_ms, &mods_snapshot, outbox);
}

pub(crate) fn build_owner_view(
    chara: &Character,
    actor_id: u32,
    zone_id: u32,
) -> ControllerOwnerView {
    let is_engaged = chara.ai_container.is_engaged();
    let current_target = chara
        .ai_container
        .current_state()
        .map(|s| s.target_actor_id)
        .filter(|id| *id != 0);
    let most_hated = chara.hate.most_hated();
    ControllerOwnerView {
        actor: ActorView {
            actor_id,
            position: chara.base.position(),
            rotation: chara.base.rotation,
            is_alive: chara.is_alive(),
            is_static: false,
            allegiance: actor_id_to_allegiance(actor_id, chara),
            party_id: 0,
            zone_id,
            is_updates_locked: false,
            is_player: false,
            is_battle_npc: true,
        },
        is_engaged,
        is_spawned: true,
        is_following_path: chara
            .ai_container
            .path_find
            .as_ref()
            .is_some_and(|p| p.is_following_path()),
        at_path_end: chara
            .ai_container
            .path_find
            .as_ref()
            .is_none_or(|p| !p.is_following_path()),
        most_hated_actor_id: most_hated,
        current_target_actor_id: current_target,
        has_prevent_movement: false,
        max_hp: chara.get_max_hp(),
        current_hp: chara.get_hp(),
        target_hpp: None,
        target_has_stealth: false,
        is_close_to_spawn: true,
        target_is_locked: false,
        attack_delay_ms: chara.get_attack_delay_ms(),
        attack_range: chara.get_attack_range(),
    }
}

fn actor_id_to_allegiance(_actor_id: u32, chara: &Character) -> u32 {
    // Allegiance is on BattleSave/Character in retail; Phase 1 treats
    // anyone with an AI controller as "BattleNpc allegiance = 2" and the
    // rest as "Player allegiance = 1". Refined in Phase 3 once the Npc
    // types carry explicit allegiance fields.
    if chara.ai_container.controller.is_some() {
        2
    } else {
        1
    }
}

// ActorArena is implemented on Zone (crate::zone::zone). We re-export it
// here as a sanity check so downstream code knows the trait is in scope.
#[allow(dead_code)]
fn _zone_is_actor_arena(z: &Zone) -> &dyn ActorArena {
    z
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Character;
    use crate::runtime::actor_registry::{ActorHandle, ActorKindTag};
    use crate::zone::area::{ActorKind, StoredActor};
    use crate::zone::navmesh::StubNavmeshLoader;
    use common::Vector3;

    fn tempdb() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("garlemald-ticker-{nanos}-{seq}.db"))
    }

    async fn setup_one_zone_one_actor() -> (GameTicker, Arc<RwLock<Zone>>) {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let db = Arc::new(Database::open(tempdb()).await.expect("database stub"));

        let mut zone = Zone::new(
            100,
            "test",
            1,
            "/Area/Zone/Test",
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
        let mut ob = AreaOutbox::new();
        zone.core.add_actor(
            StoredActor {
                actor_id: 1,
                kind: ActorKind::BattleNpc,
                position: Vector3::ZERO,
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        world.register_zone(zone).await;

        let mut character = Character::new(1);
        character.chara.hp = 1000;
        character.chara.max_hp = 1000;
        character
            .chara
            .mods
            .set(crate::actor::modifier::Modifier::Regen, 5.0);
        registry
            .insert(ActorHandle::new(
                1,
                ActorKindTag::BattleNpc,
                100,
                0,
                character,
            ))
            .await;

        let ticker = GameTicker::new(TickerConfig::default(), world.clone(), registry, db);
        let zone_arc = world.zone(100).await.unwrap();
        (ticker, zone_arc)
    }

    #[tokio::test]
    async fn one_tick_runs_without_panic() {
        let (ticker, _zone) = setup_one_zone_one_actor().await;
        ticker.tick_once(1_000).await;
    }

    #[tokio::test]
    async fn regen_tick_applies_hp_delta() {
        let (ticker, _zone) = setup_one_zone_one_actor().await;

        // Drop the actor's HP first, then tick far enough to cross the 3s
        // regen cadence.
        let handle = ticker.registry.get(1).await.unwrap();
        {
            let mut chara = handle.character.write().await;
            chara.chara.hp = 500;
        }
        ticker.tick_once(5_000).await;

        let hp_after = handle.character.read().await.chara.hp;
        assert!(
            hp_after > 500,
            "regen should have bumped hp, got {hp_after}"
        );
    }

    /// Set up one zone with one Player (id=1) and one passive BattleNpc
    /// (id=2). The NPC has no controller so it won't retaliate. Shared by
    /// the auto-attack / cast-resolution tests below.
    async fn setup_player_vs_npc() -> (GameTicker, Arc<RwLock<Zone>>) {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let db = Arc::new(Database::open(tempdb()).await.expect("database stub"));

        let mut zone = Zone::new(
            100,
            "test",
            1,
            "/Area/Zone/Test",
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
        let mut ob = AreaOutbox::new();
        zone.core.add_actor(
            StoredActor {
                actor_id: 1,
                kind: ActorKind::Player,
                position: Vector3::ZERO,
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        zone.core.add_actor(
            StoredActor {
                actor_id: 2,
                kind: ActorKind::BattleNpc,
                position: Vector3::new(3.0, 0.0, 0.0),
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        world.register_zone(zone).await;

        let mut player = Character::new(1);
        player.chara.hp = 1000;
        player.chara.max_hp = 1000;
        player.chara.level = 10;
        registry
            .insert(ActorHandle::new(1, ActorKindTag::Player, 100, 42, player))
            .await;

        let mut npc = Character::new(2);
        npc.chara.hp = 1000;
        npc.chara.max_hp = 1000;
        npc.chara.level = 10;
        npc.base.position_x = 3.0;
        registry
            .insert(ActorHandle::new(2, ActorKindTag::BattleNpc, 100, 0, npc))
            .await;

        let ticker = GameTicker::new(TickerConfig::default(), world.clone(), registry, db);
        let zone_arc = world.zone(100).await.unwrap();
        (ticker, zone_arc)
    }

    #[tokio::test]
    async fn auto_attack_drops_defender_hp() {
        let (ticker, _zone) = setup_player_vs_npc().await;

        // Player engages the NPC. The default swing delay is 2500 ms.
        {
            let handle = ticker.registry.get(1).await.unwrap();
            let mut chara = handle.character.write().await;
            chara.ai_container.internal_engage(2, 0, 2500);
        }

        let npc_handle = ticker.registry.get(2).await.unwrap();
        let initial_hp = npc_handle.character.read().await.get_hp();
        assert_eq!(initial_hp, 1000);

        // Tick through ~10 swings. Base auto-attack damage is 0..=90 with
        // a ~10% chance of zero, so run enough swings that the combined
        // probability of no damage is negligible.
        for i in 1..=10 {
            ticker.tick_once((i as u64) * 2_600).await;
        }

        let final_hp = npc_handle.character.read().await.get_hp();
        assert!(
            final_hp < initial_hp,
            "npc hp should have dropped after 10 swings, got {final_hp}"
        );
    }

    /// #28 S0.5(a) — dead actors leave the onUpdate roster: a wolf at
    /// `MAIN_STATE_DEAD` must be excluded from the monsters vector so
    /// the content script's "in the table ≡ alive" invariant holds.
    #[tokio::test]
    async fn dead_actor_excluded_from_content_rosters() {
        let registry = ActorRegistry::new();
        let director_id = 0x6608_0001u32;

        for (id, kind, dead) in [
            (0x4000_0010u32, ActorKindTag::BattleNpc, false),
            (0x4000_0011u32, ActorKindTag::BattleNpc, true),
            (0x4000_0012u32, ActorKindTag::Ally, true),
        ] {
            let mut c = Character::new(id);
            if dead {
                c.base.current_main_state = crate::actor::MAIN_STATE_DEAD;
            }
            registry.insert(ActorHandle::new(id, kind, 166, 0, c)).await;
        }

        let mut session = crate::data::Session::new(7);
        session
            .transient_director_members
            .insert(director_id, vec![0x4000_0010, 0x4000_0011, 0x4000_0012]);
        let active = crate::data::ActiveContentScript {
            parent_zone_id: 166,
            area_name: "man0g01".to_string(),
            area_class_path: String::new(),
            director_name: String::new(),
            director_actor_id: director_id,
            content_area_actor_id: 0,
            content_script: "SimpleContent30010".to_string(),
            warp_complete: true,
            spawned_actor_ids: Vec::new(),
        };

        let (players, allies, monsters) = super::build_content_rosters(
            &registry,
            &session,
            &active,
            crate::lua::command::CommandQueue::new(),
        )
        .await;
        assert!(players.is_empty(), "no player session registered");
        let monster_ids: Vec<u32> = monsters.iter().map(|m| m.actor_id).collect();
        assert_eq!(
            monster_ids,
            vec![0x4000_0010],
            "dead wolf must be filtered out of the monsters roster",
        );
        assert!(
            allies.is_empty(),
            "dead ally must be filtered out of the allies roster",
        );
    }

    /// #28 S0.5(b) — `respawn_disabled` corpses stay dead past
    /// `BNPC_DEFAULT_RESPAWN_SECS` (and despawn entirely once the
    /// corpse-linger window elapses); without the flag the same corpse
    /// respawns. Also asserts the S0.2 domain choice: the death-tick
    /// compares wall-clock `unix_timestamp()` against the wall-clock
    /// `time_of_death_utc` apply_die stamps — never the ticker's
    /// relative `now_ms` (which is 0 here and would never trigger).
    #[tokio::test]
    async fn respawn_timer_compares_wall_clock() {
        let (ticker, _zone) = setup_one_zone_one_actor().await;
        let died_at = common::utils::unix_timestamp() as u32 - (BNPC_DEFAULT_RESPAWN_SECS + 1);

        // Respawn-disabled corpse: stays dead through the respawn
        // window, then DESPAWNS once the corpse-linger window elapses
        // (BNPC_DEFAULT_RESPAWN_SECS+1 > CORPSE_DESPAWN_SECS) — the
        // registry entry is gone, never revived. (#28: tutorial wolves
        // must fade after defeat.)
        {
            let handle = ticker.registry.get(1).await.unwrap();
            let mut c = handle.character.write().await;
            c.base.current_main_state = crate::actor::MAIN_STATE_DEAD;
            c.chara.hp = 0;
            c.chara.time_of_death_utc = died_at;
            c.chara.respawn_disabled = true;
        }
        ticker.tick_once(0).await;
        assert!(
            ticker.registry.get(1).await.is_none(),
            "respawn-disabled corpse must despawn after the linger window",
        );

        // Re-insert the same corpse shape for the respawn-path half.
        {
            let mut c = crate::actor::Character::new(1);
            c.base.current_main_state = crate::actor::MAIN_STATE_DEAD;
            c.chara.hp = 0;
            c.chara.max_hp = 100;
            c.chara.time_of_death_utc = died_at;
            c.chara.respawn_disabled = true;
            ticker
                .registry
                .insert(crate::runtime::actor_registry::ActorHandle::new(
                    1,
                    crate::runtime::actor_registry::ActorKindTag::BattleNpc,
                    100,
                    0,
                    c,
                ))
                .await;
        }

        // Same corpse with the flag cleared: the elapsed wall-clock
        // window revives it even though the tick's now_ms is 0.
        {
            let handle = ticker.registry.get(1).await.unwrap();
            let mut c = handle.character.write().await;
            c.chara.respawn_disabled = false;
        }
        ticker.tick_once(0).await;
        {
            let handle = ticker.registry.get(1).await.unwrap();
            let c = handle.character.read().await;
            assert_ne!(
                c.base.current_main_state,
                crate::actor::MAIN_STATE_DEAD,
                "default corpse must respawn once the wall-clock window elapses",
            );
        }
    }

    /// Shared harness for the #28 S2.2/S2.4 movement + caster tests:
    /// one zone (100) with a Player (id 1, session 42, client channel
    /// captured) and one controller-driven NPC (id 2) at `npc_x`,
    /// engaged on the player with seeded hate.
    async fn setup_engaged_npc_vs_player(
        npc_x: f32,
        configure: impl FnOnce(&mut crate::battle::controller::Controller),
    ) -> (
        GameTicker,
        Arc<RwLock<Zone>>,
        tokio::sync::mpsc::Receiver<Vec<u8>>,
    ) {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let db = Arc::new(Database::open(tempdb()).await.expect("database stub"));

        let mut zone = Zone::new(
            100,
            "test",
            1,
            "/Area/Zone/Test",
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
        let mut ob = AreaOutbox::new();
        zone.core.add_actor(
            StoredActor {
                actor_id: 1,
                kind: ActorKind::Player,
                position: Vector3::ZERO,
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        zone.core.add_actor(
            StoredActor {
                actor_id: 2,
                kind: ActorKind::BattleNpc,
                position: Vector3::new(npc_x, 0.0, 0.0),
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        world.register_zone(zone).await;

        let mut player = Character::new(1);
        player.chara.hp = 1000;
        player.chara.max_hp = 1000;
        player.chara.level = 10;
        registry
            .insert(ActorHandle::new(1, ActorKindTag::Player, 100, 42, player))
            .await;
        let (tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(512);
        world
            .register_client(42, crate::data::ClientHandle::new(42, tx))
            .await;
        world
            .upsert_session(crate::data::Session {
                id: 42,
                current_zone_id: 100,
                ..crate::data::Session::default()
            })
            .await;

        let mut npc = Character::new(2);
        npc.chara.hp = 1000;
        npc.chara.max_hp = 1000;
        npc.chara.level = 10;
        npc.base.position_x = npc_x;
        let mut ctrl = crate::battle::controller::BattleNpcController::new_for(2);
        configure(&mut ctrl);
        npc.ai_container.controller = Some(ctrl);
        npc.hate.update_hate(1, 10);
        npc.ai_container.internal_engage(1, 0, 2500);
        registry
            .insert(ActorHandle::new(2, ActorKindTag::BattleNpc, 100, 0, npc))
            .await;

        let ticker = GameTicker::new(TickerConfig::default(), world.clone(), registry, db);
        let zone_arc = world.zone(100).await.unwrap();
        (ticker, zone_arc, rx)
    }

    fn drain_subpackets(
        rx: &mut tokio::sync::mpsc::Receiver<Vec<u8>>,
    ) -> Vec<common::subpacket::SubPacket> {
        let mut out = Vec::new();
        while let Ok(bytes) = rx.try_recv() {
            let mut offset = 0;
            while offset < bytes.len() {
                match common::subpacket::SubPacket::parse(&bytes, &mut offset) {
                    Ok(sub) => out.push(sub),
                    Err(_) => break,
                }
            }
        }
        out
    }

    /// #28 S2.2 (a)+(b)+(c) — an engaged wolf 9 y out converges to the
    /// 2.8 y melee ring in < 2 s of ticks, never steps inside it, keeps
    /// the spatial grid in sync, and emits 0x00CF on the first step,
    /// every 3rd step after, and at arrival (final move_state 0).
    #[tokio::test]
    async fn s2_2_wolf_pursuit_converges_to_ring_with_throttled_wire() {
        let (ticker, zone, mut rx) = setup_engaged_npc_vs_player(9.0, |_| {}).await;

        for i in 1..=20u64 {
            ticker.tick_once(i * 100).await;
        }

        let handle = ticker.registry.get(2).await.unwrap();
        let (x, z) = {
            let c = handle.character.read().await;
            (c.base.position_x, c.base.position_z)
        };
        let dist = (x * x + z * z).sqrt();
        assert!(
            (dist - 2.8).abs() < 1e-3,
            "wolf must park exactly on the 2.8 y stop ring, got {dist}",
        );

        // Spatial grid tracked the authoritative position.
        {
            let zr = zone.read().await;
            let stored = zr.core.find_actor(2).expect("wolf in grid");
            assert!(
                (stored.position.x - x).abs() < 1e-3,
                "grid x {} must track authoritative x {x}",
                stored.position.x,
            );
        }

        // Wire: 0x00CF at steps 1,4,7,10 (move_state 2) + arrival 13
        // (move_state 0). 13 steps × 100 ms = 1.3 s < 2 s.
        let moves: Vec<(f32, u16)> = drain_subpackets(&mut rx)
            .into_iter()
            .filter(|s| s.game_message.opcode == crate::packets::opcodes::OP_MOVE_ACTOR_TO_POSITION)
            .map(|s| {
                let x = f32::from_le_bytes(s.data[0x08..0x0C].try_into().unwrap());
                let state = u16::from_le_bytes(s.data[0x18..0x1A].try_into().unwrap());
                (x, state)
            })
            .collect();
        assert_eq!(
            moves.len(),
            5,
            "first step + every 3rd + arrival = 5 emissions, got {moves:?}",
        );
        assert!(
            moves[..4].iter().all(|(_, st)| *st == 2),
            "in-flight packets run with move_state 2, got {moves:?}",
        );
        assert_eq!(moves[4].1, 0, "arrival packet must carry move_state 0");
        assert!(
            (moves[4].0 - 2.8).abs() < 1e-3,
            "arrival packet x must sit on the ring, got {moves:?}",
        );
        // Monotonic approach — never inside the ring.
        for (x, _) in &moves {
            assert!(
                *x >= 2.8 - 1e-3,
                "no packet may report a position inside the ring"
            );
        }
    }

    /// #28 S2.2 (d) — a caster (standback_range 15) closes from 20 y to
    /// the 15 y ring and parks there.
    #[tokio::test]
    async fn s2_2_caster_standoff_stops_at_standback_ring() {
        let (ticker, _zone, _rx) = setup_engaged_npc_vs_player(20.0, |ctrl| {
            ctrl.battle.is_caster = true;
            ctrl.auto_attack_enabled = false;
            // No spell resolved — the caster stands back without casting.
        })
        .await;

        for i in 1..=20u64 {
            ticker.tick_once(i * 100).await;
        }
        let handle = ticker.registry.get(2).await.unwrap();
        let x = handle.character.read().await.base.position_x;
        assert!(
            (x - 15.0).abs() < 1e-3,
            "caster must park on the 15 y standback ring, got {x}",
        );
    }

    /// #28 S2.2 (e) — a Magic state on top suppresses the movement step
    /// (`can_change_state` gate): the actor holds position for the whole
    /// cast.
    #[tokio::test]
    async fn s2_2_magic_state_suppresses_movement_step() {
        let (ticker, _zone, _rx) = setup_engaged_npc_vs_player(9.0, |_| {}).await;
        {
            let handle = ticker.registry.get(2).await.unwrap();
            let mut c = handle.character.write().await;
            let mut cmd = crate::battle::BattleCommand::new(100, "stone");
            cmd.cast_time_ms = 60_000;
            assert!(c.ai_container.internal_cast(1, cmd, 0));
        }
        for i in 1..=10u64 {
            ticker.tick_once(i * 100).await;
        }
        let handle = ticker.registry.get(2).await.unwrap();
        let x = handle.character.read().await.base.position_x;
        assert_eq!(x, 9.0, "casting actor must not take a single step");
    }

    /// #28 S2.4 — the full caster loop against the REAL seed catalog:
    /// thunder 27313 resolves through the gamedata→battle converter,
    /// CastStart renders chant 0xF0 (0x0144) + begin-cast 0x0139 (anim
    /// 0x6F000002, cmd 27313, text 30128), completion clears the chant
    /// and lands ResolveAction damage, and the recast clock holds the
    /// next cast ≥ 8 s after the first.
    #[tokio::test]
    async fn s2_4_caster_loop_chants_casts_and_damages() {
        let (ticker, _zone, mut rx) = setup_engaged_npc_vs_player(10.0, |_| {}).await;

        // Resolve thunder out of the seeded battle-command catalog and
        // pin the converter's load-bearing fields (report C §5).
        let (catalog, _) = ticker
            .db
            .load_global_battle_command_list()
            .await
            .expect("battle command catalog");
        let thunder = catalog.get(&27313).expect("thunder 27313 seeded");
        let spell = thunder.to_battle_command();
        assert_eq!(spell.range, 20.0);
        assert_eq!(spell.cast_time_ms, 2000);
        assert_eq!(spell.recast_time_ms, 6000);
        assert_eq!(spell.cast_type, 2);
        assert_eq!(spell.battle_animation, 16781815);
        assert_eq!(spell.base_potency, 100);
        assert_eq!(spell.command_type, crate::battle::CommandType::SPELL);
        assert_eq!(spell.action_type, crate::battle::ActionType::Magic);

        {
            let handle = ticker.registry.get(2).await.unwrap();
            let mut c = handle.character.write().await;
            let ctrl = c.ai_container.controller.as_mut().unwrap();
            ctrl.battle.is_caster = true;
            ctrl.battle.spell = Some(spell);
            ctrl.auto_attack_enabled = false;
        }

        let player_initial_hp = {
            let handle = ticker.registry.get(1).await.unwrap();
            let c = handle.character.read().await;
            c.get_hp()
        };

        // First tick fires the Cast decision (10 y < range 20, inside the
        // 15 y ring → no movement); cast completes at 100 + 2000; the
        // second cast unlocks at 8100 (tick 81).
        for i in 1..=82u64 {
            ticker.tick_once(i * 100).await;
        }

        let subs = drain_subpackets(&mut rx);
        let substates: Vec<u8> = subs
            .iter()
            .filter(|s| {
                s.game_message.opcode == crate::packets::opcodes::OP_SET_ACTOR_SUB_STATE
                    && s.header.source_id == 2
            })
            .map(|s| s.data[1]) // chant_id byte
            .collect();
        assert!(
            substates.starts_with(&[0xF0, 0x00]),
            "chant glow on at cast start, off at completion; got {substates:?}",
        );

        let begin_casts: Vec<(u32, u16, u32, u16)> = subs
            .iter()
            .filter(|s| {
                s.game_message.opcode == crate::packets::opcodes::OP_COMMAND_RESULT_X01
                    && s.header.source_id == 2
            })
            .map(|s| {
                let anim = u32::from_le_bytes(s.data[0x04..0x08].try_into().unwrap());
                let cmd = u16::from_le_bytes(s.data[0x24..0x26].try_into().unwrap());
                let target = u32::from_le_bytes(s.data[0x28..0x2C].try_into().unwrap());
                let text = u16::from_le_bytes(s.data[0x2E..0x30].try_into().unwrap());
                (anim, cmd, target, text)
            })
            .collect();
        assert!(
            begin_casts.contains(&(0x6F00_0002, 27313, 1, 30128)),
            "begin-cast 0x0139: anim 0x6F000002, cmd 27313, target player, text 30128; got {begin_casts:?}",
        );
        assert!(
            begin_casts
                .iter()
                .any(|(anim, cmd, ..)| *anim == 16781815 && *cmd == 27313),
            "completion damage X01 must ride battleAnimation 16781815; got {begin_casts:?}",
        );

        // Damage landed on the target.
        let player_hp = {
            let handle = ticker.registry.get(1).await.unwrap();
            let c = handle.character.read().await;
            c.get_hp()
        };
        assert!(
            player_hp < player_initial_hp,
            "thunder completion must damage the target ({player_initial_hp} → {player_hp})",
        );

        // Recast: first cast at t=100 → next no earlier than 8100. Two
        // chant-on packets across 8 s of ticks (t=100 and t=8100), never
        // a third.
        let chant_ons = substates.iter().filter(|b| **b == 0xF0).count();
        assert_eq!(
            chant_ons, 2,
            "exactly two casts in 8 s (recast = cast 2 s + 6 s); got {substates:?}",
        );
    }

    #[tokio::test]
    async fn cast_completion_resolves_spell_damage() {
        let (ticker, _zone) = setup_player_vs_npc().await;

        // Give the NPC a little defense so the damage math actually has
        // something to bite into, and queue a spell against it.
        {
            let handle = ticker.registry.get(2).await.unwrap();
            let mut chara = handle.character.write().await;
            chara
                .chara
                .mods
                .set(crate::actor::modifier::Modifier::Defense, 50.0);
        }
        {
            let handle = ticker.registry.get(1).await.unwrap();
            let mut chara = handle.character.write().await;
            let mut cmd = crate::battle::BattleCommand::new(100, "stone");
            cmd.cast_time_ms = 1_000;
            cmd.action_type = crate::battle::ActionType::Magic;
            cmd.command_type = crate::battle::CommandType::SPELL;
            cmd.base_potency = 300;
            assert!(chara.ai_container.internal_cast(2, cmd, 0));
        }

        let npc_handle = ticker.registry.get(2).await.unwrap();
        let initial_hp = npc_handle.character.read().await.get_hp();

        // One tick after cast finish resolves the spell.
        ticker.tick_once(1_500).await;

        let final_hp = npc_handle.character.read().await.get_hp();
        assert!(
            final_hp < initial_hp,
            "cast should have dropped npc hp from {initial_hp}, got {final_hp}"
        );
    }
}
