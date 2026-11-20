# Appel d'Alliance, lot 0 : la sonde

Addon **jetable** pour WoW Forever. Il sert uniquement à vérifier qu'un message passe d'un joueur KE
à un joueur Cassos (même faction, Alliance) sur le canal partagé, avant d'écrire la moindre interface. Une fois le gate passé,
on le jette.

## Installation (les deux testeurs)

Copier le dossier `AppelAllianceSonde/` dans `World of Warcraft/_forever_/Interface/AddOns/`, lancer
le jeu, cocher « Appel d'Alliance - sonde » dans la liste des addons. `/aa0` affiche l'aide.

## Recette : un joueur KE (« K ») et un joueur Cassos (« C »)

Les deux commencent par `/aa0 etat` et recopient ce qui s'affiche dans le compte rendu.

| # | Étape | Qui | Commande | Attendu |
|---|---|---|---|---|
| 1 | Canal | K et C | `/aa0 canal <mot de passe>` | « canal KEAlliance rejoint (n° X) » |
| 2 | Option A | K | `/aa0 a test A` | chez C : `[A] K <Knights of Eternity> : test A` |
| 3 | Option B | C | `/aa0 b test B` | chez K : `[B] …`, et **rien** de visible dans le chat |
| 4 | Chuchotement d'addon | K | `/aa0 w Nom-Royaume` de C | chez K : « réponse reçue de C <Les Cassos de la Lumière> » |
| 5 | Donjon | C entre en donjon, K fait `/aa0 a pendant`, C ressort | | chez C : rien en donjon, puis « rattrapage terminé : 1 annonce » |
| 6 | Rafale | K | `/aa0 rafale` | chez K : « 10/10 acceptées » ; chez C, `/aa0 compte` : 10 reçues, aucune déconnexion |
| 7 | Compte d'essai | compte d'essai | `/aa0 a x` | message « compte d'essai » ; il reçoit quand même les annonces des autres |

Une étape refusée par le jeu s'affiche en orange ou en rouge avec le code renvoyé : le recopier tel quel.

## À noter pendant la recette

- **Même faction uniquement.** Un canal personnalisé ne relie pas l'Alliance et la Horde : l'addon
  ne fonctionne qu'entre joueurs de la même faction. KE et les Cassos sont tous deux Alliance ; un
  reroll Horde ne recevra rien. `/aa0 etat` affiche la faction.
- Les noms exacts des guildes : la sonde n'accepte que « Knights of Eternity » et « Les Cassos de la
  Lumière » (`GUILDES` en tête du fichier). Une annonce rejetée affiche « guilde … non autorisée ».

## Banc hors jeu

```bash
docker run --rm -v "$PWD/wow-addon":/w -w /w alpine:3.20 sh -c "apk add -q lua5.1 >/dev/null 2>&1 && lua5.1 test/sonde.lua"
```
