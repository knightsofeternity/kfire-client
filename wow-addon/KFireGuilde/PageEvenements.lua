-- KFIRE Guilde : les pages Événements et Statistiques (lot 3).
local _, ns = ...
local ui = ns.ui
local texte, bouton = ui.texte, ui.bouton
local OR, GRIS, VERT, ORANGE, ROUGE = ui.OR, ui.GRIS, ui.VERT, ui.ORANGE, ui.ROUGE

local COULEUR_GROUPE = { oui = VERT, peut = ORANGE, attente = OR, non = ROUGE }

local function texteEtat(code)
	local e = code and ns.ETATS_INSCRIPTION[code]
	if not e then return "" end
	return (COULEUR_GROUPE[e[2]] or GRIS) .. e[1] .. "|r"
end

local function texteType(e)
	if e.typeCal == "GUILD_ANNOUNCEMENT" then return "Annonce" end
	local t = ns.TYPES_EVENEMENT[e.typeEv] or ""
	if e.typeCal == "PLAYER" then t = t .. (t ~= "" and ", " or "") .. "invitation" end
	return t
end

-- ------------------------------------------------------------------ Événements

local selection

ns.constructeurs.evenements = function(p)
	p.entete = texte(p, "GameFontNormal", 0, 0)
	p.liste = ui.liste(p, -24, 17, 24, 330, function(r)
		r.quand = texte(r, "GameFontHighlightSmall", 4, -2)
		r.titre = texte(r, "GameFontHighlight", 4, -12)
		r.etat = texte(r, "GameFontHighlightSmall", 240, -6)
		r:SetScript("OnClick", function(self)
			if not self.donnee then return end
			selection = self.donnee.cle
			ns.ouvrirEvenement(selection)
			ns.rafraichirEvenements()
		end)
	end, function(r, e)
		r.quand:SetText(GRIS .. date("%a %d/%m %Hh%M", e.quand) .. "  " .. texteType(e) .. "|r")
		r.titre:SetText((e.cle == selection and OR or "") .. e.titre .. "|r")
		r.etat:SetText(texteEtat(e.etat))
	end)

	-- le détail, à droite
	local d = CreateFrame("Frame", nil, p)
	d:SetPoint("TOPLEFT", 345, -24)
	d:SetSize(315, 440)
	p.detail = d
	d.titre = texte(d, "GameFontNormalLarge", 0, 0)
	d.titre:SetWidth(315)
	d.quand = texte(d, "GameFontHighlight", 0, -22)
	d.description = texte(d, "GameFontHighlightSmall", 0, -44)
	d.description:SetWidth(315)
	d.compte = texte(d, "GameFontHighlightSmall", 0, -150)
	d.invites = texte(d, "GameFontHighlightSmall", 0, -170)
	d.invites:SetWidth(315)
	d.raison = texte(d, "GameFontDisableSmall", 0, -360)
	d.raison:SetWidth(315)
	d.boutons = {}
	local x = 0
	for _, b in ipairs({ { "oui", "S'inscrire" }, { "peut", "Peut-être" }, { "non", "Se désinscrire" } }) do
		local bt = bouton(d, b[2], 100, function()
			if not selection then return end
			local ok, err = ns.repondreEvenement(selection, b[1])
			if not ok and err then ns.dire(err) end
		end)
		bt:SetPoint("TOPLEFT", x, -380)
		d.boutons[b[1]] = bt
		x = x + 104
	end
	bouton(d, "Calendrier du jeu", 150, function()
		if ns.basculer then ns.basculer() end
		ns.ouvrirCalendrierDuJeu()
	end):SetPoint("TOPLEFT", 0, -410)
	p:SetScript("OnShow", function()
		ns.demanderCalendrier()
		ns.rafraichirEvenements()
	end)
end

function ns.rafraichirEvenements()
	local p = ns.pages.evenements
	if not p or not p:IsShown() then return end
	local d = p.detail
	if not ns.calendrierDisponible() then
		p.entete:SetText(GRIS .. "Le calendrier du jeu n'est pas lisible par l'addon sur ce client.|r")
		p.liste.rafraichir({})
		d.titre:SetText("")
		return
	end
	local evs = ns.listeEvenements()
	p.entete:SetText(#evs == 0 and (GRIS .. "Aucun événement dans les 30 prochains jours.|r")
		or (#evs .. " événement(s) dans les 30 prochains jours"))
	p.liste.rafraichir(evs)
	local e
	for _, x in ipairs(evs) do if x.cle == selection then e = x end end
	if not e then
		d.titre:SetText(#evs > 0 and (GRIS .. "Choisis un événement.|r") or "")
		d.quand:SetText("") d.description:SetText("") d.compte:SetText("") d.invites:SetText("") d.raison:SetText("")
		for _, b in pairs(d.boutons) do b:Hide() end
		return
	end
	d.titre:SetText(e.titre)
	d.quand:SetText(date("%A %d/%m à %Hh%M", e.quand) .. "  " .. GRIS .. texteType(e) .. "|r")
	local det = ns.detailEvenement and ns.detailEvenement.cle == e.cle and ns.detailEvenement or nil
	if det then
		d.description:SetText((det.description or "") .. (det.createur and (GRIS .. "\nPar " .. det.createur .. "|r") or ""))
		local c = det.compte
		d.compte:SetText(("%s%d|r oui, %s%d|r peut-être, %s%d|r en attente, %s%d|r non"):format(
			VERT, c.oui, ORANGE, c.peut, OR, c.attente, ROUGE, c.non))
		local noms = {}
		for i, v in ipairs(det.invites) do
			if i > 24 then noms[#noms + 1] = GRIS .. "…|r" break end
			noms[#noms + 1] = ui.couleurClasse(v.classeFichier) .. ns.prenom(v.nom) .. "|r " .. texteEtat(v.etat)
		end
		d.invites:SetText(table.concat(noms, "\n"))
	else
		d.description:SetText(GRIS .. "Chargement du détail…|r")
		d.compte:SetText("") d.invites:SetText("")
	end
	local raison = ns.empechementReponse(e.cle)
	d.raison:SetText(raison and (GRIS .. raison .. "|r") or "")
	for _, b in pairs(d.boutons) do
		b:Show()
		if raison then b:Disable() else b:Enable() end
	end
end

ns.surSignal("evenements", function() ns.rafraichirEvenements() end)
ns.surSignal("evenement", function() ns.rafraichirEvenements() end)

-- ------------------------------------------------------------------ Statistiques

local CASE_L, CASE_H = 22, 18

ns.constructeurs.stats = function(p)
	p.entete = texte(p, "GameFontNormal", 0, 0)
	texte(p, "GameFontNormalSmall", 0, -24):SetText("Semaine type : membres en ligne, en moyenne, par jour et par heure")
	for h = 0, 23, 3 do
		texte(p, "GameFontDisableSmall", 80 + h * CASE_L, -42):SetText(h .. "h")
	end
	p.cases = {}
	for j = 1, 7 do
		texte(p, "GameFontHighlightSmall", 0, -60 - (j - 1) * (CASE_H + 2)):SetText(ns.JOURS[j])
		p.cases[j] = {}
		for h = 1, 24 do
			local t = p:CreateTexture(nil, "ARTWORK")
			t:SetSize(CASE_L - 2, CASE_H)
			t:SetPoint("TOPLEFT", 80 + (h - 1) * CASE_L, -56 - (j - 1) * (CASE_H + 2))
			p.cases[j][h] = t
		end
	end
	texte(p, "GameFontNormal", 0, -210):SetText("Les 5 meilleurs créneaux")
	p.meilleurs = texte(p, "GameFontHighlight", 0, -232)
	p.legende = texte(p, "GameFontDisableSmall", 0, 0, "BOTTOMLEFT")
	p.legende:SetText("Chaque membre qui a l'addon relève le nombre de membres en ligne toutes les 10 minutes. Les relevés sont gardés 8 semaines.")
	p:SetScript("OnShow", function() ns.rafraichirStats() end)
end

function ns.rafraichirStats()
	local p = ns.pages.stats
	if not p or not p:IsShown() then return end
	local grille, max, jours = ns.semaineType()
	p.entete:SetText(("%d membres en ligne maintenant ; %d jour(s) de relevés"):format(ns.compterEnLigne(), jours))
	for j = 1, 7 do
		for h = 1, 24 do
			local v, t = grille[j][h], p.cases[j][h]
			if v and max > 0 then
				t:SetColorTexture(0.25, 0.63, 1, 0.15 + 0.85 * v / max)
			else
				t:SetColorTexture(0.3, 0.3, 0.3, 0.15)
			end
		end
	end
	local lignes = {}
	for i, c in ipairs(ns.meilleursCreneaux(5)) do
		lignes[i] = ("%d. %s de %dh à %dh : %.1f en ligne"):format(i, ns.JOURS[c.jour], c.heure, (c.heure + 1) % 24, c.moyenne)
	end
	p.meilleurs:SetText(#lignes > 0 and table.concat(lignes, "\n") or (GRIS .. "Pas encore de relevé. Reviens dans quelques heures.|r"))
end

ns.surSignal("presence", function() ns.rafraichirStats() end)
