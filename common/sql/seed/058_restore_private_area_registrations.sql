-- 058_restore_private_area_registrations.sql
--
-- garlemald's server_zones_privateareas seed (034) was ported from the
-- older project-meteor-mirror revision and is missing private-area
-- registrations the project-meteor-server quest scripts warp into via
-- GetWorldManager():WarpToPrivateArea(player, name, type). Without the
-- (parentZone, name, type) row the destination area does not exist, so the
-- warp lands nowhere and the client hangs on "Now Loading" (e.g. man0l1
-- SEQ_007 second Isandorel cutscene -> PrivateAreaMasterPast type 3 in zone
-- 230). Rows copied from project-meteor-server's
-- Data/sql/server_zones_privateareas.sql; pmeteor's (canExitArea, music)
-- map to garlemald's (dayMusic=music, nightMusic=0, battleMusic=0) — the
-- same mapping the 034 seed used. Idempotent via INSERT OR IGNORE on id.
-- (Garlemald-Server #46.)

INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (11, 230, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 3, 40, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (12, 230, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 4, 40, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (13, 209, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 5, 66, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (14, 230, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 5, 37, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (15, 128, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 2, 0, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (16, 133, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 3, 40, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (17, 230, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 6, 40, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (18, 230, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 7, 42, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (19, 230, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 8, 40, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (20, 192, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 0, 42, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (21, 192, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 1, 9, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (22, 128, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 3, 40, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (23, 209, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 5, 66, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (24, 206, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 5, 51, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (25, 128, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 5, 53, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (26, 181, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 5, 61, 0, 0);
INSERT OR IGNORE INTO "server_zones_privateareas" ("id","parentZoneId","className","privateAreaName","privateAreaType","dayMusic","nightMusic","battleMusic") VALUES (27, 172, '/Area/PrivateArea/PrivateAreaMasterPast', 'PrivateAreaMasterPast', 5, 72, 0, 0);
