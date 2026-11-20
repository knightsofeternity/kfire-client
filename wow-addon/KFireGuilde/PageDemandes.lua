-- KFIRE Guilde : les pages Demandes et Artisanat (lot 2).
local _, ns = ...
local ui = ns.ui
local texte, bouton, saisie = ui.texte, ui.bouton, ui.saisie
local OR, GRIS, VERT, ORANGE, BLEU, ROUGE = ui.OR, ui.GRIS, ui.VERT, ui.ORANGE, ui.BLEU, ui.ROUGE

local COULEUR_ETAT = { O = VERT, P = ORANGE, H = BLEU, X = GRIS }

-- Maj+clic sur un objet (sacs, fenêtre de métier, discussion) l'écrit dans la case qui a le focus.
local casesObjet = {}
local function accepterLiens(e)
	casesObjet[#casesObjet + 1] = e
end
local function crocheterLiens()
	if not hooksecurefunc then return end
	local function inserer(lien)
		for _, e in ipairs(casesObjet) do
			if e:IsShown() and e.HasFocus and e:HasFocus() then
				e:SetText(lien)
				return
			end
		end
	end
	if ChatEdit_InsertLink then hooksecurefunc("ChatEdit_InsertLink", inserer) end
end

local function nomsFabricants(liste, max)
	local t = {}
	for i = 1, math.min(max, #liste) do
		local n = liste[i]
		local enLigne = ns.roster[n] and ns.roster[n].enLigne
		t[i] = (enLigne and VERT or GRIS) .. ns.affichage(n) .. "|r"
	end
	if #liste > max then t[#t + 1] = GRIS .. "+" .. (#liste - max) .. "|r" end
	return table.concat(t, ", ")
end

-- ------------------------------------------------------------------ Demandes

local etat = { filtre = "ouvertes", recherche = "", selection = nil, type = "R" }
local FILTRES = {
	{ "ouvertes", "Ouvertes" }, { "pourmoi", "Pour moi" }, { "miennes", "Les miennes" },
	{ "prises", "Prises par moi" }, { "toutes", "Toutes" },
}

local function infoLigne(l)
	if l.etat == "P" then return ORANGE .. "pris par " .. ns.affichage(l.preneur or "?") .. "|r" end
	if l.etat == "H" then return BLEU .. "honorée" .. (l.preneur and (" par " .. ns.affichage(l.preneur)) or "") .. "|r" end
	if l.etat == "X" then return GRIS .. "retirée|r" end
	if l.possede > 0 then return VERT .. "tu en as " .. l.possede .. "|r" end
	if l.jeSaisFaire then return VERT .. "tu sais le fabriquer|r" end
	if #l.fabricants > 0 then return GRIS .. "fabricants : |r" .. nomsFabricants(l.fabricants, 2) end
	return ""
end

ns.constructeurs.demandes = function(p)
	p.compte = texte(p, "GameFontNormal", 0, 0)
	p.filtres = {}
	local x = 0
	for _, f in ipairs(FILTRES) do
		local b = bouton(p, f[2], 104, function()
			etat.filtre = f[1]
			ns.rafraichirDemandes()
		end)
		b:SetPoint("TOPLEFT", x, -22)
		p.filtres[f[1]] = b
		x = x + 108
	end
	local recherche = saisie(p, 110, 548, -22)
	recherche:SetScript("OnTextChanged", function(e)
		etat.recherche = e:GetText() or ""
		ns.rafraichirDemandes()
	end)

	local entetes = { { "Demande", 0 }, { "Par", 230 }, { "", 380 } }
	for _, c in ipairs(entetes) do texte(p, "GameFontNormalSmall", c[2] + 4, -52):SetText(c[1]) end

	p.liste = ui.liste(p, -66, 11, 22, 660, function(r)
		r.objet = texte(r, "GameFontHighlight", 4, -4)
		r.auteur = texte(r, "GameFontHighlightSmall", 234, -5)
		r.info = texte(r, "GameFontHighlightSmall", 384, -5)
		r:SetScript("OnClick", function(self, clic)
			if not self.donnee then return end
			if clic == "RightButton" then ui.chuchoter(self.donnee.auteur) return end
			etat.selection = self.donnee.cle
			ns.rafraichirDemandes()
		end)
		r:SetScript("OnEnter", function(self)
			local l = self.donnee
			if not l then return end
			GameTooltip:SetOwner(self, "ANCHOR_RIGHT")
			if l.objet and tonumber(l.objet) and GameTooltip.SetItemByID then
				pcall(GameTooltip.SetItemByID, GameTooltip, tonumber(l.objet))
			else
				GameTooltip:AddLine(l.nom)
			end
			GameTooltip:AddLine(" ")
			GameTooltip:AddLine(ns.TYPES_DEMANDE[l.type] .. ", demandée par " .. l.affichageAuteur
				.. " le " .. date("%d/%m à %Hh%M", l.maj or 0), 1, 1, 1)
			if l.texte then GameTooltip:AddLine(l.texte, 0.9, 0.9, 0.9, true) end
			if #l.fabricants > 0 then GameTooltip:AddLine("Fabricants : " .. nomsFabricants(l.fabricants, 6), 1, 1, 1, true) end
			GameTooltip:AddLine("Clic : choisir. Clic droit : chuchoter à l'auteur.", 0.25, 0.63, 1)
			GameTooltip:Show()
		end)
		r:SetScript("OnLeave", function() GameTooltip:Hide() end)
	end, function(r, l)
		local sel = l.cle == etat.selection
		r.objet:SetText((sel and "> " or "") .. (COULEUR_ETAT[l.etat] or "") .. l.qte .. " × " .. l.nom .. "|r"
			.. (l.type == "A" and (GRIS .. " (à fabriquer)|r") or ""))
		r.auteur:SetText((l.auteurEnLigne and "" or GRIS) .. l.affichageAuteur .. "|r")
		r.info:SetText(infoLigne(l))
	end)

	-- la demande choisie et ce que je peux en faire
	p.choisie = texte(p, "GameFontHighlight", 0, -318)
	p.choisie:SetWidth(660)
	p.actions = {}
	local x2 = 0
	for _, a in ipairs({
		{ "prendre", "Je m'en occupe", ns.prendreDemande }, { "lacher", "Je laisse", ns.lacherDemande },
		{ "honorer", "Honorée", ns.honorerDemande }, { "retirer", "Retirer", ns.retirerDemande },
	}) do
		local b = bouton(p, a[2], 120, function()
			if etat.selection then
				local ok, err = a[3](etat.selection)
				if not ok and err then ns.dire(err) end
			end
		end)
		b:SetPoint("TOPLEFT", x2, -338)
		p.actions[a[1]] = b
		x2 = x2 + 124
	end
	p.chuchoter = bouton(p, "Chuchoter", 100, function()
		local f = etat.selection and ns.fiches("demandes")[etat.selection]
		if f then ui.chuchoter(ns.auteurDemande(etat.selection)) end
	end)
	p.chuchoter:SetPoint("TOPLEFT", x2, -338)

	-- nouvelle demande
	texte(p, "GameFontNormal", 0, -376):SetText("Nouvelle demande")
	p.types = {}
	local x3 = 130
	for _, t in ipairs({ { "R", "Ressource" }, { "A", "À fabriquer" } }) do
		local b = bouton(p, t[2], 100, function()
			etat.type = t[1]
			for c, tb in pairs(p.types) do tb:SetAlpha(c == etat.type and 1 or 0.5) end
		end)
		b:SetPoint("TOPLEFT", x3, -372)
		p.types[t[1]] = b
		x3 = x3 + 104
	end
	texte(p, "GameFontHighlightSmall", 0, -404):SetText("Objet (Maj+clic)")
	p.objet = saisie(p, 220, 100, -398)
	accepterLiens(p.objet)
	texte(p, "GameFontHighlightSmall", 330, -404):SetText("Qté")
	p.qte = saisie(p, 46, 360, -398)
	p.qte:SetText("1")
	texte(p, "GameFontHighlightSmall", 420, -404):SetText("Mot")
	p.mot = saisie(p, 180, 452, -398)
	p.message = texte(p, "GameFontRedSmall", 0, -428)
	local publier = bouton(p, "Publier", 100, function()
		local cle, err = ns.publierDemande({ type = etat.type, objet = p.objet:GetText(), qte = p.qte:GetText(), texte = p.mot:GetText() })
		if cle then
			p.objet:SetText("")
			p.qte:SetText("1")
			p.mot:SetText("")
			p.message:SetText("")
			etat.selection, etat.filtre = cle, "miennes"
			ns.rafraichirDemandes()
		else
			p.message:SetText(err)
		end
	end)
	publier:SetPoint("TOPRIGHT", 0, -424)
	for c, tb in pairs(p.types) do tb:SetAlpha(c == etat.type and 1 or 0.5) end
	p:SetScript("OnShow", function() ns.rafraichirDemandes() end)
end

function ns.rafraichirDemandes()
	local p = ns.pages.demandes
	if not p or not p:IsShown() then return end
	local ouvertes, pourMoi = ns.compteDemandes()
	p.compte:SetText(("%d demande(s) ouverte(s), %s%d que tu peux honorer|r"):format(ouvertes, VERT, pourMoi))
	for c, b in pairs(p.filtres) do b:SetAlpha(c == etat.filtre and 1 or 0.5) end
	p.liste.rafraichir(ns.listeDemandes(etat))
	if #p.liste.donnees == 0 then
		p.choisie:SetText(GRIS .. "Aucune demande ici. Publie la tienne en bas : la guilde la verra, même quand tu seras hors ligne.|r")
	end
	local f = etat.selection and ns.fiches("demandes")[etat.selection]
	local actions = f and ns.actionsDemande(etat.selection) or {}
	if f then
		p.choisie:SetText(OR .. (f.qte or 1) .. " × " .. (f.nom or "?") .. "|r, " .. ns.affichage(ns.auteurDemande(etat.selection))
			.. " — " .. (COULEUR_ETAT[f.etat] or "") .. (ns.ETATS_DEMANDE[f.etat] or "") .. "|r"
			.. (f.texte and (GRIS .. " « " .. f.texte .. " »|r") or ""))
	elseif #p.liste.donnees > 0 then
		p.choisie:SetText(GRIS .. "Choisis une demande dans la liste.|r")
	end
	for nom, b in pairs(p.actions) do
		if actions[nom] then b:Enable() b:SetAlpha(1) else b:Disable() b:SetAlpha(0.4) end
	end
end

-- ------------------------------------------------------------------ Artisanat

local etatA = { recherche = "", metier = nil }

ns.constructeurs.artisanat = function(p)
	texte(p, "GameFontNormal", 0, 0):SetText("Qui sait fabriquer…")
	local recherche = saisie(p, 240, 150, 4)
	accepterLiens(recherche)
	recherche:SetScript("OnTextChanged", function(e)
		local t = e:GetText() or ""
		local _, nom = ns.lireObjet(t)
		etatA.recherche = nom or t
		ns.rafraichirArtisanat()
	end)
	p.metier = bouton(p, "Tous les métiers", 150, function()
		local noms = ns.nomsMetiers()
		local suivant
		if not etatA.metier then suivant = noms[1]
		else
			for i, n in ipairs(noms) do if n == etatA.metier then suivant = noms[i + 1] end end
		end
		etatA.metier = suivant
		ns.rafraichirArtisanat()
	end)
	p.metier:SetPoint("TOPRIGHT", 0, 4)
	p.titres = texte(p, "GameFontNormalSmall", 4, -30)
	p.liste = ui.liste(p, -46, 15, 22, 660, function(r)
		r.a = texte(r, "GameFontHighlight", 4, -4)
		r.b = texte(r, "GameFontHighlightSmall", 254, -5)
		r.c = texte(r, "GameFontHighlightSmall", 420, -5)
		r:SetScript("OnClick", function(self)
			local d = self.donnee
			if not d then return end
			local cible = d.perso or d.fabricants and d.fabricants[1]
			if cible then ui.chuchoter(cible) end
		end)
	end, function(r, d)
		if d.perso then
			r.a:SetText((d.enLigne and "" or GRIS) .. d.affichage .. "|r")
			r.b:SetText(d.metier)
			r.c:SetText(("%d / %d%s"):format(d.niveau, d.max,
				d.nbRecettes > 0 and (GRIS .. ", " .. d.nbRecettes .. " recettes|r") or (GRIS .. ", recettes pas encore lues|r")))
		else
			r.a:SetText(OR .. d.nom .. "|r")
			r.b:SetText(nomsFabricants(d.fabricants, 4))
			r.c:SetText("")
		end
	end)
	p.aide = texte(p, "GameFontDisableSmall", 0, 0, "BOTTOMLEFT")
	p.aide:SetText("Ouvre une fois la fenêtre de chacun de tes métiers : tes recettes sont alors partagées avec la guilde. Clic : chuchoter.")
	p:SetScript("OnShow", function() ns.rafraichirArtisanat() end)
end

function ns.rafraichirArtisanat()
	local p = ns.pages.artisanat
	if not p or not p:IsShown() then return end
	p.metier:SetText(etatA.metier or "Tous les métiers")
	if #(etatA.recherche or "") >= 2 then
		local res = ns.chercherRecettes(etatA.recherche)
		p.titres:SetText(#res == 0 and (GRIS .. "Personne dans la guilde ne connaît une recette de ce nom.|r")
			or ("Objet" .. string.rep(" ", 60) .. "Fabricants (en ligne en vert)"))
		p.liste.rafraichir(res)
	else
		local res = ns.listeMetiers(etatA.metier)
		p.titres:SetText(#res == 0 and (GRIS .. "Aucun métier partagé pour l'instant.|r") or "Membre, métier, niveau")
		p.liste.rafraichir(res)
	end
end

ns.surSignal("demandes", function() ns.rafraichirDemandes() end)
ns.surSignal("roster", function() ns.rafraichirDemandes() ns.rafraichirArtisanat() end)
ns.surSignal("metiers", function() ns.rafraichirArtisanat() ns.rafraichirDemandes() end)
ns.surSignal("charge", crocheterLiens)
