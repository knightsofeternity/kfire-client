-- Appel d'Alliance : la fenêtre principale (Annonces, Mes annonces, Alliance, Réglages),
-- le bouton de la minicarte et /alliance.
--
-- Chaque bouton passe d'abord par ns.auClic() : en option B, c'est le seul moment où les
-- messages du canal peuvent partir.
local _, ns = ...

local LARGEUR, HAUTEUR = 720, 470
local GRIS, VERT, OR, ROUGE = "|cff9a9488", "|cff3ddc68", "|cfff8d16b", "|cffee7f75"

ns.ui = { GRIS = GRIS, VERT = VERT, OR = OR, ROUGE = ROUGE }
local ui = ns.ui

-- ------------------------------------------------------------------ petits outils

function ui.texte(parent, police, x, y, point)
	local fs = parent:CreateFontString(nil, "OVERLAY", police or "GameFontHighlight")
	fs:SetPoint(point or "TOPLEFT", x or 0, y or 0)
	fs:SetJustifyH("LEFT")
	return fs
end

function ui.bouton(parent, libelle, largeur, fn)
	local b = CreateFrame("Button", nil, parent, "UIPanelButtonTemplate")
	b:SetSize(largeur or 110, 22)
	b:SetText(libelle)
	b:SetScript("OnClick", function(self, ...)
		ns.auClic()
		fn(self, ...)
	end)
	return b
end

function ui.saisie(parent, largeur, x, y)
	local e = CreateFrame("EditBox", nil, parent, "InputBoxTemplate")
	e:SetSize(largeur, 22)
	e:SetPoint("TOPLEFT", x, y)
	e:SetAutoFocus(false)
	return e
end

function ui.case(parent, libelle, x, y, fn)
	local c = CreateFrame("CheckButton", nil, parent, "UICheckButtonTemplate")
	c:SetPoint("TOPLEFT", x, y)
	ui.texte(parent, "GameFontHighlight", x + 30, y - 7):SetText(libelle)
	c:SetScript("OnClick", function(self)
		ns.auClic()
		fn(self:GetChecked() and true or false)
	end)
	return c
end

function ui.couleurClasse(fichier)
	local c = fichier and RAID_CLASS_COLORS and RAID_CLASS_COLORS[fichier]
	if c and c.colorStr then return "|c" .. c.colorStr end
	return "|cffffffff"
end

function ui.chuchoter(nom)
	local fn = (ChatFrameUtil and ChatFrameUtil.SendTell) or ChatFrame_SendTell
	if fn then fn(nom) end
end

function ui.inviter(nom)
	if C_PartyInfo and C_PartyInfo.InviteUnit then C_PartyInfo.InviteUnit(nom)
	elseif InviteUnit then InviteUnit(nom) end
end

function ui.age(t)
	local s = ns.maintenant() - (t or 0)
	if s < 60 then return "à l'instant" end
	if s < 3600 then return ("il y a %d min"):format(math.floor(s / 60)) end
	if s < 86400 then return ("il y a %d h"):format(math.floor(s / 3600)) end
	return ("il y a %d j"):format(math.floor(s / 86400))
end

-- « Donjon : Les Mortemines · Heal · niv. 18+ »
function ui.resume(a)
	local t = ns.typeDe(a.type)
	local morceaux = { (t and t.nom or "?") .. (a.lieu and (" : " .. a.lieu) or "") }
	if a.roles then
		local r = {}
		for code in a.roles:gmatch(".") do r[#r + 1] = ns.ROLES[code] end
		morceaux[#morceaux + 1] = "il manque " .. table.concat(r, ", ")
	end
	if a.places then morceaux[#morceaux + 1] = a.places .. " place(s)" end
	if a.niveau then morceaux[#morceaux + 1] = "niv. " .. a.niveau .. "+" end
	return table.concat(morceaux, " · ")
end

function ui.auteur(a)
	local couleur = ns.couleurGuilde(a.guilde)
	return ns.prenom(a.auteur) .. " " .. couleur .. "(" .. ns.sigle(a.guilde) .. ")|r"
end

-- Une liste de lignes avec la molette.
function ui.liste(p, y, nombre, hauteur, largeur, faire, remplir)
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

-- ------------------------------------------------------------------ onglet Annonces

local etat = { type = nil, membres = "tous", recherche = "" }
local fenetre, pages, onglets = nil, {}, {}
ns.pages = pages

local function construireAnnonces(p)
	p.entete = ui.texte(p, "GameFontNormal", 0, 0)
	p.filtres = {}
	local x = 0
	for _, f in ipairs({ { nil, "Toutes" }, { "D", "Donjon" }, { "Q", "Quête" }, { "E", "Élite" }, { "L", "Libre" } }) do
		local b = ui.bouton(p, f[2], 80, function()
			etat.type = f[1]
			ns.rafraichir()
		end)
		b:SetPoint("TOPLEFT", x, -20)
		p.filtres[f[1] or "tous"] = b
		x = x + 84
	end
	ui.bouton(p, "Nouvelle annonce", 140, function() ns.ouvrirFormulaire() end):SetPoint("TOPRIGHT", 0, -20)
	p.liste = ui.liste(p, -50, 9, 40, 680, function(r)
		r.titre = ui.texte(r, "GameFontHighlight", 4, -4)
		r.detail = ui.texte(r, "GameFontHighlightSmall", 4, -22)
		r.detail:SetWidth(500)
		r.interet = ui.bouton(r, "Intéressé", 100, function(b)
			local d = b:GetParent().donnee
			if d then ns.interesser(d.id) ns.rafraichir() end
		end)
		r.interet:SetPoint("TOPRIGHT", -4, -8)
		r:SetScript("OnClick", function(self, clic)
			if not self.donnee then return end
			ns.auClic()
			if clic == "RightButton" then ui.chuchoter(self.donnee.auteur) else ns.ouvrirFiche(self.donnee.id) end
		end)
	end, function(r, a)
		local _, c = ns.couleurGuilde(a.guilde)
		r.titre:SetText(ui.resume(a))
		r.detail:SetText(("par %s · %s · %d intéressé(s)%s"):format(ui.auteur(a), ui.age(a.cree), a.nbInteresses or 0,
			a.texte and (GRIS .. " · " .. a.texte .. "|r") or ""))
		if a.auteur == ns.moi() then
			r.interet:Hide()
		else
			r.interet:Show()
			r.interet:SetText(a.monInteret and "Plus intéressé" or "Intéressé")
		end
		if r.SetBackdropBorderColor then r:SetBackdropBorderColor(c[1], c[2], c[3]) end
	end)
	p.vide = ui.texte(p, "GameFontDisable", 0, -70)
	p.pied = ui.texte(p, "GameFontDisableSmall", 0, 0, "BOTTOMLEFT")
end

local function rafraichirAnnonces(p)
	local liste = ns.listeAnnonces({ type = etat.type })
	local enLigne = ns.compteMembres()
	p.entete:SetText(("%d annonce(s) active(s) · %d membre(s) de l'alliance en ligne%s"):format(
		ns.nombreAnnonces(), enLigne, ns.db.reglages.nePasDeranger and (OR .. " · Ne pas déranger|r") or ""))
	for code, b in pairs(p.filtres) do b:SetAlpha(code == (etat.type or "tous") and 1 or 0.5) end
	p.liste.rafraichir(liste)
	p.vide:SetText(#liste == 0 and "Aucune annonce en ce moment. Lance la tienne !" or "")
	local pied = {}
	if not ns.dansLeCanal() then pied[#pied + 1] = ROUGE .. "Pas dans le canal " .. ns.nomCanal() .. "|r" end
	if ns.transport() == "B" then pied[#pied + 1] = "mode texte (B)" end
	if ns.enAttenteDeClic() > 0 then pied[#pied + 1] = OR .. ns.enAttenteDeClic() .. " message(s) partiront au prochain clic|r" end
	p.pied:SetText(table.concat(pied, " · "))
end

-- ------------------------------------------------------------------ onglet Mes annonces

local function construireMiennes(p)
	p.titre = ui.texte(p, "GameFontNormalLarge", 0, 0)
	p.titre:SetWidth(680)
	p.detail = ui.texte(p, "GameFontHighlight", 0, -24)
	p.detail:SetWidth(680)
	p.complet = ui.bouton(p, "Complet", 100, function() ns.cloturer("C") ns.rafraichir() end)
	p.complet:SetPoint("TOPLEFT", 0, -50)
	p.annuler = ui.bouton(p, "Annuler", 100, function() ns.cloturer("A") ns.rafraichir() end)
	p.annuler:SetPoint("TOPLEFT", 104, -50)
	p.moins = ui.bouton(p, "Une place de moins", 150, function()
		local a = ns.mienne and ns.annonces[ns.mienne]
		if a and a.places and a.places > 1 then ns.modifierAnnonce({ places = a.places - 1 }) ns.rafraichir() end
	end)
	p.moins:SetPoint("TOPLEFT", 208, -50)
	p.nouvelle = ui.bouton(p, "Nouvelle annonce", 140, function() ns.ouvrirFormulaire() end)
	p.nouvelle:SetPoint("TOPRIGHT", 0, -50)
	ui.texte(p, "GameFontNormal", 0, -84):SetText("Intéressés")
	p.liste = ui.liste(p, -102, 12, 24, 680, function(r)
		r.nom = ui.texte(r, "GameFontHighlight", 4, -5)
		r.info = ui.texte(r, "GameFontHighlightSmall", 230, -6)
		r.inviter = ui.bouton(r, "Inviter", 80, function(b)
			local d = b:GetParent().donnee
			if d then ui.inviter(d.nom) end
		end)
		r.inviter:SetPoint("TOPRIGHT", -90, -1)
		r.chuchoter = ui.bouton(r, "Chuchoter", 86, function(b)
			local d = b:GetParent().donnee
			if d then ui.chuchoter(d.nom) end
		end)
		r.chuchoter:SetPoint("TOPRIGHT", 0, -1)
	end, function(r, i)
		r.nom:SetText(ui.couleurClasse(i.classe) .. ns.prenom(i.nom) .. "|r " .. ns.couleurGuilde(i.guilde)
			.. "(" .. ns.sigle(i.guilde) .. ")|r")
		r.info:SetText(("niv. %s%s · %s"):format(i.niveau or "?", i.role and (" · " .. ns.ROLES[i.role]) or "", ui.age(i.quand)))
	end)
end

local function rafraichirMiennes(p)
	local a = ns.mienne and ns.annonces[ns.mienne]
	if not a then
		p.titre:SetText(GRIS .. "Tu n'as pas d'annonce active.|r")
		p.detail:SetText("")
		p.complet:Hide() p.annuler:Hide() p.moins:Hide()
		p.liste.rafraichir({})
		return
	end
	p.titre:SetText(ui.resume(a))
	local reste = math.max(0, a.expire - ns.maintenant())
	p.detail:SetText((a.texte and (a.texte .. "  ") or "") .. GRIS .. (a.differee and "partira à la sortie de l'instance"
		or ("expire dans %d min"):format(math.ceil(reste / 60))) .. "|r")
	p.complet:Show() p.annuler:Show()
	if a.places and a.places > 1 then p.moins:Show() else p.moins:Hide() end
	p.liste.rafraichir(ns.mesInteresses())
end

-- ------------------------------------------------------------------ onglet Alliance (F20)

local function construireAlliance(p)
	p.entete = ui.texte(p, "GameFontNormal", 0, 0)
	p.filtres = {}
	local x = 0
	local filtres = { { "tous", "Tous" } }
	for _, g in ipairs(ns.GUILDES) do filtres[#filtres + 1] = { g.sigle, g.sigle } end
	filtres[#filtres + 1] = { "enligne", "En ligne" }
	for _, f in ipairs(filtres) do
		local b = ui.bouton(p, f[2], 84, function()
			etat.membres = f[1]
			ns.rafraichir()
		end)
		b:SetPoint("TOPLEFT", x, -20)
		p.filtres[f[1]] = b
		x = x + 88
	end
	local recherche = ui.saisie(p, 160, 520, -20)
	recherche:SetScript("OnTextChanged", function(e)
		etat.recherche = e:GetText() or ""
		ns.rafraichir()
	end)
	p.liste = ui.liste(p, -50, 16, 22, 680, function(r)
		r.nom = ui.texte(r, "GameFontHighlight", 4, -4)
		r.info = ui.texte(r, "GameFontHighlightSmall", 250, -5)
		r.ou = ui.texte(r, "GameFontHighlightSmall", 480, -5)
		r:SetScript("OnClick", function(self, clic)
			if not self.donnee then return end
			ns.auClic()
			if clic == "RightButton" then ui.inviter(self.donnee.nom) else ui.chuchoter(self.donnee.nom) end
		end)
	end, function(r, m)
		r.nom:SetText((m.enLigne and "" or GRIS) .. ui.couleurClasse(m.classe) .. ns.prenom(m.nom) .. "|r "
			.. ns.couleurGuilde(m.guilde) .. "(" .. ns.sigle(m.guilde) .. ")|r")
		r.info:SetText(("niv. %s%s%s"):format(m.niveau or "?", m.role and (" · " .. ns.ROLES[m.role]) or "",
			m.discord and (" · " .. m.discord) or ""))
		r.ou:SetText(m.enLigne and (VERT .. (m.zone or "en ligne") .. "|r") or (GRIS .. "vu " .. ui.age(m.vu) .. "|r"))
		r:SetAlpha(m.enLigne and 1 or 0.65)
	end)
	p.pied = ui.texte(p, "GameFontDisableSmall", 0, 0, "BOTTOMLEFT")
	p.pied:SetText("Seuls les joueurs qui ont l'addon figurent ici. Clic : chuchoter. Clic droit : inviter.")
end

local function rafraichirAlliance(p)
	local enLigne, total = ns.compteMembres()
	p.entete:SetText(("%d membre(s) de l'alliance connu(s), %s%d en ligne|r"):format(total, VERT, enLigne))
	for code, b in pairs(p.filtres) do b:SetAlpha(code == etat.membres and 1 or 0.5) end
	p.liste.rafraichir(ns.listeMembres({ filtre = etat.membres, recherche = etat.recherche }))
end

-- ------------------------------------------------------------------ onglet Réglages (F10, F12, F13)

local function construireReglages(p)
	ui.texte(p, "GameFontNormal", 0, 0):SetText("Mon pseudo Discord")
	p.discord = ui.saisie(p, 200, 160, 4)
	p.discord:SetScript("OnEditFocusLost", function(e) ns.changerDiscord(e:GetText()) end)
	p.discord:SetScript("OnEnterPressed", function(e) e:ClearFocus() end)
	ui.texte(p, "GameFontNormal", 380, 0):SetText("Mon rôle")
	p.roles = {}
	local x = 450
	for _, code in ipairs({ "T", "H", "D" }) do
		local b = ui.bouton(p, ns.ROLES[code], 66, function()
			ns.db.reglages.role = code
			ns.rafraichir()
			ns.direBonjour()
		end)
		b:SetPoint("TOPLEFT", x, 4)
		p.roles[code] = b
		x = x + 70
	end
	p.son = ui.case(p, "Son à chaque annonce", 0, -32, function(v) ns.db.reglages.son = v end)
	p.choixSon = ui.bouton(p, "", 160, function()
		local r = ns.db.reglages
		r.choixSon = r.choixSon % #ns.SONS + 1
		ns.jouerSon(true)
		ns.rafraichir()
	end)
	p.choixSon:SetPoint("TOPLEFT", 260, -32)
	p.npd = ui.case(p, "Ne pas déranger (les annonces arrivent dans la liste, sans son ni fenêtre)", 0, -60, function(v)
		ns.db.reglages.nePasDeranger = v
		ns.rafraichir()
	end)
	p.niveau = ui.case(p, "Pas d'alerte pour une annonce au-dessus de mon niveau", 0, -88, function(v)
		ns.db.reglages.filtreNiveau = v
	end)
	ui.texte(p, "GameFontNormal", 0, -126):SetText("Types d'annonces suivis")
	p.types = {}
	local y = -144
	for _, t in ipairs(ns.TYPES) do
		p.types[t.code] = ui.case(p, t.nom, 0, y, function(v) ns.db.reglages.types[t.code] = v end)
		y = y - 26
	end
	ui.bouton(p, "Replacer les alertes", 160, function() ns.replacerToasts() end):SetPoint("TOPLEFT", 0, y - 8)
	ui.texte(p, "GameFontNormal", 0, y - 44):SetText("Bloqués")
	p.bloques = ui.texte(p, "GameFontHighlightSmall", 0, y - 62)
	p.bloques:SetWidth(680)
	p.diag = ui.texte(p, "GameFontDisableSmall", 0, 0, "BOTTOMLEFT")
	p.diag:SetWidth(680)
end

local function rafraichirReglages(p)
	local r = ns.db.reglages
	if not p.discord:HasFocus() then p.discord:SetText(ns.db.discord or "") end
	for code, b in pairs(p.roles) do b:SetAlpha(code == r.role and 1 or 0.5) end
	p.son:SetChecked(r.son)
	p.choixSon:SetText("Son : " .. ns.SONS[r.choixSon].nom)
	p.npd:SetChecked(r.nePasDeranger)
	p.niveau:SetChecked(r.filtreNiveau)
	for code, c in pairs(p.types) do c:SetChecked(r.types[code] ~= false) end
	local b = {}
	for nom in pairs(ns.db.bloques) do b[#b + 1] = ns.prenom(nom) end
	table.sort(b)
	p.bloques:SetText(#b > 0 and (table.concat(b, ", ") .. GRIS .. "  (/alliance debloquer Nom)|r")
		or (GRIS .. "Personne. Pour bloquer un joueur : /alliance bloquer Nom.|r"))
	p.diag:SetText(ns.diagnostic():gsub("\n", " · "))
end

function ns.changerDiscord(texte)
	texte = tostring(texte or ""):gsub("[~|\r\n]", ""):gsub("^%s+", ""):gsub("%s+$", "")
	if #texte > 40 then texte = texte:sub(1, 40) end
	ns.db.discord = texte ~= "" and texte or nil
	ns.compte.discord = ns.db.discord or ns.compte.discord
end

-- ------------------------------------------------------------------ la fenêtre

local ONGLETS = {
	{ "annonces", "Annonces", construireAnnonces, rafraichirAnnonces },
	{ "miennes", "Mes annonces", construireMiennes, rafraichirMiennes },
	{ "alliance", "Alliance", construireAlliance, rafraichirAlliance },
	{ "reglages", "Réglages", construireReglages, rafraichirReglages },
}

local courant = "annonces"

function ns.rafraichir()
	if not fenetre or not fenetre:IsShown() then return end
	for _, o in ipairs(ONGLETS) do
		local p = pages[o[1]]
		if o[1] == courant then p:Show() o[4](p) else p:Hide() end
		onglets[o[1]]:SetAlpha(o[1] == courant and 1 or 0.6)
	end
	if ns.majMinicarte then ns.majMinicarte() end
end

local function construireFenetre()
	local f = CreateFrame("Frame", "AppelAllianceFenetre", UIParent, "BasicFrameTemplateWithInset")
	f:SetSize(LARGEUR, HAUTEUR)
	f:SetPoint("CENTER")
	f:SetFrameStrata("HIGH")
	f:SetMovable(true)
	f:EnableMouse(true)
	f:SetClampedToScreen(true)
	f:RegisterForDrag("LeftButton")
	f:SetScript("OnDragStart", f.StartMoving)
	f:SetScript("OnDragStop", f.StopMovingOrSizing)
	tinsert(UISpecialFrames, "AppelAllianceFenetre")
	ui.texte(f, "GameFontNormal", 0, -6, "TOP"):SetText("Appel d'Alliance — KE × Cassos")
	local x = 16
	for _, o in ipairs(ONGLETS) do
		local b = ui.bouton(f, o[2], 120, function()
			courant = o[1]
			ns.rafraichir()
		end)
		b:SetPoint("TOPLEFT", x, -30)
		onglets[o[1]] = b
		x = x + 124
		local p = CreateFrame("Frame", nil, f)
		p:SetPoint("TOPLEFT", 18, -62)
		p:SetPoint("BOTTOMRIGHT", -18, 16)
		p:Hide()
		o[3](p)
		pages[o[1]] = p
	end
	fenetre = f
end

function ns.ouvrir(onglet)
	if not ns.actif() then
		ns.ouvrirAccueil()
		return
	end
	if not fenetre then construireFenetre() end
	if onglet then courant = onglet end
	fenetre:Show()
	ns.rafraichir()
end

function ns.basculer()
	if fenetre and fenetre:IsShown() then fenetre:Hide() else ns.ouvrir() end
end

for _, s in ipairs({ "annonces", "membres", "interesse" }) do ns.surSignal(s, function() ns.rafraichir() end) end

-- ------------------------------------------------------------------ diagnostic

function ns.diagnostic()
	local s = ns.stats
	return table.concat({
		("Appel d'Alliance %s, protocole %d"):format(ns.VERSION, ns.PROTO),
		("Canal %s : %s, transport %s"):format(ns.nomCanal(), ns.dansLeCanal() and ("n° " .. ns.numeroCanal()) or "non rejoint",
			ns.transport()),
		("Messages : %d envoyés, %d en texte, %d refus de débit, %d échecs, %d reçus, %d ignorés, %d en attente, %d au clic")
			:format(s.envoyes, s.texte, s.refusDebit, s.echecs, s.recus, s.ignores, ns.enAttente(), ns.enAttenteDeClic()),
		"Dernière erreur : " .. (ns.derniereErreur or "aucune"),
	}, "\n")
end

-- ------------------------------------------------------------------ minicarte

local bouton
function ns.majMinicarte()
	if not ns.db then return end
	if ns.db.reglages.minicarteCachee or not Minimap then
		if bouton then bouton:Hide() end
		return
	end
	if not bouton then
		local b = CreateFrame("Button", "AppelAllianceMinicarte", Minimap)
		b:SetSize(31, 31)
		b:SetFrameStrata("MEDIUM")
		b:SetFrameLevel(8)
		b:RegisterForClicks("LeftButtonUp", "RightButtonUp")
		b:RegisterForDrag("LeftButton")
		local icone = b:CreateTexture(nil, "BACKGROUND")
		icone:SetSize(20, 20)
		icone:SetPoint("CENTER")
		icone:SetTexture("Interface\\Icons\\INV_BannerPVP_02")
		local bord = b:CreateTexture(nil, "OVERLAY")
		bord:SetSize(53, 53)
		bord:SetPoint("TOPLEFT")
		bord:SetTexture("Interface\\Minimap\\MiniMap-TrackingBorder")
		b.pastille = b:CreateFontString(nil, "OVERLAY", "GameFontNormalSmall")
		b.pastille:SetPoint("BOTTOMRIGHT", 2, -2)
		local function placer()
			local angle = math.rad(ns.db.reglages.angleMinicarte or 200)
			b:ClearAllPoints()
			b:SetPoint("CENTER", Minimap, "CENTER", 80 * math.cos(angle), 80 * math.sin(angle))
		end
		b:SetScript("OnDragStart", function(self)
			self:SetScript("OnUpdate", function()
				local mx, my = Minimap:GetCenter()
				local cx, cy = GetCursorPosition()
				local echelle = Minimap:GetEffectiveScale()
				ns.db.reglages.angleMinicarte = math.deg(math.atan2(cy / echelle - my, cx / echelle - mx))
				placer()
			end)
		end)
		b:SetScript("OnDragStop", function(self) self:SetScript("OnUpdate", nil) end)
		b:SetScript("OnClick", function(_, clic)
			ns.auClic()
			if clic == "RightButton" then ns.ouvrirFormulaire() else ns.basculer() end
		end)
		b:SetScript("OnEnter", function(self)
			GameTooltip:SetOwner(self, "ANCHOR_LEFT")
			GameTooltip:AddLine("Appel d'Alliance")
			GameTooltip:AddLine(ns.nombreAnnonces() .. " annonce(s) active(s)", 1, 1, 1)
			GameTooltip:AddLine("Clic : la liste. Clic droit : nouvelle annonce.", 0.25, 0.63, 1)
			GameTooltip:Show()
		end)
		b:SetScript("OnLeave", function() GameTooltip:Hide() end)
		placer()
		bouton = b
	end
	local n = ns.actif() and ns.nombreAnnonces() or 0
	bouton.pastille:SetText(n > 0 and tostring(n) or "")
	bouton:Show()
end

ns.surSignal("charge", ns.majMinicarte)
ns.surSignal("annonces", function() ns.majMinicarte() end)

-- ------------------------------------------------------------------ /alliance

SLASH_APPELALLIANCE1 = "/alliance"
SLASH_APPELALLIANCE2 = "/aa"
SlashCmdList.APPELALLIANCE = function(arg)
	local cmd, reste = (arg or ""):match("^%s*(%S*)%s*(.-)%s*$")
	cmd = (cmd or ""):lower()
	if cmd == "" then ns.basculer()
	elseif cmd == "new" or cmd == "nouvelle" then ns.ouvrirFormulaire()
	elseif cmd == "dnd" or cmd == "npd" then
		ns.db.reglages.nePasDeranger = not ns.db.reglages.nePasDeranger
		ns.dire("Ne pas déranger : " .. (ns.db.reglages.nePasDeranger and "oui" or "non") .. ".")
		ns.rafraichir()
	elseif cmd == "alliance" or cmd == "membres" then ns.ouvrir("alliance")
	elseif cmd == "reglages" then ns.ouvrir("reglages")
	elseif cmd == "canal" and reste ~= "" then
		ns.db.motDePasse = reste
		ns.rejoindreCanal()
	elseif cmd == "bloquer" and reste ~= "" then
		ns.bloquer(ns.nomComplet(reste), true)
		ns.dire(reste .. " est bloqué.")
	elseif cmd == "debloquer" and reste ~= "" then
		ns.bloquer(ns.nomComplet(reste), false)
		ns.dire(reste .. " n'est plus bloqué.")
	elseif cmd == "minicarte" then
		ns.db.reglages.minicarteCachee = not ns.db.reglages.minicarteCachee
		ns.majMinicarte()
	elseif cmd == "diag" then
		for l in ns.diagnostic():gmatch("[^\n]+") do ns.dire(l) end
	elseif cmd == "accueil" then ns.ouvrirAccueil(true)
	else
		ns.dire("/alliance : la liste ; /alliance new : nouvelle annonce ; /alliance dnd : ne pas déranger ; "
			.. "/alliance membres ; /alliance reglages ; /alliance bloquer|debloquer Nom ; /alliance canal <mot de passe> ; "
			.. "/alliance minicarte ; /alliance diag ; /alliance accueil")
	end
end
