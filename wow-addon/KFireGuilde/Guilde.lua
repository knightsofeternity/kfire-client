-- KFIRE Guilde : la liste de guilde du jeu, croisée avec les fiches des membres.
--
-- La liste du jeu connaît tout le monde, avec l'addon ou non (niveau, classe,
-- rang, zone, dernière connexion, note). Les fiches « membres » ajoutent le
-- pseudo, le personnage principal, le rôle et la version de l'addon.
local _, ns = ...

local RAFRAICHIR = 60   -- secondes entre deux demandes de liste au serveur

ns.roster = {}          -- [Nom-Royaume] = membre
ns.rosterDate = nil

function ns.demanderRoster()
	if C_GuildInfo and C_GuildInfo.GuildRoster then
		C_GuildInfo.GuildRoster()
	elseif GuildRoster then
		GuildRoster()
	end
end

local function joursDepuis(i, enLigne)
	if enLigne or not GetGuildRosterLastOnline then return 0 end
	local annees, mois, jours, heures = GetGuildRosterLastOnline(i)
	if not annees then return nil end
	return (annees or 0) * 365 + (mois or 0) * 30 + (jours or 0) + ((heures or 0) / 24)
end

-- Les valeurs secrètes (en instance) ne se comparent pas : on garde la liste d'avant.
local function secret(v)
	return issecretvalue ~= nil and issecretvalue(v) == true
end

function ns.lireRoster()
	if not IsInGuild or not IsInGuild() then
		ns.roster = {}
		return
	end
	local n = GetNumGuildMembers() or 0
	if n == 0 or secret(n) then return end
	local roster = {}
	for i = 1, n do
		local nom, rang, rangIndex, niveau, classe, zone, note, _, enLigne, _, classeFichier = GetGuildRosterInfo(i)
		if nom and not secret(nom) then
			nom = ns.nomComplet(nom)
			roster[nom] = {
				nom = nom, rang = rang, rangIndex = rangIndex or 99, niveau = niveau or 0,
				classe = classe, classeFichier = classeFichier, zone = zone, note = note,
				enLigne = enLigne and true or false, absentJours = joursDepuis(i, enLigne),
			}
		end
	end
	ns.roster = roster
	ns.rosterDate = ns.maintenant()
	ns.photographierRoster()
	ns.signaler("roster")
end

-- Une photo de la liste sur le disque : le client KFIRE la lit, jeu fermé, pour la
-- page Guilde du site (lot 5). « vu » est la dernière connexion estimée.
function ns.photographierRoster()
	local maintenant = ns.maintenant()
	local membres = {}
	for nom, m in pairs(ns.roster) do
		membres[nom] = {
			niveau = m.niveau, classe = m.classeFichier, rang = m.rang, rangIndex = m.rangIndex,
			vu = m.enLigne and maintenant or (m.absentJours and math.floor(maintenant - m.absentJours * 86400)) or nil,
		}
	end
	ns.db.roster = { date = maintenant, guilde = GetGuildInfo and GetGuildInfo("player") or nil,
		royaume = ns.royaume(), membres = membres }
end

function ns.estMembre(nom)
	return ns.roster[nom] ~= nil
end

-- ------------------------------------------------------------------ la page Guilde

-- « ajour », « ancienne » (plus vieille que la mienne) ou « recente » ; nil sans l'addon.
function ns.etatVersion(version)
	if not version then return nil end
	local c = ns.comparerVersions(version, ns.VERSION)
	return c == 0 and "ajour" or (c < 0 and "ancienne" or "recente")
end

local function ligne(m)
	local f = ns.fiches("membres")[m.nom] or {}
	return {
		nom = m.nom, prenom = ns.prenom(m.nom), affichage = ns.affichage(m.nom),
		niveau = m.niveau, classe = m.classe, classeFichier = m.classeFichier,
		rang = m.rang, rangIndex = m.rangIndex, zone = m.zone, note = m.note,
		enLigne = m.enLigne, absentJours = m.absentJours,
		pseudo = f.pseudo, main = f.main, role = f.role,
		version = f.version, etatVersion = ns.etatVersion(f.version), vuAddon = f._maj,
	}
end

local function contient(texte, cherche)
	return texte and texte:lower():find(cherche, 1, true) ~= nil
end

local TRIS = {
	enligne = function(a, b)
		if a.enLigne ~= b.enLigne then return a.enLigne end
		if a.niveau ~= b.niveau then return a.niveau > b.niveau end
		return a.nom < b.nom
	end,
	niveau = function(a, b)
		if a.niveau ~= b.niveau then return a.niveau > b.niveau end
		return a.nom < b.nom
	end,
	rang = function(a, b)
		if a.rangIndex ~= b.rangIndex then return a.rangIndex < b.rangIndex end
		return a.nom < b.nom
	end,
	version = function(a, b)
		local c = ns.comparerVersions(a.version or "0", b.version or "0")
		if c ~= 0 then return c > 0 end
		return a.nom < b.nom
	end,
}
ns.TRIS = { "enligne", "niveau", "rang", "version" }

-- options : { filtre = "tous" | "enligne" | "addon", recherche = "texte", tri = "enligne"… }
function ns.listeGuilde(options)
	options = options or {}
	local filtre, tri = options.filtre or "tous", TRIS[options.tri or "enligne"] or TRIS.enligne
	local cherche = options.recherche and options.recherche ~= "" and options.recherche:lower() or nil
	local liste = {}
	for _, m in pairs(ns.roster) do
		local l = ligne(m)
		local garder = (filtre == "tous")
			or (filtre == "enligne" and l.enLigne)
			or (filtre == "addon" and l.version ~= nil)
		if garder and cherche then
			garder = contient(l.nom, cherche) or contient(l.pseudo, cherche)
				or contient(l.zone, cherche) or contient(l.version, cherche)
				or contient(l.main, cherche)
		end
		if garder then liste[#liste + 1] = l end
	end
	table.sort(liste, tri)
	return liste
end

-- Combien de membres en ligne, combien avec l'addon : l'entête de la page.
function ns.compteGuilde()
	local total, enLigne, addon = 0, 0, 0
	local membres = ns.fiches("membres")
	for nom, m in pairs(ns.roster) do
		total = total + 1
		if m.enLigne then enLigne = enLigne + 1 end
		if membres[nom] then addon = addon + 1 end
	end
	return total, enLigne, addon
end

ns.ecouter("GUILD_ROSTER_UPDATE", function()
	if ns.db then ns.lireRoster() end
end)

ns.surSignal("connexion", function()
	ns.demanderRoster()
	-- le serveur ne renvoie la liste que si elle a changé : on redemande de temps en temps
	local function boucle()
		ns.demanderRoster()
		ns.apres(RAFRAICHIR, boucle)
	end
	ns.apres(RAFRAICHIR, boucle)
end)
