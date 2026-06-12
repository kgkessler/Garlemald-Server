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
-- Escort shape (2010-09 recording FEI03OpK99o + era guides): Sisipu
-- walks the southbound road out of Zephyr Gate ("Oschon's Torch is due
-- south"), ankle-biter packs ambush along the way, and on arrival the
-- processEvent605 echo plays at the lighthouse. Coordinates are
-- DERIVED (no 1.x capture renders XYZ): a compressed road segment at
-- the gate's seeded height, with the remaining distance masked by the
-- arrival warp — see 056_man0l1_zephyr_escort.sql. The walk pauses
-- whenever live mobs are near Sisipu or she's engaged, and the
-- completion signal requires both arrival AND a cleared road.

-- Sisipu's waypoints (escort beat order). The final entry is the
-- arrival point; ambush clusters from the 056 seed sit just past
-- waypoints 1-3.
ESCORT_WAYPOINTS = {
	{ x = -52.0, z = 210.0 },
	{ x = -36.0, z = 255.0 },
	{ x = -20.0, z = 300.0 },
	{ x = -8.0,  z = 340.0 },
};
ESCORT_Y = 33.15;            -- gate road height (seed-anchored)
WALK_STEP = 1.6;             -- units per 500 ms tick (escort walking pace)
ARRIVE_EPSILON = 3.0;        -- "reached the waypoint" distance
HOLD_RADIUS = 28.0;          -- live mob within this of Sisipu → hold the walk
ENGAGE_RADIUS = 18.0;        -- mob pulls onto the escort inside this

-- Per-player escort state, keyed by actor id: the VM is process-cached
-- per script path, so a plain scalar would interleave concurrent runs
-- (same rationale as the tutorial scripts' tutorialLiveHostiles).
escortState = {};

function onCreate(starterPlayer, contentArea, director)
	escortState[starterPlayer.actorId] = { wp = 1, signaled = false };

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

	-- Party-add the escort so her HP bar lights the HUD roster;
	-- ContentFinished clears the transient roster at teardown.
	starterPlayer.currentParty:AddMember(sisipu.actorId);

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

	-- Hold the walk while the road ahead is contested.
	if nearLive > 0 or escort:IsEngaged() then
		return
	end

	local wp = ESCORT_WAYPOINTS[state.wp]
	if not wp then return end
	local remaining = dist2d(escort.positionX, escort.positionZ, wp.x, wp.z)
	if remaining <= ARRIVE_EPSILON then
		if state.wp >= #ESCORT_WAYPOINTS then
			if #mobs == 0 then
				-- Arrived with a clear road — resume the director beat
				-- (QuestDirectorMan0l101 parks on this right after the
				-- opening cutscene).
				state.signaled = true
				sendSignal("escortComplete")
			end
			return
		end
		state.wp = state.wp + 1
		return
	end

	-- One walking step toward the current waypoint, facing travel.
	local step = math.min(WALK_STEP, remaining)
	local dx = (wp.x - escort.positionX) / remaining
	local dz = (wp.z - escort.positionZ) / remaining
	escort:MoveTo(
		escort.positionX + dx * step,
		ESCORT_Y,
		escort.positionZ + dz * step,
		math.atan(dx, dz),
		1)   -- moveState 1 = walk
end
