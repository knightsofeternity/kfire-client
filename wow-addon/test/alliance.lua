-- Banc hors jeu d'Appel d'Alliance. Lua 5.1, comme WoW.
--
-- Des joueurs KE et Cassos simulés, chacun avec son addon (son propre environnement Lua),
-- sur un canal personnalisé factice protégé par mot de passe. Le temps est simulé.
-- Le jeu peut refuser les messages d'addon sur un canal (W.canalInterdit) : on vérifie
-- alors l'option B (texte codé, au clic seulement).

local echecs = 0
local function out(s) io.write(s, "\n") end
local function verifier(cond, msg)
	if cond then out("ok   " .. msg) else echecs = echecs + 1 out("FAIL: " .. msg) end
end

local FICHIERS = {}
for l in io.lines("AppelAlliance/AppelAlliance.toc") do
	if l:match("%.lua$") then FICHIERS[#FICHIERS + 1] = "AppelAlliance/" .. l end
end

local W
local function nouveauMonde(opts)
	opts = opts or {}
	W = { now = 1763600000, minuteurs = {}, joueurs = {}, canal = {}, canalInterdit = opts.canalInterdit,
		motDePasse = nil, texte = 0 }
	math.randomseed(7)
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

local function cadre(env)
	local o = { _scripts = {}, _shown = false, _text = "", _checked = false, _focus = false }
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
	o.HasFocus = function(self) return self._focus end
	o.GetParent = function(self) return self._parent end
	o.GetPoint = function() return "TOP", nil, "TOP", 10, -100 end
	o.GetCenter = function() return 0, 0 end
	o.GetEffectiveScale = function() return 1 end
	env._cadres[#env._cadres + 1] = o
	return setmetatable(o, { __index = function(_, k)
		return function(self, ...)
			if k:find("^Create") then
				local c = cadre(env)
				c._parent = self
				return c
			end
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

local NUMERO = 5

local function auxMembresDuCanal(de, fn)
	for nom, j in pairs(W.joueurs) do
		if j.connecte and W.canal[nom] and nom ~= de then programmer(0.05, function() if j.connecte then fn(j) end end) end
	end
end

local function joueur(nom, opts)
	opts = opts or {}
	local j = { nom = nom, connecte = false, instance = false, imprime = {}, envois = 0, opts = opts, clic = false }
	local env = setmetatable({ _cadres = {} }, { __index = _G })
	env._G = env
	j.env = env
	local prenom, royaume = nom:match("^([^-]+)-(.+)$")
	env.print = function(s) j.imprime[#j.imprime + 1] = s end
	env.time = function(t) if t then return os.time(t) end return W.now end
	env.GetServerTime = env.time
	env.date = os.date
	env.C_Timer = { After = programmer }
	env.CreateFrame = function(_, _, parent)
		local c = cadre(env)
		c._parent = parent
		return c
	end
	env.UIParent = cadre(env)
	env.Minimap = cadre(env)
	env.GameTooltip = cadre(env)
	env.UISpecialFrames = {}
	env.tinsert = table.insert
	env.RAID_CLASS_COLORS = { MAGE = { colorStr = "ff3fc7eb" } }
	env.SOUNDKIT = { RAID_WARNING = 8959, TELL_MESSAGE = 3081, READY_CHECK = 8960, UI_BNET_TOAST = 18019, IG_QUEST_LIST_OPEN = 875 }
	env.PlaySound = function(id) j.sons = (j.sons or 0) + 1 j.dernierSon = id end
	env.UnitFullName = function() return prenom, royaume end
	env.GetNormalizedRealmName = function() return royaume end
	env.UnitFactionGroup = function() return opts.faction or "Alliance" end
	env.UnitLevel = function() return opts.niveau or 20 end
	env.UnitClass = function() return "Mage", "MAGE" end
	env.IsInGuild = function() return opts.guilde ~= false end
	env.GetGuildInfo = function() if opts.guilde == false then return nil end return opts.guilde or "Knights of Eternity" end
	env.IsTrialAccount = function() return opts.essai or false end
	env.IsInInstance = function() return j.instance, j.instance and "party" or "none" end
	env.GetZoneText = function() return "Marche de l'Ouest" end
	env.SlashCmdList = {}
	env.hooksecurefunc = function() end
	env.GetChannelName = function(n)
		if n == "KEAlliance" and W.canal[nom] then return NUMERO, "KEAlliance" end
		return 0
	end
	env.JoinChannelByName = function(n, mdp)
		if n ~= "KEAlliance" then return end
		W.motDePasse = W.motDePasse or mdp
		if mdp == W.motDePasse then W.canal[nom] = true end
	end
	env.SendChatMessage = function(msg, voie, _, numero)
		assert(#msg <= 255, "message texte de plus de 255 octets")
		if voie == "CHANNEL" and not j.clic then error("protégé : il faut un clic") end
		if voie ~= "CHANNEL" or numero ~= NUMERO or not W.canal[nom] then return end
		W.texte = W.texte + 1
		auxMembresDuCanal(nom, function(d)
			declencher(d.env, "CHAT_MSG_CHANNEL", msg, nom, "", "5. KEAlliance", "", "", 0, NUMERO, "KEAlliance")
		end)
	end
	env.C_ChatInfo = {
		RegisterAddonMessagePrefix = function() end,
		SendAddonMessage = function(_, msg, voie, cible)
			assert(#msg <= 255, "message de plus de 255 octets")
			if opts.essai then return 9 end
			if voie == "CHANNEL" then
				if W.canalInterdit then return 4 end
				if cible ~= NUMERO or not W.canal[nom] then return 7 end
				j.envois = j.envois + 1
				auxMembresDuCanal(nom, function(d)
					declencher(d.env, "CHAT_MSG_ADDON", "KEALLY", msg, "CHANNEL", nom, "", 0, NUMERO, "KEAlliance")
				end)
				return 0
			end
			if voie == "WHISPER" then
				j.envois = j.envois + 1
				local d = W.joueurs[cible]
				if d and d.connecte then
					programmer(0.05, function() declencher(d.env, "CHAT_MSG_ADDON", "KEALLY", msg, "WHISPER", nom) end)
				end
				return 0
			end
			return 4
		end,
	}
	env.C_PartyInfo = { InviteUnit = function(n) j.invite = n end }
	env.ChatFrame_SendTell = function(n) j.chuchote = n end
	env.AppelAllianceDB = opts.db
	env.AppelAllianceCompte = opts.compte
	j.ns = {}
	for _, f in ipairs(FICHIERS) do
		local chunk = assert(loadfile(f))
		setfenv(chunk, env)
		chunk("AppelAlliance", j.ns)
	end
	W.joueurs[nom] = j
	return j
end

local function connecter(j)
	j.connecte = true
	declencher(j.env, "ADDON_LOADED", "AppelAlliance")
	declencher(j.env, "PLAYER_ENTERING_WORLD", true, false)
end

local function deconnecter(j)
	j.connecte = false
	W.canal[j.nom] = nil
	for _, d in pairs(W.joueurs) do
		if d.connecte and W.canal[d.nom] then
			declencher(d.env, "CHAT_MSG_CHANNEL_LEAVE", "", j.nom, "", "5. KEAlliance", "", "", 0, NUMERO, "KEAlliance")
		end
	end
end

-- Un clic sur un bouton de l'addon (le jeu accepte alors un message texte sur le canal).
local function cliquer(j, fn)
	j.clic = true
	j.ns.auClic()
	local r = { fn() }
	j.clic = false
	return unpack(r)
end

-- L'accueil : rejoindre l'alliance comme le ferait le bouton.
local function installer(j, discord)
	avancer(7)
	local a = j.ns.accueil()
	a.discord:SetText(discord or "")
	j.clic = true
	a.rejoindre._scripts.OnClick(a.rejoindre)
	j.clic = false
	avancer(6)
end

local function derniereLigne(j) return j.imprime[#j.imprime] or "" end
local function contient(t, s) return t:find(s, 1, true) ~= nil end

-- ================================================================== option A

nouveauMonde()
local sym = joueur("Symbiiote-Chogall", { niveau = 22 })
local lumi = joueur("Lumi-Chogall", { guilde = "Les Cassos de la Lumière", niveau = 18 })
connecter(sym) connecter(lumi)
avancer(7)
verifier(sym.ns.accueil() and sym.ns.accueil():IsShown(), "premier lancement : la fenêtre de bienvenue s'ouvre")
verifier(contient(sym.ns.accueil().verifs:GetText(), "Faction : Alliance") and contient(sym.ns.accueil().verifs:GetText(), "Knights of Eternity"),
	"elle vérifie faction et guilde")
installer(sym, "Symbiiote#1234")
installer(lumi, "Lumi")
verifier(W.canal["Symbiiote-Chogall"] and W.canal["Lumi-Chogall"] and sym.ns.db.installe, "« Rejoindre l'alliance » rejoint le canal, mot de passe fourni par l'addon")
verifier(sym.ns.db.discord == "Symbiiote#1234" and sym.ns.compte.discord == "Symbiiote#1234", "le pseudo Discord est gardé (et proposé aux autres persos)")
avancer(5)
local membres = lumi.ns.listeMembres({})
verifier(#membres == 1 and membres[1].nom == "Symbiiote-Chogall" and membres[1].enLigne and membres[1].discord == "Symbiiote#1234",
	"onglet Alliance : Lumi voit Symbiiote en ligne, avec son Discord (HELLO)")
verifier(#sym.ns.listeMembres({ filtre = "Cassos" }) == 1 and #sym.ns.listeMembres({ filtre = "KE" }) == 0, "filtre par guilde")

-- publier
verifier(select(2, sym.ns.publierAnnonce({ type = "D", lieu = "" })) == "choisis un donjon", "un donjon sans lieu est refusé")
local id = cliquer(sym, function()
	return sym.ns.publierAnnonce({ type = "D", lieu = "Les Mortemines", roles = "H", niveau = 18, texte = "il manque un heal" })
end)
verifier(id and id:find("^Symbiiote%-Chogall:"), "Symbiiote publie une annonce donjon")
avancer(2)
local a = lumi.ns.annonces[id]
verifier(a and a.lieu == "Les Mortemines" and a.roles == "H" and a.niveau == 18 and a.guilde == "Knights of Eternity"
	and a.discord == "Symbiiote#1234", "Lumi (Cassos) la reçoit, avec guilde et Discord de l'initiateur")
verifier(lumi.sons == 1 and lumi.ns.toastsVisibles[1] == a, "alerte : un son et un toast")
local ligne = ""
for _, l in ipairs(lumi.imprime) do if contient(l, "garrmission:appelalliance:" .. id) then ligne = l end end
verifier(contient(ligne, "Les Mortemines") and contient(ligne, "[KE]"), "et une ligne cliquable dans le chat")
verifier(select(2, sym.ns.publierAnnonce({ type = "L", texte = "x" })):find("déjà une annonce active"), "une seule annonce active par personnage")

-- doublon
local _, err, d = lumi.ns.publierAnnonce({ type = "D", lieu = "les mortemines" })
verifier(d == id and contient(err, "existe déjà"), "doublon : on propose de rejoindre l'annonce existante")

-- intérêt
verifier(lumi.ns.interesser(id) == true, "Lumi clique « Intéressé »")
avancer(1)
local ints = sym.ns.mesInteresses()
verifier(#ints == 1 and ints[1].nom == "Lumi-Chogall" and ints[1].guilde == "Les Cassos de la Lumière" and ints[1].niveau == 18,
	"Symbiiote voit Lumi dans ses intéressés (guilde, niveau)")
verifier(sym.dernierSon == 3081, "un son discret pour l'initiateur")
avancer(6)
verifier(lumi.ns.annonces[id].nbInteresses == 1, "le nombre d'intéressés est diffusé")
lumi.ns.interesser(id)
avancer(1)
verifier(#sym.ns.mesInteresses() == 0, "un deuxième clic retire l'intérêt")

-- quelqu'un qui arrive : rattrapage
local servisAvant = sym.ns.rattrapage.servis + lumi.ns.rattrapage.servis
local nox = joueur("Nox-Chogall", { niveau = 10 })
connecter(nox)
installer(nox)
avancer(10)
verifier(nox.ns.annonces[id] and not nox.ns.toastsVisibles[1], "un joueur qui arrive récupère l'annonce, sans alerte")
verifier(sym.ns.rattrapage.servis + lumi.ns.rattrapage.servis == servisAvant + 1, "un seul joueur répond au rattrapage")

-- en instance : rien, puis rattrapage à la sortie
lumi.instance = true
declencher(lumi.env, "PLAYER_ENTERING_WORLD", false, false)
cliquer(sym, function() return sym.ns.cloturer("C") end)
avancer(2)
verifier(lumi.ns.annonces[id] ~= nil, "en instance, Lumi n'entend pas la clôture")
local id2 = cliquer(nox, function() return nox.ns.publierAnnonce({ type = "Q", lieu = "Marche de l'Ouest", niveau = 12 }) end)
avancer(2)
verifier(not lumi.ns.annonces[id2], "ni la nouvelle annonce")
verifier(sym.ns.annonces[id2] ~= nil, "Symbiiote, lui, la reçoit")
lumi.instance = false
declencher(lumi.env, "PLAYER_ENTERING_WORLD", false, false)
avancer(10)
verifier(lumi.ns.annonces[id2] and not lumi.ns.annonces[id], "à la sortie, le rattrapage apporte la nouvelle et retire la close")

-- F14 : au-dessus de mon niveau, pas d'alerte (mais dans la liste)
local sonsNox = nox.sons or 0
avancer(61)
local id3 = cliquer(sym, function() return sym.ns.publierAnnonce({ type = "D", lieu = "Uldaman", niveau = 41 }) end)
avancer(2)
verifier(nox.ns.annonces[id3] and (nox.sons or 0) == sonsNox, "une annonce au-dessus de mon niveau arrive sans alerte")

-- F13 : ne pas déranger
lumi.ns.db.reglages.nePasDeranger = true
local sonsLumi = lumi.sons or 0
cliquer(sym, function() return sym.ns.cloturer("A") end)
avancer(61)
local id4 = cliquer(sym, function() return sym.ns.publierAnnonce({ type = "L", texte = "Qui a un forgeron ?" }) end)
avancer(2)
verifier(lumi.ns.annonces[id4] and (lumi.sons or 0) == sonsLumi, "Ne pas déranger : dans la liste, sans son ni toast")
lumi.ns.db.reglages.nePasDeranger = false

-- 60 s entre deux publications
cliquer(sym, function() return sym.ns.cloturer("A") end)
verifier(contient(select(2, sym.ns.publierAnnonce({ type = "L", texte = "encore" })) or "", "attends encore"), "60 s minimum entre deux publications")

-- F16 : bloquer
lumi.ns.bloquer("Nox-Chogall", true)
verifier(#lumi.ns.listeAnnonces() == 0 or lumi.ns.listeAnnonces()[1].auteur ~= "Nox-Chogall", "un joueur bloqué disparaît de la liste")
lumi.ns.bloquer("Nox-Chogall", false)

-- expiration
avancer(46 * 60)
verifier(not sym.ns.annonces[id2] and not lumi.ns.annonces[id2], "une annonce expire toute seule (quête : 45 min)")

-- l'initiateur se déconnecte : son annonce tombe
avancer(61)
local id5 = cliquer(nox, function() return nox.ns.publierAnnonce({ type = "E", lieu = "Strangleronce", places = 3 }) end)
avancer(2)
verifier(lumi.ns.annonces[id5] and lumi.ns.annonces[id5].places == 3, "annonce élite avec places")
deconnecter(nox)
avancer(1)
verifier(not lumi.ns.annonces[id5] and not lumi.ns.listeMembres({ filtre = "enligne" })[3], "déconnexion de l'initiateur : annonce retirée, membre hors ligne")
local hors = nil
for _, m in ipairs(lumi.ns.listeMembres({})) do if m.nom == "Nox-Chogall" then hors = m end end
verifier(hors and not hors.enLigne, "il reste dans l'onglet Alliance, « vu il y a… »")

-- F15 : rédigée en instance, envoyée à la sortie
sym.instance = true
declencher(sym.env, "PLAYER_ENTERING_WORLD", false, false)
avancer(61)
local id6 = cliquer(sym, function() return sym.ns.publierAnnonce({ type = "L", texte = "après le donjon, on refait" }) end)
avancer(2)
verifier(id6 and not lumi.ns.annonces[id6], "en instance, l'annonce attend")
sym.instance = false
declencher(sym.env, "PLAYER_ENTERING_WORLD", false, false)
avancer(3)
verifier(lumi.ns.annonces[id6] ~= nil, "elle part à la sortie")

-- F17 : une version plus récente circule
declencher(lumi.env, "CHAT_MSG_ADDON", "KEALLY", "1~HELLO~9.0.0~Knights of Eternity~MAGE~30~D~~Hurlevent", "CHANNEL",
	"Futur-Chogall", "", 0, NUMERO, "KEAlliance")
local vu = false
for _, l in ipairs(lumi.imprime) do if contient(l, "version plus récente") then vu = true end end
verifier(vu, "une version plus récente est signalée")

-- garde-fous
declencher(lumi.env, "CHAT_MSG_ADDON", "KEALLY", "1~ANN~Pirate-Chogall:1~L~~~~~salut~La Horde Unie~~" .. (W.now + 600),
	"CHANNEL", "Pirate-Chogall", "", 0, NUMERO, "KEAlliance")
verifier(not lumi.ns.annonces["Pirate-Chogall:1"], "une annonce d'une guilde hors alliance est ignorée")
declencher(lumi.env, "CHAT_MSG_ADDON", "KEALLY", "1~DEL~" .. id6 .. "~A", "CHANNEL", "Pirate-Chogall", "", 0, NUMERO, "KEAlliance")
verifier(lumi.ns.annonces[id6] ~= nil, "seul l'initiateur peut clore son annonce")
declencher(lumi.env, "CHAT_MSG_ADDON", "KEALLY", "2~ANN~x", "CHANNEL", "Symbiiote-Chogall", "", 0, NUMERO, "KEAlliance")
verifier(lumi.ns.versionsInconnues["Symbiiote-Chogall"], "un autre protocole est repéré et ignoré")

-- interface
lumi.ns.ouvrir("annonces")
local pa = lumi.ns.pages.annonces
verifier(pa:IsShown() and pa.liste.lignes[1]._shown, "la fenêtre liste les annonces")
for _, o in ipairs({ "miennes", "alliance", "reglages" }) do
	lumi.ns.ouvrir(o)
	verifier(lumi.ns.pages[o]:IsShown(), "l'onglet " .. o .. " s'ouvre")
end
sym.ns.ouvrir("miennes")
verifier(contient(sym.ns.pages.miennes.titre:GetText(), "Libre"), "Mes annonces montre l'annonce en cours")
lumi.ns.ouvrirFormulaire()
local f = lumi.ns.formulaire()
verifier(f:IsShown() and contient(f.choix.lignes[1].nom:GetText(), "17+") and contient(f.choix.lignes[3].nom:GetText(), "13+"),
	"formulaire : les donjons de mon niveau d'abord, les plus proches en tête (niveau 18)")
f.lieu:SetText("morte")
f.lieu._scripts.OnTextChanged(f.lieu, true)
verifier(contient(f.choix.lignes[1].nom:GetText(), "Les Mortemines") and not f.choix.lignes[2]._shown, "taper filtre la liste")
f.choix.lignes[1]._scripts.OnClick(f.choix.lignes[1])
verifier(f.lieu:GetText() == "Les Mortemines" and f.niveau:GetText() == "17", "un clic choisit le donjon et son niveau")
lumi.clic = true
local idf = lumi.ns.publierDepuisFormulaire(false)
lumi.clic = false
verifier(idf ~= nil and not f:IsShown(), "« Publier » : trois clics pour un donjon")
lumi.ns.ouvrirFiche(id6)
lumi.env.SlashCmdList.APPELALLIANCE("dnd")
verifier(lumi.ns.db.reglages.nePasDeranger, "/alliance dnd")
lumi.env.SlashCmdList.APPELALLIANCE("diag")
verifier(contient(derniereLigne(lumi), "Dernière erreur"), "/alliance diag")

-- ================================================================== Horde, autre guilde, essai

local grom = joueur("Grom-Chogall", { faction = "Horde", guilde = "La Horde Unie" })
connecter(grom)
avancer(8)
verifier(not grom.ns.actif() and not W.canal["Grom-Chogall"] and contient(table.concat(grom.imprime, " "), "Horde"),
	"côté Horde : message clair, l'addon reste inactif")
local autre = joueur("Autre-Chogall", { guilde = "Les Autres" })
connecter(autre)
avancer(8)
verifier(not autre.ns.actif() and contient(table.concat(autre.imprime, " "), "ne fait pas partie"), "autre guilde : inactif, avec une explication")
local essai = joueur("Essai-Chogall", { essai = true })
connecter(essai)
installer(essai)
verifier(contient(essai.ns.accueil().verifs:GetText(), "Compte d'essai"), "l'accueil prévient un compte d'essai")
verifier(contient(select(2, essai.ns.publierAnnonce({ type = "L", texte = "x" })) or "", "comptes d'essai"), "un compte d'essai ne publie pas")

-- au retour, le canal est rejoint tout seul
deconnecter(sym)
local sym2 = joueur("Symbiiote-Chogall", { niveau = 22, db = sym.env.AppelAllianceDB, compte = sym.env.AppelAllianceCompte })
connecter(sym2)
avancer(10)
verifier(W.canal["Symbiiote-Chogall"] and not (sym2.ns.accueil() and sym2.ns.accueil():IsShown()),
	"à la connexion suivante, le canal est rejoint tout seul, sans fenêtre")

-- ================================================================== option B : le jeu refuse le type CHANNEL

nouveauMonde({ canalInterdit = true })
local k = joueur("Kael-Chogall")
local c = joueur("Cass-Chogall", { guilde = "Les Cassos de la Lumière" })
connecter(k) connecter(c)
installer(k) installer(c)
avancer(5)
verifier(k.ns.transport() == "B" and c.ns.transport() == "B", "le refus du type CHANNEL fait passer en option B")
verifier(k.ns.enAttenteDeClic() > 0, "les messages du canal attendent un clic")
local idb = cliquer(k, function() return k.ns.publierAnnonce({ type = "D", lieu = "La Prison", roles = "TD" }) end)
verifier(k.ns.enAttenteDeClic() == 1, "en B, l'annonce elle-même attend un clic (le HELLO est parti au clic de Publier)")
cliquer(k, function() end)
avancer(2)
verifier(c.ns.annonces[idb] and c.ns.annonces[idb].roles == "TD", "au clic suivant, l'annonce part en texte et arrive décodée")
verifier(W.texte >= 2, "les messages sont passés en texte sur le canal")
c.ns.interesser(idb)
avancer(1)
verifier(#k.ns.mesInteresses() == 1, "l'intérêt passe toujours en chuchotement d'addon")
local long = cliquer(k, function()
	k.ns.cloturer("C")
	return true
end)
avancer(2)
verifier(c.ns.annonces[idb] ~= nil, "la clôture attend elle aussi un clic")
cliquer(k, function() end)
avancer(2)
verifier(not c.ns.annonces[idb], "puis passe")

-- ================================================================== aucune erreur avalée

for nom, j in pairs(W.joueurs) do
	verifier(j.ns.derniereErreur == nil, "aucune erreur Lua chez " .. nom .. (j.ns.derniereErreur and (" : " .. j.ns.derniereErreur) or ""))
end

out(echecs == 0 and "\ntout est vert" or ("\n" .. echecs .. " échec(s)"))
os.exit(echecs == 0 and 0 or 1)
