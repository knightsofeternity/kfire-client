-- Appel d'Alliance, lot 0 : la sonde. JETABLE.
--
-- Elle ne prouve qu'une chose : un message passe d'un joueur KE à un joueur Cassos par un canal
-- personnalisé protégé par mot de passe. Un canal personnalisé ne traverse pas les factions : KE et
-- les Cassos sont tous deux Alliance, et l'addon ne marche qu'entre joueurs de la même faction. Aucune interface : tout se pilote par /aa0 et se lit dans
-- le chat. Chaque commande correspond à une ligne de la recette du lot 0 (cahier des charges).
--
--   Option A : message d'addon de type CHANNEL (invisible par nature).
--   Option B : message texte sur le canal, encodé, décodé à l'arrivée et masqué du chat.
--   INT      : chuchotement d'addon d'une guilde à l'autre.
--   Donjon   : rien n'est accepté en instance, rattrapage à la sortie (un seul joueur répond).
--   Rafale   : 10 annonces d'affilée, numérotées, le destinataire compte les trous.
--   Essai    : un compte d'essai reçoit mais ne publie pas, avec une explication.

local ADDON = (...) or "AppelAllianceSonde"
local PREFIXE = "KEALLY"      -- préfixe des messages d'addon (16 caractères au plus, cahier des charges)
local PROTO = 1               -- version du protocole : un message d'une autre version est ignoré
local MARQUE = "[KEA]"        -- début d'un message texte de l'option B (filtré du chat)
local CANAL_DEFAUT = "KEAlliance"
local DUREE_ANNONCE = 600     -- une annonce de la sonde vit 10 minutes
local DELAI_CANAL = 6         -- le jeu refuse parfois un canal rejoint dès la connexion

-- Guildes autorisées. Le champ guilde vient de l'annonce elle-même, donc ce n'est qu'un tri :
-- le vrai garde-fou est le mot de passe du canal. Noms exacts à confirmer (point ouvert).
local GUILDES = { ["Knights of Eternity"] = true, ["Les Cassos de la Lumière"] = true }

local A = {}                  -- état interne, exposé au banc d'essai
_G.AppelAllianceSonde = A

A.annonces = {}               -- [cle] = { auteur, guilde, texte, seq, recue }
A.rafale = {}                 -- [auteur] = { recus = n, max = seq le plus haut }
A.enInstance = false
A.rattrapage = nil            -- { id, minuteur annulable } quand on s'apprête à répondre
A.vus = {}                    -- demandes de rattrapage déjà servies par quelqu'un

local function dire(fmt, ...)
	print(("|cffd4a017[AA0]|r " .. fmt):format(...))
end

local function maintenant() return (GetServerTime or time)() end

local function apres(delai, fn)
	local annule = false
	C_Timer.After(delai, function() if not annule then fn() end end)
	return function() annule = true end
end

-- Les clients de l'ère Midnight peuvent rendre des valeurs « secrètes » (en rencontre, en
-- instance) qu'on ne peut ni comparer ni découper. On les traite comme absentes.
local function secret(v) return issecretvalue ~= nil and issecretvalue(v) == true end

local function moi()
	local nom, royaume = UnitFullName("player")
	royaume = royaume or (GetNormalizedRealmName and GetNormalizedRealmName()) or ""
	return royaume ~= "" and (nom .. "-" .. royaume) or nom
end

local function maGuilde() return (GetGuildInfo("player")) or "" end

-- Le nom d'un expéditeur arrive avec ou sans royaume selon le message : on compare sans.
local function court(nom) return (nom or ""):match("^[^-]+") or "" end
local function estMoi(nom) return court(nom) == court(moi()) end

local function essai()
	return (IsTrialAccount and IsTrialAccount()) or (IsVeteranTrialAccount and IsVeteranTrialAccount()) or false
end

local function db()
	AppelAllianceSondeDB = AppelAllianceSondeDB or {}
	return AppelAllianceSondeDB
end

-- ------------------------------------------------------------------ codage
-- Champs séparés par « | ». Un « | » dans un texte libre devient « / » (il casserait le découpage),
-- et l'option B remplace les séparateurs par « ; », car le chat refuse « | » (code de couleur).

local function nettoyer(s) return (tostring(s or ""):gsub("|", "/"):gsub("\n", " ")) end

function A.coder(genre, ...)
	local champs = { tostring(PROTO), genre }
	for i = 1, select("#", ...) do champs[#champs + 1] = nettoyer((select(i, ...))) end
	return table.concat(champs, "|")
end

function A.decoder(msg)
	if type(msg) ~= "string" or secret(msg) then return nil end
	local champs = {}
	for c in (msg .. "|"):gmatch("([^|]*)|") do champs[#champs + 1] = c end
	if tonumber(champs[1]) ~= PROTO then return nil, "version" end
	return champs[2], champs
end

-- Option B : un message texte sur le canal. « | » est interdit dans le chat, on le remplace par « ; ».
function A.versTexte(msg) return MARQUE .. msg:gsub("|", ";") end
function A.depuisTexte(txt)
	if type(txt) ~= "string" or secret(txt) or txt:sub(1, #MARQUE) ~= MARQUE then return nil end
	return (txt:sub(#MARQUE + 1):gsub(";", "|"))
end

-- ------------------------------------------------------------------ canal
function A.numeroCanal()
	local nom = db().canal or CANAL_DEFAUT
	local id = GetChannelName(nom)
	return (id and id > 0) and id or nil, nom
end

function A.rejoindre()
	local d = db()
	if not d.motDePasse then
		dire("pas de mot de passe : /aa0 canal <mot de passe> (il est donné sur les deux Discord).")
		return
	end
	JoinChannelByName(d.canal or CANAL_DEFAUT, d.motDePasse)
	apres(2, function()
		local id, nom = A.numeroCanal()
		if id then dire("canal %s rejoint (n° %d). Il relie les joueurs de la même faction (%s) seulement.", nom, id, UnitFactionGroup("player") or "?") else dire("|cffff5555échec|r : canal %s non rejoint (mot de passe ?).", nom) end
	end)
end

-- ------------------------------------------------------------------ envoi
-- Toute émission passe par ici, pour journaliser ce que le jeu répond (les clients récents
-- renvoient un code : 0 = accepté, le reste = refus, débit dépassé, etc.).
function A.envoyerAddon(msg, canal, cible)
	local f = (C_ChatInfo and C_ChatInfo.SendAddonMessage) or SendAddonMessage
	local ok, res = pcall(f, PREFIXE, msg, canal, cible)
	if not ok then dire("|cffff5555envoi refusé|r (%s) : %s", canal, tostring(res)) return false end
	if type(res) == "number" and res ~= 0 then dire("|cffff9900le jeu répond %d|r à l'envoi %s.", res, canal) return false end
	return true
end

local function peutPublier()
	if essai() then
		dire("|cffff9900compte d'essai|r : les comptes d'essai ne peuvent pas écrire dans un canal personnalisé. " ..
			"Tu reçois les annonces, mais pour en publier il faut un compte complet.")
		return false
	end
	if not A.numeroCanal() then dire("pas dans le canal : /aa0 canal <mot de passe>.") return false end
	return true
end

A.seq = 0
function A.annonce(texte)
	A.seq = A.seq + 1
	return A.coder("A", A.seq, maGuilde(), texte ~= "" and texte or "annonce d'essai")
end

-- ------------------------------------------------------------------ réception
local function guildeAutorisee(g) return GUILDES[g] == true end

function A.recevoir(msg, expediteur, voie)
	if estMoi(expediteur) then return end
	local genre, c = A.decoder(msg)
	if not genre then
		if c == "version" then dire("message de %s ignoré : version du protocole différente.", expediteur) end
		return
	end

	if genre == "A" then
		-- c = proto, A, seq, guilde, texte
		local seq, guilde, texte = tonumber(c[3]) or 0, c[4] or "", c[5] or ""
		if A.enInstance then return end
		if not guildeAutorisee(guilde) then
			dire("annonce de %s ignorée : guilde « %s » non autorisée.", expediteur, guilde)
			return
		end
		A.annonces[court(expediteur) .. "#" .. seq] =
			{ auteur = expediteur, guilde = guilde, texte = texte, seq = seq, recue = maintenant() }
		local r = A.rafale[expediteur] or { recus = 0, max = 0 }
		r.recus, r.max = r.recus + 1, math.max(r.max, seq)
		A.rafale[expediteur] = r
		dire("[%s] %s <%s> : %s |cff888888(n° %d, via %s)|r", voie, expediteur, guilde, texte, seq, voie == "B" and "texte" or "addon")

	elseif genre == "P" then -- chuchotement d'essai : on répond
		dire("chuchotement d'addon reçu de %s <%s>, réponse envoyée.", expediteur, c[3] or "?")
		A.envoyerAddon(A.coder("R", maGuilde()), "WHISPER", expediteur)

	elseif genre == "R" then
		dire("|cff55ff55réponse reçue|r de %s <%s> : le chuchotement d'addon passe d'une guilde à l'autre.", expediteur, c[3] or "?")

	elseif genre == "S" then -- quelqu'un sort d'instance et demande les annonces actives
		A.preparerRattrapage(c[3], expediteur)

	elseif genre == "H" then -- quelqu'un d'autre répond déjà à cette demande
		A.vus[c[3] or ""] = true
		if A.rattrapage and A.rattrapage.id == c[3] then
			A.rattrapage.annuler()
			A.rattrapage = nil
		end

	elseif genre == "L" then -- une annonce renvoyée par chuchotement pendant un rattrapage
		local seq, guilde, auteur, texte = tonumber(c[3]) or 0, c[4] or "", c[5] or "", c[6] or ""
		if guildeAutorisee(guilde) then
			A.annonces[court(auteur) .. "#" .. seq] =
				{ auteur = auteur, guilde = guilde, texte = texte, seq = seq, recue = maintenant() }
			A.rattrapageRecu = (A.rattrapageRecu or 0) + 1
		end
	elseif genre == "F" then
		dire("rattrapage terminé par %s : %d annonce(s) reçue(s) sur %s annoncée(s).", expediteur, A.rattrapageRecu or 0, c[3] or "?")
	end
end

-- ------------------------------------------------------------------ rattrapage
function A.actives()
	local liste, t = {}, maintenant()
	for cle, a in pairs(A.annonces) do
		if t - a.recue < DUREE_ANNONCE then liste[#liste + 1] = a else A.annonces[cle] = nil end
	end
	table.sort(liste, function(x, y) return x.recue < y.recue end)
	return liste
end

-- Un seul joueur répond : chacun attend un délai tiré au hasard, le premier annonce « H » sur
-- le canal, les autres l'entendent et se taisent. La liste part ensuite en chuchotements.
function A.preparerRattrapage(id, demandeur)
	if not id or A.vus[id] or A.enInstance or #A.actives() == 0 then return end
	if A.rattrapage then A.rattrapage.annuler() end
	local annuler = apres(0.3 + math.random() * 2.2, function()
		A.rattrapage = nil
		if A.vus[id] then return end
		A.vus[id] = true
		local liste = A.actives()
		A.envoyerAddon(A.coder("H", id), "CHANNEL", A.numeroCanal())
		for _, a in ipairs(liste) do
			A.envoyerAddon(A.coder("L", a.seq, a.guilde, a.auteur, a.texte), "WHISPER", demandeur)
		end
		A.envoyerAddon(A.coder("F", #liste), "WHISPER", demandeur)
		dire("rattrapage servi à %s : %d annonce(s).", demandeur, #liste)
	end)
	A.rattrapage = { id = id, annuler = annuler }
end

function A.demanderRattrapage()
	local id = A.numeroCanal()
	if not id then return end
	A.rattrapageRecu = 0
	local demande = court(moi()) .. tostring(maintenant())
	A.envoyerAddon(A.coder("S", demande), "CHANNEL", id)
	dire("sortie d'instance : rattrapage demandé.")
end

-- ------------------------------------------------------------------ événements
local f = CreateFrame("Frame")
f:RegisterEvent("ADDON_LOADED")
f:RegisterEvent("PLAYER_ENTERING_WORLD")
f:RegisterEvent("CHAT_MSG_ADDON")
f:RegisterEvent("CHAT_MSG_CHANNEL")

f:SetScript("OnEvent", function(_, ev, ...)
	if ev == "ADDON_LOADED" and ... == ADDON then
		local enregistrer = (C_ChatInfo and C_ChatInfo.RegisterAddonMessagePrefix) or RegisterAddonMessagePrefix
		enregistrer(PREFIXE)

	elseif ev == "PLAYER_ENTERING_WORLD" then
		local premiere, recharge = ...
		if premiere or recharge then apres(DELAI_CANAL, function() if db().motDePasse then A.rejoindre() end end) end
		local etait = A.enInstance
		local dedans, genre = IsInInstance()
		A.enInstance = dedans and (genre == "party" or genre == "raid") or false
		if A.enInstance and not etait then dire("en instance : les annonces sont ignorées jusqu'à la sortie.") end
		if etait and not A.enInstance then apres(3, A.demanderRattrapage) end

	elseif ev == "CHAT_MSG_ADDON" then
		local prefixe, msg, canal, expediteur = ...
		if prefixe ~= PREFIXE or secret(msg) then return end
		if canal == "CHANNEL" or canal == "WHISPER" then A.recevoir(msg, expediteur, canal == "CHANNEL" and "A" or "INT") end

	elseif ev == "CHAT_MSG_CHANNEL" then
		local texte, expediteur = ...
		local msg = A.depuisTexte(texte)
		if msg then A.recevoir(msg, expediteur, "B") end
	end
end)

-- Option B : les messages encodés ne s'affichent jamais dans le chat, même si quelqu'un ajoute
-- le canal à une fenêtre.
local function filtre(_, _, texte) return A.depuisTexte(texte) ~= nil end
local ajouterFiltre = (ChatFrameUtil and ChatFrameUtil.AddMessageEventFilter) or ChatFrame_AddMessageEventFilter
if ajouterFiltre then ajouterFiltre("CHAT_MSG_CHANNEL", filtre) end

-- ------------------------------------------------------------------ commandes
local AIDE = {
	"Fonctionne uniquement entre joueurs de la même faction : KE et les Cassos sont Alliance.",
	"/aa0 canal <mot de passe> [nom]  rejoindre le canal (" .. CANAL_DEFAUT .. " par défaut)",
	"/aa0 a [texte]   option A : annonce en message d'addon sur le canal",
	"/aa0 b [texte]   option B : annonce en message texte encodé (masqué du chat)",
	"/aa0 w Nom-Royaume   chuchotement d'addon, l'autre répond tout seul",
	"/aa0 rafale      10 annonces d'affilée (option A)",
	"/aa0 liste       annonces actives reçues",
	"/aa0 compte      annonces reçues par auteur (pour la rafale)",
	"/aa0 sync        demander un rattrapage comme à la sortie d'un donjon",
	"/aa0 etat        tout ce qu'il faut recopier dans le compte rendu",
	"/aa0 raz         oublier les annonces et compteurs reçus",
}

local commandes = {}

function commandes.canal(arg)
	local mdp, nom = arg:match("^(%S+)%s*(%S*)$")
	if not mdp then dire("usage : /aa0 canal <mot de passe> [nom du canal]") return end
	db().motDePasse, db().canal = mdp, (nom ~= "" and nom) or db().canal or CANAL_DEFAUT
	A.rejoindre()
end

function commandes.a(arg)
	if not peutPublier() then return end
	if A.envoyerAddon(A.annonce(arg), "CHANNEL", (A.numeroCanal())) then dire("annonce n° %d envoyée (option A).", A.seq) end
end

function commandes.b(arg)
	if not peutPublier() then return end
	-- Un message de chat sur un canal doit partir d'une action du joueur : la touche Entrée qui
	-- valide la commande en est une.
	local ok, err = pcall(SendChatMessage, A.versTexte(A.annonce(arg)), "CHANNEL", nil, (A.numeroCanal()))
	if ok then dire("annonce n° %d envoyée (option B).", A.seq) else dire("|cffff5555option B refusée|r : %s", tostring(err)) end
end

function commandes.w(arg)
	if arg == "" then dire("usage : /aa0 w Nom-Royaume") return end
	if A.envoyerAddon(A.coder("P", maGuilde()), "WHISPER", arg) then dire("chuchotement d'addon envoyé à %s.", arg) end
end

function commandes.rafale()
	if not peutPublier() then return end
	local id, envoyes = A.numeroCanal(), 0
	for i = 1, 10 do
		if A.envoyerAddon(A.annonce("rafale " .. i .. "/10"), "CHANNEL", id) then envoyes = envoyes + 1 end
	end
	dire("rafale : %d/10 acceptées par le jeu (n° %d à %d). Chez l'autre : /aa0 compte.", envoyes, A.seq - 9, A.seq)
end

function commandes.liste()
	local liste = A.actives()
	if #liste == 0 then dire("aucune annonce active.") return end
	for _, a in ipairs(liste) do dire("%s <%s> n° %d : %s", a.auteur, a.guilde, a.seq, a.texte) end
end

function commandes.compte()
	local rien = true
	for auteur, r in pairs(A.rafale) do
		rien = false
		dire("%s : %d reçue(s), numéro le plus haut %d.", auteur, r.recus, r.max)
	end
	if rien then dire("rien reçu.") end
end

function commandes.sync() A.demanderRattrapage() end

function commandes.etat()
	local id, nom = A.numeroCanal()
	local dedans, genre = IsInInstance()
	dire("version sonde %s, protocole %d, client %s (interface %s)", (C_AddOns and C_AddOns.GetAddOnMetadata or GetAddOnMetadata)(ADDON, "Version") or "?",
		PROTO, (GetBuildInfo()), select(4, GetBuildInfo()))
	dire("%s, guilde « %s » (%s), faction %s, compte d'essai : %s", moi(), maGuilde(),
		guildeAutorisee(maGuilde()) and "autorisée" or "|cffff9900non autorisée|r", UnitFactionGroup("player") or "?", essai() and "oui" or "non")
	dire("canal %s : %s ; instance : %s", nom, id and ("n° " .. id) or "|cffff5555absent|r", dedans and (genre or "oui") or "non")
	dire("%d annonce(s) active(s) reçue(s)", #A.actives())
end

function commandes.raz() A.annonces, A.rafale, A.vus = {}, {}, {} dire("oublié.") end

SLASH_APPELALLIANCESONDE1 = "/aa0"
SlashCmdList.APPELALLIANCESONDE = function(texte)
	local cmd, arg = (texte or ""):match("^%s*(%S*)%s*(.-)%s*$")
	local fn = commandes[(cmd or ""):lower()]
	if fn then fn(arg or "") return end
	for _, l in ipairs(AIDE) do dire(l) end
end
