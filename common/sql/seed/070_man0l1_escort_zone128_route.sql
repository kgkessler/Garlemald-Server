-- Garlemald-Server #46 (Man0l1 "Treasures of the Main" — Sisipu escort):
-- re-home the escort to ZONE 128 (retail geography — the Zephyr Gate →
-- Oschon's Torch road) now that the same-map content reload is solved.
--
-- History: 066 pinned the escort to zone 129 (Skull Valley) because the
-- processEvent604 after-warp veil demanded a genuinely different map
-- resource. That constraint is GONE: the wipe + 0x00E2(0x10) recipe
-- (DeleteAllActors + force-reload latch + immediate bundle, emitted by
-- apply_do_zone_change_content) completes same-map content reloads —
-- capture-proven on the 193→193 / 184→184 tutorial warps — and
-- processEvent604_3 (startFadeInCutSceneDefault) neutralises the armed
-- veil in place BEFORE the warp (proven load-bearing in the escort
-- flow). So the escort returns to retail's zone 128, per pmeteor's
-- intended (dead-code) shape: CreateContentArea on the SAME MAP at the
-- gate, Sisipu spawned at (-49.0, 36.43, 162.0, rot 2.2),
-- DoZoneChangeContent to (-63.25, 33.15, 164.51, 0.8, spawn 16).
--
-- Shape: NEW spawn rows 25-33 rather than UPDATEs of 066's 16-24. The
-- spawn loader (database.rs load_battle_npc_spawn) keys on bnpcId and
-- SimpleContentMan0l101.lua names the ids it spawns, so adding rows and
-- flipping the script is zero-risk to the 129 machinery — rows 16-24
-- keep their Skull Valley coordinates intact for rollback.
-- ROLLBACK: in scripts/lua, flip SimpleContentMan0l101.lua back to
-- spawning 16..24 and man0l1.lua's CreateContentArea zone pin 128 → 129
-- (one line each; see the paired comments there).
--
-- Coordinates are ROUTE-INTERPOLATED, not captured: no surviving 1.x
-- recording renders XYZ (the HUD never drew them), so the three ambush
-- clusters sit on the straight line gate (-63, 33, 164) → lighthouse
-- camp approach (137.44, 60.33, 1322.0 — pmeteor's SEQ_055 camp warp
-- destination) at z≈450 / z≈800 / z≈1100, with x and y tracking that
-- line (x(z) = -49 + 186.44·(z-162)/1160, y(z) = 36.43 + 23.9·(z-162)/1160).
-- Exact retail ambush positions are unknowable — flag every row for
-- in-client tuning (floaters/buried mobs are y errors; off-road mobs
-- are x errors).
--
-- groupId.zoneId is metadata only (the live spawn zone comes from the
-- content area's parent zone) but is kept agreeing with where the
-- escort runs.
--
-- Idempotent: UPDATEs converge, INSERTs are OR IGNORE. No schema change.

UPDATE "server_battlenpc_groups" SET "zoneId" = 128 WHERE "groupId" IN (11, 12);

-- Sisipu (escort target) — pmeteor QuestDirectorMan0l101's intended
-- SpawnActor coordinates, beside the gate-side warp-in point.
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (25, 'sisipu', 11, -49.0, 36.43, 162.0, 2.2);

-- Wave 1 — first ambush cluster, route point z≈450 (3 chigoes).
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (26, 'ankle_biter', 12, -8.0, 42.4, 446.0, -1.5);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (27, 'ankle_biter', 12, -2.7, 42.4, 452.0, -1.5);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (28, 'ankle_biter', 12, 3.0, 42.4, 448.0, -1.5);

-- Wave 2 — second ambush cluster, route point z≈800 (3 chigoes).
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (29, 'ankle_biter', 12, 48.0, 49.6, 796.0, -1.5);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (30, 'ankle_biter', 12, 53.6, 49.6, 803.0, -1.5);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (31, 'ankle_biter', 12, 59.0, 49.6, 798.0, -1.5);

-- Wave 3 — final ambush cluster, route point z≈1100 (2 chigoes).
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (32, 'ankle_biter', 12, 97.0, 55.8, 1096.0, -1.5);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (33, 'ankle_biter', 12, 106.0, 55.8, 1104.0, -1.5);
