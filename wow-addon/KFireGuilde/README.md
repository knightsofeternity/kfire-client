# KFIRE Guilde (WoW Forever)

Outils de la guilde Knights of Eternity dans WoW Forever. Lot 1 : page Guilde et profils.
Le cahier des charges est dans [`../docs/kfire-guilde-cahier.md`](../docs/kfire-guilde-cahier.md).

Tout passe par des messages d'add-on invisibles sur le canal de guilde : seuls les membres qui ont
l'add-on partagent ces informations. Les fiches des membres hors ligne sont gardées et relayées par
ceux qui sont connectés.

## Installation (en attendant le client KFIRE)

1. Copier le dossier `KFireGuilde/` dans `World of Warcraft/_forever_/Interface/AddOns/`.
2. Lancer le jeu, vérifier que « KFIRE Guilde » est coché dans la liste des add-ons.
3. `/ke` ou un clic sur le médaillon (déplaçable).

## Commandes

| Commande | Effet |
|---|---|
| `/ke` | ouvrir / fermer la fenêtre (`/kfg` marche aussi) |
| `/ke guilde` | la page Guilde |
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
| `Interface.lua` | fenêtre, pages, formulaire, médaillon, `/ke` |

## Tests

Banc hors jeu : plusieurs joueurs simulés sur un même canal de guilde, avec le temps simulé.
Depuis la racine du dépôt :

```bash
docker run --rm -v "$PWD/wow-addon":/w -w /w alpine:3.20 sh -c "apk add -q lua5.1 >/dev/null 2>&1 && lua5.1 test/guilde.lua"
```
