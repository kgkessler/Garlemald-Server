-- 060_restore_man0l1_npc_event_conditions.sql
--
-- man0l1 "Treasures of the Main" cast members whose gamedata_actor_class
-- eventConditions are NULL in garlemald's stale mirror seed (003) but defined
-- in project-meteor-server. Without a talkEventCondition the 1.x client never
-- fires a talk EventStart, so these NPCs are mute -- e.g. the PrivateAreaMaster
-- Past type-3 Echo cast (barracudas / Mannskoen / adventurers) the player meets
-- after the second Isandorel cutscene, and the later corpse / witness NPCs.
-- Restores each class's eventConditions verbatim from project-meteor-server
-- (normalized to strict JSON). Additive (rows are currently NULL). Idempotent.
-- (Garlemald-Server #46.)

-- 1000091  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000091;

-- 1000092  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000092;

-- 1000096  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000096;

-- 1000097  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000097;

-- 1000107  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000107;

-- 1000108  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000108;

-- 1000109  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000109;

-- 1000111  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000111;

-- 1000142  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000142;

-- 1000145  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000145;

-- 1000155  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ],
  "emoteEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "emoteId": 105,
      "conditionName": "emoteDefault1"
    },
    {
      "unknown1": 4,
      "unknown2": 0,
      "emoteId": 107,
      "conditionName": "emoteDefault2"
    },
    {
      "unknown1": 4,
      "unknown2": 0,
      "emoteId": 129,
      "conditionName": "emoteDefault3"
    },
    {
      "unknown1": 4,
      "unknown2": 0,
      "emoteId": 128,
      "conditionName": "emoteDefault4"
    },
    {
      "unknown1": 4,
      "unknown2": 0,
      "emoteId": 118,
      "conditionName": "emoteDefault5"
    },
    {
      "unknown1": 4,
      "unknown2": 0,
      "emoteId": 116,
      "conditionName": "emoteDefault6"
    }
  ]
}' WHERE "id" = 1000155;

-- 1000156  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000156;

-- 1000176  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000176;

-- 1000247  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000247;

-- 1000378  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000378;

-- 1000869  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000869;

-- 1000870  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000870;

-- 1000871  /Chara/Npc/Populace/PopulaceStandard
UPDATE "gamedata_actor_class" SET "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}' WHERE "id" = 1000871;

