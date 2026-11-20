# KFIRE Guilde (WoW Forever)

Outils de la guilde Knights of Eternity dans WoW Forever. Lots 1 à 4 : page Guilde et profils, demandes et artisanat, événements et statistiques de présence, quêtes en commun.
Le cahier des charges est dans [`../docs/kfire-guilde-cahier.md`](../docs/kfire-guilde-cahier.md).

Tout passe par des messages d'add-on invisibles sur le canal de guilde : seuls les membres qui ont
l'add-on partagent ces informations. Les fiches des membres hors ligne sont gardées et relayées par
ceux qui sont connectés.

## Installation

Le client KFIRE l'installe tout seul dans WoW Forever (onglet World of Warcraft, case « KFIRE Guilde dans
Forever », cochée par défaut), le tient à jour et propose ton pseudo KFIRE dans le profil. Jeu fermé, il
envoie à KFIRE la liste de la guilde, les profils, les métiers et la présence, avec ton temps de jeu.
Les demandes et les quêtes restent dans le jeu.

À la main :

1. Copier le dossier `KFireGuilde/` dans `World of Warcraft/_forever_/Interface/AddOns/`.
2. Lancer le jeu, vérifier que « KFIRE Guilde » est coché dans la liste des add-ons.
3. `/ke` ou un clic sur le médaillon (déplaçable).

## Commandes

| Commande | Effet |
|---|---|
| `/ke` | ouvrir / fermer la fenêtre (`/kfg` marche aussi) |
| `/ke guilde` | la page Guilde |
| `/ke demandes` | les demandes de la guilde (publier, prendre, honorer) |
| `/ke artisanat` | qui sait fabriquer quoi |
| `/ke evenements` | les événements de guilde, avec l'inscription en un clic |
| `/ke stats` | la présence de la guilde : semaine type et meilleurs créneaux |
| `/ke quetes` | les membres en ligne qui font les mêmes quêtes |
| `/ke profil` | le formulaire de profil (pseudo KFIRE, perso principal, rôle) |
| `/ke options` | les options et le diagnostic |
| `/ke synchro` | redemander les fiches à la guilde |
| `/ke diag` | l'état technique, à copier en cas de problème |
| `/ke medaillon` | afficher / cacher le médaillon |

## Fichiers

| Fichier | Rôle |
|---|---|
| `Socle.lua` | temps, évènements, signaux internes, sauvegarde, noms, versions, instance |
| `Reseau.lua` | codage des messages, morceaux, file d'envoi avec débit, réception |
| `Magasin.lua` | fiches partagées (la plus récente gagne) et synchro à la connexion |
| `Profil.lua` | mon profil et la fiche `membres` |
| `Guilde.lua` | liste de guilde du jeu croisée avec les fiches, filtres et tris |
| `Metiers.lua` | métiers et recettes de chacun, « qui sait fabriquer » |
| `Demandes.lua` | les demandes : publier, prendre, honorer, retirer, bulles |
| `Evenements.lua` | lecture du calendrier du jeu et réponses |
| `Presence.lua` | relevé du nombre de membres en ligne, semaine type |
| `Quetes.lua` | quêtes en cours partagées, quêtes en commun |
| `Interface.lua` | fenêtre, Accueil, Guilde, Options, formulaire, bulles, médaillon, `/ke` |
| `PageDemandes.lua` | pages Demandes et Artisanat |
| `PageEvenements.lua` | pages Événements et Statistiques |
| `PageQuetes.lua` | page Quêtes |

## Tests

Banc hors jeu : plusieurs joueurs simulés sur un même canal de guilde, avec le temps simulé.
Depuis la racine du dépôt :

```bash
docker run --rm -v "$PWD/wow-addon":/w -w /w alpine:3.20 sh -c "apk add -q lua5.1 >/dev/null 2>&1 && lua5.1 test/guilde.lua"
```
