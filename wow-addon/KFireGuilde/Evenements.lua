-- KFIRE Guilde : les événements de guilde du calendrier du jeu (lot 3).
--
-- Rien ne passe par le réseau de l'addon : le calendrier du jeu est déjà partagé
-- par le serveur. L'addon le lit (C_Calendar), le montre sans ouvrir le calendrier
-- et permet de répondre en un clic. Le chargement est asynchrone : on demande les
-- données, puis on relit sur les évènements CALENDAR_*.
--
-- Sans C_Calendar sur ce client, la page le dit et ne propose que d'ouvrir le
-- calendrier du jeu.
local _, ns = ...

local HORIZON = 30          -- jours affichés
local DEJA_COMMENCE = 3600  -- un événement commencé depuis moins d'une heure reste dans la liste
local ATTENTE_DEMANDE = 30  -- secondes entre deux demandes au serveur

local function resultat(ok, ...)
	if ok then return ... end
	return nil
end

-- C_Calendar.nom(...) si la fonction existe, sans jamais planter
local function cal(nom, ...)
	local f = type(C_Calendar) == "table" and C_Calendar[nom]
	if type(f) ~= "function" then return nil end
	return resultat(pcall(f, ...))
end

local function existe(nom)
	return type(C_Calendar) == "table" and type(C_Calendar[nom]) == "function"
end

function ns.calendrierDisponible()
	return existe("OpenCalendar") and (existe("GetGuildEventInfo") or existe("GetDayEvent"))
end

-- États d'inscription (Enum.CalendarStatus), avec les valeurs habituelles en repli.
local S = (type(Enum) == "table" and Enum.CalendarStatus) or {}
local ST = {
	invite = S.Invited or 0, oui = S.Available or 1, non = S.Declined or 2, confirme = S.Confirmed or 3,
	absent = S.Out or 4, reserve = S.Standby or 5, inscrit = S.Signedup or 6, pasInscrit = S.NotSignedup or 7,
	peutEtre = S.Tentative or 8,
}
ns.ETATS_INSCRIPTION = {
	[ST.invite] = { "Invité", "attente" }, [ST.oui] = { "Accepté", "oui" }, [ST.non] = { "Refusé", "non" },
	[ST.confirme] = { "Confirmé", "oui" }, [ST.absent] = { "Absent", "non" }, [ST.reserve] = { "En réserve", "peut" },
	[ST.inscrit] = { "Inscrit", "oui" }, [ST.pasInscrit] = { "Pas inscrit" }, [ST.peutEtre] = { "Peut-être", "peut" },
}

local T = (type(Enum) == "table" and Enum.CalendarEventType) or {}
ns.TYPES_EVENEMENT = {
	[T.Raid or 0] = "Raid", [T.Dungeon or 1] = "Donjon", [T.PvP or 2] = "JcJ",
	[T.Meeting or 3] = "Réunion", [T.Other or 4] = "Autre",
}
local GARDES = { GUILD_EVENT = true, GUILD_ANNOUNCEMENT = true, PLAYER = true, COMMUNITY_EVENT = true }

-- ------------------------------------------------------------------ dates

local function horodatage(t)
	if type(t) ~= "table" or not (t.year and t.month and t.monthDay) then return nil end
	local ok, v = pcall(time, { year = t.year, month = t.month, day = t.monthDay, hour = t.hour or 0, min = t.minute or 0 })
	return ok and v or nil
end

-- L'heure du serveur, comme le calendrier.
local function maintenant()
	if C_DateAndTime and C_DateAndTime.GetCurrentCalendarTime then
		local ok, t = pcall(C_DateAndTime.GetCurrentCalendarTime)
		local v = ok and horodatage(t)
		if v then return v end
	end
	return ns.maintenant()
end

-- ------------------------------------------------------------------ lecture

local function ajouter(res, index, e)
	if not e or not e.quand or not e.titre then return end
	local ancien = index[e.cle] or index[e.titre .. "@" .. e.quand]
	if ancien then
		for k, v in pairs(e) do if ancien[k] == nil then ancien[k] = v end end
		return
	end
	res[#res + 1] = e
	index[e.cle], index[e.titre .. "@" .. e.quand] = e, e
end

local function cleDe(id, titre, quand)
	if id and id ~= 0 then return "id" .. tostring(id) end
	return titre .. "@" .. quand
end

-- Les événements des 30 prochains jours : { cle, titre, quand, typeEv, typeCal, etat, coords }.
function ns.listeEvenements()
	local res, index = {}, {}
	if not ns.calendrierDisponible() then return res end
	local now = maintenant()
	local depuis, jusqua = now - DEJA_COMMENCE, now + (HORIZON + 1) * 86400
	local function garder(e) return e and e.quand >= depuis and e.quand < jusqua end

	-- 1. jour par jour, les mois comptés depuis celui qu'affiche le calendrier
	local info = cal("GetMonthInfo", 0)
	if existe("GetNumDayEvents") and type(info) == "table" and info.year and info.month then
		local midi = now - (now % 86400) + 12 * 3600
		for j = 0, HORIZON do
			local d = date("*t", midi + j * 86400)
			local decalage = (d.year - info.year) * 12 + (d.month - info.month)
			for i = 1, tonumber(cal("GetNumDayEvents", decalage, d.day)) or 0 do
				local e = cal("GetDayEvent", decalage, d.day, i)
				if type(e) == "table" and (not e.calendarType or GARDES[e.calendarType]) then
					local t = type(e.startTime) == "table" and e.startTime or e
					local quand = horodatage(t)
					if quand and e.title then
						local f = {
							cle = cleDe(e.eventID, e.title, quand), titre = e.title, quand = quand,
							typeEv = e.eventType, typeCal = e.calendarType, etat = e.inviteStatus,
							mod = e.modStatus, inviteType = e.inviteType, verrouille = e.isLocked == true,
							coords = { decalage, d.day, i },
						}
						if garder(f) then ajouter(res, index, f) end
					end
				end
			end
		end
	end
	-- 2. la liste des événements de guilde (même ceux des mois pas encore chargés)
	for i = 1, tonumber(cal("GetNumGuildEvents")) or 0 do
		local g = cal("GetGuildEventInfo", i)
		local quand = horodatage(g)
		if type(g) == "table" and quand and g.title then
			local a, b, c = cal("GetGuildEventSelectionInfo", i)
			if type(a) == "table" then a, b, c = a.offsetMonths, a.monthDay, a.eventIndex end
			local f = {
				cle = cleDe(g.eventID, g.title, quand), titre = g.title, quand = quand,
				typeEv = g.eventType, typeCal = g.calendarType or "GUILD_EVENT", etat = g.inviteStatus,
				coords = (a and b and c) and { a, b, c } or nil,
			}
			if garder(f) then ajouter(res, index, f) end
		end
	end
	table.sort(res, function(x, y)
		if x.quand ~= y.quand then return x.quand < y.quand end
		return x.titre < y.titre
	end)
	return res
end

local derniereDemande = -1000
function ns.demanderCalendrier(force)
	if not ns.calendrierDisponible() then return end
	local t = GetTime and GetTime() or ns.maintenant()
	if not force and t - derniereDemande < ATTENTE_DEMANDE then return end
	derniereDemande = t
	-- le mois en cours, sauf si le joueur regarde un autre mois dans le calendrier du jeu
	if not (CalendarFrame and CalendarFrame:IsShown()) then
		local d = date("*t", maintenant())
		cal("SetAbsMonth", d.month, d.year)
	end
	cal("OpenCalendar")
end

-- ------------------------------------------------------------------ détail et réponse

ns.detailEvenement = nil   -- { cle, description, createur, invites = { { nom, classeFichier, etat } }, compte }
local ouvert, attendu, actionEnAttente

local function lireOuvert()
	local info = cal("GetEventInfo")
	if type(info) ~= "table" or not info.title then return end
	local quand = horodatage(info.time)
	local cle = attendu
	if not cle then
		for _, e in ipairs(ns.listeEvenements()) do
			if e.titre == info.title and (not quand or e.quand == quand) then cle = e.cle break end
		end
	end
	if not cle then return end
	attendu, ouvert = nil, cle
	local d = {
		cle = cle, description = info.description, createur = info.creator, verrouille = info.isLocked == true,
		inviteType = info.inviteType, typeCal = info.calendarType, invites = {},
		compte = { oui = 0, peut = 0, attente = 0, non = 0 },
	}
	for i = 1, tonumber(cal("GetNumInvites")) or 0 do
		local v = cal("EventGetInvite", i)
		if type(v) == "table" and v.name then
			local etat = ns.ETATS_INSCRIPTION[v.inviteStatus]
			local groupe = etat and etat[2]
			if groupe then d.compte[groupe] = d.compte[groupe] + 1 end
			d.invites[#d.invites + 1] = {
				nom = ns.nomComplet(v.name), classeFichier = v.classFilename, niveau = v.level,
				etat = v.inviteStatus, mod = v.modStatus,
			}
			if ns.nomComplet(v.name) == ns.moi() then d.monEtat = v.inviteStatus end
		end
	end
	ns.detailEvenement = d
	ns.signaler("evenement", cle)
	if actionEnAttente and actionEnAttente.cle == cle then
		local a = actionEnAttente
		actionEnAttente = nil
		a.fn()
	end
end

local function trouver(cle)
	for _, e in ipairs(ns.listeEvenements()) do if e.cle == cle then return e end end
end

-- Demande le détail d'un événement au calendrier (réponse : signal « evenement »).
function ns.ouvrirEvenement(cle)
	local e = trouver(cle)
	if not e or not e.coords then return false end
	attendu = cle
	cal("OpenEvent", e.coords[1], e.coords[2], e.coords[3])
	return true
end

-- Pourquoi je ne peux pas répondre (nil : je peux).
function ns.empechementReponse(cle)
	local e = trouver(cle)
	if not e then return "événement introuvable" end
	local d = ns.detailEvenement and ns.detailEvenement.cle == cle and ns.detailEvenement or nil
	local typeCal = (d and d.typeCal) or e.typeCal
	if typeCal == "GUILD_ANNOUNCEMENT" then return "annonce de guilde : pas d'inscription" end
	if e.mod == "CREATOR" then return "tu as créé cet événement : modifie-le dans le calendrier du jeu" end
	if e.verrouille or (d and d.verrouille) then return "inscriptions closes" end
	if e.quand < maintenant() then return "événement déjà commencé" end
	if not e.coords then return "pas encore chargé : ouvre le calendrier du jeu une fois" end
	if cal("IsActionPending") == true then return "réponse en cours d'envoi" end
	return nil
end

-- quoi : "oui", "peut" ou "non". Les événements « inscription » de guilde prennent
-- EventSignUp ; les invitations, EventAvailable.
function ns.repondreEvenement(cle, quoi)
	local raison = ns.empechementReponse(cle)
	if raison then return nil, raison end
	local function envoyer()
		local d = ns.detailEvenement
		local inscription = d and d.inviteType == ((type(Enum) == "table" and Enum.CalendarInviteType
			and Enum.CalendarInviteType.Signup) or 1)
		local nom = (quoi == "peut" and "EventTentative") or (quoi == "non" and "EventDecline")
			or ((inscription and existe("EventSignUp")) and "EventSignUp" or "EventAvailable")
		if not existe(nom) then
			ns.dire("réponse impossible sur ce client : ouvre le calendrier du jeu.")
			return
		end
		cal(nom)
		ns.apres(1, function() ns.ouvrirEvenement(cle) end)
	end
	if ouvert == cle then
		envoyer()
	else
		actionEnAttente = { cle = cle, fn = envoyer }
		if not ns.ouvrirEvenement(cle) then actionEnAttente = nil return nil, "événement pas encore chargé" end
	end
	return true
end

function ns.ouvrirCalendrierDuJeu()
	if ToggleCalendar then pcall(ToggleCalendar) end
end

-- ------------------------------------------------------------------ bulles et évènements

-- Un nouvel événement de guilde : une bulle, une seule fois.
local function nouveautes()
	if not ns.db or ns.db.reglages.bullesEvenementsCoupees then return end
	local premiere = not ns.db.vus._evenementsLus
	for _, e in ipairs(ns.listeEvenements()) do
		local cle = "e:" .. e.cle
		if not ns.db.vus[cle] then
			ns.db.vus[cle] = ns.maintenant()
			-- la toute première lecture ne fait pas de bulle : tout serait « nouveau »
			if not premiere and e.typeCal ~= "PLAYER" then
				ns.bulle(("Nouvel événement : %s, %s"):format(e.titre, date("%d/%m à %Hh%M", e.quand)), "evenements")
			end
		end
	end
	ns.db.vus._evenementsLus = ns.maintenant()
end

for _, ev in ipairs({ "CALENDAR_UPDATE_EVENT_LIST", "CALENDAR_UPDATE_GUILD_EVENTS", "CALENDAR_NEW_EVENT",
	"CALENDAR_UPDATE_PENDING_INVITES" }) do
	ns.ecouter(ev, function()
		if not ns.db then return end
		ns.regrouper("calendrier", 1, function()
			nouveautes()
			ns.signaler("evenements")
		end)
	end)
end
for _, ev in ipairs({ "CALENDAR_OPEN_EVENT", "CALENDAR_UPDATE_EVENT", "CALENDAR_UPDATE_INVITE_LIST" }) do
	ns.ecouter(ev, function() if ns.db then lireOuvert() end end)
end
ns.ecouter("CALENDAR_CLOSE_EVENT", function() ouvert = nil end)
ns.ecouter("CALENDAR_UPDATE_ERROR", function(msg)
	ns.dire("le calendrier du jeu refuse : " .. tostring(msg or "?"))
end)

ns.surSignal("connexion", function()
	ns.apres(10, function() ns.demanderCalendrier(true) end)
end)
