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

//! Top-level AI orchestration. Port of
//! `Actors/Chara/Ai/AIContainer.cs` + `Helpers/ActionQueue.cs`.
//!
//! The AIContainer holds the state stack, the controller, the pathfinder,
//! and the target finder. Each tick:
//!
//! 1. If the controller is present and `can_update`, poll it for a
//!    `ControllerDecision`.
//! 2. Apply the decision (engage, disengage, change target, move, roam,
//!    or fire a Lua hook — each routed through the outbox).
//! 3. Tick the top state; pop if it signals Complete or Interrupted.
//! 4. Emit a `RecalcStats` event if the state stack emptied during the
//!    tick (mirrors `Character.PostUpdate`).

use common::Vector3;

use super::command::BattleCommand;
use super::controller::{Controller, ControllerDecision, ControllerKind, ControllerOwnerView};
use super::hate::HateContainer;
use super::outbox::{BattleEvent, BattleOutbox};
use super::path_find::PathFind;
use super::state::{BattleState, BattleStateKind, MAX_STATE_STACK, StateTickResult};
use super::target_find::ActorArena;

/// One tick's worth of movement intent, produced by `apply_decision` from
/// the controller's `MoveTo` / `FaceTarget` decisions and consumed by the
/// game ticker (#28 S2.2 Option B — straight-line step integration; the
/// `PathFind` port stays unused for this flat arena). The ticker applies
/// the step only when `can_change_state()` holds — a Magic state on top
/// suppresses movement, which also satisfies pmeteor's HasMoved
/// cast-interrupt rule without implementing interrupts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MovementStep {
    /// Walk `speed * tick` toward this stop-ring destination.
    MoveTo { destination: Vector3 },
    /// Already at the ring — face this position, no translation.
    Face { position: Vector3 },
}

/// Action-queue entry — a scheduled callback to fire after `fire_at_ms`.
#[derive(Debug, Clone)]
pub struct Action {
    pub fire_at_ms: u64,
    /// Whether the AIContainer should gate this on `can_change_state`
    /// before running.
    pub check_state: bool,
    /// Opaque caller-defined tag; the game loop decides what to do with
    /// it (e.g. fire a Lua function, emit a packet).
    pub tag: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ActionQueue {
    pub entries: Vec<Action>,
}

impl ActionQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, action: Action) {
        self.entries.push(action);
        self.entries.sort_by_key(|a| a.fire_at_ms);
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Pop every action whose `fire_at_ms` has elapsed.
    pub fn drain_due(&mut self, now_ms: u64) -> Vec<Action> {
        let mut ready = Vec::new();
        while let Some(first) = self.entries.first() {
            if first.fire_at_ms <= now_ms {
                ready.push(self.entries.remove(0));
            } else {
                break;
            }
        }
        ready
    }
}

// ---------------------------------------------------------------------------
// AIContainer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct AIContainer {
    pub owner_actor_id: u32,
    pub controller: Option<Controller>,
    pub path_find: Option<PathFind>,
    pub action_queue: ActionQueue,
    /// This tick's movement intent (#28 S2.2) — cleared at the top of
    /// every `update`, set by `apply_decision`, taken by the ticker.
    pub pending_movement: Option<MovementStep>,
    /// Steps taken since the current pursuit started — drives the
    /// ticker's every-3rd-step 0x00CF throttle.
    pub movement_step_count: u32,

    states: Vec<BattleState>,
    last_action_time_ms: u64,
    latest_update_ms: u64,
    prev_update_ms: u64,
}

impl AIContainer {
    pub fn new(
        owner_actor_id: u32,
        controller: Option<Controller>,
        path_find: Option<PathFind>,
    ) -> Self {
        Self {
            owner_actor_id,
            controller,
            path_find,
            action_queue: ActionQueue::new(),
            pending_movement: None,
            movement_step_count: 0,
            states: Vec::new(),
            last_action_time_ms: 0,
            latest_update_ms: 0,
            prev_update_ms: 0,
        }
    }

    // --- Stack queries -----------------------------------------------------

    pub fn current_state(&self) -> Option<&BattleState> {
        self.states.last()
    }

    pub fn current_state_mut(&mut self) -> Option<&mut BattleState> {
        self.states.last_mut()
    }

    pub fn state_stack(&self) -> &[BattleState] {
        &self.states
    }

    pub fn is_current(&self, kind: BattleStateKind) -> bool {
        self.current_state().map(|s| s.kind) == Some(kind)
    }

    pub fn can_change_state(&self) -> bool {
        self.current_state()
            .map(|s| s.can_change_state())
            .unwrap_or(true)
    }

    pub fn can_follow_path(&self) -> bool {
        self.path_find.is_some() && self.can_change_state()
    }

    pub fn is_engaged(&self) -> bool {
        // Any Attack state in the STACK counts, not just the top — a
        // casting NPC carries [Attack, Magic] and is still engaged. The
        // top-only read made `is_engaged()` flip false mid-cast, which
        // (a) let the controller's retaliate branch force-push a second
        // Attack on top of the Magic state and (b) made the content
        // script's `not ally:IsEngaged()` guard re-engage a busy caster.
        // (#28 S2.4.)
        self.states
            .iter()
            .any(|s| s.kind == BattleStateKind::Attack)
    }

    pub fn is_dead(&self) -> bool {
        self.current_state()
            .map(|s| s.kind == BattleStateKind::Death)
            .unwrap_or(false)
    }

    pub fn is_spawned(&self) -> bool {
        !self.is_dead()
    }

    pub fn last_action_time_ms(&self) -> u64 {
        self.last_action_time_ms
    }

    pub fn update_last_action_time(&mut self, now_ms: u64, delay_seconds: u32) {
        self.last_action_time_ms = now_ms + (delay_seconds as u64 * 1000);
    }

    // --- Mutation helpers --------------------------------------------------

    /// `ChangeState(state)` — push if allowed.
    pub fn change_state(&mut self, state: BattleState) -> bool {
        if !self.can_change_state() {
            return false;
        }
        if self.states.len() >= MAX_STATE_STACK {
            return false;
        }
        self.check_completed_states();
        self.states.push(state);
        true
    }

    /// `ForceChangeState(state)` — bypass the interrupt/gate check.
    pub fn force_change_state(&mut self, state: BattleState) -> bool {
        if self.states.len() >= MAX_STATE_STACK {
            return false;
        }
        self.check_completed_states();
        self.states.push(state);
        true
    }

    /// `CheckCompletedStates()` — pop while the top is completed.
    pub fn check_completed_states(&mut self) {
        while self.states.last().map(|s| s.is_completed).unwrap_or(false) {
            self.states.pop();
        }
    }

    /// `InterruptStates()` — set interrupt on every interruptible state
    /// and pop them.
    pub fn interrupt_states(&mut self) {
        while self.states.last().map(|s| s.can_interrupt).unwrap_or(false) {
            if let Some(mut top) = self.states.pop() {
                top.set_interrupted(true);
            }
        }
    }

    /// `ClearStates()` — pop everything.
    pub fn clear_states(&mut self) {
        self.states.clear();
    }

    pub fn reset(&mut self) {
        self.clear_states();
        if let Some(pf) = &mut self.path_find {
            pf.clear();
        }
    }

    // --- Public Internal* dispatch (mirrors the C#) -----------------------

    pub fn internal_engage(
        &mut self,
        target_actor_id: u32,
        now_ms: u64,
        attack_delay_ms: u32,
    ) -> bool {
        if self.is_engaged() {
            // Already engaged — a retarget is handled via ChangeTarget.
            return false;
        }
        let state = BattleState::attack(
            self.owner_actor_id,
            target_actor_id,
            now_ms,
            attack_delay_ms,
        );
        self.force_change_state(state)
    }

    pub fn internal_disengage(&mut self, outbox: &mut BattleOutbox) {
        if let Some(pf) = &mut self.path_find {
            pf.clear();
        }
        self.clear_states();
        outbox.push(BattleEvent::Disengage {
            owner_actor_id: self.owner_actor_id,
        });
    }

    pub fn internal_cast(&mut self, target_actor_id: u32, cmd: BattleCommand, now_ms: u64) -> bool {
        let state = BattleState::magic(self.owner_actor_id, target_actor_id, cmd, now_ms);
        self.change_state(state)
    }

    pub fn internal_ability(
        &mut self,
        target_actor_id: u32,
        cmd: BattleCommand,
        now_ms: u64,
    ) -> bool {
        let state = BattleState::ability(self.owner_actor_id, target_actor_id, cmd, now_ms);
        self.change_state(state)
    }

    pub fn internal_weapon_skill(
        &mut self,
        target_actor_id: u32,
        cmd: BattleCommand,
        now_ms: u64,
    ) -> bool {
        let state = BattleState::weapon_skill(self.owner_actor_id, target_actor_id, cmd, now_ms);
        self.change_state(state)
    }

    pub fn internal_use_item(
        &mut self,
        target_actor_id: u32,
        item_id: u32,
        cast_time_ms: u32,
        now_ms: u64,
    ) -> bool {
        let state = BattleState::item(
            self.owner_actor_id,
            target_actor_id,
            item_id,
            cast_time_ms,
            now_ms,
        );
        self.change_state(state)
    }

    pub fn internal_die(&mut self, now_ms: u64, fadeout_ms: u64, outbox: &mut BattleOutbox) {
        if let Some(pf) = &mut self.path_find {
            pf.clear();
        }
        self.clear_states();
        self.force_change_state(BattleState::death(self.owner_actor_id, now_ms, fadeout_ms));
        outbox.push(BattleEvent::Die {
            owner_actor_id: self.owner_actor_id,
        });
    }

    pub fn internal_despawn(&mut self, now_ms: u64, respawn_ms: u64, outbox: &mut BattleOutbox) {
        self.clear_states();
        self.force_change_state(BattleState::despawn(
            self.owner_actor_id,
            now_ms,
            respawn_ms,
        ));
        outbox.push(BattleEvent::Despawn {
            owner_actor_id: self.owner_actor_id,
        });
    }

    // --- Main tick --------------------------------------------------------

    pub fn update(
        &mut self,
        now_ms: u64,
        owner_view: ControllerOwnerView,
        arena: &dyn ActorArena,
        outbox: &mut BattleOutbox,
    ) {
        self.prev_update_ms = self.latest_update_ms;
        self.latest_update_ms = now_ms;
        // Movement intent is one-tick state — stale intents from a tick
        // whose step never applied (cast gate, lock contention) must not
        // replay. (#28 S2.2.)
        self.pending_movement = None;

        // Controller-less actors just follow paths (C#: plain FollowPath
        // without the controller).
        if self.controller.is_none() && self.path_find.is_some() {
            // MovementSink isn't available here; the game loop wraps the
            // actor and calls `follow_path` directly when needed.
        }

        // Run the controller, then apply its decision. We pull the decision out
        // of the `&mut self.controller` borrow before calling `apply_decision`
        // (which needs `&mut self` to push an AttackState on Engage).
        let controller_decision = if let Some(ctrl) = self.controller.as_mut()
            && ctrl.can_update
        {
            Some(ctrl.tick(now_ms, owner_view, arena))
        } else {
            None
        };
        if let Some(decision) = controller_decision {
            self.apply_decision(decision, now_ms, owner_view.attack_delay_ms, outbox);
        }

        // Process the state stack. Auto-attack swings (Attack state, ready)
        // and cast completions (Magic/Ability/WeaponSkill → Complete) emit
        // resolve events for the dispatcher to fan into CommandResult packets.
        let owner_id = self.owner_actor_id;
        let swing_delay_ms = owner_view.attack_delay_ms.max(500);
        // Swing gates (#28 S2.3, pmeteor `AttackState.CanAttack` parity):
        // the owner must have auto-attack enabled (players have no
        // controller → default true) and the target must be inside
        // attack range. The swing clock re-arms regardless — arriving
        // mid-window waits out the window. Facing gate DEFERRED (the
        // movement step already faces the target).
        let auto_attack_enabled = self
            .controller
            .as_ref()
            .is_none_or(|c| c.auto_attack_enabled);
        let attack_range = owner_view.attack_range;
        let owner_pos = owner_view.actor.position;
        while let Some(top) = self.states.last_mut() {
            let kind = top.kind;
            let target_actor_id = top.target_actor_id;
            let tick_result = top.update(now_ms);
            match tick_result {
                StateTickResult::Continue => {
                    if kind == BattleStateKind::Attack && top.is_attack_ready(now_ms) {
                        top.schedule_next_swing(now_ms, swing_delay_ms);
                        let target_view = arena.get(target_actor_id);
                        let distance_sq = target_view.as_ref().map(|t| {
                            let dx = t.position.x - owner_pos.x;
                            let dz = t.position.z - owner_pos.z;
                            dx * dx + dz * dz
                        });
                        let in_range =
                            distance_sq.is_some_and(|d2| d2 <= attack_range * attack_range);
                        if target_actor_id != 0 && auto_attack_enabled && in_range {
                            outbox.push(BattleEvent::ResolveAutoAttack {
                                attacker_actor_id: owner_id,
                                defender_actor_id: target_actor_id,
                            });
                        } else {
                            // Ready-but-skipped swings fire at most once per
                            // delay window — cheap to log, and the ONLY
                            // signal when an engaged actor silently never
                            // attacks (the SEQ_005 player TP mystery was
                            // exactly this branch, invisible in every log).
                            tracing::debug!(
                                owner = format!("0x{owner_id:08X}"),
                                target = format!("0x{target_actor_id:08X}"),
                                auto_attack_enabled,
                                target_in_arena = target_view.is_some(),
                                distance = distance_sq.map(f32::sqrt),
                                attack_range,
                                "battle: swing ready but skipped",
                            );
                        }
                    }
                    break;
                }
                StateTickResult::Complete => {
                    if matches!(
                        kind,
                        BattleStateKind::Magic
                            | BattleStateKind::Ability
                            | BattleStateKind::WeaponSkill
                    ) && target_actor_id != 0
                        && let Some(cmd) = top.command().cloned()
                    {
                        // Chant-glow teardown rides ahead of the damage
                        // fan-out (pmeteor `MagicState.Cleanup` order).
                        // (#28 S2.4.)
                        if kind == BattleStateKind::Magic {
                            outbox.push(BattleEvent::CastComplete {
                                owner_actor_id: owner_id,
                                command_id: cmd.id,
                            });
                        }
                        outbox.push(BattleEvent::ResolveAction {
                            attacker_actor_id: owner_id,
                            defender_actor_id: target_actor_id,
                            command: cmd,
                        });
                    }
                    self.states.pop();
                }
                StateTickResult::Interrupted => {
                    if kind == BattleStateKind::Magic {
                        outbox.push(BattleEvent::CastInterrupted {
                            owner_actor_id: owner_id,
                            command_id: top.command().map(|c| c.id).unwrap_or(0),
                        });
                    }
                    self.states.pop();
                }
            }
        }

        // Drain any due action-queue entries.
        let due = self.action_queue.drain_due(now_ms);
        for a in due {
            outbox.push(BattleEvent::LuaCall {
                owner_actor_id: self.owner_actor_id,
                function_name: "onActionQueueFire",
                command_id: a.tag as u16,
                target_actor_id: None,
            });
        }
    }

    fn apply_decision(
        &mut self,
        decision: ControllerDecision,
        now_ms: u64,
        attack_delay_ms: u32,
        outbox: &mut BattleOutbox,
    ) {
        let owner_id = self.owner_actor_id;
        match decision {
            ControllerDecision::Idle => {}
            ControllerDecision::Engage { target_actor_id } => {
                // Push an Attack state so the swing clock starts ticking and the
                // controller stays in its combat branch (C#: BattleNpc engages by
                // entering the active battle state, not just signalling the wire).
                self.internal_engage(target_actor_id, now_ms, attack_delay_ms);
                outbox.push(BattleEvent::Engage {
                    owner_actor_id: owner_id,
                    target_actor_id,
                });
            }
            ControllerDecision::Disengage => {
                outbox.push(BattleEvent::Disengage {
                    owner_actor_id: owner_id,
                });
            }
            ControllerDecision::MoveTo { position } => {
                // Handed to the ticker (#28 S2.2) — it steps the actor
                // `speed * 0.1` per 100 ms tick toward the stop-ring
                // destination, updates the spatial grid, and emits the
                // throttled 0x00CF fan-out.
                self.pending_movement = Some(MovementStep::MoveTo {
                    destination: position,
                });
            }
            ControllerDecision::FaceTarget { position } => {
                self.pending_movement = Some(MovementStep::Face { position });
            }
            ControllerDecision::Roam => {}
            ControllerDecision::Cast { target_actor_id } => {
                // #28 S2.4 — Papalymo caster loop. The spell is
                // pre-resolved onto the controller at spawn; a Magic
                // state already on top means a cast is in flight, so
                // skip (the controller's `next_cast_ms` gate is only
                // re-armed on success).
                if self.is_current(BattleStateKind::Magic) {
                    return;
                }
                let Some(cmd) = self
                    .controller
                    .as_ref()
                    .and_then(|c| c.battle.spell.clone())
                else {
                    return;
                };
                let (cmd_id, cast_ms, recast_ms, cast_type) =
                    (cmd.id, cmd.cast_time_ms, cmd.recast_time_ms, cmd.cast_type);
                if self.internal_cast(target_actor_id, cmd, now_ms) {
                    if let Some(ctrl) = self.controller.as_mut() {
                        ctrl.battle.next_cast_ms = now_ms + cast_ms as u64 + recast_ms as u64;
                    }
                    outbox.push(BattleEvent::CastStart {
                        owner_actor_id: owner_id,
                        target_actor_id,
                        command_id: cmd_id,
                        cast_time_ms: cast_ms,
                        cast_type,
                    });
                }
            }
            ControllerDecision::ChangeTarget { target_actor_id } => {
                // Write the retarget into the live state stack. Without
                // this, `do_combat_tick`'s `most_hated != current_target`
                // check (which sits AHEAD of the caster Cast gate)
                // re-fired every tick forever — live SEQ_005 run: Papalymo
                // emitted 569 ChangeTargets in one fight, spammed ~10
                // identical 0x0195s/s, and never reached his Cast gate
                // after the first hate divergence. (Garlemald-Server #28.)
                if let Some(new_target) = target_actor_id {
                    for s in self
                        .states
                        .iter_mut()
                        .filter(|s| s.kind == BattleStateKind::Attack)
                    {
                        s.target_actor_id = new_target;
                    }
                }
                outbox.push(BattleEvent::TargetChange {
                    owner_actor_id: owner_id,
                    new_target_actor_id: target_actor_id,
                });
            }
            ControllerDecision::LuaCombatTick { target_actor_id } => {
                outbox.push(BattleEvent::LuaCall {
                    owner_actor_id: owner_id,
                    function_name: "onCombatTick",
                    command_id: 0,
                    target_actor_id,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::controller::PlayerController;
    use crate::battle::target_find::ActorView;
    use common::Vector3;
    use std::collections::HashMap;

    fn view() -> ActorView {
        ActorView {
            actor_id: 1,
            position: Vector3::ZERO,
            rotation: 0.0,
            is_alive: true,
            is_static: false,
            allegiance: 1,
            party_id: 0,
            zone_id: 1,
            is_updates_locked: false,
            is_player: true,
            is_battle_npc: false,
        }
    }

    fn owner_view() -> ControllerOwnerView {
        ControllerOwnerView {
            actor: view(),
            is_engaged: false,
            is_spawned: true,
            is_following_path: false,
            at_path_end: true,
            most_hated_actor_id: None,
            current_target_actor_id: None,
            has_prevent_movement: false,
            max_hp: 1000,
            current_hp: 1000,
            target_hpp: None,
            target_has_stealth: false,
            is_close_to_spawn: true,
            target_is_locked: false,
            attack_delay_ms: 2500,
            attack_range: 3.0,
        }
    }

    #[test]
    fn change_state_respects_stack_depth() {
        let mut ai = AIContainer::new(1, Some(PlayerController::new_for(1)), None);
        // Fill stack with 10 interruptible magic states (each one can_change_state=false).
        // But first push a single attack state (the only one where can_change_state==true).
        assert!(ai.internal_engage(2, 0, 2000));

        // Now we can push one magic state on top.
        let mut cmd = BattleCommand::new(100, "stone");
        cmd.cast_time_ms = 3000;
        assert!(ai.internal_cast(2, cmd.clone(), 0));
        // On top of magic we cannot push more — it returns false.
        assert!(!ai.internal_cast(2, cmd, 0));
        assert_eq!(ai.state_stack().len(), 2);
    }

    #[test]
    fn internal_disengage_clears_stack() {
        let mut ai = AIContainer::new(1, None, None);
        ai.internal_engage(2, 0, 2000);
        assert_eq!(ai.state_stack().len(), 1);
        let mut ob = BattleOutbox::new();
        ai.internal_disengage(&mut ob);
        assert!(ai.state_stack().is_empty());
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, BattleEvent::Disengage { .. }))
        );
    }

    #[test]
    fn update_pops_completed_casting_states() {
        let mut ai = AIContainer::new(1, None, None);
        let mut cmd = BattleCommand::new(100, "stone");
        cmd.cast_time_ms = 1000;
        ai.internal_cast(2, cmd, 0);
        let arena: HashMap<u32, ActorView> = HashMap::new();
        let mut ob = BattleOutbox::new();
        ai.update(1500, owner_view(), &arena, &mut ob);
        assert!(ai.state_stack().is_empty());
    }

    #[test]
    fn die_force_pushes_death_state() {
        let mut ai = AIContainer::new(1, None, None);
        let mut ob = BattleOutbox::new();
        ai.internal_die(0, 5000, &mut ob);
        assert_eq!(ai.current_state().unwrap().kind, BattleStateKind::Death);
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, BattleEvent::Die { .. }))
        );
    }

    #[test]
    fn action_queue_fires_in_order() {
        let mut q = ActionQueue::new();
        q.push(Action {
            fire_at_ms: 3000,
            check_state: false,
            tag: 2,
        });
        q.push(Action {
            fire_at_ms: 1000,
            check_state: false,
            tag: 1,
        });
        q.push(Action {
            fire_at_ms: 5000,
            check_state: false,
            tag: 3,
        });
        let due = q.drain_due(3000);
        assert_eq!(due.iter().map(|a| a.tag).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(q.entries.len(), 1);
    }

    #[test]
    fn update_triggers_controller_decisions() {
        let controller = PlayerController::new_for(1);
        let mut ai = AIContainer::new(1, Some(controller), None);
        let arena: HashMap<u32, ActorView> = [(1, view())].into_iter().collect();
        let mut ob = BattleOutbox::new();
        ai.update(1000, owner_view(), &arena, &mut ob);
        // PlayerController is idle — no outbox events.
        assert!(ob.events.is_empty());
        // Latest update time is recorded.
        assert_eq!(ai.latest_update_ms, 1000);
    }

    #[test]
    fn suppress_hate_unused() {
        // Quiet the unused-import lint warning in the test module.
        let _ = HateContainer::new(1);
    }

    fn arena_view(id: u32, x: f32, allegiance: u32) -> ActorView {
        ActorView {
            actor_id: id,
            position: Vector3::new(x, 0.0, 0.0),
            rotation: 0.0,
            is_alive: true,
            is_static: false,
            allegiance,
            party_id: 0,
            zone_id: 1,
            is_updates_locked: false,
            is_player: allegiance == 1,
            is_battle_npc: allegiance != 1,
        }
    }

    fn engaged_owner_view(target: u32) -> ControllerOwnerView {
        let mut v = owner_view();
        v.actor = arena_view(1, 0.0, 2);
        v.is_engaged = true;
        v.most_hated_actor_id = Some(target);
        v.current_target_actor_id = Some(target);
        v
    }

    /// #28 S2.3 — swings require the target inside attack range; the
    /// swing clock re-arms regardless (arriving mid-window waits out the
    /// window, pmeteor `AttackState.CanAttack` parity).
    #[test]
    fn s2_3_swing_gated_on_attack_range_and_rearms() {
        use crate::battle::controller::BattleNpcController;
        let mut ai = AIContainer::new(1, Some(BattleNpcController::new_for(1)), None);
        ai.internal_engage(2, 0, 2000);

        // Out of range (10 y, range 3.0): ready swing is swallowed.
        let arena: HashMap<u32, ActorView> =
            [(1, arena_view(1, 0.0, 2)), (2, arena_view(2, 10.0, 1))]
                .into_iter()
                .collect();
        let mut ob = BattleOutbox::new();
        ai.update(2_000, engaged_owner_view(2), &arena, &mut ob);
        assert!(
            !ob.events
                .iter()
                .any(|e| matches!(e, BattleEvent::ResolveAutoAttack { .. })),
            "out-of-range swing must not resolve",
        );
        // Clock re-armed: an immediately-following in-range tick stays
        // quiet until the window elapses again.
        let arena_close: HashMap<u32, ActorView> =
            [(1, arena_view(1, 0.0, 2)), (2, arena_view(2, 2.0, 1))]
                .into_iter()
                .collect();
        let mut ob2 = BattleOutbox::new();
        ai.update(2_100, engaged_owner_view(2), &arena_close, &mut ob2);
        assert!(
            !ob2.events
                .iter()
                .any(|e| matches!(e, BattleEvent::ResolveAutoAttack { .. })),
            "swing clock must have re-armed on the gated swing",
        );
        // Next window, in range → swing.
        let mut ob3 = BattleOutbox::new();
        ai.update(4_600, engaged_owner_view(2), &arena_close, &mut ob3);
        assert!(
            ob3.events
                .iter()
                .any(|e| matches!(e, BattleEvent::ResolveAutoAttack { .. })),
            "in-range ready swing must resolve",
        );
    }

    /// #28 S2.3 — `auto_attack_enabled = false` (the Papalymo shape)
    /// never swings, even point-blank.
    #[test]
    fn s2_3_auto_attack_disabled_never_swings() {
        use crate::battle::controller::AllyController;
        let mut ctrl = AllyController::new_for(1);
        ctrl.auto_attack_enabled = false;
        let mut ai = AIContainer::new(1, Some(ctrl), None);
        ai.internal_engage(2, 0, 2000);
        let arena: HashMap<u32, ActorView> =
            [(1, arena_view(1, 0.0, 3)), (2, arena_view(2, 1.0, 2))]
                .into_iter()
                .collect();
        let mut ob = BattleOutbox::new();
        for now in [2_000u64, 4_500, 7_000] {
            ai.update(now, engaged_owner_view(2), &arena, &mut ob);
        }
        assert!(
            !ob.events
                .iter()
                .any(|e| matches!(e, BattleEvent::ResolveAutoAttack { .. })),
            "auto-attack-disabled caster must never swing",
        );
    }

    /// #28 S2.3 — players carry no controller: auto-attack defaults on
    /// (the `is_none_or(true)` read) and the range gate still applies.
    #[test]
    fn s2_3_player_without_controller_swings_in_range() {
        let mut ai = AIContainer::new(1, None, None);
        ai.internal_engage(2, 0, 2000);
        let arena: HashMap<u32, ActorView> =
            [(1, arena_view(1, 0.0, 1)), (2, arena_view(2, 2.0, 2))]
                .into_iter()
                .collect();
        let mut ob = BattleOutbox::new();
        ai.update(2_000, engaged_owner_view(2), &arena, &mut ob);
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, BattleEvent::ResolveAutoAttack { .. })),
        );
    }

    /// #28 S2.4 — the Cast decision arm: pushes a Magic state via
    /// `internal_cast`, emits `CastStart`, re-arms `next_cast_ms` to
    /// `now + cast + recast`, and completion emits `CastComplete` ahead
    /// of `ResolveAction`.
    #[test]
    fn s2_4_cast_arm_pushes_magic_and_emits_lifecycle_events() {
        use crate::battle::controller::AllyController;
        let mut ctrl = AllyController::new_for(1);
        ctrl.auto_attack_enabled = false;
        ctrl.battle.is_caster = true;
        let mut spell = BattleCommand::new(27313, "thunder");
        spell.range = 20.0;
        spell.cast_time_ms = 2000;
        spell.recast_time_ms = 6000;
        spell.cast_type = 2;
        ctrl.battle.spell = Some(spell);
        let mut ai = AIContainer::new(1, Some(ctrl), None);
        ai.internal_engage(2, 100, 2000);

        let arena: HashMap<u32, ActorView> =
            [(1, arena_view(1, 0.0, 3)), (2, arena_view(2, 10.0, 2))]
                .into_iter()
                .collect();
        let mut ob = BattleOutbox::new();
        ai.update(100, engaged_owner_view(2), &arena, &mut ob);
        assert!(
            ob.events.iter().any(|e| matches!(
                e,
                BattleEvent::CastStart {
                    target_actor_id: 2,
                    command_id: 27313,
                    cast_type: 2,
                    ..
                }
            )),
            "CastStart must carry target + command + cast_type; got {:?}",
            ob.events,
        );
        assert!(ai.is_current(BattleStateKind::Magic));
        assert_eq!(
            ai.controller.as_ref().unwrap().battle.next_cast_ms,
            100 + 2000 + 6000,
            "recast clock = now + cast + recast",
        );

        // Cast completes 2000 ms later: CastComplete rides ahead of the
        // damage ResolveAction; the Magic state pops.
        let mut ob2 = BattleOutbox::new();
        ai.update(2_200, engaged_owner_view(2), &arena, &mut ob2);
        let kinds: Vec<&'static str> = ob2
            .events
            .iter()
            .filter_map(|e| match e {
                BattleEvent::CastComplete { .. } => Some("complete"),
                BattleEvent::ResolveAction { .. } => Some("resolve"),
                BattleEvent::CastStart { .. } => Some("start"),
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            vec!["complete", "resolve"],
            "chant teardown ahead of damage, no re-cast inside the recast window",
        );
        assert!(!ai.is_current(BattleStateKind::Magic));
    }

    /// #28 S2.2 — MoveTo/FaceTarget decisions land on `pending_movement`
    /// for the ticker; intents are one-tick state.
    #[test]
    fn s2_2_pending_movement_set_and_cleared_per_tick() {
        use crate::battle::controller::BattleNpcController;
        let mut ai = AIContainer::new(1, Some(BattleNpcController::new_for(1)), None);
        ai.internal_engage(2, 0, 2000);
        let arena: HashMap<u32, ActorView> =
            [(1, arena_view(1, 0.0, 2)), (2, arena_view(2, 9.0, 1))]
                .into_iter()
                .collect();
        let mut ob = BattleOutbox::new();
        ai.update(100, engaged_owner_view(2), &arena, &mut ob);
        assert!(
            matches!(ai.pending_movement, Some(MovementStep::MoveTo { .. })),
            "pursuit decision must surface as a pending MoveTo",
        );
        // A tick whose decision is no longer movement clears the intent.
        let arena_close: HashMap<u32, ActorView> =
            [(1, arena_view(1, 0.0, 2)), (2, arena_view(2, 2.0, 1))]
                .into_iter()
                .collect();
        ai.update(200, engaged_owner_view(2), &arena_close, &mut ob);
        assert!(
            matches!(ai.pending_movement, Some(MovementStep::Face { .. })),
            "in-range decision degrades to a Face intent",
        );
    }
}
