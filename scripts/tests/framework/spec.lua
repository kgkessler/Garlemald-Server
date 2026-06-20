-- Minimal describe/it spec framework for the content-test runner.
--
-- Loaded before any spec file. Specs call describe()/it() to register cases
-- into __SPEC.tests; the Rust runner then invokes each body, treating any Lua
-- error (including a walkthrough :expect* mismatch) as a failed test.

__SPEC = { stack = {}, tests = {} }

function describe(name, body)
    table.insert(__SPEC.stack, name)
    body()
    table.remove(__SPEC.stack)
end

function it(name, body)
    local prefix = table.concat(__SPEC.stack, " > ")
    local full = (#prefix > 0) and (prefix .. " > " .. name) or name
    table.insert(__SPEC.tests, { name = full, body = body })
end

-- Quest ENPC marker flag types: mirror scripts/lua/quest.lua so specs read in
-- the same vocabulary the content scripts use.
QFLAG_OFF      = 0
QFLAG_OFF_HIDE = 1
QFLAG_TALK     = 2
QFLAG_PUSH     = 3
QFLAG_REWARD   = 4
QFLAG_MAPONLY  = 5
QFLAG_NONE     = 0
