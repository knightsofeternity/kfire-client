// Two flat dictionaries and a lookup. Two languages and about a hundred keys do
// not justify a library. The language follows the system; there is no picker.

const en = {
  "tab.home": "Home",
  "tab.settings": "Settings",

  "status.online": "Online",
  "status.invisible": "Invisible",
  "status.offline": "Offline",

  "conn.connected": "connected",
  "conn.connecting": "connecting…",
  "conn.reconnecting": "reconnecting…",
  "conn.loggedOut": "not linked",

  "home.status": "My status",
  "home.nowPlaying": "Now playing",
  "home.noGame": "No game detected.",
  "home.tracked": "match tracking on",
  "home.untracked": "match tracking off",
  "home.stop": "Stop",
  "home.ignore": "Ignore",
  "home.ignoreTitle": "Always ignore this game",
  "home.gamesCount": "{n} games recognised",

  "foot.upToDate": "up to date",
  "foot.available": "{v} available",
  "foot.tray": "Runs in the tray: closing this window keeps KFIRE running.",

  "link.intro": "Connect this app to your organization's KFIRE server.",
  "link.expired": "Your session on {url} expired. Link this device again to resume tracking.",
  "link.address": "Server address",
  "link.submit": "Link this device",
  "link.opening": "Opening browser…",
  "link.failed": "Linking failed",
  "pair.opened": "We opened your browser to confirm the link.",
  "pair.ifNot": "If it didn't open, go to:",
  "pair.approve": "and approve the code:",
  "pair.waiting": "Waiting for approval…",
  "common.cancel": "Cancel",

  "set.servers": "Servers",
  "set.useGlobal": "Use global",
  "set.serverStatusTitle": "Status for this server",
  "set.unlink": "Unlink",
  "set.unlinkTitle": "Unlink this server",
  "set.add": "+ Add a server",
  "set.link": "Link",
  "set.startup": "Startup",
  "set.autostart": "Launch KFIRE at startup",
  "set.ignored": "Ignored games",
  "set.noIgnored": "No ignored game.",
  "set.reenable": "Re-enable",

  "game.pip.on": "Active",
  "game.pip.todo": "Needs setup",
  "game.pip.off": "Off",
  "game.track": "Track my matches",
  "game.sent": "What is sent",
  "game.lastMatch": "Last match",
  "game.tech": "Technical details",
  "game.techFile": "KFIRE writes this file:",
  "game.techContent": "with exactly this content:",
  "game.noInstall": "Install folder not found. Launch {game} once, then reopen this window.",

  "field.mode": "Mode",
  "field.result": "Result",
  "field.turns": "Turns",
  "field.placement": "Placement",
  "field.hero": "Hero",
  "field.score": "Score",
  "field.goals": "Goals",
  "field.assists": "Assists",
  "field.saves": "Saves",
  "field.shots": "Shots",
  "field.demos": "Demos",
  "field.duration": "Duration",
  "field.champion": "Champion",
  "field.level": "Level",
  "field.kda": "KDA",
  "field.cs": "CS",
  "field.gold": "Gold",
  "field.time": "Game time",

  "result.win": "Win",
  "result.loss": "Loss",
  "result.draw": "Draw",
  "ago.now": "just now",

  "hs.subtitle": "Battlegrounds and regular games",
  "hs.private": "Your opponents' names and the cards played never leave this computer.",
  "hs.linux": "Hearthstone has no Linux version: nothing to track on this computer.",
  "hs.mode.battlegrounds": "Battlegrounds",
  "hs.mode.constructed": "Regular game",
  "hs.turns": "{n} turns",

  "rl.name": "Your in-game name (exactly as shown)",
  "rl.namePlaceholder": "e.g. DonZeZe",
  "rl.nameMissing": "Your in-game name is missing: KFIRE uses it to find your line on the scoreboard. It is never sent to the server.",
  "rl.subtitle": "Takes effect the next time the game starts",
  "rl.private": "The other players' names never leave this computer.",
  "rl.notRunning": "Rocket League is not running.",
  "rl.waitSocket": "Waiting for the game's socket. If you just turned tracking on, restart Rocket League: it only reads its configuration at startup.",
  "rl.connected": "Connected to Rocket League, {n} messages received.",
  "rl.silent": "Connected to Rocket League, but the game sends nothing. Start a match: the socket only talks during a match.",
  "rl.mismatch": "Your last match matched nobody. The game saw these names: {names}. One of them is yours: copy it exactly into the field above.",
  "rl.teams": "{n}v{n}",

  "lol.subtitle": "Your current game on the portal's Live games page",
  "lol.private": "The nine other players' names never leave this computer.",
  "lol.watching": "Game in progress detected.",
  "lol.idle": "No game in progress. The game's API only exists during a game: this is normal between games.",
  "lol.noSetup": "Nothing to set up: the game itself says which player you are.",
} as const;

export type Key = keyof typeof en;
export type Lang = "fr" | "en";

const fr: Record<Key, string> = {
  "tab.home": "Accueil",
  "tab.settings": "Réglages",

  "status.online": "En ligne",
  "status.invisible": "Invisible",
  "status.offline": "Hors ligne",

  "conn.connected": "connecté",
  "conn.connecting": "connexion…",
  "conn.reconnecting": "reconnexion…",
  "conn.loggedOut": "non relié",

  "home.status": "Mon statut",
  "home.nowPlaying": "En jeu maintenant",
  "home.noGame": "Aucun jeu détecté.",
  "home.tracked": "partie suivie",
  "home.untracked": "suivi désactivé",
  "home.stop": "Arrêter",
  "home.ignore": "Ignorer",
  "home.ignoreTitle": "Toujours ignorer ce jeu",
  "home.gamesCount": "{n} jeux reconnus",

  "foot.upToDate": "à jour",
  "foot.available": "{v} disponible",
  "foot.tray": "KFIRE reste dans la zone de notification quand cette fenêtre est fermée.",

  "link.intro": "Reliez cette application au serveur KFIRE de votre organisation.",
  "link.expired": "Votre session sur {url} a expiré. Reliez cet appareil pour reprendre le suivi.",
  "link.address": "Adresse du serveur",
  "link.submit": "Relier cet appareil",
  "link.opening": "Ouverture du navigateur…",
  "link.failed": "La liaison a échoué",
  "pair.opened": "Nous avons ouvert votre navigateur pour confirmer la liaison.",
  "pair.ifNot": "S'il ne s'est pas ouvert, allez sur :",
  "pair.approve": "et approuvez le code :",
  "pair.waiting": "En attente d'approbation…",
  "common.cancel": "Annuler",

  "set.servers": "Serveurs",
  "set.useGlobal": "Statut global",
  "set.serverStatusTitle": "Statut pour ce serveur",
  "set.unlink": "Délier",
  "set.unlinkTitle": "Délier ce serveur",
  "set.add": "+ Ajouter un serveur",
  "set.link": "Relier",
  "set.startup": "Démarrage",
  "set.autostart": "Lancer KFIRE au démarrage",
  "set.ignored": "Jeux ignorés",
  "set.noIgnored": "Aucun jeu ignoré.",
  "set.reenable": "Réactiver",

  "game.pip.on": "Actif",
  "game.pip.todo": "À configurer",
  "game.pip.off": "Désactivé",
  "game.track": "Suivre mes parties",
  "game.sent": "Ce qui est envoyé",
  "game.lastMatch": "Dernière partie",
  "game.tech": "Détails techniques",
  "game.techFile": "KFIRE écrit ce fichier :",
  "game.techContent": "avec exactement ce contenu :",
  "game.noInstall": "Dossier d'installation introuvable. Lancez {game} une fois, puis rouvrez cette fenêtre.",

  "field.mode": "Mode",
  "field.result": "Résultat",
  "field.turns": "Tours",
  "field.placement": "Position",
  "field.hero": "Héros",
  "field.score": "Score",
  "field.goals": "Buts",
  "field.assists": "Passes",
  "field.saves": "Arrêts",
  "field.shots": "Tirs",
  "field.demos": "Démos",
  "field.duration": "Durée",
  "field.champion": "Champion",
  "field.level": "Niveau",
  "field.kda": "KDA",
  "field.cs": "Sbires",
  "field.gold": "Or",
  "field.time": "Temps de jeu",

  "result.win": "Victoire",
  "result.loss": "Défaite",
  "result.draw": "Égalité",
  "ago.now": "à l'instant",

  "hs.subtitle": "Champs de bataille et parties classiques",
  "hs.private": "Le pseudo de vos adversaires et les cartes jouées ne quittent jamais cet ordinateur.",
  "hs.linux": "Hearthstone n'a pas de version Linux : rien à suivre sur cet ordinateur.",
  "hs.mode.battlegrounds": "Champs de bataille",
  "hs.mode.constructed": "Partie classique",
  "hs.turns": "{n} tours",

  "rl.name": "Votre pseudo en jeu (exactement comme affiché)",
  "rl.namePlaceholder": "ex. DonZeZe",
  "rl.nameMissing": "Il manque votre pseudo : KFIRE s'en sert pour retrouver votre ligne dans la feuille de match. Il n'est jamais envoyé au serveur.",
  "rl.subtitle": "Prend effet au prochain lancement du jeu",
  "rl.private": "Les pseudos des autres joueurs ne quittent jamais cet ordinateur.",
  "rl.notRunning": "Rocket League n'est pas lancé.",
  "rl.waitSocket": "En attente de la socket du jeu. Si vous venez d'activer le suivi, redémarrez Rocket League : le jeu ne lit sa configuration qu'au démarrage.",
  "rl.connected": "Connecté à Rocket League, {n} messages reçus.",
  "rl.silent": "Connecté à Rocket League, mais le jeu n'envoie rien. Lancez une partie : la socket ne parle qu'en match.",
  "rl.mismatch": "Votre dernier match n'a correspondu à personne. Le jeu a vu ces pseudos : {names}. L'un d'eux est le vôtre : copiez-le exactement dans le champ ci-dessus.",
  "rl.teams": "{n}c{n}",

  "lol.subtitle": "Votre partie en cours sur la page Jeux en live du portail",
  "lol.private": "Les pseudos des neuf autres joueurs ne quittent jamais cet ordinateur.",
  "lol.watching": "Partie en cours détectée.",
  "lol.idle": "Aucune partie en cours. L'API du jeu n'existe que pendant une partie : c'est normal entre deux parties.",
  "lol.noSetup": "Rien à configurer : le jeu dit lui-même quel joueur vous êtes.",
};

const dicts: Record<Lang, Record<Key, string>> = { en, fr };

export function pickLang(locale: string | undefined): Lang {
  return locale?.toLowerCase().startsWith("fr") ? "fr" : "en";
}

export function translate(
  lang: Lang,
  key: Key,
  params?: Record<string, string | number>,
): string {
  let s = dicts[lang][key] ?? dicts.en[key] ?? key;
  for (const [k, v] of Object.entries(params ?? {})) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}

export const lang: Lang = pickLang(
  typeof navigator === "undefined" ? undefined : navigator.language,
);

export function t(key: Key, params?: Record<string, string | number>): string {
  return translate(lang, key, params);
}
