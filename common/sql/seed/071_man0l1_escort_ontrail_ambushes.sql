-- 071: re-home the man0l1 escort ankle-biter clusters ON the recorded
-- player trail (round 7f). Seed/070 staged them at straight-line
-- route-interpolated coordinates, which sat off walkable land — Sisipu
-- (then on a blind interpolated route) was ambushed out of bounds where
-- the player could not help, died, and failed the duty. The new centers
-- are REAL pass-through points decoded from the player's own 0x00CA
-- position trail (2026-07-03 21:38-21:47 run; offset verified against
-- the warp anchor -63.25/33.16/164.51), at ~1/4, 1/2 and 3/4 of the
-- 1093-unit walked route, matching the user's marked map spots
-- ((25,34) / (26,37) / (25,38) in zone 128 sea0Field01). Offsets keep
-- each cluster's mobs within a few units of the trail. Idempotent
-- (UPDATEs converge).
--   wave 1 (bnpc 26-28): (162.5, 49.98, 181.75) — east bend before the
--                        route turns north
--   wave 2 (bnpc 29-31): (69.91, 44.56, 405.83) — the x~70 corridor
--   wave 3 (bnpc 32-33): (131.34, 54.53, 632.71) — upper bench toward
--                        the Moraby plateau
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 162.5,  "positionY" = 49.98, "positionZ" = 181.75 WHERE "bnpcId" = 26;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 158.9,  "positionY" = 50.06, "positionZ" = 186.40 WHERE "bnpcId" = 27;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 165.2,  "positionY" = 49.80, "positionZ" = 178.10 WHERE "bnpcId" = 28;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 69.91,  "positionY" = 44.56, "positionZ" = 405.83 WHERE "bnpcId" = 29;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 66.80,  "positionY" = 44.30, "positionZ" = 410.90 WHERE "bnpcId" = 30;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 72.90,  "positionY" = 44.70, "positionZ" = 400.60 WHERE "bnpcId" = 31;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 131.34, "positionY" = 54.53, "positionZ" = 632.71 WHERE "bnpcId" = 32;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 128.60, "positionY" = 54.35, "positionZ" = 638.20 WHERE "bnpcId" = 33;
