-- KFIRE Guilde : qui sait fabriquer quoi (lot 2).
--
-- Chaque personnage publie une fiche par métier : niveau, maximum et recettes
-- connues. Les niveaux se lisent à la connexion (fenêtre des compétences) ; les
-- recettes, quand le joueur ouvre la fenêtre de son métier.
--
-- Une recette est notée par l'objet qu'elle fabrique (son id), ou « s » + l'id du
-- sort quand elle ne fabrique pas d'objet (enchantements). Les recettes ne font
-- que s'ajouter : un filtre de la fenêtre de métier ne fait rien oublier.
local _, ns = ...

local VIE_FICHE = 60 * 86400
local DELAI_PUBLICATION = 3    -- la fenêtre de métier envoie plusieurs mises à jour d'affilée

ns.magasin("metiers", {
	champs = { "metier", "niveau", "max", "recettes" },
	dureeVie = VIE_FICHE,
})

-- Les entêtes de la fenêtre des compétences qui regroupent les métiers.
local ENTETES_METIERS = { "métier", "profession", "secondaire", "secondary" }

local function estEnteteMetier(nom)
	nom = (nom or ""):lower()
	for _, e in ipairs(ENTETES_METIERS) do
		if nom:find(e, 1, true) then return true end
	end
	return false
end

local function secret(v)
	return issecretvalue ~= nil and issecretvalue(v) == true
end

-- ------------------------------------------------------------------ ce que je sais

-- [métier] = { niveau, max, recettes = { [id] = true } }, pour ce personnage
local mesMetiers = {}

local function cleDe(perso, metier)
	return perso .. "/" .. metier
end

local function recettesDe(texte)
	local t = {}
	for id in tostring(texte or ""):gmatch("[^,]+") do t[id] = true end
	return t
end

local function texteRecettes(t)
	local l = {}
	for id in pairs(t) do l[#l + 1] = id end
	table.sort(l)
	return table.concat(l, ",")
end

-- Repart de ce que j'avais publié (le disque garde mes fiches comme les autres).
local function reprendre(metier)
	if mesMetiers[metier] then return mesMetiers[metier] end
	local f = ns.fiches("metiers")[cleDe(ns.moi(), metier)]
	mesMetiers[metier] = {
		niveau = f and tonumber(f.niveau) or 0, max = f and tonumber(f.max) or 0,
		recettes = recettesDe(f and f.recettes),
	}
	return mesMetiers[metier]
end

local function publierMetier(metier)
	local m = mesMetiers[metier]
	if not m then return end
	local cle = cleDe(ns.moi(), metier)
	local avant = ns.fiches("metiers")[cle]
	local recettes = texteRecettes(m.recettes)
	if avant and tonumber(avant.niveau) == m.niveau and tonumber(avant.max) == m.max
		and (avant.recettes or "") == recettes then
		return
	end
	ns.publier("metiers", cle, { metier = metier, niveau = m.niveau, max = m.max, recettes = recettes })
end

local function noter(metier, niveau, max, recettes)
	if not metier or metier == "" or secret(metier) then return end
	local m = reprendre(metier)
	niveau, max = tonumber(niveau) or m.niveau, tonumber(max) or m.max
	-- un niveau qui baisse : le métier a été oublié puis repris, les recettes repartent de zéro
	if niveau < m.niveau then m.recettes = {} end
	m.niveau, m.max = niveau, max
	for id in pairs(recettes or {}) do m.recettes[id] = true end
	ns.regrouper("metier:" .. metier, DELAI_PUBLICATION, function() publierMetier(metier) end)
end
ns.noterMetier = noter

-- ------------------------------------------------------------------ lecture dans le jeu

-- Niveaux de tous les métiers, sans ouvrir de fenêtre.
function ns.lireNiveaux()
	if GetNumSkillLines and GetSkillLineInfo then
		local dansMetiers = false
		for i = 1, GetNumSkillLines() or 0 do
			local nom, entete, _, rang, _, _, max = GetSkillLineInfo(i)
			if entete then
				dansMetiers = estEnteteMetier(nom)
			elseif dansMetiers and nom then
				noter(nom, rang, max)
			end
		end
	elseif GetProfessions and GetProfessionInfo then
		for _, idx in pairs({ GetProfessions() }) do
			if idx then
				local nom, _, rang, max = GetProfessionInfo(idx)
				noter(nom, rang, max)
			end
		end
	end
end

local function idDepuisLiens(lienObjet, lienRecette)
	local id = lienObjet and lienObjet:match("item:(%d+)")
	if id then
		local nom = lienObjet:match("|h%[(.-)%]|h")
		if nom then ns.db.noms[id] = nom end
		return id
	end
	local sort = (lienRecette or lienObjet or ""):match("[a-z]+:(%d+)")
	if sort then
		local nom = (lienRecette or lienObjet):match("|h%[(.-)%]|h")
		if nom then ns.db.noms["s" .. sort] = nom end
		return "s" .. sort
	end
end

-- La fenêtre de métier (API classique : GetTradeSkill…).
function ns.lireFenetreMetier()
	if GetNumTradeSkills and GetTradeSkillInfo and GetTradeSkillLine then
		local metier, rang, max = GetTradeSkillLine()
		if not metier or metier == "UNKNOWN" then return end
		local recettes = {}
		for i = 1, GetNumTradeSkills() or 0 do
			local nom, genre = GetTradeSkillInfo(i)
			if nom and genre ~= "header" and genre ~= "subheader" then
				local id = idDepuisLiens(GetTradeSkillItemLink and GetTradeSkillItemLink(i),
					GetTradeSkillRecipeLink and GetTradeSkillRecipeLink(i))
				if id then recettes[id] = true end
			end
		end
		noter(metier, rang, max, recettes)
	elseif C_TradeSkillUI and C_TradeSkillUI.GetAllRecipeIDs then
		local info = C_TradeSkillUI.GetBaseProfessionInfo and C_TradeSkillUI.GetBaseProfessionInfo()
		if not info or not info.professionName then return end
		local recettes = {}
		for _, rid in ipairs(C_TradeSkillUI.GetAllRecipeIDs() or {}) do
			local r = C_TradeSkillUI.GetRecipeInfo(rid)
			if r and r.learned then
				local lien = C_TradeSkillUI.GetRecipeItemLink and C_TradeSkillUI.GetRecipeItemLink(rid)
				local id = idDepuisLiens(lien, nil) or ("s" .. rid)
				if r.name and id:sub(1, 1) == "s" then ns.db.noms[id] = r.name end
				recettes[id] = true
			end
		end
		noter(info.professionName, info.skillLevel, info.maxSkillLevel, recettes)
	end
end

-- La fenêtre d'artisanat des enchanteurs (API classique : GetCraft…).
function ns.lireFenetreArtisanat()
	if not (GetNumCrafts and GetCraftInfo) then return end
	local metier, rang, max
	if GetCraftDisplaySkillLine then metier, rang, max = GetCraftDisplaySkillLine() end
	if not metier and GetCraftName then metier = GetCraftName() end
	if not metier then return end
	local recettes = {}
	for i = 1, GetNumCrafts() or 0 do
		local nom, _, genre = GetCraftInfo(i)
		if nom and genre ~= "header" then
			local id = idDepuisLiens(GetCraftItemLink and GetCraftItemLink(i), GetCraftRecipeLink and GetCraftRecipeLink(i))
			if id then recettes[id] = true end
		end
	end
	noter(metier, rang, max, recettes)
end

local function protege(fn)
	return function()
		if not ns.db then return end
		ns.regrouper(fn, 0.5, fn)
	end
end
ns.ecouter("TRADE_SKILL_SHOW", protege(ns.lireFenetreMetier))
ns.ecouter("TRADE_SKILL_UPDATE", protege(ns.lireFenetreMetier))
ns.ecouter("TRADE_SKILL_LIST_UPDATE", protege(ns.lireFenetreMetier))
ns.ecouter("CRAFT_SHOW", protege(ns.lireFenetreArtisanat))
ns.ecouter("CRAFT_UPDATE", protege(ns.lireFenetreArtisanat))
ns.ecouter("SKILL_LINES_CHANGED", protege(ns.lireNiveaux))

ns.surSignal("connexion", function()
	ns.apres(6, ns.lireNiveaux)
end)

-- ------------------------------------------------------------------ recherche

-- Les personnages qui savent fabriquer cet objet (ou lancer ce sort), en ligne d'abord.
function ns.fabricants(id)
	id = tostring(id)
	local t, vus = {}, {}
	for cle, f in pairs(ns.fiches("metiers")) do
		local perso = cle:match("^(.-)/")
		if perso and not vus[perso] and f.recettes and ("," .. f.recettes .. ","):find("," .. id .. ",", 1, true) then
			vus[perso] = true
			t[#t + 1] = perso
		end
	end
	table.sort(t, function(a, b)
		local ea, eb = ns.roster[a] and ns.roster[a].enLigne or false, ns.roster[b] and ns.roster[b].enLigne or false
		if ea ~= eb then return ea end
		return a < b
	end)
	return t
end

-- « qui sait fabriquer X » : { { id, nom, fabricants = { … } } }, par nom.
function ns.chercherRecettes(texte)
	texte = (texte or ""):lower():gsub("^%s+", ""):gsub("%s+$", "")
	if #texte < 2 then return {} end
	local parId = {}
	for _, f in pairs(ns.fiches("metiers")) do
		for id in tostring(f.recettes or ""):gmatch("[^,]+") do
			if not parId[id] then
				local nom = ns.nomObjet(id)
				if nom and nom:lower():find(texte, 1, true) then parId[id] = nom end
			end
		end
	end
	local res = {}
	for id, nom in pairs(parId) do res[#res + 1] = { id = id, nom = nom, fabricants = ns.fabricants(id) } end
	table.sort(res, function(a, b) return a.nom < b.nom end)
	return res
end

-- Les métiers de la guilde : { { perso, metier, niveau, max, nbRecettes, enLigne } }.
function ns.listeMetiers(metierChoisi)
	local res = {}
	for cle, f in pairs(ns.fiches("metiers")) do
		local perso = cle:match("^(.-)/")
		if perso and (not metierChoisi or f.metier == metierChoisi) then
			local n = 0
			for _ in tostring(f.recettes or ""):gmatch("[^,]+") do n = n + 1 end
			res[#res + 1] = {
				perso = perso, affichage = ns.affichage(perso), metier = f.metier or cle:match("/(.*)$"),
				niveau = tonumber(f.niveau) or 0, max = tonumber(f.max) or 0, nbRecettes = n,
				enLigne = ns.roster[perso] and ns.roster[perso].enLigne or false,
			}
		end
	end
	table.sort(res, function(a, b)
		if a.metier ~= b.metier then return a.metier < b.metier end
		if a.niveau ~= b.niveau then return a.niveau > b.niveau end
		return a.perso < b.perso
	end)
	return res
end

-- Les noms de métiers connus dans la guilde, pour le filtre de la page.
function ns.nomsMetiers()
	local t, vus = {}, {}
	for _, f in pairs(ns.fiches("metiers")) do
		if f.metier and not vus[f.metier] then
			vus[f.metier] = true
			t[#t + 1] = f.metier
		end
	end
	table.sort(t)
	return t
end
