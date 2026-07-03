-- 067_repair_main_skill_level.sql
--
-- One-time repair for stale characters_parametersave.mainSkillLevel. The
-- lobby reads mainSkillLevel for the character-select level display
-- (lobby-server/src/database.rs), but garlemald's AddExp level-up path
-- persisted new levels only to characters_class_levels (Database::set_level)
-- — the ported save_player_current_class (pmeteor Map Server/Database.cs:374,
-- called from Player.cs:2965 inside the AddExp level-up branch) had no
-- callers, so mainSkillLevel stayed at its creation-time value (1) forever
-- and character select showed every character at level 1. The map-server now
-- calls save_player_current_class on active-class level-ups; this migration
-- back-fills characters that leveled before that fix by copying the level of
-- each row's mainSkill out of characters_class_levels.
--
-- mainSkill → column mapping mirrors map-server/src/gamedata.rs
-- class_column (CLASSID_PUG = 2 … CLASSID_FSH = 41, per C#
-- CharacterUtils.GetClassNameForId), covering all 18 columns on
-- characters_class_levels. Guards:
--   * NULLIF(…, 0) — a 0 in characters_class_levels means "class never
--     started" (creation inserts the default-0 row and bumps only the
--     starting class to 1, lobby-server/src/database.rs:205-210), so a
--     mainSkill pointing at an unbumped column keeps the existing value
--     rather than downgrading the char-select level to 0.
--   * COALESCE(…, mainSkillLevel) — unmapped mainSkill ids (CASE without
--     ELSE yields NULL) and characters missing a characters_class_levels
--     row are no-ops.
-- UPDATE-only, idempotent; no schema.sql change per the migrations rule.

UPDATE characters_parametersave
SET mainSkillLevel = COALESCE(
    NULLIF(
        (SELECT CASE characters_parametersave.mainSkill
             WHEN 2  THEN cl.pug
             WHEN 3  THEN cl.gla
             WHEN 4  THEN cl.mrd
             WHEN 7  THEN cl.arc
             WHEN 8  THEN cl.lnc
             WHEN 22 THEN cl.thm
             WHEN 23 THEN cl.cnj
             WHEN 29 THEN cl.crp
             WHEN 30 THEN cl.bsm
             WHEN 31 THEN cl.arm
             WHEN 32 THEN cl.gsm
             WHEN 33 THEN cl.ltw
             WHEN 34 THEN cl.wvr
             WHEN 35 THEN cl.alc
             WHEN 36 THEN cl.cul
             WHEN 39 THEN cl."min"
             WHEN 40 THEN cl.btn
             WHEN 41 THEN cl.fsh
         END
         FROM characters_class_levels AS cl
         WHERE cl.characterId = characters_parametersave.characterId),
        0),
    mainSkillLevel
);
