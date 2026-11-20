-- KFIRE Guilde : la fenêtre (menu à gauche, une page par fonction), le formulaire
-- de profil, le médaillon à l'écran et la commande /ke.
--
-- Les pages Demandes, Artisanat, Événements, Statistiques et Quêtes sont dans les
-- fichiers Page*.lua : elles s'inscrivent dans ns.constructeurs et utilisent ns.ui.
local _, ns = ...

local LARGEUR, HAUTEUR = 860, 540
local LIGNES = 16          -- membres visibles sur la page Guilde
local HAUTEUR_LIGNE = 22
local ORANGE = "|cffff8a3d"
local OR = "|cfff8d16b"
local GRIS = "|cff9a9488"
local VERT = "|cff3ddc68"
local BLEU = "|cff3fa2ff"

local fenetre, pages, boutonsMenu = nil, {}, {}
ns.pages = pages  -- pour le banc de test et le diagnostic
local etatGuilde = { filtre = "tous", tri = "enligne", recherche = "", decalage = 0 }

-- ------------------------------------------------------------------ petits outils

local function texte(parent, police, x, y, point)
	local fs = parent:CreateFontString(nil, "OVERLAY", police or "GameFontHighlight")
	fs:SetPoint(point or "TOPLEFT", x or 0, y or 0)
	fs:SetJustifyH("LEFT")
	return fs
end

local function bouton(parent, libelle, largeur, fn)
	local b = CreateFrame("Button", nil, parent, "UIPanelButtonTemplate")
	b:SetSize(largeur or 110, 22)
	b:SetText(libelle)
	b:SetScript("OnClick", fn)
	return b
end

local function couleurClasse(fichier)
	local c = fichier and RAID_CLASS_COLORS and RAID_CLASS_COLORS[fichier]
	if c and c.colorStr then return "|c" .. c.colorStr end
	return "|cffffffff"
end

local function absence(l)
	if l.enLigne then return VERT .. (l.zone or "en ligne") .. "|r" end
	local j = l.absentJours
	if not j then return GRIS .. "hors ligne|r" end
	if j < 1 then return GRIS .. "aujourd'hui|r" end
	return GRIS .. ("il y a %d j"):format(math.floor(j)) .. "|r"
end

local COULEUR_VERSION = { ajour = VERT, ancienne = BLEU, recente = OR }
local function version(l)
	if not l.version then return GRIS .. "—|r" end
	return (COULEUR_VERSION[l.etatVersion] or "") .. l.version .. "|r"
end

local function chuchoter(nom)
	local fn = (ChatFrameUtil and ChatFrameUtil.SendTell) or ChatFrame_SendTell
	if fn then fn(nom) end
end

local function inviter(nom)
	if C_PartyInfo and C_PartyInfo.InviteUnit then C_PartyInfo.InviteUnit(nom)
	elseif InviteUnit then InviteUnit(nom) end
end

local function saisie(parent, largeur, x, y)
	local e = CreateFrame("EditBox", nil, parent, "InputBoxTemplate")
	e:SetSize(largeur, 22)
	e:SetPoint("TOPLEFT", x, y)
	e:SetAutoFocus(false)
	return e
end

-- Les outils partagés avec les pages des autres fichiers.
ns.ui = {
	texte = texte, bouton = bouton, saisie = saisie, couleurClasse = couleurClasse,
	chuchoter = chuchoter, inviter = inviter,
	ORANGE = ORANGE, OR = OR, GRIS = GRIS, VERT = VERT, BLEU = BLEU, ROUGE = "|cffee7f75",
}
ns.constructeurs = {}   -- [code de page] = function(page)

-- Une liste de lignes cliquables avec la molette : { lignes, rafraichir(donnees) }.
-- faire(ligne) crée les textes d'une ligne ; remplir(ligne, donnee) les met à jour.
function ns.ui.liste(p, y, nombre, hauteur, largeur, faire, remplir)
	local l = { lignes = {}, decalage = 0, donnees = {} }
	for i = 1, nombre do
		local r = CreateFrame("Button", nil, p)
		r:SetSize(largeur, hauteur)
		r:SetPoint("TOPLEFT", 0, y - (i - 1) * hauteur)
		r:RegisterForClicks("LeftButtonUp", "RightButtonUp")
		r:SetHighlightTexture("Interface\\QuestFrame\\UI-QuestTitleHighlight", "ADD")
		faire(r)
		r:Hide()
		l.lignes[i] = r
	end
	function l.rafraichir(donnees)
		l.donnees = donnees or l.donnees
		l.decalage = math.max(0, math.min(l.decalage, #l.donnees - nombre))
		for i = 1, nombre do
			local r, d = l.lignes[i], l.donnees[i + l.decalage]
			r.donnee = d
			if d then remplir(r, d) r:Show() else r:Hide() end
		end
	end
	p:EnableMouseWheel(true)
	p:SetScript("OnMouseWheel", function(_, sens)
		l.decalage = math.max(0, l.decalage - sens * 3)
		l.rafraichir()
	end)
	return l
end

-- ------------------------------------------------------------------ formulaire de profil

local formulaire
local function ouvrirProfil()
	if not formulaire then
		local f = CreateFrame("Frame", "KFireGuildeProfil", UIParent, "BasicFrameTemplateWithInset")
		f:SetSize(380, 250)
		f:SetPoint("CENTER")
		f:SetFrameStrata("DIALOG")
		f:SetMovable(true)
		f:EnableMouse(true)
		f:RegisterForDrag("LeftButton")
		f:SetScript("OnDragStart", f.StartMoving)
		f:SetScript("OnDragStop", f.StopMovingOrSizing)
		tinsert(UISpecialFrames, "KFireGuildeProfil")
		local titre = texte(f, "GameFontNormal", 0, -6, "TOP")
		titre:SetText("Complète ton profil")

		local function champ(libelle, y, aide)
			texte(f, "GameFontNormal", 20, y):SetText(libelle)
			local e = CreateFrame("EditBox", nil, f, "InputBoxTemplate")
			e:SetSize(200, 22)
			e:SetPoint("TOPLEFT", 150, y + 4)
			e:SetAutoFocus(false)
			local a = texte(f, "GameFontDisableSmall", 150, y - 18)
			a:SetText(aide)
			return e
		end
		f.pseudo = champ("Pseudo KFIRE", -40, "Le même que sur KFIRE et le Discord")
		f.main = champ("Perso principal", -90, "Son nom en jeu, sans le royaume")

		texte(f, "GameFontNormal", 20, -140):SetText("Rôle de ce perso")
		f.roles = {}
		local x = 150
		for _, code in ipairs({ "T", "H", "D" }) do
			local b = bouton(f, ns.ROLES[code], 66, function()
				f.role = code
				for c, rb in pairs(f.roles) do rb:SetAlpha(c == code and 1 or 0.45) end
			end)
			b:SetPoint("TOPLEFT", x, -136)
			f.roles[code] = b
			x = x + 70
		end
		f.message = texte(f, "GameFontRedSmall", 20, -175)
		local ok = bouton(f, "Enregistrer", 120, function()
			local reussi, err = ns.enregistrerProfil({ pseudo = f.pseudo:GetText(), main = f.main:GetText(), role = f.role })
			if reussi then f:Hide() else f.message:SetText(err) end
		end)
		ok:SetPoint("BOTTOM", 0, 16)
		formulaire = f
	end
	local p = ns.monProfil()
	formulaire.pseudo:SetText(p.pseudo or ns.pseudoPropose() or "")
	formulaire.main:SetText(p.main and ns.prenom(p.main) or "")
	formulaire.role = p.role
	for c, rb in pairs(formulaire.roles) do rb:SetAlpha((p.role == nil or c == p.role) and 1 or 0.45) end
	formulaire.message:SetText("")
	formulaire:Show()
end
ns.ouvrirProfil = ouvrirProfil

-- ------------------------------------------------------------------ page Guilde

local function infobulle(cadre, l)
	GameTooltip:SetOwner(cadre, "ANCHOR_RIGHT")
	GameTooltip:AddLine(couleurClasse(l.classeFichier) .. l.affichage .. "|r")
	GameTooltip:AddLine(("Niveau %d %s, %s"):format(l.niveau or 0, l.classe or "", l.rang or ""), 1, 1, 1)
	if l.main and l.main ~= l.nom then GameTooltip:AddLine("Perso principal : " .. ns.prenom(l.main), 1, 1, 1) end
	if l.role then GameTooltip:AddLine("Rôle : " .. ns.ROLES[l.role], 1, 1, 1) end
	if l.version then
		GameTooltip:AddLine(("KFIRE Guilde %s, vu le %s"):format(l.version, date("%d/%m à %Hh%M", l.vuAddon or 0)), 0.6, 0.6, 0.6)
	else
		GameTooltip:AddLine("Sans l'addon KFIRE Guilde", 0.6, 0.6, 0.6)
	end
	if l.note and l.note ~= "" then GameTooltip:AddLine("Note : " .. l.note, 0.8, 0.8, 0.8, true) end
	GameTooltip:AddLine("Clic : chuchoter. Clic droit : inviter dans le groupe.", 0.25, 0.63, 1)
	GameTooltip:Show()
end

local function rafraichirGuilde()
	local p = pages.guilde
	if not p or not p:IsShown() then return end
	local liste = ns.listeGuilde(etatGuilde)
	local total, enLigne, addon = ns.compteGuilde()
	p.compte:SetText(("%d membres, %s%d en ligne|r, %d avec l'addon"):format(total, VERT, enLigne, addon))
	local maxDec = math.max(0, #liste - LIGNES)
	etatGuilde.decalage = math.min(etatGuilde.decalage, maxDec)
	for i = 1, LIGNES do
		local r, l = p.lignes[i], liste[i + etatGuilde.decalage]
		r.donnee = l
		if l then
			r.nom:SetText((l.enLigne and "" or GRIS) .. couleurClasse(l.classeFichier) .. l.affichage .. "|r")
			r.niveau:SetText(tostring(l.niveau or ""))
			r.role:SetText(l.role and ns.ROLES[l.role] or GRIS .. "—|r")
			r.rang:SetText(l.rang or "")
			r.zone:SetText(absence(l))
			r.version:SetText(version(l))
			r:SetAlpha(l.enLigne and 1 or 0.6)
			r:Show()
		else
			r:Hide()
		end
	end
	p.position:SetText(#liste == 0 and "Aucun membre ne correspond"
		or ("%d–%d sur %d"):format(etatGuilde.decalage + 1, math.min(#liste, etatGuilde.decalage + LIGNES), #liste))
	for code, b in pairs(p.filtres) do b:SetAlpha(code == etatGuilde.filtre and 1 or 0.5) end
end
ns.rafraichirGuilde = rafraichirGuilde

local LIBELLES_TRI = { enligne = "En ligne d'abord", niveau = "Par niveau", rang = "Par rang", version = "Par version" }

local function construireGuilde(p)
	p.compte = texte(p, "GameFontNormal", 0, 0)
	local recherche = CreateFrame("EditBox", nil, p, "InputBoxTemplate")
	recherche:SetSize(180, 22)
	recherche:SetPoint("TOPLEFT", 0, -26)
	recherche:SetAutoFocus(false)
	recherche:SetScript("OnTextChanged", function(e)
		etatGuilde.recherche = e:GetText() or ""
		etatGuilde.decalage = 0
		rafraichirGuilde()
	end)
	texte(p, "GameFontDisableSmall", 190, -32):SetText("nom, pseudo, zone, version")

	p.filtres = {}
	local x = 360
	for _, f in ipairs({ { "tous", "Tous" }, { "enligne", "En ligne" }, { "addon", "Avec l'addon" } }) do
		local b = bouton(p, f[2], 90, function()
			etatGuilde.filtre = f[1]
			etatGuilde.decalage = 0
			rafraichirGuilde()
		end)
		b:SetPoint("TOPLEFT", x, -26)
		p.filtres[f[1]] = b
		x = x + 94
	end
	local tri
	tri = bouton(p, LIBELLES_TRI.enligne, 120, function()
		for i, code in ipairs(ns.TRIS) do
			if code == etatGuilde.tri then
				etatGuilde.tri = ns.TRIS[i % #ns.TRIS + 1]
				break
			end
		end
		tri:SetText(LIBELLES_TRI[etatGuilde.tri])
		rafraichirGuilde()
	end)
	tri:SetPoint("TOPRIGHT", 0, -26)

	local colonnes = { { "Membre", 0 }, { "Niv.", 210 }, { "Rôle", 250 }, { "Rang", 330 }, { "Zone / dernière connexion", 440 }, { "Addon", 600 } }
	for _, c in ipairs(colonnes) do texte(p, "GameFontNormalSmall", c[2] + 4, -58):SetText(c[1]) end

	p.lignes = {}
	for i = 1, LIGNES do
		local r = CreateFrame("Button", nil, p)
		r:SetSize(660, HAUTEUR_LIGNE)
		r:SetPoint("TOPLEFT", 0, -72 - (i - 1) * HAUTEUR_LIGNE)
		r:RegisterForClicks("LeftButtonUp", "RightButtonUp")
		r:SetHighlightTexture("Interface\\QuestFrame\\UI-QuestTitleHighlight", "ADD")
		r.nom = texte(r, "GameFontHighlight", 4, -4)
		r.niveau = texte(r, "GameFontHighlight", 214, -4)
		r.role = texte(r, "GameFontHighlightSmall", 254, -5)
		r.rang = texte(r, "GameFontHighlightSmall", 334, -5)
		r.zone = texte(r, "GameFontHighlightSmall", 444, -5)
		r.version = texte(r, "GameFontHighlightSmall", 604, -5)
		r:SetScript("OnEnter", function(self) if self.donnee then infobulle(self, self.donnee) end end)
		r:SetScript("OnLeave", function() GameTooltip:Hide() end)
		r:SetScript("OnClick", function(self, clic)
			if not self.donnee then return end
			if clic == "RightButton" then inviter(self.donnee.nom) else chuchoter(self.donnee.nom) end
		end)
		p.lignes[i] = r
	end
	p:EnableMouseWheel(true)
	p:SetScript("OnMouseWheel", function(_, sens)
		etatGuilde.decalage = math.max(0, etatGuilde.decalage - sens * 3)
		rafraichirGuilde()
	end)
	p.position = texte(p, "GameFontDisableSmall", 0, 0, "BOTTOMLEFT")
	local monProfil = bouton(p, "Mon profil", 110, ouvrirProfil)
	monProfil:SetPoint("BOTTOMRIGHT", 0, -4)
	p:SetScript("OnShow", function()
		ns.demanderRoster()
		rafraichirGuilde()
	end)
end

-- ------------------------------------------------------------------ Accueil et Options

local function construireAccueil(p)
	p.titre = texte(p, "GameFontNormalLarge", 0, 0)
	p.corps = texte(p, "GameFontHighlight", 0, -34)
	p.corps:SetWidth(640)
	p:SetScript("OnShow", function()
		local total, enLigne, addon = ns.compteGuilde()
		p.titre:SetText("Bienvenue dans KFIRE Guilde")
		local lignes = {
			("%d membres dans la guilde, %d en ligne, %d avec l'addon."):format(total, enLigne, addon),
			"",
		}
		local ouvertes, pourMoi = ns.compteDemandes()
		lignes[#lignes + 1] = OR .. "Demandes|r : " .. ouvertes .. " ouverte(s)"
			.. (pourMoi > 0 and (", " .. VERT .. pourMoi .. " que tu peux honorer|r") or "") .. "."
		local evs = ns.listeEvenements()
		if #evs > 0 then
			lignes[#lignes + 1] = OR .. "Prochain événement|r : " .. evs[1].titre .. ", " .. date("%A %d/%m à %Hh%M", evs[1].quand) .. "."
		elseif ns.calendrierDisponible() then
			lignes[#lignes + 1] = OR .. "Événements|r : rien de prévu dans les 30 jours."
		end
		local qc = ns.quetesEnCommun()
		if #qc > 0 then
			local noms = {}
			for i = 1, math.min(3, #qc) do noms[i] = qc[i].affichage end
			lignes[#lignes + 1] = OR .. "Quêtes en commun|r avec " .. table.concat(noms, ", ") .. (#qc > 3 and "…" or "") .. "."
		end
		local c = ns.meilleursCreneaux(1)[1]
		if c then
			lignes[#lignes + 1] = OR .. "Meilleur créneau|r : " .. ns.JOURS[c.jour] .. " " .. c.heure .. "h ("
				.. ("%.1f"):format(c.moyenne) .. " en ligne en moyenne)."
		end
		local s = ns.synchro.recue
		if s then
			lignes[#lignes + 1] = ""
			lignes[#lignes + 1] = GRIS .. ("Synchronisé avec %s : %d fiche(s).|r"):format(ns.affichage(s.de), s.nombre)
		end
		p.corps:SetText(table.concat(lignes, "\n"))
	end)
end

local function compter(magasin)
	local n = 0
	for _ in pairs(ns.fiches(magasin)) do n = n + 1 end
	return n
end

local function diagnostic()
	local s = ns.stats
	local _, _, _, interface = GetBuildInfo()
	return table.concat({
		("KFIRE Guilde %s, protocole %d, interface %s"):format(ns.VERSION, ns.PROTO, tostring(interface)),
		("Moi : %s, guilde : %s"):format(tostring(ns.moi()), tostring(GetGuildInfo("player"))),
		("Liste de guilde : %d membres%s"):format((select(1, ns.compteGuilde())), ns.rosterDate and "" or " (pas encore reçue)"),
		("Messages : %d envoyés, %d refusés pour débit, %d échecs, %d reçus, %d ignorés, %d en attente")
			:format(s.envoyes, s.refusDebit, s.echecs, s.recus, s.ignores, ns.enAttente()),
		("En instance : %s ; dernière synchro : %s"):format(ns.enInstance and "oui" or "non",
			ns.db.derniereSynchro > 0 and date("%d/%m %Hh%M", ns.db.derniereSynchro) or "jamais"),
		("Fiches : %d membres, %d demandes, %d métiers, %d jours de présence, %d listes de quêtes ; calendrier : %s")
			:format(compter("membres"), compter("demandes"), compter("metiers"), compter("presence"), compter("quetes"),
				ns.calendrierDisponible() and "lisible" or "absent"),
		"Dernière erreur : " .. (ns.derniereErreur or "aucune"),
	}, "\n")
end
ns.diagnostic = diagnostic

local function construireOptions(p)
	local c = CreateFrame("CheckButton", nil, p, "UICheckButtonTemplate")
	c:SetPoint("TOPLEFT", 0, 0)
	texte(p, "GameFontHighlight", 32, -7):SetText("Afficher le médaillon KFIRE Guilde à l'écran")
	c:SetScript("OnClick", function(self)
		ns.db.reglages.medaillonCache = not self:GetChecked()
		ns.majMedaillon()
	end)
	local cases = {}
	local y = -28
	for _, b in ipairs({
		{ "bullesDemandesCoupees", "Bulle quand un membre demande ce que je peux honorer" },
		{ "bullesEvenementsCoupees", "Bulle pour chaque nouvel événement de guilde" },
		{ "bullesQuetesCoupees", "Bulle quand un membre en ligne fait une de mes quêtes" },
	}) do
		local k = CreateFrame("CheckButton", nil, p, "UICheckButtonTemplate")
		k:SetPoint("TOPLEFT", 0, y)
		texte(p, "GameFontHighlight", 32, y - 7):SetText(b[2])
		k:SetScript("OnClick", function(self) ns.db.reglages[b[1]] = not self:GetChecked() end)
		cases[b[1]] = k
		y = y - 28
	end
	bouton(p, "Mon profil", 120, ouvrirProfil):SetPoint("TOPLEFT", 0, y - 12)
	texte(p, "GameFontNormal", 0, y - 56):SetText("Diagnostic (à copier en cas de problème, /ke diag)")
	local d = texte(p, "GameFontHighlightSmall", 0, y - 76)
	d:SetWidth(640)
	p:SetScript("OnShow", function()
		c:SetChecked(not ns.db.reglages.medaillonCache)
		for cle, k in pairs(cases) do k:SetChecked(not ns.db.reglages[cle]) end
		d:SetText(diagnostic())
	end)
end

local function construireDepuis(code, ce)
	return function(p)
		local fn = ns.constructeurs[code]
		if fn then return fn(p) end
		texte(p, "GameFontNormalLarge", 0, 0):SetText(ce)
	end
end

-- ------------------------------------------------------------------ la fenêtre

local SECTIONS = {
	{ "accueil", "Accueil", construireAccueil },
	{ "guilde", "Guilde", construireGuilde },
	{ "demandes", "Demandes", construireDepuis("demandes", "Demandes de la guilde") },
	{ "artisanat", "Artisanat", construireDepuis("artisanat", "Qui sait fabriquer quoi") },
	{ "evenements", "Événements", construireDepuis("evenements", "Événements de la guilde") },
	{ "stats", "Statistiques", construireDepuis("stats", "Présence de la guilde") },
	{ "quetes", "Quêtes", construireDepuis("quetes", "Quêtes en commun") },
	{ "options", "Options", construireOptions },
}

local function majAlerteProfil()
	if not fenetre then return end
	local manque = ns.profilManquant()
	if #manque > 0 then
		fenetre.alerte:SetText(ORANGE .. "Profil incomplet|r : il manque " .. table.concat(manque, ", ") .. ". Clique ici.")
		fenetre.alerteBouton:Show()
	else
		fenetre.alerteBouton:Hide()
	end
end

local function montrer(code)
	for c, p in pairs(pages) do
		if c == code then p:Show() else p:Hide() end
	end
	for c, b in pairs(boutonsMenu) do b:SetAlpha(c == code and 1 or 0.6) end
	ns.db.reglages.page = code
end

local function construireFenetre()
	local f = CreateFrame("Frame", "KFireGuildeFenetre", UIParent, "BasicFrameTemplateWithInset")
	f:SetSize(LARGEUR, HAUTEUR)
	f:SetPoint("CENTER")
	f:SetFrameStrata("HIGH")
	f:SetMovable(true)
	f:EnableMouse(true)
	f:SetClampedToScreen(true)
	f:RegisterForDrag("LeftButton")
	f:SetScript("OnDragStart", f.StartMoving)
	f:SetScript("OnDragStop", f.StopMovingOrSizing)
	tinsert(UISpecialFrames, "KFireGuildeFenetre")
	texte(f, "GameFontNormal", 0, -6, "TOP"):SetText("KFIRE Guilde — Knights of Eternity")

	f.alerteBouton = CreateFrame("Button", nil, f)
	f.alerteBouton:SetSize(LARGEUR - 40, 18)
	f.alerteBouton:SetPoint("TOPLEFT", 20, -30)
	f.alerte = texte(f.alerteBouton, "GameFontHighlightSmall", 0, -2)
	f.alerteBouton:SetScript("OnClick", ouvrirProfil)

	local y = -56
	for _, s in ipairs(SECTIONS) do
		local b = bouton(f, s[2], 140, function() montrer(s[1]) end)
		b:SetPoint("TOPLEFT", 16, y)
		boutonsMenu[s[1]] = b
		y = y - 28
		local p = CreateFrame("Frame", nil, f)
		p:SetPoint("TOPLEFT", 176, -56)
		p:SetPoint("BOTTOMRIGHT", -18, 18)
		p:Hide()
		s[3](p)
		pages[s[1]] = p
	end
	f:SetScript("OnShow", majAlerteProfil)
	fenetre = f
	return f
end

function ns.ouvrir(code)
	if not fenetre then construireFenetre() end
	fenetre:Show()
	majAlerteProfil()
	montrer(code or ns.db.reglages.page or "accueil")
	-- La première fois de la session, un profil incomplet ouvre le formulaire.
	if #ns.profilManquant() > 0 and not ns.profilPropose then
		ns.profilPropose = true
		ouvrirProfil()
	end
end

ns.montrerPage = function(code) ns.ouvrir(code) end

function ns.basculer()
	if fenetre and fenetre:IsShown() then fenetre:Hide() else ns.ouvrir() end
end

ns.surSignal("roster", rafraichirGuilde)
ns.surSignal("membres", rafraichirGuilde)
ns.surSignal("profil", function()
	majAlerteProfil()
	rafraichirGuilde()
end)

-- ------------------------------------------------------------------ bulles

-- Une petite annonce en haut de l'écran pendant quelques secondes (et dans la
-- discussion) ; un clic ouvre la page concernée. Elles passent une par une.
local DUREE_BULLE = 7
local bulleCadre, fileBulles, bulleOccupee = nil, {}, false
ns.bullesRecentes = {}

local function bulleSuivante()
	local b = table.remove(fileBulles, 1)
	if not b then
		bulleOccupee = false
		if bulleCadre then bulleCadre:Hide() end
		return
	end
	bulleOccupee = true
	if not bulleCadre then
		-- BackdropTemplate n'existe pas sur tous les clients : sans lui, une bulle sans cadre
		local ok, f = pcall(CreateFrame, "Button", "KFireGuildeBulle", UIParent, "BackdropTemplate")
		if not ok or not f then f = CreateFrame("Button", "KFireGuildeBulle", UIParent) end
		f:SetSize(420, 44)
		f:SetPoint("TOP", 0, -140)
		f:SetFrameStrata("DIALOG")
		if f.SetBackdrop then
			f:SetBackdrop({ bgFile = "Interface\\Tooltips\\UI-Tooltip-Background",
				edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border", edgeSize = 14,
				insets = { left = 3, right = 3, top = 3, bottom = 3 } })
			f:SetBackdropColor(0.05, 0.05, 0.08, 0.92)
		end
		local icone = f:CreateTexture(nil, "ARTWORK")
		icone:SetSize(30, 30)
		icone:SetPoint("LEFT", 8, 0)
		icone:SetTexture("Interface\\Icons\\INV_Misc_Tournaments_banner_Human")
		f.texte = texte(f, "GameFontHighlight", 46, 0, "LEFT")
		f.texte:SetWidth(360)
		f:SetScript("OnClick", function(self)
			if self.page then ns.ouvrir(self.page) end
			self:Hide()
		end)
		bulleCadre = f
	end
	bulleCadre.texte:SetText(b.texte)
	bulleCadre.page = b.page
	bulleCadre:Show()
	if PlaySound and SOUNDKIT and SOUNDKIT.TELL_MESSAGE then pcall(PlaySound, SOUNDKIT.TELL_MESSAGE) end
	ns.apres(DUREE_BULLE, bulleSuivante)
end

-- page : la page qu'ouvre un clic sur la bulle
function ns.bulle(texteBulle, page)
	if ns.db and ns.db.reglages.bullesCoupees then return end
	table.insert(ns.bullesRecentes, 1, { texte = texteBulle, page = page, date = ns.maintenant() })
	if #ns.bullesRecentes > 20 then table.remove(ns.bullesRecentes) end
	ns.dire(texteBulle)
	-- en combat ou en instance, la discussion suffit
	if ns.enInstance or (InCombatLockdown and InCombatLockdown()) then return end
	fileBulles[#fileBulles + 1] = { texte = texteBulle, page = page }
	if not bulleOccupee then bulleSuivante() end
end

-- ------------------------------------------------------------------ médaillon

local medaillon
function ns.majMedaillon()
	if ns.db.reglages.medaillonCache then
		if medaillon then medaillon:Hide() end
		return
	end
	if not medaillon then
		local b = CreateFrame("Button", "KFireGuildeMedaillon", UIParent)
		b:SetSize(36, 36)
		b:SetFrameStrata("MEDIUM")
		b:SetMovable(true)
		b:SetClampedToScreen(true)
		b:RegisterForDrag("LeftButton")
		b:RegisterForClicks("LeftButtonUp", "RightButtonUp")
		local fond = b:CreateTexture(nil, "BACKGROUND")
		fond:SetAllPoints()
		fond:SetTexture("Interface\\Icons\\INV_Misc_Tournaments_banner_Human")
		local p = ns.db.reglages.medaillon
		if p then b:SetPoint(p[1], UIParent, p[1], p[2], p[3]) else b:SetPoint("TOPRIGHT", -220, -40) end
		b:SetScript("OnDragStart", b.StartMoving)
		b:SetScript("OnDragStop", function(self)
			self:StopMovingOrSizing()
			local point, _, _, x, y = self:GetPoint()
			ns.db.reglages.medaillon = { point, x, y }
		end)
		b:SetScript("OnClick", function(_, clic)
			if clic == "RightButton" then ns.ouvrirProfil() else ns.basculer() end
		end)
		b:SetScript("OnEnter", function(self)
			GameTooltip:SetOwner(self, "ANCHOR_LEFT")
			GameTooltip:AddLine("KFIRE Guilde")
			GameTooltip:AddLine("Clic : ouvrir. Clic droit : mon profil. Glisser : déplacer.", 1, 1, 1)
			GameTooltip:Show()
		end)
		b:SetScript("OnLeave", function() GameTooltip:Hide() end)
		medaillon = b
	end
	medaillon:Show()
end

ns.surSignal("charge", ns.majMedaillon)

-- ------------------------------------------------------------------ /ke

SLASH_KFIREGUILDE1 = "/ke"
SLASH_KFIREGUILDE2 = "/kfg"
SlashCmdList.KFIREGUILDE = function(arg)
	local cmd = (arg or ""):lower():match("^%s*(%S*)")
	if cmd == "" then ns.basculer()
	elseif cmd == "guilde" then ns.ouvrir("guilde")
	elseif cmd == "profil" then ns.ouvrirProfil()
	elseif cmd == "options" then ns.ouvrir("options")
	elseif cmd == "demandes" or cmd == "artisanat" or cmd == "evenements" or cmd == "stats" or cmd == "quetes" then
		ns.ouvrir(cmd)
	elseif cmd == "synchro" then
		ns.demanderSynchro()
		ns.dire("synchronisation demandée à la guilde.")
	elseif cmd == "diag" then
		for l in diagnostic():gmatch("[^\n]+") do ns.dire(l) end
	elseif cmd == "medaillon" then
		ns.db.reglages.medaillonCache = not ns.db.reglages.medaillonCache
		ns.majMedaillon()
	else
		ns.dire("/ke : la fenêtre ; /ke guilde | demandes | artisanat | evenements | stats | quetes | profil | options ; /ke synchro ; /ke diag ; /ke medaillon")
	end
end
