-- 068_characters_aetherytes.sql
--
-- Aetheryte attunement persistence (Garlemald-Server #46, round 5). Until
-- now the set of aetherytes a character could teleport to lived only in
-- the in-memory Player helper (`unlocked_aetherytes`,
-- map-server/src/actor/player.rs), seeded solely by SetHomePoint — so a
-- relog forgot every attunement and the teleport menu gates
-- (AetheryteParent.lua doNormalMenu, TeleportCommand.lua) had nothing
-- durable to check. This table backs the new first-touch unlock
-- (AetheryteParent.lua / AetheryteChild.lua onEventStarted →
-- UnlockAetheryteNode write-through) and the TeleportCommand
-- server-authoritative destination gate. aetheryteId is the aetheryte's
-- actor class id (the 128xxxx keys of aetheryteTeleportPositions /
-- aetheryteParentLinks in scripts/lua/aetheryte.lua) — the same namespace
-- as characters.homepoint.
--
-- Back-fill: a character's homepoint is the one attunement the old code
-- path did record (set_home_point inserted it into the in-memory set), so
-- seed each existing character's homepoint aetheryte. homepoint == 0 means
-- "never set" (schema default) and is skipped; everything else self-heals
-- via the first-touch unlock. INSERT OR IGNORE + IF NOT EXISTS keep the
-- migration idempotent; no schema.sql change per the migrations rule.

CREATE TABLE IF NOT EXISTS characters_aetherytes (
    characterId INTEGER NOT NULL,
    aetheryteId INTEGER NOT NULL,
    PRIMARY KEY (characterId, aetheryteId)
);

INSERT OR IGNORE INTO characters_aetherytes (characterId, aetheryteId)
SELECT id, homepoint
FROM characters
WHERE homepoint != 0;
