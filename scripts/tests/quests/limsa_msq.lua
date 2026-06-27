-- Limsa Lominsa Main Story route - "Shapeless Melody" (Man0l0).
--
-- One file per nation route. SEQ_000
-- and the SEQ_010 hand-off are pure quest-hook walks (`walkthrough` bridge);
-- SEQ_005 is the combat tutorial on the real battle substrate (`combat` bridge,
-- force-kill). Later route quests (Man0l1 -> ...) extend here.

-- Actor class ids (from scripts/lua/quests/man/man0l0.lua).
local ROSTNSTHAL   = 1001652
local VIXEN        = 1000447
local BABYFACE     = 1000442
local EXIT_TRIGGER = 1090025
local HOB          = 1000151

-- Quest flag bits.
local MINITUT0 = 0
local MINITUT1 = 1
local MINITUT2 = 2
local MINITUT3 = 3

describe("Limsa Lominsa - Shapeless Melody (Man0l0)", function()
    it("SEQ_000: walks the boat tutorial through the exit-door choice", function()
        local w = walkthrough.start("Man0l0")

        w:onStart():expectStartSequence(0)

        -- Fresh boat: only Rostnsthal is lit (talk marker); the rest are dark.
        w:stateChange(0)
            :expectEnpc(ROSTNSTHAL, QFLAG_TALK)
            :expectEnpc(VIXEN, QFLAG_OFF)
            :expectEnpc(BABYFACE, QFLAG_OFF)
            :expectEnpc(EXIT_TRIGGER, QFLAG_OFF)

        -- First Rostnsthal talk -> MINITUT0; the passengers light up next.
        w:talk(ROSTNSTHAL):expectDelegate("processTtrNomal003")
            :ack():expectFlagSet(MINITUT0)
        w:stateChange(0)
            :expectEnpc(VIXEN, QFLAG_TALK)
            :expectEnpc(BABYFACE, QFLAG_TALK)

        -- Both passengers (order is interchangeable in retail).
        w:talk(VIXEN):expectDelegate("processTtrMini002"):ack():expectFlagSet(MINITUT2)
        w:talk(BABYFACE):expectDelegate("processTtrMini003"):ack():expectFlagSet(MINITUT3)

        -- Rostnsthal re-lights for the "sirens" line -> MINITUT1.
        w:stateChange(0):expectEnpc(ROSTNSTHAL, QFLAG_TALK)
        w:talk(ROSTNSTHAL):expectDelegate("processTtrMini001"):ack():expectFlagSet(MINITUT1)

        -- All four beats done (flags 0xF): the exit door arms its push.
        w:stateChange(0):expectEnpc(EXIT_TRIGGER, QFLAG_PUSH)

        -- Push the door, answer "yes": storm cutscene -> StartSequence(SEQ_005)
        -- + the content-area warp burst.
        w:push(EXIT_TRIGGER):expectDelegate("processEventNewRectAsk")
            :answer(1):expectDelegate("processEvent000_2")
            :ack()
            :expectStartSequence(5)
            :expectCreateContentArea()
            :expectZoneChangeContent()
            :expectSequence(5)
    end)

    it("SEQ_005: spawns the aurelias, force-kills them, advances to SEQ_010", function()
        local c = combat.start("Man0l0")

        -- onCreate spawns aurelias (8,9,10) + Sthalmann (11) + Y'shtola (12).
        c:expectSpawn(8):expectSpawn(9):expectSpawn(10)
        c:expectSpawn(11):expectSpawn(12)

        c:startDirector()
        c:killMonsters()

        c:expectSequence(10)
        c:expectContentCleared()
        c:expectZone(230)
    end)

    it("SEQ_010: hands off to Treasures of the Main (Man0l1)", function()
        -- Post-combat at SEQ_010 (the warp cleared the SEQ_000 flags).
        local w = walkthrough.start("Man0l0", 10)

        w:stateChange(10):expectEnpc(HOB, QFLAG_TALK)

        -- Talking Hob and answering "yes" completes Man0l0 -> Man0l1.
        w:talk(HOB):expectDelegate("processEvent020_9")
            :answer(1):expectHandoff(110002)
    end)
end)
