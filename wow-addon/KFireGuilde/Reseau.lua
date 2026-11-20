-- KFIRE Guilde : messages d'addon sur le canal de guilde et en chuchotement.
--
-- Format d'un message : « 1~G~champ~champ… » (protocole, genre, champs).
-- Le « ~ » et les retours à la ligne d'un texte libre deviennent « - ».
-- Un message de plus de 255 octets part en morceaux : « 1~#~id~n°~total~bout ».
--
-- Le jeu limite le nombre de messages : une file unique, deux priorités
-- (le direct avant la synchro), et on réessaie une seconde plus tard quand
-- le jeu répond « trop de messages ». En instance, la file attend la sortie.
local _, ns = ...

local LONGUEUR_MAX = 255
local SEP = "~"
local REESSAI = 1        -- secondes avant de réessayer après un refus pour débit
local PAUSE = 0.15       -- entre deux messages, pour ne pas vider le crédit d'un coup
local MORCEAUX_VIE = 60  -- secondes avant d'oublier un message en morceaux incomplet

-- ------------------------------------------------------------------ codage

local function nettoyer(v)
	if v == nil then return "" end
	return (tostring(v):gsub("[~\r\n%z]", "-"))
end

function ns.coder(genre, ...)
	local t = { tostring(ns.PROTO), genre }
	for i = 1, select("#", ...) do t[#t + 1] = nettoyer((select(i, ...))) end
	return table.concat(t, SEP)
end

-- Découpe en gardant les champs vides : « a~~b » → { "a", "", "b" }.
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

-- Renvoie genre, { champs } ; nil, "version" pour un autre protocole.
function ns.decoder(msg)
	local t = ns.decouper(msg, SEP)
	if t[1] ~= tostring(ns.PROTO) then return nil, "version" end
	local genre = t[2]
	if not genre or genre == "" then return nil, "vide" end
	local champs = {}
	for i = 3, #t do champs[#champs + 1] = t[i] end
	return genre, champs
end

-- ------------------------------------------------------------------ morceaux

local compteur = 0
local function nouvelId()
	compteur = compteur + 1
	return ("%x%x"):format(math.random(0, 255), compteur % 4096)
end

function ns.morceler(msg)
	if #msg <= LONGUEUR_MAX then return { msg } end
	local id = nouvelId()
	-- l'entête la plus longue possible : « 1~#~id~999~999~ »
	local place = LONGUEUR_MAX - #ns.coder("#", id, 999, 999, "")
	local total = math.ceil(#msg / place)
	local bouts = {}
	for i = 1, total do
		local bout = msg:sub((i - 1) * place + 1, i * place)
		-- le bout n'est pas nettoyé : il contient des « ~ » du message d'origine
		bouts[i] = table.concat({ tostring(ns.PROTO), "#", id, tostring(i), tostring(total), bout }, SEP)
	end
	return bouts
end

local enCours = {}  -- [expediteur .. id] = { total, recus, bouts, date }

-- Rassemble les morceaux ; renvoie le message entier quand tout est arrivé.
function ns.rassembler(expediteur, msg)
	-- les 5 premiers champs seulement : le bout garde ses « ~ »
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

-- ------------------------------------------------------------------ file d'envoi

local fileHaute, fileBasse = {}, {}
local pompeArmee = false
ns.stats = { envoyes = 0, refusDebit = 0, echecs = 0, recus = 0, ignores = 0 }

local function resultatOk(r)
	return r == true or r == 0 or r == nil
end

-- Codes de Enum.SendAddonMessageResult qui veulent dire « trop de messages » :
-- on garde le message et on réessaie plus tard.
local function resultatDebit(r)
	return r == false or r == 3 or r == 8
end

local pomper
local function armer(delai)
	if pompeArmee then return end
	pompeArmee = true
	ns.apres(delai, function()
		pompeArmee = false
		pomper()
	end)
end

pomper = function()
	if ns.enInstance then return end  -- reprise au signal « instance » de sortie
	local file = #fileHaute > 0 and fileHaute or fileBasse
	local m = file[1]
	if not m then return end
	local envoi = (C_ChatInfo and C_ChatInfo.SendAddonMessage) or SendAddonMessage
	local ok, r = pcall(envoi, ns.PREFIXE, m.msg, m.voie, m.cible)
	if ok and resultatDebit(r) then
		ns.stats.refusDebit = ns.stats.refusDebit + 1
		armer(REESSAI)
		return
	end
	table.remove(file, 1)
	if ok and resultatOk(r) then
		ns.stats.envoyes = ns.stats.envoyes + 1
	else
		ns.stats.echecs = ns.stats.echecs + 1
		ns.erreur("envoi refusé (" .. tostring(r) .. ") vers " .. m.voie)
	end
	if #fileHaute + #fileBasse > 0 then armer(PAUSE) end
end

ns.surSignal("instance", function(dedans)
	if not dedans then armer(1) end
end)

-- options : { cible = "Nom-Royaume" } pour un chuchotement, { basse = true } pour la synchro
function ns.envoyer(genre, champs, options)
	options = options or {}
	local msg = ns.coder(genre, unpack(champs or {}))
	local voie = options.cible and "WHISPER" or "GUILD"
	local file = options.basse and fileBasse or fileHaute
	for _, bout in ipairs(ns.morceler(msg)) do
		file[#file + 1] = { msg = bout, voie = voie, cible = options.cible }
	end
	if not pompeArmee then pomper() end
end

function ns.enAttente()
	return #fileHaute + #fileBasse
end

-- ------------------------------------------------------------------ réception

ns.gestionnaires = {}       -- [genre] = function(expediteur, champs, voie)
ns.versionsInconnues = {}   -- [expediteur] = true : il parle un autre protocole

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
	ns.stats.recus = ns.stats.recus + 1
	fn(expediteur, champs, voie)
end
ns.traiter = traiter

ns.ecouter("CHAT_MSG_ADDON", function(prefixe, msg, voie, expediteur)
	if prefixe ~= ns.PREFIXE then return end
	if voie ~= "GUILD" and voie ~= "WHISPER" then return end
	expediteur = ns.nomComplet(expediteur)
	if not expediteur or expediteur == ns.moi() then return end
	-- Un chuchotement peut venir de n'importe qui : seuls les membres de la guilde comptent.
	if voie == "WHISPER" and not ns.estMembre(expediteur) then
		ns.stats.ignores = ns.stats.ignores + 1
		return
	end
	traiter(expediteur, msg, voie)
end)

ns.surSignal("charge", function()
	local inscrire = (C_ChatInfo and C_ChatInfo.RegisterAddonMessagePrefix) or RegisterAddonMessagePrefix
	inscrire(ns.PREFIXE)
end)
