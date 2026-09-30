-- KFire records each character's /played in its SavedVariables, so the KFIRE
-- desktop client can read it once the game has written it to disk (on logout
-- or /reload) and report it. The game gives no playtime through any web API.
--
-- Runs on every client from 3.3.5 (Project Ascension) to Midnight (Retail,
-- Forever): anything added since 2010 is used only when it exists.
local ADDON = (...) or "KFire"

local REGIONS = { [1] = "us", [2] = "kr", [3] = "eu", [4] = "tw", [5] = "cn" }
local LOGIN_DELAY = 5     -- the realm does not always answer right at login
local REPLY_TIMEOUT = 10  -- give the chat back even if no reply ever comes

local frame = CreateFrame("Frame")
local seenWorld = false   -- the first PLAYER_ENTERING_WORLD of this UI session
local pending = false     -- a silent request of ours is in flight
local muted = {}          -- chat frames we took TIME_PLAYED_MSG away from
local last                -- { total = seconds, at = time } of the latest reply
local secretSeen = false  -- the game handed us an unreadable (secret) value

-- C_Timer only exists since 6.0: older clients count time in OnUpdate.
local timers, ticker = {}, nil
local function after(delay, fn)
  if C_Timer and C_Timer.After then
    return C_Timer.After(delay, fn)
  end
  if not ticker then
    ticker = CreateFrame("Frame")
    ticker:SetScript("OnUpdate", function(_, elapsed)
      for i = #timers, 1, -1 do
        local t = timers[i]
        t.left = t.left - elapsed
        if t.left <= 0 then
          table.remove(timers, i)
          t.fn()
        end
      end
    end)
  end
  timers[#timers + 1] = { left = delay, fn = fn }
end

local function now()
  return (GetServerTime or time)()
end

-- Midnight-era clients (Retail, Forever) can hand out "secret" values that
-- cannot be compared or stored. Older clients have no issecretvalue at all.
local function isSecret(v)
  return issecretvalue ~= nil and issecretvalue(v) == true
end

local function region()
  return REGIONS[GetCurrentRegion and GetCurrentRegion() or 0] or "unknown"
end

-- The realm without spaces or dashes, like GetNormalizedRealmName (5.x+).
local function normalizedRealm()
  if GetNormalizedRealmName then
    return GetNormalizedRealmName() or ""
  end
  local realm = (GetRealmName() or ""):gsub("[%s%-]", "")
  return realm
end

local function mute()
  for i = 1, (NUM_CHAT_WINDOWS or 10) do
    local cf = _G["ChatFrame" .. i]
    -- IsEventRegistered is missing from old clients, where every chat frame
    -- listens to TIME_PLAYED_MSG anyway.
    if cf and (not cf.IsEventRegistered or cf:IsEventRegistered("TIME_PLAYED_MSG")) then
      cf:UnregisterEvent("TIME_PLAYED_MSG")
      muted[#muted + 1] = cf
    end
  end
end

local function unmute()
  for i, cf in ipairs(muted) do
    cf:RegisterEvent("TIME_PLAYED_MSG")
    muted[i] = nil
  end
end

local function requestSilently()
  pending = true
  mute()
  RequestTimePlayed()
  after(REPLY_TIMEOUT, function()
    if pending then
      pending = false
      unmute()
    end
  end)
end

local function record(total)
  local realmNorm = normalizedRealm()
  local name = UnitName("player") or ""
  if realmNorm == "" or name == "" then
    return -- too early in the login to know who we are; the logout pass retries
  end
  local reg = region()
  local _, classToken = UnitClass("player")
  KFirePlayed.chars[reg .. "/" .. realmNorm .. "/" .. name] = {
    region = reg,
    realm = GetRealmName() or realmNorm,
    realmNorm = realmNorm,
    name = name,
    played = total,
    level = UnitLevel("player"),
    class = classToken,
    at = now(),
  }
end

frame:RegisterEvent("ADDON_LOADED")
frame:RegisterEvent("PLAYER_ENTERING_WORLD")
frame:RegisterEvent("TIME_PLAYED_MSG")
frame:RegisterEvent("PLAYER_LOGOUT")

frame:SetScript("OnEvent", function(_, event, ...)
  if event == "ADDON_LOADED" then
    if ... ~= ADDON then return end
    if type(KFirePlayed) ~= "table" or KFirePlayed.v ~= 1 then
      KFirePlayed = { v = 1, chars = {} }
    end
    KFirePlayed.chars = KFirePlayed.chars or {}
  elseif event == "PLAYER_ENTERING_WORLD" then
    -- The first one after loading is a login or a /reload (both restart the
    -- UI); later ones are zoning. Old clients do not say which, so we count.
    if not seenWorld then
      seenWorld = true
      after(LOGIN_DELAY, requestSilently)
    end
  elseif event == "TIME_PLAYED_MSG" then
    local total = ...
    if pending then
      pending = false
      -- Next frame: the chat frames must not get THIS reply, only later ones.
      after(0, unmute)
    end
    if isSecret(total) or type(total) ~= "number" then
      secretSeen = true
      return
    end
    last = { total = total, at = now() }
    record(total)
  elseif event == "PLAYER_LOGOUT" then
    if last then
      record(last.total + (now() - last.at))
    end
  end
end)

SLASH_KFIRE1 = "/kfire"
SlashCmdList.KFIRE = function()
  local n = 0
  for _ in pairs(KFirePlayed and KFirePlayed.chars or {}) do n = n + 1 end
  print(("KFIRE : interface %s, /played %s, valeur secrète %s, %d personnage(s) enregistré(s)")
    :format(tostring(select(4, GetBuildInfo())),
      last and tostring(last.total) or "inconnu",
      secretSeen and "OUI" or "non", n))
end
