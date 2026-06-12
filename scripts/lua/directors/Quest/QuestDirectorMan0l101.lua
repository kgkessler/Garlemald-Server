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
--   the content script's onUpdate fires the signal when Sisipu reaches
--   the final waypoint with the road cleared → arrival cutscene
--   (processEvent605, the wiki-documented "arrive at Oschon's Torch,
--   enters an echo" beat) → SEQ_055 → ContentFinished teardown → warp
--   into the lighthouse echo private area (the same destination the
--   previous skip path warped to).

function init()
	return "/Director/Quest/QuestDirectorMan0l101";
end

function onEventStarted(player, actor, triggerName)
	man0l1Quest = player:GetQuest("Man0l1");

	-- Close the notice kick's event context; the escort walk runs free
	-- (no client event open) until the arrival beat below.
	player:EndEvent();
	waitForSignal("escortComplete");

	-- Render-settle beat (Man0l001 pattern): without it the arrival
	-- cutscene lands in the same drain as the last death packets.
	wait(2);

	-- Reopen the event context BEFORE delegating — a bare delegate
	-- ships with owner=0 and the client echo-drops it (Man0g001/
	-- Man0l001 pattern).
	kickEventContinue(player, actor, "noticeEvent", "noticeEvent");
	-- Arrival echo at Oschon's Torch; expects the zone change right
	-- after, exactly like the previous skip path's 605 → warp pair.
	callClientFunction(player, "delegateEvent", player, man0l1Quest, "processEvent605");
	man0l1Quest:StartSequence(SEQ_055);
	player:EndEvent();
	player:GetZone():ContentFinished();
	GetWorldManager():DoZoneChange(player, 128, "PrivateAreaMasterPast", 2, 15, 137.44, 60.33, 1322.0, -1.60);
end

function main()
end
