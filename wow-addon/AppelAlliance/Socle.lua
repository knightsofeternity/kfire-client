-- Appel d'Alliance : le socle (temps, évènements, signaux, sauvegarde, noms, guildes,
-- instance, comptes d'essai).
--
-- Client WoW Forever (interface 16001, client « Midnight ») : en instance, les addons
-- n'envoient rien et les messages arrivent en valeurs secrètes. L'addon y est sourd et
-- muet : Ne pas déranger automatique, puis rattrapage à la sortie.
local ADDON, ns = ...

ns.ADDON = ADDON or "AppelAlliance"

-- ------------------------------------------------------------------ temps

function ns.maintenant()
	return (GetServerTime or time)()
end

function ns.apres(delai, fn)
	local annule = false
	C_Timer.After(delai, function()
		if annule then return end
		local ok, err = pcall(fn)
		if not ok then ns.erreur(err) end
	end)
	return function() annule = true end
end

-- ------------------------------------------------------------------ erreurs et messages

ns.derniereErreur = nil
function ns.erreur(err)
	ns.derniereErreur = tostring(err)
end

ns.COULEUR = "|cffd4a017"
function ns.dire(texte)
	print(ns.COULEUR .. "Appel d'Alliance|r : " .. texte)
end

-- ------------------------------------------------------------------ évènements et signaux

local cadre = CreateFrame("Frame")
local ecouteurs = {}

function ns.ecouter(evenement, fn)
	if not ecouteurs[evenement] then
		ecouteurs[evenement] = {}
		cadre:RegisterEvent(evenement)
	end
	table.insert(ecouteurs[evenement], fn)
end

cadre:SetScript("OnEvent", function(_, evenement, ...)
	for _, fn in ipairs(ecouteurs[evenement] or {}) do
		local ok, err = pcall(fn, ...)
		if not ok then ns.erreur(err) end
	end
end)

local abonnes = {}
function ns.surSignal(nom, fn)
	abonnes[nom] = abonnes[nom] or {}
	table.insert(abonnes[nom], fn)
end

function ns.signaler(nom, ...)
	for _, fn in ipairs(abonnes[nom] or {}) do
		local ok, err = pcall(fn, ...)
		if not ok then ns.erreur(err) end
	end
end

-- ------------------------------------------------------------------ sauvegarde

-- AppelAllianceDB est par personnage (réglages, pseudo Discord, blocages) ;
-- AppelAllianceCompte est commun au compte (les membres de l'alliance déjà vus,
-- le dernier pseudo Discord saisi pour le proposer aux autres personnages).
function ns.preparerDB()
	if type(AppelAllianceDB) ~= "table" or AppelAllianceDB.v ~= 1 then AppelAllianceDB = { v = 1 } end
	if type(AppelAllianceCompte) ~= "table" or AppelAllianceCompte.v ~= 1 then AppelAllianceCompte = { v = 1 } end
	local db, compte = AppelAllianceDB, AppelAllianceCompte
	db.reglages = db.reglages or {}
	local r = db.reglages
	if r.son == nil then r.son = true end
	r.choixSon = r.choixSon or 1
	r.types = r.types or { D = true, Q = true, E = true, L = true }
	if r.filtreNiveau == nil then r.filtreNiveau = true end
	db.bloques = db.bloques or {}
	db.dernier = db.dernier or {}        -- dernier donjon, dernière zone saisis
	compte.membres = compte.membres or {}
	db.discord = db.discord or compte.discord
	ns.db, ns.compte = db, compte
	return db
end

-- ------------------------------------------------------------------ noms

function ns.royaume()
	local _, r = UnitFullName("player")
	if r and r ~= "" then return r end
	if GetNormalizedRealmName then
		local ok, n = pcall(GetNormalizedRealmName)
		if ok and n and n ~= "" then return n end
	end
	return ((GetRealmName and GetRealmName()) or ""):gsub("[%s%-]", "")
end

function ns.nomComplet(nom)
	if not nom or nom == "" then return nil end
	if nom:find("-", 1, true) then return nom end
	return nom .. "-" .. ns.royaume()
end

function ns.moi()
	return ns.nomComplet((UnitFullName("player")))
end

function ns.prenom(nom)
	return (nom or ""):match("^[^-]+") or nom
end

function ns.secret(v)
	return issecretvalue ~= nil and issecretvalue(v) == true
end

-- ------------------------------------------------------------------ guildes et faction

-- L'entrée de ns.GUILDES pour ce nom de guilde, ou nil si elle n'est pas de l'alliance.
function ns.guilde(nom)
	if not nom or nom == "" then return nil end
	for _, g in ipairs(ns.GUILDES) do
		if g.nom == nom then return g end
	end
	return nil
end

function ns.maGuilde()
	return (GetGuildInfo and GetGuildInfo("player")) or nil
end

function ns.sigle(nom)
	local g = ns.guilde(nom)
	return g and g.sigle or "?"
end

function ns.couleurGuilde(nom)
	local g = ns.guilde(nom)
	local c = g and g.couleur or { 0.7, 0.7, 0.7 }
	return ("|cff%02x%02x%02x"):format(c[1] * 255, c[2] * 255, c[3] * 255), c
end

function ns.essai()
	return (IsTrialAccount and IsTrialAccount()) or (IsVeteranTrialAccount and IsVeteranTrialAccount()) or false
end

-- Pourquoi l'addon reste inactif sur ce personnage (nil : tout va bien).
-- La guilde peut arriver quelques secondes après la connexion : « attente » le dit.
function ns.empechement()
	local faction = UnitFactionGroup and UnitFactionGroup("player")
	if faction and faction ~= "Alliance" then return "horde" end
	if IsInGuild and not IsInGuild() then return "sansguilde" end
	local g = ns.maGuilde()
	if not g then return "attente" end
	if not ns.guilde(g) then return "guilde" end
	return nil
end

-- ------------------------------------------------------------------ niveau, classe, rôle

function ns.monNiveau()
	return UnitLevel and UnitLevel("player") or 0
end

function ns.maClasse()
	local _, fichier = UnitClass("player")
	return fichier
end

-- ------------------------------------------------------------------ versions

function ns.comparerVersions(a, b)
	local ta, tb = {}, {}
	for n in tostring(a or ""):gmatch("%d+") do ta[#ta + 1] = tonumber(n) end
	for n in tostring(b or ""):gmatch("%d+") do tb[#tb + 1] = tonumber(n) end
	for i = 1, math.max(#ta, #tb) do
		local x, y = ta[i] or 0, tb[i] or 0
		if x ~= y then return x < y and -1 or 1 end
	end
	return 0
end

-- ------------------------------------------------------------------ instance

ns.enInstance = false
local function majInstance()
	local dedans, genre = IsInInstance()
	local avant = ns.enInstance
	ns.enInstance = dedans and genre ~= "none" or false
	if avant ~= ns.enInstance then ns.signaler("instance", ns.enInstance) end
end

ns.ecouter("ADDON_LOADED", function(nom)
	if nom ~= ns.ADDON then return end
	ns.preparerDB()
	ns.signaler("charge")
end)

ns.ecouter("PLAYER_ENTERING_WORLD", function(connexion, rechargement)
	majInstance()
	if connexion or rechargement then ns.signaler("connexion") end
end)
