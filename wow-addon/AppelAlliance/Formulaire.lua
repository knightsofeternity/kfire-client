-- Appel d'Alliance : le formulaire de création (F01). Le type d'abord, puis seulement les
-- champs de ce type. Un donjon se publie en trois clics : Donjon, le donjon, Publier.
local _, ns = ...
local ui = ns.ui
local GRIS, OR = ui.GRIS, ui.OR

local LIGNES_CHOIX = 7
local f
local saisie = { type = "D", roles = "" }

-- Les lieux proposés : les donjons de mon niveau d'abord, ou les zones ; filtrés par le texte tapé.
function ns.choixLieux(genre, texte)
	texte = (texte or ""):lower()
	local res = {}
	if genre == "donjon" then
		local niv = ns.monNiveau()
		for _, d in ipairs(ns.DONJONS) do
			if texte == "" or d[1]:lower():find(texte, 1, true) then res[#res + 1] = { nom = d[1], niveau = d[2] } end
		end
		table.sort(res, function(a, b)
			local pa, pb = a.niveau <= niv, b.niveau <= niv
			if pa ~= pb then return pa end
			if a.niveau ~= b.niveau then
				if pa then return a.niveau > b.niveau end
				return a.niveau < b.niveau
			end
			return a.nom < b.nom
		end)
	elseif genre == "zone" then
		for _, z in ipairs(ns.ZONES) do
			if texte == "" or z:lower():find(texte, 1, true) then res[#res + 1] = { nom = z } end
		end
	end
	return res
end

local function majChamps()
	local t = ns.typeDe(saisie.type)
	for code, b in pairs(f.types) do b:SetAlpha(code == saisie.type and 1 or 0.45) end
	local c = t.champs
	local function montrer(cadre, oui) if oui then cadre:Show() else cadre:Hide() end end
	montrer(f.lieuTitre, c.lieu) montrer(f.lieu, c.lieu) montrer(f.choix, c.lieu)
	f.lieuTitre:SetText(c.lieu == "donjon" and "Donjon" or "Zone")
	for _, b in pairs(f.roles) do montrer(b, c.roles) end
	montrer(f.rolesTitre, c.roles)
	montrer(f.placesTitre, c.places) montrer(f.places, c.places)
	montrer(f.niveauTitre, c.niveau) montrer(f.niveau, c.niveau)
	f.texteTitre:SetText(c.quetes and "Quête(s) visée(s)" or "Texte (120 max.)")
	for code, b in pairs(f.roles) do b:SetAlpha(saisie.roles:find(code, 1, true) and 1 or 0.45) end
	f.duree:SetText(GRIS .. "Expire après " .. math.floor(t.duree / 60) .. " min.|r")
	if c.lieu then
		f.choix.rafraichir(ns.choixLieux(c.lieu, f.lieu:GetText()))
	end
end

local function construire()
	f = CreateFrame("Frame", "AppelAllianceFormulaire", UIParent, "BasicFrameTemplateWithInset")
	f:SetSize(460, 430)
	f:SetPoint("CENTER", 120, 0)
	f:SetFrameStrata("DIALOG")
	f:SetMovable(true)
	f:EnableMouse(true)
	f:RegisterForDrag("LeftButton")
	f:SetScript("OnDragStart", f.StartMoving)
	f:SetScript("OnDragStop", f.StopMovingOrSizing)
	tinsert(UISpecialFrames, "AppelAllianceFormulaire")
	ui.texte(f, "GameFontNormal", 0, -6, "TOP"):SetText("Nouvelle annonce")

	f.types = {}
	local x = 14
	for _, t in ipairs(ns.TYPES) do
		local b = ui.bouton(f, t.nom, 104, function()
			saisie.type = t.code
			local c = t.champs
			if c.lieu then f.lieu:SetText(ns.db.dernier[c.lieu] or "") end
			majChamps()
		end)
		b:SetPoint("TOPLEFT", x, -32)
		f.types[t.code] = b
		x = x + 108
	end

	f.lieuTitre = ui.texte(f, "GameFontNormal", 16, -68)
	f.lieu = ui.saisie(f, 300, 130, -62)
	f.lieu:SetScript("OnTextChanged", function(_, saisi) if saisi then majChamps() end end)
	local cadreChoix = CreateFrame("Frame", nil, f)
	cadreChoix:SetPoint("TOPLEFT", 130, -88)
	cadreChoix:SetSize(300, LIGNES_CHOIX * 18)
	f.choix = ui.liste(cadreChoix, 0, LIGNES_CHOIX, 18, 300, function(r)
		r.nom = ui.texte(r, "GameFontHighlightSmall", 4, -3)
		r:SetScript("OnClick", function(self)
			ns.auClic()
			local d = self.donnee
			if not d then return end
			f.lieu:SetText(d.nom)
			if d.niveau and f.niveau:GetText() == "" then f.niveau:SetText(tostring(d.niveau)) end
			majChamps()
		end)
	end, function(r, d)
		local choisi = d.nom == f.lieu:GetText()
		r.nom:SetText((choisi and OR or "") .. d.nom .. "|r" .. (d.niveau and (GRIS .. "  " .. d.niveau .. "+|r") or ""))
	end)
	f.choix.Show = function() cadreChoix:Show() end
	f.choix.Hide = function() cadreChoix:Hide() end

	local y = -88 - LIGNES_CHOIX * 18 - 10
	f.rolesTitre = ui.texte(f, "GameFontNormal", 16, y - 6)
	f.rolesTitre:SetText("Il manque")
	f.roles = {}
	local xr = 130
	for _, code in ipairs({ "T", "H", "D" }) do
		local b = ui.bouton(f, ns.ROLES[code], 70, function()
			if saisie.roles:find(code, 1, true) then saisie.roles = saisie.roles:gsub(code, "")
			else saisie.roles = saisie.roles .. code end
			majChamps()
		end)
		b:SetPoint("TOPLEFT", xr, y)
		f.roles[code] = b
		xr = xr + 74
	end
	y = y - 30
	f.niveauTitre = ui.texte(f, "GameFontNormal", 16, y - 6)
	f.niveauTitre:SetText("Niveau mini")
	f.niveau = ui.saisie(f, 40, 130, y)
	f.placesTitre = ui.texte(f, "GameFontNormal", 200, y - 6)
	f.placesTitre:SetText("Places")
	f.places = ui.saisie(f, 40, 260, y)
	y = y - 30
	f.texteTitre = ui.texte(f, "GameFontNormal", 16, y - 6)
	f.texte = ui.saisie(f, 300, 130, y)
	f.texte:SetMaxLetters(120)
	y = y - 28
	f.duree = ui.texte(f, "GameFontHighlightSmall", 130, y)
	f.message = ui.texte(f, "GameFontRedSmall", 16, y - 18)
	f.message:SetWidth(420)

	f.rejoindre = ui.bouton(f, "Rejoindre l'existante", 160, function()
		if f.doublon then
			ns.interesser(f.doublon)
			ns.ouvrirFiche(f.doublon)
			f:Hide()
		end
	end)
	f.rejoindre:SetPoint("BOTTOMLEFT", 14, 14)
	f.forcer = ui.bouton(f, "Publier quand même", 150, function()
		ns.publierDepuisFormulaire(true)
	end)
	f.forcer:SetPoint("BOTTOMLEFT", 178, 14)
	f.publier = ui.bouton(f, "Publier", 100, function() ns.publierDepuisFormulaire(false) end)
	f.publier:SetPoint("BOTTOMRIGHT", -14, 14)
end

function ns.publierDepuisFormulaire(ignorerDoublon)
	local id, err, doublon = ns.publierAnnonce({
		type = saisie.type, lieu = f.lieu:GetText(), roles = saisie.roles, places = f.places:GetText(),
		niveau = f.niveau:GetText(), texte = f.texte:GetText(), ignorerDoublon = ignorerDoublon,
	})
	if id then
		f:Hide()
		ns.ouvrir("miennes")
		return id
	end
	f.message:SetText(err)
	f.doublon = doublon
	if doublon then f.rejoindre:Show() f.forcer:Show() else f.rejoindre:Hide() f.forcer:Hide() end
	return nil, err
end

function ns.ouvrirFormulaire()
	if not ns.actif() then ns.ouvrirAccueil() return end
	if not f then construire() end
	local raison = ns.empechementPublier()
	saisie.roles = ""
	f.places:SetText("")
	f.niveau:SetText("")
	f.texte:SetText("")
	local t = ns.typeDe(saisie.type)
	f.lieu:SetText(t.champs.lieu and (ns.db.dernier[t.champs.lieu] or "") or "")
	f.message:SetText(raison or "")
	f.rejoindre:Hide()
	f.forcer:Hide()
	f.doublon = nil
	f:Show()
	majChamps()
end
ns.formulaire = function() return f end
