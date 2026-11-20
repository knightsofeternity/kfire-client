-- Appel d'Alliance : ce que les deux guildes règlent ensemble. Tout le reste en découle.
--
-- Les points ouverts du cahier des charges sont ici : nom et mot de passe du canal,
-- noms exacts des guildes, liens Discord, listes des donjons et des zones.
local _, ns = ...

ns.VERSION = "1.0.0"
ns.PROTO = 1                 -- version du protocole : un message d'une autre version est ignoré
ns.PREFIXE = "KEALLY"        -- préfixe des messages d'addon (16 caractères au plus)
ns.MARQUE = "[KEA]"          -- début d'un message texte de l'option B (filtré du chat)

-- Le canal commun. Le mot de passe est fourni par l'addon (décision du 20/11/2026) : rien à
-- taper. Il ne circule que sur les deux Discord, avec l'addon. « /alliance canal <mot de passe> »
-- le change si les deux guildes en décident un autre.
ns.CANAL = "KEAlliance"
ns.MOT_DE_PASSE = "kexcassos"

-- Les guildes de l'alliance. Le nom doit être exactement celui du jeu.
-- discord : l'invitation de la guilde, affichée dans un champ copiable (un addon n'ouvre pas de lien).
ns.GUILDES = {
	{ nom = "Knights of Eternity", sigle = "KE", couleur = { 0.25, 0.63, 1 }, discord = "" },
	{ nom = "Les Cassos de la Lumière", sigle = "Cassos", couleur = { 0.98, 0.72, 0.25 }, discord = "" },
}

-- Comment les annonces circulent (lot 0) :
--   "A" : messages d'addon sur le canal (invisibles, envoi automatique possible) ;
--   "B" : messages texte codés sur le canal, masqués du chat (seulement au clic du joueur).
-- En "auto", l'addon commence en A et passe en B si le jeu refuse le type CHANNEL.
ns.TRANSPORT = "auto"

-- Types d'annonce : durée de vie par défaut et champs du formulaire.
ns.TYPES = {
	{ code = "D", nom = "Donjon", duree = 30 * 60, champs = { lieu = "donjon", roles = true, niveau = true } },
	{ code = "Q", nom = "Quête / zone", duree = 45 * 60, champs = { lieu = "zone", quetes = true, niveau = true } },
	{ code = "E", nom = "Élite / boss / JcJ", duree = 20 * 60, champs = { lieu = "zone", places = true } },
	{ code = "L", nom = "Libre", duree = 15 * 60, champs = {} },
}

ns.ROLES = { T = "Tank", H = "Heal", D = "DPS" }

-- Les sons natifs du jeu proposés (SOUNDKIT) : pas de fichier à installer.
ns.SONS = {
	{ nom = "Alerte de raid", kit = "RAID_WARNING" },
	{ nom = "Prêt ?", kit = "READY_CHECK" },
	{ nom = "Chuchotement", kit = "TELL_MESSAGE" },
	{ nom = "Cloche", kit = "UI_BNET_TOAST" },
	{ nom = "Quête acceptée", kit = "IG_QUEST_LIST_OPEN" },
}

-- Donjons et raids (nom, niveau conseillé). Liste à confirmer pour Forever.
ns.DONJONS = {
	{ "Gouffre de Ragefeu", 13 }, { "Les Mortemines", 17 }, { "Cavernes des Lamentations", 17 },
	{ "Donjon d'Ombrecroc", 22 }, { "Profondeurs de Brassenoire", 24 }, { "La Prison", 24 },
	{ "Gnomeregan", 29 }, { "Kraal de Tranchebauge", 29 }, { "Monastère écarlate : Cimetière", 30 },
	{ "Monastère écarlate : Bibliothèque", 33 }, { "Monastère écarlate : Armurerie", 35 },
	{ "Monastère écarlate : Cathédrale", 37 }, { "Souilles de Tranchebauge", 37 }, { "Uldaman", 41 },
	{ "Zul'Farrak", 44 }, { "Maraudon", 46 }, { "Temple d'Atal'Hakkar", 50 },
	{ "Profondeurs de Rochenoire", 52 }, { "Pic de Rochenoire inférieur", 55 }, { "Pic de Rochenoire supérieur", 58 },
	{ "Hache-Tripes", 55 }, { "Scholomance", 58 }, { "Stratholme", 58 },
	{ "Cœur du Magma", 60 }, { "Repaire d'Onyxia", 60 }, { "Zul'Gurub", 60 }, { "Repaire de l'Aile noire", 60 },
	{ "Ruines d'Ahn'Qiraj", 60 }, { "Temple d'Ahn'Qiraj", 60 }, { "Naxxramas", 60 },
}

-- Zones (côté Alliance et neutres). Liste à confirmer pour Forever.
ns.ZONES = {
	"Forêt d'Elwynn", "Dun Morogh", "Teldrassil", "Marche de l'Ouest", "Loch Modan", "Sombrivage",
	"Les Carmines", "Bois de la Pénombre", "Les Paluns", "Contreforts de Hautebrande", "Les Tarides",
	"Orneval", "Serres-Rocheuses", "Mille pointes", "Désolace", "Strangleronce", "Marécage d'Âprefange",
	"Hautes-terres d'Arathi", "Terres ingrates", "Les Hinterlands", "Féralas", "Tanaris", "Azshara",
	"Terres foudroyées", "Gorge des Vents brûlants", "Steppes ardentes", "Marais des Chagrins",
	"Cratère d'Un'Goro", "Gangrebois", "Berceau-de-l'Hiver", "Maleterres de l'Ouest", "Maleterres de l'Est",
	"Silithus", "Hurlevent", "Forgefer", "Darnassus",
}
