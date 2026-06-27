-- Ul'dah Main Story route - "Flowers for All" (Man0u0).
--
-- One file per nation route. SEQ_000
-- and the SEQ_010 hand-off are pure quest-hook walks (`walkthrough` bridge);
-- SEQ_005 is the combat tutorial on the real battle substrate (`combat` bridge,
-- force-kill). Ul'dah's combat has a SINGLE hostile (a goobbue).
-- Later route quests (Man0u1 -> ...) extend here.

-- Actor class ids (from scripts/lua/quests/man/man0u0.lua).
local ASCILIA           = 1000042
local FRETFUL           = 1001491
local GILDIGGING        = 1001495
local EXIT_TRIGGER      = 1090372
local ULDAH_OPENING_EXIT = 1099046

-- Quest flag bits.
local MINITUT0 = 0
local MINITUT1 = 1
local MINITUT2 = 2
local MINITUT3 = 3

describe("Ul'dah - Flowers for All (Man0u0)", function()
    it("SEQ_000: walks the Merchant Strip push-then-talk tutorial", function()
        local w = walkthrough.start("Man0u0")

        w:onStart():expectStartSequence(0)

        -- Fresh Merchant Strip: only Ascilia is lit; passengers gated off.
        w:stateChange(0)
            :expectEnpc(ASCILIA, QFLAG_TALK)
            :expectEnpc(FRETFUL, QFLAG_OFF)
            :expectEnpc(GILDIGGING, QFLAG_OFF)

        -- Proximity push beat (no flag change) - exercises push + ack.
        w:push(ASCILIA):expectDelegate("processTtrNomal002"):ack()

        -- First Ascilia talk -> MINITUT0; the passengers light up.
        w:talk(ASCILIA):expectDelegate("processTtrNomal003"):ack():expectFlagSet(MINITUT0)
        w:stateChange(0)
            :expectEnpc(FRETFUL, QFLAG_TALK)
            :expectEnpc(GILDIGGING, QFLAG_TALK)

        -- Both passengers.
        w:talk(FRETFUL):expectDelegate("processTtrMini002_first"):ack():expectFlagSet(MINITUT2)
        w:talk(GILDIGGING):expectDelegate("processTtrMini003_first"):ack():expectFlagSet(MINITUT3)

        -- Ascilia again -> MINITUT1 (the closing tutorial beat).
        w:talk(ASCILIA):expectDelegate("processTtrMini001"):ack():expectFlagSet(MINITUT1)

        -- All four beats done (flags 0xF): the exit trigger arms its push.
        w:stateChange(0):expectEnpc(EXIT_TRIGGER, QFLAG_PUSH)

        -- Push the exit: doExitTrigger runs straight through (no ack) into
        -- StartSequence(SEQ_005) + the content warp burst.
        w:push(EXIT_TRIGGER)
            :expectStartSequence(5)
            :expectCreateContentArea()
            :expectZoneChangeContent()
            :expectSequence(5)
    end)

    it("SEQ_005: spawns the goobbue, force-kills it, advances to SEQ_010", function()
        local c = combat.start("Man0u0")

        -- onCreate spawns the goobbue (13) + Thancred (14) + Niellefresne (15).
        c:expectSpawn(13)
        c:expectSpawn(14):expectSpawn(15)

        c:startDirector()
        c:killMonsters()

        c:expectSequence(10)
        c:expectContentCleared()
        c:expectZone(175)
    end)

    it("SEQ_010: hands off to A Land Long Lost (Man0u1)", function()
        -- Post-combat at SEQ_010 (the warp cleared the SEQ_000 flags).
        local w = walkthrough.start("Man0u0", 10)

        w:stateChange(10):expectEnpc(ULDAH_OPENING_EXIT, QFLAG_PUSH)

        -- Pushing the exit completes Man0u0 and starts Man0u1.
        w:push(ULDAH_OPENING_EXIT):expectHandoff(110010)
    end)
end)
