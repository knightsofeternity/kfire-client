# KFIRE Guilde : la liaison avec KFIRE (lot 5)

Version du 20/11/2026. Ce document fixe le contrat entre l'add-on, le client KFIRE et le serveur KFIRE.

## Ce que fait le client KFIRE

1. **Installation.** Le client embarque KFireGuilde, comme il embarque déjà l'add-on KFire du /played. Il l'installe dans l'édition **Forever** seulement (produit `wowf`, ou `wow_classic_beta` pour l'ancienne bêta), dans `Interface/AddOns/KFireGuilde/`. Le dossier porte le marqueur `.kfire` ; un dossier `KFireGuilde` sans marqueur appartient à quelqu'un d'autre et n'est jamais touché. La ligne `## Interface:` prend le numéro exact du client.
2. **Pseudo proposé.** Le client écrit aussi `Interface/AddOns/KFireGuilde/Propose.lua`, qui contient le pseudo KFIRE du membre :
   `KFireGuildePropose = { pseudo = "Oura" }`
   Ce fichier est le dernier du `.toc`. L'add-on s'en sert pour pré-remplir son formulaire de profil quand le membre n'a pas encore de pseudo. Il ne remplace jamais un pseudo déjà choisi.
3. **Lecture.** Le jeu fermé, le client lit `WTF/Account/*/SavedVariables/KFireGuilde.lua` (la variable `KFireGuildeDB`). Il ajoute un objet `guild` au `match_result` déjà envoyé pour l'édition Forever, celui qui porte le /played. Si plusieurs comptes de jeu ont un fichier, le plus récent (`roster.date`) l'emporte.

## Ce que l'add-on écrit (`KFireGuildeDB`, v = 1)

| Clé | Contenu |
|---|---|
| `moi` | `pseudo`, `main` (« Nom-Royaume »), `roles[perso] = "T" / "H" / "D"` |
| `magasins.membres[perso]` | `_maj`, `pseudo`, `main`, `role`, `version` |
| `magasins.metiers["perso/métier"]` | `_maj`, `metier`, `niveau`, `max`, `recettes` (« 2996,4343,s7418 ») |
| `magasins.presence["AAAAMMJJ"]` | `_maj`, `h` (24 nombres séparés par des virgules, « - » si inconnu) |
| `noms[id]` | nom d'un objet (ou d'un sort, `s` + numéro) |
| `roster` | `date`, `guilde`, `royaume`, `membres[perso] = { niveau, classe, rang, rangIndex, vu }` (`vu` : dernière connexion estimée, heure du serveur) |

Les demandes et les quêtes restent dans le jeu : elles ne remontent pas.

## L'objet `guild` du `match_result`

Il est facultatif : un serveur qui ne le connaît pas l'ignore, et le /played passe comme avant. Les dates sont en RFC 3339, en UTC.

```json
{
  "game_slug": "world-of-warcraft-forever",
  "characters": [ "… le /played, inchangé …" ],
  "guild": {
    "addon_version": "0.2.0",
    "guild_name": "Knights of Eternity",
    "realm_norm": "Chogall",
    "snapshot_at": "2026-11-20T21:04:00+00:00",
    "me": { "pseudo": "Oura", "main": "Ouranos-Chogall" },
    "roster": [
      { "name": "Ouranos-Chogall", "level": 60, "class": "MAGE", "rank": "Officier", "rank_index": 1,
        "last_seen_at": "2026-11-20T21:04:00+00:00" }
    ],
    "members": [
      { "name": "Ouranos-Chogall", "pseudo": "Oura", "main": "Ouranos-Chogall", "role": "D",
        "addon_version": "0.2.0", "updated_at": "2026-11-20T20:00:00+00:00" }
    ],
    "professions": [
      { "name": "Malkai-Chogall", "profession": "Couture", "level": 75, "max": 150,
        "recipes": ["2996", "4343"], "updated_at": "2026-11-20T19:00:00+00:00" }
    ],
    "item_names": { "2996": "Rouleau d'étoffe de lin", "4343": "Pantalon en lin brun" },
    "presence": [ { "day": "2026-11-20", "hours": [3, null, null, 1, "… 24 valeurs …"] } ]
  }
}
```

### Règles

- `name` est toujours « Nom-Royaume » (2 à 24 caractères pour le nom, 1 à 64 pour le royaume). `role` vaut `T`, `H`, `D` ou `null`. `class` est le nom de classe anglais en majuscules (`MAGE`, `PALADIN`…).
- Limites : 1000 lignes de `roster`, 1000 `members`, 3000 `professions` (1000 recettes chacune), 20 000 `item_names`, 70 jours de `presence` (24 valeurs chacun, `null` ou un entier de 0 à 1000).
- Une ligne invalide est écartée, pas l'objet entier. Un `guild` invalide dans son ensemble est ignoré, sans faire échouer le /played.
- Côté serveur, une guilde est identifiée par `guild_name` + `realm_norm`, sans tenir compte de la casse du nom. Chaque fiche (membre, métier) garde la version la plus récente (`updated_at`). La liste de guilde est remplacée par une photo plus récente (`snapshot_at`). La présence se fusionne : le plus grand nombre de chaque heure.
- Le serveur relie un personnage à un membre KFIRE quand ce personnage figure dans le /played de ce membre (table `wow_played`). C'est ce qui permet à la page Guilde du site de pointer vers le profil KFIRE.
