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

Le dossier `kfire-guilde/` est un sous-module : le dépôt
[knightsofeternity/wow-kfire-guilde](https://github.com/knightsofeternity/wow-kfire-guilde), celui du
deuxième add-on, les outils de la guilde dans WoW Forever (page Guilde, profils, demandes, artisanat,
événements, statistiques de présence, quêtes en commun). Le client embarque `kfire-guilde/KFireGuilde/`
à la compilation : il faut donc cloner avec `git clone --recurse-submodules`, ou lancer
`git submodule update --init` après coup.

Le cahier des charges, le contrat de liaison et le banc de tests sont dans ce dépôt-là. Pour livrer une
nouvelle version de l'add-on dans le client : la pousser sur `wow-kfire-guilde`, puis accepter la PR
Dependabot qui fait avancer le sous-module ici (vérification chaque semaine), ou le faire à la main :

```bash
git submodule update --remote wow-addon/kfire-guilde
git add wow-addon/kfire-guilde && git commit -m "chore(wow): bump KFIRE Guilde"
```

Si la liste des fichiers Lua change, mettre à jour `src-tauri/src/wow/guild_addon.rs` : un test vérifie
que le TOC et la liste embarquée correspondent.

## Appel d'Alliance (WoW Forever)

`appel-alliance/` est un deuxième sous-module : le dépôt
[knightsofeternity/wow-appel-alliance](https://github.com/knightsofeternity/wow-appel-alliance),
l'appel de groupe partagé avec Les Cassos de la Lumière. Le client embarque `appel-alliance/AppelAlliance/`
et l'installe dans Forever, case « Appel d'Alliance » de l'onglet World of Warcraft. Les Cassos, qui n'ont
pas KFIRE, l'installent à la main avec le zip. Mise à jour : comme KFIRE Guilde, par la PR Dependabot ou
`git submodule update --remote wow-addon/appel-alliance`. Un test vérifie que le TOC et
`src-tauri/src/wow/alliance_addon.rs` listent les mêmes fichiers.
