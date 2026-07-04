require ("global")

-- ChigoeLesserStandard — class script for the chigoe monster family
-- (man0l1 escort "ankle biter", actor class 2205603 → this classPath via
-- seed 056). (Garlemald-Server #46.)
--
-- THIS FILE IS LOAD-BEARING FOR CLIENT SURVIVAL: the 0x00CC
-- ActorInstantiate's script-bind tail is built from this init()'s
-- returns (world_manager.rs `call_npc_init` → pmeteor
-- `Npc.CreateScriptBindPacket` parity). With NO class script the server
-- ships the populace-shaped fallback tail (false, false, 0, 0) —
-- numBattleCommon=0 — so the client-side npcWork.battleCommon.aggro
-- stays nil, and round-3's forced battle-nameplate bit sends
-- DepictionJudge's judgeNameplate into a fatal `0 < getAggro()`
-- nil-compare → crash to character select (wire-proven 2026-07-03, 2
-- reproductions). The tutorial jellyfish/wolves survive because their
-- class scripts return the 10-slot battleCommon tuple below.
--
-- Tuple mirrors base/chara/npc/monster/Wolf/WolfStandard.lua — the
-- aggressive standard-monster variant (position 6 `true`, vs the
-- passive-until-attacked jellyfish's `false`) — chigoes are aggressive
-- small vermin, so the wolf shape is the retail-plausible pick.
function init(npc)
	return true, true, 10, 0, 1, true, false, false, false, false, false, false, false, 0;
end
