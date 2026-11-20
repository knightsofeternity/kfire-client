# KFire, l'addon World of Warcraft de KFIRE

Blizzard ne publie le temps de jeu nulle part. Cet addon demande le /played de chaque
personnage à la connexion, sans l'afficher dans le chat, et l'écrit dans
`WTF/Account/<COMPTE>/SavedVariables/KFire.lua`. Le client KFIRE lit ce fichier après la
fermeture du jeu et envoie le temps de jeu à ton serveur KFIRE.

Il fonctionne dans Retail, Classic, Forever et Ascension (client 3.3.5). `/kfire` affiche son état.

Normalement, c'est le client KFIRE qui l'installe et le met à jour.

## Tests

Sans Lua installé, depuis la racine du dépôt :

```bash
docker run --rm -v "$PWD/wow-addon":/w -w /w alpine:3.20 sh -c "apk add -q lua5.1 >/dev/null 2>&1 && lua5.1 test/harness.lua"
```

## KFIRE Guilde (WoW Forever)

Le dossier `KFireGuilde/` est un deuxième add-on, celui des outils de la guilde dans WoW Forever (page Guilde,
profils, puis demandes, artisanat, événements, quêtes). Voir [`KFireGuilde/README.md`](KFireGuilde/README.md)
et le cahier des charges [`docs/kfire-guilde-cahier.md`](docs/kfire-guilde-cahier.md). Banc : `lua5.1 test/guilde.lua`.
