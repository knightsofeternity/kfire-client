-- KFire records each character's /played in its SavedVariables, so the KFIRE
-- desktop client can read it once the game has written it to disk (on logout
-- or /reload) and report it. The game gives no playtime through any web API.
local ADDON = ...

local REGIONS = { [1] = "us", [2] = "kr", [3] = "eu", [4] = "tw", [5] = "cn" }
local LOGIN_DELAY = 5     -- the realm does not always answer right at login
local REPLY_TIMEOUT = 10  -- give the chat back even if no reply ever comes

local frame = CreateFrame("Frame")
local pending = false     -- a silent request of ours is in flight
local muted = {}          -- chat frames we took TIME_PLAYED_MSG away from
local last                -- { total = seconds, at = server time } of the latest reply
local secretSeen = false  -- the game handed us an unreadable (secret) value

-- Midnight-era clients (Retail, Forever) can hand out "secret" values that
-- cannot be compared or stored. Older clients have no issecretvalue at all.
local function isSecret(v)
  return issecretvalue ~= nil and issecretvalue(v) == true
end

local function mute()
  for i = 1, (NUM_CHAT_WINDOWS or 10) do
    local cf = _G["ChatFrame" .. i]
    if cf and cf:IsEventRegistered("TIME_PLAYED_MSG") then
      cf:UnregisterEvent("TIME_PLAYED_MSG")
      muted[#muted + 1] = cf
    end
  end
end

local function unmute()
  for _, cf in ipairs(muted) do
    cf:RegisterEvent("TIME_PLAYED_MSG")
  end
  wipe(muted)
end

local function requestSilently()
  pending = true
  mute()
  RequestTimePlayed()
  C_Timer.After(REPLY_TIMEOUT, function()
    if pending then
      pending = false
      unmute()
    end
  end)
end

local function record(total)
  local region = REGIONS[GetCurrentRegion and GetCurrentRegion() or 0] or "unknown"
  local realmNorm = GetNormalizedRealmName() or ""
  local name = UnitName("player") or ""
  if realmNorm == "" or name == "" then
    return -- too early in the login to know who we are; the logout pass retries
  end
  local _, classToken = UnitClass("player")
  KFirePlayed.chars[region .. "/" .. realmNorm .. "/" .. name] = {
    region = region,
    realm = GetRealmName() or realmNorm,
    realmNorm = realmNorm,
    name = name,
    played = total,
    level = UnitLevel("player"),
    class = classToken,
    at = GetServerTime(),
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
    local isInitialLogin, isReloadingUi = ...
    if isInitialLogin or isReloadingUi then
      C_Timer.After(LOGIN_DELAY, requestSilently)
    end
  elseif event == "TIME_PLAYED_MSG" then
    local total = ...
    if pending then
      pending = false
      -- Next frame: the chat frames must not get THIS reply, only later ones.
      C_Timer.After(0, unmute)
    end
    if isSecret(total) or type(total) ~= "number" then
      secretSeen = true
      return
    end
    last = { total = total, at = GetServerTime() }
    record(total)
  elseif event == "PLAYER_LOGOUT" then
    if last then
      record(last.total + (GetServerTime() - last.at))
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
