-- 073: man0l1 escort — minimap duty-halo ring class + lighthouse camp
-- music (Garlemald-Server #46, round 9).
--
-- (1) Ring class: fill the empty gamedata_actor_class row 1290003 with
-- the retail ContentPrivateAreaRange shape so the escort content script
-- can SpawnActor an invisible, server-moved minimap halo that rides
-- with Sisipu. The client attaches a native range map-marker to any
-- NON-talkable actor whose class script defines getMapMarkerRange()
-- (depictionjudge.lua:773-791); ContentPrivateAreaRange returns
-- "exit","caution" and the marker radii resolve from the actor's
-- wire-sent pushWithCircle event conditions of those names — the JSON
-- below copies sibling row 1290002 (PrivateAreaPastExit, seed/003)
-- verbatim. 1290003 exists client-side with graphics 0 (invisible) and
-- ships with an EMPTY classPath upstream, so the guarded UPDATE only
-- ever claims the unclaimed row; '/Chara/Npc/Object/
-- ContentPrivateAreaRange' is a retail client script (registry.json).
UPDATE "gamedata_actor_class"
SET "classPath" = '/Chara/Npc/Object/ContentPrivateAreaRange',
    "propertyFlags" = '1',
    "eventConditions" = '{
  "talkEventConditions": [],
  "noticeEventConditions": [],
  "emoteEventConditions": [],
  "pushWithCircleEventConditions": [
    {
	"conditionName": "exit",
	"radius": 60.0,
	"silent": true,
	"outwards": false
    },
    {
	"conditionName": "caution",
	"radius": 50.0,
	"silent": true,
	"outwards": false
    }
  ]
}'
WHERE "id" = 1290003 AND "classPath" = '';

-- (2) Camp music: the SEQ_055 lighthouse-camp echo destination (zone
-- 128 PrivateAreaMasterPast type 2, seed/058 row id 15) carries the
-- stale upstream dayMusic=0, so completing the escort would restore
-- SILENCE after the duty's "Daring Dalliances" override. 40 = "The
-- Echo" — the track every other PrivateAreaMasterPast echo row uses
-- (rows 11/16/17/19/22). Idempotent (UPDATE converges).
UPDATE "server_zones_privateareas" SET "dayMusic" = 40 WHERE "id" = 15 AND "dayMusic" = 0;
