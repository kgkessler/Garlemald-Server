-- 062_quest_scenario_counter4.sql
--
-- Add the 4th quest counter column. pmeteor's QuestData carries counter1..4
-- (IncCounter `case 0..3`); garlemald's schema + runtime only had three, so a
-- quest reading/incrementing counter index 3 silently no-op'd. man0l1 SEQ_040
-- (the Fisherman's-Guild hand-signal test) keys on CNTR_SEQ40_FSH = index 3 —
-- the bow/clap/… counter never advanced and Sisipu never moved to the next
-- signal. (Garlemald-Server #46.)
--
-- ALTER ADD COLUMN only (not also in schema.sql) — SQLite ALTER has no
-- IF NOT EXISTS, so duplicating the column there would break this migration's
-- transaction on a fresh DB. Existing rows default counter4 to 0.

ALTER TABLE "characters_quest_scenario" ADD COLUMN "counter4" INTEGER NOT NULL DEFAULT 0;
