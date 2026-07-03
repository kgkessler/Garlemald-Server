require ("global")
require ("quests/man/man0l1")

-- Man0l1 SEQ_050 Zephyr Gate escort director (Garlemald-Server #46) —
-- rewritten from the dead upstream port (its onCreateContentArea was
-- never called by any engine, pmeteor's included; spawning now lives in
-- content/SimpleContentMan0l101.lua's onCreate like every tutorial
-- fight). This director owns only the completion beat, following the
-- QuestDirectorMan0l001 coroutine pattern:
--
--   client noticeEvent kick (startMan0l1Content's KickEvent) →
--   onEventStarted closes the kick and parks on "escortComplete" →
--   the content script's onUpdate fires the signal once Sisipu has led
--   the player down the zone-128 road to the lighthouse approach
--   (waypoint arrival — NOT a kill count; the ambush waves are
--   incidental) → arrival cutscene (processEvent605, the
--   wiki-documented "arrive at Oschon's Torch, enters an echo" beat)
--   → SEQ_055 journal update + "no longer bound by duty" →
--   ContentFinished teardown → warp into the lighthouse echo private
--   area (pmeteor's SEQ_055 camp shape).
--
--   The content script ALSO fires "escortComplete" on its FAIL flow
--   (Sisipu death / 30-min expiry / leave-duty onAbort) purely to
--   DRAIN this park (a stale park would double-fire the arrival on a
--   retry — drain_signal wakes every parked coroutine). In that case
--   the player was already rolled back to SEQ_048 and warped out, so
--   the kickEventContinue below emits a kick for a wiped director
--   actor — the client drops it (wire-proven, session 53943), this
--   coroutine parks on _WAIT_EVENT and is later displaced, and the
--   StartSequence(SEQ_055) tail never runs. (Garlemald-Server #46.)

function init()
	return "/Director/Quest/QuestDirectorMan0l101";
end

function onEventStarted(player, actor, triggerName)
	man0l1Quest = player:GetQuest("Man0l1");

	-- This noticeEvent kick fires AFTER the duty warp into the zone-128
	-- content instance (the Rust side defers the 0x012F TX to the
	-- post-warp ack). The cutscene's after-warp veil was already
	-- neutralised PRE-warp at the gate (man0l1.lua startMan0l1Content:
	-- processEvent604 → processEvent604_3 → EndEvent →
	-- DoZoneChangeContent), so the warp itself is a clean man0g0-style
	-- warp and the client lands interactive. Just close the kick and
	-- park on completion. NO tutorial-mode handlers (orderTutorialMode
	-- would silently re-gate the menu). (Garlemald-Server #46.)
	player:EndEvent();

	waitForSignal("escortComplete");

	-- Render-settle beat (Man0l001 pattern): without it the arrival
	-- cutscene lands in the same drain as the last death packets.
	wait(2);

	-- Reopen the event context BEFORE delegating — a bare delegate
	-- ships with owner=0 and the client echo-drops it (Man0g001/
	-- Man0l001 pattern).
	kickEventContinue(player, actor, "noticeEvent", "noticeEvent");
	-- Arrival echo at Oschon's Torch. processEvent605 ends by arming an
	-- after-warp veil (startFadeInCutSceneAfterWarp) — it MUST be
	-- followed by the camp warp below, whose real private-area load
	-- resolves the veil.
	callClientFunction(player, "delegateEvent", player, man0l1Quest, "processEvent605");
	-- Camp handoff (retail order: journal update → 34108 → "no longer
	-- bound by duty"). StartSequence(SEQ_055) IS the journal update;
	-- 34108 "You have entered an instance." rides the Rust private-area
	-- warp path (the deferred-bundle notify_private_area arm), so the
	-- script never sends it. The unbind line goes out BEFORE the
	-- EndEvent + warp — a 0x0157 game message queued after the warp
	-- ships into the client's Now-Loading gap (the man0l0 Hob-crash
	-- shape; see man0l1.lua onStart) — its id is an UNVERIFIED
	-- candidate pending an in-client probe (man0l1.lua TEXT_*).
	man0l1Quest:StartSequence(SEQ_055);
	player:SendGameMessage(GetWorldMaster(), TEXT_UNBOUND_FROM_DUTY, 0x20);
	-- EndEvent BEFORE the warp — an EndEvent landing mid-reload loses
	-- the client's _onPostEvent teardown → desktopWidgetMode-16 mask
	-- ("tutorial mode" menu lock; wire-proven). (Garlemald-Server #46.)
	player:EndEvent();
	player:GetZone():ContentFinished();
	-- pmeteor's SEQ_055 lighthouse camp shape: DoZoneChange(128,
	-- 'PrivateAreaMasterPast', 2, 15, 137.44, 60.33, 1322.0, -1.60).
	GetWorldManager():DoZoneChange(player, 128, "PrivateAreaMasterPast", 2, 15, 137.44, 60.33, 1322.0, -1.60);
end

function main()
end
