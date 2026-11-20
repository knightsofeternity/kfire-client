-- KFIRE Guilde : les demandes de la guilde (lot 2).
--
-- Un membre publie un besoin : une ressource, ou un objet à fabriquer, avec une
-- quantité. Toute la guilde le voit, même quand l'auteur est hors ligne. Un membre
-- le prend en charge, puis l'auteur ou le preneur le marque honoré.
--
-- Une demande est une fiche du magasin « demandes », de clé « Auteur-Royaume/numéro ».
-- Chaque changement d'état republie la fiche ; la plus récente gagne.
local _, ns = ...

local QTE_MAX = 999
local TEXTE_MAX = 80
local VIE_FINIE = 3 * 86400       -- une demande honorée ou retirée est oubliée après 3 jours
local VIE_OUVERTE = 30 * 86400    -- une autre, après 30 jours sans changement
local RECENTE = 600               -- une demande de moins de 10 min peut faire une bulle

-- Les rangs qui peuvent retirer la demande d'un autre (0 = maître de guilde, 1 = officiers).
ns.RANG_MODERATION = 1

ns.TYPES_DEMANDE = { R = "Ressource", A = "Artisanat" }
ns.ETATS_DEMANDE = { O = "Ouverte", P = "Prise en charge", H = "Honorée", X = "Retirée" }

ns.magasin("demandes", {
	champs = { "type", "objet", "nom", "qte", "texte", "etat", "preneur" },
	dureeVie = function(f)
		return (f.etat == "H" or f.etat == "X") and VIE_FINIE or VIE_OUVERTE
	end,
})

local function nettoyer(t, max)
	t = tostring(t or ""):gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""):gsub("[~|\r\n]", " ")
	t = t:gsub("^%s+", ""):gsub("%s+$", "")
	if max and #t > max then t = t:sub(1, max) end
	return t
end

local function auteurDe(cle)
	return (cle or ""):match("^(.-)/")
end
ns.auteurDemande = auteurDe

function ns.peutModerer()
	local m = ns.roster[ns.moi()]
	return m ~= nil and (m.rangIndex or 99) <= ns.RANG_MODERATION
end

local dernierNumero = 0
local function nouvelleCle()
	local n = math.max(ns.maintenant(), dernierNumero + 1)
	dernierNumero = n
	return ns.moi() .. "/" .. n
end

-- valeurs : { type = "R" | "A", objet = lien ou texte, qte = nombre, texte = "…" }
-- Renvoie la clé, ou nil et le message d'erreur.
function ns.publierDemande(valeurs)
	local genre = valeurs.type or "R"
	if not ns.TYPES_DEMANDE[genre] then return nil, "type de demande inconnu" end
	local id, nom = ns.lireObjet(valeurs.objet)
	nom = nettoyer(nom, 60)
	if nom == "" then return nil, "indique l'objet demandé (glisse son lien avec Maj+clic)" end
	local qte = tonumber(valeurs.qte) or 1
	qte = math.floor(qte)
	if qte < 1 or qte > QTE_MAX then return nil, "quantité entre 1 et " .. QTE_MAX end
	local cle = nouvelleCle()
	ns.publier("demandes", cle, {
		type = genre, objet = id, nom = nom, qte = qte,
		texte = nettoyer(valeurs.texte, TEXTE_MAX), etat = "O",
	})
	return cle
end

local function changer(cle, etat, preneur)
	local f = ns.fiches("demandes")[cle]
	local v = {}
	for _, champ in ipairs(ns.magasins.demandes.champs) do v[champ] = f[champ] end
	v.etat, v.preneur = etat, preneur
	ns.publier("demandes", cle, v)
	return true
end

-- Ce que je peux faire d'une demande : { prendre, lacher, honorer, retirer }.
function ns.actionsDemande(cle)
	local f = ns.fiches("demandes")[cle]
	if not f then return {} end
	local moi = ns.moi()
	local auteur = auteurDe(cle) == moi
	local preneur = f.preneur == moi
	return {
		prendre = f.etat == "O" and not auteur,
		lacher = f.etat == "P" and (preneur or auteur),
		honorer = (f.etat == "O" or f.etat == "P") and (auteur or preneur),
		retirer = (f.etat == "O" or f.etat == "P") and (auteur or ns.peutModerer()),
	}
end

local function action(nom, cle, etat, preneur)
	if not ns.actionsDemande(cle)[nom] then return nil, "action impossible sur cette demande" end
	return changer(cle, etat, preneur)
end

function ns.prendreDemande(cle) return action("prendre", cle, "P", ns.moi()) end
function ns.lacherDemande(cle) return action("lacher", cle, "O", nil) end
function ns.honorerDemande(cle)
	local f = ns.fiches("demandes")[cle]
	return action("honorer", cle, "H", f and (f.preneur or (auteurDe(cle) ~= ns.moi() and ns.moi()) or nil))
end
function ns.retirerDemande(cle)
	local f = ns.fiches("demandes")[cle]
	return action("retirer", cle, "X", f and f.preneur)
end

-- ------------------------------------------------------------------ la liste

local ORDRE_ETAT = { O = 1, P = 2, H = 3, X = 4 }

local function ligne(cle, f)
	local auteur = auteurDe(cle)
	local possede = f.type == "R" and ns.compter(f.objet) or 0
	local fabricants = f.objet and ns.fabricants(f.objet) or {}
	local jeSais = false
	for _, n in ipairs(fabricants) do if n == ns.moi() then jeSais = true end end
	return {
		cle = cle, auteur = auteur, affichageAuteur = ns.affichage(auteur),
		type = f.type, objet = f.objet, nom = f.nom or ns.nomObjet(f.objet) or "?",
		qte = tonumber(f.qte) or 1, texte = f.texte, etat = f.etat or "O",
		preneur = f.preneur, maj = f._maj, possede = possede,
		fabricants = fabricants, jeSaisFaire = jeSais,
		auteurEnLigne = ns.roster[auteur] and ns.roster[auteur].enLigne or false,
		honorable = (f.etat == "O") and auteur ~= ns.moi() and (possede > 0 or jeSais),
	}
end

-- options : { filtre = "ouvertes" | "pourmoi" | "miennes" | "prises" | "toutes", recherche = "…" }
function ns.listeDemandes(options)
	options = options or {}
	local filtre = options.filtre or "ouvertes"
	local cherche = options.recherche and options.recherche ~= "" and options.recherche:lower() or nil
	local moi, liste = ns.moi(), {}
	for cle, f in pairs(ns.fiches("demandes")) do
		local l = ligne(cle, f)
		local actif = l.etat == "O" or l.etat == "P"
		local garder = (filtre == "toutes")
			or (filtre == "ouvertes" and actif)
			or (filtre == "pourmoi" and l.honorable)
			or (filtre == "miennes" and l.auteur == moi)
			or (filtre == "prises" and l.preneur == moi and l.etat == "P")
		if garder and cherche then
			garder = (l.nom:lower():find(cherche, 1, true) or (l.texte or ""):lower():find(cherche, 1, true)
				or l.affichageAuteur:lower():find(cherche, 1, true)) ~= nil
		end
		if garder then liste[#liste + 1] = l end
	end
	table.sort(liste, function(a, b)
		if a.honorable ~= b.honorable then return a.honorable end
		if ORDRE_ETAT[a.etat] ~= ORDRE_ETAT[b.etat] then return (ORDRE_ETAT[a.etat] or 9) < (ORDRE_ETAT[b.etat] or 9) end
		if (a.maj or 0) ~= (b.maj or 0) then return (a.maj or 0) > (b.maj or 0) end
		return a.cle < b.cle
	end)
	return liste
end

function ns.compteDemandes()
	local ouvertes, pourMoi = 0, 0
	for _, l in ipairs(ns.listeDemandes({ filtre = "ouvertes" })) do
		ouvertes = ouvertes + 1
		if l.honorable then pourMoi = pourMoi + 1 end
	end
	return ouvertes, pourMoi
end

-- ------------------------------------------------------------------ bulles

-- Une demande toute fraîche d'un autre membre, que je peux honorer : une bulle, une seule fois.
ns.surSignal("demandes", function(cle, f)
	if not ns.db or ns.db.reglages.bullesDemandesCoupees then return end
	if f.etat ~= "O" or auteurDe(cle) == ns.moi() then return end
	if ns.maintenant() - (f._maj or 0) > RECENTE then return end
	if ns.db.vus["d:" .. cle] then return end
	local l = ligne(cle, f)
	if not l.honorable then return end
	ns.db.vus["d:" .. cle] = ns.maintenant()
	local pourquoi = l.possede > 0 and ("tu en as " .. l.possede) or "tu sais le fabriquer"
	ns.bulle(("%s demande %d × %s (%s)"):format(ns.affichage(l.auteur), l.qte, l.nom, pourquoi), "demandes")
end)

-- Une de mes demandes est prise ou honorée par un autre : je veux le savoir.
ns.surSignal("demandes", function(cle, f)
	if not ns.db or auteurDe(cle) ~= ns.moi() or not f.preneur or f.preneur == ns.moi() then return end
	if f._auteur == ns.moi() or ns.maintenant() - (f._maj or 0) > RECENTE then return end
	local vu = "d:" .. cle .. ":" .. (f.etat or "")
	if ns.db.vus[vu] then return end
	ns.db.vus[vu] = ns.maintenant()
	if f.etat == "P" then
		ns.bulle(("%s s'occupe de ta demande : %s"):format(ns.affichage(f.preneur), f.nom or "?"), "demandes")
	elseif f.etat == "H" then
		ns.bulle(("%s a honoré ta demande : %s"):format(ns.affichage(f.preneur), f.nom or "?"), "demandes")
	end
end)

-- Les « déjà vus » ne servent plus après quelques jours.
ns.surSignal("charge", function()
	local limite = ns.maintenant() - 40 * 86400
	for k, t in pairs(ns.db.vus) do
		if type(t) ~= "number" or t < limite then ns.db.vus[k] = nil end
	end
end)
