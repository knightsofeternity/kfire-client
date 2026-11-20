-- KFIRE Guilde : les fiches partagées par toute la guilde, et leur synchronisation.
--
-- Un « magasin » range des fiches par clé (un membre, une demande…). Chaque fiche
-- porte sa date de mise à jour (heure du serveur) et son auteur ; entre deux
-- versions d'une même fiche, la plus récente gagne. Les fiches sont gardées sur
-- disque : un membre hors ligne reste visible, et ceux qui sont connectés
-- relaient ses fiches à ceux qui arrivent.
--
-- Messages :
--   R~magasin~cle~maj~auteur~champs…  une fiche (canal de guilde, ou chuchotée pendant une synchro)
--   S~depuis~demandeur                « je viens d'arriver, envoyez-moi ce qui a changé depuis »
--   Z~demandeur                       « je m'en occupe » : les autres se taisent
--   F~demandeur~nombre                fin de la synchro (chuchoté)
local _, ns = ...

local MARGE_SYNCHRO = 600         -- on redemande 10 minutes de plus, au cas où
local DELAI_REPONSE_MIN = 0.5     -- l'élu est celui qui tire le délai le plus court
local DELAI_REPONSE_MAX = 3.5
local DELAI_SYNCHRO = 8           -- après la connexion, le temps que la liste de guilde arrive

ns.magasins = {}

-- def : { champs = { "pseudo", "main"… },
--         dureeVie = secondes, ou function(fiche, cle) → secondes,
--         fusionner = function(avant, nouvelle) → fiche (facultatif) }
-- Sans « fusionner », la plus récente gagne. Avec, les deux versions se combinent
-- (les relevés de présence prennent le plus grand nombre de chaque heure).
function ns.magasin(nom, def)
	def.nom = nom
	ns.magasins[nom] = def
	return def
end

local function donnees(nom)
	local m = ns.db.magasins
	m[nom] = m[nom] or {}
	return m[nom]
end
ns.fiches = donnees

-- Range une fiche si elle est plus récente que celle qu'on a ; renvoie true si elle a changé.
function ns.ranger(nom, cle, maj, auteur, valeurs)
	local def = ns.magasins[nom]
	if not def or not cle or cle == "" then return false end
	maj = tonumber(maj) or 0
	local fiches = donnees(nom)
	local avant = fiches[cle]
	local f = { _maj = maj, _auteur = auteur }
	for i, champ in ipairs(def.champs) do
		local v = valeurs[i]
		if v ~= nil and v ~= "" then f[champ] = tostring(v) end
	end
	if def.fusionner and avant then
		local fusion = def.fusionner(avant, f)
		local pareil = true
		for _, champ in ipairs(def.champs) do
			if fusion[champ] ~= avant[champ] then pareil = false end
		end
		if pareil then return false end
		fusion._maj = math.max(avant._maj or 0, maj)
		fusion._auteur = auteur
		f = fusion
	elseif avant and avant._maj >= maj then
		return false
	end
	fiches[cle] = f
	ns.signaler(nom, cle, f)
	return true
end

local function champsDe(nom, f)
	local t = {}
	for i, champ in ipairs(ns.magasins[nom].champs) do t[i] = f[champ] or "" end
	return t
end

local function messageFiche(nom, cle, f)
	local t = { nom, cle, f._maj, f._auteur or "" }
	for _, v in ipairs(champsDe(nom, f)) do t[#t + 1] = v end
	return t
end

-- Modifie une fiche à mon nom et la diffuse sur le canal de guilde.
function ns.publier(nom, cle, valeursParChamp)
	local def = ns.magasins[nom]
	local t = {}
	for i, champ in ipairs(def.champs) do t[i] = valeursParChamp[champ] end
	local maj = math.max(ns.maintenant(), ((donnees(nom)[cle] or {})._maj or 0) + 1)
	ns.ranger(nom, cle, maj, ns.moi(), t)
	ns.envoyer("R", messageFiche(nom, cle, donnees(nom)[cle]))
end

ns.gestionnaires.R = function(_, c)
	local nom, cle, maj, auteur = c[1], c[2], c[3], c[4]
	local valeurs = {}
	for i = 5, #c do valeurs[#valeurs + 1] = c[i] end
	ns.ranger(nom, cle, maj, auteur ~= "" and auteur or nil, valeurs)
end

-- Oublie les fiches trop anciennes (chaque magasin a sa durée de vie).
function ns.nettoyerMagasins()
	local maintenant = ns.maintenant()
	for nom, def in pairs(ns.magasins) do
		if def.dureeVie then
			local fiches = donnees(nom)
			for cle, f in pairs(fiches) do
				local vie = def.dureeVie
				if type(vie) == "function" then vie = vie(f, cle) end
				if vie and maintenant - (f._maj or 0) > vie then fiches[cle] = nil end
			end
		end
	end
end

-- ------------------------------------------------------------------ synchronisation

local reponses = {}   -- [demandeur] = { annule = bool, servi = date }
ns.synchro = { demandee = nil, recue = nil, servies = 0 }

function ns.demanderSynchro()
	local depuis = math.max(0, (ns.db.derniereSynchro or 0) - MARGE_SYNCHRO)
	ns.synchro.demandee = ns.maintenant()
	ns.envoyer("S", { depuis, ns.moi() })
end

local function servir(demandeur, depuis)
	local n = 0
	for nom in pairs(ns.magasins) do
		for cle, f in pairs(donnees(nom)) do
			if (f._maj or 0) > depuis then
				ns.envoyer("R", messageFiche(nom, cle, f), { cible = demandeur, basse = true })
				n = n + 1
			end
		end
	end
	ns.envoyer("F", { demandeur, n }, { cible = demandeur, basse = true })
	ns.synchro.servies = ns.synchro.servies + 1
end

-- Tout le monde entend la demande ; chacun tire un délai, le premier répond
-- « Z » sur le canal et les autres abandonnent.
ns.gestionnaires.S = function(expediteur, c, voie)
	if voie ~= "GUILD" then return end
	local depuis, demandeur = tonumber(c[1]) or 0, ns.nomComplet(c[2]) or expediteur
	if demandeur ~= expediteur then return end
	local r = { annule = false }
	reponses[demandeur] = r
	local delai = DELAI_REPONSE_MIN + math.random() * (DELAI_REPONSE_MAX - DELAI_REPONSE_MIN)
	ns.apres(delai, function()
		if r.annule or reponses[demandeur] ~= r then return end
		reponses[demandeur] = nil
		ns.envoyer("Z", { demandeur })
		servir(demandeur, depuis)
	end)
end

ns.gestionnaires.Z = function(_, c)
	local r = reponses[ns.nomComplet(c[1]) or ""]
	if r then r.annule = true end
end

ns.gestionnaires.F = function(expediteur, c, voie)
	if voie ~= "WHISPER" or ns.nomComplet(c[1]) ~= ns.moi() then return end
	-- On ne prend la date qu'à la fin : une synchro coupée sera redemandée en entier.
	ns.db.derniereSynchro = ns.synchro.demandee or ns.maintenant()
	ns.synchro.recue = { de = expediteur, nombre = tonumber(c[2]) or 0, date = ns.maintenant() }
	ns.signaler("synchro", ns.synchro.recue)
end

ns.surSignal("charge", function()
	ns.nettoyerMagasins()
	-- et une fois par heure, pour ceux qui restent connectés longtemps
	local function boucle()
		ns.nettoyerMagasins()
		ns.apres(3600, boucle)
	end
	ns.apres(3600, boucle)
end)

ns.surSignal("connexion", function()
	ns.apres(DELAI_SYNCHRO, ns.demanderSynchro)
end)
