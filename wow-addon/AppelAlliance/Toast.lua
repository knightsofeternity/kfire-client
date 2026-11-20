-- Appel d'Alliance : l'alerte à réception (F03 son, F04 toast, F05 ligne dans le chat)
-- et la fiche de l'initiateur (F10).
local _, ns = ...
local ui = ns.ui

local DUREE = 15
local MAX = 3
local HAUTEUR = 74

-- ------------------------------------------------------------------ son

function ns.jouerSon(forcer, discret)
	if not forcer and not ns.db.reglages.son then return end
	local kit = discret and "TELL_MESSAGE" or ns.SONS[ns.db.reglages.choixSon or 1].kit
	local id = SOUNDKIT and SOUNDKIT[kit]
	-- canal Effets du jeu : le volume est celui des effets sonores
	if id and PlaySound then pcall(PlaySound, id, "SFX") end
end

-- ------------------------------------------------------------------ toasts

local ancre
local toasts = {}      -- les cadres, réutilisés
ns.toastsVisibles = {} -- annonces affichées, la plus récente en haut

local function placerAncre()
	if not ancre then
		ancre = CreateFrame("Frame", "AppelAllianceAncre", UIParent)
		ancre:SetSize(380, 10)
		ancre:SetMovable(true)
		ancre:SetClampedToScreen(true)
	end
	local p = ns.db.reglages.toast
	ancre:ClearAllPoints()
	if p then ancre:SetPoint(p[1], UIParent, p[1], p[2], p[3]) else ancre:SetPoint("TOP", UIParent, "TOP", 0, -120) end
end

function ns.replacerToasts()
	ns.db.reglages.toast = nil
	placerAncre()
	ns.dire("les alertes reviennent en haut au centre de l'écran.")
end

local function fermer(t)
	t:Hide()
	for i, a in ipairs(ns.toastsVisibles) do
		if a == t.annonce then table.remove(ns.toastsVisibles, i) break end
	end
	t.annonce = nil
	ns.empilerToasts()
end

local function nouveauCadre()
	local ok, t = pcall(CreateFrame, "Frame", nil, UIParent, "BackdropTemplate")
	if not ok or not t then t = CreateFrame("Frame", nil, UIParent) end
	t:SetSize(380, HAUTEUR)
	t:SetFrameStrata("DIALOG")
	t:EnableMouse(true)
	t:RegisterForDrag("LeftButton")
	-- glisser un toast déplace tous les toasts (F12 : position)
	t:SetScript("OnDragStart", function() ancre:StartMoving() end)
	t:SetScript("OnDragStop", function()
		ancre:StopMovingOrSizing()
		local point, _, _, x, y = ancre:GetPoint()
		ns.db.reglages.toast = { point, x, y }
	end)
	if t.SetBackdrop then
		t:SetBackdrop({ bgFile = "Interface\\Tooltips\\UI-Tooltip-Background",
			edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border", edgeSize = 16,
			insets = { left = 4, right = 4, top = 4, bottom = 4 } })
		t:SetBackdropColor(0.05, 0.05, 0.08, 0.94)
	end
	t.titre = ui.texte(t, "GameFontNormal", 10, -8)
	t.ligne = ui.texte(t, "GameFontHighlight", 10, -24)
	t.ligne:SetWidth(360)
	t.par = ui.texte(t, "GameFontHighlightSmall", 10, -40)
	t.interet = ui.bouton(t, "Intéressé", 100, function()
		if t.annonce then ns.interesser(t.annonce.id) end
		fermer(t)
	end)
	t.interet:SetPoint("BOTTOMRIGHT", -112, 6)
	t.ignorer = ui.bouton(t, "Ignorer", 100, function() fermer(t) end)
	t.ignorer:SetPoint("BOTTOMRIGHT", -8, 6)
	toasts[#toasts + 1] = t
	return t
end

function ns.empilerToasts()
	local i = 0
	for _, t in ipairs(toasts) do
		if t.annonce then
			t:ClearAllPoints()
			t:SetPoint("TOP", ancre, "TOP", 0, -i * (HAUTEUR + 4))
			i = i + 1
		end
	end
end

function ns.montrerToast(a)
	placerAncre()
	-- au-delà de 3, la plus ancienne laisse sa place
	if #ns.toastsVisibles >= MAX then
		local vieille = ns.toastsVisibles[#ns.toastsVisibles]
		for _, t in ipairs(toasts) do if t.annonce == vieille then fermer(t) break end end
	end
	local t
	for _, x in ipairs(toasts) do if not x.annonce then t = x break end end
	t = t or nouveauCadre()
	t.annonce = a
	table.insert(ns.toastsVisibles, 1, a)
	local couleur, c = ns.couleurGuilde(a.guilde)
	if t.SetBackdropBorderColor then t:SetBackdropBorderColor(c[1], c[2], c[3]) end
	local type = ns.typeDe(a.type)
	t.titre:SetText(couleur .. "[" .. ns.sigle(a.guilde) .. "]|r Nouvelle annonce · " .. (type and type.nom or ""))
	t.ligne:SetText(ui.resume(a):gsub("^[^:]*: ", "") .. (a.texte and (" · " .. a.texte) or ""))
	t.par:SetText("par " .. ns.prenom(a.auteur) .. " (" .. (a.guilde or "?") .. ")"
		.. (a.discord and (" · Discord " .. a.discord) or ""))
	t:Show()
	ns.empilerToasts()
	ns.apres(DUREE, function()
		if t.annonce == a then fermer(t) end
	end)
end

-- ------------------------------------------------------------------ ligne dans le chat

-- Un lien cliquable qui rouvre l'annonce. Le type « garrmission » est celui que le jeu
-- laisse passer sans erreur ; on reconnaît les nôtres à « appelalliance ».
function ns.lienAnnonce(a)
	return ("|cffd4a017|Hgarrmission:appelalliance:%s|h[%s]|h|r"):format(a.id, ui.resume(a))
end

local function ligneChat(a)
	local couleur = ns.couleurGuilde(a.guilde)
	print(("%s[%s]|r %s par %s%s"):format(couleur, ns.sigle(a.guilde), ns.lienAnnonce(a), ns.prenom(a.auteur),
		a.texte and (" : " .. a.texte) or ""))
end

if hooksecurefunc and SetItemRef then
	hooksecurefunc("SetItemRef", function(lien)
		local id = type(lien) == "string" and lien:match("^garrmission:appelalliance:(.+)$")
		if id then ns.ouvrirFiche(id) end
	end)
end

-- ------------------------------------------------------------------ à réception

ns.surSignal("nouvelle", function(a, alerter)
	ligneChat(a)
	if alerter then
		ns.jouerSon()
		ns.montrerToast(a)
	end
end)

-- Pour l'initiateur : un son discret à chaque nouvel intéressé.
ns.surSignal("interesse", function(a, nom)
	ns.jouerSon(false, true)
	ns.dire(ns.prenom(nom) .. " est intéressé par ton annonce.")
end)

-- Une annonce close disparaît aussi de l'écran.
ns.surSignal("annonces", function(id, motif)
	if not motif then return end
	for _, t in ipairs(toasts) do
		if t.annonce and t.annonce.id == id then fermer(t) end
	end
end)

-- ------------------------------------------------------------------ fiche de l'initiateur (F10)

local fiche
function ns.ouvrirFiche(id)
	local a = ns.annonces[id]
	if not a then
		ns.dire("cette annonce n'est plus active.")
		return
	end
	if not fiche then
		local f = CreateFrame("Frame", "AppelAllianceFiche", UIParent, "BasicFrameTemplateWithInset")
		f:SetSize(400, 230)
		f:SetPoint("CENTER", 0, 80)
		f:SetFrameStrata("DIALOG")
		f:SetMovable(true)
		f:EnableMouse(true)
		f:RegisterForDrag("LeftButton")
		f:SetScript("OnDragStart", f.StartMoving)
		f:SetScript("OnDragStop", f.StopMovingOrSizing)
		tinsert(UISpecialFrames, "AppelAllianceFiche")
		ui.texte(f, "GameFontNormal", 0, -6, "TOP"):SetText("Annonce")
		f.resume = ui.texte(f, "GameFontHighlight", 16, -34)
		f.resume:SetWidth(368)
		f.texte = ui.texte(f, "GameFontHighlightSmall", 16, -54)
		f.texte:SetWidth(368)
		f.auteur = ui.texte(f, "GameFontHighlight", 16, -82)
		ui.texte(f, "GameFontHighlightSmall", 16, -110):SetText("Discord de sa guilde (à copier) :")
		f.lien = ui.saisie(f, 240, 140, -104)
		f.lien:SetScript("OnTextChanged", function(e, saisi) if saisi then e:SetText(e.lien or "") e:HighlightText() end end)
		f.interet = ui.bouton(f, "Intéressé", 110, function()
			if f.id then ns.interesser(f.id) ns.ouvrirFiche(f.id) end
		end)
		f.interet:SetPoint("BOTTOMLEFT", 14, 12)
		f.chuchoter = ui.bouton(f, "Chuchoter", 100, function()
			local x = f.id and ns.annonces[f.id]
			if x then ui.chuchoter(x.auteur) end
		end)
		f.chuchoter:SetPoint("BOTTOMLEFT", 128, 12)
		f.bloquer = ui.bouton(f, "Bloquer", 90, function()
			local x = f.id and ns.annonces[f.id]
			if x then ns.bloquer(x.auteur, true) f:Hide() end
		end)
		f.bloquer:SetPoint("BOTTOMRIGHT", -14, 12)
		fiche = f
	end
	local f = fiche
	f.id = id
	f.resume:SetText(ui.resume(a))
	f.texte:SetText(a.texte or "")
	f.auteur:SetText("par " .. ns.prenom(a.auteur) .. " " .. ns.couleurGuilde(a.guilde) .. "(" .. (a.guilde or "?") .. ")|r"
		.. (a.discord and ("  · Discord : " .. a.discord) or ""))
	local g = ns.guilde(a.guilde)
	f.lien.lien = (g and g.discord ~= "" and g.discord) or "lien pas encore fourni"
	f.lien:SetText(f.lien.lien)
	local moi = a.auteur == ns.moi()
	if moi then f.interet:Hide() f.bloquer:Hide() else
		f.interet:Show() f.bloquer:Show()
		f.interet:SetText(a.monInteret and "Plus intéressé" or "Intéressé")
	end
	f:Show()
end
