require ("global")
require ("quest")
require ("tutorial")

--[[

Quest Script

Name: 	Treasures of the Main
Code: 	Man0l1
Id: 	110002
Prereq: Shapeless Melody (Man0l0 - 110001)

]]

-- Sequence Numbers
SEQ_000	= 0;  	-- (Private Area) Drowning Wench Echo Scene.
SEQ_003	= 3;  	-- Go attune to Camp Bearded Rock.
SEQ_005	= 5;  	-- Attuned, go back to Baderon. Info: <param1> If 1, Baderon gave you a tutorial guildleve else 0.
SEQ_006	= 6;  	-- Talk to Baderon again
SEQ_007	= 7;  	-- Find the CUL and MSK Guilds. Info: Params '0,5,20' will show the msg that you visited both guilds and to notify Baderon on the LS.
SEQ_035	= 35;	-- Go to the FSH Guild.
SEQ_040	= 40;	-- Learn hand signals from the guild
SEQ_048	= 48;	-- Travel to Zephyr Gate
SEQ_050	= 50;	-- Escort mission
SEQ_055	= 55;	-- Search lighthouse for corpse
SEQ_060	= 60;	-- Talk to Sisipu
SEQ_065	= 65;	-- Return to FSH Guild
SEQ_070	= 70;	-- Contact Baderon on LS
SEQ_075	= 75;	-- Go to the ARM and BSM Guilds. Talk to Bodenolf.
SEQ_080	= 80;	-- Speak with H'naanza
SEQ_085	= 85;	-- Walk into push trigger
SEQ_090	= 90;	-- Contact Baderon on LS
SEQ_092	= 92;	-- Return to Baderon.

-- Actor Class Ids
-- Echo in Adv Guild
YSHTOLA 				= 1000001;
CRAPULOUS_ADVENTURER 	= 1000075;
DUPLICITOUS_TRADER	 	= 1000076;
DEBONAIR_PIRATE 		= 1000077;
ONYXHAIRED_ADVENTURER	= 1000098;
SKITTISH_ADVENTURER		= 1000099;
RELAXING_ADVENTURER 	= 1000100;
BADERON 				= 1000137;
MYTESYN 				= 1000167;
COCKAHOOP_COCKSWAIN 	= 1001643;
SENTENIOUS_SELLSWORD 	= 1001649;
SOLICITOUS_SELLSWORD 	= 1001650;

-- Sequence 003
BEARDEDROCK_AETHERYTE	= 1280002;

-- Sequence 007
CHARLYS					= 1000138;
ISANDOREL				= 1000152;
MERLZIRN				= 1000472;
MSK_TRIGGER				= 1090001;

-- Echo in MSK Guild
NERVOUS_BARRACUDA		= 1000096;
INTIMIDATING_BARRACUDA	= 1000097;
OVEREAGER_BARRACUDA		= 1000107;
SOPHISTICATED_BARRACUDA	= 1000108;
SMIRKING_BARRACUDA		= 1000109;
MANNSKOEN				= 1000142;
TOTORUTO				= 1000161;
ADVENTURER1				= 1000869;
ADVENTURER2				= 1000870;
ADVENTURER3				= 1000871;
ECHO_EXIT_TRIGGER		= 1090003;

-- Fsh Guild
NNMULIKA				= 1000153;
SISIPU_EMOTE			= 1000155;
ZEPHYR_TRIGGER			= 1090004;

-- Sequence 055, 060, 065
SISIPU					= 1000156;
WINDWORN_CORPSE			= 1000091;
GLASSYEYED_CORPSE		= 1000092;
FEARSTRICKEN_CORPSE		= 1000378;
FSH_TRIGGER				= 1090006;

-- Echo in the Bsm Guild
TATTOOED_PIRATE			= 1000111;
IOFA					= 1000135;
BODENOLF				= 1000144;
HNAANZA					= 1000145;
MIMIDOA					= 1000176;
JOELLAUT				= 1000163;
WERNER					= 1000247;
HIHINE					= 1000267;
TRINNE					= 1000268;
ECHO_EXIT_TRIGGER2		= 1090007;

-- Quest Markers
--
-- Marker ids select rows in the client's quest_marker sheet (see
-- pmeteor RequestQuestJournalCommand.lua: "Check the quest sheet and
-- quest_marker sheet for valid entries for your quest"). The sibling
-- quests follow a questId-suffix block convention (man0u0/110009 →
-- 1100090x) EXCEPT the Limsa pair, which is swapped: man0l0 (110001)
-- ships with the 1100020x block, so man0l1 (110002) takes 1100010x.
-- The sheet lives client-side only — if the markers render in the
-- wrong spots (or not at all), flip MRKR_BASE to 11000200, the only
-- other candidate block. Indices are in quest beat order, the same
-- ordering every sibling block uses. (Garlemald-Server #46)
MRKR_BASE			= 11000100;
MRKR_BADERON		= MRKR_BASE + 1;	-- SEQ_000/005/006/092: Baderon at the Drowning Wench
MRKR_BEARDED_ROCK	= MRKR_BASE + 2;	-- SEQ_003: Camp Bearded Rock aetheryte
MRKR_CUL_GUILD		= MRKR_BASE + 3;	-- SEQ_007: Charlys at the Bismarck
MRKR_MSK_GUILD		= MRKR_BASE + 4;	-- SEQ_007: Isandorel at Naldiq & Vymelli's
MRKR_FSH_GUILD		= MRKR_BASE + 5;	-- SEQ_035/040/065: Nnmulika at the Fishermen's Guild
MRKR_ZEPHYR_GATE	= MRKR_BASE + 6;	-- SEQ_048: Zephyr Gate escort muster
MRKR_LIGHTHOUSE		= MRKR_BASE + 7;	-- SEQ_055: the corpse at Oschon's Torch
MRKR_SISIPU			= MRKR_BASE + 8;	-- SEQ_060: Sisipu at the lighthouse
MRKR_ARM_BSM_GUILD	= MRKR_BASE + 9;	-- SEQ_075: Bodenolf at Naldiq & Vymelli's forge
MRKR_HNAANZA		= MRKR_BASE + 10;	-- SEQ_080: H'naanza in the forge interior
MRKR_GUILD_EXIT		= MRKR_BASE + 11;	-- SEQ_085: the Echo exit trigger

-- Quest Data
CNTR_SEQ7_CUL		= 1;
CNTR_SEQ7_MSK		= 2;
CNTR_SEQ40_FSH		= 3;
CNTR_LS_MSG			= 4;

-- Quest Flags
-- Latched by the SEQ_007 ECHO_EXIT push chain right before its
-- WarpToPublicArea. A save sitting at subseqMSK>=4 WITHOUT this flag
-- relogged mid-chain (the pre-neutralizer processEvent060 veil hang
-- forced exactly that kill-and-relog) and never got the exit warp —
-- the onStateChange rescue below re-issues it once. Lua has no
-- private-area probe (PlayerSnapshot carries zone_id only), so this
-- flag is the "already exited the MSK Echo" signal instead.
-- (Garlemald-Server #46.)
FLAG_SEQ7_MSK_EXITED	= 0;
-- One-shot handoff latch for the SEQ_050 escort start: set by
-- startMan0l1Content IMMEDIATELY BEFORE StartSequence(SEQ_050), and
-- CONSUMED (ClearFlag) by the very next onStateChange run — the one
-- that StartSequence itself fires. Any LATER onStateChange run at
-- SEQ_050 (the processor's relog re-arm, or a wedged pre-escort save
-- like the one stranded at SEQ_050 today) therefore sees the flag
-- CLEAR and takes the rescue arm: roll back to SEQ_048 so the
-- ZEPHYR_TRIGGER re-arms for a retry. See onStateChange.
-- (Garlemald-Server #46 — escort leg.)
FLAG_ESCORT_HANDOFF		= 1;

-- ===== Escort duty system messages (Garlemald-Server #46) =====
-- 34108 "You have entered an instance." is CONFIRMED (worldMaster
-- sheet; EchoGate sends it right before its DoZoneChangeContent wipe;
-- garlemald emits it Rust-side in apply_do_zone_change_content /
-- the deferred private-area bundle — scripts never send it).
--
-- The four ids below are UNVERIFIED CANDIDATES in the worldMaster
-- "instance" family around 34108, pending an in-client probe: if a
-- candidate renders wrong text, the user reports it and only the
-- number changes here. Retail line order at escort entry (OCR'd
-- playthroughs + Mirke transcripts): journal update → 34108 →
-- "Protect Sisipu from harm." → "You are now bound by duty." →
-- "There are 30 minutes remaining." (log + centre banner; the banner
-- half is pending the same id probe).
TEXT_PROTECT_SISIPU		= 34109;	-- UNVERIFIED candidate: "Protect Sisipu from harm."
TEXT_BOUND_BY_DUTY		= 34110;	-- UNVERIFIED candidate: "You are now bound by duty."
TEXT_UNBOUND_FROM_DUTY	= 34111;	-- UNVERIFIED candidate: "You are no longer bound by duty."
TEXT_TIME_REMAINING		= 34112;	-- UNVERIFIED candidate: "There are <param> minutes remaining."

-- Sisipu's on-the-road guidance bark ("Oschon's Torch is due south...")
-- — man0l1 QUEST-sheet say id 337, CONFIRMED in the decoded client
-- processTtrBlkNml001. (Her SEQ_055 camp talk is say 119 — that one
-- belongs to the camp leg, not the escort.)
SAY_ESCORT_GUIDANCE		= 337;

-- Msg packs for the Npc LS
NPCLS_MSGS = {
	{339},
	{80, 81, 82},
	{131, 326, 132},
	{161, 162, 163, 164}
};

-- Round 5 (Garlemald-Server #46): the round-2 stageAfterQuestWarpDirector
-- helper and its per-warp kicks are gone. Wire-proven: the wipe +
-- 0x00E2(0x10) warp recipe completes on its own — warp-END (RX 0x0007)
-- arrives independent of any kick — so the Now-Loading veil never needed
-- the noticeEvent round-trip. Worse, the kick actively broke the session:
-- the client orders desktopWidgetMode(16) on EVERY event begin
-- (_onPreEvent) and only _onPostEvent cancels it — a kick whose EndEvent
-- lands mid-reload loses that teardown and leaves the session-global
-- desktopWidget masked (the "tutorial mode" menu lock). pmeteor stages
-- this director at exactly ONE site: the Baderon SEQ_000→SEQ_003 beat
-- (seq000_onTalk below), whose kicked noticeEvent drives the
-- processEventTu_001 tutorial hook — that one is retail's design and
-- stays. onNotice keeps its else→EndEvent arm as a safety net for it
-- (and any stray kick).

function onStart(player, quest)
	quest:StartSequence(SEQ_000);
	
	-- Immediately move to the Adventurer's Guild private area.
	callClientFunction(player, "delegateEvent", player, quest, "processEvent010");
	-- pmeteor's script order sends the arrival texts BEFORE the warp;
	-- the previous port put them after, which (with the deferred
	-- same-region bundle) shipped 0x0157 game messages into the
	-- client's Now-Loading gap — present in every crashing Hob run and
	-- absent from every working load of the identical destination (cold
	-- login + the GM private-area warp).
	player:SendGameMessage(quest, 320, 0x20);
	player:SendGameMessage(quest, 321, 0x20);
	-- Close the Hob talk event, THEN warp — 0x0131 ahead of the 0x00E2
	-- reload latch is retail's invariant ordering on every captured
	-- transition (the talk event's owner is also torn down by the
	-- warp's keep-list commit, so it must close first).
	player:EndEvent();
	GetWorldManager():DoZoneChange(player, 133, "PrivateAreaMasterPast", 2, 15, -459.619873, 40.0005722, 196.370377, 2.010813);
end

function onFinish(player, quest)
end

function onStateChange(player, quest, sequence)
	local data = quest:GetData();

	if (sequence == SEQ_000) then
		quest:SetENpc(YSHTOLA);
		quest:SetENpc(CRAPULOUS_ADVENTURER);
		quest:SetENpc(DUPLICITOUS_TRADER);
		quest:SetENpc(DEBONAIR_PIRATE);
		quest:SetENpc(ONYXHAIRED_ADVENTURER);
		quest:SetENpc(SKITTISH_ADVENTURER);
		quest:SetENpc(RELAXING_ADVENTURER);
		quest:SetENpc(BADERON, QFLAG_TALK);
		quest:SetENpc(MYTESYN);
		quest:SetENpc(COCKAHOOP_COCKSWAIN);
		quest:SetENpc(SENTENIOUS_SELLSWORD);
		quest:SetENpc(SOLICITOUS_SELLSWORD);
	elseif (sequence == SEQ_003) then
		quest:SetENpc(BADERON);
	elseif (sequence == SEQ_005) then
		quest:SetENpc(BADERON, QFLAG_TALK);
	elseif (sequence == SEQ_006) then
		quest:SetENpc(BADERON, QFLAG_TALK);
	elseif (sequence == SEQ_007) then
		local subseqCUL = data:GetCounter(CNTR_SEQ7_CUL);
		local subseqMSK = data:GetCounter(CNTR_SEQ7_MSK);
		-- Always active in this seqence
		quest:SetENpc(BADERON);
		quest:SetENpc(CHARLYS, subseqCUL == 0 and QFLAG_TALK or QFLAG_NONE);
		-- Down and Up the MSK guild
		quest:SetENpc(ISANDOREL, (subseqMSK == 0 or subseqMSK == 2) and QFLAG_TALK or QFLAG_NONE);
		if (subseqMSK == 1) then
			quest:SetENpc(MSK_TRIGGER, QFLAG_PUSH, false, true);
		elseif (subseqMSK == 2) then
			quest:SetENpc(MERLZIRN);
		end
		-- In Echo
		quest:SetENpc(NERVOUS_BARRACUDA);
		quest:SetENpc(INTIMIDATING_BARRACUDA);
		quest:SetENpc(OVEREAGER_BARRACUDA);
		quest:SetENpc(SOPHISTICATED_BARRACUDA);
		quest:SetENpc(SMIRKING_BARRACUDA);
		quest:SetENpc(MANNSKOEN);
		quest:SetENpc(TOTORUTO);
		quest:SetENpc(ADVENTURER1);
		quest:SetENpc(ADVENTURER2);
		quest:SetENpc(ADVENTURER3);
		quest:SetENpc(ECHO_EXIT_TRIGGER, subseqMSK == 3 and QFLAG_PUSH or QFLAG_NONE, false, subseqMSK == 3);
		-- Zone-in rescue (Garlemald-Server #46): a player who relogged
		-- inside the MSK Echo (PrivateAreaMasterPast/3) after the counter
		-- latched at 4 is trapped — the exit trigger above is QFLAG_NONE at
		-- subseqMSK==4, so the doors are dead. Re-issue the exit warp the
		-- dropped push-chain coroutine never got to. onStateChange re-runs
		-- at login zone-in (the processor's relog re-arm) and after every
		-- talk (quest:UpdateENPCs), so the rescue reaches the victim on the
		-- next relog or Echo-NPC talk. One-shot and chain-safe by
		-- construction: the flag is set BEFORE the warp here, the live
		-- ECHO_EXIT push chain sets it before ITS warp (so this never
		-- double-fires mid-chain), and at subseqMSK==3 (Echo still in
		-- progress) or in the SEQ_050 escort content the sequence/counter
		-- gates keep it dormant. A pre-flag save that already exited
		-- cleanly eats one same-zone reload flicker on its next
		-- onStateChange, then latches.
		if (subseqMSK >= 4 and not data:GetFlag(FLAG_SEQ7_MSK_EXITED)) then
			data:SetFlag(FLAG_SEQ7_MSK_EXITED);
			GetWorldManager():WarpToPublicArea(player);
		end
	elseif (sequence == SEQ_035) then
		quest:SetENpc(NNMULIKA, QFLAG_TALK);
	elseif (sequence == SEQ_040) then
		quest:SetENpc(SISIPU_EMOTE, QFLAG_TALK, true, false, true);
		quest:SetENpc(NNMULIKA);
	elseif (sequence == SEQ_048) then
		quest:SetENpc(BADERON);
		quest:SetENpc(ZEPHYR_TRIGGER, QFLAG_PUSH, false, true);
		quest:SetENpc(NNMULIKA);
	elseif (sequence == SEQ_050) then
		-- Escort-duty rescue (Garlemald-Server #46). onStateChange runs
		-- at SEQ_050 in exactly two situations:
		--   1. The legit escort start: startMan0l1Content sets
		--      FLAG_ESCORT_HANDOFF then calls StartSequence(SEQ_050) —
		--      the commands apply in order, so THIS run sees the flag
		--      SET, consumes it, and stays dormant.
		--   2. Everything else — the processor's relog re-arm for a
		--      player whose escort content died with the session
		--      (content instances don't persist), or a save wedged at
		--      SEQ_050 by an older build (the flag was never set —
		--      one such save exists today). The flag is CLEAR → roll
		--      back to SEQ_048 so the ZEPHYR_TRIGGER re-arms and the
		--      duty can be retried from the gate.
		-- One-shot-safe by construction: the rescue moves the sequence
		-- to SEQ_048, so this arm no longer matches. No mid-run
		-- onStateChange re-run is expected while the duty is live (no
		-- ENPCs are registered at SEQ_050, so no talk/UpdateENPCs path
		-- exists inside the content area); if a stray one ever fires,
		-- the failure mode is a graceful eject-to-retry, not a wedge.
		local data = quest:GetData();
		if (data:GetFlag(FLAG_ESCORT_HANDOFF)) then
			data:ClearFlag(FLAG_ESCORT_HANDOFF);
		else
			quest:StartSequence(SEQ_048);
		end
	elseif (sequence == SEQ_055) then
		quest:SetENpc(WINDWORN_CORPSE, QFLAG_TALK);
		quest:SetENpc(GLASSYEYED_CORPSE);
		quest:SetENpc(FEARSTRICKEN_CORPSE);
		quest:SetENpc(SISIPU);
	elseif (sequence == SEQ_060) then
		quest:SetENpc(SISIPU, QFLAG_TALK);
		quest:SetENpc(WINDWORN_CORPSE);
		quest:SetENpc(GLASSYEYED_CORPSE);
		quest:SetENpc(FEARSTRICKEN_CORPSE);
	elseif (sequence == SEQ_065) then
		quest:SetENpc(FSH_TRIGGER, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_075) then	
		quest:SetENpc(BODENOLF, QFLAG_TALK);
	elseif (sequence == SEQ_080) then	
		quest:SetENpc(HNAANZA, QFLAG_TALK);
		quest:SetENpc(TATTOOED_PIRATE);
		quest:SetENpc(IOFA);
		quest:SetENpc(BODENOLF);
		quest:SetENpc(MIMIDOA);
		quest:SetENpc(JOELLAUT);
		quest:SetENpc(WERNER);
		quest:SetENpc(HIHINE);
		quest:SetENpc(TRINNE);
	elseif (sequence == SEQ_085) then	
		quest:SetENpc(HNAANZA);
		quest:SetENpc(TATTOOED_PIRATE);
		quest:SetENpc(WERNER);
		quest:SetENpc(HIHINE);
		quest:SetENpc(TRINNE);
		quest:SetENpc(ECHO_EXIT_TRIGGER2, QFLAG_PUSH, false, true);
	elseif (sequence == SEQ_092) then	
		quest:SetENpc(BADERON, QFLAG_REWARD);
	end	
	
end

function onTalk(player, quest, npc)
	local sequence = quest:getSequence();
	local classId = npc:GetActorClassId();
	
	if (sequence == SEQ_000) then
		seq000_onTalk(player, quest, npc, classId);
	elseif (sequence == SEQ_003) then
		if (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent020_2");
			player:EndEvent();
		end
	elseif (sequence == SEQ_005) then
		if (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent026");
			player:EndEvent();
			quest:StartSequence(SEQ_006);
		end
	elseif (sequence == SEQ_006) then
		if (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent027");			
			player:EndEvent();
			player:SendGameMessage(GetWorldMaster(), 25117, 0x20, 11000125); -- You obtain Baderon's Recommendation
			quest:StartSequence(SEQ_007);
		end
	elseif (sequence == SEQ_007) then
		seq007_onTalk(player, quest, npc, classId);
	elseif (sequence == SEQ_035) then
		if (classId == NNMULIKA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent600");
			quest:StartSequence(SEQ_040);
			player:EndEvent();
			GetWorldManager():WarpToPrivateArea(player, "PrivateAreaMasterPast", 5);
		end
	elseif (sequence == SEQ_040) then
		if (classId == SISIPU_EMOTE) then
			local emoteTestStep = quest:GetData():GetCounter(CNTR_SEQ40_FSH);
			if (emoteTestStep == 0 or emoteTestStep == 1) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent601_1");
				player:SendGameMessage(GetWorldMaster(), 25083, MESSAGE_TYPE_SYSTEM, 1);
				if (emoteTestStep == 0) then
					quest:GetData():IncCounter(CNTR_SEQ40_FSH);
				end
			elseif (emoteTestStep == 2) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent601_2");
				player:SendGameMessage(GetWorldMaster(), 25083, MESSAGE_TYPE_SYSTEM, 1);
			elseif (emoteTestStep == 3) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent601_3");
				player:SendGameMessage(GetWorldMaster(), 25083, MESSAGE_TYPE_SYSTEM, 1);
			elseif (emoteTestStep == 4) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent601_4");
				player:SendGameMessage(GetWorldMaster(), 25083, MESSAGE_TYPE_SYSTEM, 1);
			elseif (emoteTestStep == 5) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent601_5");
				player:SendGameMessage(GetWorldMaster(), 25083, MESSAGE_TYPE_SYSTEM, 1);
			elseif (emoteTestStep == 6) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent601_6");
				player:SendGameMessage(GetWorldMaster(), 25083, MESSAGE_TYPE_SYSTEM, 1);
			end			
		elseif (classId == NNMULIKA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent600_2");
		end
		player:EndEvent();
	elseif (sequence == SEQ_048) then
		if (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent602_3");			
		elseif (classId == NNMULIKA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent602_2");		
		end		
		player:EndEvent();
	elseif (sequence == SEQ_055 or sequence == SEQ_060) then
		if (classId == SISIPU) then
			if (sequence == SEQ_060) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent615");
				quest:StartSequence(SEQ_065);
				player:EndEvent();
				GetWorldManager():WarpToPublicArea(player, -42.0, 37.678, 155.694, -1.25);
				return;
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent605_2");					
			end
		elseif (classId == WINDWORN_CORPSE) then
			if (sequence == SEQ_055) then
				callClientFunction(player, "delegateEvent", player, quest, "processEvent610");
				quest:StartSequence(SEQ_060);
			else
				callClientFunction(player, "delegateEvent", player, quest, "processEvent610_2");
			end
		elseif (classId == FEARSTRICKEN_CORPSE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent610_2");
		elseif (classId == GLASSYEYED_CORPSE) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent610_2");		
		end		
		player:EndEvent();
	elseif (sequence == SEQ_070) then
		if (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent615_2");
		end
		player:EndEvent();
	elseif (sequence == SEQ_075) then
		if (classId == BODENOLF) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent630");
			player:EndEvent();
			quest:StartSequence(SEQ_080);
			GetWorldManager():WarpToPrivateArea(player, "PrivateAreaMasterPast", 4, -504.985, 42.490, 433.712, 2.35);
		end
	elseif (sequence == SEQ_080) then
		if (classId == HNAANZA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent632");
			player:EndEvent();
			quest:StartSequence(SEQ_085);
		else
			seq080_085_onTalk(player, quest, npc, classId);
		end
	elseif (sequence == SEQ_085) then
		if (classId == HNAANZA) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent632_2");
		else
			seq080_085_onTalk(player, quest, npc, classId);
		end
	elseif (sequence == SEQ_092) then
		if (classId == BADERON) then
			callClientFunction(player, "delegateEvent", player, quest, "processEventComplete");
			callClientFunction(player, "delegateEvent", player, quest, "sqrwa", 300, 1, 1, 2);
			-- Completion rewards: 200 EXP + 6,000 gil (era guides;
			-- FFXIVenturer "Treasures of the Main" completion box). The
			-- guides also list an unnamed food item — no surviving
			-- source names it (quest_new_reward.csv isn't in any local
			-- dump), so the item grant stays TODO until the sheet row
			-- is recovered. (Garlemald-Server #46)
			player:AddExp(200, player.charaWork.parameterSave.state_mainSkill[0], 0);
			player:AddGil(6000);
			player:EndEvent();
			player:CompleteQuest(quest);
			return;
		end
	end
	
	quest:UpdateENPCs();
end

function seq000_onTalk(player, quest, npc, classId)
	if     (classId == CRAPULOUS_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent010_2");
	elseif (classId == SKITTISH_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent010_3");
	elseif (classId == DUPLICITOUS_TRADER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent010_4");	
	elseif (classId == DEBONAIR_PIRATE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent010_5");
	elseif (classId == ONYXHAIRED_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent010_6");
	elseif (classId == RELAXING_ADVENTURER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent010_7");
	elseif (classId == YSHTOLA) then		
		callClientFunction(player, "delegateEvent", player, quest, "processEvent010_8");
	elseif (classId == BADERON) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent020");
		quest:NewNpcLsMsg(1);
		quest:StartSequence(SEQ_003);
		player:EndEvent();		
		
		local director = GetWorldManager():GetArea(133):CreateDirector("AfterQuestWarpDirector", false);		
		player:AddDirector(director);
		director:StartDirector(true);
		player:SetLoginDirector(director);		
		player:KickEvent(director, "noticeEvent", true);
		
		quest:UpdateENPCs();
		GetWorldManager():DoZoneChange(player, 133, nil, 0, 15, player.positionX, player.positionY, player.positionZ, player.rotation);
		return;
	elseif (classId == MYTESYN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent010_7");	
	elseif (classId == COCKAHOOP_COCKSWAIN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEtc001");
	elseif (classId == SOLICITOUS_SELLSWORD) then
		callClientFunction(player, "delegateEvent", player, quest, "processEtc002");
	elseif (classId == SENTENIOUS_SELLSWORD) then
		callClientFunction(player, "delegateEvent", player, quest, "processEtc003");
	end
	
	player:EndEvent();
end

function seq007_onTalk(player, quest, npc, classId)
	local data = quest:GetData();
	local subseqCUL = data:GetCounter(CNTR_SEQ7_CUL);
	local subseqMSK = data:GetCounter(CNTR_SEQ7_MSK);
	
	if (classId == BADERON) then
		if (subseqCUL == 1) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent027_3");
		elseif (subseqMSK == 4) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent027_4");
		else
			callClientFunction(player, "delegateEvent", player, quest, "processEvent027_2");
		end
		-- Self-heal (Garlemald-Server #46): both guild errands latched
		-- (CUL sale done, MSK Echo exited) but no linkshell chain pending
		-- — the seq007_endSequence NewNpcLsMsg beat was lost (its coroutine
		-- died in the pre-neutralizer processEvent060 veil hang), leaving
		-- the save unable to reach onNpcLS/SEQ_035. Re-queue it: the Rust
		-- NewNpcLsMsg apply (`set_npc_ls_from`, actor/quest.rs) resets
		-- npc_ls_msg_step to 1, so the re-fired pack-2 chain starts at its
		-- first message. GetNpcLsFrom()==0 guards re-fires while a chain
		-- is already pending, so repeat Baderon talks can't spam the glow.
		if (subseqCUL >= 1 and subseqMSK >= 4 and data:GetNpcLsFrom() == 0) then
			quest:NewNpcLsMsg(1);
		end
	elseif (classId == CHARLYS) then
		if (subseqCUL == 0) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030");
			data:IncCounter(CNTR_SEQ7_CUL);
			if (data:GetCounter(CNTR_SEQ7_MSK) == 4) then
				seq007_endSequence(player, quest);
			end
			-- Charlys buys the Balloonfish for 2,000 gil — the upstream
			-- "--give 1000g" TODO undershot it; processEvent030's dialogue
			-- and the era guides both say 2,000 ("Sell the balloonfish to
			-- Charlys ... You will receive 2000 gil", FFXIVenturer
			-- "Treasures of the Main"). (Garlemald-Server #46)
			player:AddGil(2000);
		else
			callClientFunction(player, "delegateEvent", player, quest, "processEvent030_2");
		end
	elseif (classId == ISANDOREL) then
		if (subseqMSK == 2) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent050");
			data:IncCounter(CNTR_SEQ7_MSK);
			-- Close the talk event, THEN warp (0x0131 ahead of the 0x00E2
			-- reload latch — see onStart). This branch used to fall through
			-- to the shared EndEvent below, putting the teardown AFTER the
			-- warp; the early return keeps that shared EndEvent serving the
			-- other branches exactly once.
			player:EndEvent();
			GetWorldManager():WarpToPrivateArea(player, "PrivateAreaMasterPast", 3);
			return;
		elseif (subseqMSK == 0) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent035");
			data:IncCounter(CNTR_SEQ7_MSK);
		elseif (subseqMSK == 1) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent035_2");
		end
	elseif (classId == MERLZIRN) then
		-- "processEvent40_2" is NOT a typo for processEvent040_2: the
		-- client's decompiled Man0l1 event table really names this one
		-- without the zero padding, unlike its 0-padded siblings
		-- (meteor-decomp tools/decode_lpb.py on Man0l1.lpb). Renaming it
		-- would make the client drop the RPC. (Garlemald-Server #46)
		callClientFunction(player, "delegateEvent", player, quest, "processEvent40_2");
	elseif (classId == INTIMIDATING_BARRACUDA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_2");
	elseif (classId == TOTORUTO) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_4");		
	elseif (classId == MANNSKOEN) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_6");
	elseif (classId == NERVOUS_BARRACUDA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_7");
	elseif (classId == OVEREAGER_BARRACUDA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_8");
	elseif (classId == SOPHISTICATED_BARRACUDA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_9");
	elseif (classId == SMIRKING_BARRACUDA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_10");
	elseif (classId == ADVENTURER2) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_13");
	elseif (classId == ADVENTURER3) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_14");
	elseif (classId == ADVENTURER1) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent050_15");
	end
		
	player:EndEvent();
end

function seq007_endSequence(player, quest)
	-- processEvent033 is just two worldMaster:say lines (decoded client
	-- Man0l1.lua:596-610) — no player interaction. Wire-proven (round-3
	-- ECHO_EXIT capture): the client answers processEvent060 and
	-- processEvent604_3 but NEVER answers 033's EventUpdate, so the
	-- coroutine parked here forever and the EndEvent/SetFlag/exit-warp
	-- tail never ran. Deliver both lines server-side instead — text ids
	-- 333/334 on the man0l1 QUEST sheet, same sheet as the working
	-- 320/321 sends in onStart (wire-confirmed text_owner_id=2700193202).
	player:SendGameMessage(quest, 333, 0x20);
	player:SendGameMessage(quest, 334, 0x20);
	quest:NewNpcLsMsg(1);
end

function seq080_085_onTalk(player, quest, npc, classId)
	if (classId == IOFA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent630_2");
	elseif (classId == TRINNE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent630_3");
	elseif (classId == HIHINE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent630_4");
	elseif (classId == MIMIDOA) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent630_5");
	elseif (classId == WERNER) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent630_6");
	elseif (classId == TATTOOED_PIRATE) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent630_7");
	elseif (classId == JOELLAUT) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent630_8");
	elseif (classId == BODENOLF) then
		callClientFunction(player, "delegateEvent", player, quest, "processEvent630_9");
	end
	player:EndEvent();
end

function onPush(player, quest, npc)
	local data = quest:GetData();
	local sequence = quest:getSequence();
	local classId = npc:GetActorClassId();
	
	if (sequence == SEQ_007) then
		if (classId == MSK_TRIGGER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent040");
			data:IncCounter(CNTR_SEQ7_MSK);
			player:EndEvent();
			quest:UpdateENPCs();
			GetWorldManager():DoZoneChange(player, 230, nil, 0, 15, -620.0, 29.476, -70.050, 0.791);
		elseif (classId == ECHO_EXIT_TRIGGER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent060");
			-- No processEvent604_3 neutralizer here anymore (round 4):
			-- seq007_endSequence now delivers 033's two lines server-side
			-- (SendGameMessage — see that function), so nothing needs to
			-- play on a live screen between processEvent060 and the exit
			-- warp. The startFadeInCutSceneAfterWarp veil armed by 060
			-- (decoded Man0l1.lua:1135) stays up as the transition cover
			-- and the warp's own warp-END (RX 0x0007) clears it
			-- post-reload — no director kick (see the round-5 note above
			-- onStart). (Garlemald-Server #46, r4/r5.)
			data:IncCounter(CNTR_SEQ7_MSK);
			if (data:GetCounter(CNTR_SEQ7_CUL) == 1) then
				seq007_endSequence(player, quest);
			end
			-- Latch "exit warp issued" BEFORE quest:UpdateENPCs(): the
			-- re-run onStateChange sees the fresh flag and keeps the
			-- relog-rescue arm dormant on this (live) path.
			data:SetFlag(FLAG_SEQ7_MSK_EXITED);
			player:EndEvent();
			quest:UpdateENPCs();
			GetWorldManager():WarpToPublicArea(player);
		end
	elseif (sequence == SEQ_048) then
		if (classId == ZEPHYR_TRIGGER) then
			local result = callClientFunction(player, "delegateEvent", player, quest, "contentsJoinAskInBasaClass");
			if (result == 1) then
				-- The Zephyr Gate escort duty (Garlemald-Server #46) —
				-- replaces the inherited pmeteor skip ("For now just
				-- skip the sequence") that jumped straight to the
				-- processEvent605 arrival. The escort's completion beat
				-- (605 → SEQ_055 → lighthouse-echo warp) now lives in
				-- QuestDirectorMan0l101's coroutine.
				startMan0l1Content(player, quest);
				return;
			end
			player:EndEvent();
		end
	elseif (sequence == SEQ_065) then
		if (classId == FSH_TRIGGER) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent620");
			-- Fishermen's Guild payout for the strongbox delivery —
			-- 3,000 gil per the upstream TODO and the era guides
			-- (FFXIVenturer "Treasures of the Main": "You will receive
			-- 3000 gil"). (Garlemald-Server #46)
			player:AddGil(3000);
			player:EndEvent();
			quest:NewNpcLsMsg(1);
			quest:StartSequence(SEQ_070);
		end		
	elseif (sequence == SEQ_085) then
		if (classId == ECHO_EXIT_TRIGGER2) then
			callClientFunction(player, "delegateEvent", player, quest, "processEvent635");
			player:EndEvent();
			quest:NewNpcLsMsg(1);
			quest:StartSequence(SEQ_090);
			quest:UpdateENPCs();
			GetWorldManager():WarpToPublicArea(player);
		end
	end
end

function onEmote(player, quest, npc, eventName)
	local data = quest:GetData();
	local sequence = quest:getSequence();
	local classId = npc:GetActorClassId();	

	-- Play the emote
	if (eventName == "emoteDefault1") then 		-- Bow
		player:DoEmote(npc.Id, 5, 21041);
	elseif (eventName == "emoteDefault2") then	-- Clap
		player:DoEmote(npc.Id, 7, 21061);
	elseif (eventName == "emoteDefault3") then	-- Congratulate
		player:DoEmote(npc.Id, 29, 21281);
	elseif (eventName == "emoteDefault4") then	-- Poke
		player:DoEmote(npc.Id, 28, 21271);
	elseif (eventName == "emoteDefault5") then	-- Joy
		player:DoEmote(npc.Id, 18, 21171);
	elseif (eventName == "emoteDefault6") then	-- Wave
		player:DoEmote(npc.Id, 16, 21151);
	end
	wait(2.5);
	
	-- Handle the result
	if (sequence == SEQ_040) then
		if (classId == SISIPU_EMOTE) then
			local emoteTestStep = data:GetCounter(CNTR_SEQ40_FSH);
			-- Bow
			if (emoteTestStep == 1) then
				if (eventName == "emoteDefault1") then
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_7");
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_2");
					player:SendGameMessage(GetWorldMaster(), 25083, MESSAGE_TYPE_SYSTEM, 1);
					data:IncCounter(CNTR_SEQ40_FSH);
				else
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_8");
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_1");
				end
			-- Clap
			elseif (emoteTestStep == 2) then
				if (eventName == "emoteDefault2") then
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_7");
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_3");
					player:SendGameMessage(GetWorldMaster(), 25083, MESSAGE_TYPE_SYSTEM, 1);
					data:IncCounter(CNTR_SEQ40_FSH);
				else
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_8");					
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_2");
				end
			-- Congratulate
			elseif (emoteTestStep == 3) then
				if (eventName == "emoteDefault3") then
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_7");
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_4");
					player:SendGameMessage(GetWorldMaster(), 25083, MESSAGE_TYPE_SYSTEM, 1);
					data:IncCounter(CNTR_SEQ40_FSH);
				else
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_8");					
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_3");
					player:SendGameMessage(GetWorldMaster(), 25083, MESSAGE_TYPE_SYSTEM, 1);
				end
			-- Poke
			elseif (emoteTestStep == 4) then
				if (eventName == "emoteDefault4") then
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_7");
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_5");
					player:SendGameMessage(GetWorldMaster(), 25083, MESSAGE_TYPE_SYSTEM, 1);
					data:IncCounter(CNTR_SEQ40_FSH);					
				else
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_8");
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_4");
				end
			-- Joy
			elseif (emoteTestStep == 5) then
				if (eventName == "emoteDefault5") then
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_7");
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_6");
					player:SendGameMessage(GetWorldMaster(), 25083, MESSAGE_TYPE_SYSTEM, 1);
					data:IncCounter(CNTR_SEQ40_FSH);
				else
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_8");					
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_5");
				end
			-- Wave
			elseif (emoteTestStep == 6) then
				if (eventName == "emoteDefault6") then
					callClientFunction(player, "delegateEvent", player, quest, "processEvent602");
					player:EndEvent();
					GetWorldManager():WarpToPublicArea(player);
					quest:StartSequence(SEQ_048);
					return;
				else
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_8");					
					callClientFunction(player, "delegateEvent", player, quest, "processEvent601_6");
				end
			end
		end
	end
		
	player:EndEvent();
	quest:UpdateENPCs();
end

function onNotice(player, quest, target)
	local sequence = quest:getSequence();

	if (sequence == SEQ_003) then
		-- Lift the tutorial desktop-widget mask the client entered during the
		-- Baderon talk. processEvent020 deliberately leaves the client in
		-- desktopWidgetMode 16 (HUD hidden, Map/menu/linkpearl/NPC/aetheryte
		-- all disabled), verified by disassembling the shipped Man0l1.lpb. The
		-- ONLY thing that cancels mode 16 for the Limsa opener is the
		-- client-side `processEventTu_001` (the NPC-linkshell tutorial: it
		-- calls cancelDesktopWidgetMode(16) → desktopWidgetMode 120 — which
		-- the menu gate processCommandMap ALLOWS — plus setTutorialMask +
		-- openTutorialWidget(15) for the Path-Companion linkpearl).
		--
		-- IMPORTANT: drive it with the RAW (non-parking) RunEventFunction, NOT
		-- callClientFunction. callClientFunction does RunEventFunction THEN
		-- coroutine.yield("_WAIT_EVENT") and PARKS until the client replies
		-- with a 0x012E EventUpdate. processEventTu_001 is a synchronous UI
		-- mask (no _wait/cutscene/finishEvent) so it sends NO EventUpdate — so
		-- callClientFunction here parks forever, the trailing EndEvent never
		-- runs, the noticeEvent stays OPEN, and the client is event-locked
		-- (movement locked + every menu/pearl click suppressed → softlock).
		-- The raw RunEventFunction does not park, so the EndEvent below runs,
		-- closing the noticeEvent; the wire order is the proven
		-- RunEventFunction-then-EndEvent. After this the menu works (mode 120)
		-- and the now-flashing linkpearl is clickable → onNpcLS →
		-- endTutorialMode → full unlock (mode 8, free movement).
		-- (Garlemald-Server #46 — Baderon menu/softlock root cause.)
		player:RunEventFunction("delegateEvent", player, quest, "processEventTu_001");
		player:EndEvent();
	else
		-- Safety net (round 5: the only staged AfterQuestWarpDirector kick
		-- left is the Baderon SEQ_000→SEQ_003 beat — see the note above
		-- onStart). A kicked noticeEvent that lands after the sequence has
		-- already advanced (or any stray kick, e.g. the escort content
		-- director's) still routes here, and the client only needs the
		-- EndEvent to close it. NO quest state is touched: leaving the
		-- event open instead would event-lock the client (movement + menus
		-- dead, see the SEQ_003 comment above). (Garlemald-Server #46.)
		player:EndEvent();
	end

	quest:UpdateENPCs();
end

function onNpcLS(player, quest, from, msgStep)
	local sequence = quest:getSequence();
	local msgPack;

	if (from == 1) then
		-- Get the right msg pack
		if (sequence == SEQ_003) then
			msgPack = 1;
		elseif (sequence == SEQ_007 or sequence == SEQ_035) then
			msgPack = 2;
		elseif (sequence == SEQ_070 or sequence == SEQ_075) then
			msgPack = 3;
		elseif (sequence == SEQ_090 or sequence == SEQ_092) then
			msgPack = 4;
		end	
				
		-- Quick way to handle all msgs nicely.
		player:SendGameMessageLocalizedDisplayName(quest, NPCLS_MSGS[msgPack][msgStep], MESSAGE_TYPE_NPC_LINKSHELL, 1000015);
		if (msgStep >= #NPCLS_MSGS[msgPack]) then
			quest:EndOfNpcLsMsgs();
		else
			quest:ReadNpcLsMsg();
		end
		
		-- Handle anything else
		if (sequence == SEQ_003) then
			-- Player clicked the now-clickable Path-Companion linkpearl:
			-- show the success widget, close the NPC-linkshell tutorial that
			-- onNotice's processEventTu_001 opened, then end tutorial mode
			-- (final cleanup, cancels mode 120 / clears tutorialFlag → full
			-- menu). Mirrors man0g1; the wait(3) the Gridania script uses to
			-- pace the success animation is omitted here to keep onNpcLS a
			-- non-parking, run-to-completion hook.
			showTutorialSuccessWidget(player, 9080);
			closeTutorialWidget(player);
			endTutorialMode(player);
		elseif (sequence == SEQ_007) then
			quest:StartSequenceForNpcLs(SEQ_035);
		elseif (sequence == SEQ_070) then
			quest:StartSequenceForNpcLs(SEQ_075);
		elseif (sequence == SEQ_090) then
			quest:StartSequenceForNpcLs(SEQ_092);
		end
	end
	
	player:EndEvent();
end

function startMan0l1Content(player, quest)
	-- One-shot rescue latch FIRST, journal update SECOND: the commands
	-- apply in queue order, so the onStateChange(SEQ_050) run that
	-- StartSequence fires sees the flag SET and stays dormant (it
	-- consumes the flag — see the SEQ_050 rescue arm). This is also
	-- retail's first entry beat (journal update precedes the instance
	-- messages). (Garlemald-Server #46 — escort leg.)
	quest:GetData():SetFlag(FLAG_ESCORT_HANDOFF);
	quest:StartSequence(SEQ_050);

	-- ===== CUTSCENE → SAME-MAP DUTY WARP (Garlemald-Server #46) =====
	-- The escort runs as a content instance on ZONE 128 — retail
	-- geography, pmeteor's intended (dead-code) shape: CreateContentArea
	-- on the same map at the gate, then DoZoneChangeContent to pmeteor's
	-- (-63.25, 33.15, 164.51, 0.8) with spawnType 16.
	--
	-- The same-map reload is SOLVED: apply_do_zone_change_content's wipe
	-- + 0x00E2(0x10) recipe (DeleteAllActors + force-reload latch +
	-- immediate bundle) completes same-map content warps — capture-proven
	-- on the 193→193 / 184→184 tutorial transitions — because
	-- processEvent604_3 below neutralises the cutscene's after-warp veil
	-- IN PLACE before the warp (the veil, not the map resource, was the
	-- original hang; see seed/066 for the superseded zone-129 workaround
	-- rationale).
	-- ROLLBACK: flip the 6th CreateContentArea arg back to 129 and the
	-- warp destination back to (-991.88, 61.71, -1120.79, 0.0), and spawn
	-- bnpc 16..24 in SimpleContentMan0l101.lua — the seed/066 Skull
	-- Valley rows are kept intact for exactly that (seed/070).
	local contentArea = player.CurrentArea:CreateContentArea(player, "/Area/PrivateArea/Content/PrivateAreaMasterSimpleContent", "Man0l101", "SimpleContentMan0l101", "Quest/QuestDirectorMan0l101", 128);
	if (contentArea == nil) then
		return;
	end
	local director = contentArea:GetContentDirector();
	player:AddDirector(director);
	director:StartDirector(false);
	-- Script order keeps the kick BEFORE the warp (pmeteor parity); the
	-- Rust side defers the actual 0x012F TX until the post-warp ack —
	-- a kick riding the warp bundle AFTER DeleteAllActors is dropped by
	-- the client (the director actor was wiped), the client never
	-- answers, and the director coroutine never arms (wire-proven,
	-- session 53943). (Garlemald-Server #46.)
	player:KickEvent(director, "noticeEvent", true);
	player:SetLoginDirector(director);

	-- 1. Cutscene IN PLACE at the gate. processEvent604 = fadeOut +
	--    NQCutScene("man0l604") + startFadeInCutSceneAfterWarp (== engine
	--    _fadeInAfterWarp(), which ARMS a Now-Loading veil that waits for a warp).
	--    callClientFunction parks until the cut plays + arms the fade, then returns.
	callClientFunction(player, "delegateEvent", player, quest, "processEvent604");

	-- 2. NEUTRALISE the armed after-warp veil BEFORE warping. processEvent604_3 =
	--    startFadeInCutSceneDefault (_waitForMapLoaded → _fadeIn(1) → _waitForFading):
	--    the map is still the gate (loaded), so it fades the screen back in in place
	--    and clears the _fadeInAfterWarp pending state. This is the load-bearing fix
	--    (PROVEN root cause, packet-level + man0g0 contrast): the content warp itself
	--    is byte-correct (0x0007 DeleteAllActors + 0x00E2(0x10) force-reload +
	--    0x0005 SetMap + 0x00CE all delivered, client echoes RX 0x0007), and man0g0
	--    does the IDENTICAL same-zone 0x10 warp successfully — the ONLY difference is
	--    that man0g0 never arms an after-warp cutscene veil. With the veil cleared
	--    here, the following warp is a clean man0g0-style warp whose warp-END resolves
	--    normally → no stuck "Now Loading". (Garlemald-Server #46.)
	callClientFunction(player, "delegateEvent", player, quest, "processEvent604_3");

	-- 3. Close the push event BEFORE the warp. An EndEvent (0x0131)
	--    landing mid-reload loses the client's _onPostEvent teardown →
	--    the session-global desktopWidgetMode-16 mask ("tutorial mode"
	--    menu lock) — wire-proven; 0x0131 ahead of the 0x00E2 reload
	--    latch is retail's invariant ordering on every captured
	--    transition (see onStart). (Garlemald-Server #46.)
	player:EndEvent();

	-- 4. Duty warp into the zone-128 gate-side content instance (escort
	--    NPCs seeded on the southbound road — seed/070). spawnType 16
	--    (0x10) → apply_do_zone_change_content force-reload branch
	--    (34108 "You have entered an instance." + DeleteAllActors +
	--    0x00E2(0x10) + zone-in bundle), same as man0g0. The rest of the
	--    retail entry text ("Protect Sisipu from harm." / "You are now
	--    bound by duty." / "There are 30 minutes remaining.") is emitted
	--    post-warp by SimpleContentMan0l101.lua's onZoneIn — messages
	--    queued here after the warp command would ship into the client's
	--    Now-Loading gap (the man0l0 Hob-crash shape, see onStart).
	GetWorldManager():DoZoneChangeContent(player, contentArea, -63.25, 33.15, 164.51, 0.8, 16);
end

function getJournalInformation(player, quest)
	-- The leading 0 is intentional, not a stub: the journal sheet's
	-- first qtdata arg slot is unused for this quest (the SEQ_007
	-- objective text reads args 2-3 — '0,5,20' renders "visited both
	-- guilds", see the SEQ_007 sequence note up top). Sibling quests
	-- pass quest-item ids in this slot; man0l1 has none.
	return 0, quest:GetData():GetCounter(CNTR_SEQ7_CUL) * 5, quest:GetData():GetCounter(CNTR_SEQ7_MSK) * 5;
end

function getJournalMapMarkerList(player, quest)
	local sequence = quest:getSequence();
	local data = quest:GetData();
	local possibleMarkers = {};

	if (sequence == SEQ_000 or sequence == SEQ_005 or sequence == SEQ_006) then
		table.insert(possibleMarkers, MRKR_BADERON);
	elseif (sequence == SEQ_003) then
		table.insert(possibleMarkers, MRKR_BEARDED_ROCK);
	elseif (sequence == SEQ_007) then
		-- Both guild errands run in parallel; drop each marker as its
		-- sub-quest completes (CUL counter latches at 1 after the
		-- Balloonfish sale, MSK at 4 after the Echo exit).
		if (data:GetCounter(CNTR_SEQ7_CUL) == 0) then
			table.insert(possibleMarkers, MRKR_CUL_GUILD);
		end
		if (data:GetCounter(CNTR_SEQ7_MSK) < 4) then
			table.insert(possibleMarkers, MRKR_MSK_GUILD);
		end
	elseif (sequence == SEQ_035 or sequence == SEQ_040 or sequence == SEQ_065) then
		table.insert(possibleMarkers, MRKR_FSH_GUILD);
	elseif (sequence == SEQ_048) then
		table.insert(possibleMarkers, MRKR_ZEPHYR_GATE);
	elseif (sequence == SEQ_055) then
		table.insert(possibleMarkers, MRKR_LIGHTHOUSE);
	elseif (sequence == SEQ_060) then
		table.insert(possibleMarkers, MRKR_SISIPU);
	elseif (sequence == SEQ_075) then
		table.insert(possibleMarkers, MRKR_ARM_BSM_GUILD);
	elseif (sequence == SEQ_080) then
		table.insert(possibleMarkers, MRKR_HNAANZA);
	elseif (sequence == SEQ_085) then
		table.insert(possibleMarkers, MRKR_GUILD_EXIT);
	elseif (sequence == SEQ_092) then
		table.insert(possibleMarkers, MRKR_BADERON);
	end
	-- SEQ_050 (escort instance) and the SEQ_070/SEQ_090 linkshell beats
	-- have no map destination — empty list, same as the sibling quests'
	-- non-travel sequences.

	return unpack(possibleMarkers);
end