-- KFIRE Guilde : la page Quêtes en commun (lot 4).
local _, ns = ...
local ui = ns.ui
local texte, bouton = ui.texte, ui.bouton
local OR, GRIS, VERT = ui.OR, ui.GRIS, ui.VERT

local LIGNES = 9

ns.constructeurs.quetes = function(p)
	p.entete = texte(p, "GameFontNormal", 0, 0)
	p.liste = ui.liste(p, -26, LIGNES, 44, 660, function(r)
		r.nom = texte(r, "GameFontHighlight", 4, -4)
		r.ou = texte(r, "GameFontHighlightSmall", 250, -6)
		r.quetes = texte(r, "GameFontHighlightSmall", 4, -22)
		r.quetes:SetWidth(540)
		r.inviter = bouton(r, "Inviter", 90, function(b)
			local d = b:GetParent().donnee
			if d then ui.inviter(d.nom) end
		end)
		r.inviter:SetPoint("TOPRIGHT", -4, -10)
		r:SetScript("OnClick", function(self)
			if self.donnee then ui.chuchoter(self.donnee.nom) end
		end)
	end, function(r, m)
		r.nom:SetText(ui.couleurClasse(m.classeFichier) .. m.affichage .. "|r " .. GRIS .. "niv. " .. (m.niveau or "?") .. "|r")
		r.ou:SetText(VERT .. (m.zone or "") .. "|r")
		local titres = {}
		for i, q in ipairs(m.quetes) do titres[i] = (q.terminee and GRIS or OR) .. (q.titre or ("quête " .. q.id)) .. "|r" end
		r.quetes:SetText(table.concat(titres, ", "))
	end)
	p.aide = texte(p, "GameFontDisableSmall", 0, 0, "BOTTOMLEFT")
	p.aide:SetText("Les membres en ligne qui ont l'addon et au moins une quête de ton journal. Clic : chuchoter.")
	p:SetScript("OnShow", function() ns.rafraichirQuetes() end)
end

function ns.rafraichirQuetes()
	local p = ns.pages.quetes
	if not p or not p:IsShown() then return end
	local membres = ns.quetesEnCommun()
	local partagees = ns.quetesPartagees()
	p.entete:SetText(#membres == 0
		and (GRIS .. "Aucun membre en ligne ne fait une de tes quêtes pour l'instant.|r")
		or (("%d membre(s) en ligne partagent %d de tes quêtes"):format(#membres, #partagees)))
	p.liste.rafraichir(membres)
end

ns.surSignal("quetes", function() ns.rafraichirQuetes() end)
ns.surSignal("roster", function() ns.rafraichirQuetes() end)
ns.ecouter("QUEST_LOG_UPDATE", function()
	if ns.pages.quetes then ns.regrouper("quetes:page", 1, ns.rafraichirQuetes) end
end)
