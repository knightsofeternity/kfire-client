# Cahier des charges : add-on « KFIRE Guilde » (WoW Forever)

Version du 20/11/2026 (addon 0.2.0, lots 1 à 4 écrits). Nom provisoire.

## Le besoin

La guilde SMB vit presque tout en jeu grâce à son add-on (SMB-Forever, un fork de GuildUp) : demandes,
artisanat, quêtes, événements. KE veut la même chose, **écrite par nous et liée à KFIRE** :

- le pseudo d'un membre est celui de KFIRE et du Discord ;
- le client KFIRE installe l'add-on et le met à jour, comme il le fait déjà pour l'add-on KFire du /played ;
- ce que l'add-on apprend (profils, métiers, présence) peut remonter sur le site KFIRE.

On n'en reprend aucun code : SMB-Forever est en GPL-3 et kfire-client en MIT. On en garde les idées qui ont fait leurs preuves.

## Périmètre V1

| Bloc | Ce que ça fait |
|---|---|
| **Page Guilde + profil** | Tous les membres, connectés ou non : niveau, classe, rang, zone ou dernière connexion, pseudo, perso principal, rôle, version de l'add-on. Recherche, filtres (Tous / En ligne / Avec l'addon), tris. Clic : chuchoter ; clic droit : inviter. |
| **Demandes + artisanat** | Chacun publie un besoin (une ressource, ou un objet à fabriquer, avec une quantité). Toute la guilde voit la liste, même quand l'auteur est hors ligne. Un membre la prend en charge, puis la marque honorée. Il y a aussi un registre « qui sait fabriquer quoi ». |
| **Événements + statistiques** | Les événements de guilde du calendrier, sans ouvrir le calendrier, avec l'inscription en un clic. La présence de la guilde par jour et par heure, pour choisir le créneau des sorties. |
| **Quêtes en commun** | Les quêtes que d'autres membres connectés ont aussi, avec « Inviter » en un clic. |

### Hors V1

WoWdex, jeux (tirage, dés), carte et repères des membres, portraits, plaques de nom. On les reverra si la guilde les demande.

## Principes techniques

- **Client WoW Forever** : interface 16001 (client 1.60.1, de l'ère Midnight). En instance, les add-ons ne peuvent rien envoyer et certaines valeurs du jeu sont secrètes. L'add-on met donc ses envois en attente et ne lit pas la liste de guilde en instance.
- **Canal** : messages d'add-on (préfixe `KFGU`) sur le canal de guilde (`GUILD`) et en chuchotement entre membres. Seuls les membres qui ont l'add-on voient ces données. Un chuchotement d'un non-membre est ignoré.
- **Fiches partagées** : toutes les données sont des fiches (un membre, une demande, un métier…). Chaque fiche porte l'heure du serveur de sa dernière mise à jour, et la plus récente gagne. Chacun garde les fiches sur son disque, si bien qu'un membre hors ligne reste visible.
- **Synchro à la connexion** : celui qui arrive demande ce qui a changé depuis sa dernière synchro. Chaque membre connecté tire un délai au hasard ; le premier annonce « je m'en occupe », les autres se taisent, et l'élu chuchote les fiches.
- **Débit** : le jeu limite le nombre de messages. L'add-on utilise une file unique à deux priorités (le direct passe avant la synchro) et réessaie quand le jeu refuse. Un message de plus de 255 octets part en morceaux.
- **Protocole versionné** : un message d'une autre version du protocole est ignoré, et le membre concerné est repéré (pour lui dire de mettre à jour).
- **Confiance** : on fait confiance aux membres de la guilde. Un membre pourrait relayer une fausse fiche ; ce n'est pas un enjeu pour V1.
- **Durées de vie** : une fiche membre sans nouvelles depuis 60 jours est oubliée. Une demande honorée ou retirée est oubliée après 3 jours, une autre après 30 jours sans changement.

### Protocole

| Message | Voie | Contenu |
|---|---|---|
| `R` | guilde, ou chuchoté pendant une synchro | une fiche : magasin, clé, date, auteur, champs |
| `S` | guilde | demande de synchro : depuis quand, qui demande |
| `Z` | guilde | « je m'en occupe » : les autres répondants abandonnent |
| `F` | chuchoté | fin de synchro, nombre de fiches envoyées |
| `#` | les deux | un morceau d'un message trop long |

Format : `1~R~membres~Ouranos-Chogall~1763600000~Ouranos-Chogall~Oura~Ouranos-Chogall~D~0.1.0`.

### Magasins de fiches

| Magasin | Clé | Champs | Lot |
|---|---|---|---|
| `membres` | personnage | pseudo, perso principal, rôle, version | 1 |
| `demandes` | auteur + numéro | type (ressource / artisanat), objet, quantité, texte, état, preneur | 2 |
| `metiers` | personnage + métier | niveau, maximum, recettes connues | 2 |
| `presence` | jour + heure | nombre de membres en ligne relevé | 3 |
| `quetes` | personnage | quêtes en cours (seulement pour les membres en ligne) | 4 |

## Lots

Chaque lot se termine par une recette en jeu à deux joueurs KE.

| Lot | Contenu | État |
|---|---|---|
| **1. Socle + Guilde** | Réseau, fiches et synchro, profil (pseudo, perso principal, rôle), page Guilde, médaillon, `/ke diag` | écrit, testé hors jeu |
| **2. Demandes + artisanat** | Publier, prendre, honorer, retirer une demande. « Tu en as N » (sacs et banque). Bulle quand un membre publie une demande qu'on peut honorer. Relevé des métiers et des recettes à l'ouverture de la fenêtre de métier. Recherche « qui sait fabriquer X » | écrit, testé hors jeu |
| **3. Événements + stats** | Liste des événements de guilde sur 30 jours, avec détail, inscrits, S'inscrire / Peut-être / Se désinscrire. Relevé du nombre de membres en ligne toutes les 10 min, gardé 8 semaines, semaine type et 5 meilleurs créneaux | écrit, testé hors jeu |
| **4. Quêtes en commun** | Les quêtes partagées par les membres en ligne, « Inviter », bulle quand un membre qui a la même quête est en ligne | écrit, testé hors jeu |
| **5. Liaison KFIRE** | Le client installe et met à jour KFireGuilde. Il lit ce que l'add-on a écrit sur le disque (jeu fermé) et l'envoie au serveur KFIRE : la page Guilde WoW Forever du site, les métiers, la présence. Il propose le pseudo KFIRE au premier lancement | à faire |

## Recette du lot 1 (deux joueurs KE)

- [ ] `/ke` ouvre la fenêtre, et le formulaire de profil s'ouvre la première fois.
- [ ] Le profil enregistré chez A apparaît chez B en moins de 5 s (pseudo, rôle, version).
- [ ] B se déconnecte. A change de profil. Au retour de B, `/ke diag` montre la synchro et le nouveau profil est là.
- [ ] Un troisième membre se connecte après la déconnexion de A : il voit quand même le profil de A.
- [ ] En donjon, changer son profil : rien ne part (`/ke diag` : 1 en attente). Il part à la sortie.
- [ ] Page Guilde : recherche, filtres, tris, clic (chuchoter) et clic droit (inviter).
- [ ] Aucune erreur Lua (`/ke diag` : « Dernière erreur : aucune »).

## Choix faits pendant l'écriture des lots 2 à 4

- **Modération** : le maître de guilde et les officiers (rangs 0 et 1) peuvent retirer la demande d'un autre. C'est la constante `ns.RANG_MODERATION` dans `Demandes.lua`.
- **Qui peut agir sur une demande** : un autre membre la prend en charge ; l'auteur ou le preneur la marque honorée ou la relâche ; l'auteur (ou un modérateur) la retire.
- **Recettes** : notées par l'objet fabriqué (son numéro), ou par le sort pour les enchantements. Elles ne font que s'ajouter, si bien qu'un filtre de la fenêtre de métier n'en fait oublier aucune. Elles repartent de zéro si le niveau du métier baisse (métier oublié puis repris).
- **Présence** : une fiche par jour, avec le plus grand nombre de membres en ligne pour chaque heure. Deux relevés du même jour se fusionnent. Pour ne pas multiplier les messages, seul le premier membre en ligne avec l'addon (par ordre alphabétique) publie, toutes les 30 minutes.
- **Quêtes** : seuls comptent les membres en ligne dont la liste a moins de 12 heures. Une bulle par membre et par quête, une seule fois par session.
- **Bulles** : chacune se coupe dans les Options. En combat ou en instance, seul le message dans la discussion apparaît.
- **Événements** : rien ne passe par le réseau de l'addon. Le calendrier du jeu est lu (`C_Calendar`) ; « S'inscrire » prend `EventSignUp` pour un événement à inscription, sinon `EventAvailable`. « Se désinscrire » prend `EventDecline`.

## Recette des lots 2 à 4 (deux joueurs KE)

- [ ] A publie une demande d'un objet que B a dans ses sacs : B a une bulle « tu en as N », et la demande est dans son filtre « Pour moi ».
- [ ] B clique « Je m'en occupe » : A a une bulle. B clique « Honorée » : A a une bulle, et la demande sort des ouvertes.
- [ ] Maj+clic sur un objet des sacs remplit la case Objet du formulaire.
- [ ] B ouvre sa fenêtre de métier : chez A, la page Artisanat montre son métier et ses recettes, et la recherche « qui sait fabriquer » le trouve.
- [ ] Page Événements : les événements de guilde apparaissent sans ouvrir le calendrier ; « S'inscrire » inscrit pour de vrai (à vérifier dans le calendrier du jeu).
- [ ] Un officier crée un événement : les autres ont une bulle.
- [ ] Page Statistiques : après quelques heures, la grille se remplit ; `/ke diag` compte les jours de présence.
- [ ] A et B ont une même quête : chacun a une bulle, la page Quêtes les montre, « Inviter » marche.
- [ ] Aucune erreur Lua (`/ke diag`).

## Points ouverts

- Nom définitif de l'add-on et commande (`/ke` est pris par autre chose ?).
- La modération (rangs 0 et 1) convient-elle ?
- Où afficher les données sur le site KFIRE (lot 5) : une page Guilde WoW Forever, ou dans la fiche membre ?
- Cohabitation avec « Appel d'Alliance » (KE × Cassos) : deux add-ons séparés, ou un onglet de KFIRE Guilde une fois le lot 0 validé ?
