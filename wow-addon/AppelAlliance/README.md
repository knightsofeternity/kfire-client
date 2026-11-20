# Appel d'Alliance (WoW Forever)

Add-on commun à **Knights of Eternity** et aux **Cassos de la Lumière** : chaque demande de groupe devient une
alerte visible et sonore (son, petite fenêtre, ligne dans le chat) avec un bouton « Intéressé ».
**Alliance seulement** : un canal personnalisé ne traverse pas les factions.

## Installation

1. Copier le dossier `AppelAlliance/` dans `World of Warcraft/_forever_/Interface/AddOns/`.
2. En jeu, la fenêtre de bienvenue vérifie la faction, la guilde et le compte, demande le pseudo Discord
   et rejoint le canal `KEAlliance` (le mot de passe est fourni par l'add-on). Ensuite, le canal est
   rejoint tout seul à chaque connexion.

## Commandes

| Commande | Effet |
|---|---|
| `/alliance` (ou `/aa`) | la liste des annonces |
| `/alliance new` | nouvelle annonce (clic droit sur le bouton de la minicarte aussi) |
| `/alliance dnd` | Ne pas déranger : les annonces arrivent dans la liste, sans son ni fenêtre |
| `/alliance membres` | l'onglet Alliance : les joueurs KE et Cassos qui ont l'add-on |
| `/alliance reglages` | pseudo Discord, rôle, son, types suivis, filtre de niveau |
| `/alliance bloquer Nom` / `debloquer Nom` | ignorer les annonces d'un joueur |
| `/alliance canal <mot de passe>` | changer de mot de passe si les deux guildes en décident un autre |
| `/alliance diag` | l'état technique, à copier en cas de problème |

## Comment ça circule

- Les annonces, leurs mises à jour, leur clôture et la présence (HELLO) passent par le canal `KEAlliance`.
- L'intérêt et le rattrapage passent en chuchotement d'add-on.
- **Option A** (par défaut) : messages d'add-on sur le canal, invisibles.
- **Option B** : si le jeu refuse ce type, l'add-on bascule tout seul en messages texte `[KEA]…`, masqués du chat.
  Le jeu n'accepte ces messages que pendant un clic du joueur : ils partent au prochain clic sur un bouton de
  l'add-on. Le lot 0 (sonde `/aa0`) dira laquelle des deux marche sur Forever.
- En instance, l'add-on est sourd et muet. À la sortie, il fait un rattrapage auprès d'un seul joueur, et
  une annonce rédigée en instance part à ce moment-là.

## À régler ensemble (`Config.lua`)

Le nom et le mot de passe du canal, les noms exacts des guildes, les liens d'invitation Discord, les listes
de donjons et de zones, les durées de vie des annonces.

## Tests

```bash
docker run --rm -v "$PWD/wow-addon":/w -w /w alpine:3.20 sh -c "apk add -q lua5.1 >/dev/null 2>&1 && lua5.1 test/alliance.lua"
```
