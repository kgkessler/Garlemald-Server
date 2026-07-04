require ("global")
require ("modifiers")
-- onUpdate drives ally/mob engagement through allyGlobal.EngageTarget;
-- without this require the global is nil and every tick errors
-- (swallowed by the ticker drain). Same shape as SimpleContent30002.
require ("ally")
-- SEQ_* constants + the escort TEXT_*/SAY_* constants (proven loadable
-- outside the quest runtime — QuestDirectorMan0l101 requires it too).
require ("quests/man/man0l1")

-- Man0l1 "Treasures of the Main" SEQ_050 — the Zephyr Gate → Oschon's
-- Torch escort duty (Garlemald-Server #46). Net-new content: upstream
-- pmeteor shipped the trigger arm commented out ("DO ESCORT DUTY HERE
-- ... For now just skip the sequence"), so there is no upstream
-- reference implementation — this is built on the #28 tutorial-fight
-- machinery (SpawnBattleNpcById onCreate spawning, the 500 ms onUpdate
-- driver, allyGlobal engagement, sendSignal → director coroutine).
--
-- Runtime shape (retail spec from OCR'd playthroughs + Mirke
-- transcripts): the duty runs as a content instance on ZONE 128 (retail
-- geography — see seed/070; the zone-129 Skull Valley pin from seed/066
-- is kept for rollback). Sisipu LEADS the walk due south from the gate
-- to the lighthouse approach along the ROUTE waypoints, barking her
-- say-337 guidance every ~20 s; the eight seeded ankle biters
-- (chigoes) sit in three route clusters and pull as the party reaches
-- each segment. Arrival (both Sisipu and the player inside the final-
-- waypoint radius, road clear) fires sendSignal("escortComplete") → the
-- director plays the processEvent605 arrival echo and warps to the
-- lighthouse camp. Failure (Sisipu death OR the 30-minute limit) tears
-- the content down, rolls the quest back to SEQ_048 (the ZEPHYR_TRIGGER
-- re-arms for retry) and ejects the player to the gate.

-- Escort pacing / geometry. The onUpdate driver ticks every 500 ms
-- (runtime/ticker.rs CONTENT_UPDATE_PERIOD_MS), so ticks = seconds * 2.
WALK_STEP = 1.6;             -- units per 500 ms tick (escort walking pace)
RUN_STEP = 3.4;              -- units per 500 ms tick (run pace — 7e walk was too slow)
HOLD_RADIUS = 28.0;          -- live mob within this of Sisipu/player → hold the walk
-- Render-before-fight invariant: ENGAGE_RADIUS (18) + PLAYER_LEASH (15)
-- stays below the server's INSTANCE_STREAM_RADIUS (50, world_manager.rs)
-- — as does the controller's MAX_DETECT_DISTANCE (20) — so every mob is
-- AddActor'd to the client before it can possibly pull or reach the
-- party. Widen these past 50 and mobs fight invisibly again (#46).
ENGAGE_RADIUS = 18.0;        -- mob pulls onto the escort party inside this
PLAYER_LEASH = 15.0;         -- Sisipu > this from the player → close the gap directly
WAYPOINT_RADIUS = 4.0;       -- close enough to the lead point → stop stepping
ARRIVAL_RADIUS = 20.0;       -- player inside this of the lighthouse goal → arrival
LEAD_DISTANCE = 5.0;         -- how far ahead of the player Sisipu paces while walking
MOVE_EPSILON = 0.3;          -- player displacement/tick above this = "walking"
TICKS_PER_SECOND = 2;
ESCORT_LIMIT_TICKS = 30 * 60 * TICKS_PER_SECOND;    -- retail: "There are 30 minutes remaining."
REMIND_10MIN_TICKS = 20 * 60 * TICKS_PER_SECOND;    -- 10 minutes remaining
REMIND_5MIN_TICKS  = 25 * 60 * TICKS_PER_SECOND;    -- 5 minutes remaining
BARK_INTERVAL_TICKS = 40;                           -- guidance bark every ~20 s

-- Sisipu's path: the PLAYER'S OWN recorded footsteps (round 7f). Decoded
-- from the 2026-07-03 21:38-21:47 escort run's inbound 0x00CA position
-- packets (offset verified against the warp anchor -63.25/33.16/164.51 and
-- a mid-run corridor sample) and downsampled to ~20-unit breadcrumbs: every
-- point is REAL walked ground with true terrain Y, so tracing them can never
-- leave walkable land or float (the navmesh is still a stub). The recording
-- reaches z~810 (~56% of the way); past the last breadcrumb Sisipu switches
-- to player-relative follow (round 7e logic) for the final stretch. Re-record
-- a fuller run to extend. ARRIVAL_GOAL is pmeteor's SEQ_055 camp point.
TRAIL = {
	{ x = -63.25, y = 33.16, z = 164.51 },
	{ x = -43.93, y = 37.11, z = 156.42 },
	{ x = -27.49, y = 41.10, z = 147.08 },
	{ x = -10.56, y = 44.70, z = 141.54 },
	{ x = 9.67, y = 45.20, z = 142.33 },
	{ x = 29.18, y = 43.50, z = 138.89 },
	{ x = 46.76, y = 45.79, z = 139.23 },
	{ x = 62.63, y = 46.50, z = 131.88 },
	{ x = 81.39, y = 45.86, z = 127.04 },
	{ x = 100.35, y = 44.34, z = 129.65 },
	{ x = 105.90, y = 47.10, z = 146.56 },
	{ x = 122.67, y = 45.81, z = 153.04 },
	{ x = 141.81, y = 46.77, z = 161.73 },
	{ x = 157.87, y = 48.45, z = 170.87 },
	{ x = 162.43, y = 50.15, z = 190.54 },
	{ x = 161.41, y = 48.73, z = 209.05 },
	{ x = 150.93, y = 46.43, z = 227.20 },
	{ x = 141.69, y = 45.26, z = 246.37 },
	{ x = 140.46, y = 45.34, z = 263.22 },
	{ x = 135.40, y = 45.30, z = 282.75 },
	{ x = 123.47, y = 45.33, z = 300.39 },
	{ x = 111.30, y = 45.34, z = 315.91 },
	{ x = 101.78, y = 43.77, z = 331.01 },
	{ x = 90.06, y = 41.19, z = 346.85 },
	{ x = 82.41, y = 42.37, z = 366.92 },
	{ x = 74.50, y = 45.65, z = 382.13 },
	{ x = 70.14, y = 44.87, z = 402.50 },
	{ x = 69.93, y = 43.04, z = 422.19 },
	{ x = 69.66, y = 41.34, z = 439.97 },
	{ x = 69.33, y = 41.57, z = 461.57 },
	{ x = 69.48, y = 43.69, z = 480.82 },
	{ x = 86.94, y = 41.49, z = 490.07 },
	{ x = 104.71, y = 41.87, z = 489.96 },
	{ x = 122.22, y = 44.96, z = 501.00 },
	{ x = 131.49, y = 45.89, z = 518.22 },
	{ x = 136.96, y = 44.34, z = 535.47 },
	{ x = 140.17, y = 49.92, z = 556.61 },
	{ x = 138.78, y = 54.40, z = 574.13 },
	{ x = 134.25, y = 52.75, z = 595.02 },
	{ x = 134.05, y = 52.47, z = 612.74 },
	{ x = 131.34, y = 54.53, z = 632.71 },
	{ x = 127.51, y = 54.30, z = 653.87 },
	{ x = 124.33, y = 52.29, z = 671.40 },
	{ x = 120.48, y = 55.54, z = 692.71 },
	{ x = 118.79, y = 56.85, z = 710.58 },
	{ x = 111.83, y = 62.77, z = 727.47 },
	{ x = 90.54, y = 64.47, z = 726.65 },
	{ x = 77.07, y = 62.68, z = 736.72 },
	{ x = 66.36, y = 57.28, z = 755.40 },
	{ x = 52.76, y = 57.17, z = 764.91 },
	{ x = 46.25, y = 59.88, z = 775.92 },
	{ x = 53.72, y = 61.38, z = 793.35 },
	{ x = 44.38, y = 63.70, z = 807.00 },
};
ARRIVAL_GOAL = { x = 137.44, y = 60.33, z = 1322.0 };

-- Sisipu's bark slots. Only `guidance` has a wire-confirmed id
-- (man0l1 QUEST-sheet say 337, decoded client processTtrBlkNml001 —
-- "Oschon's Torch is due south..."). The conditional wave-outcome
-- variants are attested verbatim in the retail recordings but their
-- say ids are UNKNOWN — the nil slots are skipped by emitBark, so a
-- future id-probe just fills numbers in. (Say 119 is her SEQ_055 camp
-- talk and does NOT belong here.)
BARKS = {
	guidance   = SAY_ESCORT_GUIDANCE,   -- 337, confirmed
	waveClean  = nil,   -- TODO id: "Good show! ..." (clean wave clear)
	waveClose  = nil,   -- TODO id: "Phew... That was much closer..." (close call)
	sisipuHurt = nil,   -- TODO id: "Ow ow ow..." (Sisipu took a beating)
	dawdle     = nil,   -- TODO id: "Now put away your toys and come along, <Forename>..."
	arrival    = nil,   -- TODO id: "We've finally arrived...and in one piece!"
};

-- Per-player escort state, keyed by actor id: the VM is process-cached
-- per script path, so a plain scalar would interleave concurrent runs
-- (same rationale as the tutorial scripts' tutorialLiveHostiles).
escortState = {};

function onCreate(starterPlayer, contentArea, director)
	escortState[starterPlayer.actorId] = {
		done = false,        -- terminal latch (arrival signalled OR failed)
		wpIndex = 1,         -- next TRAIL breadcrumb (re-seated to nearest on first tick)
		startTick = nil,     -- latched on the first onUpdate tick (post-warp)
		lastBarkTick = nil,
		lastNear = 0,        -- live-mob count near the party last tick (wave-outcome beat)
		reminded10 = false,
		reminded5 = false,
		sawEscort = false,   -- Sisipu observed alive at least once (death detection)
	};

	-- Zone-128 route spawns (seed/070). Rows 16-24 are the superseded
	-- zone-129 Skull Valley set, kept for rollback — flip these ids back
	-- to 16..24 alongside man0l1.lua's zone pin to roll back.
	sisipu = GetWorldManager().SpawnBattleNpcById(25, contentArea);
	local mobs = {};
	for bnpcId = 26, 33 do
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

	-- The PLAYER keeps the tutorial-style 1-HP floor (retail wipes are
	-- a rez-and-retry; garlemald's death/return flow isn't wired for
	-- content yet). Sisipu takes REAL damage now — her death is the
	-- retail fail condition, detected in onUpdate (she leaves the
	-- live-only ally roster) → escortFail. (Garlemald-Server #46 —
	-- replaces the first cut's MinimumHpLock on her.)
	starterPlayer:SetMod(modifiersGlobal.MinimumHpLock, 1);

	director:AddMember(starterPlayer);
	director:AddMember(director);
	director:AddMember(sisipu);
	for i = 1, #mobs do
		director:AddMember(mobs[i]);
	end
end

-- Entry text moved to the DIRECTOR's post-warp chain (round 7): this
-- hook fires inside apply_do_zone_change_content immediately after the
-- bundle flush — ~4s BEFORE the client's RX 0x0007 (wire-proven
-- 2026-07-03 14:24: the three lines + bark 337 + Sisipu's first walk
-- steps all shipped into the Now-Loading gap). The director emits them
-- after its fade-in delegate instead, and the go-latch below holds the
-- rest of the duty. (Garlemald-Server #46, round 7.)
function onZoneIn(player, contentArea, director)
end

function onDestroy()
end

local function dist2d(ax, az, bx, bz)
	local dx = ax - bx;
	local dz = az - bz;
	return math.sqrt(dx * dx + dz * dz);
end

-- Sisipu say-line delivery: quest-sheet text via the server-side
-- SendGameMessage mechanism (same shape as man0l1.lua's
-- seq007_endSequence 333/334 sends — text owner = the Man0l1 quest
-- actor). nil ids (unprobed bark slots) are skipped.
local function emitBark(owner, sayId)
	if (sayId == nil) then
		return;
	end
	local quest = owner:GetQuest("Man0l1");
	owner:SendGameMessage(quest, sayId, 0x20);
end

-- FAIL flow (Sisipu died / 30-minute expiry / confirmed leave-duty):
-- roll the quest back to SEQ_048 — its onStateChange re-arms the
-- ZEPHYR_TRIGGER push circle, so the duty is retryable from the gate —
-- unbind message, tear the instance down, eject to the gate.
-- (Garlemald-Server #46 — retail: failure ejects back to the gate and
-- the escort can be restarted.)
local function escortFail(owner, area, state)
	state.done = true;
	local quest = owner:GetQuest("Man0l1");
	quest:StartSequence(SEQ_048);
	owner:SendGameMessage(GetWorldMaster(), TEXT_UNBOUND_FROM_DUTY, 0x20);
	-- ContentFinished BEFORE the warp-out (QuestDirectorMan0l001
	-- teardown order); the eject is a public-area warp to the gate.
	area:ContentFinished();
	GetWorldManager():WarpToPublicArea(owner, -63.25, 33.15, 164.51, 0.8);
	-- Drain the director coroutine parked on waitForSignal
	-- ("escortComplete") so a stale park can't double-fire the arrival
	-- flow on a later retry (drain_signal wakes EVERY parked coroutine
	-- with that name). The woken coroutine's kickEventContinue emits a
	-- KickEvent for the just-wiped director actor — the client drops
	-- kicks for wiped actors (wire-proven, session 53943), the
	-- coroutine parks on _WAIT_EVENT and is displaced by the player's
	-- next event; crucially its StartSequence(SEQ_055) tail sits AFTER
	-- the never-answered callClientFunction, so no quest state moves.
	-- (Garlemald-Server #46 — known-limitation breadcrumb: a Rust-side
	-- scheduler purge on ContentFinished would retire this dance.)
	sendSignal("escortComplete");
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
	if not state or state.done then return end

	-- NO cross-VM go-latch (round 7d). The director and this content
	-- script load into SEPARATE Lua VMs (the vm_cache is keyed by script
	-- path), so a `man0l1EscortGo` global the director set in ITS VM is
	-- invisible here — the previous gate read this VM's copy (always
	-- false) and Sisipu never moved (wire-proven 20:50: zero
	-- MoveActorToPosition). The ticker already parks the content driver
	-- on `content_warp_acked` (the client's post-warp zone-in echo), and
	-- the veil is cleared by the director's questBaseRewardSeting ~1s in,
	-- so the escort simply runs from the first post-ack tick — a couple
	-- of Sisipu's steps during the fade are invisible. (Garlemald-Server
	-- #46, round 7d.)

	-- ---- Timer (retail: 30-minute limit) ----
	-- `tick` is the ticker's monotonic 500 ms frame counter; latch the
	-- first faded-in frame and measure elapsed from there. The 10/5-min
	-- reminder LINES are omitted for now (round 7d): their text id was
	-- the boundary-family 34112 "Enter this instance?" — wrong. The
	-- expiry FAIL is real and kept; the reminder sends return once the
	-- correct "There are N minutes remaining." id is probed.
	state.startTick = state.startTick or tick;
	local elapsed = tick - state.startTick;
	if elapsed >= ESCORT_LIMIT_TICKS then
		escortFail(owner, area, state);
		return;
	end

	-- ---- Sisipu death = fail (rosters are live-only, so a dead
	-- escort simply vanishes from GetAllies) ----
	local escort = allies[1]
	if escort then
		state.sawEscort = true;
	elseif state.sawEscort then
		escortFail(owner, area, state);
		return;
	else
		-- Pre-onCreate tick (roster not populated yet) — wait.
		return;
	end

	-- ---- Ambush waves ----
	-- The three seeded clusters are >100 units apart along the route, so
	-- the engage radius activates exactly the wave whose segment the
	-- party has reached: any live ankle biter inside ENGAGE_RADIUS of
	-- the escort or the player joins the fight (2-3 at once — cluster
	-- size); Sisipu fights back like the tutorial allies do.
	-- Round 7h: skip actors whose roster position is still unsynced
	-- (0,0,0) — on the spawn tick every distance reads ~0, so the whole
	-- wave-1 cluster engaged the player AND Sisipu instantly from ~225
	-- units away (log 23:05:49: ActorEngage rows at content-spawn time =
	-- the "invisible enemies" report). A real position never sits exactly
	-- at origin in zone 128.
	local function positionLive(a)
		return a and not (a.positionX == 0 and a.positionY == 0 and a.positionZ == 0);
	end
	if not positionLive(escort) or not positionLive(owner) then
		return;
	end

	local nearLive = 0
	local anyEngaged = false
	local nearestMob, nearestD = nil, math.huge
	for i = 1, #mobs do
		local mob = mobs[i]
		if mob and positionLive(mob) then
			local dMob = math.min(
				dist2d(mob.positionX, mob.positionZ, escort.positionX, escort.positionZ),
				dist2d(mob.positionX, mob.positionZ, owner.positionX, owner.positionZ))
			if dMob <= HOLD_RADIUS then
				nearLive = nearLive + 1
			end
			if mob:IsEngaged() then
				anyEngaged = true
			end
			if dMob < nearestD then
				nearestMob, nearestD = mob, dMob
			end
			if dMob <= ENGAGE_RADIUS and not mob:IsEngaged() then
				allyGlobal.EngageTarget(mob, (i % 2 == 0) and escort or owner)
			end
		end
	end
	-- Sisipu joins the fight against the NEAREST in-range mob only.
	-- (Round 7g: this used to target mobs[1] — the first ROSTER entry,
	-- which could be a far unstreamed cluster hundreds of units away →
	-- ranged combat against an INVISIBLE attacker, the 8/8 report.)
	if not escort:IsEngaged() and nearestMob ~= nil and nearestD <= ENGAGE_RADIUS then
		allyGlobal.EngageTarget(escort, nearestMob)
	end

	-- Wave-outcome beat: the contested count dropping back to zero =
	-- the wave was cleared this tick. The conditional variants (clean /
	-- close-call / Sisipu-hurt) need her HP, which the roster userdata
	-- doesn't expose yet — emit the (unprobed) clean slot only.
	if nearLive == 0 and state.lastNear > 0 then
		emitBark(owner, BARKS.waveClean);
	end
	state.lastNear = nearLive;

	-- ---- Hold while contested (round 7g: she stays PUT until every
	-- enemy of the active gate is dead — any live mob engaged with the
	-- party, or still lurking inside the hold radius, pins her) ----
	if nearLive > 0 or anyEngaged or escort:IsEngaged() then
		return
	end

	-- ---- Arrival ----
	-- The PLAYER reaching the lighthouse-approach point ends the duty
	-- (the journal objective marker points them there). Sisipu is leashed
	-- to the player below, so she is always in range too. → arrival bark
	-- → the director's kickEventContinue machinery (processEvent605 echo →
	-- SEQ_055 → camp warp) takes over.
	if dist2d(owner.positionX, owner.positionZ, ARRIVAL_GOAL.x, ARRIVAL_GOAL.z) <= ARRIVAL_RADIUS then
		state.done = true;
		emitBark(owner, BARKS.arrival);
		sendSignal("escortComplete");
		return;
	end

	-- ---- Sisipu TRACES the player's recorded footsteps (round 7f) ----
	-- She advances breadcrumb-to-breadcrumb along TRAIL — every point is
	-- ground the player actually walked (real terrain Y), so she cannot
	-- straight-line into the ocean or float (the 7e player-relative
	-- follow cut corners off walkable land). She waits when the player
	-- falls behind the leash, and once the recorded trail is exhausted
	-- (the recording stops at z~810) she transitions to the 7e
	-- player-relative follow for the final unrecorded stretch, as
	-- specified. (Garlemald-Server #46, round 7f.)
	local dPlayer = dist2d(escort.positionX, escort.positionZ, owner.positionX, owner.positionZ);

	-- One-time: start from the trail point nearest her spawn (the first
	-- breadcrumb is the warp-in spot behind her seed position).
	if not state.trailInit then
		state.trailInit = true;
		local bestI, bestD = 1, math.huge;
		for i = 1, #TRAIL do
			local d = dist2d(escort.positionX, escort.positionZ, TRAIL[i].x, TRAIL[i].z);
			if d < bestD then bestI, bestD = i, d; end
		end
		state.wpIndex = bestI;
	end

	if dPlayer > PLAYER_LEASH then
		-- Player lagging (fight, detour) — she holds and (rate-limited)
		-- chides rather than walking off; the trail keeps her honest, the
		-- leash keeps her helpful.
		if state.lastBarkTick == nil or tick - state.lastBarkTick >= BARK_INTERVAL_TICKS then
			state.lastBarkTick = tick;
			emitBark(owner, BARKS.dawdle);
		end
		return;
	end

	if state.wpIndex <= #TRAIL then
		-- ON-TRAIL: step toward the next recorded breadcrumb at ITS
		-- recorded ground Y. RUN pace (moveState 2) — the 7e walk was
		-- reported too slow to keep up with a running player.
		-- Skip THROUGH every breadcrumb already within radius in one
		-- tick (round 7g: overlapping recorded points used to cost one
		-- idle tick each — she stood in place burning time), then move
		-- toward the first genuinely-ahead one this same tick.
		local wp, d;
		repeat
			wp = TRAIL[state.wpIndex];
			d = wp and dist2d(escort.positionX, escort.positionZ, wp.x, wp.z) or nil;
			if d ~= nil and d <= WAYPOINT_RADIUS then
				state.wpIndex = state.wpIndex + 1;
			end
		until wp == nil or d == nil or d > WAYPOINT_RADIUS or state.wpIndex > #TRAIL;
		if wp ~= nil and d ~= nil and d > WAYPOINT_RADIUS then
			local dx = (wp.x - escort.positionX) / d;
			local dz = (wp.z - escort.positionZ) / d;
			local step = math.min(RUN_STEP, d);
			escort:MoveTo(escort.positionX + dx * step, wp.y, escort.positionZ + dz * step,
				math.atan(dx, dz), 2);
		end
	else
		-- TRAIL EXHAUSTED: player-relative follow (7e) for the last
		-- stretch — her Y comes from the player's real ground height.
		local py = owner.positionY;
		local last = state.lastOwnerX and { x = state.lastOwnerX, z = state.lastOwnerZ } or nil;
		local moved = last and dist2d(owner.positionX, owner.positionZ, last.x, last.z) or 0;
		if dPlayer > LEAD_DISTANCE and moved > MOVE_EPSILON then
			local d = math.max(dPlayer, 0.001);
			local dx = (owner.positionX - escort.positionX) / d;
			local dz = (owner.positionZ - escort.positionZ) / d;
			local step = math.min(RUN_STEP, dPlayer);
			escort:MoveTo(escort.positionX + dx * step, py, escort.positionZ + dz * step,
				math.atan(dx, dz), 2);
		end
	end
	state.lastOwnerX = owner.positionX;
	state.lastOwnerZ = owner.positionZ;

	-- Guidance bark ("Oschon's Torch is due south...") every ~20 s while
	-- escorting — say 337, wire-confirmed.
	if state.lastBarkTick == nil or tick - state.lastBarkTick >= BARK_INTERVAL_TICKS then
		state.lastBarkTick = tick;
		emitBark(owner, BARKS.guidance);
	end
end

-- Leave-duty teardown (the commandContent confirmed-leave, driven from
-- the Rust command surface — there is no Lua command script for it):
-- same eject-and-retry flow as a timeout/death fail. Rust can fire this
-- via call_content_hook(script, "onAbort", ...) with the same
-- (player, contentArea, director) shape as onZoneIn.
-- (Garlemald-Server #46 — coordinate with the R5 leave-duty track.)
function onAbort(player, contentArea, director)
	local state = escortState[player.actorId];
	if state == nil then
		state = { done = false };
		escortState[player.actorId] = state;
	end
	if state.done then
		return;
	end
	escortFail(player, contentArea, state);
end
