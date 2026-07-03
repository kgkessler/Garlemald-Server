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
HOLD_RADIUS = 28.0;          -- live mob within this of Sisipu/player → hold the walk
ENGAGE_RADIUS = 18.0;        -- mob pulls onto the escort party inside this
PLAYER_LEASH = 15.0;         -- Sisipu advances only while the player is within this
WAYPOINT_RADIUS = 4.0;       -- close enough → next waypoint
ARRIVAL_RADIUS = 20.0;       -- player+Sisipu inside this of the final waypoint → arrival
TICKS_PER_SECOND = 2;
ESCORT_LIMIT_TICKS = 30 * 60 * TICKS_PER_SECOND;    -- retail: "There are 30 minutes remaining."
REMIND_10MIN_TICKS = 20 * 60 * TICKS_PER_SECOND;    -- 10 minutes remaining
REMIND_5MIN_TICKS  = 25 * 60 * TICKS_PER_SECOND;    -- 5 minutes remaining
BARK_INTERVAL_TICKS = 40;                           -- guidance bark every ~20 s

-- The southbound road, gate → lighthouse approach. Route-INTERPOLATED
-- between the gate (-63, 33, 164) and pmeteor's SEQ_055 camp warp
-- destination (137.44, 60.33, 1322.0) — exact retail positions are
-- unknowable (the 1.x HUD never drew XYZ); same derivation as the
-- seed/070 ambush clusters, flagged for in-client tuning. The last
-- entry is the arrival point.
ROUTE = {
	{ x = -34.9, y = 38.2, z = 250.0 },
	{ x =  -2.7, y = 42.4, z = 450.0 },   -- wave 1 cluster (bnpc 26-28)
	{ x =  24.6, y = 45.9, z = 620.0 },
	{ x =  53.6, y = 49.6, z = 800.0 },   -- wave 2 cluster (bnpc 29-31)
	{ x =  77.7, y = 52.7, z = 950.0 },
	{ x = 101.8, y = 55.8, z = 1100.0 },  -- wave 3 cluster (bnpc 32-33)
	{ x = 130.7, y = 59.5, z = 1280.0 },  -- lighthouse approach (arrival)
};

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
	-- Fresh run: hold the duty until the director's post-warp chain has
	-- the client faded in and interactive (retry-safe reset — the
	-- director sets it TRUE at the end of its noticeEvent chain).
	man0l1EscortGo = false;

	escortState[starterPlayer.actorId] = {
		done = false,        -- terminal latch (arrival signalled OR failed)
		wpIndex = 1,         -- next ROUTE waypoint Sisipu walks toward
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

	-- Hold EVERYTHING (clock, waypoints, barks, wave beats) until the
	-- director's post-warp chain reports the client faded-in and
	-- interactive — otherwise the duty runs under the Now-Loading veil
	-- and the 30-minute clock cheats the player (round 7).
	if not man0l1EscortGo then return end

	-- ---- Timer (retail: 30-minute limit, reminder lines) ----
	-- `tick` is the ticker's monotonic 500 ms frame counter; latch the
	-- first faded-in frame and measure elapsed from there.
	state.startTick = state.startTick or tick;
	local elapsed = tick - state.startTick;
	if (not state.reminded10) and elapsed >= REMIND_10MIN_TICKS then
		state.reminded10 = true;
		owner:SendGameMessage(GetWorldMaster(), TEXT_TIME_REMAINING, 0x20, 10);
	end
	if (not state.reminded5) and elapsed >= REMIND_5MIN_TICKS then
		state.reminded5 = true;
		owner:SendGameMessage(GetWorldMaster(), TEXT_TIME_REMAINING, 0x20, 5);
	end
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

	-- Wave-outcome beat: the contested count dropping back to zero =
	-- the wave was cleared this tick. The conditional variants (clean /
	-- close-call / Sisipu-hurt) need her HP, which the roster userdata
	-- doesn't expose yet — emit the (unprobed) clean slot only.
	if nearLive == 0 and state.lastNear > 0 then
		emitBark(owner, BARKS.waveClean);
	end
	state.lastNear = nearLive;

	-- ---- Hold while contested ----
	if nearLive > 0 or escort:IsEngaged() then
		return
	end

	-- ---- Arrival ----
	-- Road clear and both Sisipu and the player inside the lighthouse-
	-- approach radius → arrival bark → the director's kickEventContinue
	-- machinery (processEvent605 echo → SEQ_055 → camp warp) takes over.
	local goal = ROUTE[#ROUTE];
	if state.wpIndex > #ROUTE
		or (dist2d(escort.positionX, escort.positionZ, goal.x, goal.z) <= ARRIVAL_RADIUS
			and dist2d(owner.positionX, owner.positionZ, goal.x, goal.z) <= ARRIVAL_RADIUS) then
		state.done = true;
		emitBark(owner, BARKS.arrival);
		sendSignal("escortComplete");
		return;
	end

	-- ---- Sisipu LEADS (retail: she walks the route, the player
	-- follows; she waits when the player dawdles or a fight is on) ----
	local dPlayer = dist2d(escort.positionX, escort.positionZ, owner.positionX, owner.positionZ);
	if dPlayer > PLAYER_LEASH then
		-- Player lagging — she waits and (rate-limited) chides. The
		-- dawdle say id is an unprobed nil slot today.
		if state.lastBarkTick == nil or tick - state.lastBarkTick >= BARK_INTERVAL_TICKS then
			state.lastBarkTick = tick;
			emitBark(owner, BARKS.dawdle);
		end
		return;
	end

	local wp = ROUTE[state.wpIndex];
	local d = dist2d(escort.positionX, escort.positionZ, wp.x, wp.z);
	if d <= WAYPOINT_RADIUS then
		state.wpIndex = state.wpIndex + 1;
		return;
	end
	local step = math.min(WALK_STEP, d);
	local dx = (wp.x - escort.positionX) / d;
	local dz = (wp.z - escort.positionZ) / d;
	escort:MoveTo(
		escort.positionX + dx * step,
		wp.y,
		escort.positionZ + dz * step,
		math.atan(dx, dz),
		1)   -- moveState 1 = walk

	-- Guidance bark ("Oschon's Torch is due south...") every ~20 s of
	-- walking — say 337, wire-confirmed.
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
