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
--   onEventStarted runs the post-warp chain (fade-in 604_3 →
--   processTtrBlkNml001 tutorial block → entry text → EndEvent →
--   escort go-latch) and parks on "escortComplete" →
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

	-- ROUND 7 — the tutorial-proven post-warp chain. The kick is emitted
	-- WITH the pre-warp content trio (apply_do_zone_change_content 3a);
	-- the client queues the 0x012F across the reload and answers this
	-- noticeEvent mid-load, ~1s before its RX 0x0007 (wire-proven
	-- 2026-07-03 14:24, identical to the man0l0 tutorial at 05:50). On a
	-- same-map wipe+0x10 reload NOTHING auto-fades the client in — both
	-- working tutorials drop the Now-Loading veil from the director's
	-- FIRST post-kick delegate (processTtrBtl001 ends in
	-- startFadeInCutSceneDefault, answered post-load). Answering with a
	-- bare EndEvent instead (the previous shape) leaves the client
	-- veiled forever — the 14:24 hang. (Garlemald-Server #46, round 7.)
	--
	-- (a) Post-warp fade-in: processEvent604_3 = startFadeInCutSceneDefault
	--     (_waitForMapLoaded → _fadeIn(1) → _waitForFading) — the reload
	--     has finished by the time the client executes it, so this is the
	--     veil-dropper, in exactly the slot the tutorials use.
	callClientFunction(player, "delegateEvent", player, man0l1Quest, "processEvent604_3");

	-- (b) The retail escort-start tutorial block (decoded client
	--     Man0l1.lua:2431-2497): orderTutorialMode-if-needed, camera
	--     aim/lookAt + gesture schedules on the 4000604 Sisipu anchor,
	--     sayFreeDisplayName(4000604, quest, 337) ("Oschon's Torch is due
	--     south..."), setTutorialMask(false x5, 3), cancels. Unresolvable
	--     client refs inside the block degrade gracefully (the man0l0
	--     tutorial runs processTtrBtl001 past its own 1900006 anchor with
	--     no server-side mapping) — worst case the camera pan/named say
	--     don't render; the content script's bark loop re-delivers 337.
	callClientFunction(player, "delegateEvent", player, man0l1Quest, "processTtrBlkNml001");

	-- (c) Retail entry text, now that the client is faded in and can see
	--     the log (retail order after the journal update + 34108, which
	--     both already rode the warp). Ids are UNVERIFIED candidates
	--     (man0l1.lua TEXT_*), pending an in-client probe.
	player:SendGameMessage(GetWorldMaster(), TEXT_PROTECT_SISIPU, 0x20);
	player:SendGameMessage(GetWorldMaster(), TEXT_BOUND_BY_DUTY, 0x20);
	player:SendGameMessage(GetWorldMaster(), TEXT_TIME_REMAINING, 0x20, 30);

	-- (d) Close the kick — post-load, like the tutorials (their
	--     noticeEvent EndEvent goes out well after the reload).
	player:EndEvent();

	-- (e) Release the duty: the content script's onUpdate holds the
	--     30-minute clock, Sisipu's waypoint walk and the bark loop on
	--     this latch so the escort doesn't run under the veil.
	man0l1EscortGo = true;

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
