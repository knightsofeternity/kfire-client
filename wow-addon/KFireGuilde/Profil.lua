-- KFIRE Guilde : mon profil (pseudo, personnage principal, rôle) et la fiche
-- « membres » que chaque personnage publie à la connexion.
--
-- Le pseudo est celui du membre sur KFIRE et sur le Discord : la page Guilde
-- affiche « Pseudo (Prénom) » partout où un membre apparaît.
local _, ns = ...

local DELAI_ANNONCE = 5          -- après la connexion
local VIE_FICHE = 60 * 86400     -- une fiche sans nouvelles depuis 60 jours est oubliée
local PSEUDO_MAX = 24

ns.ROLES = { T = "Tank", H = "Soigneur", D = "Dégâts" }

-- Une fiche par personnage. Sa date de mise à jour dit aussi quand le membre
-- a été vu pour la dernière fois avec l'addon, et dans quelle version.
ns.magasin("membres", {
	champs = { "pseudo", "main", "role", "version" },
	dureeVie = VIE_FICHE,
})

local function nettoyerTexte(t, max)
	t = tostring(t or ""):gsub("^%s+", ""):gsub("%s+$", ""):gsub("[~|\r\n]", "")
	if max and #t > max then t = t:sub(1, max) end
	return t
end

function ns.monProfil()
	local moi = ns.db.moi
	return {
		pseudo = moi.pseudo,
		main = moi.main,
		role = moi.roles[ns.moi()],
		version = ns.VERSION,
	}
end

-- Ce qui manque au profil, dans l'ordre où le formulaire le demande.
function ns.profilManquant()
	local p, manque = ns.monProfil(), {}
	if not p.pseudo then manque[#manque + 1] = "pseudo" end
	if not p.main then manque[#manque + 1] = "personnage principal" end
	if not p.role then manque[#manque + 1] = "rôle" end
	return manque
end

function ns.annoncerProfil()
	ns.publier("membres", ns.moi(), ns.monProfil())
end

-- valeurs : { pseudo =, main =, role = } ; renvoie nil, message si quelque chose ne va pas.
function ns.enregistrerProfil(valeurs)
	local pseudo = nettoyerTexte(valeurs.pseudo, PSEUDO_MAX)
	local main = nettoyerTexte(valeurs.main)
	local role = valeurs.role
	if role and not ns.ROLES[role] then return nil, "rôle inconnu : " .. tostring(role) end
	if main ~= "" and main:find("%s") then return nil, "le personnage principal est un nom de personnage, sans espace" end
	local moi = ns.db.moi
	moi.pseudo = pseudo ~= "" and pseudo or nil
	moi.main = main ~= "" and ns.nomComplet(main) or nil
	moi.roles[ns.moi()] = role
	ns.annoncerProfil()
	ns.signaler("profil")
	return true
end

-- « Pseudo (Prénom) » si le membre a un pseudo, sinon son prénom.
function ns.affichage(nom)
	local f = ns.fiches("membres")[nom]
	local prenom = ns.prenom(nom)
	if f and f.pseudo and f.pseudo ~= prenom then
		return f.pseudo .. " (" .. prenom .. ")"
	end
	return prenom
end

ns.surSignal("connexion", function()
	ns.apres(DELAI_ANNONCE, ns.annoncerProfil)
end)
