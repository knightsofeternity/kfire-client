-- KFIRE Guilde : le socle commun aux autres fichiers (temps, évènements du jeu,
-- signaux internes, sauvegarde, noms des joueurs, versions).
--
-- Client WoW Forever (interface 16001, client « Midnight ») : les valeurs du jeu
-- peuvent être secrètes en instance, et les addons n'y envoient rien. Tout ce
-- qui part sur le réseau attend donc la sortie d'instance (voir Reseau.lua).
local ADDON, ns = ...

ns.ADDON = ADDON or "KFireGuilde"
ns.VERSION = "0.2.0"
ns.PROTO = 1          -- version du protocole réseau : un message d'une autre version est ignoré
ns.PREFIXE = "KFGU"   -- préfixe des messages d'addon (16 caractères au plus)

-- ------------------------------------------------------------------ temps

-- L'heure du serveur, la même pour tous les membres : elle date les fiches partagées.
function ns.maintenant()
	return (GetServerTime or time)()
end

function ns.apres(delai, fn)
	C_Timer.After(delai, fn)
end

-- Regroupe les appels rapprochés : fn ne part qu'une fois, « delai » après le dernier appel.
local regroupes = {}
function ns.regrouper(cle, delai, fn)
	local jeton = {}
	regroupes[cle] = jeton
	ns.apres(delai, function()
		if regroupes[cle] ~= jeton then return end
		regroupes[cle] = nil
		local ok, err = pcall(fn)
		if not ok then ns.erreur(err) end
	end)
end

-- ------------------------------------------------------------------ erreurs

-- Les erreurs Lua sont souvent masquées en jeu : on garde la dernière pour /ke diag.
ns.derniereErreur = nil
function ns.erreur(err)
	ns.derniereErreur = tostring(err)
end

function ns.dire(texte)
	print("|cff3fa2ffKFIRE Guilde|r : " .. texte)
end

-- ------------------------------------------------------------------ évènements du jeu

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

-- ------------------------------------------------------------------ signaux internes

-- Un fichier annonce un changement (« membres », « roster »…), l'interface se rafraîchit.
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

-- KFireGuildeDB est par compte : pseudo et personnage principal valent pour tous
-- les personnages ; le rôle est rangé par personnage.
local DB_VERSION = 1
function ns.preparerDB()
	if type(KFireGuildeDB) ~= "table" or KFireGuildeDB.v ~= DB_VERSION then
		KFireGuildeDB = { v = DB_VERSION }
	end
	local db = KFireGuildeDB
	db.moi = db.moi or { roles = {} }
	db.moi.roles = db.moi.roles or {}
	db.reglages = db.reglages or {}
	db.magasins = db.magasins or {}
	db.derniereSynchro = db.derniereSynchro or 0
	db.noms = db.noms or {}          -- [id d'objet] = nom, pour chercher hors du cache du jeu
	db.vus = db.vus or {}            -- ce qui a déjà fait l'objet d'une bulle
	ns.db = db
	return db
end

-- ------------------------------------------------------------------ noms

-- Mon royaume sans espaces, comme le jeu l'écrit dans « Nom-Royaume ».
function ns.royaume()
	local _, r = UnitFullName("player")
	if r and r ~= "" then return r end
	if GetNormalizedRealmName then
		local ok, n = pcall(GetNormalizedRealmName)
		if ok and n and n ~= "" then return n end
	end
	return ((GetRealmName and GetRealmName()) or ""):gsub("[%s%-]", "")
end

-- Tous les noms passent par ici : « Nom-Royaume », même pour un joueur de mon royaume.
function ns.nomComplet(nom)
	if not nom or nom == "" then return nil end
	if nom:find("-", 1, true) then return nom end
	return nom .. "-" .. ns.royaume()
end

function ns.moi()
	return ns.nomComplet((UnitFullName("player")))
end

-- « Ouranos-Chogall » → « Ouranos »
function ns.prenom(nom)
	return (nom or ""):match("^[^-]+") or nom
end

-- ------------------------------------------------------------------ versions

-- Compare « 0.1.10 » et « 0.1.9 » chiffre par chiffre : -1, 0 ou 1.
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

-- ------------------------------------------------------------------ objets

-- Un lien d'objet (« |Hitem:2589:…|h[Étoffe de lin]|h ») ou un texte : renvoie id, nom.
-- Un sort (enchantement) devient l'id « s7418 ».
function ns.lireObjet(texte)
	texte = tostring(texte or "")
	local id, nom = texte:match("|Hitem:(%d+)[^|]*|h%[(.-)%]|h")
	if id then
		ns.db.noms[id] = nom
		return id, nom
	end
	local sort, nomSort = texte:match("|H[a-z]+:(%d+)[^|]*|h%[(.-)%]|h")
	if sort then
		ns.db.noms["s" .. sort] = nomSort
		return "s" .. sort, nomSort
	end
	nom = texte:gsub("^%s+", ""):gsub("%s+$", "")
	if nom == "" then return nil, nil end
	-- un nom d'objet que le jeu connaît déjà (dans son cache)
	local lien = GetItemInfo and select(2, GetItemInfo(nom))
	if lien then
		local i = lien:match("item:(%d+)")
		if i then ns.db.noms[i] = nom return i, nom end
	end
	for i, n in pairs(ns.db.noms) do
		if n:lower() == nom:lower() then return i, n end
	end
	return nil, nom
end

function ns.nomObjet(id)
	if not id then return nil end
	id = tostring(id)
	local n = ns.db.noms[id]
	if n then return n end
	local num = tonumber(id:match("^s?(%d+)$"))
	if not num then return nil end
	if id:sub(1, 1) == "s" then
		n = GetSpellInfo and GetSpellInfo(num)
		if type(n) == "table" then n = n.name end
	else
		n = (C_Item and C_Item.GetItemNameByID and C_Item.GetItemNameByID(num)) or (GetItemInfo and GetItemInfo(num))
		if not n and C_Item and C_Item.RequestLoadItemDataByID then pcall(C_Item.RequestLoadItemDataByID, num) end
	end
	if n then ns.db.noms[id] = n end
	return n
end

-- Combien j'en ai, sacs et banque (la banque est celle vue à la dernière visite).
function ns.compter(id)
	local num = tonumber(id)
	if not num then return 0 end
	local f = (C_Item and C_Item.GetItemCount) or GetItemCount
	if not f then return 0 end
	local ok, n = pcall(f, num, true)
	return ok and tonumber(n) or 0
end

-- ------------------------------------------------------------------ instance

-- En instance, les addons ne peuvent rien envoyer : on retient l'état pour le réseau.
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
