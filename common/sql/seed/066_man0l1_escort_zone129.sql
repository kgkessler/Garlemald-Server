-- Garlemald-Server #46 (Man0l1 "Treasures of the Main" — Sisipu escort):
-- re-home the escort to zone 129 (Western La Noscea, 'sea0Field02').
--
-- The escort cannot run in the player's current zone (128, 'sea0Field01').
-- Its intro cutscene is delegated via processEvent604, which ends with
-- startFadeInCutSceneAfterWarp == engine `_fadeInAfterWarp()` — that raises
-- the "Now Loading" overlay and the 1.x client tears it down ONLY when a
-- real off-disk map load COMPLETES (warp-END handler). So the cutscene MUST
-- be followed by a warp into a GENUINELY DIFFERENT map resource. Every
-- same-map attempt hung forever:
--   * 065 in-place reveal (no warp)                         -> no load -> hang
--   * spawnType 7 teleport respawn / 0x16 in-place          -> no load -> hang
--   * 064 zone 141 'sea0Field01a' (aliases 128's resource)  -> no load -> hang
--   * 128->128 force-reload (same 'sea0Field01' resource)   -> no load -> hang
-- (exhaustively bisected; captures/issue28-rca/04-decomp-unlock.md).
--
-- Zone 129 'sea0Field02' is the only different-map zone in region 101, so the
-- escort now runs as a content instance pinned to 129 (man0l1.lua passes 129
-- as the 6th CreateContentArea arg). DoZoneChangeContent then migrates the
-- player 128->129; SetMap carries 129's different resource + the 0x00E2(0x10)
-- force-reload latch (needed because 128->129 is same-REGION) schedules the
-- load -> it completes -> the cutscene veil resolves AND the command-inhibit
-- latch clears (mode 0x10 != 0x16). Result: cutscene -> Now Loading ->
-- game-world, with menu/map/weaponskills live on arrival.
--
-- Coordinates: the escort BattleNpcs are clustered on the zone-129 Camp Skull
-- Valley standing ground (warp-in at the aetheryte -991.88/61.71/-1120.79;
-- the camp NPCs sit at y~61.5, z -1109..-1133), so they ride the zone-in
-- bundle's 50-yalm actors_around reveal and stand on solid floor. Y is the
-- camp floor (~61.5); flag for in-client tuning if any float/bury.
--
-- The spawn loader (database.rs load_battle_npc_spawn) keys on bnpcId and the
-- spawn zone comes from the content area's parent zone, so groupId.zoneId is
-- metadata only — but keep it agreeing with where the escort runs (129).
--
-- Idempotent UPDATEs; no schema change. Supersedes 064 (141) and 065 (128).

UPDATE "server_battlenpc_groups" SET "zoneId" = 129 WHERE "groupId" IN (11, 12);

-- Sisipu (escort target) beside the warp-in point.
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -992.0, "positionY" = 61.6, "positionZ" = -1116.0, "rotation" = 0.0 WHERE "bnpcId" = 16;
-- Eight ankle biters clustered on the Skull Valley floor within ~15 yalms of
-- the warp-in so they render immediately and are fightable.
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -986.0, "positionY" = 61.5, "positionZ" = -1111.0, "rotation" = -1.5 WHERE "bnpcId" = 17;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -998.0, "positionY" = 61.5, "positionZ" = -1113.0, "rotation" = -1.5 WHERE "bnpcId" = 18;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -984.0, "positionY" = 61.5, "positionZ" = -1119.0, "rotation" = -1.5 WHERE "bnpcId" = 19;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -1000.0, "positionY" = 61.5, "positionZ" = -1121.0, "rotation" = -1.5 WHERE "bnpcId" = 20;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -987.0, "positionY" = 61.5, "positionZ" = -1126.0, "rotation" = -1.5 WHERE "bnpcId" = 21;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -997.0, "positionY" = 61.5, "positionZ" = -1128.0, "rotation" = -1.5 WHERE "bnpcId" = 22;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -991.0, "positionY" = 61.5, "positionZ" = -1131.0, "rotation" = -1.5 WHERE "bnpcId" = 23;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -994.0, "positionY" = 61.5, "positionZ" = -1109.0, "rotation" = -1.5 WHERE "bnpcId" = 24;
