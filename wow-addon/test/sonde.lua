-- Banc hors jeu de la sonde Appel d'Alliance (lot 0). Lua 5.1, comme WoW.
-- Ne remplace pas la recette en jeu : il vérifie le codage, le tri et l'élection du rattrapage.

local echecs = 0
local function out(s) io.write(s, "\n") end
local function verifier(cond, msg)
	if cond then out("ok   " .. msg) else echecs = echecs + 1 out("FAIL: " .. msg) end
end

-- ------------------------------------------------------------------ le monde simulé
local W
local function monde(opts)
	opts = opts or {}
	W = { now = 1763550000, minuteurs = {}, envois = {}, chat = {}, imprime = {}, filtres = {},
		canaux = { KEAlliance = 5 }, guilde = opts.guilde or "Knights of Eternity",
		instance = false, essai = opts.essai or false }
	AppelAllianceSondeDB = { motDePasse = "x", canal = "KEAlliance" }
	print = function(s) W.imprime[#W.imprime + 1] = s end
	time = function() return W.now end
	GetServerTime = time
	C_Timer = { After = function(d, fn) W.minuteurs[#W.minuteurs + 1] = { d = d, fn = fn } end }
	C_ChatInfo = {
		RegisterAddonMessagePrefix = function() end,
		SendAddonMessage = function(p, m, c, cible) W.envois[#W.envois + 1] = { p = p, m = m, c = c, cible = cible } return 0 end,
	}
	SendChatMessage = function(m, c, _, n) W.chat[#W.chat + 1] = { m = m, c = c, n = n } end
	ChatFrame_AddMessageEventFilter = function(ev, fn) W.filtres[ev] = fn end
	UnitFullName = function() return "Ouranos", "Chogall" end
	GetGuildInfo = function() return W.guilde end
	IsTrialAccount = function() return W.essai end
	IsInInstance = function() return W.instance, W.instance and "party" or "none" end
	GetChannelName = function(n) return W.canaux[n] or 0 end
	JoinChannelByName = function() end
	UnitFactionGroup = function() return "Alliance" end
	SlashCmdList = {}
	CreateFrame = function()
		local fr = {}
		function fr:RegisterEvent() end
		function fr:SetScript(_, fn) W.evenement = fn end
		return fr
	end
	local chunk = assert(loadfile("AppelAllianceSonde/AppelAllianceSonde.lua"))
	chunk("AppelAllianceSonde")
	W.evenement(nil, "ADDON_LOADED", "AppelAllianceSonde")
	return AppelAllianceSonde
end

local function lancerMinuteurs()
	local m = W.minuteurs
	W.minuteurs = {}
	for _, t in ipairs(m) do t.fn() end
end
local function addon(msg, canal, de) W.evenement(nil, "CHAT_MSG_ADDON", "KEALLY", msg, canal, de) end
local function slash(t) SlashCmdList.APPELALLIANCESONDE(t) end
local function dernierImprime() return W.imprime[#W.imprime] or "" end

-- ------------------------------------------------------------------ codage
local A = monde()
local msg = A.coder("A", 3, "Les Cassos de la Lumière", "RFC | 2 dps")
local genre, c = A.decoder(msg)
verifier(genre == "A" and c[3] == "3" and c[4] == "Les Cassos de la Lumière" and c[5] == "RFC / 2 dps", "un « | » dans le texte ne casse pas le découpage")
verifier(A.decoder("2|A|1|Cassos|x") == nil, "une autre version du protocole est ignorée")
local txt = A.versTexte(msg)
verifier(not txt:find("|", 1, true), "l'option B n'envoie aucun « | » dans le chat")
verifier(A.depuisTexte(txt) == msg, "l'option B se décode à l'identique")
verifier(A.depuisTexte("salut tout le monde") == nil, "un message de chat ordinaire n'est pas pris pour une annonce")
verifier(W.filtres.CHAT_MSG_CHANNEL(nil, nil, txt) == true, "le message encodé est masqué du chat")
verifier(W.filtres.CHAT_MSG_CHANNEL(nil, nil, "bonjour") == false, "le chat ordinaire reste visible")

-- ------------------------------------------------------------------ réception
A = monde()
addon(A.coder("A", 1, "Les Cassos de la Lumière", "Mortemines"), "CHANNEL", "Malkai-Chogall")
verifier(#A.actives() == 1, "une annonce Cassos reçue sur le canal est gardée")
addon(A.coder("A", 1, "Les Intrus", "spam"), "CHANNEL", "Troll-Chogall")
verifier(#A.actives() == 1 and dernierImprime():find("non autorisée"), "une guilde hors liste est ignorée")
addon(A.coder("A", 9, "Knights of Eternity", "moi"), "CHANNEL", "Ouranos")
verifier(#A.actives() == 1, "mes propres messages renvoyés par le canal sont ignorés")
W.evenement(nil, "CHAT_MSG_CHANNEL", A.versTexte(A.coder("A", 2, "Les Cassos de la Lumière", "par texte")), "Malkai")
verifier(#A.actives() == 2, "l'option B est reçue")

W.now = W.now + 601
verifier(#A.actives() == 0, "une annonce expire au bout de 10 minutes")

-- ------------------------------------------------------------------ instance
A = monde()
W.instance = true
W.evenement(nil, "PLAYER_ENTERING_WORLD", false, false)
addon(A.coder("A", 1, "Les Cassos de la Lumière", "pendant le donjon"), "CHANNEL", "Malkai")
verifier(#A.actives() == 0, "en donjon, rien n'est accepté")
W.instance = false
W.evenement(nil, "PLAYER_ENTERING_WORLD", false, false)
lancerMinuteurs()
local dmd = W.envois[#W.envois]
verifier(dmd and dmd.c == "CHANNEL" and A.decoder(dmd.m) == "S", "à la sortie, une demande de rattrapage part sur le canal")

-- ------------------------------------------------------------------ élection du rattrapage
A = monde()
addon(A.coder("A", 1, "Les Cassos de la Lumière", "Brasier"), "CHANNEL", "Malkai")
W.envois = {}
addon(A.coder("S", "Trayn123"), "CHANNEL", "Trayn")
addon(A.coder("H", "Trayn123"), "CHANNEL", "Nenos") -- quelqu'un a répondu plus vite
lancerMinuteurs()
verifier(#W.envois == 0, "si un autre joueur répond d'abord, on se tait")

A = monde()
addon(A.coder("A", 1, "Les Cassos de la Lumière", "Brasier"), "CHANNEL", "Malkai")
addon(A.coder("A", 2, "Les Cassos de la Lumière", "Gnomeregan"), "CHANNEL", "Malkai")
W.envois = {}
addon(A.coder("S", "Trayn124"), "CHANNEL", "Trayn")
lancerMinuteurs()
local genres = {}
for _, e in ipairs(W.envois) do genres[#genres + 1] = A.decoder(e.m) .. ":" .. e.c end
verifier(table.concat(genres, ",") == "H:CHANNEL,L:WHISPER,L:WHISPER,F:WHISPER",
	"le premier à répondre l'annonce sur le canal puis chuchote la liste (" .. table.concat(genres, ",") .. ")")
addon(A.coder("S", "Trayn124"), "CHANNEL", "Trayn")
W.envois = {}
lancerMinuteurs()
verifier(#W.envois == 0, "une demande déjà servie ne l'est pas deux fois")

-- Côté demandeur : la liste chuchotée remplit ses annonces.
A = monde()
addon(A.coder("L", 4, "Les Cassos de la Lumière", "Malkai", "Brasier"), "WHISPER", "Nenos")
addon(A.coder("F", 1), "WHISPER", "Nenos")
verifier(#A.actives() == 1 and dernierImprime():find("1 annonce"), "le demandeur récupère la liste chuchotée")

verifier(A.versTexte("x"):sub(1, 5) == "[KEA]", "l'option B est préfixée [KEA] comme dans le cahier")
addon(A.coder("A", 1, "Cassos", "ancien nom"), "CHANNEL", "Malkai")
verifier(dernierImprime():find("non autorisée"), "le nom de guilde doit être exact (« Cassos » seul est refusé)")

-- ------------------------------------------------------------------ publication
A = monde()
slash("a groupe RFC")
verifier(#W.envois == 1 and W.envois[1].c == "CHANNEL" and W.envois[1].cible == 5, "option A : sur le canal n° 5")
slash("b groupe RFC")
verifier(#W.chat == 1 and W.chat[1].c == "CHANNEL" and W.chat[1].n == 5, "option B : message de chat sur le canal n° 5")
W.envois = {}
slash("rafale")
verifier(#W.envois == 10, "la rafale envoie 10 annonces")
W.envois = {}
slash("w Malkai-Chogall")
verifier(#W.envois == 1 and W.envois[1].c == "WHISPER", "chuchotement d'addon")
addon(A.coder("P", "Les Cassos de la Lumière"), "WHISPER", "Malkai")
verifier(W.envois[#W.envois].c == "WHISPER" and A.decoder(W.envois[#W.envois].m) == "R", "un chuchotement d'essai reçoit une réponse")

A = monde({ essai = true })
slash("a coucou")
verifier(#W.envois == 0 and dernierImprime():find("compte d'essai"), "un compte d'essai ne publie pas et voit pourquoi")
addon(A.coder("A", 1, "Les Cassos de la Lumière", "x"), "CHANNEL", "Malkai")
verifier(#A.actives() == 1, "un compte d'essai reçoit les annonces")

A = monde()
W.canaux = {}
slash("a coucou")
verifier(#W.envois == 0 and dernierImprime():find("pas dans le canal"), "hors du canal, rien ne part")

out(echecs == 0 and "\ntout est vert" or ("\n" .. echecs .. " échec(s)"))
os.exit(echecs == 0 and 0 or 1)
