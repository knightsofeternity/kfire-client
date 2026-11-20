-- Appel d'Alliance : le canal commun et les messages.
--
-- Ce qui concerne tout le monde passe par le canal personnalisé KEAlliance (protégé par
-- mot de passe, même faction seulement) ; ce qui ne concerne qu'une personne part en
-- chuchotement d'addon.
--
-- Deux façons de parler sur le canal (lot 0) :
--   A : messages d'addon de type CHANNEL. Invisibles, et l'addon peut les envoyer tout seul.
--   B : messages texte « [KEA]… » masqués du chat par un filtre. Le jeu ne les accepte que
--       pendant un clic du joueur : ils attendent le prochain clic sur un bouton de l'addon.
-- En « auto », on commence en A et on passe en B si le jeu refuse le type CHANNEL.
--
-- Format : « 1~G~champ~champ… » (protocole, genre, champs). Un « ~ », un « | » ou un retour à
-- la ligne dans un texte libre deviennent « - ». Plus de 255 octets : « 1~#~id~n°~total~bout ».
local _, ns = ...

local LONGUEUR_MAX = 255
local SEP = "~"
local REESSAI = 1
local PAUSE = 0.2
local MORCEAUX_VIE = 60

-- ------------------------------------------------------------------ codage

local function nettoyer(v)
	if v == nil then return "" end
	return (tostring(v):gsub("[~|\r\n%z]", "-"))
end
ns.nettoyerChamp = nettoyer

function ns.coder(genre, champs)
	local t = { tostring(ns.PROTO), genre }
	for i = 1, (champs.n or #champs) do t[#t + 1] = nettoyer(champs[i]) end
	return table.concat(t, SEP)
end

function ns.decouper(texte, sep)
	local t, debut = {}, 1
	while true do
		local i = texte:find(sep, debut, true)
		if not i then t[#t + 1] = texte:sub(debut) break end
		t[#t + 1] = texte:sub(debut, i - 1)
		debut = i + #sep
	end
	return t
end

function ns.decoder(msg)
	if type(msg) ~= "string" or ns.secret(msg) then return nil, "secret" end
	local t = ns.decouper(msg, SEP)
	if t[1] ~= tostring(ns.PROTO) then return nil, "version" end
	local genre = t[2]
	if not genre or genre == "" then return nil, "vide" end
	local champs = {}
	for i = 3, #t do champs[#champs + 1] = t[i] end
	return genre, champs
end

local compteur = 0
function ns.morceler(msg, max)
	max = max or LONGUEUR_MAX
	if #msg <= max then return { msg } end
	compteur = compteur + 1
	local id = ("%x%x"):format(math.random(0, 255), compteur % 4096)
	local place = max - #table.concat({ tostring(ns.PROTO), "#", id, "999", "999", "" }, SEP)
	local total = math.ceil(#msg / place)
	local bouts = {}
	for i = 1, total do
		bouts[i] = table.concat({ tostring(ns.PROTO), "#", id, tostring(i), tostring(total),
			msg:sub((i - 1) * place + 1, i * place) }, SEP)
	end
	return bouts
end

local enCours = {}
function ns.rassembler(expediteur, msg)
	local _, _, id, n, total, bout = msg:match("^([^~]*)~([^~]*)~([^~]*)~(%d+)~(%d+)~(.*)$")
	n, total = tonumber(n), tonumber(total)
	if not id or not n or not total or n < 1 or n > total or total > 999 then return nil end
	local cle = expediteur .. "/" .. id
	local maintenant = ns.maintenant()
	for k, e in pairs(enCours) do
		if maintenant - e.date > MORCEAUX_VIE then enCours[k] = nil end
	end
	local e = enCours[cle]
	if not e or e.total ~= total then
		e = { total = total, recus = 0, bouts = {}, date = maintenant }
		enCours[cle] = e
	end
	if not e.bouts[n] then
		e.bouts[n] = bout
		e.recus = e.recus + 1
	end
	if e.recus < e.total then return nil end
	enCours[cle] = nil
	return table.concat(e.bouts)
end

-- ------------------------------------------------------------------ le canal

function ns.nomCanal()
	return (ns.db and ns.db.canal) or ns.CANAL
end

function ns.motDePasse()
	return (ns.db and ns.db.motDePasse) or ns.MOT_DE_PASSE
end

function ns.numeroCanal()
	if not GetChannelName then return nil end
	local id = GetChannelName(ns.nomCanal())
	if ns.secret(id) then return nil end
	return (id and id > 0) and id or nil
end

function ns.dansLeCanal()
	return ns.numeroCanal() ~= nil
end

-- Rejoint le canal ; le signal « canal » dit si c'est fait.
function ns.rejoindreCanal()
	if ns.dansLeCanal() then
		ns.signaler("canal", true)
		return
	end
	if JoinChannelByName then JoinChannelByName(ns.nomCanal(), ns.motDePasse()) end
	ns.apres(2, function()
		local ok = ns.dansLeCanal()
		if not ok then ns.dire("impossible de rejoindre le canal " .. ns.nomCanal() .. " (mot de passe ?).") end
		ns.signaler("canal", ok)
	end)
end

-- ------------------------------------------------------------------ transport

-- "A" ou "B". En auto, le premier refus du type CHANNEL fait passer en B pour de bon.
function ns.transport()
	if ns.TRANSPORT == "A" or ns.TRANSPORT == "B" then return ns.TRANSPORT end
	return (ns.db and ns.db.transportDetecte) or "A"
end

local function refusType(r)
	-- Enum.SendAddonMessageResult : 4 = type de discussion invalide, 7 = canal invalide, 9 = erreur générale
	return r == 4 or r == 9
end

-- ------------------------------------------------------------------ file d'envoi (A et chuchotements)

local fileHaute, fileBasse = {}, {}
local pompeArmee = false
ns.stats = { envoyes = 0, refusDebit = 0, echecs = 0, recus = 0, ignores = 0, texte = 0 }

local function resultatOk(r) return r == true or r == 0 or r == nil end
local function resultatDebit(r) return r == false or r == 3 or r == 8 end

local pomper
local function armer(delai)
	if pompeArmee then return end
	pompeArmee = true
	ns.apres(delai, function()
		pompeArmee = false
		pomper()
	end)
end

-- Les messages du canal qui attendent un clic (option B), dans l'ordre.
local attenteClic = {}

pomper = function()
	if ns.enInstance then return end
	local file = #fileHaute > 0 and fileHaute or fileBasse
	local m = file[1]
	if not m then return end
	local cible = m.cible
	if m.voie == "CHANNEL" then
		cible = ns.numeroCanal()
		if not cible then
			-- pas (encore) dans le canal : on réessaie plus tard
			armer(5)
			return
		end
	end
	local envoi = (C_ChatInfo and C_ChatInfo.SendAddonMessage) or SendAddonMessage
	local ok, r = pcall(envoi, ns.PREFIXE, m.msg, m.voie, cible)
	if ok and resultatDebit(r) then
		ns.stats.refusDebit = ns.stats.refusDebit + 1
		armer(REESSAI)
		return
	end
	table.remove(file, 1)
	if ok and resultatOk(r) then
		ns.stats.envoyes = ns.stats.envoyes + 1
	elseif m.voie == "CHANNEL" and ns.TRANSPORT == "auto" and (not ok or refusType(r)) then
		-- le jeu refuse les messages d'addon sur un canal : on passe en texte (option B)
		ns.db.transportDetecte = "B"
		ns.dire("le jeu refuse les messages d'addon sur le canal : les annonces passeront en texte, au clic.")
		attenteClic[#attenteClic + 1] = m.entier
		for _, f in ipairs({ fileHaute, fileBasse }) do
			local garder = {}
			for _, x in ipairs(f) do
				if x.voie ~= "CHANNEL" then
					garder[#garder + 1] = x
				elseif x.premier and x.entier ~= m.entier then
					attenteClic[#attenteClic + 1] = x.entier
				end
			end
			for i = #f, 1, -1 do f[i] = nil end
			for i, x in ipairs(garder) do f[i] = x end
		end
		ns.signaler("transport", "B")
	else
		ns.stats.echecs = ns.stats.echecs + 1
		ns.erreur("envoi refusé (" .. tostring(ok and r or "erreur") .. ") vers " .. m.voie)
	end
	if #fileHaute + #fileBasse > 0 then armer(PAUSE) end
end

ns.surSignal("instance", function(dedans)
	if not dedans then armer(1) end
end)

local function mettreEnFile(msg, voie, cible, basse)
	local file = basse and fileBasse or fileHaute
	for i, bout in ipairs(ns.morceler(msg)) do
		file[#file + 1] = { msg = bout, voie = voie, cible = cible, entier = msg, premier = i == 1 }
	end
	if not pompeArmee then pomper() end
end

-- ------------------------------------------------------------------ option B : texte au clic

local function envoyerTexte(msg)
	local canal = ns.numeroCanal()
	if not canal or not SendChatMessage then return false end
	for _, bout in ipairs(ns.morceler(msg, LONGUEUR_MAX - #ns.MARQUE)) do
		local ok, err = pcall(SendChatMessage, ns.MARQUE .. bout, "CHANNEL", nil, canal)
		if not ok then
			ns.erreur("message texte refusé : " .. tostring(err))
			return false
		end
		ns.stats.texte = ns.stats.texte + 1
	end
	return true
end

-- Chaque bouton de l'addon appelle ceci en premier : c'est le seul moment où le jeu
-- accepte un message texte sur le canal. Les messages en attente partent alors.
function ns.auClic()
	if #attenteClic == 0 or ns.enInstance or not ns.dansLeCanal() then return end
	local a = attenteClic
	attenteClic = {}
	for _, msg in ipairs(a) do envoyerTexte(msg) end
end

function ns.enAttenteDeClic()
	return #attenteClic
end

-- ------------------------------------------------------------------ envoyer

-- À tout le canal. options : { basse = true } pour le rattrapage, { remplace = clé } pour ne
-- garder qu'un message de ce genre en attente de clic (ex. un seul HELLO).
function ns.diffuser(genre, champs, options)
	options = options or {}
	local msg = ns.coder(genre, champs)
	if ns.transport() == "A" then
		mettreEnFile(msg, "CHANNEL", nil, options.basse)
	else
		if options.remplace then
			for i = #attenteClic, 1, -1 do
				if attenteClic[i]:find("^%d+~" .. options.remplace .. "~") then table.remove(attenteClic, i) end
			end
		end
		attenteClic[#attenteClic + 1] = msg
	end
end

-- À une seule personne (chuchotement d'addon, toujours possible).
function ns.chuchoter(cible, genre, champs, options)
	mettreEnFile(ns.coder(genre, champs), "WHISPER", cible, options and options.basse)
end

function ns.enAttente()
	return #fileHaute + #fileBasse
end

-- ------------------------------------------------------------------ réception

ns.gestionnaires = {}       -- [genre] = function(expediteur, champs, voie)
ns.versionsInconnues = {}

local function traiter(expediteur, msg, voie)
	local genre, champs = ns.decoder(msg)
	if not genre then
		if champs == "version" then ns.versionsInconnues[expediteur] = true end
		ns.stats.ignores = ns.stats.ignores + 1
		return
	end
	if genre == "#" then
		local entier = ns.rassembler(expediteur, msg)
		if entier then traiter(expediteur, entier, voie) end
		return
	end
	local fn = ns.gestionnaires[genre]
	if not fn then ns.stats.ignores = ns.stats.ignores + 1 return end
	-- en instance, l'addon est sourd : le rattrapage complétera à la sortie
	if ns.enInstance then ns.stats.ignores = ns.stats.ignores + 1 return end
	ns.stats.recus = ns.stats.recus + 1
	fn(expediteur, champs, voie)
end
ns.traiter = traiter

local function estNotreCanal(nom)
	if not nom or ns.secret(nom) then return false end
	local base = nom:gsub("^%d+%.%s*", "")
	return base:lower() == ns.nomCanal():lower()
end

ns.ecouter("CHAT_MSG_ADDON", function(prefixe, msg, voie, expediteur, _, _, _, nomCanal)
	if prefixe ~= ns.PREFIXE or ns.secret(msg) or ns.secret(expediteur) then return end
	if voie ~= "CHANNEL" and voie ~= "WHISPER" then return end
	if voie == "CHANNEL" and nomCanal and not estNotreCanal(nomCanal) then return end
	expediteur = ns.nomComplet(expediteur)
	if not expediteur or expediteur == ns.moi() then return end
	traiter(expediteur, msg, voie)
end)

ns.ecouter("CHAT_MSG_CHANNEL", function(texte, expediteur, _, _, _, _, _, _, nomBase)
	if ns.secret(texte) or type(texte) ~= "string" or texte:sub(1, #ns.MARQUE) ~= ns.MARQUE then return end
	if nomBase and not estNotreCanal(nomBase) then return end
	expediteur = ns.nomComplet(expediteur)
	if not expediteur or expediteur == ns.moi() then return end
	traiter(expediteur, texte:sub(#ns.MARQUE + 1), "CHANNEL")
end)

-- Un joueur quitte le canal (déconnexion comprise) : ses annonces tombent.
ns.ecouter("CHAT_MSG_CHANNEL_LEAVE", function(_, joueur, _, _, _, _, _, _, nomBase)
	if ns.secret(joueur) or (nomBase and not estNotreCanal(nomBase)) then return end
	local nom = ns.nomComplet(joueur)
	if nom and nom ~= ns.moi() then ns.signaler("depart", nom) end
end)

-- Les messages codés de l'option B ne s'affichent jamais, même si le canal est dans une fenêtre.
local function filtre(_, _, texte)
	return type(texte) == "string" and not ns.secret(texte) and texte:sub(1, #ns.MARQUE) == ns.MARQUE
end
local ajouterFiltre = (ChatFrameUtil and ChatFrameUtil.AddMessageEventFilter) or ChatFrame_AddMessageEventFilter
if ajouterFiltre then ajouterFiltre("CHAT_MSG_CHANNEL", filtre) end

ns.surSignal("charge", function()
	local inscrire = (C_ChatInfo and C_ChatInfo.RegisterAddonMessagePrefix) or RegisterAddonMessagePrefix
	inscrire(ns.PREFIXE)
end)

