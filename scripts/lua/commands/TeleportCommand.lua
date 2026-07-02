--[[

TeleportCommand Script

Functions:

eventRegion(numAnima)
eventAetheryte(region, animaCost1, animaCost2, animaCost3, animaCost4, animaCost5, animaCost6)
eventConfirm(isReturn, isInBattle, cityReturnNum, 138821, forceAskReturnOnly)

--]]

require ("global")
require ("aetheryte")
require ("utils")

teleportMenuToAetheryte = {
	[1] = {
		[1] = 1280001,
		[2] = 1280002,
		[3] = 1280003,
		[4] = 1280004,
		[5] = 1280005,
		[6] = 1280006
	},
	[2] = {
		[1] = 1280092,
		[2] = 1280093,
		[3] = 1280094,
		[4] = 1280095,
		[5] = 1280096
	},
	[3] = {
		[1] = 1280061,
		[2] = 1280062,
		[3] = 1280063,
		[4] = 1280064,
		[5] = 1280065,
		[6] = 1280066
	},
	[4] = {
		[1] = 1280031,
		[2] = 1280032,
		[3] = 1280033,
		[4] = 1280034,
		[5] = 1280035,
		[6] = 1280036
	},
	[5] = {
		[1] = 1280121,
		[2] = 1280122		
	}
}

-- Belt-and-braces veil-clearer for teleport warps (Garlemald-Server #46,
-- round 2). Same-map / same-resident-geometry destinations never complete
-- the 0x00E2(0x02) reload recipe (the client schedules a reload with no
-- completion path — RX 0x0007 never comes — and the Now-Loading veil hangs
-- forever). The Baderon recipe (man0l1.lua seq000_onTalk, wire-proven
-- 04:23:21) covers this: stage an AfterQuestWarpDirector +
-- SetLoginDirector + KickEvent BEFORE the warp; the login arm defers the
-- kick until after the zone-in bundle (and spawns the login director into
-- the destination bundle regardless of which zone created it — wire-proven
-- by the 04:27:13 bundle carrying a zone-133-created director into 230),
-- and the client's noticeEvent -> onNotice -> EndEvent round-trip runs
-- MyPlayer slot 66 _fadeInNowLoadingForNoticeEventJustInArea to clear the
-- veil. Harmless on cross-map warps and on the 0x16 seamless-bypass
-- recipe too: the kick just EndEvents.
--
-- GATED two ways (both load-bearing):
--  * On the two quests AfterQuestWarpDirector.lua actually routes
--    (110002 man0l1 / 110006 man0g1): its onEventStarted only hands the
--    notice to those quests' onNotice, so for any other player NOTHING
--    would EndEvent the kicked noticeEvent and the open event would
--    event-lock the client (movement + menus dead — see man0l1.lua
--    onNotice). Quest-less teleports are covered by the Rust-side 0x16
--    same-seamless-family bypass instead.
--  * On the player being ALIVE: SetLoginDirector is the tell that
--    reroutes the WHOLE resumed command burst through the login applier
--    (processor.rs is_login_scoped_burst), which has no SetHP /
--    ChangeState arms — the dead-Return heal bandaid below would be
--    silently dropped and the player would warp home still dead. Dead
--    returns keep the runtime drain (and the 0x16 bypass) instead.
--    (Same rerouting also costs the cosmetic 0x4000FFB departure
--    animation on staged warps — accepted trade for an un-stuck veil.)
function stageAfterQuestWarpDirector(player, destZoneId)
	if ((player:HasQuest(110002) or player:HasQuest(110006)) and player:GetHP() > 0) then
		local director = GetWorldManager():GetArea(destZoneId):CreateDirector("AfterQuestWarpDirector", false);
		player:AddDirector(director);
		director:StartDirector(true);
		player:SetLoginDirector(director);
		player:KickEvent(director, "noticeEvent", true);
	end
end

function onEventStarted(player, actor, triggerName, isTeleport)

	local worldMaster = GetWorldMaster();

	if (isTeleport == 0) then		
		while (true) do
			regionChoice = callClientFunction(player, "delegateCommand", actor, "eventRegion", 100);
			
			if (regionChoice == nil) then break	end
		
			while (true) do
				aetheryteChoice = callClientFunction(player, "delegateCommand", actor, "eventAetheryte", regionChoice, 2, 2, 2, 4, 4, 4);
				
				if (aetheryteChoice == nil) then break end
				
				player:PlayAnimation(0x4000FFA);
				player:SendGameMessage(worldMaster, 34101, 0x20, 2, teleportMenuToAetheryte[regionChoice][aetheryteChoice], 100, 100);	
				confirmChoice = callClientFunction(player, "delegateCommand", actor, "eventConfirm", false, false, 1, 138824, false);				
				if (confirmChoice == 1) then
					player:PlayAnimation(0x4000FFB);
					player:SendGameMessage(worldMaster, 34105, 0x20);
					--Do teleport
					destination = aetheryteTeleportPositions[teleportMenuToAetheryte[regionChoice][aetheryteChoice]];
					if (destination ~= nil) then
						randoPos = getRandomPointInBand(destination[2], destination[4], 3, 5);
						rotation = getAngleFacing(randoPos.x, randoPos.y, destination[2], destination[4]);
						-- Close the teleport menu event, THEN warp — 0x0131 ahead
						-- of the 0x00E2 reload latch is retail's invariant ordering
						-- on every captured transition (see man0l1.lua onStart).
						-- EndEvent after the warp shipped the event teardown into
						-- the client's Now-Loading gap and left the veil stuck.
						-- (Garlemald-Server #46.)
						stageAfterQuestWarpDirector(player, destination[1]);
						player:EndEvent();
						GetWorldManager():DoZoneChange(player, destination[1], nil, 0, 2, randoPos.x, destination[3], randoPos.y, rotation);
						return;
					end
				end
				player:EndEvent();
				return;
			end
			
		end
	else
		player:PlayAnimation(0x4000FFA);
		local choice, isInn = callClientFunction(player, "delegateCommand", actor, "eventConfirm", true, false, player:GetHomePointInn(), player:GetHomePoint(), false);		
		if (choice == 1) then
			player:PlayAnimation(0x4000FFB);
			player:SendGameMessage(worldMaster, 34104, 0x20);

			--bandaid fix for returning while dead, missing things like weakness and the heal number
			if (player:GetHP() == 0) then
				-- `:GetMaxHP()` — the inherited `.GetMaxHP()` dot-call passed
				-- no self to the mlua method and errored, killing the
				-- coroutine before EndEvent/warp ever ran on a dead Return.
				-- (Garlemald-Server #46, round 2 drive-by.)
				player:SetHP(player:GetMaxHP());
				player:ChangeState(0);
				player:PlayAnimation(0x01000066);
			end

			-- Same 0x0131-before-0x00E2 ordering as the teleport branch
			-- above: each warping path closes the event first and returns,
			-- so the shared EndEvent at the bottom only serves the
			-- non-warp exits (declined confirm, unset inn slot, unknown
			-- homepoint). (Garlemald-Server #46.)
			if (isInn) then
				--Return to Inn
				if (player:GetHomePointInn() == 1) then
					stageAfterQuestWarpDirector(player, 244);
					player:EndEvent();
					GetWorldManager():DoZoneChange(player, 244, nil, 0, 15, -160.048, 0, -165.737, 0);
					return;
				elseif (player:GetHomePointInn() == 2) then
					stageAfterQuestWarpDirector(player, 244);
					player:EndEvent();
					GetWorldManager():DoZoneChange(player, 244, nil, 0, 15, 160.048, 0, 154.263, 0);
					return;
				elseif (player:GetHomePointInn() == 3) then
					stageAfterQuestWarpDirector(player, 244);
					player:EndEvent();
					GetWorldManager():DoZoneChange(player, 244, nil, 0, 15, 0.048, 0, -5.737, 0);
					return;
				end
			elseif (choice == 1 and isInn == nil) then
				--Return to Homepoint
				destination = aetheryteTeleportPositions[player:GetHomePoint()];
				if (destination ~= nil) then
					randoPos = getRandomPointInBand(destination[2], destination[4], 3, 5);
					rotation = getAngleFacing(randoPos.x, randoPos.y, destination[2], destination[4]);
					stageAfterQuestWarpDirector(player, destination[1]);
					player:EndEvent();
					GetWorldManager():DoZoneChange(player, destination[1], nil, 0, 2, randoPos.x, destination[3], randoPos.y, rotation);
					return;
				end
			end
		end
	end	

	player:EndEvent();
end