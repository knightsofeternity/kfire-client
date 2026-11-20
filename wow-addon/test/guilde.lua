-- Banc hors jeu de KFIRE Guilde (lot 1). Lua 5.1, comme WoW.
--
-- Plusieurs joueurs simulés, chacun avec son propre addon chargé (son propre
-- environnement Lua), parlent sur un même canal de guilde factice. Le temps est
-- simulé : avancer(s) fait passer les minuteurs et la livraison des messages.
-- Ne remplace pas l'essai en jeu (débit réel du serveur, fenêtres).

local echecs = 0
local function out(s) io.write(s, "\n") end
local function verifier(cond, msg)
	if cond then out("ok   " .. msg) else echecs = echecs + 1 out("FAIL: " .. msg) end
end

local FICHIERS = {}
for l in io.lines("KFireGuilde/KFireGuilde.toc") do
	if l:match("%.lua$") then FICHIERS[#FICHIERS + 1] = "KFireGuilde/" .. l end
end

-- ------------------------------------------------------------------ le monde

local W
local function nouveauMonde()
	W = { now = 1763600000, minuteurs = {}, joueurs = {}, guilde = {}, envoisRefuses = 0 }
	math.randomseed(42)
end

local function programmer(delai, fn)
	W.minuteurs[#W.minuteurs + 1] = { t = W.now + delai, fn = fn }
end

local function avancer(secondes)
	local fin = W.now + secondes
	while true do
		local iMin
		for i, m in ipairs(W.minuteurs) do
			if m.t <= fin and (not iMin or m.t < W.minuteurs[iMin].t) then iMin = i end
		end
		if not iMin then break end
		local m = table.remove(W.minuteurs, iMin)
		if m.t > W.now then W.now = m.t end
		m.fn()
	end
	W.now = fin
end

-- Un cadre factice : toute méthode inconnue ne fait rien, Create… rend un cadre.
local function cadre(env)
	local o = { _scripts = {}, _shown = false, _text = "", _checked = false }
	o.SetScript = function(self, n, fn) self._scripts[n] = fn end
	o.GetScript = function(self, n) return self._scripts[n] end
	o.Show = function(self)
		local avant = self._shown
		self._shown = true
		if not avant and self._scripts.OnShow then self._scripts.OnShow(self) end
	end
	o.Hide = function(self) self._shown = false end
	o.IsShown = function(self) return self._shown end
	o.SetText = function(self, t) self._text = t end
	o.GetText = function(self) return self._text end
	o.SetChecked = function(self, v) self._checked = v end
	o.GetChecked = function(self) return self._checked end
	o.GetPoint = function() return "CENTER", nil, "CENTER", 10, 20 end
	env._cadres[#env._cadres + 1] = o
	return setmetatable(o, { __index = function(_, k)
		return function()
			if k:find("^Create") then return cadre(env) end
			return nil
		end
	end })
end

local function declencher(env, ev, ...)
	for _, c in ipairs(env._cadres) do
		local s = c._scripts.OnEvent
		if s then s(c, ev, ...) end
	end
end

-- Livre un message d'addon aux autres joueurs connectés, un peu plus tard.
local function livrer(de, msg, voie, cible)
	for _, j in pairs(W.joueurs) do
		if j.connecte and (voie == "GUILD" or j.nom == cible) then
			programmer(0.05, function()
				if j.connecte then declencher(j.env, "CHAT_MSG_ADDON", "KFGU", msg, voie, de) end
			end)
		end
	end
end

local function rosterIndex()
	local l = {}
	for _, m in ipairs(W.guilde) do l[#l + 1] = m end
	return l
end

-- Charge l'addon pour un joueur : un environnement Lua à lui, l'API du jeu simulée.
local function joueur(nom, opts)
	opts = opts or {}
	local j = { nom = nom, connecte = false, instance = false, imprime = {}, envois = 0, opts = opts }
	local env = setmetatable({ _cadres = {} }, { __index = _G })
	env._G = env
	j.env = env
	local prenom, royaume = nom:match("^([^-]+)-(.+)$")
	env.print = function(s) j.imprime[#j.imprime + 1] = s end
	env.time = function() return W.now end
	env.GetServerTime = env.time
	env.date = os.date
	env.C_Timer = { After = programmer }
	env.CreateFrame = function() return cadre(env) end
	env.UIParent = cadre(env)
	env.GameTooltip = cadre(env)
	env.UISpecialFrames = {}
	env.tinsert = table.insert
	env.RAID_CLASS_COLORS = { MAGE = { colorStr = "ff3fc7eb" }, PALADIN = { colorStr = "fff48cba" } }
	env.UnitFullName = function() return prenom, royaume end
	env.GetNormalizedRealmName = function() return royaume end
	env.GetBuildInfo = function() return "1.60.1", "1", "nov 2026", 16001 end
	env.IsInInstance = function() return j.instance, j.instance and "party" or "none" end
	env.IsInGuild = function() return true end
	env.GetGuildInfo = function() return "Knights of Eternity" end
	env.SlashCmdList = {}
	env.C_ChatInfo = {
		RegisterAddonMessagePrefix = function() end,
		SendAddonMessage = function(_, msg, voie, cible)
			assert(#msg <= 255, "message de plus de 255 octets")
			if j.refuser and j.refuser > 0 then
				j.refuser = j.refuser - 1
				W.envoisRefuses = W.envoisRefuses + 1
				return 3
			end
			j.envois = j.envois + 1
			livrer(nom, msg, voie, cible)
			return 0
		end,
	}
	env.C_GuildInfo = { GuildRoster = function()
		programmer(0.2, function() if j.connecte then declencher(env, "GUILD_ROSTER_UPDATE") end end)
	end }
	env.GetNumGuildMembers = function() return #W.guilde end
	env.GetGuildRosterInfo = function(i)
		local m = rosterIndex()[i]
		local enLigne = W.joueurs[m.nom] and W.joueurs[m.nom].connecte or m.enLigne or false
		return m.nom, m.rang, m.rangIndex, m.niveau, m.classe, m.zone, m.note, "", enLigne, 0, m.fichier
	end
	env.GetGuildRosterLastOnline = function(i)
		local m = rosterIndex()[i]
		return 0, 0, m.absent or 2, 0
	end
	env.C_PartyInfo = { InviteUnit = function(n) j.invite = n end }
	env.ChatFrame_SendTell = function(n) j.chuchote = n end
	env.KFireGuildeDB = opts.db
	j.ns = {}
	for _, f in ipairs(FICHIERS) do
		local chunk = assert(loadfile(f))
		setfenv(chunk, env)
		chunk("KFireGuilde", j.ns)
	end
	W.joueurs[nom] = j
	return j
end

local function connecter(j)
	j.connecte = true
	declencher(j.env, "ADDON_LOADED", "KFireGuilde")
	declencher(j.env, "PLAYER_ENTERING_WORLD", true, false)
end

local function deconnecter(j)
	j.connecte = false
	j.db = j.env.KFireGuildeDB   -- ce que le jeu écrit sur le disque
end

local GUILDE = {
	{ nom = "Ouranos-Chogall", rang = "Officier", rangIndex = 1, niveau = 60, classe = "Mage", fichier = "MAGE", zone = "Hurlevent" },
	{ nom = "Malkai-Chogall", rang = "Membre", rangIndex = 3, niveau = 42, classe = "Paladin", fichier = "PALADIN", zone = "Les Carmines" },
	{ nom = "Nenos-Chogall", rang = "Membre", rangIndex = 3, niveau = 25, classe = "Mage", fichier = "MAGE", zone = "Bois de la Pénombre" },
	{ nom = "Trayn-Chogall", rang = "Maître", rangIndex = 0, niveau = 60, classe = "Paladin", fichier = "PALADIN", zone = "Forgefer" },
	{ nom = "Sansaddon-Chogall", rang = "Recrue", rangIndex = 4, niveau = 12, classe = "Mage", fichier = "MAGE", zone = "Comté-de-l'Or", absent = 40 },
}

-- ------------------------------------------------------------------ codage

nouveauMonde()
W.guilde = GUILDE
local o = joueur("Ouranos-Chogall")
local ns = o.ns
local m = ns.coder("R", "membres", "Ouranos-Chogall", 5, "", "Oura~nos", "", "x\ny")
local g, c = ns.decoder(m)
verifier(g == "R" and c[5] == "Oura-nos" and c[4] == "" and c[7] == "x-y", "un « ~ » ou un retour à la ligne dans un texte ne casse pas le découpage")
verifier(ns.decoder("2~R~x") == nil, "un message d'un autre protocole est ignoré")

local long = ns.coder("R", "membres", "Ouranos-Chogall", 5, "", string.rep("abcdefghij", 70))
local bouts = ns.morceler(long)
local tousCourts = true
for _, b in ipairs(bouts) do if #b > 255 then tousCourts = false end end
verifier(#long > 700 and #bouts == 4 and tousCourts, "un message de " .. #long .. " octets part en 4 morceaux de 255 au plus")
local ordre, resultat = { 3, 1, 1, 4, 2 }, nil
for i, k in ipairs(ordre) do
	local r = ns.rassembler("X", bouts[k])
	if i < #ordre and r then resultat = "trop tôt" end
	if i == #ordre then resultat = resultat or r end
end
verifier(resultat == long, "les morceaux se rassemblent dans le désordre, un doublon ne compte pas")
verifier(ns.comparerVersions("0.1.10", "0.1.9") == 1 and ns.comparerVersions("0.1", "0.1.0") == 0
	and ns.comparerVersions("0.0.9", "0.1.0") == -1, "comparaison des versions chiffre par chiffre")

-- ------------------------------------------------------------------ profil

connecter(o)
avancer(1)
verifier(#ns.profilManquant() == 3, "au départ, il manque pseudo, perso principal et rôle")
verifier(ns.enregistrerProfil({ pseudo = "Oura", main = "Ouranos Bis", role = "D" }) == nil, "un perso principal avec une espace est refusé")
verifier(ns.enregistrerProfil({ pseudo = "Oura", main = "Ouranos", role = "X" }) == nil, "un rôle inconnu est refusé")
verifier(ns.enregistrerProfil({ pseudo = "  Oura|cff  ", main = "Ouranos", role = "D" }) == true and #ns.profilManquant() == 0,
	"un profil complet est enregistré")
verifier(ns.db.moi.pseudo == "Ouracff" and ns.db.moi.main == "Ouranos-Chogall", "le pseudo est nettoyé, le perso principal prend le royaume")
verifier(ns.affichage("Ouranos-Chogall") == "Oura (Ouranos)" or ns.affichage("Ouranos-Chogall") == "Ouracff (Ouranos)",
	"un membre avec un pseudo s'affiche « Pseudo (Prénom) »")
verifier(ns.affichage("Sansaddon-Chogall") == "Sansaddon", "sans fiche, le prénom seul")

-- ------------------------------------------------------------------ deux joueurs, puis un troisième

nouveauMonde()
W.guilde = GUILDE
o = joueur("Ouranos-Chogall")
connecter(o)
avancer(2)
o.ns.enregistrerProfil({ pseudo = "Oura", main = "Ouranos", role = "D" })
local t = joueur("Trayn-Chogall")
connecter(t)
avancer(2)
t.ns.enregistrerProfil({ pseudo = "Trayn", main = "Trayn", role = "T" })
avancer(20)
verifier(o.ns.fiches("membres")["Trayn-Chogall"] and o.ns.fiches("membres")["Trayn-Chogall"].role == "T",
	"Ouranos reçoit la fiche de Trayn en direct")
verifier(t.ns.fiches("membres")["Ouranos-Chogall"] and t.ns.fiches("membres")["Ouranos-Chogall"].pseudo == "Oura",
	"Trayn, arrivé après, récupère la fiche d'Ouranos par la synchro")

-- Nenos passe, remplit son profil et part avant que Malkai n'arrive.
local n = joueur("Nenos-Chogall")
connecter(n)
avancer(15)
n.ns.enregistrerProfil({ pseudo = "Nenos", main = "Nenos", role = "H" })
avancer(2)
deconnecter(n)
local servies = o.ns.synchro.servies + t.ns.synchro.servies
local mk = joueur("Malkai-Chogall")
connecter(mk)
avancer(20)
local fm = mk.ns.fiches("membres")
verifier(fm["Nenos-Chogall"] and fm["Nenos-Chogall"].role == "H", "la fiche d'un membre hors ligne est relayée à celui qui arrive")
verifier(fm["Ouranos-Chogall"] and fm["Trayn-Chogall"], "il reçoit aussi les fiches des membres en ligne")
verifier(o.ns.synchro.servies + t.ns.synchro.servies == servies + 1, "un seul membre répond à une demande de synchro")
verifier(mk.ns.synchro.recue and mk.ns.db.derniereSynchro > 0, "la fin de synchro est notée")

-- Nenos revient : sa fiche à jour l'emporte, une vieille fiche ne l'écrase pas.
verifier(o.ns.ranger("membres", "Nenos-Chogall", 1, "Nenos-Chogall", { "Vieux", "", "T", "0.0.1" }) == false
	and o.ns.fiches("membres")["Nenos-Chogall"].pseudo == "Nenos", "une fiche plus ancienne n'écrase pas la récente")

-- Il revient avec ce que le jeu avait gardé sur son disque.
local n2 = joueur("Nenos-Chogall", { db = n.db })
connecter(n2)
avancer(20)
verifier(n2.ns.db.moi.pseudo == "Nenos" and n2.ns.fiches("membres")["Malkai-Chogall"],
	"au retour, le profil est sur le disque et la synchro apporte les nouveaux")

-- ------------------------------------------------------------------ garde-fous

local avant = o.ns.stats.ignores
declencher(o.env, "CHAT_MSG_ADDON", "KFGU", o.ns.coder("R", "membres", "Pirate-Chogall", W.now, "", "Pirate"), "WHISPER", "Pirate-Chogall")
verifier(o.ns.stats.ignores == avant + 1 and not o.ns.fiches("membres")["Pirate-Chogall"], "un chuchotement d'un non-membre est ignoré")
declencher(o.env, "CHAT_MSG_ADDON", "KFGU", "2~R~membres~x", "GUILD", "Malkai-Chogall")
verifier(o.ns.versionsInconnues["Malkai-Chogall"], "un membre sur un autre protocole est repéré")
declencher(o.env, "CHAT_MSG_ADDON", "AUTRE", "1~R~membres~x", "GUILD", "Malkai-Chogall")
verifier(not o.ns.fiches("membres")["x"], "un autre préfixe n'est pas lu")

-- En instance, rien ne part ; tout part à la sortie.
o.instance = true
declencher(o.env, "PLAYER_ENTERING_WORLD", false, false)
local envois = o.envois
o.ns.enregistrerProfil({ pseudo = "Oura2", main = "Ouranos", role = "D" })
avancer(5)
verifier(o.envois == envois and o.ns.enAttente() == 1, "en instance, le message attend")
o.instance = false
declencher(o.env, "PLAYER_ENTERING_WORLD", false, false)
avancer(3)
verifier(o.envois == envois + 1 and mk.ns.fiches("membres")["Ouranos-Chogall"].pseudo == "Oura2", "à la sortie, il part et arrive")

-- Le jeu refuse pour débit : on réessaie.
o.refuser = 2
o.ns.enregistrerProfil({ pseudo = "Oura3", main = "Ouranos", role = "D" })
avancer(5)
verifier(W.envoisRefuses == 2 and o.ns.stats.refusDebit == 2 and mk.ns.fiches("membres")["Ouranos-Chogall"].pseudo == "Oura3",
	"un refus pour débit est réessayé jusqu'à passer")

-- ------------------------------------------------------------------ la page Guilde

local ns2 = mk.ns
ns2.lireRoster()  -- la liste du jeu, comme à la prochaine mise à jour
local tous = ns2.listeGuilde({})
verifier(#tous == 5 and tous[1].enLigne and not tous[#tous].enLigne, "tous les membres, en ligne d'abord")
verifier(#ns2.listeGuilde({ filtre = "enligne" }) == 4, "filtre « En ligne »")
verifier(#ns2.listeGuilde({ filtre = "addon" }) == 4, "filtre « Avec l'addon » (Sansaddon n'y est pas)")
local r = ns2.listeGuilde({ recherche = "oura3" })
verifier(#r == 1 and r[1].nom == "Ouranos-Chogall", "la recherche trouve un pseudo")
verifier(#ns2.listeGuilde({ recherche = "carmines" }) == 1, "la recherche trouve une zone")
local parRang = ns2.listeGuilde({ tri = "rang" })
verifier(parRang[1].nom == "Trayn-Chogall", "tri par rang : le Maître en premier")
verifier(r[1].etatVersion == "ajour", "même version : à jour")
ns2.ranger("membres", "Trayn-Chogall", W.now + 100, "Trayn-Chogall", { "Trayn", "Trayn-Chogall", "T", "0.2.0" })
verifier(ns2.listeGuilde({ recherche = "trayn" })[1].etatVersion == "recente", "une version plus récente est signalée")
local total, enLigne, addon = ns2.compteGuilde()
verifier(total == 5 and enLigne == 4 and addon == 4, "compte de l'entête : 5 membres, 4 en ligne, 4 avec l'addon")

-- ------------------------------------------------------------------ fiches oubliées

local vieux = { v = 1, moi = { roles = {} }, reglages = {}, derniereSynchro = 0,
	magasins = { membres = {
		["Ancien-Chogall"] = { _maj = W.now - 61 * 86400, pseudo = "Ancien" },
		["Recent-Chogall"] = { _maj = W.now - 10 * 86400, pseudo = "Recent" },
	} } }
local v = joueur("Trayn-Chogall", { db = vieux })
connecter(v)
verifier(not v.ns.fiches("membres")["Ancien-Chogall"] and v.ns.fiches("membres")["Recent-Chogall"],
	"une fiche sans nouvelles depuis 60 jours est oubliée au chargement")

-- ------------------------------------------------------------------ interface

local nouveau = joueur("Sansaddon-Chogall")
connecter(nouveau)
avancer(20)
nouveau.ns.ouvrir("guilde")
local p = nouveau.ns.pages.guilde
verifier(p:IsShown() and p.lignes[1]._shown and p.lignes[1].nom._text:find("Ouranos"), "la page Guilde s'ouvre et liste les membres")
verifier(p.compte._text:find("5 membres"), "l'entête compte les membres")
p.lignes[1]._scripts.OnClick(p.lignes[1], "RightButton")
verifier(nouveau.invite == p.lignes[1].donnee.nom, "clic droit sur un membre : invitation")
p.lignes[2]._scripts.OnClick(p.lignes[2], "LeftButton")
verifier(nouveau.chuchote == p.lignes[2].donnee.nom, "clic sur un membre : chuchotement")
verifier(nouveau.ns.profilPropose, "un profil incomplet ouvre le formulaire la première fois")
nouveau.ns.ouvrir("options")
verifier(nouveau.ns.pages.options:IsShown() and not p:IsShown(), "le menu change de page")
nouveau.env.SlashCmdList.KFIREGUILDE("diag")
verifier(#nouveau.imprime >= 5 and nouveau.imprime[1]:find("0.1.0"), "/ke diag affiche le diagnostic")

-- ------------------------------------------------------------------ aucune erreur avalée

for nom, j in pairs(W.joueurs) do
	verifier(j.ns.derniereErreur == nil, "aucune erreur Lua chez " .. nom .. (j.ns.derniereErreur and (" : " .. j.ns.derniereErreur) or ""))
end

out(echecs == 0 and "\ntout est vert" or ("\n" .. echecs .. " échec(s)"))
os.exit(echecs == 0 and 0 or 1)
