# KFire, l'addon World of Warcraft de KFIRE

Blizzard ne publie le temps de jeu nulle part. Cet addon demande le /played de chaque
personnage à la connexion, sans l'afficher dans le chat, et l'écrit dans
`WTF/Account/<COMPTE>/SavedVariables/KFire.lua`. Le client KFIRE lit ce fichier après la
fermeture du jeu et envoie le temps de jeu à ton serveur KFIRE.

Il fonctionne dans Retail, Classic et Forever. `/kfire` affiche son état.

Normalement, c'est le client KFIRE qui l'installe et le met à jour.

## Tests

Sans Lua installé, depuis la racine du dépôt :

```bash
docker run --rm -v "$PWD/wow-addon":/w -w /w alpine:3.20 sh -c "apk add -q lua5.1 >/dev/null 2>&1 && lua5.1 test/harness.lua"
```
