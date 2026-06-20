-- Gridania Main Story route - "Sundered Skies" (Man0g0).
--
-- One file per nation route: the
-- describe groups the whole route and each `it` walks one leg. SEQ_000 and the
-- SEQ_010 hand-off are pure quest-hook walks (the `walkthrough` bridge); SEQ_005
-- is the instanced combat tutorial on the real battle substrate (the `combat`
-- bridge, force-kill). Later route quests (Man0g1 -> ...) extend here.

-- Actor class ids (from scripts/lua/quests/man/man0g0.lua).
local YDA            = 1000009
local PAPALYMO       = 1000010
local PUSH_ADV_GUILD = 1099046

-- Quest flag bits.
local MINITUT0 = 0 -- talked to Yda
local MINITUT1 = 1 -- talked to Papalymo

describe("Gridania - Sundered Skies (Man0g0)", function()
    it("SEQ_000: walks Yda -> Papalymo -> Yda into the combat warp", function()
        local w = walkthrough.start("Man0g0")

        w:onStart():expectStartSequence(0)

        -- Fresh: Yda lit (talk + proximity push); Papalymo dark.
        w:stateChange(0)
            :expectEnpc(YDA, QFLAG_TALK)
            :expectEnpc(PAPALYMO, QFLAG_OFF)

        -- First Yda talk -> MINITUT0; Yda goes dark, Papalymo lights up.
        w:talk(YDA):expectDelegate("processTtrNomal003"):ack():expectFlagSet(MINITUT0)
        w:stateChange(0)
            :expectEnpc(YDA, QFLAG_OFF)
            :expectEnpc(PAPALYMO, QFLAG_TALK)

        -- Papalymo -> MINITUT1; Yda re-lights for the hand-off talk.
        w:talk(PAPALYMO):expectDelegate("processEvent000_2"):ack():expectFlagSet(MINITUT1)
        w:stateChange(0)
            :expectEnpc(YDA, QFLAG_TALK)
            :expectEnpc(PAPALYMO, QFLAG_OFF)

        -- Final Yda talk routes straight into doContentArea (no ack): clears
        -- the quest data, StartSequence(SEQ_005), then the content warp burst.
        w:talk(YDA)
            :expectStartSequence(5)
            :expectCreateContentArea()
            :expectZoneChangeContent()
            :expectSequence(5)
    end)

    it("SEQ_005: spawns the wolves, force-kills them, advances to SEQ_010", function()
        local c = combat.start("Man0g0")

        -- onCreate spawns the 3 wolves (3,4,5) + Yda (6) + Papalymo (7).
        c:expectSpawn(3):expectSpawn(4):expectSpawn(5)
        c:expectSpawn(6):expectSpawn(7)

        -- Drive the director to its battleComplete wait, then kill every
        -- hostile through the real death path (force-kill); battleComplete
        -- fires organically and the win sequence runs to completion.
        c:startDirector()
        c:killMonsters()

        c:expectSequence(10)     -- Man0g0 advanced past the combat tutorial
        c:expectContentCleared() -- the instanced content was torn down
        c:expectZone(155)        -- warped back out to Gridania
    end)

    it("SEQ_010: hands off to Souls Gone Wild (Man0g1)", function()
        -- Pick the quest up post-combat at SEQ_010 (the warp clears the
        -- SEQ_000 flags, so flags start at 0).
        local w = walkthrough.start("Man0g0", 10)

        -- The adventurers' guild push is armed.
        w:stateChange(10):expectEnpc(PUSH_ADV_GUILD, QFLAG_PUSH)

        -- Pushing it completes Man0g0 and starts Man0g1.
        w:push(PUSH_ADV_GUILD):expectHandoff(110006)
    end)
end)
