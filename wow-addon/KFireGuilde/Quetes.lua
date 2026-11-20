-- KFIRE Guilde : les quêtes en commun avec les membres en ligne (lot 4).
--
-- Chaque membre publie la liste de ses quêtes en cours (leurs numéros) à la
-- connexion et quand son journal change. On croise avec mon journal : les quêtes
-- que d'autres membres connectés ont aussi, avec « Inviter » en un clic.
-- Seuls les membres en ligne comptent : une liste de plus de 12 heures est ignorée.
local _, ns = ...

local VIE_FICHE = 86400
local FRAICHEUR = 12 * 3600
local DELAI_PUBLICATION = 5     -- le journal envoie beaucoup de mises à jour d'affilée

ns.magasin("quetes", {
	champs = { "liste" },
	dureeVie = VIE_FICHE,
})

-- ------------------------------------------------------------------ mon journal

-- { [questID] = { titre, niveau, terminee } }
function ns.mesQuetes()
	local t = {}
	if C_QuestLog and C_QuestLog.GetNumQuestLogEntries and C_QuestLog.GetInfo then
		for i = 1, C_QuestLog.GetNumQuestLogEntries() or 0 do
			local q = C_QuestLog.GetInfo(i)
			if q and not q.isHeader and q.questID and q.questID > 0 and not q.isHidden then
				t[q.questID] = { titre = q.title, niveau = q.level,
					terminee = C_QuestLog.IsComplete and C_QuestLog.IsComplete(q.questID) or false }
			end
		end
	elseif GetNumQuestLogEntries and GetQuestLogTitle then
		for i = 1, (GetNumQuestLogEntries()) or 0 do
			local titre, niveau, _, entete, _, terminee, _, id = GetQuestLogTitle(i)
			if titre and not entete and id and id > 0 then
				t[id] = { titre = titre, niveau = niveau, terminee = terminee == 1 or terminee == true }
			end
		end
	end
	return t
end

local function texteListe(quetes)
	local ids = {}
	for id in pairs(quetes) do ids[#ids + 1] = id end
	table.sort(ids)
	return table.concat(ids, ",")
end

function ns.publierQuetes()
	if not ns.db then return end
	local liste = texteListe(ns.mesQuetes())
	local f = ns.fiches("quetes")[ns.moi()]
	-- même liste et publiée il y a moins d'une heure : rien à dire
	if f and (f.liste or "") == liste and ns.maintenant() - (f._maj or 0) < 3600 then return end
	ns.publier("quetes", ns.moi(), { liste = liste })
end

-- ------------------------------------------------------------------ en commun

-- Par membre : { { nom, affichage, niveau, zone, quetes = { { id, titre } } } }, le plus de quêtes d'abord.
function ns.quetesEnCommun()
	local miennes = ns.mesQuetes()
	local res = {}
	local now = ns.maintenant()
	for nom, f in pairs(ns.fiches("quetes")) do
		local m = ns.roster[nom]
		if nom ~= ns.moi() and m and m.enLigne and now - (f._maj or 0) < FRAICHEUR then
			local communes = {}
			for id in tostring(f.liste or ""):gmatch("%d+") do
				local q = miennes[tonumber(id)]
				if q then communes[#communes + 1] = { id = tonumber(id), titre = q.titre, terminee = q.terminee } end
			end
			if #communes > 0 then
				table.sort(communes, function(a, b) return (a.titre or "") < (b.titre or "") end)
				res[#res + 1] = { nom = nom, affichage = ns.affichage(nom), niveau = m.niveau, zone = m.zone,
					classeFichier = m.classeFichier, quetes = communes }
			end
		end
	end
	table.sort(res, function(a, b)
		if #a.quetes ~= #b.quetes then return #a.quetes > #b.quetes end
		return a.nom < b.nom
	end)
	return res
end

-- Par quête : { { id, titre, membres = { nom… } } }
function ns.quetesPartagees()
	local parId, res = {}, {}
	for _, m in ipairs(ns.quetesEnCommun()) do
		for _, q in ipairs(m.quetes) do
			local e = parId[q.id]
			if not e then
				e = { id = q.id, titre = q.titre, membres = {} }
				parId[q.id] = e
				res[#res + 1] = e
			end
			e.membres[#e.membres + 1] = m.nom
		end
	end
	table.sort(res, function(a, b)
		if #a.membres ~= #b.membres then return #a.membres > #b.membres end
		return (a.titre or "") < (b.titre or "")
	end)
	return res
end

-- ------------------------------------------------------------------ bulles

-- Un membre en ligne fait une de mes quêtes : une bulle par membre et par quête, par session.
local dejaDit = {}
local function verifierCommun()
	if not ns.db or ns.db.reglages.bullesQuetesCoupees then return end
	for _, m in ipairs(ns.quetesEnCommun()) do
		local nouvelles = {}
		for _, q in ipairs(m.quetes) do
			local cle = m.nom .. "/" .. q.id
			if not dejaDit[cle] and not q.terminee then
				dejaDit[cle] = true
				nouvelles[#nouvelles + 1] = q.titre or ("quête " .. q.id)
			end
		end
		if #nouvelles > 0 then
			ns.bulle(("%s fait aussi %s"):format(m.affichage,
				#nouvelles == 1 and ("« " .. nouvelles[1] .. " »") or (#nouvelles .. " de tes quêtes")), "quetes")
		end
	end
end

ns.surSignal("quetes", function(nom)
	if nom == ns.moi() then return end
	ns.regrouper("quetes:commun", 2, verifierCommun)
end)

ns.ecouter("QUEST_LOG_UPDATE", function()
	if not ns.db then return end
	ns.regrouper("quetes:publier", DELAI_PUBLICATION, ns.publierQuetes)
end)

ns.surSignal("connexion", function()
	ns.apres(12, function()
		ns.publierQuetes()
		verifierCommun()
	end)
end)
