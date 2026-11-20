-- Appel d'Alliance : les annonces (F01, F02, F06, F07, F09, F11, F13 à F16).
--
-- Messages :
--   ANN~id~type~lieu~roles~places~niveau~texte~guilde~discord~expire   nouvelle annonce (canal)
--   UPD~id~places~roles~interesses                                    changement (canal)
--   DEL~id~motif                                                      fin : C complet, A annulé, E expiré
--   INT~id~etat~classe~niveau~role~guilde                             intérêt (chuchoté à l'initiateur), etat 1 / 0
--   SYNC_REQ~demandeur                                                « je reviens, quoi de neuf ? » (canal)
--   SYNC_Z~demandeur                                                  « je m'en occupe » (canal)
--   SA~auteur~… comme ANN …                                           une annonce du rattrapage (chuchotée)
--   SYNC_F~nombre                                                     fin du rattrapage (chuchoté)
-- L'id d'une annonce est « Auteur-Royaume:heure ». Il doit commencer par le nom de son auteur.
local _, ns = ...

local TEXTE_MAX = 120
local ENTRE_DEUX = 60            -- secondes minimum entre deux publications
local DELAI_REPONSE_MIN, DELAI_REPONSE_MAX = 0.5, 3.5
local VERIFICATION = 30          -- secondes entre deux passages d'expiration

ns.annonces = {}                 -- [id] = annonce
ns.mienne = nil                  -- id de mon annonce active
local dernierePublication = -1000

local function typeDe(code)
	for _, t in ipairs(ns.TYPES) do if t.code == code then return t end end
end
ns.typeDe = typeDe

function ns.actif()
	return ns.db ~= nil and ns.db.installe == true and ns.empechement() == nil
end

function ns.ignorerAlertes()
	return ns.db.reglages.nePasDeranger or ns.enInstance
end

-- ------------------------------------------------------------------ créer

local function nettoyer(t, max)
	t = tostring(t or ""):gsub("[~|\r\n]", " "):gsub("^%s+", ""):gsub("%s+$", "")
	if max and #t > max then t = t:sub(1, max) end
	return t
end

-- Pourquoi je ne peux pas publier (nil : je peux).
function ns.empechementPublier()
	if not ns.actif() then return "l'addon n'est pas actif sur ce personnage" end
	if ns.essai() then return "les comptes d'essai ne peuvent pas publier (ils reçoivent les annonces)" end
	if ns.mienne and ns.annonces[ns.mienne] then return "tu as déjà une annonce active : clôture-la d'abord" end
	local reste = ENTRE_DEUX - (ns.maintenant() - dernierePublication)
	if reste > 0 then return ("attends encore %d s avant de publier"):format(reste) end
	return nil
end

-- Une annonce active du même type au même endroit : mieux vaut la rejoindre.
function ns.doublon(code, lieu)
	if not lieu or lieu == "" then return nil end
	for id, a in pairs(ns.annonces) do
		if a.type == code and a.lieu and a.lieu:lower() == lieu:lower() and a.auteur ~= ns.moi() then return id end
	end
end

local function messageAnnonce(a)
	return { a.id, a.type, a.lieu or "", a.roles or "", a.places or "", a.niveau or "", a.texte or "",
		a.guilde, a.discord or "", a.expire, n = 11 }
end

local function envoyerAnnonce(a)
	ns.diffuser("ANN", messageAnnonce(a))
end

-- v : { type, lieu, roles = "TH", places, niveau, texte, ignorerDoublon }
-- Renvoie l'id, ou nil, message d'erreur [, id du doublon].
function ns.publierAnnonce(v)
	local raison = ns.empechementPublier()
	if raison then return nil, raison end
	local t = typeDe(v.type)
	if not t then return nil, "type d'annonce inconnu" end
	local lieu = nettoyer(v.lieu, 60)
	local texte = nettoyer(v.texte, TEXTE_MAX)
	if t.champs.lieu and lieu == "" then return nil, t.champs.lieu == "donjon" and "choisis un donjon" or "choisis une zone" end
	if t.code == "L" and texte == "" then return nil, "écris ce que tu cherches" end
	local roles = ""
	for _, r in ipairs({ "T", "H", "D" }) do
		if t.champs.roles and (v.roles or ""):find(r, 1, true) then roles = roles .. r end
	end
	local places = t.champs.places and tonumber(v.places) or nil
	if places and (places < 1 or places > 39) then return nil, "places entre 1 et 39" end
	local niveau = t.champs.niveau and tonumber(v.niveau) or nil
	if niveau and (niveau < 1 or niveau > 80) then return nil, "niveau entre 1 et 80" end
	if not v.ignorerDoublon then
		local d = ns.doublon(t.code, lieu)
		if d then return nil, "une annonce du même type existe déjà à cet endroit", d end
	end
	local maintenant = ns.maintenant()
	local a = {
		id = ns.moi() .. ":" .. maintenant, type = t.code, lieu = lieu ~= "" and lieu or nil,
		roles = roles ~= "" and roles or nil, places = places, niveau = niveau,
		texte = texte ~= "" and texte or nil, auteur = ns.moi(), guilde = ns.maGuilde(),
		discord = ns.db.discord, cree = maintenant, expire = maintenant + t.duree, interesses = {}, nbInteresses = 0,
	}
	if t.champs.lieu then ns.db.dernier[t.champs.lieu] = lieu end
	ns.annonces[a.id] = a
	ns.mienne = a.id
	dernierePublication = maintenant
	-- F15 : rédigée en instance, elle part à la sortie
	if ns.enInstance then
		a.differee = true
		ns.dire("tu es en instance : ton annonce partira à la sortie.")
	else
		envoyerAnnonce(a)
	end
	ns.signaler("annonces", a.id)
	return a.id
end

ns.surSignal("instance", function(dedans)
	if dedans then return end
	local a = ns.mienne and ns.annonces[ns.mienne]
	if a and a.differee then
		a.differee = nil
		a.expire = ns.maintenant() + typeDe(a.type).duree
		envoyerAnnonce(a)
		if ns.transport() == "B" then ns.dire("ton annonce est prête : clique sur un bouton de l'addon pour l'envoyer.") end
	end
end)

-- ------------------------------------------------------------------ clôturer

local MOTIFS = { C = "complet", A = "annulé", E = "expiré" }
ns.MOTIFS = MOTIFS

local function retirer(id, motif)
	local a = ns.annonces[id]
	if not a then return end
	ns.annonces[id] = nil
	if ns.mienne == id then ns.mienne = nil end
	ns.signaler("annonces", id, motif)
end

-- motif : "C" (complet) ou "A" (annulé)
function ns.cloturer(motif)
	local id = ns.mienne
	if not id or not ns.annonces[id] then return nil, "pas d'annonce active" end
	local differee = ns.annonces[id].differee
	retirer(id, motif)
	if not differee then ns.diffuser("DEL", { id, motif }) end
	return true
end

-- Changer places ou rôles de mon annonce (les intéressés aussi, en passant).
local majPrevue = false
local function envoyerMaj()
	local a = ns.mienne and ns.annonces[ns.mienne]
	if not a or a.differee then return end
	ns.diffuser("UPD", { a.id, a.places or "", a.roles or "", a.nbInteresses or 0 }, { remplace = "UPD" })
end

function ns.modifierAnnonce(v)
	local a = ns.mienne and ns.annonces[ns.mienne]
	if not a then return nil, "pas d'annonce active" end
	if v.places ~= nil then a.places = tonumber(v.places) end
	if v.roles ~= nil then a.roles = v.roles ~= "" and v.roles or nil end
	envoyerMaj()
	ns.signaler("annonces", a.id)
	return true
end

local function majBientot()
	if majPrevue then return end
	majPrevue = true
	ns.apres(5, function()
		majPrevue = false
		envoyerMaj()
	end)
end

-- ------------------------------------------------------------------ recevoir

local function auteurDe(id)
	return (id or ""):match("^(.-):%d+$")
end

-- Range une annonce reçue. alerter : false pendant un rattrapage.
local function ranger(auteur, c, alerter)
	local id, code = c[1], c[2]
	if auteurDe(id) ~= auteur or not typeDe(code) then ns.stats.ignores = ns.stats.ignores + 1 return end
	local guilde = c[8]
	if not ns.guilde(guilde) then ns.stats.ignores = ns.stats.ignores + 1 return end
	local expire = tonumber(c[10]) or 0
	if expire <= ns.maintenant() then return end
	local deja = ns.annonces[id]
	local a = deja or { interesses = {} }
	a.id, a.type, a.auteur, a.guilde = id, code, auteur, guilde
	a.lieu = c[3] ~= "" and c[3] or nil
	a.roles = c[4] ~= "" and c[4] or nil
	a.places = tonumber(c[5])
	a.niveau = tonumber(c[6])
	a.texte = c[7] ~= "" and c[7] or nil
	a.discord = c[9] ~= "" and c[9] or nil
	a.expire = math.min(expire, ns.maintenant() + 3 * 3600)
	a.cree = a.cree or tonumber(id:match(":(%d+)$")) or ns.maintenant()
	a.nbInteresses = a.nbInteresses or 0
	a.recue = ns.maintenant()
	ns.annonces[id] = a
	if deja then
		ns.signaler("annonces", id)
		return
	end
	-- F16, F12, F14, F13 : bloqué, type non suivi, hors niveau, ne pas déranger
	local r = ns.db.reglages
	local bloque = ns.db.bloques[auteur]
	local suivi = r.types[code] ~= false
	local niveauOk = not r.filtreNiveau or not a.niveau or ns.monNiveau() >= a.niveau
	a.cache = bloque or false
	ns.signaler("annonces", id)
	if bloque or not suivi then return end
	ns.signaler("nouvelle", a, alerter and niveauOk and not ns.ignorerAlertes())
end

ns.gestionnaires.ANN = function(expediteur, c, voie)
	if voie ~= "CHANNEL" then return end
	ranger(expediteur, c, true)
end

ns.gestionnaires.SA = function(expediteur, c, voie)
	if voie ~= "WHISPER" then return end
	local auteur = c[1]
	local reste = {}
	for i = 2, #c do reste[#reste + 1] = c[i] end
	if auteur == ns.moi() then return end
	if ns.rattrapage.vues then ns.rattrapage.vues[reste[1]] = true end
	ranger(auteur, reste, false)
end

ns.gestionnaires.UPD = function(expediteur, c)
	local a = ns.annonces[c[1]]
	if not a or a.auteur ~= expediteur then return end
	a.places = tonumber(c[2])
	a.roles = c[3] ~= "" and c[3] or nil
	a.nbInteresses = tonumber(c[4]) or a.nbInteresses
	ns.signaler("annonces", a.id)
end

ns.gestionnaires.DEL = function(expediteur, c)
	local a = ns.annonces[c[1]]
	if not a or a.auteur ~= expediteur then return end
	retirer(a.id, c[2])
end

-- L'initiateur quitte le canal (ou se déconnecte) : ses annonces tombent.
ns.surSignal("depart", function(nom)
	for id, a in pairs(ns.annonces) do
		if a.auteur == nom then retirer(id, "A") end
	end
end)

-- ------------------------------------------------------------------ intérêt

-- F06 : un clic envoie mon intérêt, un deuxième le retire.
function ns.interesser(id)
	local a = ns.annonces[id]
	if not a or a.auteur == ns.moi() then return nil, "annonce introuvable" end
	if ns.essai() then return nil, "les comptes d'essai ne peuvent pas répondre" end
	a.monInteret = not a.monInteret
	ns.chuchoter(a.auteur, "INT", { id, a.monInteret and 1 or 0, ns.maClasse() or "", ns.monNiveau(),
		ns.db.reglages.role or "", ns.maGuilde() or "", n = 6 })
	ns.signaler("annonces", id)
	return a.monInteret
end

ns.gestionnaires.INT = function(expediteur, c, voie)
	if voie ~= "WHISPER" then return end
	local a = ns.annonces[c[1]]
	if not a or a.auteur ~= ns.moi() then return end
	if not ns.guilde(c[6]) then return end
	local avant = a.interesses[expediteur]
	if c[2] == "1" then
		a.interesses[expediteur] = { classe = c[3] ~= "" and c[3] or nil, niveau = tonumber(c[4]),
			role = ns.ROLES[c[5]] and c[5] or nil, guilde = c[6], quand = ns.maintenant() }
		if not avant then ns.signaler("interesse", a, expediteur) end
	else
		a.interesses[expediteur] = nil
	end
	local n = 0
	for _ in pairs(a.interesses) do n = n + 1 end
	a.nbInteresses = n
	majBientot()
	ns.signaler("annonces", a.id)
end

-- Les intéressés de mon annonce : { { nom, classe, niveau, role, guilde, quand } }.
function ns.mesInteresses()
	local a = ns.mienne and ns.annonces[ns.mienne]
	local t = {}
	if not a then return t end
	for nom, i in pairs(a.interesses) do
		t[#t + 1] = { nom = nom, classe = i.classe, niveau = i.niveau, role = i.role, guilde = i.guilde, quand = i.quand }
	end
	table.sort(t, function(x, y) return (x.quand or 0) < (y.quand or 0) end)
	return t
end

-- ------------------------------------------------------------------ liste

-- F08 : options : { type = code ou nil }
function ns.listeAnnonces(options)
	options = options or {}
	local t = {}
	for _, a in pairs(ns.annonces) do
		if not a.cache and (not options.type or a.type == options.type) then t[#t + 1] = a end
	end
	table.sort(t, function(x, y)
		if (x.cree or 0) ~= (y.cree or 0) then return (x.cree or 0) > (y.cree or 0) end
		return x.id < y.id
	end)
	return t
end

function ns.nombreAnnonces()
	return #ns.listeAnnonces()
end

-- F16 : bloquer un joueur, ses annonces disparaissent.
function ns.bloquer(nom, oui)
	ns.db.bloques[nom] = oui and true or nil
	for _, a in pairs(ns.annonces) do
		if a.auteur == nom then a.cache = oui and true or false end
	end
	ns.signaler("annonces")
end

-- ------------------------------------------------------------------ expiration

function ns.expirer()
	local maintenant = ns.maintenant()
	for id, a in pairs(ns.annonces) do
		if a.expire and a.expire <= maintenant and not a.differee then
			if a.auteur == ns.moi() then ns.diffuser("DEL", { id, "E" }) end
			retirer(id, "E")
		end
	end
end

-- ------------------------------------------------------------------ rattrapage (F11)

local reponses = {}
ns.rattrapage = { demande = nil, recu = nil, servis = 0 }

function ns.demanderRattrapage()
	if not ns.actif() or not ns.dansLeCanal() then return end
	ns.rattrapage.demande = ns.maintenant()
	ns.rattrapage.vues = {}
	ns.diffuser("SYNC_REQ", { ns.moi() }, { remplace = "SYNC_REQ" })
end

local function servir(demandeur)
	local n = 0
	for _, a in pairs(ns.annonces) do
		if not a.differee and a.expire > ns.maintenant() then
			local m = messageAnnonce(a)
			table.insert(m, 1, a.auteur)
			m.n = 12
			ns.chuchoter(demandeur, "SA", m, { basse = true })
			n = n + 1
		end
	end
	ns.chuchoter(demandeur, "SYNC_F", { n }, { basse = true })
	ns.rattrapage.servis = ns.rattrapage.servis + 1
end

ns.gestionnaires.SYNC_REQ = function(expediteur, c, voie)
	if voie ~= "CHANNEL" or ns.nomComplet(c[1]) ~= expediteur then return end
	local r = { annule = false }
	reponses[expediteur] = r
	ns.apres(DELAI_REPONSE_MIN + math.random() * (DELAI_REPONSE_MAX - DELAI_REPONSE_MIN), function()
		if r.annule or reponses[expediteur] ~= r then return end
		reponses[expediteur] = nil
		ns.diffuser("SYNC_Z", { expediteur })
		servir(expediteur)
	end)
end

ns.gestionnaires.SYNC_Z = function(_, c)
	local r = reponses[ns.nomComplet(c[1]) or ""]
	if r then r.annule = true end
end

ns.gestionnaires.SYNC_F = function(expediteur, c, voie)
	if voie ~= "WHISPER" then return end
	ns.rattrapage.recu = { de = expediteur, nombre = tonumber(c[1]) or 0, date = ns.maintenant() }
	-- ce que le rattrapage ne contient pas a été clos pendant mon absence
	local vues = ns.rattrapage.vues
	if vues then
		for id, a in pairs(ns.annonces) do
			if not vues[id] and a.auteur ~= ns.moi() and (a.recue or 0) < (ns.rattrapage.demande or 0) then
				retirer(id, "E")
			end
		end
		ns.rattrapage.vues = nil
	end
	ns.signaler("annonces")
end

ns.surSignal("canal", function(ok)
	if ok then ns.apres(3, ns.demanderRattrapage) end
end)

ns.surSignal("instance", function(dedans)
	if not dedans then ns.apres(3, ns.demanderRattrapage) end
end)

ns.surSignal("charge", function()
	local function boucle()
		ns.expirer()
		ns.apres(VERIFICATION, boucle)
	end
	ns.apres(VERIFICATION, boucle)
end)
