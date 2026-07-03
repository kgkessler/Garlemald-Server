require ("global")
require ("modifiers")
-- onUpdate calls allyGlobal.EngageTarget; without this require the
-- global is nil and every onUpdate tick errors (swallowed by the
-- ticker drain). Same shape as SimpleContent30010. (#28 S0.4)
require ("ally")

-- Limsa SEQ_005 deck battle (Man0l0 "Shapeless Melody") — port of the
-- SimpleContent30010 rewrite to the ship fight (Garlemald-Server #25
-- follow-up). The upstream import spawned Y'shtola/Sthalmann/the three
-- aurelias as synthetic contentArea:SpawnActor props: no combat AI, no
-- HP model, no kill gate, lying-down spawn states. BNpc seed rows live
-- in common/sql/seed/054_limsa_seq005_tutorial.sql (spawn ids 8-12).

function onCreate(starterPlayer, contentArea, director)
	-- Reset the engagement latch: the engine caches ONE VM per script
	-- path for the whole server process, so the top-level
	-- `battleStarted = false` below runs only once — a second run of
	-- this tutorial (new character, or re-entry after a disconnect)
	-- would otherwise inherit battleStarted=true and let the allies
	-- attack before the targeting tutorial returns.
	battleStarted = false;
	-- Clear this player's kill-EXP counter entry (see onUpdate) — a
	-- stale entry from an aborted previous run would mis-pay the first
	-- grant. Other players' in-flight entries are left alone.
	tutorialLiveHostiles = tutorialLiveHostiles or {};
	tutorialLiveHostiles[starterPlayer.actorId] = nil;
	sthalmann = GetWorldManager().SpawnBattleNpcById(11, contentArea);
	yshtola = GetWorldManager().SpawnBattleNpcById(12, contentArea);
	mob1 = GetWorldManager().SpawnBattleNpcById(8, contentArea);
	mob2 = GetWorldManager().SpawnBattleNpcById(9, contentArea);
	mob3 = GetWorldManager().SpawnBattleNpcById(10, contentArea);
	-- Active/engaged MainState for the melee ally + the mobs so they
	-- stand up and render hostile (SimpleContent30010 pattern).
	-- Y'shtola (healer/caster) stays at the default passive state like
	-- Papalymo does in the Gridania fight.
	sthalmann:ChangeState(2);
	mob1:ChangeState(2);
	mob2:ChangeState(2);
	mob3:ChangeState(2);
	-- Party-add the allies (Garlemald-Server #46, round 4 — the round-2
	-- removal was WRONG): decomp-proven, the bottom-right ally HP rows
	-- are the PartyParameterWidget reading getPlayerParty() — the
	-- extended-temp 10001 party group EXCLUSIVELY; the widget connector
	-- has no arm for the 30006 content group, which triggers no party UI
	-- at all. The round-1 party-adds were correct and failed only
	-- because the NPC rows shipped an empty name field — the party X08
	-- row encoding now carries the localized display_name_id, and '???'
	-- HP rows are retail-correct for NPC party members. Allies only —
	-- never the mobs or the director.
	starterPlayer.currentParty:AddMember(yshtola.actorId);
	starterPlayer.currentParty:AddMember(sthalmann.actorId);
	-- No-die guarantees for the tutorial (MinimumHpLock floor-1 clamp);
	-- the player's lock is cleared at ContentFinished.
	starterPlayer:SetMod(modifiersGlobal.MinimumHpLock, 1);
	sthalmann:SetMod(modifiersGlobal.MinimumHpLock, 1);
	yshtola:SetMod(modifiersGlobal.MinimumHpLock, 1);

	director:AddMember(starterPlayer);
	director:AddMember(director);
	director:AddMember(yshtola);
	director:AddMember(sthalmann);
	director:AddMember(mob1);
	director:AddMember(mob2);
	director:AddMember(mob3);
end

function onDestroy()

end

-- Everyone fights once the player commits (#28 S2.5). Script-VM
-- global: persists across onUpdate calls. The VM is process-cached
-- per script path (NOT per content area), so onCreate re-arms it for
-- each fresh run.
battleStarted = false;
-- Per-PLAYER live-hostile counts from the previous tick — drives the
-- per-kill EXP grant below. Keyed by player actor id because this VM
-- (and so this table) is shared by EVERY session ticking this script;
-- a plain scalar would interleave concurrent same-city runs and mint
-- phantom EXP. onCreate clears the starting player's entry.
tutorialLiveHostiles = {};

function onUpdate(tick, area)
	if not area then return end
	local players = area:GetPlayers()
	local mobs    = area:GetMonsters()   -- live-only (dead filtered by S0.5)
	local allies  = area:GetAllies()

	local engagedPlayer, firstPlayer = nil, nil
	for player in players do
		if player then
			if not firstPlayer then firstPlayer = player end
			if player:IsEngaged() and player.target then
				engagedPlayer = player
				break
			end
		end
	end

	-- Tutorial kill EXP: retail granted 1000 EXP per aurelia — 3000 for
	-- the trio, enough to clear level 2 (570 SP) on the first kill.
	-- (FFXIVenturer "Shapeless Melody" guide: "You will receive 1000 EXP
	-- from each monster"; 2010-09 open-beta footage OCRs "You gain 1000
	-- experience points." per kill.) `mobs` is live-only, so a drop in
	-- the count since the last tick = that many kills. Granted to the
	-- player regardless of who landed the blow — retail pays the full
	-- amount on ally killing blows too, which the onKillBNpc quest hook's
	-- player-only attacker gate would miss. The `#allies > 0` gate is
	-- the "fight is set up" signal: pre-onCreate ticks (no roster yet)
	-- never touch the counter, so a stale count can't pay out against
	-- an unspawned fight — and the allies are MinimumHpLock-floored, so
	-- the roster stays populated through the final kill's grant.
	if #allies > 0 then
		local owner = engagedPlayer or firstPlayer
		if owner then
			local prev = tutorialLiveHostiles[owner.actorId]
			if prev ~= nil and #mobs < prev then
				-- One grant per kill (not a summed lump) so a
				-- multi-kill tick still produces retail's per-kill
				-- chat lines.
				for _ = 1, prev - #mobs do
					owner:AddExp(1000, owner.charaWork.parameterSave.state_mainSkill[0], 0)
				end
			end
			tutorialLiveHostiles[owner.actorId] = #mobs
		end
	end

	if not battleStarted then
		-- Nothing moves until the player attacks. Latching on the
		-- player's engagement keeps the aurelias alive and targetable
		-- through processTtrBtl002's targeting tutorial (allies killing
		-- them early could soft-lock _waitForTargetTutorial client-side).
		if not engagedPlayer then return end
		battleStarted = true
	end

	-- Allies: spread across live mobs; re-engage as mobs die.
	local mi = 0
	for i = 1, #allies do
		local ally = allies[i]
		if ally and not ally:IsEngaged() and #mobs > 0 then
			local target = mobs[(mi % #mobs) + 1]
			mi = mi + 1
			ally:SetMod(modifiersGlobal.MovementSpeed, 8)
			allyGlobal.EngageTarget(ally, target)   -- Engage + AddBaseHate (ally.lua)
		end
	end
	-- Aurelias: proactive once battle starts (retaliation hate already
	-- covers the struck one; this brings the bystanders in).
	for i = 1, #mobs do
		local mob = mobs[i]
		if mob and not mob:IsEngaged() then
			local foe = (i % 2 == 0 and #allies > 0) and allies[1]
						or (engagedPlayer or (#allies > 0 and allies[1]))
			if foe then allyGlobal.EngageTarget(mob, foe) end
		end
	end
end
