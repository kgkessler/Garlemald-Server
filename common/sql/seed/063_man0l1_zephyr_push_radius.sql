-- Garlemald-Server #46 (Man0l1 "Treasures of the Main" — Zephyr Gate
-- escort): widen the SEQ_048 push trigger's circle so the escort reliably
-- fires when the player walks out of Limsa town (zone 133) into the Lower
-- La Noscea field (zone 128).
--
-- The ZEPHYR_TRIGGER (class 1090004) is an INVISIBLE push actor at
-- (-63.25, 33.15, 164.51) in zone 128, armed by the quest's
-- SetENpc(ZEPHYR_TRIGGER, QFLAG_PUSH) at SEQ_048. Migration 056 seeded it
-- with pmeteor's retail radius of 12.0. Packet-capture forensics of a live
-- SEQ_048 session confirmed the server DOES spawn it, stream it, emit its
-- pushWithCircle condition, AND enable it (SetEventStatus enabled=1,
-- "pushDefault") — yet the escort never started because the player never
-- physically entered the 12.0-unit circle.
--
-- Why 12.0 is too tight here: garlemald's 133->128 seamless boundary
-- (server_zones_seamless_boundaries id=5) drops the player at their crossing
-- position, which — depending on which boundary box they cross through —
-- lands them ~7 units (merge box) to ~21.5 units (zone1 box) from the
-- trigger. So a 12.0 circle only catches some crossing paths; on the others
-- the player exits onto an invisible trigger they can't see and walks past
-- it. A 25.0 circle covers every crossing path (max ~21.5), so stepping out
-- of the gate reliably starts the escort. The push only exists on the
-- zone-128 actor and is only enabled while the quest sits at SEQ_048, so a
-- wider circle cannot fire from inside town or for players not on this beat;
-- onPush still gates on the contentsJoinAskInBasaClass yes/no prompt.
--
-- Idempotent: re-sets the full eventConditions JSON for the one class.

UPDATE "gamedata_actor_class" SET
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
      "radius": "25.0",
      "outwards": "false",
      "silent": "false",
      "conditionName": "pushDefault"
    }
  ]
}'
WHERE "id" = 1090004;
