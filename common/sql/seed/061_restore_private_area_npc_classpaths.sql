-- 061_restore_private_area_npc_classpaths.sql
--
-- The Past/Echo NPC classes that populate the private areas restored in 059
-- have an EMPTY classPath (and propertyFlags=0) in garlemald's stale
-- project-meteor-mirror gamedata_actor_class seed, where project-meteor-server
-- gives them a real class (/Chara/Npc/Populace/PopulaceStandard etc.) + the
-- 0x13 populace property flags. An actor spawned with no class path cannot be
-- instantiated client-side: the 1.x client HARD-CRASHES (Wine dies) on the
-- zone-in into that private area. This never fired before because the warp
-- into these areas was itself broken (see 057-060 + the WarpToPrivateArea
-- runtime-drain fix) -- man0l1 SEQ_035->040 warps into the Fisherman's Guild
-- (PrivateAreaMasterPast type 5, zone 230) where class 1000155 (SISIPU_EMOTE)
-- is the empty-classPath actor that crashed the client. Restores classPath /
-- displayNameId / propertyFlags verbatim from project-meteor-server for every
-- such SPAWNED class (eventConditions are left to 057/060). Idempotent.
-- (Garlemald-Server #46.)

UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1200028, "propertyFlags"=19 WHERE "id"=1000008;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000016, "propertyFlags"=19 WHERE "id"=1000091;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000017, "propertyFlags"=19 WHERE "id"=1000092;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000020, "propertyFlags"=19 WHERE "id"=1000096;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000021, "propertyFlags"=19 WHERE "id"=1000097;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000023, "propertyFlags"=19 WHERE "id"=1000101;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000022, "propertyFlags"=19 WHERE "id"=1000102;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000024, "propertyFlags"=19 WHERE "id"=1000103;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000025, "propertyFlags"=19 WHERE "id"=1000104;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000026, "propertyFlags"=19 WHERE "id"=1000105;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000028, "propertyFlags"=19 WHERE "id"=1000107;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000029, "propertyFlags"=19 WHERE "id"=1000108;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000030, "propertyFlags"=19 WHERE "id"=1000109;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000031, "propertyFlags"=19 WHERE "id"=1000110;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000035, "propertyFlags"=19 WHERE "id"=1000111;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000032, "propertyFlags"=19 WHERE "id"=1000112;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000033, "propertyFlags"=19 WHERE "id"=1000113;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000038, "propertyFlags"=19 WHERE "id"=1000115;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000040, "propertyFlags"=19 WHERE "id"=1000117;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000041, "propertyFlags"=19 WHERE "id"=1000118;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000042, "propertyFlags"=19 WHERE "id"=1000119;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000003, "propertyFlags"=19 WHERE "id"=1000120;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000003, "propertyFlags"=19 WHERE "id"=1000121;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1600112, "propertyFlags"=19 WHERE "id"=1000142;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1900029, "propertyFlags"=19 WHERE "id"=1000145;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1500024, "propertyFlags"=19 WHERE "id"=1000155;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1500024, "propertyFlags"=19 WHERE "id"=1000156;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1400012, "propertyFlags"=19 WHERE "id"=1000176;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000117, "propertyFlags"=19 WHERE "id"=1000182;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000001, "propertyFlags"=19 WHERE "id"=1000183;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000001, "propertyFlags"=19 WHERE "id"=1000184;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1000029, "propertyFlags"=19 WHERE "id"=1000238;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1100025, "propertyFlags"=19 WHERE "id"=1000239;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1000028, "propertyFlags"=19 WHERE "id"=1000247;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000051, "propertyFlags"=19 WHERE "id"=1000378;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1300028, "propertyFlags"=19 WHERE "id"=1000410;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1100294, "propertyFlags"=19 WHERE "id"=1000411;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1000414, "propertyFlags"=19 WHERE "id"=1000412;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000003, "propertyFlags"=19 WHERE "id"=1000452;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000003, "propertyFlags"=19 WHERE "id"=1000453;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000015, "propertyFlags"=19 WHERE "id"=1000868;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000099, "propertyFlags"=19 WHERE "id"=1000869;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000099, "propertyFlags"=19 WHERE "id"=1000870;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000099, "propertyFlags"=19 WHERE "id"=1000871;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000335, "propertyFlags"=19 WHERE "id"=1000952;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000336, "propertyFlags"=19 WHERE "id"=1000953;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1000104, "propertyFlags"=19 WHERE "id"=1000954;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1000052, "propertyFlags"=19 WHERE "id"=1001903;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=2200152, "propertyFlags"=19 WHERE "id"=1001995;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1900226, "propertyFlags"=19 WHERE "id"=1001996;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1000117, "propertyFlags"=19 WHERE "id"=1002065;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1000026, "propertyFlags"=19 WHERE "id"=1002066;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=2200152, "propertyFlags"=19 WHERE "id"=1002067;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1200068, "propertyFlags"=19 WHERE "id"=1002071;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=1000281, "propertyFlags"=19 WHERE "id"=1002114;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=1 WHERE "id"=1080056;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=1 WHERE "id"=1080057;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=1 WHERE "id"=1080058;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=3 WHERE "id"=1080090;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=3 WHERE "id"=1080091;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=3 WHERE "id"=1080092;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=3 WHERE "id"=1080093;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=3 WHERE "id"=1080094;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=1 WHERE "id"=1090080;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=1 WHERE "id"=1090081;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=1 WHERE "id"=1090083;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=1 WHERE "id"=1090084;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=1 WHERE "id"=1090085;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=1 WHERE "id"=1090087;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=0, "propertyFlags"=1 WHERE "id"=1090089;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Object/ObjectEventDoor', "displayNameId"=0, "propertyFlags"=1 WHERE "id"=1090098;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Object/ObjectEventDoor', "displayNameId"=0, "propertyFlags"=1 WHERE "id"=1090099;
UPDATE "gamedata_actor_class" SET "classPath"='/Chara/Npc/Populace/PopulaceStandard', "displayNameId"=4000257, "propertyFlags"=19 WHERE "id"=1200412;
