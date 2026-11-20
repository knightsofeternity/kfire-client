-- Appel d'Alliance : le premier lancement (F21) et le démarrage à chaque connexion.
--
-- Tant que l'installation n'est pas finie, une fenêtre de bienvenue s'ouvre : elle vérifie
-- la faction (Alliance), la guilde (KE ou Cassos) et le compte (un compte d'essai reçoit mais
-- ne publie pas), demande le pseudo Discord, fait essayer le son, et rejoint le canal. Ensuite
-- le canal est rejoint tout seul à chaque connexion et la fenêtre ne revient plus.
-- Côté Horde ou hors des deux guildes : un message clair, l'addon reste inactif.
local _, ns = ...
local ui = ns.ui
local GRIS, VERT, ROUGE, OR = ui.GRIS, ui.VERT, ui.ROUGE, ui.OR

local DELAI = 6          -- après la connexion, le temps que la guilde soit connue
local ESSAIS = 12        -- puis toutes les 5 s, une minute au plus

local RAISONS = {
	horde = "Appel d'Alliance relie les joueurs Alliance de KE et des Cassos : un canal personnalisé ne traverse pas "
		.. "les factions. Sur ce personnage Horde, l'addon reste inactif.",
	sansguilde = "Ce personnage n'est dans aucune guilde. Appel d'Alliance est réservé à Knights of Eternity et aux "
		.. "Cassos de la Lumière : il reste inactif.",
	guilde = "Ta guilde ne fait pas partie de l'alliance (Knights of Eternity, Les Cassos de la Lumière) : "
		.. "l'addon reste inactif sur ce personnage.",
	attente = "La guilde de ce personnage n'est pas encore connue. Réessaie dans quelques secondes.",
}
ns.RAISONS = RAISONS

local f
local function ligneVerif(ok, texte, attention)
	return (ok and (VERT .. "✓|r ") or ((attention and OR or ROUGE) .. "✗|r ")) .. texte
end

local function majAccueil()
	local raison = ns.empechement()
	local g = ns.maGuilde()
	local faction = UnitFactionGroup and UnitFactionGroup("player") or "?"
	local lignes = {
		ligneVerif(faction == "Alliance", "Faction : " .. faction),
		ligneVerif(raison ~= "guilde" and raison ~= "sansguilde" and g ~= nil, "Guilde : " .. (g or "aucune")),
		ligneVerif(not ns.essai(), ns.essai() and "Compte d'essai : tu recevras les annonces, sans pouvoir en publier"
			or "Compte complet", true),
	}
	f.verifs:SetText(table.concat(lignes, "\n"))
	if raison then
		f.message:SetText(RAISONS[raison])
		f.formulaire:Hide()
		f.rejoindre:Hide()
	else
		f.message:SetText(ns.db.installe and (VERT .. "Tout est prêt. Tu es dans le canal " .. ns.nomCanal() .. ".|r")
			or "Il ne reste qu'à rejoindre le canal de l'alliance. Le mot de passe est fourni par l'addon.")
		f.formulaire:Show()
		f.rejoindre:Show()
		f.rejoindre:SetText(ns.db.installe and "Fermer" or "Rejoindre l'alliance")
		if not f.discord:HasFocus() then f.discord:SetText(ns.db.discord or ns.compte.discord or "") end
	end
end

local function construire()
	f = CreateFrame("Frame", "AppelAllianceAccueil", UIParent, "BasicFrameTemplateWithInset")
	f:SetSize(440, 330)
	f:SetPoint("CENTER", 0, 60)
	f:SetFrameStrata("DIALOG")
	f:SetMovable(true)
	f:EnableMouse(true)
	f:RegisterForDrag("LeftButton")
	f:SetScript("OnDragStart", f.StartMoving)
	f:SetScript("OnDragStop", f.StopMovingOrSizing)
	tinsert(UISpecialFrames, "AppelAllianceAccueil")
	ui.texte(f, "GameFontNormal", 0, -6, "TOP"):SetText("Bienvenue dans Appel d'Alliance")
	local intro = ui.texte(f, "GameFontHighlight", 18, -32)
	intro:SetWidth(404)
	intro:SetText("Chaque demande de groupe de KE ou des Cassos devient une alerte : un son, une petite fenêtre, "
		.. "et un bouton « Intéressé ».")
	f.verifs = ui.texte(f, "GameFontHighlight", 18, -76)
	f.message = ui.texte(f, "GameFontHighlightSmall", 18, -136)
	f.message:SetWidth(404)

	f.formulaire = CreateFrame("Frame", nil, f)
	f.formulaire:SetPoint("TOPLEFT", 18, -180)
	f.formulaire:SetSize(404, 90)
	ui.texte(f.formulaire, "GameFontNormal", 0, 0):SetText("Ton pseudo Discord")
	f.discord = ui.saisie(f.formulaire, 200, 150, 4)
	ui.texte(f.formulaire, "GameFontDisableSmall", 150, -20):SetText("pour qu'on te retrouve sur le Discord de ta guilde")
	ui.bouton(f.formulaire, "Essayer le son", 130, function() ns.jouerSon(true) end):SetPoint("TOPLEFT", 0, -44)

	f.rejoindre = ui.bouton(f, "Rejoindre l'alliance", 170, function()
		ns.changerDiscord(f.discord:GetText())
		if ns.db.installe then f:Hide() return end
		ns.db.installe = true
		ns.rejoindreCanal()
		f:Hide()
		ns.dire("bienvenue ! /alliance ouvre la liste, /alliance new publie une annonce.")
	end)
	f.rejoindre:SetPoint("BOTTOMRIGHT", -16, 14)
end

function ns.ouvrirAccueil()
	if not f then construire() end
	f:Show()
	majAccueil()
end
ns.accueil = function() return f end

-- ------------------------------------------------------------------ démarrage

local dejaDit = false
local function demarrer(essai)
	local raison = ns.empechement()
	if raison == "attente" and essai < ESSAIS then
		ns.apres(5, function() demarrer(essai + 1) end)
		return
	end
	if raison then
		if not dejaDit then
			dejaDit = true
			ns.dire(RAISONS[raison])
		end
		ns.majMinicarte()
		return
	end
	if ns.db.installe then
		ns.rejoindreCanal()
	else
		ns.ouvrirAccueil()
	end
	ns.majMinicarte()
end
ns.demarrer = demarrer

ns.surSignal("connexion", function()
	ns.apres(DELAI, function() demarrer(1) end)
end)
