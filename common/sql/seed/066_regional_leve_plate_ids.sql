-- Populate `gamedata_regional_leves.plateId` for the battlecraft
-- scaffold leves so the levemete NPC UI (PopulaceGuildlevePublisher.lua)
-- can map a clicked guildleve card back to a catalog leve id and drive
-- `player:AcceptRegionalLeve` / `player:HandInRegionalLeve`.
--
-- The `plateId` column already exists (schema.sql + seed 048's CREATE
-- TABLE, DEFAULT 0); this is a DATA migration only — no schema change,
-- so it is NOT mirrored in schema.sql (UPDATE-only seeds have precedent
-- in 052_fix_tutorial_ally_pools.sql).
--
-- ===================================================================
-- PROVISIONAL card -> leve mapping.
-- ===================================================================
-- PopulaceGuildlevePublisher.lua renders a hardcoded battlecraft card
-- grid in its `eventTalkCard` call:
--   cards = {0x30C3, 0x30C4, 0x30C1, 0x30C5, 0x30C6, 0x30C7, 0x30C8, 0x30C9}
-- We map the three seeded battlecraft leves (140_001..140_003) onto the
-- first three of those cards so the accept loop is exercisable end to
-- end. This card<->leve assignment is a PLACEHOLDER pending real 1.x
-- guildleve plate-table data (the canonical guildleve-plate dat maps
-- each plate id to a specific issued leve); swap these in once that
-- table is mined. Card ids are written in decimal with the source hex
-- alongside.
--
-- The fieldcraft scaffold leves (130_001..130_003) are intentionally
-- left at plateId 0: the script's fieldcraft menu branches (menuChoice
-- 0x15/0x16/0x17) do not render a card grid yet, so there is no card id
-- to map them to. They resolve via the item-target index instead.

UPDATE "gamedata_regional_leves" SET "plateId" = 12483 WHERE "id" = 140001; -- 0x30C3
UPDATE "gamedata_regional_leves" SET "plateId" = 12484 WHERE "id" = 140002; -- 0x30C4
UPDATE "gamedata_regional_leves" SET "plateId" = 12481 WHERE "id" = 140003; -- 0x30C1
