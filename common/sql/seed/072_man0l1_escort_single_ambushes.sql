-- 072: re-home the man0l1 escort ankle biters ONE PER AMBUSH POINT
-- (round 8). Player screenshots show ankle biters spawn ONE per
-- location, not in 2-3-mob clusters — the seed/070/071 three-cluster
-- staging was a guess. The eight biters (bnpc 26-33) now sit at eight
-- single on-trail ambush points at even ninths of the player's FULL
-- recorded walk (2026-07-04 01:09-01:17 run's 0x00CA position trail;
-- offset verified against the warp anchor -63.25/33.16/164.51, ending
-- at the lighthouse arrival 136.44/60.70/1324.01 — a 1720-unit arc), so
-- every point is REAL walked ground with true terrain Y and the
-- escort's ENGAGE_RADIUS activates exactly one biter per stretch.
-- Idempotent (UPDATEs converge).
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 112.30, "positionY" = 47.20, "positionZ" = 148.17  WHERE "bnpcId" = 26;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 129.15, "positionY" = 44.25, "positionZ" = 291.87  WHERE "bnpcId" = 27;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 69.96,  "positionY" = 41.22, "positionZ" = 449.76  WHERE "bnpcId" = 28;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 134.69, "positionY" = 52.41, "positionZ" = 597.19  WHERE "bnpcId" = 29;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 74.46,  "positionY" = 61.65, "positionZ" = 741.17  WHERE "bnpcId" = 30;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = -2.35,  "positionY" = 53.43, "positionZ" = 878.18  WHERE "bnpcId" = 31;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 48.45,  "positionY" = 43.43, "positionZ" = 1038.45 WHERE "bnpcId" = 32;
UPDATE "server_battlenpc_spawn_locations" SET "positionX" = 124.43, "positionY" = 46.40, "positionZ" = 1205.66 WHERE "bnpcId" = 33;
