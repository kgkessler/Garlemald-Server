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
--   entry text → EndEvent → escort go-latch; the retail
--   TtrBlkNml001 tutorial block is a follow-up, see round 7b note) →
--   parks on "escortComplete" →
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
	-- (a) Post-warp Now-Loading DISMISSAL (round 7c, decomp-final). The
	--     overlay after a kick-at-entry is a DESIGNED staging veil: the
	--     kicked noticeEvent arms MyPlayer+0x128/+0x12c and the ONLY
	--     dismissal is a client Lua script calling MyPlayer slot 66
	--     _fadeInNowLoadingForNoticeEventJustInArea (zero C++ callers —
	--     the server's EndEvent does NOT clear it; wire-proven 20:08:
	--     604_3 answered, EndEvent delivered, overlay stayed forever).
	--     processEvent604_3 was fade-LAYER-only (the client faded in
	--     BEHIND the overlay). questBaseRewardSeting is the QuestBase-
	--     class delegate whose client body is exactly the dismissal:
	--     _fadeInNowLoadingForNoticeEventJustInArea() +
	--     startFadeInCutSceneDefault + _wait(0.5)
	--     (QuestBaseClass_common.lua:1018-1033). The tutorials dismiss
	--     via the SAME slot-66 call inside startCutScene's prologue
	--     (their post-kick delegate starts an HQ cutscene) — not via the
	--     fade, as the round-7 comment wrongly claimed.
	callClientFunction(player, "delegateEvent", player, man0l1Quest, "questBaseRewardSeting");

	-- (b) NO processTtrBlkNml001 (round 7b). Wire-proven 2026-07-03
	--     19:58: the client ANSWERED the 604_3 fade above, then received
	--     the TtrBlkNml001 delegate and Wine hard-died without answering
	--     (22s silence → disconnect). The "unresolvable refs degrade
	--     gracefully" assumption was wrong: the working man0l0 tutorial
	--     precedes its Ttr block with a startTutorialMode 0x0133 data
	--     packet (QuestDirectorMan0l001 three-part chain) — without
	--     tutorial mode armed, the block's engine calls on the hardcoded
	--     4000604 Sisipu anchor (aim camera / lookAt / chara scheduler /
	--     sayFreeDisplayName) crash the client process. The block is
	--     cosmetic (camera pan + named say + mask; the content script's
	--     bark loop delivers say 337 regardless) — retail-parity
	--     follow-up: port the FULL Man0l001 chain (startTutorialMode →
	--     TtrBlkNml001 → the endTutorialMode exit) once the leg is
	--     playable end-to-end. (Garlemald-Server #46, round 7b.)

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
