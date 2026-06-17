-- 057_fix_push_trigger_event_conditions.sql
--
-- Restore the missing push event conditions on quest "trigger" actor
-- classes. garlemald's gamedata_actor_class seed (003) was ported from
-- project-meteor-mirror, an older revision whose eventConditions for these
-- classes lack the pushWithCircleEventConditions array (and, for several
-- 109xxxx triggers, carry a spurious talkDefault instead, or are NULL). The
-- quest scripts in scripts/lua/quests/ were ported from project-meteor-server
-- (Ioncannon), which defines these same classes WITH the push circle and
-- drives them via quest:SetENpc(<TRIGGER>, QFLAG_PUSH, false, true). Without
-- the circle the client never receives a SetPushEventConditionWithCircle
-- (0x016F) geometry packet, so the push can never fire (the actor streams in
-- but has no circle to test the player against). Symptom: every "walk into
-- the trigger" beat silently does nothing -- e.g. man0l1 "Treasures of the
-- Main" SEQ_007, the Musketeers' Guild "go downstairs" push (class 1090001).
-- (Garlemald-Server #46.)
--
-- Values copied verbatim from project-meteor-server's
-- Data/sql/gamedata_actor_class.sql (the canonical source the quest scripts
-- target). Only the eventConditions column is touched.
--
-- The canonical push conditions ship "isEnabled": "false" (or omit it, which
-- the parser coerces to false), so a streamed trigger arrives DISABLED and
-- only fires once its owning quest enables it -- the map-server streaming /
-- zone-in paths now honour that per-condition default and each player's quest
-- ENPC push state (see build_actor_event_status_packets + push_npc_spawn).
-- That makes restoring the circle safe even for triggers that sit next to the
-- player while still disabled (e.g. man0l1 ECHO_EXIT, which would otherwise
-- warp the player out) and for the lone talk NPC in the set (1000008
-- MERODAULYN, driven talk-only by man1l0 -- its push stays disabled).
--
-- Idempotent: re-running the UPDATEs is a no-op once the rows hold the JSON.

-- 1000008  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 1,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 1,
      "unknown2": 0,
      "conditionName": "noticeEvent"
    }
  ],
  "emoteEventConditions": [],
  "pushWithCircleEventConditions": [
    {
      "conditionName": "pushDefault",
      "radius": 3.0,
      "silent": false,
      "outwards": false
    }
  ]
}' WHERE "id" = 1000008;

-- 1090001  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090001;

-- 1090003  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090003;

-- 1090006  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090006;

-- 1090007  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "10.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090007;

-- 1090008  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090008;

-- 1090009  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090009;

-- 1090042  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090042;

-- 1090058  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090058;

-- 1090080  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090080;

-- 1090081  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "1",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090081;

-- 1090082  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090082;

-- 1090083  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090083;

-- 1090084  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090084;

-- 1090085  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090085;

-- 1090086  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090086;

-- 1090087  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090087;

-- 1090088  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090088;

-- 1090089  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090089;

-- 1090090  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090090;

-- 1090091  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "2",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090091;

-- 1090092  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090092;

-- 1090098  /Chara/Npc/Object/ObjectEventDoor  (door)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "2.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090098;

-- 1090099  /Chara/Npc/Object/ObjectEventDoor  (door)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "2.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090099;

-- 1090159  /Chara/Npc/Object/ObjectEventDoor  (door)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090159;

-- 1090160  /Chara/Npc/Object/ObjectEventDoor  (door)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "3.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090160;

-- 1090161  /Chara/Npc/Object/ObjectEventDoor  (door)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "3.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090161;

-- 1090162  /Chara/Npc/Object/ObjectEventDoor  (door)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "3.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090162;

-- 1090386  /Chara/Npc/Populace/PopulaceStandard  (populace/trigger)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
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
      "radius": "6.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}' WHERE "id" = 1090386;

-- 1290004  /Chara/Npc/Object/BgKeepout  (keepout)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ],
  "pushWithCircleEventConditions": [
    {
      "conditionName": "caution",
      "radius": 2.0,
      "silent": true,
      "outwards": false
    }
  ]
}' WHERE "id" = 1290004;

-- 1290022  /Chara/Npc/Object/ElevatorStandard  (elevator)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [],
  "noticeEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "pushCommand"
    },
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ],
  "pushWithCircleEventConditions": [
    {
      "radius": "2.0",
      "outwards": "false",
      "silent": "true",
      "conditionName": "pushCommandIn"
    },
    {
      "radius": "2.0",
      "outwards": "true",
      "silent": "true",
      "conditionName": "pushCommandOut"
    }
  ]
}' WHERE "id" = 1290022;

-- 1290033  /Chara/Npc/Object/BgKeepout  (keepout)
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ],
  "pushWithCircleEventConditions": [
    {
      "conditionName": "caution",
      "radius": 2.0,
      "silent": true,
      "outwards": false
    }
  ]
}' WHERE "id" = 1290033;

