-- KFIRE Guilde : la présence de la guilde, pour choisir le créneau des sorties (lot 3).
--
-- Toutes les 10 minutes, chaque membre qui a l'addon compte les membres en ligne
-- (avec ou sans l'addon) et garde, pour chaque heure, le plus grand nombre vu.
-- Une fiche par jour : « presence », clé « AAAAMMJJ », champ « h » = 24 nombres
-- séparés par des virgules (« - » quand personne n'a relevé cette heure-là).
--
-- Deux relevés d'un même jour se fusionnent (le plus grand nombre de chaque heure).
-- Pour ne pas parler à plusieurs, seul le premier membre en ligne avec l'addon
-- (par ordre alphabétique) publie le jour en cours, toutes les 30 minutes.
local _, ns = ...

local RELEVE = 600              -- secondes entre deux relevés
local PUBLICATION = 1800        -- secondes entre deux publications par l'élu
local GARDE = 8 * 7 * 86400     -- 8 semaines

local function jourDe(t)
	return date("%Y%m%d", t)
end

local function heuresDe(texte)
	local h = {}
	local i = 0
	for v in tostring(texte or ""):gmatch("[^,]+") do
		i = i + 1
		h[i] = tonumber(v)
	end
	return h
end

local function texteHeures(h)
	local t = {}
	for i = 1, 24 do t[i] = h[i] and tostring(h[i]) or "-" end
	return table.concat(t, ",")
end

local function horodatageJour(cle)
	local a, m, j = tostring(cle):match("^(%d%d%d%d)(%d%d)(%d%d)$")
	if not a then return nil end
	return time({ year = tonumber(a), month = tonumber(m), day = tonumber(j), hour = 12 })
end

ns.magasin("presence", {
	champs = { "h" },
	-- l'âge se compte depuis le jour relevé, pas depuis la dernière fusion
	dureeVie = function(f, cle)
		local t = horodatageJour(cle)
		return t and (GARDE + t - (f._maj or 0)) or GARDE
	end,
	fusionner = function(a, b)
		local ha, hb = heuresDe(a.h), heuresDe(b.h)
		local h = {}
		for i = 1, 24 do
			local x, y = ha[i], hb[i]
			h[i] = (x and y) and math.max(x, y) or x or y
		end
		return { h = texteHeures(h) }
	end,
})

-- ------------------------------------------------------------------ relevé

function ns.compterEnLigne()
	local n = 0
	for _, m in pairs(ns.roster) do if m.enLigne then n = n + 1 end end
	return n
end

-- Suis-je celui qui publie ? Le premier membre en ligne avec l'addon, par ordre alphabétique.
function ns.estPublieurPresence()
	local membres = ns.fiches("membres")
	local premier
	for nom, m in pairs(ns.roster) do
		if m.enLigne and (membres[nom] or nom == ns.moi()) and (not premier or nom < premier) then premier = nom end
	end
	return premier == nil or premier == ns.moi()
end

local dernierePublication = 0

function ns.releverPresence()
	if not ns.rosterDate or ns.enInstance then return end
	local t = ns.maintenant()
	local cle = jourDe(t)
	local heure = tonumber(date("%H", t)) + 1
	local f = ns.fiches("presence")[cle]
	local h = heuresDe(f and f.h)
	local n = ns.compterEnLigne()
	if h[heure] and h[heure] >= n and t - dernierePublication < PUBLICATION then return end
	h[heure] = math.max(h[heure] or 0, n)
	if ns.estPublieurPresence() and t - dernierePublication >= PUBLICATION then
		dernierePublication = t
		ns.publier("presence", cle, { h = texteHeures(h) })
	else
		-- gardé pour moi seulement : la fiche locale garde sa date, l'élu publiera
		ns.ranger("presence", cle, f and f._maj or 1, f and f._auteur or ns.moi(), { texteHeures(h) })
	end
end

ns.surSignal("connexion", function()
	local function boucle()
		ns.releverPresence()
		ns.apres(RELEVE, boucle)
	end
	ns.apres(30, boucle)
end)

-- ------------------------------------------------------------------ semaine type

ns.JOURS = { "Lundi", "Mardi", "Mercredi", "Jeudi", "Vendredi", "Samedi", "Dimanche" }

-- grille[jour 1..7, lundi d'abord][heure 1..24] = moyenne des relevés ; aussi le nombre de jours relevés.
function ns.semaineType()
	local somme, nombre = {}, {}
	for j = 1, 7 do somme[j], nombre[j] = {}, {} end
	local jours = 0
	for cle, f in pairs(ns.fiches("presence")) do
		local t = horodatageJour(cle)
		if t then
			jours = jours + 1
			local wday = tonumber(date("%w", t))    -- 0 = dimanche
			local j = wday == 0 and 7 or wday
			for i, v in pairs(heuresDe(f.h)) do
				somme[j][i] = (somme[j][i] or 0) + v
				nombre[j][i] = (nombre[j][i] or 0) + 1
			end
		end
	end
	local grille, max = {}, 0
	for j = 1, 7 do
		grille[j] = {}
		for i = 1, 24 do
			if nombre[j][i] then
				grille[j][i] = somme[j][i] / nombre[j][i]
				if grille[j][i] > max then max = grille[j][i] end
			end
		end
	end
	return grille, max, jours
end

-- Les 5 meilleurs créneaux d'une heure : { { jour, heure (0-23), moyenne } }.
function ns.meilleursCreneaux(n)
	local grille = ns.semaineType()
	local t = {}
	for j = 1, 7 do
		for i = 1, 24 do
			if grille[j][i] then t[#t + 1] = { jour = j, heure = i - 1, moyenne = grille[j][i] } end
		end
	end
	table.sort(t, function(a, b)
		if a.moyenne ~= b.moyenne then return a.moyenne > b.moyenne end
		if a.jour ~= b.jour then return a.jour < b.jour end
		return a.heure < b.heure
	end)
	local res = {}
	for i = 1, math.min(n or 5, #t) do res[i] = t[i] end
	return res
end
