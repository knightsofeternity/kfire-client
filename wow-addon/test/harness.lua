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

if failures > 0 then
  out(failures .. " test(s) failed")
  os.exit(1)
end
out("all tests passed")
