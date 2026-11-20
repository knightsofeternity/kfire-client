-- Appel d'Alliance : les membres de l'alliance qui ont l'addon (F20) et le contrôle
-- de version (F17).
--
-- Chacun annonce sa présence (HELLO) en rejoignant le canal, puis toutes les 15 minutes :
-- version, personnage, guilde, classe, niveau, rôle, pseudo Discord, zone. Un membre sans
-- HELLO depuis 20 minutes, ou qui quitte le canal, passe hors ligne. Les membres vus sont
-- gardés 30 jours. On ne relaie pas les listes de guilde : seuls ceux qui ont l'addon figurent.
local _, ns = ...

local INTERVALLE = 15 * 60
local SILENCE = 20 * 60
local GARDE = 30 * 86400

-- HELLO~version~guilde~classe~niveau~rôle~discord~zone
local function champsHello()
	return { ns.VERSION, ns.maGuilde() or "", ns.maClasse() or "", ns.monNiveau(), ns.db.reglages.role or "",
		ns.db.discord or "", (GetZoneText and GetZoneText()) or "" }
end

function ns.direBonjour()
	if not ns.actif() then return end
	ns.diffuser("HELLO", champsHello(), { remplace = "HELLO" })
end

local versionSignalee = false
ns.gestionnaires.HELLO = function(expediteur, c)
	local guilde = c[2]
	if not ns.guilde(guilde) then ns.stats.ignores = ns.stats.ignores + 1 return end
	local avant = ns.compte.membres[expediteur]
	ns.compte.membres[expediteur] = {
		version = c[1], guilde = guilde, classe = c[3] ~= "" and c[3] or nil, niveau = tonumber(c[4]),
		role = ns.ROLES[c[5]] and c[5] or nil, discord = c[6] ~= "" and c[6] or nil,
		zone = c[7] ~= "" and c[7] or nil, vu = ns.maintenant(), enLigne = true,
	}
	-- F17 : une version plus récente circule
	if not versionSignalee and ns.comparerVersions(c[1], ns.VERSION) > 0 then
		versionSignalee = true
		ns.dire(("une version plus récente circule (%s, tu as %s) : récupère-la sur le Discord de ta guilde.")
			:format(c[1], ns.VERSION))
	end
	-- un nouveau venu reçoit mon HELLO en retour, une fois, pour me voir sans attendre 15 min
	if not avant or not avant.enLigne then
		ns.chuchoter(expediteur, "HI", champsHello(), { basse = true })
	end
	ns.signaler("membres", expediteur)
end

-- La réponse chuchotée à un HELLO : même contenu, sans réponse en retour.
ns.gestionnaires.HI = function(expediteur, c)
	if not ns.guilde(c[2]) then return end
	ns.compte.membres[expediteur] = {
		version = c[1], guilde = c[2], classe = c[3] ~= "" and c[3] or nil, niveau = tonumber(c[4]),
		role = ns.ROLES[c[5]] and c[5] or nil, discord = c[6] ~= "" and c[6] or nil,
		zone = c[7] ~= "" and c[7] or nil, vu = ns.maintenant(), enLigne = true,
	}
	ns.signaler("membres", expediteur)
end

ns.surSignal("depart", function(nom)
	local m = ns.compte.membres[nom]
	if m and m.enLigne then
		m.enLigne = false
		ns.signaler("membres", nom)
	end
end)

-- Hors ligne après 20 minutes de silence ; oublié après 30 jours.
function ns.majPresence()
	local maintenant = ns.maintenant()
	for nom, m in pairs(ns.compte.membres) do
		if maintenant - (m.vu or 0) > GARDE then
			ns.compte.membres[nom] = nil
		elseif m.enLigne and maintenant - (m.vu or 0) > SILENCE then
			m.enLigne = false
		end
	end
end

-- options : { filtre = "tous" | "KE" | "Cassos" | "enligne", recherche = "…" }
function ns.listeMembres(options)
	options = options or {}
	ns.majPresence()
	local filtre = options.filtre or "tous"
	local cherche = options.recherche and options.recherche ~= "" and options.recherche:lower() or nil
	local res = {}
	for nom, m in pairs(ns.compte.membres) do
		local garder = filtre == "tous" or (filtre == "enligne" and m.enLigne) or ns.sigle(m.guilde) == filtre
		if garder and cherche then
			garder = (nom:lower():find(cherche, 1, true) or (m.discord or ""):lower():find(cherche, 1, true)
				or (m.zone or ""):lower():find(cherche, 1, true)) ~= nil
		end
		if garder then
			res[#res + 1] = { nom = nom, version = m.version, guilde = m.guilde, classe = m.classe, niveau = m.niveau,
				role = m.role, discord = m.discord, zone = m.zone, vu = m.vu, enLigne = m.enLigne }
		end
	end
	table.sort(res, function(a, b)
		if a.enLigne ~= b.enLigne then return a.enLigne end
		if (a.vu or 0) ~= (b.vu or 0) and not a.enLigne then return (a.vu or 0) > (b.vu or 0) end
		return a.nom < b.nom
	end)
	return res
end

function ns.compteMembres()
	local enLigne, total = 0, 0
	ns.majPresence()
	for _, m in pairs(ns.compte.membres) do
		total = total + 1
		if m.enLigne then enLigne = enLigne + 1 end
	end
	return enLigne, total
end

ns.surSignal("canal", function(ok)
	if ok then ns.direBonjour() end
end)

ns.surSignal("charge", function()
	-- à la connexion, personne n'est en ligne tant qu'on ne l'a pas entendu
	for _, m in pairs(ns.compte.membres) do m.enLigne = false end
	ns.majPresence()
	local function boucle()
		if ns.dansLeCanal() then ns.direBonjour() end
		ns.majPresence()
		ns.apres(INTERVALLE, boucle)
	end
	ns.apres(INTERVALLE, boucle)
end)
