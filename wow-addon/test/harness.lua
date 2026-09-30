-- Offline test bench for KFire.lua. Simulates just enough of the WoW client
-- (frames, events, timers, player info) to drive the addon. Lua 5.1, like WoW.

local failures = 0
-- The addon's print() is captured by the simulated world, so the bench writes
-- its own lines with io.write.
local function out(s) io.write(s, "\n") end
local function check(cond, msg)
  if not cond then
    failures = failures + 1
    out("FAIL: " .. msg)
  else
    out("ok   " .. msg)
  end
end

-- ---------------------------------------------------------------- the world
local W -- current simulated world, rebuilt by reset()

local function newChatFrame()
  local cf = { events = { TIME_PLAYED_MSG = true } }
  function cf:IsEventRegistered(e) return self.events[e] == true end
  function cf:UnregisterEvent(e) self.events[e] = nil end
  function cf:RegisterEvent(e) self.events[e] = true end
  return cf
end

local function reset(opts)
  opts = opts or {}
  W = {
    now = opts.now or 1759200000,
    frames = {},
    timers = {},
    requests = 0,
    printed = {},
  }
  -- Globals the addon reads.
  KFirePlayed = opts.saved
  NUM_CHAT_WINDOWS = 2
  ChatFrame1 = newChatFrame()
  ChatFrame2 = newChatFrame()
  issecretvalue = opts.issecretvalue
  SLASH_KFIRE1 = nil
  SlashCmdList = {}

  function CreateFrame()
    local f = { registered = {} }
    function f:RegisterEvent(e) self.registered[e] = true end
    function f:SetScript(_, fn) self.onEvent = fn end
    table.insert(W.frames, f)
    return f
  end
  C_Timer = {
    After = function(delay, fn) table.insert(W.timers, { delay = delay, fn = fn }) end,
  }
  function wipe(t) for k in pairs(t) do t[k] = nil end return t end
  function RequestTimePlayed() W.requests = W.requests + 1 end
  function GetServerTime() return W.now end
  function GetCurrentRegion() return opts.region or 3 end
  function GetRealmName() return opts.realm or "Confrérie du Thorium" end
  function GetNormalizedRealmName() return opts.realmNorm or "ConfrérieduThorium" end
  function UnitName() return opts.name or "Thrall" end
  function UnitClass() return "Chaman", "SHAMAN", 7 end
  function UnitLevel() return opts.level or 80 end
  function GetBuildInfo() return "12.1.0", "70009", "Sep 1 2026", 120100 end
  function print(...) table.insert(W.printed, table.concat({ ... }, " ")) end

  local chunk = assert(loadfile("KFire/KFire.lua"))
  chunk("KFire", {})
end

-- Delivers an event to every frame registered for it (the addon's frame, and
-- the chat frames when they still listen to TIME_PLAYED_MSG).
local function fire(event, ...)
  for _, f in ipairs(W.frames) do
    if f.registered[event] and f.onEvent then f.onEvent(f, event, ...) end
  end
end

-- Runs the timers queued so far whose delay is <= maxDelay, in order.
local function runTimers(maxDelay)
  local pending = W.timers
  W.timers = {}
  for _, t in ipairs(pending) do
    if t.delay <= maxDelay then t.fn() else table.insert(W.timers, t) end
  end
end

local function login()
  fire("ADDON_LOADED", "KFire")
  fire("PLAYER_ENTERING_WORLD", true, false)
  runTimers(5)
end

-- -------------------------------------------------------------------- tests
do -- 1. ADDON_LOADED creates the saved table
  reset()
  fire("ADDON_LOADED", "KFire")
  check(type(KFirePlayed) == "table" and KFirePlayed.v == 1 and type(KFirePlayed.chars) == "table",
    "ADDON_LOADED initialises KFirePlayed {v=1, chars={}}")
end

do -- 2. an earlier SavedVariables table is kept, other characters untouched
  reset({ saved = { v = 1, chars = { ["eu/Hyjal/Jaina"] = { played = 42 } } } })
  login()
  fire("TIME_PLAYED_MSG", 4474800, 3600)
  check(KFirePlayed.chars["eu/Hyjal/Jaina"].played == 42, "existing characters are kept")
end

do -- 3. an unknown format version is replaced, not misread
  reset({ saved = { v = 99, stuff = true } })
  fire("ADDON_LOADED", "KFire")
  check(KFirePlayed.v == 1 and next(KFirePlayed.chars) == nil, "unknown version resets the table")
end

do -- 4. another addon's ADDON_LOADED is ignored
  reset()
  fire("ADDON_LOADED", "SomethingElse")
  check(KFirePlayed == nil, "ADDON_LOADED of another addon does nothing")
end

do -- 5. login: request after the delay, never before
  reset()
  fire("ADDON_LOADED", "KFire")
  fire("PLAYER_ENTERING_WORLD", true, false)
  check(W.requests == 0, "no request before the login delay")
  runTimers(5)
  check(W.requests == 1, "one request after the login delay")
end

do -- 6. zoning (not a login, not a reload) sends nothing
  reset()
  fire("ADDON_LOADED", "KFire")
  fire("PLAYER_ENTERING_WORLD", false, false)
  runTimers(60)
  check(W.requests == 0, "zoning does not request /played")
end

do -- 7. a /reload requests too
  reset()
  fire("ADDON_LOADED", "KFire")
  fire("PLAYER_ENTERING_WORLD", false, true)
  runTimers(5)
  check(W.requests == 1, "a UI reload requests /played")
end

do -- 8. our request is silent, then chat gets its event back
  reset()
  login()
  check(not ChatFrame1:IsEventRegistered("TIME_PLAYED_MSG")
    and not ChatFrame2:IsEventRegistered("TIME_PLAYED_MSG"), "chat frames muted during our request")
  fire("TIME_PLAYED_MSG", 4474800, 3600)
  check(not ChatFrame1:IsEventRegistered("TIME_PLAYED_MSG"), "still muted while this reply is delivered")
  runTimers(0)
  check(ChatFrame1:IsEventRegistered("TIME_PLAYED_MSG")
    and ChatFrame2:IsEventRegistered("TIME_PLAYED_MSG"), "chat frames get TIME_PLAYED_MSG back")
end

do -- 9. the reply is recorded with every field
  reset()
  login()
  fire("TIME_PLAYED_MSG", 4474800, 3600)
  local c = KFirePlayed.chars["eu/ConfrérieduThorium/Thrall"]
  check(c ~= nil, "character stored under region/realmNorm/name")
  check(c and c.played == 4474800 and c.level == 80 and c.class == "SHAMAN", "played, level, class stored")
  check(c and c.realm == "Confrérie du Thorium" and c.realmNorm == "ConfrérieduThorium", "both realm forms stored")
  check(c and c.region == "eu" and c.at == 1759200000, "region and timestamp stored")
end

do -- 10. logout adds the time played since the reply
  reset()
  login()
  fire("TIME_PLAYED_MSG", 1000, 10)
  W.now = W.now + 3600
  fire("PLAYER_LOGOUT")
  local c = KFirePlayed.chars["eu/ConfrérieduThorium/Thrall"]
  check(c.played == 4600 and c.at == 1759203600, "logout records played + elapsed")
end

do -- 11. logout without any reply writes nothing
  reset()
  fire("ADDON_LOADED", "KFire")
  fire("PLAYER_LOGOUT")
  check(next(KFirePlayed.chars) == nil, "no reply, nothing recorded at logout")
end

do -- 12. a secret value is never stored
  reset({ issecretvalue = function() return true end })
  login()
  fire("TIME_PLAYED_MSG", 4474800, 3600)
  check(next(KFirePlayed.chars) == nil, "secret /played is not stored")
  SlashCmdList.KFIRE()
  check(W.printed[#W.printed]:find("valeur secrète OUI", 1, true) ~= nil, "/kfire reports the secret value")
end

do -- 13. no reply: chat is given back after the timeout
  reset()
  login()
  runTimers(10)
  check(ChatFrame1:IsEventRegistered("TIME_PLAYED_MSG"), "chat unmuted after the reply timeout")
end

do -- 14. a /played typed by the player is recorded but not muted
  reset()
  fire("ADDON_LOADED", "KFire")
  fire("TIME_PLAYED_MSG", 5000, 50)
  check(ChatFrame1:IsEventRegistered("TIME_PLAYED_MSG"), "a manual /played stays visible")
  check(KFirePlayed.chars["eu/ConfrérieduThorium/Thrall"].played == 5000, "a manual /played is recorded too")
end

do -- 15. unknown realm at reply time: nothing stored, no crash
  reset({ realmNorm = "" })
  login()
  fire("TIME_PLAYED_MSG", 5000, 50)
  check(next(KFirePlayed.chars) == nil, "no realm yet, nothing stored")
end

do -- 16. /kfire in the normal case
  reset()
  login()
  fire("TIME_PLAYED_MSG", 4474800, 3600)
  SlashCmdList.KFIRE()
  local line = W.printed[#W.printed]
  check(line:find("interface 120100", 1, true) and line:find("/played 4474800", 1, true)
    and line:find("1 personnage", 1, true), "/kfire prints interface, played and count")
end

if failures > 0 then
  out(failures .. " test(s) failed")
  os.exit(1)
end
out("all tests passed")
