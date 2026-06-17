-- Garlemald-Server #46 (Man0l1 "Treasures of the Main" — Zephyr Gate
-- escort duty): seed data for the SEQ_048 → SEQ_050 escort content.
--
-- Four data gaps blocked the escort end-to-end:
--
--  1. The ZEPHYR_TRIGGER actor class (1090004) is stripped in the 003
--     seed ('', 0, 0, null) — pmeteor's gamedata_actor_class.sql:2395
--     carries the real row: PopulaceStandard with a noticeEvent
--     condition + a pushWithCircleEventConditions circle (radius 12.0,
--     conditionName pushDefault, disabled until the quest's
--     SetENpc(ZEPHYR_TRIGGER, QFLAG_PUSH) arms it at SEQ_048).
--  2. No spawn row existed for it — pmeteor's
--     server_eventnpc_spawn_locations.sql:1139 places it in zone 128 at
--     (-63.25, 33.15, 164.51) with uniqueId
--     'seafld0_push_limsa_entrance' (the Zephyr Gate roadside).
--  3. Sisipu's escort-instance combatant class (2290007 — the actor
--     QuestDirectorMan0l101's dead onCreateContentArea spawned upstream)
--     is stripped in BOTH garlemald and pmeteor (upstream never wired
--     the escort either). Appearance row 2290007 exists (her Lalafell
--     look) and displayNameId 1500024 resolves "Sisipu"; the class path
--     here borrows the proven melee-ally kit the tutorial allies use
--     (FighterAllyOpeningAttacker — same convention as Y'shtola /
--     Sthalmann sharing the FighterAlly paths with their own
--     appearance rows).
--  4. The ambush mob: the 2010-09 escort recording
--     (youtube-watcher FEI03OpK99o) shows "The ankle biter is defeated"
--     kill lines; mozk-tabetai displayNameId 3205603 = "ankle biter"
--     (アンクルバイター) → actor class 2205603 by the standard monster
--     +1,000,000 convention (sibling band: 3205605 "orchard chigoe" —
--     it's a chigoe-family mob). Class row is stripped in both trees;
--     appearance row 2205603 exists. ChigoeLesserStandard is the
--     family's model path (2105612 carries it).
--
-- BNpc pools/groups/spawns follow the 054/055 tutorial shape. Spawn
-- positions are DERIVED, not captured — no surviving 1.x recording
-- renders coordinates (the HUD never drew XYZ), so the escort runs a
-- compressed road segment heading due south from the gate (Sisipu:
-- "Oschon's Torch is due south") at the gate's seeded road height;
-- the arrival beat fades out and warps to the Oschon's Torch private
-- area exactly like the previous skip path did. Flagged for in-client
-- position tuning.
--
-- UPDATEs are idempotent and only fill rows the 003 seed left stripped;
-- INSERTs are OR IGNORE. No schema change.

UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Populace/PopulaceStandard',
    "propertyFlags" = 1,
    "eventConditions" = '{
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ],
  "pushWithCircleEventConditions": [
    {
      "isEnabled": "false",
      "radius": "12.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}'
WHERE "id" = 1090004 AND "classPath" = '';

UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Monster/Fighter/FighterAllyOpeningAttacker'
WHERE "id" = 2290007 AND "classPath" = '';

UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Monster/Chigoe/ChigoeLesserStandard'
WHERE "id" = 2205603 AND "classPath" = '';

INSERT OR IGNORE INTO "server_spawn_locations"
    ("actorClassId", "uniqueId", "zoneId", "privateAreaName", "privateAreaLevel",
     "positionX", "positionY", "positionZ", "rotation", "actorState", "animationId", "customDisplayName")
VALUES
    (1090004, 'seafld0_push_limsa_entrance', 128, '', 0, -63.25, 33.15, 164.51, 0, 0, 0, null);

-- Pools: Sisipu rides the Fighter genus (29) like the tutorial allies;
-- ankle biters ride the Chigoe genus (36).
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (11, 2290007, 'sisipu', 29, 2, 1, 4200, 1, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (12, 2205603, 'ankle_biter', 36, 0, 1, 4200, 1, 0, 0, 0, 0, 0);

-- Groups: Sisipu is an ally (allegiance 1, MinimumHpLock'd in-script on
-- top of the comfortable pool); ankle biters die to 2-3 early-rank
-- weaponskills (the recording shows ~20-21 damage Heavy Strikes
-- finishing them) at the road's level band.
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (11, 11, 'sisipu', 1, 1, 0, 800, 0, 0, 1, 1, 0, 0, '', 0, 128);
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (12, 12, 'ankle_biter', 5, 7, 0, 60, 0, 0, 0, 1, 0, 0, '', 0, 128);

-- Spawn rows 16-24 (tutorials hold 3-15). Sisipu starts beside the
-- content entry point; the three ambush clusters sit on the southbound
-- road at the waypoints SimpleContentMan0l101.lua walks her through.
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (16, 'sisipu', 11, -60.0, 33.15, 170.0, 0.0);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (17, 'ankle_biter', 12, -54.0, 33.15, 214.0, -1.5);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (18, 'ankle_biter', 12, -50.0, 33.15, 219.0, -1.5);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (19, 'ankle_biter', 12, -47.0, 33.15, 222.0, -1.5);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (20, 'ankle_biter', 12, -38.0, 33.15, 258.0, -1.5);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (21, 'ankle_biter', 12, -34.0, 33.15, 263.0, -1.5);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (22, 'ankle_biter', 12, -31.0, 33.15, 267.0, -1.5);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (23, 'ankle_biter', 12, -22.0, 33.15, 302.0, -1.5);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (24, 'ankle_biter', 12, -17.0, 33.15, 308.0, -1.5);
