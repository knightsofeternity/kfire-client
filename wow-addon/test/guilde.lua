-- Banc hors jeu de KFIRE Guilde (lots 1 à 4). Lua 5.1, comme WoW.
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
	if l:match("%.lua$") and not l:match("^#") and io.open("KFireGuilde/" .. l) then FICHIERS[#FICHIERS + 1] = "KFireGuilde/" .. l end
end

-- ------------------------------------------------------------------ le monde

local W
local function nouveauMonde()
	W = { now = 1763600000, minuteurs = {}, joueurs = {}, guilde = {}, envoisRefuses = 0,
		objets = { [2589] = "Étoffe de lin", [2996] = "Rouleau d'étoffe de lin", [4343] = "Pantalon en lin brun" },
		sorts = { [7418] = "Enchantement de bracelets (Vie mineure)" } }
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
	env.time = function(t) if t then return os.time(t) end return W.now end
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
	-- objets, métiers, quêtes, calendrier (lots 2 à 4)
	j.sacs, j.quetes = {}, {}
	env.GetItemCount = function(id) return j.sacs[tonumber(id)] or 0 end
	env.GetItemInfo = function(x)
		for id, n in pairs(W.objets) do
			if x == n or tonumber(x) == id then return n, ("|cffffffff|Hitem:%d::::|h[%s]|h|r"):format(id, n) end
		end
	end
	env.GetSpellInfo = function(id) return W.sorts[id] end
	env.InCombatLockdown = function() return false end
	env.hooksecurefunc = function() end
	env.GetNumSkillLines = function() return #(j.competences or {}) end
	env.GetSkillLineInfo = function(i)
		local c = j.competences[i]
		return c[1], c[2], false, c[3], 0, 0, c[4]
	end
	env.GetTradeSkillLine = function() return j.fenetre.nom, j.fenetre.rang, j.fenetre.max end
	env.GetNumTradeSkills = function() return #j.fenetre.recettes end
	env.GetTradeSkillInfo = function(i)
		local r = j.fenetre.recettes[i]
		return r.nom, r.entete and "header" or "easy"
	end
	env.GetTradeSkillItemLink = function(i)
		local r = j.fenetre.recettes[i]
		return r.id and ("|cffffffff|Hitem:%d::::|h[%s]|h|r"):format(r.id, r.nom) or nil
	end
	env.GetTradeSkillRecipeLink = function(i)
		local r = j.fenetre.recettes[i]
		return r.sort and ("|cffffd000|Henchant:%d|h[%s]|h|r"):format(r.sort, r.nom) or nil
	end
	env.GetNumQuestLogEntries = function() return #j.quetes end
	env.GetQuestLogTitle = function(i)
		local q = j.quetes[i]
		return q[2], 20, nil, q.entete or false, false, nil, nil, q[1]
	end
	env.C_Calendar = W.calendrier and W.calendrier(j) or nil
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
ns2.ranger("membres", "Trayn-Chogall", W.now + 100, "Trayn-Chogall", { "Trayn", "Trayn-Chogall", "T", "0.9.0" })
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
verifier(#nouveau.imprime >= 5 and nouveau.imprime[1]:find(nouveau.ns.VERSION, 1, true), "/ke diag affiche le diagnostic")

-- ================================================================== lots 2 à 4

local function lien(id) return ("|cffffffff|Hitem:%d::::|h[%s]|h|r"):format(id, W.objets[id]) end
local function derniereBulle(j) return j.ns.bullesRecentes[1] and j.ns.bullesRecentes[1].texte or "" end

-- Un calendrier du jeu factice, le même pour tous : un raid dans deux jours.
local function calendrierFactice(evenements)
	return function(j)
		local C = {}
		C.OpenCalendar = function() programmer(0.2, function() declencher(j.env, "CALENDAR_UPDATE_GUILD_EVENTS") end) end
		C.SetAbsMonth = function() end
		C.GetNumGuildEvents = function() return #evenements end
		C.GetGuildEventInfo = function(i)
			local e = evenements[i]
			local d = os.date("*t", e.quand)
			return { title = e.titre, year = d.year, month = d.month, monthDay = d.day, hour = d.hour, minute = d.min,
				eventType = 0, calendarType = "GUILD_EVENT", eventID = e.id }
		end
		C.GetGuildEventSelectionInfo = function(i) return 0, os.date("*t", evenements[i].quand).day, i end
		C.OpenEvent = function(_, _, i)
			j.ouvert = evenements[i]
			programmer(0.1, function() declencher(j.env, "CALENDAR_OPEN_EVENT", "GUILD") end)
		end
		C.GetEventInfo = function()
			local e = j.ouvert
			local d = os.date("*t", e.quand)
			return { title = e.titre, description = "Réunion devant MC", creator = "Trayn", inviteType = 1,
				calendarType = "GUILD_EVENT", time = { year = d.year, month = d.month, monthDay = d.day, hour = d.hour, minute = d.min } }
		end
		C.GetNumInvites = function() return #j.ouvert.inscrits end
		C.EventGetInvite = function(i) return j.ouvert.inscrits[i] end
		C.IsActionPending = function() return false end
		local function repondre(quoi, etat)
			return function()
				j.reponse = quoi
				table.insert(j.ouvert.inscrits, { name = j.nom:match("^[^-]+"), inviteStatus = etat, classFilename = "MAGE" })
			end
		end
		C.EventSignUp = repondre("EventSignUp", 6)
		C.EventAvailable = repondre("EventAvailable", 1)
		C.EventTentative = repondre("EventTentative", 8)
		C.EventDecline = repondre("EventDecline", 2)
		return C
	end
end

nouveauMonde()
W.guilde = GUILDE
local raid = { titre = "Raid Cœur du Magma", quand = W.now + 2 * 86400, id = 11, inscrits = {} }
local evenements = { raid }
W.calendrier = calendrierFactice(evenements)
o = joueur("Ouranos-Chogall")
mk = joueur("Malkai-Chogall")
n = joueur("Nenos-Chogall")
connecter(o) connecter(mk) connecter(n)
avancer(20)

-- ------------------------------------------------------------------ demandes

mk.sacs[2589] = 30
verifier(o.ns.publierDemande({ type = "R", objet = "", qte = 2 }) == nil, "une demande sans objet est refusée")
verifier(o.ns.publierDemande({ type = "R", objet = lien(2589), qte = 0 }) == nil, "une quantité de 0 est refusée")
local cle = o.ns.publierDemande({ type = "R", objet = lien(2589), qte = 20, texte = "pour des sacs~de 6" })
avancer(2)
local dm = mk.ns.fiches("demandes")[cle]
verifier(dm and dm.objet == "2589" and dm.nom == "Étoffe de lin" and dm.qte == "20" and dm.etat == "O",
	"la demande d'Ouranos arrive chez Malkai (objet, quantité, état)")
local pm = mk.ns.listeDemandes({ filtre = "pourmoi" })
verifier(#pm == 1 and pm[1].possede == 30, "Malkai la voit dans « Pour moi » : il en a 30")
verifier(derniereBulle(mk):find("Étoffe de lin", 1, true) and derniereBulle(mk):find("tu en as 30", 1, true),
	"une bulle prévient Malkai qu'il peut l'honorer")
verifier(#n.ns.listeDemandes({ filtre = "pourmoi" }) == 0 and #n.ns.listeDemandes({}) == 1,
	"Nenos la voit dans les ouvertes, pas dans « Pour moi »")
verifier(not o.ns.actionsDemande(cle).prendre and o.ns.actionsDemande(cle).retirer, "l'auteur ne prend pas sa propre demande, il peut la retirer")
verifier(n.ns.retirerDemande(cle) == nil, "un simple membre ne retire pas la demande d'un autre")

verifier(mk.ns.prendreDemande(cle) == true, "Malkai prend la demande")
avancer(2)
verifier(o.ns.fiches("demandes")[cle].etat == "P" and o.ns.fiches("demandes")[cle].preneur == "Malkai-Chogall",
	"Ouranos voit qui s'en occupe")
verifier(derniereBulle(o):find("s'occupe de ta demande", 1, true), "et une bulle le lui dit")
verifier(n.ns.prendreDemande(cle) == nil, "une demande prise ne se prend pas deux fois")
mk.ns.honorerDemande(cle)
avancer(2)
verifier(o.ns.fiches("demandes")[cle].etat == "H" and derniereBulle(o):find("a honoré", 1, true), "honorée : l'auteur est prévenu")
verifier(#o.ns.listeDemandes({ filtre = "ouvertes" }) == 0 and #o.ns.listeDemandes({ filtre = "toutes" }) == 1,
	"une demande honorée sort des ouvertes")

-- modération : un officier retire la demande d'un membre
local cle2 = n.ns.publierDemande({ type = "R", objet = "Plume magique", qte = 5 })
avancer(2)
verifier(o.ns.peutModerer() and not mk.ns.peutModerer(), "Ouranos (officier) peut modérer, Malkai (membre) non")
verifier(o.ns.retirerDemande(cle2) == true, "l'officier retire la demande de Nenos")
avancer(2)
verifier(n.ns.fiches("demandes")[cle2].etat == "X", "Nenos la voit retirée")

-- celui qui arrive reçoit les demandes par la synchro
local t2 = joueur("Trayn-Chogall")
connecter(t2)
avancer(20)
verifier(t2.ns.fiches("demandes")[cle] and t2.ns.fiches("demandes")[cle2], "un membre qui arrive reçoit les demandes par la synchro")

-- durées de vie : honorée 3 jours, ouverte 30 jours
local fd = o.ns.fiches("demandes")
fd["Vieux-Chogall/1"] = { _maj = W.now - 4 * 86400, etat = "H", nom = "x" }
fd["Vieux-Chogall/2"] = { _maj = W.now - 4 * 86400, etat = "O", nom = "y" }
fd["Vieux-Chogall/3"] = { _maj = W.now - 31 * 86400, etat = "O", nom = "z" }
o.ns.nettoyerMagasins()
verifier(not fd["Vieux-Chogall/1"] and fd["Vieux-Chogall/2"] and not fd["Vieux-Chogall/3"],
	"honorée oubliée après 3 jours, ouverte gardée 30 jours")

-- ------------------------------------------------------------------ métiers

mk.fenetre = { nom = "Couture", rang = 75, max = 150, recettes = {
	{ nom = "Couture", entete = true }, { nom = "Rouleau d'étoffe de lin", id = 2996 }, { nom = "Pantalon en lin brun", id = 4343 },
} }
declencher(mk.env, "TRADE_SKILL_SHOW")
avancer(6)
local fm2 = o.ns.fiches("metiers")["Malkai-Chogall/Couture"]
verifier(fm2 and fm2.niveau == "75" and fm2.recettes == "2996,4343", "ouvrir la fenêtre de métier publie niveau et recettes")
verifier(o.ns.fabricants(4343)[1] == "Malkai-Chogall", "Ouranos sait que Malkai fabrique le pantalon")
local rr = o.ns.chercherRecettes("pantalon")
verifier(#rr == 1 and rr[1].fabricants[1] == "Malkai-Chogall", "« qui sait fabriquer pantalon » : Malkai")
local envoisAvant = mk.envois
mk.fenetre.recettes = { { nom = "Pantalon en lin brun", id = 4343 } }  -- un filtre dans la fenêtre
declencher(mk.env, "TRADE_SKILL_UPDATE")
avancer(6)
verifier(o.ns.fiches("metiers")["Malkai-Chogall/Couture"].recettes == "2996,4343" and mk.envois == envoisAvant,
	"un filtre de la fenêtre ne fait oublier aucune recette (et rien ne repart)")

o.fenetre = { nom = "Enchantement", rang = 120, max = 150, recettes = { { nom = "Enchantement de bracelets (Vie mineure)", sort = 7418 } } }
declencher(o.env, "TRADE_SKILL_SHOW")
avancer(6)
verifier(mk.ns.fiches("metiers")["Ouranos-Chogall/Enchantement"].recettes == "s7418"
	and #mk.ns.chercherRecettes("bracelets") == 1, "un enchantement est noté par son sort et se retrouve par son nom")

o.competences = { { "Métiers", true }, { "Enchantement", false, 121, 150 }, { "Compétences secondaires", true },
	{ "Cuisine", false, 50, 75 }, { "Armes", true }, { "Épées", false, 100, 300 } }
o.ns.lireNiveaux()
avancer(6)
local fo = mk.ns.fiches("metiers")
verifier(fo["Ouranos-Chogall/Cuisine"] and fo["Ouranos-Chogall/Cuisine"].niveau == "50" and not fo["Ouranos-Chogall/Épées"]
	and fo["Ouranos-Chogall/Enchantement"].niveau == "121" and fo["Ouranos-Chogall/Enchantement"].recettes == "s7418",
	"les niveaux viennent des compétences (métiers seulement), les recettes sont gardées")

-- une demande d'artisanat : celui qui sait la fabriquer est prévenu
local cle3 = o.ns.publierDemande({ type = "A", objet = lien(4343), qte = 1 })
avancer(2)
verifier(derniereBulle(mk):find("tu sais le fabriquer", 1, true) and mk.ns.listeDemandes({ filtre = "pourmoi" })[1].cle == cle3,
	"Malkai, couturier, est prévenu d'une demande de pantalon")

-- ------------------------------------------------------------------ événements

verifier(o.ns.calendrierDisponible() and #o.ns.listeEvenements() == 1 and o.ns.listeEvenements()[1].titre == raid.titre,
	"les événements de guilde du calendrier sont lus")
verifier(not derniereBulle(o):find("Nouvel événement", 1, true), "la première lecture ne fait pas de bulle")
evenements[2] = { titre = "Sortie Mortemines", quand = W.now + 5 * 86400, id = 12, inscrits = {} }
declencher(o.env, "CALENDAR_UPDATE_GUILD_EVENTS")
avancer(2)
verifier(derniereBulle(o):find("Sortie Mortemines", 1, true), "un nouvel événement fait une bulle")
local ev = o.ns.listeEvenements()[1]
verifier(o.ns.repondreEvenement(ev.cle, "oui") == true, "répondre à un événement pas encore ouvert")
avancer(3)
verifier(o.reponse == "EventSignUp", "un événement à inscription prend « EventSignUp »")
verifier(o.ns.detailEvenement and o.ns.detailEvenement.compte.oui == 1 and o.ns.detailEvenement.description == "Réunion devant MC",
	"le détail est relu : description et inscrits")
evenements[3] = { titre = "Hier", quand = W.now - 86400, id = 13, inscrits = {} }
verifier(#o.ns.listeEvenements() == 2, "un événement passé n'est pas listé")
W.calendrier = nil
local sansCal = joueur("Sansaddon-Chogall")
connecter(sansCal)
avancer(15)
verifier(not sansCal.ns.calendrierDisponible() and #sansCal.ns.listeEvenements() == 0, "sans C_Calendar, la liste est vide et rien ne plante")
sansCal.connecte = false

-- ------------------------------------------------------------------ présence

o.ns.lireRoster() mk.ns.lireRoster()
verifier(mk.ns.estPublieurPresence() and not o.ns.estPublieurPresence(), "un seul publie la présence : le premier par ordre alphabétique")
local envoisO = o.envois
o.ns.releverPresence()
mk.ns.releverPresence()
avancer(2)
local jour = os.date("%Y%m%d", W.now)
local heure = tonumber(os.date("%H", W.now)) + 1
local fp = o.ns.fiches("presence")[jour]
verifier(o.envois == envoisO and fp and o.ns.decouper(fp.h, ",")[heure] == "4", "Malkai publie le relevé (4 en ligne), Ouranos se tait")
o.ns.ranger("presence", "20251101", 10, "X", { "3,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-" })
o.ns.ranger("presence", "20251101", 5, "Y", { "1,6,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-" })
verifier(o.ns.fiches("presence")["20251101"].h:sub(1, 4) == "3,6,", "deux relevés d'un même jour se fusionnent (le plus grand par heure)")
-- quatre mardis à 21h bien remplis
local fiches = o.ns.fiches("presence")
for k in pairs(fiches) do fiches[k] = nil end
for s = 1, 4 do
	local t = W.now - s * 7 * 86400
	local h = {}
	for i = 1, 24 do h[i] = "1" end
	h[22] = tostring(10 + s)
	fiches[os.date("%Y%m%d", t)] = { _maj = t, h = table.concat(h, ",") }
end
local best = o.ns.meilleursCreneaux(5)
local wday = tonumber(os.date("%w", W.now))
verifier(#best == 5 and best[1].heure == 21 and best[1].jour == (wday == 0 and 7 or wday) and math.abs(best[1].moyenne - 12.5) < 0.01,
	"semaine type : le meilleur créneau est le bon jour à 21h (12,5 en moyenne)")
fiches["20200101"] = { _maj = W.now, h = "1" }
o.ns.nettoyerMagasins()
verifier(not fiches["20200101"] and fiches[os.date("%Y%m%d", W.now - 28 * 86400)], "un jour de plus de 8 semaines est oublié, même fusionné récemment")

-- ------------------------------------------------------------------ quêtes

o.quetes = { { 101, "La Mine" }, { 102, "Les Défias" }, { 0, "Marche de l'Ouest", entete = true } }
mk.quetes = { { 102, "Les Défias" }, { 103, "Les Gnolls" } }
n.quetes = { { 103, "Les Gnolls" } }
declencher(o.env, "QUEST_LOG_UPDATE")
declencher(mk.env, "QUEST_LOG_UPDATE")
declencher(n.env, "QUEST_LOG_UPDATE")
avancer(10)
local qc = o.ns.quetesEnCommun()
verifier(#qc == 1 and qc[1].nom == "Malkai-Chogall" and qc[1].quetes[1].titre == "Les Défias", "Ouranos et Malkai ont « Les Défias » en commun")
local qm = mk.ns.quetesEnCommun()
verifier(#qm == 2 and #mk.ns.quetesPartagees() == 2, "Malkai partage une quête avec Ouranos et une avec Nenos")
verifier(derniereBulle(o):find("Les Défias", 1, true), "une bulle le dit")
local nb = #o.ns.bullesRecentes
declencher(mk.env, "QUEST_LOG_UPDATE")
mk.quetes[3] = { 104, "Autre" }
avancer(10)
verifier(#o.ns.bullesRecentes == nb, "pas deux fois la même bulle")
deconnecter(mk)
o.ns.lireRoster()
verifier(#o.ns.quetesEnCommun() == 0, "un membre hors ligne ne compte plus")

-- ------------------------------------------------------------------ lot 5 : ce que lit le client KFIRE

local photo = o.ns.db.roster
verifier(photo and photo.guilde == "Knights of Eternity" and photo.royaume == "Chogall" and photo.membres["Malkai-Chogall"]
	and photo.membres["Malkai-Chogall"].classe == "PALADIN" and photo.membres["Sansaddon-Chogall"].vu < W.now - 30 * 86400,
	"la photo de la liste de guilde est sur le disque (classe, dernière connexion)")
local prop = joueur("Trayn-Chogall")
prop.env.KFireGuildePropose = { pseudo = "TraynKF|r" }
connecter(prop)
avancer(1)
verifier(prop.ns.pseudoPropose() == "TraynKFr" and prop.ns.monProfil().pseudo == nil, "le pseudo proposé par le client est lu, sans remplacer le profil")
prop.ns.ouvrirProfil()
prop.connecte = false

-- ------------------------------------------------------------------ pages

for _, page in ipairs({ "demandes", "artisanat", "evenements", "stats", "quetes", "accueil" }) do
	o.ns.ouvrir(page)
	verifier(o.ns.pages[page]:IsShown(), "la page " .. page .. " s'ouvre")
end
o.ns.ouvrir("demandes")
local pd = o.ns.pages.demandes
verifier(pd.liste.lignes[1]._shown, "la page Demandes liste les demandes")
o.ns.ouvrir("artisanat")
verifier(o.ns.pages.artisanat.liste.lignes[1]._shown, "la page Artisanat liste les métiers")
o.env.SlashCmdList.KFIREGUILDE("quetes")
verifier(o.ns.pages.quetes:IsShown(), "/ke quetes ouvre la page Quêtes")


-- ------------------------------------------------------------------ aucune erreur avalée

for nom, j in pairs(W.joueurs) do
	verifier(j.ns.derniereErreur == nil, "aucune erreur Lua chez " .. nom .. (j.ns.derniereErreur and (" : " .. j.ns.derniereErreur) or ""))
end

out(echecs == 0 and "\ntout est vert" or ("\n" .. echecs .. " échec(s)"))
os.exit(echecs == 0 and 0 or 1)
