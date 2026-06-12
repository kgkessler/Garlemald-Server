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

//! Shared quest-mutation command application.
//!
//! Both the packet processor (`map-server/src/processor.rs`) and the
//! battle-path quest-hook dispatcher (`runtime/quest_hook.rs`) need to
//! drain `LuaCommand::Quest*` / `AddExp` / `AddGil` variants the same
//! way. This module holds the free-function version so neither caller
//! owns the logic — the processor forwards its runtime-safe arms here,
//! and `fire_on_kill_bnpc` can route hook-emitted commands through the
//! same pipeline without needing an `Arc<PacketProcessor>` threaded
//! through the battle dispatcher.
//!
//! Callers still need a `Database` / `ActorRegistry` / `WorldManager`
//! (for ENPC broadcasts) + optional `LuaEngine` (for auto-fire hooks
//! like `onStateChange` from a `QuestStartSequence` command).
//!
//! Login-flow-only commands (`SetLoginDirector`, `CreateDirector`,
//! `SetPos` during tutorial spawn) stay on the processor because they
//! mutate session state this module doesn't see. `KickEvent` has TWO
//! homes by design: the processor's login arm captures into
//! `session.pending_kick_event` (deferred emission at the end of the
//! zone-in bundle / pre-warp window), while the runtime arm here sends
//! immediately — the mid-flow `kickEventContinue` shape drained from
//! ticker/signal contexts where no bundle is coming. (#28 S1.1.)

#![allow(dead_code)]

use std::sync::Arc;

use crate::actor::quest::{AddEnpcOutcome, QuestEnpc};
use crate::database::Database;
use crate::lua::LuaCommandKind;
use crate::lua::LuaEngine;
use crate::runtime::actor_registry::{ActorHandle, ActorKindTag, ActorRegistry};
use crate::world_manager::WorldManager;

/// Whether `apply_runtime_lua_command` consumed the command. `false` means
/// the variant is login-scoped (processor handles it) or simply unrecognised.
pub type Handled = bool;

/// Dispatch a single `LuaCommand` through the runtime-safe command set
/// (Quest* mutations, AddExp, AddGil, Die/Revive) using only the four
/// long-lived Arcs every runtime subsystem holds. Returns `true` when
/// the command was recognised + applied; `false` when the caller should
/// fall back to its own handler (login-scoped variants).
pub async fn apply_runtime_lua_command(
    cmd: LuaCommandKind,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) -> Handled {
    use LuaCommandKind as LC;
    match cmd {
        LC::AddQuest {
            player_id,
            quest_id,
        } => {
            apply_add_quest(player_id, quest_id, registry, db, lua).await;
            true
        }
        LC::CompleteQuest {
            player_id,
            quest_id,
        } => {
            apply_complete_quest(player_id, quest_id, registry, db, lua).await;
            true
        }
        LC::AbandonQuest {
            player_id,
            quest_id,
        } => {
            apply_abandon_quest(player_id, quest_id, registry, db, lua).await;
            true
        }
        LC::QuestClearData {
            player_id,
            quest_id,
        } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| q.clear_data()).await;
            true
        }
        LC::QuestClearFlags {
            player_id,
            quest_id,
        } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| q.clear_flags()).await;
            true
        }
        LC::QuestSetFlag {
            player_id,
            quest_id,
            bit,
        } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| q.set_flag(bit)).await;
            true
        }
        LC::QuestClearFlag {
            player_id,
            quest_id,
            bit,
        } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| q.clear_flag(bit)).await;
            true
        }
        LC::QuestSetCounter {
            player_id,
            quest_id,
            idx,
            value,
        } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| {
                q.set_counter(idx as usize, value)
            })
            .await;
            true
        }
        LC::QuestIncCounter {
            player_id,
            quest_id,
            idx,
        } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| {
                q.inc_counter(idx as usize);
            })
            .await;
            true
        }
        LC::QuestDecCounter {
            player_id,
            quest_id,
            idx,
        } => {
            apply_quest_mutation(player_id, quest_id, registry, db, |q| {
                q.dec_counter(idx as usize);
            })
            .await;
            true
        }
        LC::QuestStartSequence {
            player_id,
            quest_id,
            sequence,
        } => {
            apply_quest_start_sequence(player_id, quest_id, sequence, registry, db, world, lua)
                .await;
            true
        }
        LC::QuestSetEnpc {
            player_id,
            quest_id,
            actor_class_id,
            quest_flag_type,
            is_talk_enabled,
            is_push_enabled,
            is_emote_enabled,
            is_spawned,
        } => {
            apply_quest_set_enpc(
                player_id,
                quest_id,
                actor_class_id,
                quest_flag_type,
                is_talk_enabled,
                is_push_enabled,
                is_emote_enabled,
                is_spawned,
                registry,
                world,
            )
            .await;
            true
        }
        LC::QuestUpdateEnpcs {
            player_id,
            quest_id,
        } => {
            apply_quest_update_enpcs(player_id, quest_id, registry, db, world, lua).await;
            true
        }
        LC::SetQuestComplete {
            player_id,
            quest_id,
            flag,
        } => {
            apply_set_quest_complete(player_id, quest_id, flag, registry, db).await;
            true
        }
        // `GetWorldManager():DoPlayerMoveInZone(...)` / `WarpToPosition`
        // from a quest/NPC hook. The login pipeline handles this command
        // in `PacketProcessor::apply_login_lua_command`, but the quest
        // drain (chat-resume `apply_event_script_commands` → here) had
        // NO arm — so man0u0's `onPush(EXIT_TRIGGER)` bounce and the
        // OpeningStoper "exit" bounce played their dialogue and then
        // silently dropped the move, letting the player walk out of the
        // Ul'dah opening zone (issue #26 retest).
        LC::WarpToPosition {
            actor_id,
            x,
            y,
            z,
            rotation,
            spawn_type,
        } => {
            apply_warp_to_position_runtime(
                actor_id, x, y, z, rotation, spawn_type, registry, world,
            )
            .await;
            true
        }
        // `player:SendGameMessage(...)` from a quest/NPC hook — e.g. the
        // opening stoppers' "caution" circle (text 34109 "off limits").
        LC::SendGameMessage {
            actor_id,
            text_id,
            log_type,
        } => {
            apply_send_game_message(actor_id, text_id, log_type, registry, world).await;
            true
        }
        LC::AddExp {
            actor_id,
            class_id,
            exp,
        } => {
            apply_add_exp(actor_id, class_id, exp, registry, db, Some(world), lua).await;
            true
        }
        LC::AddGil { actor_id, amount } => {
            apply_add_gil(actor_id, amount, db).await;
            true
        }
        LC::AddItem {
            actor_id,
            item_package,
            item_id,
            quantity,
        } => {
            apply_add_item(actor_id, item_package, item_id, quantity, db).await;
            // Tier 3 #13 — tick any accepted fieldcraft leves whose
            // objective targets this item. Runs after `apply_add_item`
            // so the DB write sequence is: inventory row → leve
            // progress. Short-circuits cleanly when the catalog isn't
            // installed (fresh-DB boot) or the player has no matching
            // active leve.
            if item_package == crate::inventory::PKG_NORMAL && quantity > 0 && item_id != 0 {
                let delta = quantity.min(u16::MAX as i32) as u16;
                advance_fieldcraft_leves(actor_id, item_id, delta, registry, db, lua).await;
            }
            true
        }
        LC::AddItemToRetainer {
            retainer_id,
            item_package,
            item_id,
            quantity,
        } => {
            apply_add_item_to_retainer(retainer_id, item_package, item_id, quantity, db).await;
            true
        }
        // Garlemald-Server #28 — `player:GetEquipment():Set(...)`. Equip
        // the bag items the per-class `equipClassItems` table names into
        // the matching gear slots, but only when the slot is currently
        // EMPTY (idempotent backfill — see `apply_equip_from_package`).
        LC::EquipFromPackage {
            player_id,
            gear_slots,
            src_positions,
            src_package,
        } => {
            apply_equip_from_package(
                player_id,
                &gear_slots,
                &src_positions,
                src_package,
                registry,
                db,
                world,
                lua,
            )
            .await;
            true
        }
        LC::HandInRegionalLeve { player_id, leve_id } => {
            let _ = apply_regional_leve_hand_in(player_id, leve_id, registry, db, lua).await;
            true
        }
        LC::AcceptRegionalLeve {
            player_id,
            leve_id,
            difficulty,
        } => {
            let _ =
                apply_accept_regional_leve(player_id, leve_id, difficulty, registry, db, lua).await;
            true
        }
        LC::PurchaseRetainerBazaarItem {
            buyer_id,
            retainer_id,
            server_item_id,
        } => {
            let _ = apply_purchase_retainer_bazaar_item(buyer_id, retainer_id, server_item_id, db)
                .await;
            true
        }
        LC::TryStatus {
            source_actor_id,
            target_actor_id,
            status_id,
            duration_s,
            magnitude,
            tick_ms,
            tier,
        } => {
            let _ = apply_try_status(
                source_actor_id,
                target_actor_id,
                status_id,
                duration_s,
                magnitude,
                tick_ms,
                tier,
                registry,
                db,
                world,
                lua,
            )
            .await;
            true
        }
        LC::QuestOnNotice {
            player_id,
            quest_id,
        } => {
            apply_quest_on_notice(player_id, quest_id, registry, db, world, lua).await;
            true
        }
        // --- Director outbox ops --------------------------------------
        //
        // Leve-side bindings — `director:StartGuildleve()`,
        // `EndGuildleve`, etc. The runtime drain needs to handle
        // these because scheduler-resumed director `main` coroutines
        // (parked on `wait(N)`) emit them from inside
        // `runtime::ticker::tick_once`, where the PacketProcessor's
        // `apply_login_lua_command` isn't reachable. Same lock +
        // drain shape as the processor's `apply_director_outbox_op`
        // helper.
        LC::EndGuildleve {
            director_actor_id,
            was_completed,
        } => {
            let now = common::utils::unix_timestamp() as u32;
            apply_director_outbox_op(
                director_actor_id,
                "EndGuildleve",
                registry,
                db,
                world,
                |gld, ob| gld.end_guildleve(now, was_completed, ob),
            )
            .await;
            true
        }
        LC::StartGuildleve { director_actor_id } => {
            let now = common::utils::unix_timestamp() as u32;
            apply_director_outbox_op(
                director_actor_id,
                "StartGuildleve",
                registry,
                db,
                world,
                |gld, ob| gld.start_guildleve(now, ob),
            )
            .await;
            true
        }
        LC::AbandonGuildleve { director_actor_id } => {
            let now = common::utils::unix_timestamp() as u32;
            apply_director_outbox_op(
                director_actor_id,
                "AbandonGuildleve",
                registry,
                db,
                world,
                |gld, ob| gld.abandon_guildleve(now, ob),
            )
            .await;
            true
        }
        LC::UpdateAimNumNow {
            director_actor_id,
            index,
            value,
        } => {
            apply_director_outbox_op(
                director_actor_id,
                "UpdateAimNumNow",
                registry,
                db,
                world,
                |gld, ob| gld.update_aim_num_now(index, value, ob),
            )
            .await;
            true
        }
        LC::UpdateUiState {
            director_actor_id,
            index,
            value,
        } => {
            apply_director_outbox_op(
                director_actor_id,
                "UpdateUIState",
                registry,
                db,
                world,
                |gld, ob| gld.update_ui_state(index, value, ob),
            )
            .await;
            true
        }
        LC::UpdateMarkers {
            director_actor_id,
            index,
            x,
            y,
            z,
        } => {
            apply_director_outbox_op(
                director_actor_id,
                "UpdateMarkers",
                registry,
                db,
                world,
                |gld, ob| gld.update_marker(index, x, y, z, ob),
            )
            .await;
            true
        }
        LC::SyncAllInfo { director_actor_id } => {
            apply_director_outbox_op(
                director_actor_id,
                "SyncAllInfo",
                registry,
                db,
                world,
                |gld, ob| gld.sync_all(ob),
            )
            .await;
            true
        }
        LC::AddRetainerBazaarItem {
            retainer_id,
            item_id,
            quantity,
            quality,
            price_gil,
        } => {
            apply_add_retainer_bazaar_item(retainer_id, item_id, quantity, quality, price_gil, db)
                .await;
            true
        }
        LC::SetActorMod {
            actor_id,
            modifier_key,
            value,
        } => {
            apply_set_actor_mod(actor_id, modifier_key, value, registry).await;
            true
        }
        LC::ActorEngage {
            actor_id,
            target_actor_id,
        } => {
            apply_actor_engage(actor_id, target_actor_id, registry, world).await;
            true
        }
        LC::ChangeState {
            actor_id,
            main_state,
        } => {
            apply_change_state(actor_id, main_state, registry, world).await;
            true
        }
        LC::DirectorAddMember {
            director_actor_id,
            member_actor_id,
        } => {
            apply_director_add_member(director_actor_id, member_actor_id, world).await;
            true
        }
        LC::PartyAddMember {
            leader_actor_id,
            member_actor_id,
        } => {
            apply_party_add_member(leader_actor_id, member_actor_id, registry, world).await;
            true
        }
        LC::PlayAnimation {
            actor_id,
            animation_id,
        } => {
            apply_play_animation(actor_id, animation_id, registry, world).await;
            true
        }
        LC::ChangeMusic {
            player_id,
            music_id,
        } => {
            apply_change_music(player_id, music_id, registry, world).await;
            true
        }
        LC::SendDataPacket { player_id, params } => {
            apply_send_data_packet(player_id, &params, registry, world).await;
            true
        }
        LC::DespawnActor { zone_id, actor_id } => {
            apply_despawn_actor(zone_id, actor_id, registry, world).await;
            true
        }
        LC::HateContainerAddBaseHate {
            actor_id,
            target_actor_id,
        } => {
            apply_hate_container_add_base_hate(actor_id, target_actor_id, registry).await;
            true
        }
        LC::MoveActorToPosition {
            actor_id,
            x,
            y,
            z,
            rotation,
            move_state,
        } => {
            apply_move_actor_to_position(actor_id, x, y, z, rotation, move_state, registry, world)
                .await;
            true
        }
        LC::SetActorTargetAnimated {
            source_actor_id,
            target_actor_id,
        } => {
            apply_set_actor_target_animated(source_actor_id, target_actor_id, registry, world)
                .await;
            true
        }
        LC::KickEvent {
            player_id,
            actor_id,
            trigger,
            args,
        } => {
            apply_kick_event(player_id, actor_id, &trigger, &args, registry, world).await;
            true
        }
        LC::DoZoneChange {
            player_id,
            zone_id,
            private_area,
            private_area_type,
            spawn_type,
            x,
            y,
            z,
            rotation,
        } => {
            apply_do_zone_change(
                player_id,
                zone_id,
                private_area,
                private_area_type,
                spawn_type,
                x,
                y,
                z,
                rotation,
                registry,
                db,
                world,
                lua,
            )
            .await;
            true
        }
        LC::ContentFinished {
            parent_zone_id,
            area_name,
        } => {
            apply_content_finished(parent_zone_id, &area_name, registry, world, lua).await;
            true
        }
        _ => false,
    }
}

/// Phase C3 — port of C# `Controller::Engage(target)` /
/// `BattleNpcController::Engage`. Pushes a fresh `BattleState::Attack`
/// onto the actor's AIContainer (target locked in, swing-clock armed
/// against `Character::get_attack_delay_ms`). The AIContainer's
/// per-tick `update` loop emits the `BattleEvent::Engage` /
/// `BattleEvent::ResolveAutoAttack` events from there.
///
/// Quietly no-ops for actors not in the registry or actors already
/// engaged — re-engaging the same target would clobber the existing
/// state's swing clock, restarting the swing window. The C# engage
/// path has the same gate (`if (IsEngaged) return false`).
async fn apply_actor_engage(
    actor_id: u32,
    target_actor_id: u32,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    if target_actor_id == 0 {
        tracing::debug!(
            actor = format!("0x{actor_id:08X}"),
            "ActorEngage skipped — target_actor_id == 0",
        );
        return;
    }
    let Some(handle) = registry.get(actor_id).await else {
        tracing::debug!(
            actor = format!("0x{actor_id:08X}"),
            target = format!("0x{target_actor_id:08X}"),
            "ActorEngage skipped — actor not in registry",
        );
        return;
    };
    // Shared battle-clock anchor (`runtime/clock.rs`) — the ticker drives
    // `AIContainer::update` in this domain. Arming the swing clock with
    // epoch ms (the pre-#28 bug) parked `is_attack_ready` ~56 years out:
    // script-engaged allies entered the engaged state but never swung.
    let now_ms = crate::runtime::clock::server_now_ms();
    let started = {
        let mut c = handle.character.write().await;
        let delay = c.get_attack_delay_ms();
        let started = c
            .ai_container
            .internal_engage(target_actor_id, now_ms, delay);
        // Seed hate toward the target. AI-controlled actors (allies driven by
        // `allyGlobal.EngageTarget`, scripted BattleNpcs) run `do_combat_tick`,
        // which `should_deaggro`s the instant `most_hated()` is None. Attacking
        // only seeds hate on the *defender* (`resolve_auto_attack`), so without
        // this the engager would emit a Disengage on its very next tick and spin
        // an engage/disengage loop while its AttackState keeps swinging. Mirrors
        // the `BattleEvent::Engage` hate-seed in `dispatcher.rs`. (Garlemald #28.)
        c.hate.update_hate(target_actor_id, 1);
        tracing::debug!(
            actor = format!("0x{actor_id:08X}"),
            target = format!("0x{target_actor_id:08X}"),
            delay,
            started,
            "ActorEngage applied",
        );
        started
    };
    // Script-driven engages (allyGlobal.EngageTarget) bypass the
    // BattleEvent::Engage dispatch arm, so emit the State trio here —
    // the per-NPC mode-change action pmeteor broadcasts at engage time
    // that arms the client's combat presentation for this actor (see
    // `dispatcher::emit_change_state_trio`). Fresh engages only — a
    // re-engage no-op shouldn't re-spam the trio.
    if started && let Some(zone_arc) = world.zone(handle.zone_id).await {
        crate::runtime::dispatcher::emit_change_state_trio(
            actor_id,
            crate::actor::MAIN_STATE_ACTIVE,
            registry,
            world,
            &zone_arc,
        )
        .await;
    }
}

/// `actor:ChangeState(main_state)` — port of pmeteor `Actor.cs::ChangeState`.
/// Updates the actor's stored `current_main_state` AND broadcasts the
/// `0x0134 SetActorState` packet to nearby players so the client renders
/// the new pose (e.g. main_state 2 = active/combat stance — Yda + tutorial
/// wolves in `SimpleContent30010` need this to stand up instead of lying in
/// the default passive state). Without this applier the LuaCommand falls
/// through to the unhandled-tag log and the actor stays in its spawn pose.
async fn apply_change_state(
    actor_id: u32,
    main_state: u16,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(actor_id).await else {
        tracing::debug!(
            actor = format!("0x{actor_id:08X}"),
            main_state,
            "ChangeState skipped — actor not in registry",
        );
        return;
    };
    let Some(zone_arc) = world.zone(handle.zone_id).await else {
        tracing::debug!(
            actor = format!("0x{actor_id:08X}"),
            zone = handle.zone_id,
            "ChangeState skipped — zone not loaded",
        );
        return;
    };
    // pmeteor's State-flag flush (Character.cs PostUpdate:411-419) ships a
    // TRIO, not just the 0x0134: SetActorState + CommandResultX00 (anim
    // 0x72000062) + CommandResultX01 (anim 0x7C000062, command 21001
    // Activate, one self-hit). The 1.x client plays the draw-/sheathe-
    // weapon animation (and arms per-actor combat presentation) off the
    // X00/X01 battle actions — a bare 0x0134 changes the logical state
    // invisibly. The shared helper owns the byte shape, the pre-warp
    // content-roster suppression, and the corpse guard. (Garlemald #28.)
    crate::runtime::dispatcher::emit_change_state_trio(
        actor_id, main_state, registry, world, &zone_arc,
    )
    .await;
}

/// `director:AddMember(actor)` — populates the director's transient
/// roster (`session.transient_director_members[director_actor_id]`).
/// The runtime ticker reads this roster to build the script's
/// `area:GetPlayers() / :GetAllies() / :GetMonsters()` iterators —
/// without this applier the iterators yield empty tables and the
/// content script's ally/enemy AI loops never fire (e.g.
/// `SimpleContent30010.lua::onUpdate`'s `EngageTarget` branch never
/// runs, so Yda/Papalymo never engage the wolves).
///
/// The Lua command doesn't carry a session_id; we resolve it by
/// scanning sessions for the one whose active content script targets
/// this director. Each player has at most one active content script,
/// so the scan terminates quickly.
async fn apply_director_add_member(
    director_actor_id: u32,
    member_actor_id: u32,
    world: &WorldManager,
) {
    if director_actor_id == 0 || member_actor_id == 0 {
        return;
    }
    let sessions = world.all_sessions().await;
    let mut target = None;
    for snap in sessions {
        if let Some(active) = snap.active_content_script.as_ref()
            && active.director_actor_id == director_actor_id
        {
            target = Some(snap);
            break;
        }
    }
    let Some(mut snap) = target else {
        tracing::debug!(
            director = format!("0x{director_actor_id:08X}"),
            member = format!("0x{member_actor_id:08X}"),
            "DirectorAddMember skipped — no session with this content director",
        );
        return;
    };
    let session_id = snap.id;
    let roster = snap
        .transient_director_members
        .entry(director_actor_id)
        .or_default();
    if !roster.contains(&member_actor_id) {
        roster.push(member_actor_id);
    }
    let count = roster.len();
    world.upsert_session(snap).await;
    tracing::debug!(
        session = session_id,
        director = format!("0x{director_actor_id:08X}"),
        member = format!("0x{member_actor_id:08X}"),
        roster_size = count,
        "DirectorAddMember applied",
    );
}

/// `currentParty:AddMember(actor)` — appends `member_actor_id` to the
/// leader's transient party roster (`session.transient_party_members`)
/// and re-emits the GroupHeader / GroupMembersBegin / X08 / End trio to
/// the leader's client so the in-game party panel shows the new member.
/// Used by `SimpleContent30010.lua::onCreate` to add Yda + Papalymo
/// (battle-NPC allies) to the player's party UI — without this they
/// never appear in the party panel even though they spawn fine in the
/// world.
async fn apply_party_add_member(
    leader_actor_id: u32,
    member_actor_id: u32,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    if leader_actor_id == 0 || member_actor_id == 0 {
        return;
    }
    let Some(leader_handle) = registry.get(leader_actor_id).await else {
        tracing::debug!(
            leader = format!("0x{leader_actor_id:08X}"),
            "PartyAddMember skipped — leader not in registry",
        );
        return;
    };
    let session_id = leader_handle.session_id;
    if session_id == 0 {
        tracing::debug!(
            leader = format!("0x{leader_actor_id:08X}"),
            "PartyAddMember skipped — leader has no session",
        );
        return;
    }
    let Some(mut snap) = world.session(session_id).await else {
        tracing::debug!(
            session = session_id,
            "PartyAddMember skipped — session not found",
        );
        return;
    };
    if !snap.transient_party_members.contains(&member_actor_id) {
        snap.transient_party_members.push(member_actor_id);
    }
    let roster_ids: Vec<u32> = snap.transient_party_members.clone();
    world.upsert_session(snap).await;

    // Build the roster: leader first, then the transient members.
    // Names come from each actor's `character.base.display_name()`.
    let leader_name = {
        let c = leader_handle.character.read().await;
        c.base.display_name().to_string()
    };
    let mut members: Vec<crate::packets::send::groups::GroupMember> = Vec::new();
    members.push(crate::packets::send::groups::GroupMember {
        actor_id: leader_actor_id,
        localized_name: -1,
        unknown2: 0,
        flag1: false,
        is_online: true,
        name: leader_name,
    });
    for &mid in &roster_ids {
        let name = if let Some(h) = registry.get(mid).await {
            let c = h.character.read().await;
            c.base.display_name().to_string()
        } else {
            format!("bnpc_{mid:08X}")
        };
        members.push(crate::packets::send::groups::GroupMember {
            actor_id: mid,
            localized_name: -1,
            unknown2: 0,
            flag1: false,
            is_online: true,
            name,
        });
    }

    // Emit the trio. Mirrors the solo-party block in
    // `world_manager.rs::send_zone_in_bundle` (lines ~1885-1942):
    // group_index = SOLO_FLAG | leader_actor_id, GROUP_TYPE_PARTY=0x2711.
    const PARTY_SOLO_SELF_FLAG: u64 = 0x8000_0000_0000_0000;
    const GROUP_TYPE_PARTY: u32 = 0x2711;
    let group_index: u64 = PARTY_SOLO_SELF_FLAG | (leader_actor_id as u64);
    let location_code = leader_handle.zone_id as u64;
    let sequence_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default();

    let mut offset = 0usize;
    let pkts = vec![
        crate::packets::send::groups::build_group_header(
            leader_actor_id,
            location_code,
            sequence_id,
            group_index,
            GROUP_TYPE_PARTY,
            -1,
            "",
            members.len() as u32,
        ),
        crate::packets::send::groups::build_group_members_begin(
            leader_actor_id,
            location_code,
            sequence_id,
            group_index,
            members.len() as u32,
        ),
        crate::packets::send::groups::build_group_members_x08(
            leader_actor_id,
            location_code,
            sequence_id,
            &members,
            &mut offset,
        ),
        crate::packets::send::groups::build_group_members_end(
            leader_actor_id,
            location_code,
            sequence_id,
            group_index,
        ),
    ];

    let Some(client) = world.client(session_id).await else {
        tracing::debug!(
            session = session_id,
            "PartyAddMember: roster updated but client gone — wire emit skipped",
        );
        return;
    };
    for mut sub in pkts {
        sub.set_target_id(session_id);
        client.send_bytes(sub.to_bytes()).await;
    }

    tracing::debug!(
        leader = format!("0x{leader_actor_id:08X}"),
        member = format!("0x{member_actor_id:08X}"),
        roster_size = members.len(),
        "PartyAddMember applied (transient roster updated + party trio re-emitted)",
    );
}

/// `actor:PlayAnimation(animation_id)` — port of C#
/// `Actor::PlayAnimation`. Broadcasts the `0x00E0`
/// PlayAnimationOnActor packet to nearby players so the client triggers
/// the actor's animation sequence (used by cinematics and quest events).
async fn apply_play_animation(
    actor_id: u32,
    animation_id: u32,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(actor_id).await else {
        tracing::debug!(
            actor = format!("0x{actor_id:08X}"),
            animation_id,
            "PlayAnimation skipped — actor not in registry",
        );
        return;
    };
    let Some(zone_arc) = world.zone(handle.zone_id).await else {
        return;
    };
    let sub = crate::packets::send::actor::build_play_animation_on_actor(actor_id, animation_id);
    let recipients = crate::runtime::broadcast::broadcast_around_actor(
        world,
        registry,
        &zone_arc,
        actor_id,
        sub.to_bytes(),
    )
    .await;
    tracing::debug!(
        actor = format!("0x{actor_id:08X}"),
        animation_id,
        recipients,
        "PlayAnimation applied",
    );
}

/// `player:ChangeMusic(music_id)` — port of C# `Player::ChangeMusic`.
/// Sends `0x006D SetMusic` to the player's client to override the zone's
/// background music (used for combat themes, scripted scenes, etc.).
/// Music track mode is 0 (immediate switch); MUSIC_CROSSFADE etc. are
/// available constants but not currently parameterised — pmeteor's Lua
/// API takes a single id.
async fn apply_change_music(
    player_id: u32,
    music_id: u16,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(player_id).await else {
        tracing::debug!(
            player = format!("0x{player_id:08X}"),
            music_id,
            "ChangeMusic skipped — player not in registry",
        );
        return;
    };
    let session_id = handle.session_id;
    if session_id == 0 {
        return;
    }
    let Some(client) = world.client(session_id).await else {
        return;
    };
    let mut sub = crate::packets::send::misc::build_set_music(player_id, music_id, 0);
    sub.set_target_id(session_id);
    client.send_bytes(sub.to_bytes()).await;
    tracing::debug!(
        player = format!("0x{player_id:08X}"),
        music_id,
        "ChangeMusic applied",
    );
}

/// `player:SendDataPacket(dataType, ...)` — port of C#
/// `Player::SendDataPacket` (emits a 0x0133 GenericDataPacket). Marshals the
/// Lua args to a LuaParam list and ships them to the player's client. This is
/// what `tutorial.lua`'s `startTutorialMode` / `openTutorialWidget` /
/// `showTutorialSuccessWidget` / `closeTutorialWidget` and `"attention"`
/// messages ride on. The SEQ_005 director's first action is
/// `startTutorialMode(player)` = `SendDataPacket(9)`, which arms the client's
/// active-mode (F / draw-weapon) toggle; without it the client never lets the
/// player press F and the director hangs on `waitForSignal("playerActive")`.
///
/// IMPORTANT — target_id MUST be the player's ACTOR id, not the session id.
/// The 1.x client silently drops 0x0133 subpackets whose
/// `SubPacketHeader.target_id` != the receiving actor's id (the same gotcha
/// `send_quest_journal_data` documents for the qtdata 0x0133). `build_set_music`
/// above gets away with `set_target_id(session_id)` because 0x006D isn't gated
/// that way; 0x0133 is. (Garlemald-Server #28.)
async fn apply_send_data_packet(
    player_id: u32,
    params: &[crate::lua::command::LuaCommandArg],
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(player_id).await else {
        tracing::debug!(
            player = format!("0x{player_id:08X}"),
            "SendDataPacket skipped — player not in registry",
        );
        return;
    };
    let session_id = handle.session_id;
    if session_id == 0 {
        return;
    }
    let Some(client) = world.client(session_id).await else {
        return;
    };
    let lua_params: Vec<common::luaparam::LuaParam> = params
        .iter()
        .map(crate::event::lua_bridge::arg_to_lua_param)
        .collect();
    let mut sub = crate::packets::send::player::build_generic_data(player_id, &lua_params);
    sub.set_target_id(player_id);
    client.send_bytes(sub.to_bytes()).await;
    tracing::debug!(
        player = format!("0x{player_id:08X}"),
        params = params.len(),
        "SendDataPacket applied (0x0133 GenericData)",
    );
}

/// `zone:DespawnActor(actor_id)` — port of C# `Zone::DespawnActor`.
/// Broadcasts `0x00CB RemoveActor` to nearby players and removes the
/// actor from the registry. Used by cinematics and content cleanup.
pub(crate) async fn apply_despawn_actor(
    _zone_id: u32,
    actor_id: u32,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(actor_id).await else {
        tracing::debug!(
            actor = format!("0x{actor_id:08X}"),
            "DespawnActor skipped — actor not in registry",
        );
        return;
    };
    let zone = handle.zone_id;
    let Some(zone_arc) = world.zone(zone).await else {
        return;
    };
    // Disarm the actor's event-condition triggers BEFORE the remove.
    // The 1.23b client does NOT clear armed push circles when an actor
    // is deleted — live-confirmed in the Ul'dah opener teardown (#26):
    // the arena fence's outwards "exit" circle (radius 40) survived the
    // stopper's RemoveActor, fired against the ghost when the SEQ_010
    // warp repositioned the player >40y away, and the orphan event
    // exchange (client EventStart vs our EndEvent echoes, mid
    // zone-change) hard-crashed the client ~3 s into the warp.
    let conditions = {
        let c = handle.character.read().await;
        c.base.event_conditions.clone()
    };
    if !conditions.is_empty() {
        for sub in crate::packets::send::build_actor_event_status_packets(
            actor_id,
            &conditions,
            false,
            false,
            Some(false),
            false,
        ) {
            crate::runtime::broadcast::broadcast_around_actor(
                world,
                registry,
                &zone_arc,
                actor_id,
                sub.to_bytes(),
            )
            .await;
        }
    }
    let sub = crate::packets::send::actor::build_remove_actor(actor_id);
    let recipients = crate::runtime::broadcast::broadcast_around_actor(
        world,
        registry,
        &zone_arc,
        actor_id,
        sub.to_bytes(),
    )
    .await;
    // Remove from registry AND the zone's spatial grid after the
    // broadcast (so spatial-grid lookups during fan-out still see it).
    // The grid removal mirrors the spawn appliers' `zone.core.add_actor`
    // insertion — without it a despawned actor leaks a ghost entry that
    // `actors_around` keeps yielding to AI/broadcast radius queries.
    // (#28 S1.3.)
    registry.remove(actor_id).await;
    {
        let mut zone_write = zone_arc.write().await;
        let mut ob = crate::zone::outbox::AreaOutbox::new();
        zone_write.core.remove_actor(actor_id, &mut ob);
    }
    tracing::debug!(
        actor = format!("0x{actor_id:08X}"),
        zone,
        recipients,
        "DespawnActor applied",
    );
}

/// Phase C3 — port of C# `HateContainer::AddBaseHate(target)`. Inserts
/// a zero-enmity hate entry for `target_actor_id` on `actor_id`'s
/// hate container so `most_hated()` resolves to this target. The
/// damage path (`update_hate(target, dmg)`) is responsible for
/// incrementing enmity after each hit; this primes the container.
async fn apply_hate_container_add_base_hate(
    actor_id: u32,
    target_actor_id: u32,
    registry: &ActorRegistry,
) {
    if target_actor_id == 0 {
        return;
    }
    let Some(handle) = registry.get(actor_id).await else {
        tracing::debug!(
            actor = format!("0x{actor_id:08X}"),
            "HateContainerAddBaseHate skipped — actor not in registry",
        );
        return;
    };
    let mut c = handle.character.write().await;
    c.hate.add_base_hate(target_actor_id);
    tracing::debug!(
        actor = format!("0x{actor_id:08X}"),
        target = format!("0x{target_actor_id:08X}"),
        "HateContainerAddBaseHate applied",
    );
}

/// `actor:MoveTo(x, y, z, rotation, move_state)` — port of C#
/// `Actor.SetPos(x, y, z, rot, instant=false, ...)`'s broadcast
/// branch. Updates the actor's stored position AND fans a 0x00CF
/// MoveActorToPositionPacket out to nearby players so they see the
/// actor walk/run/sprint to the new coords. Mirrors pmeteor
/// `Actor.cs:665`:
/// ```csharp
/// CurrentArea.BroadcastPacketAroundActor(this,
///     MoveActorToPositionPacket.BuildPacket(Id, x, y, z, rot, moveState));
/// ```
///
/// `move_state` values follow the wiki + pmeteor
/// `UpdatePlayerPositionPacket.cs:33` (0 = standing, 1 = walking,
/// 2 = running). The dispatch is best-effort: if the actor isn't in
/// the registry or has no zone we just log and drop, matching the C#
/// implicit silent-skip when CurrentArea is null.
#[allow(clippy::too_many_arguments)]
async fn apply_move_actor_to_position(
    actor_id: u32,
    x: f32,
    y: f32,
    z: f32,
    rotation: f32,
    move_state: u16,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(actor_id).await else {
        tracing::debug!(
            actor = format!("0x{actor_id:08X}"),
            "MoveActorToPosition skipped — actor not in registry",
        );
        return;
    };
    // 1. Update the character's stored position. This makes
    //    subsequent position reads (other Lua scripts, neighbour
    //    spatial-grid resolution) see the new coords even before
    //    the move animation finishes.
    {
        let mut c = handle.character.write().await;
        c.base.position_x = x;
        c.base.position_y = y;
        c.base.position_z = z;
        c.base.rotation = rotation;
    }
    // 2. Broadcast the 0x00CF MoveActorToPositionPacket to nearby
    //    players. Use the actor's zone (read from the handle) to
    //    scope the fan-out.
    let Some(zone_arc) = world.zone(handle.zone_id).await else {
        tracing::debug!(
            actor = format!("0x{actor_id:08X}"),
            zone = handle.zone_id,
            "MoveActorToPosition skipped — zone not loaded",
        );
        return;
    };
    // 2b. Re-insert into the zone's spatial grid — `actors_around`
    //     (the AI arena + broadcast radius source) reads the grid, not
    //     `Character.base.position_*`, so without this every scripted
    //     move desyncs aggro/visibility from the authoritative
    //     position. Mirrors the player path
    //     (`world_manager.rs::update_actor_position`). (#28 S0.3.)
    {
        let mut zone_write = zone_arc.write().await;
        let mut ob = crate::zone::outbox::AreaOutbox::new();
        zone_write
            .core
            .update_actor_position(actor_id, common::Vector3::new(x, y, z), &mut ob);
    }
    let sub = crate::packets::send::actor::build_move_actor_to_position(
        actor_id, x, y, z, rotation, move_state,
    );
    let recipients = crate::runtime::broadcast::broadcast_around_actor(
        world,
        registry,
        &zone_arc,
        actor_id,
        sub.to_bytes(),
    )
    .await;
    tracing::debug!(
        actor = format!("0x{actor_id:08X}"),
        x,
        y,
        z,
        rotation,
        move_state,
        recipients,
        "MoveActorToPosition applied",
    );
}

/// `actor:LookAt(target)` — broadcasts a 0x00D3
/// SetActorTargetAnimatedPacket so the client animates the actor's
/// head/body turning to face the target. No position state mutation;
/// the rotation that follows is computed client-side from the
/// source actor's position and the target's position.
///
/// Auto-fires in pmeteor when a player sends an inbound SetTarget;
/// here we expose it as an explicit Lua call so cinematic scripts
/// can choreograph look-at directly between dialogue beats.
async fn apply_set_actor_target_animated(
    source_actor_id: u32,
    target_actor_id: u32,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(source_actor_id).await else {
        tracing::debug!(
            actor = format!("0x{source_actor_id:08X}"),
            "SetActorTargetAnimated skipped — source actor not in registry",
        );
        return;
    };
    let Some(zone_arc) = world.zone(handle.zone_id).await else {
        tracing::debug!(
            actor = format!("0x{source_actor_id:08X}"),
            zone = handle.zone_id,
            "SetActorTargetAnimated skipped — zone not loaded",
        );
        return;
    };
    let sub = crate::packets::send::actor::build_set_actor_target_animated(
        source_actor_id,
        target_actor_id,
    );
    let recipients = crate::runtime::broadcast::broadcast_around_actor(
        world,
        registry,
        &zone_arc,
        source_actor_id,
        sub.to_bytes(),
    )
    .await;
    tracing::debug!(
        actor = format!("0x{source_actor_id:08X}"),
        target = format!("0x{target_actor_id:08X}"),
        recipients,
        "SetActorTargetAnimated applied",
    );
}

/// `player:KickEvent(director, trigger, ...)` drained from a runtime /
/// event-bridge context (ticker resume, signal resume, death path) —
/// the mid-flow `kickEventContinue` shape. Builds + sends the 0x012F
/// immediately, copying the post-zone-in direct-dispatch shape
/// (`processor::apply_post_zone_in_lua_command`). This is deliberately
/// NOT the login-bundle capture: `apply_login_lua_command`'s KickEvent
/// arm defers emission to the zone-in bundle / pre-warp window, which
/// never comes for a director resumed mid-flow. `event_type` is always
/// 5 — the "noticeEvent" tag the client's kick receiver[+0x80] gate
/// requires; any other value is a silent client-side drop. The
/// event-bridge translator keeps its deliberate KickEvent exclusion
/// (`event/lua_bridge.rs`); this arm is the one runtime home. (#28 S1.1.)
async fn apply_kick_event(
    player_id: u32,
    actor_id: u32,
    trigger: &str,
    args: &[crate::lua::command::LuaCommandArg],
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    if actor_id == 0 {
        tracing::debug!(
            player = format!("0x{player_id:08X}"),
            trigger,
            "KickEvent skipped — no owner actor id",
        );
        return;
    }
    let Some(handle) = registry.get(player_id).await else {
        tracing::debug!(
            player = format!("0x{player_id:08X}"),
            trigger,
            "KickEvent skipped — player not in registry",
        );
        return;
    };
    let session_id = handle.session_id;
    if session_id == 0 {
        tracing::debug!(
            player = format!("0x{player_id:08X}"),
            trigger,
            "KickEvent skipped — player has no session",
        );
        return;
    }
    let Some(client) = world.client(session_id).await else {
        tracing::warn!(
            session = session_id,
            trigger,
            "KickEvent dropped — no client handle",
        );
        return;
    };
    let lua_params: Vec<common::luaparam::LuaParam> = args
        .iter()
        .map(crate::event::lua_bridge::arg_to_lua_param)
        .collect();
    let mut sub = crate::packets::send::events::build_kick_event(
        player_id,
        actor_id,
        trigger,
        5,
        &lua_params,
    );
    sub.set_target_id(session_id);
    client.send_bytes(sub.to_bytes()).await;
    tracing::info!(
        session = session_id,
        trigger_actor = format!("0x{player_id:08X}"),
        owner_actor = format!("0x{actor_id:08X}"),
        event = %trigger,
        args = lua_params.len(),
        "KickEvent dispatched directly to client (runtime drain)",
    );
}

/// `zone:ContentFinished()` / `area:ContentFinished()` — full teardown
/// of the active scripted content. pmeteor's `PrivateAreaContent.
/// ContentFinished` only sets `isContentFinished` and defers the real
/// cleanup to the next `DoZoneChange`'s CheckDestroy sweep; garlemald's
/// fixed director command order (ContentFinished immediately followed
/// by DoZoneChange in the same drained batch) lets us be eager here
/// (#28 S1.3, report E §3.2):
///
/// 1. resolve the owning session by `active_content_script` scan
///    (`area_name` is empty from the LuaZone binding — the live area is
///    whatever the session carries);
/// 2. despawn the content NPCs: director-roster members with kind ∈
///    {BattleNpc, Ally} plus every id the spawn appliers recorded on
///    `spawned_actor_ids` (catches the `openingstoper` trigger that is
///    SpawnActor'd but never AddMember'd). RemoveActor packets pre-warp
///    are harmless — the warp's DeleteAllActors wipes client state;
/// 3. clear the director roster + (defensively) the transient party;
/// 4. `active_content_script = None` — stops the 500 ms onUpdate driver
///    and the 0x0133 `/_init` content branch;
/// 5. clear the player's tutorial `MinimumHpLock` (set in onCreate;
///    mods survive warps — without this the player is unkillable
///    forever);
/// 6. purge the player's parked coroutines so a stale `_WAIT_EVENT`
///    director can't resume into the torn-down instance.
///
/// Wire: nothing mandatory — the reference capture carries zero 0x0143
/// DeleteGroup in 60,232 lines; the warp's DeleteAllActors resets
/// client group state and the post-warp zone-in bundle re-sends the
/// solo party trio.
pub(crate) async fn apply_content_finished(
    parent_zone_id: u32,
    area_name: &str,
    registry: &ActorRegistry,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    // 1. Owning session — the one whose active content matches this
    //    parent zone (+ area name when the caller knew it).
    let mut owner: Option<crate::data::Session> = None;
    for snap in world.all_sessions().await {
        if let Some(active) = snap.active_content_script.as_ref()
            && active.parent_zone_id == parent_zone_id
            && (area_name.is_empty() || active.area_name == area_name)
        {
            owner = Some(snap);
            break;
        }
    }
    let Some(mut snap) = owner else {
        tracing::info!(
            parent_zone = parent_zone_id,
            area = %area_name,
            "ContentFinished: no session with active content — nothing to tear down",
        );
        return;
    };
    let session_id = snap.id;
    let Some(active) = snap.active_content_script.clone() else {
        return;
    };
    let director_id = active.director_actor_id;

    // 2. Despawn set: roster content-NPCs (never the player or the
    //    director) + the spawn appliers' recorded ids, deduped.
    let mut despawn: Vec<u32> = Vec::new();
    if let Some(roster) = snap.transient_director_members.get(&director_id) {
        for &member_id in roster {
            let Some(member) = registry.get(member_id).await else {
                continue;
            };
            if matches!(member.kind, ActorKindTag::BattleNpc | ActorKindTag::Ally)
                && !despawn.contains(&member_id)
            {
                despawn.push(member_id);
            }
        }
    }
    for &spawned_id in &active.spawned_actor_ids {
        if !despawn.contains(&spawned_id) {
            despawn.push(spawned_id);
        }
    }
    let despawn_count = despawn.len();
    for actor_id in despawn {
        apply_despawn_actor(parent_zone_id, actor_id, registry, world).await;
    }

    // 3 + 4. Roster + content-script clears.
    snap.transient_director_members.remove(&director_id);
    snap.transient_party_members.clear();
    snap.active_content_script = None;
    world.upsert_session(snap).await;

    // 5. Player teardown — MinimumHpLock off.
    let player_id = match registry.by_session(session_id).await {
        Some(player) => {
            {
                let mut c = player.character.write().await;
                c.chara.mods.set(crate::actor::Modifier::MinimumHpLock, 0.0);
            }
            player.actor_id
        }
        None => 0,
    };

    // 6. Scheduler purge.
    let purged = match (player_id, lua) {
        (0, _) | (_, None) => 0,
        (pid, Some(engine)) => engine
            .scheduler()
            .lock()
            .map(|mut s| s.purge_owner(pid))
            .unwrap_or(0),
    };

    tracing::info!(
        session = session_id,
        parent_zone = parent_zone_id,
        area = %active.area_name,
        director = format!("0x{director_id:08X}"),
        despawned = despawn_count,
        purged_coroutines = purged,
        "ContentFinished applied (full teardown)",
    );
}

/// B3 of the SEQ_005 unblock plan — port of C# `Chara::SetMod`.
/// Writes `value` into the target actor's `ModifierMap` keyed by
/// the numeric modifier id (the Rust `Modifier` enum's `as_u32`).
/// Tolerant of unknown modifier keys: the map stores them keyed by
/// the raw u32 even if no enum variant matches, so future scripts
/// that touch new ids don't abort here.
async fn apply_set_actor_mod(
    actor_id: u32,
    modifier_key: u32,
    value: i64,
    registry: &ActorRegistry,
) {
    let Some(handle) = registry.get(actor_id).await else {
        tracing::debug!(
            actor = format!("0x{actor_id:08X}"),
            modifier_key,
            value,
            "SetActorMod skipped — actor not in registry",
        );
        return;
    };
    {
        let mut c = handle.character.write().await;
        c.chara.mods.set_raw(modifier_key, value as f64);
    }
    tracing::debug!(
        actor = format!("0x{actor_id:08X}"),
        modifier_key,
        value,
        "SetActorMod applied",
    );
}

/// Runtime-side counterpart to the processor's
/// `apply_add_retainer_bazaar_item`: transactional upsert into the
/// `characters_retainer_bazaar` table. Exposed for scheduler-resumed
/// coroutines so a parked retainer-bazaar-seed script (rare, but
/// plausible once NPC-vendor bazaar seeding moves into director main
/// coroutines) can drain without reaching back through the
/// PacketProcessor.
async fn apply_add_retainer_bazaar_item(
    retainer_id: u32,
    item_id: u32,
    quantity: i32,
    quality: u8,
    price_gil: i32,
    db: &Database,
) {
    match db
        .add_retainer_bazaar_item(retainer_id, item_id, quantity, quality, price_gil)
        .await
    {
        Ok(server_item_id) => {
            tracing::info!(
                retainer_id,
                item_id,
                quantity,
                quality,
                price_gil,
                server_item_id,
                "AddRetainerBazaarItem applied (runtime)",
            );
        }
        Err(e) => {
            tracing::warn!(
                retainer_id,
                item_id,
                quantity,
                quality,
                price_gil,
                err = %e,
                "AddRetainerBazaarItem (runtime): DB upsert failed",
            );
        }
    }
}

/// Runtime-side counterpart to the processor's
/// `apply_director_outbox_op`: lets `apply_runtime_lua_command`
/// route `EndGuildleve` / `StartGuildleve` / `UpdateAimNumNow` /
/// etc. without reaching back through the processor. Same semantics:
/// single zone write lock, roster snapshot BEFORE `mutate` (so ops
/// that tear down the director — `abandon_guildleve`, which clears
/// `player_members` via `Director::end` — still fan to the right
/// recipients), immediate drain via `dispatch_director_event`.
async fn apply_director_outbox_op<F>(
    director_actor_id: u32,
    op_name: &'static str,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    mutate: F,
) where
    F: FnOnce(&mut crate::director::GuildleveDirector, &mut crate::director::DirectorOutbox),
{
    let zone_id = (director_actor_id >> 19) & 0x1FF;
    let Some(zone_arc) = world.zone(zone_id).await else {
        tracing::debug!(
            director = director_actor_id,
            zone = zone_id,
            op = op_name,
            "runtime director-outbox op skipped — zone not loaded",
        );
        return;
    };
    let (events, player_members) = {
        let mut zone = zone_arc.write().await;
        let Some(gld) = zone.core.guildleve_director_mut(director_actor_id) else {
            tracing::debug!(
                director = director_actor_id,
                zone = zone_id,
                op = op_name,
                "runtime director-outbox op skipped — guildleve director not on zone",
            );
            return;
        };
        let roster: Vec<u32> = gld.base.player_members().collect();
        let mut outbox = crate::director::DirectorOutbox::new();
        mutate(gld, &mut outbox);
        (outbox.drain(), roster)
    };
    // Pass the Arc<Database> through so `award_leve_completion_seals`
    // can persist on the `GuildleveEnded { was_completed: true }`
    // branch.
    for e in events {
        crate::director::dispatch_director_event(&e, &player_members, registry, world, Some(db))
            .await;
    }
    tracing::debug!(
        director = director_actor_id,
        zone = zone_id,
        op = op_name,
        "runtime director-outbox op applied",
    );
}

/// `GetWorldManager():DoZoneChange(player, zoneId, privateArea_or_nil,
/// privateAreaType, spawnType, x, y, z, rot)` — full cross-zone warp.
/// Extracted from the processor (#28 S1.2) so the runtime drain
/// (ticker / signal resumes — the SEQ_005 director's final warp-out
/// runs in a death-path call stack) can reach it; the processor's
/// login arm delegates here.
///
/// Same-zone targets short-circuit the registry move and behave like a
/// glorified `WarpToPosition` followed by a re-render. `private_area =
/// Some` routes the actor into that `PrivateArea` instance's core pool
/// (zone 155 ships a `PrivateAreaMasterPast` level-1 replica in the
/// seed — `034_server_zones_privateareas.sql` row 5 — so the tutorial
/// return warp resolves it rather than taking the parent-zone fallback
/// logged by `do_zone_change_with_private_area`).
///
/// Every directly-built subpacket in the warp burst is target-stamped
/// with the session id — untargeted subpackets are dropped by the
/// world-server proxy fan-out.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn apply_do_zone_change(
    player_id: u32,
    zone_id: u32,
    private_area: Option<String>,
    private_area_type: u32,
    spawn_type: u8,
    x: f32,
    y: f32,
    z: f32,
    rotation: f32,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(handle) = registry.get(player_id).await else {
        tracing::warn!(player = player_id, "DoZoneChange: actor missing");
        return;
    };
    let session_id = handle.session_id;
    let actor_id = handle.actor_id;
    if session_id == 0 {
        tracing::debug!(player = player_id, "DoZoneChange: no session (NPC?)");
        return;
    }

    // 1. Migrate the actor between zones (no-op if zone_id is the
    //    same as the current zone). `do_zone_change_with_private_area`
    //    also updates the session's destination + zone +
    //    private-area fields. `private_area = Some` routes the
    //    actor into that PrivateArea instance's core pool;
    //    `None` (or unknown name) goes to the parent zone's core.
    let spawn = common::Vector3::new(x, y, z);
    if let Err(e) = world
        .do_zone_change_with_private_area(
            actor_id,
            session_id,
            zone_id,
            private_area.clone(),
            private_area_type,
            spawn,
            rotation,
        )
        .await
    {
        tracing::error!(
            error = %e,
            player = player_id,
            zone = zone_id,
            ?private_area,
            "DoZoneChange: world.do_zone_change_with_private_area failed"
        );
        return;
    }

    // 2. Update the character's persistent zone_id + position
    //    (the registry move above only touches the spatial grid;
    //    the Character row's `base.zone_id` is what `send_zone_in_bundle`
    //    reads on the next login + what persists to disk). Also
    //    purge any status effects tagged LOSE_ON_ZONING — mirrors
    //    Meteor's `Player.CleanupAndSave`
    //    (`Map Server/Actors/Chara/Player/Player.cs:844`) which
    //    drops these effects unconditionally as the player crosses
    //    the zone boundary.
    let mut status_outbox = crate::status::StatusOutbox::new();
    {
        let mut c = handle.character.write().await;
        c.base.zone_id = zone_id;
        c.base.position_x = x;
        c.base.position_y = y;
        c.base.position_z = z;
        c.base.rotation = rotation;
        c.status_effects.remove_by_flag(
            crate::status::StatusEffectFlags::LOSE_ON_ZONING,
            &mut status_outbox,
        );
    }
    // Same drain shape as the processor's `drain_status_outbox` —
    // without the Lua engine the wire/save/recalc fan-out is dropped
    // but the in-memory purge above has already landed.
    if let Some(lua_ref) = lua {
        for evt in status_outbox.drain() {
            crate::runtime::dispatcher::dispatch_status_event(
                &evt,
                registry,
                world,
                db,
                lua_ref.catalogs(),
            )
            .await;
        }
    }

    // 3. Carry the requested spawn_type through to the zone-in
    //    bundle so the client plays the right "you arrived" anim.
    if let Some(mut snap) = world.session(session_id).await {
        snap.destination_spawn_type = spawn_type;
        world.upsert_session(snap).await;
    }

    // 4. Emit the zone-change packet trio — same order as
    //    `apply_do_zone_change_content`.
    let Some(client) = world.client(session_id).await else {
        tracing::warn!(player = player_id, "DoZoneChange: no client");
        return;
    };
    // Tagged with the session id — untargeted subpackets are dropped by
    // the world-server proxy fan-out, so this pair never reached the
    // client before (see the matching block + decomp notes in
    // `apply_do_zone_change_content`). Cross-zone warps still completed
    // because a real region change takes the SetMap handler's
    // region-mismatch arm; delivering the wipe + 0x00E2 restores pmeteor
    // parity (`DoZoneChange`, WorldManager.cs:877-879). Subcode 0x02 is
    // what pmeteor sends for full zone changes (0x10 is the in-place /
    // content-instance variant) — both set the client's force-reload
    // latch, so the distinction is parity-only. (Garlemald-Server #28.)
    {
        let mut wipe = crate::packets::send::handshake::build_delete_all_actors(actor_id);
        wipe.set_target_id(session_id);
        client.send_bytes(wipe.to_bytes()).await;
        let mut e2 = crate::packets::send::handshake::build_0xe2(actor_id, 0x02);
        e2.set_target_id(session_id);
        client.send_bytes(e2.to_bytes()).await;
    }

    // 5. Replay the zone-in bundle. `send_zone_in_bundle` reads
    //    from the session + character we just updated, so the
    //    bundle spawns the player at the new coords.
    world
        .send_zone_in_bundle(
            registry,
            db,
            lua.map(|l| l.catalogs()),
            session_id,
            spawn_type as u16,
        )
        .await;

    // 6. pmeteor sends the 34108 "instance" message AFTER the bundle
    //    when the destination is a PrivateArea (`if (newArea is
    //    PrivateArea)`, WorldManager.cs:887-888) — the SEQ_005 return
    //    warp into zone 155's PrivateAreaMasterPast takes this branch
    //    in the reference capture. (Garlemald-Server #28.)
    if private_area.is_some() {
        let mut msg = crate::packets::send::misc::build_text_sheet_no_source_x28(
            crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
            crate::packets::send::misc::WORLD_MASTER_ACTOR_ID,
            34108,
            0x20,
        );
        msg.set_target_id(session_id);
        client.send_bytes(msg.to_bytes()).await;
    }

    tracing::info!(
        player = player_id,
        zone = zone_id,
        ?private_area,
        private_area_type,
        spawn_type,
        x,
        y,
        z,
        rotation,
        "DoZoneChange applied (cross-zone warp + zone-in replay)",
    );
}

/// Apply a script's drained `LuaCommand`s with EVENT-bridge routing —
/// the shared engine behind `PacketProcessor::apply_event_script_commands`
/// and the ticker's per-owner coroutine drains. Event-flavoured commands
/// (`RunEventFunction` / `EndEvent` / `KickEvent` / ...) are translated
/// through the `EventOutbox` + dispatcher so cinematic packets reach the
/// wire (the plain runtime drain drops them at its catch-all); the rest
/// go through the runtime; and any `SendSignal` resumes the coroutines
/// parked on `waitForSignal(name)` (e.g. the combat tutorial director on
/// "playerActive") and applies THEIR commands, recursively.
/// (Garlemald-Server #28.)
pub(crate) async fn apply_event_script_commands(
    handle: &ActorHandle,
    commands: Vec<crate::lua::command::LuaCommand>,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    use crate::lua::command::LuaCommand;
    if commands.is_empty() {
        return;
    }
    // Pull `SendSignal` out — it re-enters the engine (resumes parked
    // coroutines) rather than being applied as a state mutation.
    let mut signals: Vec<String> = Vec::new();
    let mut rest: Vec<LuaCommand> = Vec::with_capacity(commands.len());
    for c in commands {
        match c {
            LuaCommand::SendSignal { name } => signals.push(name),
            other => rest.push(other),
        }
    }
    if !rest.is_empty() {
        let event_session_snapshot = {
            let c = handle.character.read().await;
            c.event_session.clone()
        };
        // Process commands in their ORIGINAL order, routing each to
        // exactly one path — order is load-bearing on the wire (see the
        // per-command interleave rationale in the PacketProcessor
        // delegate's history).
        for c in rest {
            let mut outbox = crate::event::outbox::EventOutbox::new();
            crate::event::lua_bridge::translate_lua_commands_into_outbox(
                std::slice::from_ref(&c),
                &event_session_snapshot,
                &mut outbox,
            );
            let events = outbox.drain();
            if events.is_empty() {
                apply_runtime_lua_command(c, registry, db, world, lua).await;
            } else {
                for e in events {
                    Box::pin(crate::event::dispatcher::dispatch_event_event(
                        &e, registry, world, db, lua,
                    ))
                    .await;
                }
            }
        }
    }
    if let Some(lua_engine) = lua {
        for name in signals {
            let resumed = lua_engine.fire_signal_and_drain(&name);
            if resumed.is_empty() {
                // Either nothing was parked on this signal, or the
                // resumed coroutine queued no commands before its next
                // yield (e.g. straight into `wait(1)`) — the
                // `fire_signal: draining parked coroutines` line above
                // disambiguates.
                tracing::debug!(
                    signal = %name,
                    "sendSignal resumed no commands",
                );
            } else {
                tracing::debug!(
                    signal = %name,
                    commands = resumed.len(),
                    "sendSignal resumed parked coroutine(s)",
                );
                Box::pin(apply_event_script_commands(
                    handle, resumed, registry, db, world, lua,
                ))
                .await;
            }
        }
    }
}

/// Bulk-drain helper — calls [`apply_runtime_lua_command`] for every
/// command in `cmds`. Commands that fall through (return `false`) are
/// logged at `debug` level; callers expecting only the runtime-safe
/// subset can pass arbitrary command vecs without pre-filtering.
pub async fn apply_runtime_lua_commands(
    cmds: Vec<LuaCommandKind>,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    for cmd in cmds {
        // Keep a copy for diagnostics only when DEBUG is enabled for this
        // target, so non-debug filters pay nothing. The drain is low-frequency
        // (quest-hook command batches), so the clone cost is negligible. We log
        // the whole command (variant name + fields) rather than the opaque
        // `Discriminant(N)`: event-flavoured commands (e.g. `RunEventFunction`,
        // `EndEvent`) routinely fall through here *after* the event bridge has
        // already emitted them, so naming the command is what lets a genuinely
        // dropped command be told apart from expected post-bridge noise.
        let diag = tracing::enabled!(tracing::Level::DEBUG).then(|| cmd.clone());
        let handled = apply_runtime_lua_command(cmd, registry, db, world, lua).await;
        if !handled && let Some(cmd) = diag {
            tracing::debug!(
                ?cmd,
                "runtime lua command unhandled (login-scoped or unrecognised)",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Quest-mutation helpers (ported from Meteor's `Quest.cs` / `QuestData.cs`
// runtime surface — same logic lives in `PacketProcessor`, kept in sync via
// thin wrappers there).
// ---------------------------------------------------------------------------

pub async fn apply_quest_mutation<F>(
    player_id: u32,
    quest_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    mutate: F,
) where
    F: FnOnce(&mut crate::actor::quest::Quest),
{
    let Some(handle) = registry.get(player_id).await else {
        tracing::debug!(
            player = player_id,
            quest = quest_id,
            "quest mutation skipped — player not in registry",
        );
        return;
    };
    let save_tuple = {
        let mut c = handle.character.write().await;
        let Some(slot) = c.quest_journal.slot_of(quest_id) else {
            tracing::debug!(
                player = player_id,
                quest = quest_id,
                "quest mutation skipped — quest not in journal",
            );
            return;
        };
        let Some(q) = c.quest_journal.slots[slot].as_mut() else {
            return;
        };
        mutate(q);
        if q.is_dirty() {
            let sequence = q.get_sequence();
            let flags = q.get_flags();
            let counters = [q.get_counter(0), q.get_counter(1), q.get_counter(2)];
            let actor_id = q.actor_id;
            q.clear_dirty();
            Some((slot as i32, actor_id, sequence, flags, counters))
        } else {
            None
        }
    };
    if let Some((slot, actor_id, sequence, flags, [c1, c2, c3])) = save_tuple
        && let Err(e) = db
            .save_quest(player_id, slot, actor_id, sequence, flags, c1, c2, c3)
            .await
    {
        tracing::warn!(
            error = %e,
            player = player_id,
            quest = quest_id,
            "quest save failed",
        );
    }
}

pub async fn apply_add_quest(
    player_id: u32,
    quest_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    let save_tuple = {
        let mut c = handle.character.write().await;
        if c.quest_journal.has(quest_id) {
            tracing::debug!(
                player = player_id,
                quest = quest_id,
                "AddQuest skipped — already in journal",
            );
            return;
        }
        if c.quest_journal.is_completed(quest_id) {
            tracing::debug!(
                player = player_id,
                quest = quest_id,
                "AddQuest skipped — already completed",
            );
            return;
        }
        let actor_id = crate::actor::quest::quest_actor_id(quest_id);
        let name = lua
            .and_then(|e| e.catalogs().quest_script_name(quest_id))
            .unwrap_or_default();
        let quest = crate::actor::quest::Quest::new(actor_id, name);
        let Some(slot) = c.quest_journal.add(quest) else {
            tracing::warn!(
                player = player_id,
                quest = quest_id,
                "AddQuest failed — journal full",
            );
            return;
        };
        (slot as i32, actor_id)
    };
    let (slot, actor_id) = save_tuple;
    if let Err(e) = db
        .save_quest(player_id, slot, actor_id, 0, 0, 0, 0, 0)
        .await
    {
        tracing::warn!(
            error = %e,
            player = player_id,
            quest = quest_id,
            "AddQuest DB persist failed",
        );
    }
    tracing::info!(
        player = player_id,
        quest = quest_id,
        slot,
        "AddQuest applied",
    );
    if let Some(lua_engine) = lua {
        fire_quest_hook(
            &handle,
            quest_id,
            "onStart",
            Vec::new(),
            lua_engine,
            registry,
            db,
            None,
        )
        .await;
    }
}

pub async fn apply_complete_quest(
    player_id: u32,
    quest_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    if let Some(lua_engine) = lua {
        fire_quest_hook(
            &handle,
            quest_id,
            "onFinish",
            vec![crate::lua::QuestHookArg::Bool(true)],
            lua_engine,
            registry,
            db,
            None,
        )
        .await;
    }
    let removed_slot = {
        let mut c = handle.character.write().await;
        let slot = c.quest_journal.slot_of(quest_id);
        c.quest_journal.complete(quest_id);
        slot.map(|s| s as i32)
    };
    if let Some(slot) = removed_slot
        && let Err(e) = db.remove_quest(player_id, quest_id).await
    {
        tracing::warn!(
            error = %e,
            player = player_id,
            quest = quest_id,
            slot,
            "CompleteQuest: scenario-row delete failed",
        );
    }
    if let Err(e) = db.complete_quest(player_id, quest_id).await {
        tracing::warn!(
            error = %e,
            player = player_id,
            quest = quest_id,
            "CompleteQuest: bitstream save failed",
        );
    }
    tracing::info!(
        player = player_id,
        quest = quest_id,
        "CompleteQuest applied",
    );
}

pub async fn apply_abandon_quest(
    player_id: u32,
    quest_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    if let Some(lua_engine) = lua {
        fire_quest_hook(
            &handle,
            quest_id,
            "onFinish",
            vec![crate::lua::QuestHookArg::Bool(false)],
            lua_engine,
            registry,
            db,
            None,
        )
        .await;
    }
    let had = {
        let mut c = handle.character.write().await;
        c.quest_journal.remove(quest_id).is_some()
    };
    if !had {
        tracing::debug!(
            player = player_id,
            quest = quest_id,
            "AbandonQuest skipped — not in journal",
        );
        return;
    }
    if let Err(e) = db.remove_quest(player_id, quest_id).await {
        tracing::warn!(
            error = %e,
            player = player_id,
            quest = quest_id,
            "AbandonQuest DB delete failed",
        );
    }
    tracing::info!(player = player_id, quest = quest_id, "AbandonQuest applied",);
}

pub async fn apply_quest_start_sequence(
    player_id: u32,
    quest_id: u32,
    sequence: u32,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    apply_quest_mutation(player_id, quest_id, registry, db, |q| {
        q.start_sequence(sequence)
    })
    .await;
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    {
        let mut c = handle.character.write().await;
        if let Some(q) = c.quest_journal.get_mut(quest_id) {
            q.state.begin_sequence_swap();
        }
    }
    if let Some(lua_engine) = lua {
        fire_quest_hook(
            &handle,
            quest_id,
            "onStateChange",
            vec![crate::lua::QuestHookArg::Int(sequence as i64)],
            lua_engine,
            registry,
            db,
            Some(world),
        )
        .await;
    }
    let stale: Vec<QuestEnpc> = {
        let mut c = handle.character.write().await;
        match c.quest_journal.get_mut(quest_id) {
            Some(q) => q.state.drain_stale_enpcs().collect(),
            None => Vec::new(),
        }
    };
    for enpc in stale {
        broadcast_quest_enpc_clear(player_id, enpc, registry, world).await;
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn apply_quest_set_enpc(
    player_id: u32,
    quest_id: u32,
    actor_class_id: u32,
    quest_flag_type: u8,
    is_talk_enabled: bool,
    is_push_enabled: bool,
    is_emote_enabled: bool,
    is_spawned: bool,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    let enpc = QuestEnpc::new(
        actor_class_id,
        quest_flag_type,
        is_spawned,
        is_talk_enabled,
        is_emote_enabled,
        is_push_enabled,
    );
    let outcome = {
        let mut c = handle.character.write().await;
        let Some(q) = c.quest_journal.get_mut(quest_id) else {
            return;
        };
        q.state.add_enpc(enpc)
    };
    match outcome {
        AddEnpcOutcome::Unchanged => {}
        AddEnpcOutcome::New(snapshot) | AddEnpcOutcome::Updated(snapshot) => {
            broadcast_quest_enpc_update(player_id, snapshot, registry, world).await;
        }
    }
}

pub async fn apply_quest_update_enpcs(
    player_id: u32,
    quest_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    // Mirror Meteor's `QuestState.UpdateState()` — re-run the script's
    // `onStateChange(sequence)` hook so flag-dependent `quest:SetENpc(...)`
    // calls re-evaluate, then drain stale entries and broadcast clears.
    // Same fix as `processor::apply_quest_update_enpcs`; both paths are
    // hot now that `dispatch_event_updated_drain` (post-cinematic
    // resume of a parked _WAIT_EVENT coroutine) routes through
    // `apply_runtime_lua_commands` instead of the processor's login-
    // command pipeline. Without the runtime re-run, the post-talk-tutorial
    // Yda→off / Papalymo→talk swap never propagated to the client and
    // Papalymo's quest icon stayed dark.
    let sequence = {
        let c = handle.character.read().await;
        c.quest_journal.get(quest_id).map(|q| q.get_sequence())
    };
    if let (Some(sequence), Some(lua_engine)) = (sequence, lua) {
        // begin_sequence_swap: move `current` to `old` so SetEnpc calls
        // populate a fresh `current` and we can diff for stale clears.
        {
            let mut c = handle.character.write().await;
            if let Some(q) = c.quest_journal.get_mut(quest_id) {
                q.state.begin_sequence_swap();
            }
        }
        fire_quest_hook(
            &handle,
            quest_id,
            "onStateChange",
            vec![crate::lua::QuestHookArg::Int(sequence as i64)],
            lua_engine,
            registry,
            db,
            Some(world),
        )
        .await;
    }
    let stale: Vec<QuestEnpc> = {
        let mut c = handle.character.write().await;
        match c.quest_journal.get_mut(quest_id) {
            Some(q) => q.state.drain_stale_enpcs().collect(),
            None => Vec::new(),
        }
    };
    for enpc in stale {
        broadcast_quest_enpc_clear(player_id, enpc, registry, world).await;
    }

    // Re-broadcast the quest GRAPHIC (head marker) of every ENPC
    // currently active for this quest — but NOT the SetEventStatus
    // overrides.
    //
    // Why the graphic: `apply_quest_set_enpc` only emits a broadcast
    // when `add_enpc` returns `New` or `Updated`. After a cinematic
    // noticeEvent ack, re-running `onStateChange` typically computes
    // the *same* flags as zone-in (e.g. man0g0 SEQ_000 with
    // MINITUT0/MINITUT1 still false), so every SetEnpc returns
    // `Unchanged` and no packets go out — yet the 1.x client drops
    // quest-graphic state across cinematic playback, so without a
    // fresh graphic Yda's `!` icon never reappears.
    //
    // Why NOT the statuses: pmeteor's `QuestState.AddENpc` is silent
    // for unchanged entries — quest SetEventStatus overrides only go
    // out for new/changed registrations, and entries dropped from the
    // sequence get RESET to the actor's own per-condition defaults
    // (`UpdateQuestNpcInInstance(enpc, clearInstance=true)` →
    // `GetSetEventStatusPackets()` with `pushEnabled = null`).
    // Re-emitting the overrides here permanently re-disabled the
    // Ul'dah opening stopper's own "exit"/"caution" push circles —
    // man0u0 registers it with `isPushEnabled=false` every sequence,
    // so each UpdateENPCs run re-killed the conditions the spawn
    // bundle's defaults had armed, and the player could walk straight
    // out of the Merchant Strip (issue #26 retest).
    let active: Vec<QuestEnpc> = {
        let c = handle.character.read().await;
        c.quest_journal
            .get(quest_id)
            .map(|q| q.state.current.values().copied().collect())
            .unwrap_or_default()
    };
    for enpc in active {
        broadcast_quest_enpc_graphic(player_id, enpc, registry, world).await;
    }
}

// ---------------------------------------------------------------------------
// Rewards
// ---------------------------------------------------------------------------

/// Apply an XP gain to `actor_id`'s `class_id` skill pool.
///
/// Pipeline (Tier 4 #19 refresh):
///   1. Read `restBonus` (0..=100 percentage from CharaState).
///   2. Apply rested-XP multiplier via [`consume_rested_xp`] — the
///      effective gain is `exp + floor(exp * restBonus / 100)`,
///      and the rested pool decays by roughly `exp / 50` per call
///      (so ~5000 XP at 100% rested drains the pool).
///   3. Run the existing level-up rollover via
///      `battle::save::level_up_if_threshold_crossed`.
///   4. Persist `skillPoint` / `skillLevel` / `restBonus` to DB.
///   5. When `world` is available, emit `SetActorProperty` packets
///      so the client UI refreshes without a full re-login:
///        - `charaWork.battleSave.skillPoint[class-1]` on every gain,
///        - `charaWork.battleSave.skillLevel[class-1]` +
///          `charaWork.parameterSave.state_mainSkillLevel` on level-up,
///        - `playerWork.restBonusExpRate` when rested decayed.
///
/// `world` is optional so existing unit tests that don't wire a
/// WorldManager keep working; the packet branch silently skips when
/// `None`.
pub async fn apply_add_exp(
    actor_id: u32,
    class_id: u8,
    exp: i32,
    registry: &ActorRegistry,
    db: &Database,
    world: Option<&WorldManager>,
    lua: Option<&Arc<LuaEngine>>,
) {
    if exp == 0 {
        return;
    }
    let Some(handle) = registry.get(actor_id).await else {
        return;
    };
    let class_slot = class_id as usize;
    // Read-modify-write inside the write lock so a concurrent AddExp
    // doesn't lose a level-up crossing to a race. `new_exp` and
    // `new_level` are the post-rollover values.
    let Some((effective_gain, new_exp, new_level, levels_gained, rested_before, rested_after)) = ({
        let mut c = handle.character.write().await;
        if class_slot >= c.battle_save.skill_point.len() {
            tracing::warn!(class = class_id, "AddExp: class_id out of range");
            None
        } else {
            let rested_before = c.chara.rest_bonus_exp_rate;
            let (effective_gain, rested_after) = consume_rested_xp(exp, rested_before);
            c.chara.rest_bonus_exp_rate = rested_after;
            let prior_sp = c.battle_save.skill_point[class_slot];
            let combined = prior_sp.saturating_add(effective_gain).max(0);
            let prior_level = c
                .battle_save
                .skill_level
                .get(class_slot)
                .copied()
                .unwrap_or(1)
                .max(1);
            let (lvl, sp, gained) =
                crate::battle::save::level_up_if_threshold_crossed(prior_level, combined);
            c.battle_save.skill_point[class_slot] = sp;
            if gained > 0 {
                if let Some(slot) = c.battle_save.skill_level.get_mut(class_slot) {
                    *slot = lvl;
                }
                // If this class is the active slot, also refresh the
                // top-level `chara.level` the stat pipeline reads. No
                // other class gets reflected into `chara.level` — the
                // player has one active class at a time.
                if c.chara.class as i32 == class_id as i32 {
                    c.chara.level = lvl;
                }
            }
            Some((effective_gain, sp, lvl, gained, rested_before, rested_after))
        }
    }) else {
        return;
    };

    if let Err(e) = db.set_exp(actor_id, class_id, new_exp).await {
        tracing::warn!(
            actor = actor_id,
            class = class_id,
            err = %e,
            "AddExp: DB persist failed",
        );
    }
    if levels_gained > 0 {
        if let Err(e) = db.set_level(actor_id, class_id, new_level).await {
            tracing::warn!(
                actor = actor_id,
                class = class_id,
                err = %e,
                "AddExp: set_level DB persist failed",
            );
        }
        tracing::info!(
            actor = actor_id,
            class = class_id,
            new_level,
            levels_gained,
            "AddExp: level up",
        );
    }
    if rested_after != rested_before
        && let Err(e) = db.set_rest_bonus_exp_rate(actor_id, rested_after).await
    {
        tracing::warn!(
            actor = actor_id,
            err = %e,
            "AddExp: restBonus DB persist failed",
        );
    }

    // Client-facing property emits — only fire when we have a
    // WorldManager to reach the session → client handle. Also
    // carries the ability-unlock lookup through `lua.catalogs()` so
    // the learn-commands game-messages fire for the player when
    // level-up crosses a threshold that unlocks an ability.
    if let Some(world) = world {
        emit_exp_property_updates(
            actor_id,
            class_id,
            exp,
            effective_gain,
            new_exp,
            new_level,
            levels_gained,
            rested_before,
            rested_after,
            &handle,
            world,
            registry,
            lua,
        )
        .await;
    }

    tracing::info!(
        actor = actor_id,
        class = class_id,
        delta = exp,
        applied = effective_gain,
        skill_point = new_exp,
        level = new_level,
        rested_before,
        rested_after,
        "AddExp applied",
    );
}

/// Apply rested-XP bonus to an incoming gain.
///
/// `rested` is the 0..=100 bonus percentage stored on
/// `CharaState.rest_bonus_exp_rate`. Returns `(total_gain, new_rested)`.
/// The bonus is `floor(exp * rested_pct / 100)` — a 100%-rested
/// player gets double XP on their next gain. Decay is `max(1, exp/50)`
/// per call: ~5000 XP at steady 100% rested drains the pool; smaller
/// gains sip more slowly. Negative `rested` clamps to 0. Zero / negative
/// `exp` is a no-op and leaves the pool alone (matches the `exp == 0`
/// early return in `apply_add_exp`).
pub fn consume_rested_xp(exp: i32, rested: i32) -> (i32, i32) {
    if exp <= 0 || rested <= 0 {
        return (exp, rested.max(0));
    }
    let rested_pct = rested.min(100);
    let bonus = (exp as i64 * rested_pct as i64 / 100) as i32;
    let total = exp.saturating_add(bonus);
    // ~1 point decayed per 50 XP of base gain, min 1 so tiny gains
    // don't freeload.
    let decay = ((exp + 49) / 50).max(1);
    let new_rested = (rested - decay).max(0);
    (total, new_rested)
}

/// Per-class "You earn [exp] experience points." text ids — pmeteor's
/// `BattleUtils.ClassExperienceTextIds` (`Map Server/Actors/Chara/Ai/
/// Utils/BattleUtils.cs:102-123`). One id per class because non-English
/// locales inflect the class name into the line. Returns `None` for
/// ids outside the table (e.g. jobs, retired classes): the gain still
/// applies, the chat line is just skipped.
fn class_experience_text_id(class_id: u8) -> Option<u16> {
    Some(match class_id {
        2 => 33934,  // Pugilist
        3 => 33935,  // Gladiator
        4 => 33936,  // Marauder
        7 => 33937,  // Archer
        8 => 33938,  // Lancer
        10 => 33939, // Sentinel (retired class; the text id survives in the client files)
        22 => 33940, // Thaumaturge
        23 => 33941, // Conjurer
        29 => 33945, // Carpenter
        30 => 33946, // Blacksmith
        31 => 33947, // Armorer
        32 => 33948, // Goldsmith
        33 => 33949, // Leatherworker
        34 => 33950, // Weaver
        35 => 33951, // Alchemist
        36 => 33952, // Culinarian
        39 => 33953, // Miner
        40 => 33954, // Botanist
        41 => 33955, // Fisher
        _ => return None,
    })
}

/// Emit the wire updates Meteor's `AddExp` sends after a successful
/// gain, in pmeteor's order (`Player.cs:2932-2976` + the caller's
/// `DoBattleAction(0, 0, actionList)`):
///
///   1. `charaWork/stateForAll` → `skillLevel[class-1]`,
///      `state_mainSkillLevel` (self + nearby broadcast, level-up only),
///   2. `charaWork/battleStateForSelf` → `skillPoint[class-1]`,
///      `playerWork.restBonusExpRate` (self-only),
///   3. one 0x0139-family CommandResult batch carrying the text rows
///      ("You earn …" / "You attain level …" / "You learn …").
///
/// Wire rules (the reason this path's first live run showed nothing
/// client-side):
///   - Ship RAW subpacket bytes — the connection write task wraps each
///     mpsc frame in a BasePacket itself
///     (`server.rs`/`wrap_subpackets_in_basepacket`); pre-wrapping with
///     `BasePacket::create_from_subpacket` double-framed every packet
///     and the world-server proxy read the inner BasePacket header as
///     a garbage subpacket.
///   - Stamp the owner's session into self-bound subpackets — the
///     proxy fan-out drops `target_id == 0` frames (the rule
///     `send_to_self_if_player` / `apply_send_game_message` already
///     follow). Broadcast legs stay 0 for per-recipient stamping
///     inside `broadcast_around_actor`.
///   - Text ids 33909/33926/the 33934-family render ONLY as
///     CommandResult rows on the battle-log channel; the previous
///     `build_game_message` emission used opcode 0x01FD, which exists
///     in neither pmeteor's packet family nor the 1.x opcode table.
#[allow(clippy::too_many_arguments)]
async fn emit_exp_property_updates(
    actor_id: u32,
    class_id: u8,
    base_exp: i32,
    effective_gain: i32,
    new_exp: i32,
    new_level: i16,
    levels_gained: i16,
    rested_before: i32,
    rested_after: i32,
    handle: &ActorHandle,
    world: &WorldManager,
    registry: &ActorRegistry,
    lua: Option<&Arc<LuaEngine>>,
) {
    let session_id = handle.session_id;
    if session_id == 0 {
        return;
    }
    let Some(client) = world.client(session_id).await else {
        return;
    };
    let class_slot = class_id.saturating_sub(1);
    // Silent None when the zone isn't live (pure DB-only tests) — the
    // broadcast legs just skip.
    let zone = world.zone(handle.zone_id).await;

    // 1. Level-up: skillLevel + state_mainSkillLevel, self + nearby —
    // `/stateForAll` is retail's "everyone who can see this actor"
    // convention. The source is excluded by `actors_around`, so the
    // direct self leg is required (same pair the live combat log's
    // `broadcast_results` uses).
    if levels_gained > 0 {
        let mut b = crate::packets::send::actor::ActorPropertyPacketBuilder::new(
            actor_id,
            "charaWork/stateForAll",
        );
        b.add_short(
            &format!("charaWork.battleSave.skillLevel[{}]", class_slot),
            new_level as u16,
        );
        b.add_short(
            "charaWork.parameterSave.state_mainSkillLevel",
            new_level as u16,
        );
        for sub in b.done() {
            let bytes = sub.to_bytes();
            crate::runtime::dispatcher::send_to_self_if_player(
                registry,
                world,
                actor_id,
                bytes.clone(),
            )
            .await;
            if let Some(zone) = &zone {
                let _ = crate::runtime::broadcast::broadcast_around_actor(
                    world,
                    registry,
                    zone,
                    handle.actor_id,
                    bytes,
                )
                .await;
            }
        }
    }

    // 2. Self-only: skillPoint + restBonusExpRate — owner sees their
    // own XP bar and rested-exp UI widget, nobody else needs to.
    {
        let mut b = crate::packets::send::actor::ActorPropertyPacketBuilder::new(
            actor_id,
            "charaWork/battleStateForSelf",
        );
        b.add_int(
            &format!("charaWork.battleSave.skillPoint[{}]", class_slot),
            new_exp as u32,
        );
        if rested_before != rested_after {
            b.add_int("playerWork.restBonusExpRate", rested_after as u32);
        }
        for mut sub in b.done() {
            sub.set_target_id(session_id);
            client.send_bytes(sub.to_bytes()).await;
        }
    }

    // 3. The text batch — pmeteor's actionList rows verbatim
    // (`CommandResult(targetId, worldMasterTextId, effectId, amount,
    // param, hitNum = 1)`):
    //   exp:   (Id, ClassExperienceTextIds[class], 0, exp, bonus%) —
    //          "You earn [exp](+[bonus]%) experience points."
    //          (Player.cs:2939; amount caps at u16 — "the exp graphic
    //          overflows after ~65k", Player.cs:2931). The bonus here
    //          is garlemald's rested-XP cut.
    //   level: (Id, 33909, 0, level) — "You attain level [level]."
    //          one per crossed level, oldest first (Player.cs:3011).
    //   learn: (Id, 33926, 0, commandId) — "You learn [command]." per
    //          `Catalogs::commands_unlocked_at` (Player.cs:2995,
    //          `EquipAbilitiesAtLevel`).
    let mut rows: Vec<crate::packets::send::actor_battle::CommandResult> = Vec::new();
    if effective_gain > 0
        && let Some(text_id) = class_experience_text_id(class_id)
    {
        let bonus_pct = if base_exp > 0 && effective_gain > base_exp {
            (((effective_gain - base_exp) as i64 * 100) / base_exp as i64) as u32
        } else {
            0
        };
        rows.push(crate::packets::send::actor_battle::CommandResult {
            target_id: actor_id,
            worldmaster_text_id: text_id,
            amount: effective_gain.min(u16::MAX as i32) as u32,
            param: bonus_pct.min(u8::MAX as u32),
            hit_num: 1,
            ..Default::default()
        });
    }
    for gained_idx in (0..levels_gained).rev() {
        // `new_level` is the *final* post-rollover level; the
        // intermediate levels we passed through are at
        // `new_level - gained_idx`.
        let at_level = new_level - gained_idx;
        rows.push(crate::packets::send::actor_battle::CommandResult {
            target_id: actor_id,
            worldmaster_text_id: 33909,
            amount: at_level.max(0) as u32,
            hit_num: 1,
            ..Default::default()
        });
        if let Some(lua) = lua {
            for command_id in lua.catalogs().commands_unlocked_at(class_id, at_level) {
                tracing::info!(
                    actor = actor_id,
                    class = class_id,
                    level = at_level,
                    command_id,
                    "ability unlock: You learn <command>",
                );
                rows.push(crate::packets::send::actor_battle::CommandResult {
                    target_id: actor_id,
                    worldmaster_text_id: 33926,
                    amount: command_id as u32,
                    hit_num: 1,
                    ..Default::default()
                });
            }
        }
    }
    let mut offset = 0usize;
    while offset < rows.len() {
        // pmeteor container choice (matches `broadcast_results`): X01
        // for a single row, X10 up to 10, X18 beyond.
        let remaining = rows.len() - offset;
        let sub = if remaining == 1 {
            let row = &rows[offset];
            offset += 1;
            crate::packets::send::actor_battle::build_command_result_x01(actor_id, 0, 0, row)
        } else if remaining <= 10 {
            crate::packets::send::actor_battle::build_command_result_x10(
                actor_id,
                0,
                0,
                &rows,
                &mut offset,
            )
        } else {
            crate::packets::send::actor_battle::build_command_result_x18(
                actor_id,
                0,
                0,
                &rows,
                &mut offset,
            )
        };
        let bytes = sub.to_bytes();
        crate::runtime::dispatcher::send_to_self_if_player(
            registry,
            world,
            actor_id,
            bytes.clone(),
        )
        .await;
        if let Some(zone) = &zone {
            let _ = crate::runtime::broadcast::broadcast_around_actor(
                world,
                registry,
                zone,
                handle.actor_id,
                bytes,
            )
            .await;
        }
    }
}

/// `player:SetQuestComplete(id, flag)` — direct-set the 2048-bit
/// completion bit without running the quest's `onFinish` hook. Used by
/// GM `!completedQuest` debug commands and cross-quest prerequisites.
pub async fn apply_set_quest_complete(
    player_id: u32,
    quest_id: u32,
    flag: bool,
    registry: &ActorRegistry,
    db: &Database,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    {
        let mut c = handle.character.write().await;
        c.quest_journal.set_completed(quest_id, flag);
    }
    if flag {
        if let Err(e) = db.complete_quest(player_id, quest_id).await {
            tracing::warn!(
                error = %e,
                player = player_id,
                quest = quest_id,
                "SetQuestComplete(true): bitstream save failed",
            );
        }
    } else {
        // Clearing a bit: reload the current bitstream from DB, flip
        // the bit, write back. `db.complete_quest` is set-only; the
        // complement path lives here inline.
        match db.load_completed_quests(player_id).await {
            Ok(mut bs) => {
                if let Some(bit) = crate::actor::quest::quest_id_to_bit(quest_id) {
                    bs.clear(bit);
                    if let Err(e) = db.save_completed_quests(player_id, &bs).await {
                        tracing::warn!(
                            error = %e,
                            player = player_id,
                            quest = quest_id,
                            "SetQuestComplete(false): bitstream save failed",
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    player = player_id,
                    quest = quest_id,
                    "SetQuestComplete(false): bitstream load failed",
                );
            }
        }
    }
    tracing::info!(
        player = player_id,
        quest = quest_id,
        flag,
        "SetQuestComplete applied",
    );
}

/// Grant one stack of `item_id` to the NORMAL bag on `actor_id`.
/// Used by:
///  * Gathering's `HarvestReward` Lua helper when the strike minigame
///    lands a copper/rock-salt/ore drop (actor_id = player character
///    id, item_package = `PKG_NORMAL = 0`).
///  * Future `onReward` quest-finish hooks once those land.
///
/// Persistence is direct-DB via [`Database::add_harvest_item`] — the
/// in-memory `ItemPackage` on `Player` is not yet accessible from the
/// registry (`Player` is not in `ActorRegistry`; the registry stores
/// only the `Character` sub-struct). The player picks up the new
/// stack on the next inventory resync / zone-in, which matches
/// retail 1.x behaviour where the `textInputWidget` remains open and
/// the bag only refreshes on the next `/_init` bundle.
///
/// Silently no-ops for:
///  * non-NORMAL packages (currency / key items go through their own
///    paths — `AddGil` and a future AddKeyItem),
///  * zero or negative quantity (`player:AddItem(..., 0)` is a legal
///    Lua no-op that shouldn't insert a zero-quantity row),
///  * item id 0.
pub async fn apply_add_item(
    actor_id: u32,
    item_package: u16,
    item_id: u32,
    quantity: i32,
    db: &Database,
) {
    if quantity <= 0 || item_id == 0 {
        return;
    }
    // Route currency stacks through add_gil so the 1_000_001 gil row
    // stays the single-stack well-known layout. The gathering path
    // never lands here (Copper Ore is a NORMAL-bag item), but Lua
    // scripts that incorrectly call `GetItemPackage(99):AddItem(1000001, 10)`
    // should still do the right thing.
    if item_package == crate::inventory::PKG_CURRENCY_CRYSTALS {
        apply_add_gil(actor_id, quantity, db).await;
        return;
    }
    // Everything else lands in NORMAL for the first cut. Key-items /
    // bazaar / trade bags get their own paths as they're wired up.
    if item_package != crate::inventory::PKG_NORMAL {
        tracing::debug!(
            actor = actor_id,
            package = item_package,
            item = item_id,
            qty = quantity,
            "AddItem: non-NORMAL packages not yet implemented — logging only",
        );
        return;
    }
    match db.add_harvest_item(actor_id, item_id, quantity, 1).await {
        Ok(total) => {
            tracing::info!(
                actor = actor_id,
                item = item_id,
                delta = quantity,
                total,
                "AddItem applied",
            );
        }
        Err(e) => {
            tracing::warn!(
                actor = actor_id,
                item = item_id,
                delta = quantity,
                err = %e,
                "AddItem: DB persist failed",
            );
        }
    }
}

/// Garlemald-Server #28 — runtime drain of `player:GetEquipment():Set(
/// slots, srcPositions, srcPackage)`. For each paired
/// `(gear_slot, src_position)`, equip the item currently sitting in the
/// player's bag at `(src_package, src_position)` into gear slot
/// `gear_slot`.
///
/// Flow per index:
///  1. Resolve the bag item's `serverItemId` + catalog id via
///     [`Database::resolve_bag_slot_item_id`]; skip empty bag slots.
///  2. Skip the gear slot if it's ALREADY equipped
///     ([`Database::is_gear_slot_equipped`]) — this is the idempotence
///     guarantee that lets `player.lua::equipClassItems` run on EVERY
///     login (FIX C) to backfill broken / seed characters without ever
///     clobbering a normal player's chosen gear.
///  3. Equip through the EXISTING `InventoryEvent::DbEquip` path
///     ([`dispatch_inventory_event`]) — it writes
///     `characters_inventory_equipment` via `db.equip_item(...)` and runs
///     `apply_recalc_stats(...)`, so STR/VIT/HP/MP/weapon-damage all
///     refresh exactly like a manual equip.
///
/// After all slots are processed, re-send the 0x014E
/// SetInitialEquipment packet (built from the now-updated DB) to the
/// player's own client so the equipment table the client gates Active
/// mode on is populated mid-session — without this the freshly-equipped
/// Gladiator still couldn't press F until a re-zone.
///
/// `lua` carries the [`Catalogs`] the `DbEquip` recalc needs; if it's
/// absent (battle-path callers without a LuaEngine) we fall back to a
/// direct `db.equip_item` + best-effort skip so the DB row still lands.
///
/// [`Catalogs`]: crate::lua::Catalogs
/// [`dispatch_inventory_event`]: crate::runtime::dispatcher::dispatch_inventory_event
#[allow(clippy::too_many_arguments)]
pub async fn apply_equip_from_package(
    player_id: u32,
    gear_slots: &[u16],
    src_positions: &[u16],
    src_package: u32,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    // The active class id keys the equipment table (undergarments use
    // class 0, handled inside `equip_item`). Read it once up front.
    let class_id = resolve_player_class_id(registry, player_id).await;
    let mut equipped_any = false;
    // Pair the two index tables positionally; a length mismatch simply
    // drops the trailing unpaired entries (zip semantics).
    for (&gear_slot, &src_position) in gear_slots.iter().zip(src_positions.iter()) {
        // (1) Resolve the bag item at (src_package, src_position).
        let resolved = match db
            .resolve_bag_slot_item_id(player_id, src_package as u16, src_position)
            .await
        {
            Ok(Some(pair)) => pair,
            Ok(None) => {
                tracing::debug!(
                    player = format!("0x{player_id:08X}"),
                    gear_slot,
                    src_position,
                    src_package,
                    "EquipFromPackage: bag slot empty — skipping",
                );
                continue;
            }
            Err(e) => {
                tracing::warn!(
                    player = format!("0x{player_id:08X}"),
                    gear_slot,
                    src_position,
                    err = %e,
                    "EquipFromPackage: bag-slot resolve failed",
                );
                continue;
            }
        };
        let (server_item_id, _catalog_id) = resolved;
        // (2) Idempotence — only fill EMPTY gear slots.
        match db
            .is_gear_slot_equipped(player_id, class_id, gear_slot)
            .await
        {
            Ok(true) => {
                tracing::debug!(
                    player = format!("0x{player_id:08X}"),
                    gear_slot,
                    "EquipFromPackage: gear slot already filled — skipping (idempotent)",
                );
                continue;
            }
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(
                    player = format!("0x{player_id:08X}"),
                    gear_slot,
                    err = %e,
                    "EquipFromPackage: occupancy check failed — skipping for safety",
                );
                continue;
            }
        }
        // (3) Equip via the existing DbEquip path (DB write + recalc).
        if let Some(lua) = lua {
            let event = crate::inventory::InventoryEvent::DbEquip {
                owner_actor_id: player_id,
                equip_slot: gear_slot,
                unique_item_id: server_item_id,
            };
            crate::runtime::dispatcher::dispatch_inventory_event(
                &event,
                registry,
                world,
                db,
                lua.catalogs(),
            )
            .await;
            equipped_any = true;
        } else {
            // No LuaEngine in scope — write the equip row directly so the
            // gear at least persists; stat recalc is best-effort skipped
            // (callers without a LuaEngine don't drive Active mode).
            let is_undergarment = gear_slot == crate::actor::player::SLOT_UNDERSHIRT
                || gear_slot == crate::actor::player::SLOT_UNDERGARMENT;
            if let Err(e) = db
                .equip_item(
                    player_id,
                    class_id as u8,
                    gear_slot,
                    server_item_id,
                    is_undergarment,
                )
                .await
            {
                tracing::warn!(
                    player = format!("0x{player_id:08X}"),
                    gear_slot,
                    err = %e,
                    "EquipFromPackage: direct equip_item failed",
                );
            } else {
                equipped_any = true;
            }
        }
    }

    // Re-send the 0x014E SetInitialEquipment packet from the now-updated
    // DB so the client's equipment table reflects the new gear without a
    // re-zone — this is what unblocks Active mode for the tutorial.
    if equipped_any {
        resend_initial_equipment(player_id, class_id, registry, db, world).await;
    }
}

/// Garlemald-Server #28 — resolve the player's active class id (current
/// job if set, else the class slot). Mirrors the dispatcher's private
/// `resolve_current_class_id` but returns `u16` to match the equipment
/// table's `classId` column type used by `is_gear_slot_equipped` /
/// `get_equipment`.
async fn resolve_player_class_id(registry: &ActorRegistry, player_id: u32) -> u16 {
    let Some(handle) = registry.get(player_id).await else {
        return 0;
    };
    let c = handle.character.read().await;
    if c.chara.current_job != 0 {
        c.chara.current_job as u16
    } else {
        c.chara.class.max(0) as u16
    }
}

/// Garlemald-Server #28 — re-send the 0x014E SetInitialEquipment packet
/// to the player's own client, wrapped in the inventory begin/set/end
/// brackets the zone-in bundle uses. Built from
/// [`Database::load_equipped_catalog_ids`] so the `(equip_slot,
/// catalog_id)` pairs match what FIX B sends at zone-in. No-ops cleanly
/// when the player isn't in the registry or has no live session.
async fn resend_initial_equipment(
    player_id: u32,
    class_id: u16,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    let session_id = handle.session_id;
    if session_id == 0 {
        return;
    }
    let Some(client) = world.client(session_id).await else {
        return;
    };
    let pairs = load_initial_equipment_pairs(db, player_id, class_id as u8).await;
    let mut subs = vec![
        crate::packets::send::actor_inventory::build_inventory_begin_change(player_id, false),
        crate::packets::send::actor_inventory::build_inventory_set_begin(player_id, 35, 0x00FE),
    ];
    subs.extend(
        crate::packets::send::actor_inventory::build_set_initial_equipment(player_id, &pairs),
    );
    subs.push(crate::packets::send::actor_inventory::build_inventory_set_end(player_id));
    subs.push(crate::packets::send::actor_inventory::build_inventory_end_change(player_id));
    for mut sub in subs {
        sub.set_target_id(session_id);
        client.send_bytes(sub.to_bytes()).await;
    }
    tracing::debug!(
        player = format!("0x{player_id:08X}"),
        slots = pairs.len(),
        "EquipFromPackage: re-sent SetInitialEquipment",
    );
}

/// Garlemald-Server #28 — load the player's equipped `(equip_slot,
/// catalog_id)` pairs ready for `build_set_initial_equipment`. Shared by
/// the FIX-A mid-session re-send (above) and the FIX-B zone-in bundle
/// (`world_manager::send_zone_in_bundle`). The catalog id is the item
/// graphic the client renders; `load_equipped_catalog_ids` resolves it
/// by joining `characters_inventory_equipment` → `server_items`.
pub async fn load_initial_equipment_pairs(
    db: &Database,
    player_id: u32,
    class_id: u8,
) -> Vec<(u16, u32)> {
    let mut pairs: Vec<(u16, u32)> = db
        .load_equipped_catalog_ids(player_id, class_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();
    // Deterministic order — the HashMap iteration order is otherwise
    // nondeterministic, which would make the packet bytes (and tests)
    // flaky.
    pairs.sort_by_key(|(slot, _)| *slot);
    pairs
}

/// Tier 3 #13 — advance any fieldcraft leves the player currently
/// has accepted whose band-0 objective matches `item_catalog_id`.
/// Returns the list of leve ids that transitioned to completed on
/// this call (used by callers that want to emit a "leve complete"
/// GameMessage without a before/after diff).
///
/// Short-circuits when:
///  * no [`RegionalLeveResolver`] is installed (fresh DB, boot
///    race) — catalogs hand out `None` and we early-return;
///  * the resolver reports zero leves targeting this item — no
///    matching active leve can possibly exist;
///  * the player isn't in [`ActorRegistry`] — mirrors every other
///    apply helper.
///
/// Progress persists through [`Database::save_quest`] exactly like
/// any other quest mutation — the dirty-bit on [`Quest`] flips
/// inside [`RegionalLeveView::advance_progress`] so existing
/// machinery picks it up.
///
/// [`RegionalLeveResolver`]: crate::leve::RegionalLeveResolver
/// [`RegionalLeveView::advance_progress`]: crate::leve::RegionalLeveView::advance_progress
pub async fn advance_fieldcraft_leves(
    player_id: u32,
    item_catalog_id: u32,
    delta: u16,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) -> Vec<u32> {
    if delta == 0 {
        return Vec::new();
    }
    let Some(lua) = lua else {
        return Vec::new();
    };
    let Some(resolver) = lua.catalogs().regional_leve_resolver() else {
        return Vec::new();
    };
    let leve_ids = resolver.fieldcraft_leves_for_item(item_catalog_id);
    if leve_ids.is_empty() {
        return Vec::new();
    }
    advance_regional_leves(player_id, leve_ids, delta, &resolver, registry, db).await
}

/// Tier 3 #13 — battlecraft counterpart. Advance any accepted
/// battlecraft leves whose band-0 objective matches
/// `actor_class_id`. Invoked from [`fire_on_kill_bnpc`] after the
/// kill is resolved.
///
/// [`fire_on_kill_bnpc`]: crate::runtime::quest_hook::fire_on_kill_bnpc
pub async fn advance_battlecraft_leves(
    player_id: u32,
    actor_class_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) -> Vec<u32> {
    let Some(lua) = lua else {
        return Vec::new();
    };
    let Some(resolver) = lua.catalogs().regional_leve_resolver() else {
        return Vec::new();
    };
    let leve_ids = resolver.battlecraft_leves_for_class(actor_class_id);
    if leve_ids.is_empty() {
        return Vec::new();
    }
    advance_regional_leves(player_id, leve_ids, 1, &resolver, registry, db).await
}

/// Shared fieldcraft/battlecraft inner loop. Walks the candidate
/// leve ids, finds each one's quest slot, advances the view, and
/// persists any dirty slots. Keeps the fan-out shape in one place
/// so the fieldcraft and battlecraft entry points stay narrow.
async fn advance_regional_leves(
    player_id: u32,
    leve_ids: &[u32],
    delta: u16,
    resolver: &crate::leve::RegionalLeveResolver,
    registry: &ActorRegistry,
    db: &Database,
) -> Vec<u32> {
    let Some(handle) = registry.get(player_id).await else {
        return Vec::new();
    };
    let mut completed = Vec::new();
    // Collect dirty-slot save work under the write lock, then drop
    // the lock before awaiting the DB so a slow disk write doesn't
    // hold the player's character lock.
    let pending_saves: Vec<(i32, u32, u32, u32, [u16; 3], u32)> = {
        let mut c = handle.character.write().await;
        let mut saves = Vec::new();
        for &leve_id in leve_ids {
            let Some(data) = resolver.by_id(leve_id) else {
                continue;
            };
            let Some(slot) = c.quest_journal.slot_of(leve_id) else {
                continue;
            };
            let Some(quest) = c.quest_journal.slots[slot].as_mut() else {
                continue;
            };
            let just_completed = {
                let mut view = crate::leve::RegionalLeveView::new(quest, data);
                view.advance_progress(delta)
            };
            if just_completed {
                completed.push(leve_id);
            }
            if quest.is_dirty() {
                let sequence = quest.get_sequence();
                let flags = quest.get_flags();
                let counters = [
                    quest.get_counter(0),
                    quest.get_counter(1),
                    quest.get_counter(2),
                ];
                let actor_id = quest.actor_id;
                quest.clear_dirty();
                saves.push((slot as i32, actor_id, sequence, flags, counters, leve_id));
            }
        }
        saves
    };
    for (slot, actor_id, sequence, flags, [c1, c2, c3], leve_id) in pending_saves {
        if let Err(e) = db
            .save_quest(player_id, slot, actor_id, sequence, flags, c1, c2, c3)
            .await
        {
            tracing::warn!(
                player = player_id,
                leve = leve_id,
                err = %e,
                "regional leve progress: save_quest failed",
            );
        }
    }
    completed
}

/// Tier 4 #14 C — grant a stack to a retainer's personal
/// inventory. Parallel to [`apply_add_item`] (the player-scoped
/// variant) but routes to
/// [`Database::add_retainer_inventory_item`] so the write lands in
/// `characters_retainer_inventory` rather than
/// `characters_inventory`.
///
/// Silently no-ops for:
///  * non-NORMAL packages — retainer bazaar adds go through the
///    dedicated `AddRetainerBazaarItem` command + `add_retainer_bazaar_item`
///    DB helper, not this path.
///  * zero or negative quantity — mirrors the player-side behaviour.
///  * item id 0.
pub async fn apply_add_item_to_retainer(
    retainer_id: u32,
    item_package: u16,
    item_id: u32,
    quantity: i32,
    db: &Database,
) {
    if quantity <= 0 || item_id == 0 {
        return;
    }
    // Non-NORMAL packages on a retainer are unexpected today; the
    // only script path that reaches here is
    // `retainer:GetItemPackage(0):AddItem(...)` which always uses
    // INVENTORY_NORMAL = 0. Log + bail so a future Lua typo surfaces
    // visibly.
    if item_package != crate::inventory::PKG_NORMAL {
        tracing::debug!(
            retainer = retainer_id,
            package = item_package,
            item = item_id,
            qty = quantity,
            "AddItemToRetainer: non-NORMAL packages not implemented — logging only",
        );
        return;
    }
    match db
        .add_retainer_inventory_item(retainer_id, item_id, quantity, 1, item_package)
        .await
    {
        Ok(total) => {
            tracing::info!(
                retainer = retainer_id,
                item = item_id,
                delta = quantity,
                total,
                "AddItemToRetainer applied",
            );
        }
        Err(e) => {
            tracing::warn!(
                retainer = retainer_id,
                item = item_id,
                delta = quantity,
                err = %e,
                "AddItemToRetainer: DB persist failed",
            );
        }
    }
}

/// Tier 1 #2 C — Lua-driven status-effect application. Parallels
/// the internal `add_status_effect` path the Rust dispatcher uses
/// during combat resolution, but gated behind a dedicated
/// [`LuaCommand::TryStatus`] variant so Lua scripts can apply
/// buffs / debuffs / DoTs without going through the full
/// battle-event pipeline.
///
/// Behaviour matches Meteor's `action.TryStatus(action, target,
/// status, tier?, magnitude?, duration?)` shape: build a fresh
/// [`StatusEffect`] on the target, insert into its
/// [`StatusEffectContainer`] (which honours the existing overwrite
/// rules + 20-effect cap), and drain the resulting
/// [`StatusOutbox`] through the shared
/// [`crate::runtime::dispatcher::dispatch_status_event`] so the
/// gain packet + `onGain` Lua hook fire just as they would for a
/// Rust-internal apply.
///
/// Returns `true` when the effect landed (fresh or successful
/// overwrite), `false` on any no-op path (missing target, full
/// table, overwrite-rejected). Short-circuits silently when `lua`
/// is `None` (test harness without a Catalogs clone) since the
/// dispatcher requires a real `Arc<Catalogs>`.
///
/// [`StatusEffect`]: crate::status::StatusEffect
/// [`StatusEffectContainer`]: crate::status::StatusEffectContainer
/// [`StatusOutbox`]: crate::status::StatusOutbox
#[allow(clippy::too_many_arguments)]
pub async fn apply_try_status(
    source_actor_id: u32,
    target_actor_id: u32,
    status_id: u32,
    duration_s: u32,
    magnitude: f64,
    tick_ms: u32,
    tier: u8,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) -> bool {
    let Some(target) = registry.get(target_actor_id).await else {
        tracing::debug!(
            target = target_actor_id,
            status = status_id,
            "TryStatus: target not in registry",
        );
        return false;
    };
    // Status effects use ms-precision for tick accounting. Convert
    // the seconds-precision `unix_timestamp()` helper into ms; this
    // drifts by at most 999 ms vs a true ms clock, which is well
    // below the finest granularity any ticking effect uses
    // (typically 3 s).
    let now_ms = (common::utils::unix_timestamp() as u64).saturating_mul(1000);
    let mut outbox = crate::status::StatusOutbox::new();
    let landed = {
        let mut c = target.character.write().await;
        let effect = crate::status::StatusEffect::new(
            target_actor_id,
            status_id,
            magnitude,
            tick_ms,
            duration_s,
            tier,
            now_ms,
        );
        c.status_effects.add_status_effect(
            effect,
            source_actor_id,
            now_ms,
            crate::status::DEFAULT_GAIN_TEXT_ID,
            &mut outbox,
        )
    };
    if !landed {
        tracing::debug!(
            source = source_actor_id,
            target = target_actor_id,
            status = status_id,
            "TryStatus: effect did not land (overwrite-rejected or table full)",
        );
        return false;
    }
    // Drain the outbox through the status dispatcher so packets /
    // DB save / on-gain Lua hooks fire. Dispatcher needs a
    // Catalogs Arc — reuse the LuaEngine's when available; fall
    // back to a fresh empty Catalogs for the test-harness case.
    let catalogs = lua
        .map(|e| e.catalogs().clone())
        .unwrap_or_else(|| std::sync::Arc::new(crate::lua::Catalogs::new()));
    for event in outbox.drain() {
        crate::runtime::dispatcher::dispatch_status_event(&event, registry, world, db, &catalogs)
            .await;
    }
    tracing::info!(
        source = source_actor_id,
        target = target_actor_id,
        status = status_id,
        duration_s,
        magnitude,
        "TryStatus applied",
    );
    true
}

/// Tier 4 #14 D — bazaar purchase drain helper. Thin wrapper over
/// [`Database::purchase_retainer_bazaar_item`] that logs the
/// outcome at the right level: `info` on success, `debug` for the
/// "legitimate rejection" paths (`InsufficientGil`, `ListingGone`,
/// `CannotBuyFromSelf`, `NoOwner`), `warn` only on actual DB
/// errors. Callers rarely need the outcome enum beyond logging;
/// the test harness can still reach the DB method directly when
/// it does.
pub async fn apply_purchase_retainer_bazaar_item(
    buyer_id: u32,
    retainer_id: u32,
    server_item_id: u64,
    db: &Database,
) -> Option<crate::database::PurchaseOutcome> {
    match db
        .purchase_retainer_bazaar_item(buyer_id, retainer_id, server_item_id)
        .await
    {
        Ok(outcome) => {
            tracing::info!(
                buyer = buyer_id,
                retainer = retainer_id,
                server_item = server_item_id,
                outcome = ?outcome,
                "PurchaseRetainerBazaarItem outcome",
            );
            Some(outcome)
        }
        Err(e) => {
            tracing::warn!(
                buyer = buyer_id,
                retainer = retainer_id,
                server_item = server_item_id,
                err = %e,
                "PurchaseRetainerBazaarItem: DB error",
            );
            None
        }
    }
}

/// Tier 3 #13 accept-side binding. The levemete counterpart to
/// [`apply_regional_leve_hand_in`]: installs the leve in the
/// player's journal with [`crate::leve::ACCEPTED_FLAG_BIT`] set and
/// the chosen difficulty band stamped on `counter2` so the
/// fieldcraft / battlecraft progress hooks tick correctly against
/// the band's objective quantity.
///
/// Returns `true` when a fresh journal entry was created, `false`
/// on any no-op path: missing catalog, missing player, missing
/// leve data row, journal full, already-accepted. The idempotent
/// already-in-journal path silently succeeds — retail levemetes
/// just re-render the "you already have this leve" dialog line.
pub async fn apply_accept_regional_leve(
    player_id: u32,
    leve_id: u32,
    difficulty: u8,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) -> bool {
    let Some(lua) = lua else {
        return false;
    };
    let Some(resolver) = lua.catalogs().regional_leve_resolver() else {
        return false;
    };
    if resolver.by_id(leve_id).is_none() {
        tracing::debug!(
            player = player_id,
            leve = leve_id,
            "AcceptRegionalLeve: leve id not in catalog",
        );
        return false;
    }
    let Some(handle) = registry.get(player_id).await else {
        return false;
    };
    // Clamp to the valid band range up front — mirrors
    // `RegionalLeveData::clamp_difficulty`. Saturating is cheaper
    // than failing since retail scripts sometimes pass the 1-indexed
    // UI band; we normalise to the 0-indexed storage band.
    let band = difficulty.min(3);

    let save_tuple = {
        let mut c = handle.character.write().await;
        if c.quest_journal.has(leve_id) {
            tracing::debug!(
                player = player_id,
                leve = leve_id,
                "AcceptRegionalLeve: already in journal (idempotent no-op)",
            );
            return false;
        }
        let actor_id = crate::actor::quest::quest_actor_id(leve_id);
        // Regional leves don't have `gamedata_quests` catalog rows
        // (they're a separate data model), so there's no
        // script-name lookup. Use a formulaic name so the DB row
        // is distinguishable in audits — same convention my test
        // fixtures used.
        let name = format!("leve{leve_id}");
        let mut quest = crate::actor::quest::Quest::new(actor_id, name);
        quest.set_flag(crate::leve::ACCEPTED_FLAG_BIT);
        quest.set_counter(1, band as u16);
        quest.clear_dirty();
        let Some(slot) = c.quest_journal.add(quest) else {
            tracing::warn!(
                player = player_id,
                leve = leve_id,
                "AcceptRegionalLeve: journal full",
            );
            return false;
        };
        let flags = 1u32 << crate::leve::ACCEPTED_FLAG_BIT;
        (slot as i32, actor_id, flags)
    };
    let (slot, actor_id, flags) = save_tuple;
    // save_quest params (per database.rs:2118): counter1 / counter2 /
    // counter3 = the DB column names. RegionalLeveView's
    // `set_counter(1, band)` writes the *in-memory* idx-1 counter,
    // which persists to the `counter2` DB column. So: counter1 = 0
    // (progress starts fresh), counter2 = band (difficulty),
    // counter3 = 0 (reserved).
    if let Err(e) = db
        .save_quest(player_id, slot, actor_id, 0, flags, 0, band as u16, 0)
        .await
    {
        tracing::warn!(
            player = player_id,
            leve = leve_id,
            err = %e,
            "AcceptRegionalLeve: DB persist failed",
        );
    }
    tracing::info!(
        player = player_id,
        leve = leve_id,
        slot,
        band,
        "AcceptRegionalLeve applied",
    );
    true
}

/// Outcome of a [`apply_regional_leve_hand_in`] call. Carried back
/// so callers (and tests) can assert exactly which side effects
/// fired without re-reading the DB for each assertion.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LeveHandInOutcome {
    /// `true` when the leve was in the journal + marked completed
    /// and the reward pipeline ran. `false` on any no-op path
    /// (leve not in journal, not completed, catalog row missing,
    /// etc.).
    pub applied: bool,
    pub gil_granted: i32,
    /// `(item_catalog_id, quantity)` when a reward item was
    /// granted. `None` when the band's `reward_item_id == 0` or
    /// the grant path was skipped.
    pub item_granted: Option<(u32, i32)>,
    /// `(gc, seals)` granted for an enlisted battlecraft hand-in.
    /// `None` for fieldcraft, unenlisted battlecraft, or any
    /// no-op path.
    pub seals_granted: Option<(u8, i32)>,
}

/// Tier 3 #13 reward payout + Tier 4 #16 C seal accrual.
/// Drain-side helper for the levemete hand-in flow. Semantics:
///
///  * the leve must be present in the player's journal AND have
///    `COMPLETED_FLAG_BIT` set (i.e. a prior `advance_progress`
///    call must already have saturated the objective) — otherwise
///    no rewards fire and the journal is left untouched;
///  * reward_gil for the active band is granted via
///    [`Database::add_gil`];
///  * if `reward_item_id[band] != 0`, that quantity of the item
///    lands in `characters_inventory` via
///    [`Database::add_harvest_item`];
///  * for battlecraft leves, if the player is enlisted (`gc_current
///    != 0`), Storm / Serpent / Flame seals are granted to their
///    current GC via [`Database::add_seals`] at a rate of
///    `reward_gil / 2` (placeholder — retail had dedicated per-leve
///    seal-reward columns mozk-tabetai doesn't publish). Fieldcraft
///    never grants seals.
///  * on success the leve is removed from the journal (in-memory +
///    DB) so the slot frees up for another levemete pickup.
///
/// Intended call sites: the future levemete-hand-in RPC
/// (`handInLeve` / `completeLeve` `callClientFunction`) and the
/// `LC::HandInRegionalLeve` runtime drain.
///
/// Short-circuits silently when the `RegionalLeveResolver` isn't
/// installed — catalogs hand out `None` and the no-op outcome lets
/// the caller distinguish "catalog missing" from "reward paid".
pub async fn apply_regional_leve_hand_in(
    player_id: u32,
    leve_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    lua: Option<&Arc<LuaEngine>>,
) -> LeveHandInOutcome {
    let mut outcome = LeveHandInOutcome::default();
    let Some(lua) = lua else {
        return outcome;
    };
    let Some(resolver) = lua.catalogs().regional_leve_resolver() else {
        return outcome;
    };
    let Some(data) = resolver.by_id(leve_id).cloned() else {
        return outcome;
    };
    let Some(handle) = registry.get(player_id).await else {
        return outcome;
    };

    // Snapshot everything we need + clear the journal slot, under
    // one write lock. If the leve isn't completed we bail without
    // touching the journal.
    let (band, gc_current, was_removed) = {
        let mut c = handle.character.write().await;
        let (is_completed, band) = {
            let Some(quest) = c.quest_journal.get(leve_id) else {
                return outcome;
            };
            // Read the same counter/flag positions
            // `RegionalLeveView` does, without constructing a
            // mutable view (we don't mutate through it here).
            let completed = quest.get_flag(crate::leve::COMPLETED_FLAG_BIT);
            let difficulty = quest.get_counter(1).min(3) as usize;
            (completed, difficulty)
        };
        if !is_completed {
            return outcome;
        }
        let gc = c.chara.gc_current;
        let removed = c.quest_journal.remove(leve_id).is_some();
        (band, gc, removed)
    };
    if !was_removed {
        return outcome;
    }

    // DB side: drop the scenario row so a fresh accept of the same
    // leve id starts from zero progress + flags.
    if let Err(e) = db.remove_quest(player_id, leve_id).await {
        tracing::warn!(
            player = player_id,
            leve = leve_id,
            err = %e,
            "LeveHandIn: DB scenario clear failed (journal was already updated in-memory)",
        );
    }

    // Rewards. Apply in the order retail's `handInLeve` ticks them
    // so the client's message log reads gil → item → seals.
    let gil = data.reward_gil.get(band).copied().unwrap_or(0);
    if gil > 0 {
        apply_add_gil(player_id, gil, db).await;
        outcome.gil_granted = gil;
    }
    let item_id = data.reward_item_id.get(band).copied().unwrap_or(0);
    let item_qty = data.reward_quantity.get(band).copied().unwrap_or(0);
    if item_id > 0 && item_qty > 0 {
        if let Err(e) = db
            .add_harvest_item(player_id, item_id as u32, item_qty, 1)
            .await
        {
            tracing::warn!(
                player = player_id,
                leve = leve_id,
                item = item_id,
                err = %e,
                "LeveHandIn: reward-item grant failed",
            );
        } else {
            outcome.item_granted = Some((item_id as u32, item_qty));
        }
    }
    // Seal accrual — battlecraft + enlisted only. Tier 4 #16 C.
    if data.leve_type == crate::leve::LeveType::Battlecraft
        && crate::actor::gc::is_valid_gc(gc_current)
    {
        let seals = gil / 2;
        if seals > 0 {
            match db.add_seals(player_id, gc_current, seals).await {
                Ok(_) => {
                    outcome.seals_granted = Some((gc_current, seals));
                }
                Err(e) => tracing::warn!(
                    player = player_id,
                    leve = leve_id,
                    gc = gc_current,
                    err = %e,
                    "LeveHandIn: seal accrual failed",
                ),
            }
        }
    }
    outcome.applied = true;
    tracing::info!(
        player = player_id,
        leve = leve_id,
        band,
        gil = outcome.gil_granted,
        item = ?outcome.item_granted,
        seals = ?outcome.seals_granted,
        "LeveHandIn applied",
    );
    outcome
}

pub async fn apply_add_gil(actor_id: u32, amount: i32, db: &Database) {
    if amount == 0 {
        return;
    }
    match db.add_gil(actor_id, amount).await {
        Ok(total) => {
            tracing::info!(actor = actor_id, delta = amount, total, "AddGil applied",);
        }
        Err(e) => {
            tracing::warn!(
                actor = actor_id,
                delta = amount,
                err = %e,
                "AddGil: DB persist failed",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// ENPC broadcast
// ---------------------------------------------------------------------------

pub(crate) async fn broadcast_quest_enpc_update(
    player_id: u32,
    enpc: QuestEnpc,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(player_handle) = registry.get(player_id).await else {
        return;
    };
    let session_id = player_handle.session_id;
    if session_id == 0 {
        return;
    }
    let Some(client) = world.client(session_id).await else {
        return;
    };
    // Resolve the player's CURRENT zone + private-area routing from the
    // session — the ActorHandle's zone_id is frozen at registration
    // (`reassign_zone` has no production callers), so after the man0g0
    // zone-166 → 155 warp the handle still says 166 and the SEQ_010
    // quest NPCs spawned into 155 would never resolve. Pre-warp call
    // sites read the same zone either way. (Garlemald-Server #28, req 4.)
    let (zone_id, requester_area) = match world.session(session_id).await {
        Some(s) if s.current_zone_id != 0 => (
            s.current_zone_id,
            s.current_private_area_name
                .clone()
                .map(|n| (n, s.current_private_area_level)),
        ),
        _ => (player_handle.zone_id, None),
    };
    let Some(npc_handle) = find_npc_by_class_id(
        registry,
        zone_id,
        enpc.actor_class_id,
        requester_area.as_ref(),
    )
    .await
    else {
        tracing::debug!(
            player = player_id,
            class_id = enpc.actor_class_id,
            "quest ENPC broadcast skipped — no live NPC",
        );
        return;
    };
    let (npc_actor_id, conditions) = {
        let c = npc_handle.character.read().await;
        (c.base.actor_id, c.base.event_conditions.clone())
    };

    let subpackets = crate::packets::send::build_actor_event_status_packets(
        npc_actor_id,
        &conditions,
        enpc.is_talk_enabled,
        enpc.is_emote_enabled,
        Some(enpc.is_push_enabled),
        true,
    );
    for mut sub in subpackets {
        // 1.x client silently drops event-related subpackets whose
        // SubPacketHeader.target_id != receiving actor's session id.
        // See `processor::broadcast_quest_enpc_update` for the longer
        // diagnosis — the upshot is that without setting target_id the
        // SetEventStatus + SetActorQuestGraphic broadcasts evaporate on
        // the wire and the client never updates the talk-arrow icon
        // when a quest's `onStateChange` swaps which ENPC is active.
        sub.set_target_id(player_id);
        client.send_bytes(sub.to_bytes()).await;
    }
    let mut graphic =
        crate::packets::send::build_set_actor_quest_graphic(npc_actor_id, enpc.quest_flag_type);
    graphic.set_target_id(player_id);
    client.send_bytes(graphic.to_bytes()).await;
}

/// Same-zone teleport for the quest-drain pipeline. Mirrors
/// `PacketProcessor::apply_warp_to_position` (the login-pipeline arm,
/// live-verified by the SEQ_005 content warp): mutate the pose, refresh
/// the session destination, emit a target-stamped `SetActorPosition`.
/// `player:SendGameMessage(...)` — emit the localized text-sheet line
/// (Meteor `GameMessagePacket` 0x157 "with actor ×1", WorldMaster as
/// the text owner) to the player's client, target-stamped for the
/// world relay. Mirrors `Player.SendGameMessage(sourceActor,
/// textIdOwner, textId, log)`.
pub(crate) async fn apply_send_game_message(
    actor_id: u32,
    text_id: u32,
    log_type: u8,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(actor_id).await else {
        return;
    };
    let session_id = handle.session_id;
    let Some(client) = world.client(session_id).await else {
        return;
    };
    let mut sub = crate::packets::send::build_game_message_actor1(
        crate::packets::send::WORLD_MASTER_ACTOR_ID,
        actor_id,
        crate::packets::send::WORLD_MASTER_ACTOR_ID,
        text_id.min(u16::MAX as u32) as u16,
        log_type,
    );
    sub.set_target_id(session_id);
    client.send_bytes(sub.to_bytes()).await;
    tracing::debug!(
        actor = actor_id,
        text_id,
        log = format!("0x{log_type:02X}"),
        "SendGameMessage emitted",
    );
}

/// Used by the `LC::WarpToPosition` arm above — quest `onPush` bounce
/// paths (`DoPlayerMoveInZone`) ride this.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn apply_warp_to_position_runtime(
    actor_id: u32,
    x: f32,
    y: f32,
    z: f32,
    rotation: f32,
    spawn_type: u8,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(actor_id).await else {
        tracing::debug!(actor = actor_id, "WarpToPosition: actor not in registry");
        return;
    };
    let session_id = handle.session_id;
    {
        let mut c = handle.character.write().await;
        c.base.position_x = x;
        c.base.position_y = y;
        c.base.position_z = z;
        c.base.rotation = rotation;
    }
    if let Some(mut session) = world.session(session_id).await {
        session.destination_x = x;
        session.destination_y = y;
        session.destination_z = z;
        session.destination_rot = rotation;
        session.destination_spawn_type = spawn_type;
        world.upsert_session(session).await;
    }
    if let Some(client) = world.client(session_id).await {
        let mut pkt = crate::packets::send::build_set_actor_position(
            actor_id,
            actor_id as i32,
            x,
            y,
            z,
            rotation,
            spawn_type.into(),
            false,
        );
        // Untargeted subpackets are dropped by the world-server proxy
        // fan-out (Garlemald-Server #28).
        pkt.set_target_id(session_id);
        client.send_bytes(pkt.to_bytes()).await;
        tracing::info!(
            actor = actor_id,
            x,
            y,
            z,
            rotation,
            spawn_type,
            "WarpToPosition (quest drain) applied + SetActorPosition emitted"
        );
    } else {
        tracing::debug!(
            actor = actor_id,
            "WarpToPosition (quest drain): no client handle — pose updated, no packet"
        );
    }
}

/// Graphic-only variant of [`broadcast_quest_enpc_update`] — re-emits the
/// head-marker (`SetActorQuestGraphic`) WITHOUT the SetEventStatus
/// overrides. Used by `apply_quest_update_enpcs`'s unchanged-entry
/// refresh: pmeteor's `QuestState.AddENpc` sends nothing for unchanged
/// registrations, so repeating the status overrides there diverges from
/// the reference — concretely it kept re-disabling the Ul'dah opening
/// stopper's own "exit"/"caution" push circles (registered with
/// `isPushEnabled=false` every sequence), undoing the spawn bundle's
/// per-condition defaults and letting the player walk out of the
/// Merchant Strip.
async fn broadcast_quest_enpc_graphic(
    player_id: u32,
    enpc: QuestEnpc,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(player_handle) = registry.get(player_id).await else {
        return;
    };
    let session_id = player_handle.session_id;
    if session_id == 0 {
        return;
    }
    let Some(client) = world.client(session_id).await else {
        return;
    };
    // Same session-driven zone + private-area resolution as
    // `broadcast_quest_enpc_update` — the ActorHandle's zone_id is
    // frozen at registration.
    let (zone_id, requester_area) = match world.session(session_id).await {
        Some(s) if s.current_zone_id != 0 => (
            s.current_zone_id,
            s.current_private_area_name
                .clone()
                .map(|n| (n, s.current_private_area_level)),
        ),
        _ => (player_handle.zone_id, None),
    };
    let Some(npc_handle) = find_npc_by_class_id(
        registry,
        zone_id,
        enpc.actor_class_id,
        requester_area.as_ref(),
    )
    .await
    else {
        return;
    };
    let npc_actor_id = {
        let c = npc_handle.character.read().await;
        c.base.actor_id
    };
    let mut graphic =
        crate::packets::send::build_set_actor_quest_graphic(npc_actor_id, enpc.quest_flag_type);
    graphic.set_target_id(player_id);
    client.send_bytes(graphic.to_bytes()).await;
}

async fn broadcast_quest_enpc_clear(
    player_id: u32,
    enpc: QuestEnpc,
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(player_handle) = registry.get(player_id).await else {
        return;
    };
    let session_id = player_handle.session_id;
    if session_id == 0 {
        return;
    }
    let Some(client) = world.client(session_id).await else {
        return;
    };
    // Session-resolved zone + area (see broadcast_quest_enpc_update).
    let (zone_id, requester_area) = match world.session(session_id).await {
        Some(s) if s.current_zone_id != 0 => (
            s.current_zone_id,
            s.current_private_area_name
                .clone()
                .map(|n| (n, s.current_private_area_level)),
        ),
        _ => (player_handle.zone_id, None),
    };
    let Some(npc_handle) = find_npc_by_class_id(
        registry,
        zone_id,
        enpc.actor_class_id,
        requester_area.as_ref(),
    )
    .await
    else {
        return;
    };
    let (npc_actor_id, conditions) = {
        let c = npc_handle.character.read().await;
        (c.base.actor_id, c.base.event_conditions.clone())
    };

    let subpackets = crate::packets::send::build_actor_event_status_packets(
        npc_actor_id,
        &conditions,
        false,
        false,
        Some(false),
        false,
    );
    for mut sub in subpackets {
        // Same target_id requirement as `broadcast_quest_enpc_update`.
        sub.set_target_id(player_id);
        client.send_bytes(sub.to_bytes()).await;
    }
    let mut graphic = crate::packets::send::build_set_actor_quest_graphic(npc_actor_id, 0);
    graphic.set_target_id(player_id);
    client.send_bytes(graphic.to_bytes()).await;
}

/// Resolve a quest ENPC by actor class id within a zone, preferring the
/// copy whose private-area pool matches the requesting player's routing.
/// Several city NPCs are seeded both at the zone root and inside a
/// `PrivateAreaMasterPast` phase under the same class id (Baderon,
/// Momodi, Miounne, …) — without the area preference this pick was
/// HashMap-iteration-order nondeterministic, and a broadcast bound to
/// the copy the client never spawned is silently dropped. Falls back to
/// a zone-root copy when the player's area has none (private-area player
/// whose quest NPC only exists at the root). (Garlemald-Server #28.)
async fn find_npc_by_class_id(
    registry: &ActorRegistry,
    zone_id: u32,
    class_id: u32,
    requester_area: Option<&(String, u32)>,
) -> Option<ActorHandle> {
    let actors = registry.actors_in_zone(zone_id).await;
    let mut root_match: Option<ActorHandle> = None;
    for h in actors {
        let matches = {
            let c = h.character.read().await;
            c.chara.actor_class_id == class_id
        };
        if !matches {
            continue;
        }
        match (&h.private_area, requester_area) {
            (Some(npc_area), Some(req)) if npc_area.as_ref() == req => return Some(h),
            (None, None) => return Some(h),
            // Root copy — keep as fallback for a private-area player.
            (None, Some(_)) if root_match.is_none() => root_match = Some(h),
            _ => {}
        }
    }
    root_match
}

// ---------------------------------------------------------------------------
// Cross-script quest dispatch — `quest:OnNotice(player)` triggered from a
// director script (`AfterQuestWarpDirector` et al.) routes through this
// helper so the target quest's `onNotice(player, quest, target)` hook
// fires with full command-drain support (unlike `fire_quest_hook` which
// only drains a narrow subset).
// ---------------------------------------------------------------------------

/// Dispatch a `quest:OnNotice(player)` call: look up the target quest's
/// script, build a fresh player snapshot + quest handle, invoke
/// `onNotice(player, quest, target)` via `spawn_blocking`, and drain any
/// emitted `LuaCommand`s through `apply_runtime_lua_commands` so scripted
/// side effects (flag flips, sequence starts, ENPC registration) land
/// after the cross-script hop.
///
/// No-ops quietly if:
/// * the player isn't in the registry,
/// * the player doesn't actually hold the quest (director may have
///   fired us after the quest was abandoned mid-zone-change),
/// * the quest id isn't in the catalog (no className → no script path),
/// * or the script file is missing on disk.
///
/// The `target` arg is fired as `nil` — mirroring how the C# LuaEngine
/// surfaces an unsupplied `triggerName` when directors call
/// `quest:OnNotice(player)` with just one arg.
pub async fn apply_quest_on_notice(
    player_id: u32,
    quest_id: u32,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(lua) = lua else {
        tracing::debug!(
            player = player_id,
            quest = quest_id,
            "quest:OnNotice dropped — no LuaEngine handle",
        );
        return;
    };
    let Some(handle) = registry.get(player_id).await else {
        tracing::debug!(
            player = player_id,
            quest = quest_id,
            "quest:OnNotice skipped — player not in registry",
        );
        return;
    };
    if !matches!(handle.kind, ActorKindTag::Player) {
        tracing::debug!(
            player = player_id,
            quest = quest_id,
            kind = ?handle.kind,
            "quest:OnNotice skipped — actor handle is not a Player",
        );
        return;
    }
    let Some(script_name) = lua.catalogs().quest_script_name(quest_id) else {
        tracing::debug!(
            player = player_id,
            quest = quest_id,
            "quest:OnNotice skipped — quest id not in catalog",
        );
        return;
    };
    tracing::debug!(
        player = player_id,
        quest = quest_id,
        script = %script_name,
        "quest:OnNotice — passed guards, running hook",
    );
    let script_path = lua.resolver().quest(&script_name);
    if !script_path.exists() {
        tracing::debug!(
            player = player_id,
            quest = quest_id,
            script = %script_path.display(),
            "quest:OnNotice skipped — script file missing",
        );
        return;
    }

    let (snapshot, quest_handle) = {
        let c = handle.character.read().await;
        if !c.quest_journal.has(quest_id) {
            tracing::debug!(
                player = player_id,
                quest = quest_id,
                "quest:OnNotice skipped — player no longer holds quest",
            );
            return;
        }
        let snap = crate::lua::userdata::PlayerSnapshot {
            actor_id: c.base.actor_id,
            name: c.base.actor_name.clone(),
            zone_id: c.base.zone_id,
            pos: (c.base.position_x, c.base.position_y, c.base.position_z),
            rotation: c.base.rotation,
            state: c.base.current_main_state,
            hp: c.chara.hp,
            max_hp: c.chara.max_hp,
            mp: c.chara.mp,
            max_mp: c.chara.max_mp,
            tp: c.chara.tp,
            active_quests: c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| q.quest_id())
                .collect(),
            active_quest_states: c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| crate::lua::QuestStateSnapshot {
                    quest_id: q.quest_id(),
                    sequence: q.get_sequence(),
                    flags: q.get_flags(),
                    counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
                    npc_ls_from: q.get_npc_ls_from(),
                    npc_ls_msg_step: q.get_npc_ls_msg_step(),
                })
                .collect(),
            completed_quests: c.quest_journal.iter_completed().collect(),
            ..Default::default()
        };
        let q = c
            .quest_journal
            .get(quest_id)
            .expect("quest_journal.has is true");
        let quest_handle = crate::lua::LuaQuestHandle {
            player_id: snap.actor_id,
            quest_id,
            has_quest: true,
            sequence: q.get_sequence(),
            flags: q.get_flags(),
            counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
            npc_ls_from: q.get_npc_ls_from(),
            npc_ls_msg_step: q.get_npc_ls_msg_step(),
            queue: crate::lua::command::CommandQueue::new(),
        };
        (snap, quest_handle)
    };

    let engine_clone = lua.clone();
    let script_path_clone = script_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        engine_clone.call_quest_hook(
            &script_path_clone,
            "onNotice",
            snapshot,
            quest_handle,
            Vec::new(),
        )
    })
    .await;

    let result = match result {
        Ok(r) => r,
        Err(join_err) => {
            tracing::warn!(
                error = %join_err,
                quest = quest_id,
                "quest:OnNotice dispatch panicked",
            );
            return;
        }
    };
    if let Some(e) = result.error {
        tracing::debug!(
            error = %e,
            quest = quest_id,
            "quest:OnNotice errored",
        );
    }
    if !result.commands.is_empty() {
        // The quest's `onNotice` hook is what kicks the per-city intro
        // cutscene — `callClientFunction(player, "delegateEvent", player,
        // quest, "processTtrNomal001withHQ")` becomes a
        // `LuaCommand::RunEventFunction` and `player:EndEvent()` becomes
        // a `LuaCommand::EndEvent`. Both are event-flavoured: they have
        // no arm in `apply_runtime_lua_command` and would be silently
        // logged as "unhandled" — the cutscene packets would never reach
        // the client and the player would sit at "Now Loading" forever.
        // Translate them into the EventOutbox first (using the player's
        // EventSession for the in-flight event_owner / event_name /
        // event_type that the bridge needs), drain through
        // `dispatch_event_event` to actually emit the
        // `RunEventFunctionPacket` / `EndEventPacket`, then fall through
        // to the runtime apply for non-event commands. Mirrors the
        // pattern in `event::dispatcher::dispatch_director_event_started`.
        let event_session_snapshot = {
            let c = handle.character.read().await;
            c.event_session.clone()
        };
        let mut outbox = crate::event::outbox::EventOutbox::new();
        crate::event::lua_bridge::translate_lua_commands_into_outbox(
            &result.commands,
            &event_session_snapshot,
            &mut outbox,
        );
        for e in outbox.drain() {
            // Box::pin matches the recursion-guard already used below.
            Box::pin(crate::event::dispatcher::dispatch_event_event(
                &e,
                registry,
                world,
                db,
                Some(lua),
            ))
            .await;
        }

        // `apply_runtime_lua_commands` → ... → `apply_quest_on_notice`
        // is a potential recursion cycle (an `onNotice` hook could emit
        // another `QuestOnNotice`). Box the future so the compiler
        // doesn't need a statically-known size.
        Box::pin(apply_runtime_lua_commands(
            result.commands,
            registry,
            db,
            world,
            Some(lua),
        ))
        .await;

        // The opening-cutscene hook (e.g. `man0g0.lua::onNotice`) runs:
        //
        //     callClientFunction(player, "delegateEvent", player, quest,
        //                        "processTtrNomal001withHQ")
        //     player:EndEvent()
        //     quest:UpdateENPCs()
        //
        // `callClientFunction` (scripts/lua/global.lua) does
        // `coroutine.yield("_WAIT_EVENT", player)`, so the coroutine parks
        // after emitting just the `RunEventFunction` (0x0130) above. The
        // remaining lines — `player:EndEvent()` (0x0131, which frees the
        // client from event-locked state) and `quest:UpdateENPCs()` — must
        // run ONLY after the client has finished playing the cinematic.
        // The 1.x client signals completion with an `EventUpdate` (0x012E),
        // which `handle_event_update` → `dispatch_event_updated_drain` →
        // `fire_player_event_and_drain` uses to resume this parked coroutine
        // (the player-0 bare-string park is picked up by that path's
        // `take_event(0)` fallback). This mirrors Meteor's
        // `LuaEngine.OnEventUpdate` (`Map Server/Lua/LuaEngine.cs`).
        //
        // We deliberately DO NOT auto-fire the parked coroutine here. A
        // previous version did — to dodge a hang back when the `target_id`
        // bug made the client ignore event subpackets and never send the
        // `EventUpdate` — but that emitted `EndEvent` in the same pass as
        // `RunEventFunction`, tearing the event down mid-cinematic and
        // crashing the client at the dialog center→top-left handoff, before
        // movement control was granted. With `target_id` now set to the
        // player (see `set_target_id` in event/dispatcher.rs) the client
        // receives the cutscene and drives the resume itself, so leaving the
        // coroutine parked is both correct and necessary.
    }
}

/// Proximity-push dispatcher: walk the player's active quests, find
/// any push-enabled ENPC the quest registered via `quest:SetENpc(...,
/// canPush=true)`, look up its actor in the same zone, compute the
/// distance to the player, and if it's inside the trigger radius fire
/// `onPush(player, quest, npc)` once. The hook's emitted commands
/// (typically `callClientFunction("delegateEvent", "processTtr...") +
/// player:EndEvent()`) flow through the same EventOutbox bridge that
/// `talkto` uses.
///
/// Tracking the already-fired pushes lives on the per-quest
/// `QuestState::recently_pushed` set: without it, the hook would re-
/// fire on every inbound `0x00CA UpdatePlayerPosition` packet (~3 per
/// second), spamming the client with the same cutscene call. The set
/// is cleared on every `begin_sequence_swap` so a new sequence can
/// re-arm pushes for the same NPCs.
///
/// Trigger radius is hardcoded at 3.0 world units — matches the
/// 1.x-era retail observed value (most NPC-side `pushOffsetXZ` values
/// in the spawn data sit between 2 and 4 units).
#[allow(clippy::too_many_arguments)]
pub async fn check_quest_proximity_pushes(
    player_id: u32,
    pos: (f32, f32, f32),
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(lua) = lua else { return };
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    if !matches!(handle.kind, ActorKindTag::Player) {
        return;
    }

    // Snapshot the (quest_id, push-enabled class_ids, recently_pushed
    // set) tuples so we can iterate without holding the character
    // write lock across `await` points.
    let quest_pushes: Vec<(u32, Vec<u32>)> = {
        let c = handle.character.read().await;
        c.quest_journal
            .slots
            .iter()
            .flatten()
            .map(|q| {
                let to_check: Vec<u32> = q
                    .state
                    .current
                    .values()
                    .filter(|e| {
                        e.is_push_enabled && !q.state.recently_pushed.contains(&e.actor_class_id)
                    })
                    .map(|e| e.actor_class_id)
                    .collect();
                (q.quest_id(), to_check)
            })
            .filter(|(_, v)| !v.is_empty())
            .collect()
    };
    if quest_pushes.is_empty() {
        return;
    }

    let zone_id = {
        let c = handle.character.read().await;
        c.base.zone_id
    };
    let zone_actors = registry.actors_in_zone(zone_id).await;

    const TRIGGER_RADIUS: f32 = 3.0;
    let trigger_radius_sq = TRIGGER_RADIUS * TRIGGER_RADIUS;

    for (quest_id, class_ids) in quest_pushes {
        for class_id in class_ids {
            // Find the NPC's live actor in this zone with matching class.
            let mut npc_handle = None;
            for h in &zone_actors {
                let m = {
                    let c = h.character.read().await;
                    c.chara.actor_class_id == class_id
                };
                if m {
                    npc_handle = Some(h.clone());
                    break;
                }
            }
            let Some(npc_handle) = npc_handle else {
                continue;
            };

            let npc_spec = {
                let c = npc_handle.character.read().await;
                let dx = pos.0 - c.base.position_x;
                let dz = pos.2 - c.base.position_z;
                let dist_sq = dx * dx + dz * dz;
                // Per-class circle radius from the parsed event
                // conditions — the client arms its pushDefault circle
                // with this exact value (the Carline Canopy exit is
                // r=6.0), so a flat 3.0 server gate rejects pushes the
                // client legitimately fired from 3-6y out. (#28, req 4.)
                let class_radius_sq = c
                    .base
                    .event_conditions
                    .push_circle
                    .iter()
                    .map(|pc| pc.radius * pc.radius)
                    .fold(trigger_radius_sq, f32::max);
                if dist_sq > class_radius_sq {
                    continue;
                }
                crate::lua::LuaNpcSpec {
                    actor_id: c.base.actor_id,
                    name: c.base.actor_name.clone(),
                    class_name: c.base.class_name.clone(),
                    class_path: c.base.class_path.clone(),
                    unique_id: String::new(),
                    zone_id: c.base.zone_id,
                    zone_name: String::new(),
                    state: c.base.current_main_state,
                    pos: (c.base.position_x, c.base.position_y, c.base.position_z),
                    rotation: c.base.rotation,
                    actor_class_id: c.chara.actor_class_id,
                    quest_graphic: 0,
                }
            };

            // Mark as pushed BEFORE firing the hook — if the hook
            // somehow re-enters this path (recursive event dispatch),
            // we don't want a double-fire.
            {
                let mut c = handle.character.write().await;
                if let Some(q) = c.quest_journal.get_mut(quest_id) {
                    q.state.recently_pushed.insert(class_id);
                }
            }

            tracing::info!(
                player = player_id,
                quest = quest_id,
                npc_class = class_id,
                "proximity push triggered",
            );

            fire_quest_on_push_via_command(
                &handle,
                quest_id,
                npc_spec,
                registry,
                db,
                world,
                Some(lua),
            )
            .await;
        }
    }
}

/// Proximity-push dispatcher (KickEvent variant). Walks the player's
/// active quests, finds any push-enabled ENPC inside trigger radius,
/// and emits `KickEventPacket("pushDefault", owner=npc_actor_id,
/// type=2)` directly to the client. The 1.x client responds with an
/// `EventStart(eventType=2, owner=npc)` which lands in
/// `processor::handle_event_start` — that handler sets up the
/// player's `EventSession` (owner / event_name / event_type) and runs
/// the per-quest `onPush` fan-out within that active context, so the
/// resulting `RunEventFunction` packet from the script's
/// `callClientFunction(...)` carries the correct event-routing fields.
///
/// Same per-actor-class debouncing as the older
/// `check_quest_proximity_pushes` (via `QuestState::recently_pushed`)
/// so we don't spam KickEvent on every position update inside the
/// trigger radius.
pub async fn kick_quest_proximity_pushes(
    player_id: u32,
    session_id: u32,
    pos: (f32, f32, f32),
    registry: &ActorRegistry,
    world: &WorldManager,
) {
    let Some(handle) = registry.get(player_id).await else {
        return;
    };
    if !matches!(handle.kind, ActorKindTag::Player) {
        return;
    }
    let Some(client) = world.client(session_id).await else {
        return;
    };

    // Snapshot the (quest_id, push-enabled class_ids) tuples. Filter
    // out class ids that already fired this sequence — without this
    // the trigger re-fires every ~350ms while the player sits inside
    // the radius.
    #[allow(clippy::type_complexity)]
    let (quest_pushes, journal_summary): (Vec<(u32, Vec<u32>)>, Vec<(u32, usize, usize)>) = {
        let c = handle.character.read().await;
        let pushes: Vec<(u32, Vec<u32>)> = c
            .quest_journal
            .slots
            .iter()
            .flatten()
            .map(|q| {
                let to_check: Vec<u32> = q
                    .state
                    .current
                    .values()
                    .filter(|e| {
                        e.is_push_enabled && !q.state.recently_pushed.contains(&e.actor_class_id)
                    })
                    .map(|e| e.actor_class_id)
                    .collect();
                (q.quest_id(), to_check)
            })
            .filter(|(_, v)| !v.is_empty())
            .collect();
        let summary: Vec<(u32, usize, usize)> = c
            .quest_journal
            .slots
            .iter()
            .flatten()
            .map(|q| {
                let push_enabled = q
                    .state
                    .current
                    .values()
                    .filter(|e| e.is_push_enabled)
                    .count();
                (q.quest_id(), q.state.current.len(), push_enabled)
            })
            .collect();
        (pushes, summary)
    };
    if quest_pushes.is_empty() {
        if !journal_summary.is_empty() {
            // Throttle: only log every Nth call so we don't spam.
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            if n.is_multiple_of(30) {
                tracing::info!(
                    player = player_id,
                    journal = ?journal_summary,
                    "proximity push: nothing to check (no push-enabled, un-fired ENPCs)",
                );
            }
        }
        return;
    }
    tracing::info!(
        player = player_id,
        pos = ?pos,
        quest_pushes = ?quest_pushes,
        "proximity push: walking active push-enabled NPCs",
    );

    let zone_id = {
        let c = handle.character.read().await;
        c.base.zone_id
    };
    let zone_actors = registry.actors_in_zone(zone_id).await;

    const TRIGGER_RADIUS: f32 = 3.0;
    let trigger_radius_sq = TRIGGER_RADIUS * TRIGGER_RADIUS;

    for (quest_id, class_ids) in quest_pushes {
        for class_id in class_ids {
            // Find the NPC's live actor in this zone with matching class.
            let mut npc_handle = None;
            for h in &zone_actors {
                let m = {
                    let c = h.character.read().await;
                    c.chara.actor_class_id == class_id
                };
                if m {
                    npc_handle = Some(h.clone());
                    break;
                }
            }
            let Some(npc_handle) = npc_handle else {
                continue;
            };

            let npc_actor_id = {
                let c = npc_handle.character.read().await;
                let dx = pos.0 - c.base.position_x;
                let dz = pos.2 - c.base.position_z;
                let dist_sq = dx * dx + dz * dz;
                // Per-class circle radius (see check_quest_proximity_pushes).
                let class_radius_sq = c
                    .base
                    .event_conditions
                    .push_circle
                    .iter()
                    .map(|pc| pc.radius * pc.radius)
                    .fold(trigger_radius_sq, f32::max);
                if dist_sq > class_radius_sq {
                    continue;
                }
                c.base.actor_id
            };

            // Mark as pushed BEFORE firing so a re-entrant position-
            // update inside the radius doesn't double-kick.
            {
                let mut c = handle.character.write().await;
                if let Some(q) = c.quest_journal.get_mut(quest_id) {
                    q.state.recently_pushed.insert(class_id);
                }
            }

            tracing::info!(
                player = player_id,
                quest = quest_id,
                npc_class = class_id,
                npc_actor = format!("0x{:08X}", npc_actor_id),
                "proximity push: kicking pushDefault on client",
            );

            // Build + send `KickEventPacket("pushDefault", owner=npc,
            // event_type=2)`. Server-side `EventSession` is left alone —
            // the client's reply EventStart(eventType=2) is what
            // populates it via `processor::handle_event_start`.
            let mut sub = crate::packets::send::events::build_kick_event(
                player_id,
                npc_actor_id,
                "pushDefault",
                2,
                &[],
            );
            sub.set_target_id(player_id);
            client.send_bytes(sub.to_bytes()).await;
        }
    }
}

/// Fire a quest's `onPush(player, quest, npc)` hook. Mirrors
/// `fire_quest_on_talk_via_command` exactly — the only difference is
/// the hook name. Both run the script, bridge event-flavoured
/// commands into the EventOutbox, drain the rest via runtime apply,
/// and auto-resume any `_WAIT_EVENT`-parked coroutine so
/// `player:EndEvent()` after `callClientFunction` doesn't stall.
#[allow(clippy::too_many_arguments)]
pub async fn fire_quest_on_push_via_command(
    handle: &ActorHandle,
    quest_id: u32,
    npc_spec: crate::lua::LuaNpcSpec,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    fire_quest_npc_hook_via_command(
        handle, quest_id, "onPush", npc_spec, registry, db, world, lua,
    )
    .await;
}

/// Shared backend for `fire_quest_on_talk_via_command` and
/// `fire_quest_on_push_via_command` — runs the named hook with a
/// `(player, quest, npc)` arg list and drains commands through the
/// event-outbox + runtime-apply pipelines. Auto-resumes parked
/// `_WAIT_EVENT` coroutines.
#[allow(clippy::too_many_arguments)]
async fn fire_quest_npc_hook_via_command(
    handle: &ActorHandle,
    quest_id: u32,
    hook_name: &'static str,
    npc_spec: crate::lua::LuaNpcSpec,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(lua) = lua else { return };
    if !matches!(handle.kind, ActorKindTag::Player) {
        return;
    }
    let Some(script_name) = lua.catalogs().quest_script_name(quest_id) else {
        return;
    };
    let script_path = lua.resolver().quest(&script_name);
    if !script_path.exists() {
        return;
    }

    let (snapshot, quest_handle) = {
        let c = handle.character.read().await;
        if !c.quest_journal.has(quest_id) {
            return;
        }
        let snap = crate::lua::userdata::PlayerSnapshot {
            actor_id: c.base.actor_id,
            name: c.base.actor_name.clone(),
            zone_id: c.base.zone_id,
            pos: (c.base.position_x, c.base.position_y, c.base.position_z),
            rotation: c.base.rotation,
            state: c.base.current_main_state,
            hp: c.chara.hp,
            max_hp: c.chara.max_hp,
            mp: c.chara.mp,
            max_mp: c.chara.max_mp,
            tp: c.chara.tp,
            active_quests: c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| q.quest_id())
                .collect(),
            active_quest_states: c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| crate::lua::QuestStateSnapshot {
                    quest_id: q.quest_id(),
                    sequence: q.get_sequence(),
                    flags: q.get_flags(),
                    counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
                    npc_ls_from: q.get_npc_ls_from(),
                    npc_ls_msg_step: q.get_npc_ls_msg_step(),
                })
                .collect(),
            completed_quests: c.quest_journal.iter_completed().collect(),
            ..Default::default()
        };
        let q = c.quest_journal.get(quest_id).expect("has");
        let qh = crate::lua::LuaQuestHandle {
            player_id: snap.actor_id,
            quest_id,
            has_quest: true,
            sequence: q.get_sequence(),
            flags: q.get_flags(),
            counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
            npc_ls_from: q.get_npc_ls_from(),
            npc_ls_msg_step: q.get_npc_ls_msg_step(),
            queue: crate::lua::command::CommandQueue::new(),
        };
        (snap, qh)
    };

    // Reset the server-side `EventSession` to its zero / no-event
    // state before running the Lua hook. Without this, `EventSession`
    // still carries the previous event's `current_event_owner`,
    // `current_event_name` (e.g. `"noticeEvent"` from the
    // OpeningDirector login cinematic) and `current_event_type`,
    // because retail-shaped `LuaCommand::EndEvent` only dispatches an
    // `EndEventPacket` to the client — it never calls
    // `EventSession::end_event` server-side, so the server's view of
    // "what event is active" never gets cleared.
    //
    // Concretely: the next `callClientFunction` from the Lua hook
    // produces a `RunEventFunction` packet whose owner /
    // event_name / event_type are inherited from that stale session.
    // The client has already torn the prior event down (it received
    // the EndEvent packet), so it sees a `RunEventFunction` for an
    // event it doesn't think exists and silently drops it — visible
    // symptom: walking into Rostnsthal's push radius after the
    // opening cinematic does nothing on screen, even though the
    // server-side proximity dispatcher fires and the 712-byte packet
    // is on the wire.
    //
    // Zeroing the session — `(owner=0, name="", type=0)` — mirrors
    // how `EventSession::end_event` would have left it, and matches
    // the working "no cinematic enabled" baseline from earlier in
    // this session, where the proximity push's `RunEventFunction`
    // packet went out with owner=0 / event_name="" and the client
    // accepted it as a free-form scripted call.
    {
        let mut c = handle.character.write().await;
        c.event_session.current_event_owner = 0;
        c.event_session.current_event_name.clear();
        c.event_session.current_event_type = 0;
    }

    let lua_clone = lua.clone();
    let extra = vec![crate::lua::QuestHookArg::Npc(npc_spec)];
    let result = tokio::task::spawn_blocking(move || {
        lua_clone.call_quest_hook(&script_path, hook_name, snapshot, quest_handle, extra)
    })
    .await;
    let result = match result {
        Ok(r) => r,
        Err(join_err) => {
            tracing::warn!(error = %join_err, quest = quest_id, hook = hook_name, "panicked");
            return;
        }
    };
    if let Some(e) = result.error {
        tracing::debug!(error = %e, quest = quest_id, hook = hook_name, "errored");
    }
    if result.commands.is_empty() {
        return;
    }

    let event_session_snapshot = {
        let c = handle.character.read().await;
        c.event_session.clone()
    };
    let mut outbox = crate::event::outbox::EventOutbox::new();
    crate::event::lua_bridge::translate_lua_commands_into_outbox(
        &result.commands,
        &event_session_snapshot,
        &mut outbox,
    );
    for e in outbox.drain() {
        Box::pin(crate::event::dispatcher::dispatch_event_event(
            &e,
            registry,
            world,
            db,
            Some(lua),
        ))
        .await;
    }
    Box::pin(apply_runtime_lua_commands(
        result.commands,
        registry,
        db,
        world,
        Some(lua),
    ))
    .await;

    // Auto-resume parked `_WAIT_EVENT` coroutine.
    let player_id = handle.actor_id;
    if let Some(after) = lua.fire_player_event_and_drain(player_id, &[])
        && !after.is_empty()
    {
        let session_after = {
            let c = handle.character.read().await;
            c.event_session.clone()
        };
        let mut outbox = crate::event::outbox::EventOutbox::new();
        crate::event::lua_bridge::translate_lua_commands_into_outbox(
            &after,
            &session_after,
            &mut outbox,
        );
        for e in outbox.drain() {
            Box::pin(crate::event::dispatcher::dispatch_event_event(
                &e,
                registry,
                world,
                db,
                Some(lua),
            ))
            .await;
        }
        Box::pin(apply_runtime_lua_commands(
            after,
            registry,
            db,
            world,
            Some(lua),
        ))
        .await;
    }
}

/// Fire a quest's `onTalk(player, quest, npc)` hook on behalf of an
/// out-of-band caller (currently the GM `talkto` command). Mirrors
/// `PacketProcessor::fire_quest_hook` + the EventOutbox bridge step
/// from `apply_quest_on_notice`: runs the hook, translates the
/// event-flavoured commands (RunEventFunction / EndEvent / KickEvent)
/// into the outbox so their packets actually reach the client, then
/// falls through to `apply_runtime_lua_commands` for the rest.
///
/// Without this, `talkto` only fires `EventStarted` against the NPC's
/// class script, and the actual cutscene-driving lines in
/// `man0l0.lua::seq000_onTalk` (the ROSTNSTHAL branch that calls
/// `processTtrNomal003`) never run.
#[allow(clippy::too_many_arguments)]
pub async fn fire_quest_on_talk_via_command(
    handle: &ActorHandle,
    quest_id: u32,
    npc_spec: crate::lua::LuaNpcSpec,
    registry: &ActorRegistry,
    db: &Database,
    world: &WorldManager,
    lua: Option<&Arc<LuaEngine>>,
) {
    let Some(lua) = lua else { return };
    if !matches!(handle.kind, ActorKindTag::Player) {
        return;
    }
    let Some(script_name) = lua.catalogs().quest_script_name(quest_id) else {
        return;
    };
    let script_path = lua.resolver().quest(&script_name);
    if !script_path.exists() {
        return;
    }

    let (snapshot, quest_handle) = {
        let c = handle.character.read().await;
        if !c.quest_journal.has(quest_id) {
            return;
        }
        let snap = crate::lua::userdata::PlayerSnapshot {
            actor_id: c.base.actor_id,
            name: c.base.actor_name.clone(),
            zone_id: c.base.zone_id,
            pos: (c.base.position_x, c.base.position_y, c.base.position_z),
            rotation: c.base.rotation,
            state: c.base.current_main_state,
            hp: c.chara.hp,
            max_hp: c.chara.max_hp,
            mp: c.chara.mp,
            max_mp: c.chara.max_mp,
            tp: c.chara.tp,
            active_quests: c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| q.quest_id())
                .collect(),
            active_quest_states: c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| crate::lua::QuestStateSnapshot {
                    quest_id: q.quest_id(),
                    sequence: q.get_sequence(),
                    flags: q.get_flags(),
                    counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
                    npc_ls_from: q.get_npc_ls_from(),
                    npc_ls_msg_step: q.get_npc_ls_msg_step(),
                })
                .collect(),
            completed_quests: c.quest_journal.iter_completed().collect(),
            ..Default::default()
        };
        let q = c
            .quest_journal
            .get(quest_id)
            .expect("has(quest_id) is true");
        let qh = crate::lua::LuaQuestHandle {
            player_id: snap.actor_id,
            quest_id,
            has_quest: true,
            sequence: q.get_sequence(),
            flags: q.get_flags(),
            counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
            npc_ls_from: q.get_npc_ls_from(),
            npc_ls_msg_step: q.get_npc_ls_msg_step(),
            queue: crate::lua::command::CommandQueue::new(),
        };
        (snap, qh)
    };

    let lua_clone = lua.clone();
    let extra = vec![crate::lua::QuestHookArg::Npc(npc_spec)];
    let result = tokio::task::spawn_blocking(move || {
        lua_clone.call_quest_hook(&script_path, "onTalk", snapshot, quest_handle, extra)
    })
    .await;
    let result = match result {
        Ok(r) => r,
        Err(join_err) => {
            tracing::warn!(error = %join_err, quest = quest_id, "onTalk panicked");
            return;
        }
    };
    if let Some(e) = result.error {
        tracing::debug!(error = %e, quest = quest_id, "onTalk errored");
    }

    if result.commands.is_empty() {
        return;
    }

    // Bridge step — translate event-flavoured commands into the
    // EventOutbox, drain through `dispatch_event_event`. Same
    // pattern as `apply_quest_on_notice` and the patched
    // `dispatch_npc_event_started`.
    let event_session_snapshot = {
        let c = handle.character.read().await;
        c.event_session.clone()
    };
    let mut outbox = crate::event::outbox::EventOutbox::new();
    crate::event::lua_bridge::translate_lua_commands_into_outbox(
        &result.commands,
        &event_session_snapshot,
        &mut outbox,
    );
    for e in outbox.drain() {
        Box::pin(crate::event::dispatcher::dispatch_event_event(
            &e,
            registry,
            world,
            db,
            Some(lua),
        ))
        .await;
    }
    // Then drain the rest (quest flag mutates, AddExp, UpdateENPCs,
    // etc.) through the regular runtime apply pipeline.
    Box::pin(apply_runtime_lua_commands(
        result.commands,
        registry,
        db,
        world,
        Some(lua),
    ))
    .await;

    // Auto-resume any coroutine the onTalk hook parked via
    // `callClientFunction`'s `coroutine.yield("_WAIT_EVENT", player)`.
    // Mirrors the same auto-resume in `apply_quest_on_notice` — without
    // it, `player:EndEvent()` after `callClientFunction` never runs and
    // the client stays in event-locked state.
    let player_id = handle.actor_id;
    if let Some(after) = lua.fire_player_event_and_drain(player_id, &[])
        && !after.is_empty()
    {
        let session_after = {
            let c = handle.character.read().await;
            c.event_session.clone()
        };
        let mut outbox = crate::event::outbox::EventOutbox::new();
        crate::event::lua_bridge::translate_lua_commands_into_outbox(
            &after,
            &session_after,
            &mut outbox,
        );
        for e in outbox.drain() {
            Box::pin(crate::event::dispatcher::dispatch_event_event(
                &e,
                registry,
                world,
                db,
                Some(lua),
            ))
            .await;
        }
        Box::pin(apply_runtime_lua_commands(
            after,
            registry,
            db,
            world,
            Some(lua),
        ))
        .await;
    }
}

// ---------------------------------------------------------------------------
// Lua hook firing — mirror of `PacketProcessor::fire_quest_hook` that
// drains emitted commands back through `apply_runtime_lua_command`.
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn fire_quest_hook(
    handle: &ActorHandle,
    quest_id: u32,
    hook_name: &str,
    extra_args: Vec<crate::lua::QuestHookArg>,
    lua: &Arc<LuaEngine>,
    registry: &ActorRegistry,
    db: &Database,
    world: Option<&WorldManager>,
) {
    // Skip Lua work on actors that aren't Players — NPCs / BattleNpcs
    // carry a default-empty quest_journal but shouldn't ever reach this
    // path in practice, and a missing session id would drop any
    // downstream packet anyway.
    if !matches!(handle.kind, ActorKindTag::Player) {
        return;
    }
    let Some(script_name) = lua.catalogs().quest_script_name(quest_id) else {
        return;
    };
    let script_path = lua.resolver().quest(&script_name);
    if !script_path.exists() {
        return;
    }

    let (snapshot, quest_handle) = {
        let c = handle.character.read().await;
        let snap = crate::lua::userdata::PlayerSnapshot {
            actor_id: c.base.actor_id,
            name: c.base.actor_name.clone(),
            zone_id: c.base.zone_id,
            pos: (c.base.position_x, c.base.position_y, c.base.position_z),
            rotation: c.base.rotation,
            state: c.base.current_main_state,
            hp: c.chara.hp,
            max_hp: c.chara.max_hp,
            mp: c.chara.mp,
            max_mp: c.chara.max_mp,
            tp: c.chara.tp,
            active_quests: c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| q.quest_id())
                .collect(),
            active_quest_states: c
                .quest_journal
                .slots
                .iter()
                .flatten()
                .map(|q| crate::lua::QuestStateSnapshot {
                    quest_id: q.quest_id(),
                    sequence: q.get_sequence(),
                    flags: q.get_flags(),
                    counters: [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
                    npc_ls_from: q.get_npc_ls_from(),
                    npc_ls_msg_step: q.get_npc_ls_msg_step(),
                })
                .collect(),
            completed_quests: c.quest_journal.iter_completed().collect(),
            ..Default::default()
        };
        let quest = c
            .quest_journal
            .get(quest_id)
            .map(|q| {
                (
                    q.get_sequence(),
                    q.get_flags(),
                    [q.get_counter(0), q.get_counter(1), q.get_counter(2)],
                    q.get_npc_ls_from(),
                    q.get_npc_ls_msg_step(),
                )
            })
            .unwrap_or((0, 0, [0; 3], 0, 0));
        let handle = crate::lua::LuaQuestHandle {
            player_id: snap.actor_id,
            quest_id,
            has_quest: c.quest_journal.has(quest_id),
            sequence: quest.0,
            flags: quest.1,
            counters: quest.2,
            npc_ls_from: quest.3,
            npc_ls_msg_step: quest.4,
            queue: crate::lua::command::CommandQueue::new(),
        };
        (snap, handle)
    };

    let lua_clone = lua.clone();
    let hook_name_owned = hook_name.to_string();
    let result = tokio::task::spawn_blocking(move || {
        lua_clone.call_quest_hook(
            &script_path,
            &hook_name_owned,
            snapshot,
            quest_handle,
            extra_args,
        )
    })
    .await;
    let result = match result {
        Ok(r) => r,
        Err(join_err) => {
            tracing::warn!(error = %join_err, quest = quest_id, hook = hook_name, "hook panicked");
            return;
        }
    };
    if let Some(e) = result.error {
        tracing::debug!(error = %e, quest = quest_id, hook = hook_name, "hook errored");
    }

    // Drain emitted commands. When the caller has a `WorldManager`,
    // route them through `apply_runtime_lua_commands` so world-needing
    // commands (QuestSetEnpc → broadcast_quest_enpc_update,
    // QuestUpdateEnpcs, QuestStartSequence's stale-drain) actually fire.
    // Without this, re-running `onStateChange` (e.g. via
    // `apply_quest_update_enpcs` after a cinematic noticeEvent) would
    // produce SetENpc lua commands that get silently dropped — leaving
    // the post-cinematic `current` ENPC set empty and `drain_stale_enpcs`
    // broadcasting CLEAR (SetEventStatus enabled=0) for every NPC that
    // was supposed to remain active. Symptom: walking into Yda after
    // the man0g0 opening cinematic does nothing because her pushDefault
    // and talkDefault triggers were just disabled.
    //
    // Callers without a `world` (apply_complete_quest /
    // apply_abandon_quest's onFinish, plus the apply_add_quest onStart
    // path which today is reached via the processor's login pipeline)
    // pass `None` and the legacy log-and-drop behaviour is preserved.
    // Box::pin handles the recursive future size since hooks can emit
    // AddQuest which re-enters fire_quest_hook.
    if !result.commands.is_empty() {
        if let Some(world) = world {
            tracing::debug!(
                quest = quest_id,
                hook = hook_name,
                commands = result.commands.len(),
                "draining hook commands through apply_runtime_lua_commands",
            );
            Box::pin(apply_runtime_lua_commands(
                result.commands,
                registry,
                db,
                world,
                Some(lua),
            ))
            .await;
        } else {
            tracing::debug!(
                quest = quest_id,
                hook = hook_name,
                commands = result.commands.len(),
                "hook emitted runtime commands (not drained from fire_quest_hook)",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Character;
    use crate::actor::quest::{Quest, quest_actor_id};
    use crate::runtime::actor_registry::ActorKindTag;

    fn tmpdir() -> std::path::PathBuf {
        // Two parallel tests landing on the same nanosecond tick would
        // share this dir and clobber each other's scripts; the atomic
        // counter guarantees uniqueness.
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("garlemald-onnotice-{nanos}-{seq}"));
        std::fs::create_dir_all(dir.join("quests/man")).unwrap();
        dir
    }

    fn tempdb() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("garlemald-onnotice-{nanos}-{seq}.db"))
    }

    /// `apply_quest_on_notice` resolves the script, fires
    /// `onNotice(player, quest, target)`, and drains any commands the
    /// hook emits. We have the hook flip a quest flag bit so we can
    /// assert both halves (hook ran, drain applied) from one side
    /// effect.
    #[tokio::test]
    async fn apply_quest_on_notice_fires_hook_and_drains_commands() {
        let root = tmpdir();
        std::fs::write(
            root.join("quests/man/man0l1.lua"),
            r#"
                function onNotice(player, quest, target)
                    quest:SetQuestFlag(3)
                end
            "#,
        )
        .unwrap();

        let lua = Arc::new(LuaEngine::new(&root));
        {
            let mut quests = std::collections::HashMap::new();
            quests.insert(
                110_002u32,
                crate::gamedata::QuestMeta {
                    id: 110_002,
                    quest_name: "Call of the Sea".to_string(),
                    class_name: "Man0l1".to_string(),
                    prerequisite: 0,
                    min_level: 1,
                },
            );
            lua.catalogs().install_quests(quests);
        }

        let registry = ActorRegistry::new();
        let mut character = Character::new(13);
        let mut quest = Quest::new(quest_actor_id(110_002), "Man0l1".to_string());
        quest.clear_dirty();
        character.quest_journal.add(quest);
        let handle = ActorHandle::new(13, ActorKindTag::Player, 100, 42, character);
        registry.insert(handle.clone()).await;
        let world = WorldManager::new();
        let db = crate::database::Database::open(tempdb())
            .await
            .expect("db stub");

        apply_quest_on_notice(13, 110_002, &registry, &db, &world, Some(&lua)).await;

        // The onNotice hook's `SetQuestFlag(3)` should have walked the
        // drain → `apply_quest_mutation` → `Quest::set_flag(3)`, leaving
        // bit 3 set on the live quest in the registry.
        let flags = {
            let c = handle.character.read().await;
            c.quest_journal
                .get(110_002)
                .map(|q| q.get_flags())
                .unwrap_or(0)
        };
        assert_eq!(
            flags & (1 << 3),
            1 << 3,
            "onNotice should have set flag bit 3 via drained SetQuestFlag",
        );

        let _ = std::fs::remove_dir_all(root);
    }

    /// Missing `onNotice` function is a quiet no-op — mirrors how
    /// `AfterQuestWarpDirector` can fire `quest:OnNotice` on any quest
    /// in the journal without every script defining the hook.
    #[tokio::test]
    async fn apply_quest_on_notice_is_a_quiet_no_op_when_hook_missing() {
        let root = tmpdir();
        // Script with no onNotice — just a top-level global assignment
        // so load_script succeeds.
        std::fs::write(
            root.join("quests/man/man0l0.lua"),
            "_no_notice_defined = true",
        )
        .unwrap();

        let lua = Arc::new(LuaEngine::new(&root));
        {
            let mut quests = std::collections::HashMap::new();
            quests.insert(
                110_001u32,
                crate::gamedata::QuestMeta {
                    id: 110_001,
                    quest_name: "Shapeless Melody".to_string(),
                    class_name: "Man0l0".to_string(),
                    prerequisite: 0,
                    min_level: 1,
                },
            );
            lua.catalogs().install_quests(quests);
        }

        let registry = ActorRegistry::new();
        let mut character = Character::new(21);
        let mut quest = Quest::new(quest_actor_id(110_001), "Man0l0".to_string());
        quest.clear_dirty();
        character.quest_journal.add(quest);
        let handle = ActorHandle::new(21, ActorKindTag::Player, 100, 42, character);
        registry.insert(handle.clone()).await;
        let world = WorldManager::new();
        let db = crate::database::Database::open(tempdb())
            .await
            .expect("db stub");

        // Should not panic; should not emit any side effects.
        apply_quest_on_notice(21, 110_001, &registry, &db, &world, Some(&lua)).await;

        let flags = {
            let c = handle.character.read().await;
            c.quest_journal
                .get(110_001)
                .map(|q| q.get_flags())
                .unwrap_or(0)
        };
        assert_eq!(flags, 0, "missing onNotice leaves flags untouched");

        let _ = std::fs::remove_dir_all(root);
    }

    /// Player-not-in-registry (e.g. the director fired OnNotice after
    /// a fast logout) is a quiet no-op. Guard the happy path from
    /// panicking on a stale cross-script reference.
    #[tokio::test]
    async fn apply_quest_on_notice_skips_unknown_player() {
        let root = tmpdir();
        let lua = Arc::new(LuaEngine::new(&root));

        let registry = ActorRegistry::new();
        let world = WorldManager::new();
        let db = crate::database::Database::open(tempdb())
            .await
            .expect("db stub");

        apply_quest_on_notice(9999, 110_001, &registry, &db, &world, Some(&lua)).await;
        // Assertion here is "no panic". The function walks out of the
        // `registry.get` branch without touching the LuaEngine.
        let _ = std::fs::remove_dir_all(root);
    }

    /// B3: `apply_set_actor_mod` writes the value through to the
    /// target character's `ModifierMap`. Tests both registered actors
    /// (success) and unknown actors (quiet skip).
    #[tokio::test]
    async fn apply_set_actor_mod_writes_modifier_map() {
        let registry = ActorRegistry::new();
        let actor_id = 0x1234_5678u32;
        let character = Character::new(actor_id);
        registry
            .insert(crate::runtime::actor_registry::ActorHandle::new(
                actor_id,
                ActorKindTag::Player,
                /* zone */ 166,
                /* session */ 0,
                character,
            ))
            .await;

        // MinimumHpLock = 114 → 1.0
        apply_set_actor_mod(actor_id, 114, 1, &registry).await;

        let handle = registry.get(actor_id).await.expect("registered");
        let c = handle.character.read().await;
        let lock = c.chara.mods.get_raw(114);
        assert_eq!(lock, 1.0);
    }

    #[tokio::test]
    async fn apply_set_actor_mod_unknown_actor_is_quiet() {
        let registry = ActorRegistry::new();
        // 0xDEADBEEF isn't registered — the call should log+return
        // without panicking.
        apply_set_actor_mod(0xDEAD_BEEF, 114, 1, &registry).await;
    }

    /// Phase C3 — `apply_actor_engage` pushes the actor's AIContainer
    /// into a `BattleState::Attack` state with the target locked in.
    /// Subsequent ticks drive auto-attack swings through this state.
    #[tokio::test]
    async fn c3_apply_actor_engage_pushes_attack_state() {
        let registry = ActorRegistry::new();
        let actor_id = 0x4000_0001u32;
        let target_id = 0x4000_0099u32;
        let character = Character::new(actor_id);
        registry
            .insert(crate::runtime::actor_registry::ActorHandle::new(
                actor_id,
                ActorKindTag::Ally,
                166,
                0,
                character,
            ))
            .await;

        // Pre-condition: not engaged.
        {
            let handle = registry.get(actor_id).await.expect("registered");
            let c = handle.character.read().await;
            assert!(!c.ai_container.is_engaged());
        }

        apply_actor_engage(actor_id, target_id, &registry, &WorldManager::new()).await;

        // Post-condition: engaged, current state targets `target_id`.
        let handle = registry.get(actor_id).await.expect("registered");
        let c = handle.character.read().await;
        assert!(
            c.ai_container.is_engaged(),
            "ally should be engaged after Engage()"
        );
        let cs = c
            .ai_container
            .current_state()
            .expect("battle state should exist");
        assert_eq!(cs.target_actor_id, target_id);
    }

    /// Phase C3 — re-engaging the same target with `apply_actor_engage`
    /// is a quiet no-op (matches C# `Controller::Engage`'s `if IsEngaged`
    /// gate). Re-engaging would clobber the existing swing clock.
    #[tokio::test]
    async fn c3_apply_actor_engage_when_already_engaged_is_noop() {
        let registry = ActorRegistry::new();
        let actor_id = 0x4000_0001u32;
        let target_id = 0x4000_0099u32;
        registry
            .insert(crate::runtime::actor_registry::ActorHandle::new(
                actor_id,
                ActorKindTag::Ally,
                166,
                0,
                Character::new(actor_id),
            ))
            .await;

        apply_actor_engage(actor_id, target_id, &registry, &WorldManager::new()).await;
        // Second call with a different target should NOT change state
        // — re-engage is gated on `IsEngaged` (use ChangeTarget for
        // retargets).
        apply_actor_engage(actor_id, 0x4000_00AAu32, &registry, &WorldManager::new()).await;

        let handle = registry.get(actor_id).await.expect("registered");
        let c = handle.character.read().await;
        assert_eq!(
            c.ai_container.current_state().unwrap().target_actor_id,
            target_id,
            "second Engage with different target should not retarget the existing state",
        );
    }

    /// Phase C3 — `apply_hate_container_add_base_hate` seeds a base
    /// hate entry. Without this seed, `most_hated()` returns None and
    /// `should_deaggro` fires on the very next combat tick.
    #[tokio::test]
    async fn c3_apply_hate_container_add_base_hate_seeds_entry() {
        let registry = ActorRegistry::new();
        let actor_id = 0x4000_0001u32;
        let target_id = 0x4000_0099u32;
        registry
            .insert(crate::runtime::actor_registry::ActorHandle::new(
                actor_id,
                ActorKindTag::Ally,
                166,
                0,
                Character::new(actor_id),
            ))
            .await;

        apply_hate_container_add_base_hate(actor_id, target_id, &registry).await;

        let handle = registry.get(actor_id).await.expect("registered");
        let c = handle.character.read().await;
        assert!(
            c.hate.has_hate_for(target_id),
            "base hate entry should exist"
        );
    }

    /// Phase C3 — apply paths quietly skip when target=0 or actor
    /// isn't registered. Mirrors `apply_set_actor_mod_unknown_actor_is_quiet`.
    #[tokio::test]
    async fn c3_apply_actor_engage_skips_zero_target_and_missing_actor() {
        let registry = ActorRegistry::new();
        // No registered actors — both calls should log+return.
        apply_actor_engage(0xDEAD_BEEF, 0x4000_0099, &registry, &WorldManager::new()).await;
        apply_actor_engage(0xDEAD_BEEF, 0, &registry, &WorldManager::new()).await;
        apply_hate_container_add_base_hate(0xDEAD_BEEF, 0x4000_0099, &registry).await;
        apply_hate_container_add_base_hate(0xDEAD_BEEF, 0, &registry).await;
    }

    /// Garlemald-Server #28 — `load_initial_equipment_pairs` builds the
    /// `&[(equip_slot, catalog_id)]` slice both the FIX-A mid-session
    /// re-send and the FIX-B zone-in bundle feed into
    /// `build_set_initial_equipment`. Verifies (a) the slot→catalog
    /// mapping is correct, (b) the result is sorted by equip slot
    /// (deterministic packet bytes), and (c) a character with no
    /// equipment yields an empty slice (the legacy zone-in behaviour).
    #[tokio::test]
    async fn load_initial_equipment_pairs_builds_sorted_slot_value_slice() {
        let db = crate::database::Database::open(tempdb()).await.unwrap();

        // No equipment yet → empty slice (must stay valid for zone-in).
        let empty = load_initial_equipment_pairs(&db, 42, 3).await;
        assert!(empty.is_empty(), "no equipment → empty slice");

        // Seed two bag items, equip them into out-of-order gear slots, and
        // confirm the pairs come back sorted with the right catalog ids.
        // `add_harvest_item` lands NORMAL slots 0 (4030010) and 1 (8050245).
        db.add_harvest_item(42, 4030010, 1, 1).await.unwrap();
        db.add_harvest_item(42, 8050245, 1, 1).await.unwrap();
        let (weapon_sid, _) = db
            .resolve_bag_slot_item_id(42, 0, 0)
            .await
            .unwrap()
            .unwrap();
        let (legs_sid, _) = db
            .resolve_bag_slot_item_id(42, 0, 1)
            .await
            .unwrap()
            .unwrap();
        // Equip legs (slot 10) BEFORE main-hand (slot 0) so insertion
        // order is reversed vs. the expected sorted output.
        db.equip_item(42, 3, 10, legs_sid, false).await.unwrap();
        db.equip_item(
            42,
            3,
            crate::actor::player::SLOT_MAINHAND,
            weapon_sid,
            false,
        )
        .await
        .unwrap();

        let pairs = load_initial_equipment_pairs(&db, 42, 3).await;
        assert_eq!(pairs.len(), 2, "two equipped slots");
        // Sorted by equip slot: main-hand (0) first, legs (10) second.
        assert_eq!(pairs[0].0, crate::actor::player::SLOT_MAINHAND);
        assert_eq!(pairs[0].1, 4030010, "main-hand catalog id");
        assert_eq!(pairs[1].0, 10);
        assert_eq!(pairs[1].1, 8050245, "legs catalog id");
    }
}

#[cfg(test)]
mod change_state_self_send_tests {
    use super::*;
    use crate::actor::Character;
    use crate::data::ClientHandle;
    use crate::runtime::actor_registry::ActorKindTag;
    use crate::zone::area::StoredActor;
    use crate::zone::navmesh::StubNavmeshLoader;
    use crate::zone::{ActorKind, Zone};
    use common::Vector3;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tokio::sync::mpsc;

    /// A player toggling their own main state (the F-press Engage) must
    /// receive their own 0x0134 SetActorState — `broadcast_around_actor`
    /// excludes the source, so without the explicit self-send the weapon
    /// never draws on the acting client (`recipients=0` in the SEQ_005
    /// live test). (Garlemald-Server #28.)
    #[tokio::test]
    async fn change_state_reaches_the_acting_player() {
        let world = WorldManager::new();
        let registry = ActorRegistry::new();

        let mut zone = Zone::new(
            166,
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
        let mut ob = crate::zone::outbox::AreaOutbox::new();
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
        world.register_zone(zone).await;

        registry
            .insert(crate::runtime::actor_registry::ActorHandle::new(
                1,
                ActorKindTag::Player,
                166,
                7,
                Character::new(1),
            ))
            .await;
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(4);
        world.register_client(7, ClientHandle::new(7, tx)).await;

        apply_change_state(1, 2, &registry, &world).await;

        let got = rx.try_recv().expect("player should receive own state trio");
        let mut off = 0usize;
        let sub = common::subpacket::SubPacket::parse(&got, &mut off).unwrap();
        assert_eq!(sub.game_message.opcode, 0x0134);
        assert_eq!(
            sub.header.target_id, 7,
            "self-send must stamp the session id"
        );
        assert_eq!(sub.data[0], 2, "main_state byte");
        // pmeteor parity: the draw-animation battle actions ride along.
        // Byte layout re-pinned to the retail/pmeteor container after
        // the S0.6 re-lay (pmeteor capture map-packets.log:38304ff —
        // F-press 0x13C + 0x139 trio members): sourceActorId at +0x00,
        // animationId at +0x04, commandId at +0x24, row from +0x28.
        let x00 = common::subpacket::SubPacket::parse(&got, &mut off).unwrap();
        assert_eq!(x00.game_message.opcode, 0x013C);
        assert_eq!(x00.header.target_id, 7);
        assert_eq!(&x00.data[..4], &1u32.to_le_bytes());
        assert_eq!(&x00.data[4..8], &0x7200_0062u32.to_le_bytes());
        let x01 = common::subpacket::SubPacket::parse(&got, &mut off).unwrap();
        assert_eq!(x01.game_message.opcode, 0x0139);
        assert_eq!(&x01.data[..4], &1u32.to_le_bytes());
        assert_eq!(&x01.data[4..8], &0x7C00_0062u32.to_le_bytes());
        assert_eq!(&x01.data[0x24..0x26], &21001u16.to_le_bytes());
        assert_eq!(
            &x01.data[0x28..0x2C],
            &1u32.to_le_bytes(),
            "self-targeted draw row"
        );
        assert_eq!(&x01.data[0x30..0x34], &1u32.to_le_bytes(), "effectId 1");
        assert_eq!(x01.data[0x35], 1, "hitNum 1");

        // Stored state mutated too.
        let handle = registry.get(1).await.unwrap();
        let c = handle.character.read().await;
        assert_eq!(c.base.current_main_state, 2);
    }
}

#[cfg(test)]
mod actor_engage_clock_tests {
    use super::*;
    use crate::actor::Character;
    use crate::battle::outbox::{BattleEvent, BattleOutbox};
    use crate::runtime::actor_registry::ActorKindTag;
    use crate::zone::area::StoredActor;
    use crate::zone::navmesh::StubNavmeshLoader;
    use crate::zone::{ActorKind, Zone};
    use common::Vector3;

    /// Regression test for "Yda engages but never swings" (#28 S0.2):
    /// `apply_actor_engage` must arm `AttackState.next_swing_ms` in the
    /// same clock domain the ticker drives `AIContainer::update` with
    /// (`runtime/clock.rs`). With the old epoch-ms arming, the swing
    /// stayed ~56 years out and this test's single update tick emitted
    /// nothing.
    #[tokio::test]
    async fn script_engage_swings_on_the_shared_clock() {
        let world = WorldManager::new();
        let registry = ActorRegistry::new();

        let mut zone = Zone::new(
            166,
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
        let mut ob = crate::zone::outbox::AreaOutbox::new();
        for (id, pos) in [(1u32, Vector3::ZERO), (2u32, Vector3::new(2.0, 0.0, 0.0))] {
            zone.core.add_actor(
                StoredActor {
                    actor_id: id,
                    kind: ActorKind::BattleNpc,
                    position: pos,
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );
        }
        world.register_zone(zone).await;

        for (id, kind) in [(1u32, ActorKindTag::Ally), (2u32, ActorKindTag::BattleNpc)] {
            let mut c = Character::new(id);
            c.chara.hp = 100;
            c.chara.max_hp = 100;
            registry
                .insert(crate::runtime::actor_registry::ActorHandle::new(
                    id, kind, 166, 0, c,
                ))
                .await;
        }

        // Script-driven engage (the `allyGlobal.EngageTarget` path).
        apply_actor_engage(1, 2, &registry, &world).await;

        let handle = registry.get(1).await.unwrap();
        let delay = { handle.character.read().await.get_attack_delay_ms() };
        // One AI update past the swing window, in the shared domain.
        let now_ms = crate::runtime::clock::server_now_ms() + delay as u64 + 10;
        let zone_arc = world.zone(166).await.unwrap();
        let mut outbox = BattleOutbox::new();
        {
            let zone_read = zone_arc.read().await;
            let mut c = handle.character.write().await;
            let view = crate::runtime::ticker::build_owner_view(&c, 1, 166);
            c.ai_container
                .update(now_ms, view, &*zone_read, &mut outbox);
        }
        let events = outbox.drain();
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::ResolveAutoAttack {
                    attacker_actor_id: 1,
                    defender_actor_id: 2,
                }
            )),
            "engaged ally must emit ResolveAutoAttack on the first ready tick; got {events:?}",
        );
    }
}

#[cfg(test)]
mod move_actor_grid_sync_tests {
    use super::*;
    use crate::actor::Character;
    use crate::runtime::actor_registry::ActorKindTag;
    use crate::zone::area::StoredActor;
    use crate::zone::navmesh::StubNavmeshLoader;
    use crate::zone::{ActorKind, Zone};
    use common::Vector3;

    /// `apply_move_actor_to_position` must re-insert the actor into the
    /// zone's spatial grid — `actors_around*` (the AI arena) reads the
    /// grid, and without the sync it diverges from the authoritative
    /// `Character.base.position_*` the moment anything moves. (#28 S0.3.)
    #[tokio::test]
    async fn move_actor_updates_the_spatial_grid() {
        let world = WorldManager::new();
        let registry = ActorRegistry::new();

        let mut zone = Zone::new(
            166,
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
        let mut ob = crate::zone::outbox::AreaOutbox::new();
        zone.core.add_actor(
            StoredActor {
                actor_id: 9,
                kind: ActorKind::BattleNpc,
                position: Vector3::ZERO,
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        world.register_zone(zone).await;
        registry
            .insert(crate::runtime::actor_registry::ActorHandle::new(
                9,
                ActorKindTag::BattleNpc,
                166,
                0,
                Character::new(9),
            ))
            .await;

        // Move far enough to cross grid cells (BOUNDING_GRID_SIZE = 50).
        apply_move_actor_to_position(9, 500.0, 4.0, 500.0, 0.0, 2, &registry, &world).await;

        let zone_arc = world.zone(166).await.unwrap();
        let zone_read = zone_arc.read().await;
        let at_new: Vec<u32> = zone_read
            .core
            .actors_around_point(500.0, 500.0, 10.0)
            .iter()
            .map(|a| a.actor_id)
            .collect();
        let at_old: Vec<u32> = zone_read
            .core
            .actors_around_point(0.0, 0.0, 10.0)
            .iter()
            .map(|a| a.actor_id)
            .collect();
        assert!(at_new.contains(&9), "grid must track the new position");
        assert!(!at_old.contains(&9), "grid must drop the old position");
        // The stored Character position moved too.
        let c = registry.get(9).await.unwrap();
        let c = c.character.read().await;
        assert_eq!(
            (c.base.position_x, c.base.position_z),
            (500.0, 500.0),
            "authoritative position must match the grid",
        );
    }
}
