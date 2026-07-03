require ("global")
require ("modifiers")
-- onUpdate drives ally/mob engagement through allyGlobal.EngageTarget;
-- without this require the global is nil and every tick errors
-- (swallowed by the ticker drain). Same shape as SimpleContent30002.
require ("ally")

-- Man0l1 "Treasures of the Main" SEQ_050 — the Zephyr Gate → Oschon's
-- Torch escort duty (Garlemald-Server #46). Net-new content: upstream
-- pmeteor shipped the trigger arm commented out ("DO ESCORT DUTY HERE
-- ... For now just skip the sequence") and its QuestDirectorMan0l101
-- onCreateContentArea spawn was dead code, so there is no upstream
-- reference implementation — this is built on the #28 tutorial-fight
-- machinery (SpawnBattleNpcById onCreate spawning, the 500 ms onUpdate
-- driver, allyGlobal engagement, sendSignal → director coroutine).
--
-- Runtime shape: the duty runs as a content instance in zone 129
-- ('sea0Field02', Camp Skull Valley — see seed 066), warped into from the
-- Zephyr Gate (man0l1.lua startMan0l1Content). onCreate spawns Sisipu +
-- eight ankle biters clustered on the camp floor; onUpdate has Sisipu
-- FOLLOW the player (who walks the real navmesh) and pull/hold on nearby
-- live mobs. Completion is the kill count — once every ankle biter is dead
-- the road is "clear" and we sendSignal("escortComplete"); the director
-- then plays the processEvent605 arrival echo and warps to the lighthouse.
-- (The old hardcoded southbound-waypoint walk was removed: it had no
-- navmesh and walked Sisipu off the cliff — 2026-06-17.)

WALK_STEP = 1.6;             -- units per 500 ms tick (escort walking pace)
HOLD_RADIUS = 28.0;          -- live mob within this of Sisipu → hold the walk
ENGAGE_RADIUS = 18.0;        -- mob pulls onto the escort inside this

-- Per-player escort state, keyed by actor id: the VM is process-cached
-- per script path, so a plain scalar would interleave concurrent runs
-- (same rationale as the tutorial scripts' tutorialLiveHostiles).
escortState = {};

function onCreate(starterPlayer, contentArea, director)
	escortState[starterPlayer.actorId] = { signaled = false };

	sisipu = GetWorldManager().SpawnBattleNpcById(16, contentArea);
	local mobs = {};
	for bnpcId = 17, 24 do
		table.insert(mobs, GetWorldManager().SpawnBattleNpcById(bnpcId, contentArea));
	end

	-- Active MainState so Sisipu stands and the ankle biters render
	-- hostile (tutorial-fight pattern).
	sisipu:ChangeState(2);
	for i = 1, #mobs do
		mobs[i]:ChangeState(2);
	end

	-- NO party-add here. onCreate runs PRE-warp, and a currentParty:AddMember
	-- broadcasts a content/party group trio (0x017C/D/F/E) referencing Sisipu
	-- before the client has spawned her — a pre-kick divergence from pmeteor's
	-- working SEQ_005 warp burst (which emits no party trio pre-kick). The HUD
	-- HP-bar party-add is a cosmetic follow-up to wire post-warp once the warp
	-- itself is solid. (Garlemald-Server #46.)

	-- No-fail guarantees: retail fails the escort when Sisipu dies and
	-- restarts it; garlemald's first cut floors her (and the player)
	-- at 1 HP instead — the eject-and-restart leash is a follow-up.
	starterPlayer:SetMod(modifiersGlobal.MinimumHpLock, 1);
	sisipu:SetMod(modifiersGlobal.MinimumHpLock, 1);

	director:AddMember(starterPlayer);
	director:AddMember(director);
	director:AddMember(sisipu);
	for i = 1, #mobs do
		director:AddMember(mobs[i]);
	end
end

function onDestroy()
end

local function dist2d(ax, az, bx, bz)
	local dx = ax - bx;
	local dz = az - bz;
	return math.sqrt(dx * dx + dz * dz);
end

function onUpdate(tick, area)
	if not area then return end
	local players = area:GetPlayers()
	local mobs    = area:GetMonsters()   -- live-only (dead filtered)
	local allies  = area:GetAllies()

	local owner = nil
	for player in players do
		if player then owner = owner or player end
	end
	if not owner then return end
	local state = escortState[owner.actorId]
	if not state or state.signaled then return end

	-- Sisipu is the lone ally in this content.
	local escort = allies[1]
	if not escort then return end

	-- Mob pulls: any live ankle biter inside the engage radius of the
	-- escort or the player joins the fight; Sisipu fights back like the
	-- tutorial allies do.
	local nearLive = 0
	for i = 1, #mobs do
		local mob = mobs[i]
		if mob then
			local dMob = math.min(
				dist2d(mob.positionX, mob.positionZ, escort.positionX, escort.positionZ),
				dist2d(mob.positionX, mob.positionZ, owner.positionX, owner.positionZ))
			if dMob <= HOLD_RADIUS then
				nearLive = nearLive + 1
			end
			if dMob <= ENGAGE_RADIUS and not mob:IsEngaged() then
				allyGlobal.EngageTarget(mob, (i % 2 == 0) and escort or owner)
			end
		end
	end
	if not escort:IsEngaged() and nearLive > 0 and #mobs > 0 then
		allyGlobal.EngageTarget(escort, mobs[1])
	end

	-- Completion = the road is cleared (every ankle biter defeated). Sisipu
	-- FOLLOWS the player (who walks the real navmesh) rather than leading a
	-- hardcoded route. (Garlemald-Server #46 — pathing follow-up: a real
	-- navmesh route to Oschon's Torch so she can lead like retail.)
	if #mobs == 0 then
		state.signaled = true
		sendSignal("escortComplete")
		return
	end

	-- Hold while contested; otherwise trail the player, staying ~3u behind so
	-- she never outruns navigable ground (the player navigates valid terrain).
	if nearLive > 0 or escort:IsEngaged() then
		return
	end
	local FOLLOW_GAP = 3.0
	local d = dist2d(escort.positionX, escort.positionZ, owner.positionX, owner.positionZ)
	if d > FOLLOW_GAP then
		local step = math.min(WALK_STEP, d - FOLLOW_GAP)
		local dx = (owner.positionX - escort.positionX) / d
		local dz = (owner.positionZ - escort.positionZ) / d
		escort:MoveTo(
			escort.positionX + dx * step,
			owner.positionY,
			escort.positionZ + dz * step,
			math.atan(dx, dz),
			1)   -- moveState 1 = walk
	end
end
