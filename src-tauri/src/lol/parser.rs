//! Le décodage de `/liveclientdata/allgamedata`, sans aucune entrée/sortie.
//!
//! Séparé de `api.rs` pour la même raison que `rl/parser.rs` l'est de
//! `rl/socket.rs` : la réponse du jeu se décode et se résume sans réseau, donc
//! ça se teste sans partie en cours.
//!
//! Ce module voit la partie ENTIÈRE : les dix joueurs, leurs Riot ID, leurs
//! équipes, leurs objets et l'historique des évènements. Il n'en fait sortir
//! que des nombres sur le membre. C'est ici, et seulement ici, que le nom du
//! membre est lu, pour retrouver sa ligne.

use serde_json::Value;

/// L'état du membre pendant la partie, et rien d'autre.
///
/// Aucun champ ne porte de nom : ni celui du membre, ni celui des neuf autres.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Live {
    pub champion: String,
    pub level: i64,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub creep_score: i64,
    pub gold: i64,
    pub game_time_seconds: i64,
}

/// Décode une réponse d'`allgamedata` en l'état du membre.
///
/// Rend `None` plutôt que de paniquer dès que la réponse n'a pas la forme
/// attendue : hors partie l'API n'existe pas, mais pendant l'écran de
/// chargement elle peut déjà répondre sans que la liste des joueurs soit
/// remplie, et une réponse tronquée est un cas courant, pas une anomalie.
pub fn parse(raw: &str) -> Option<Live> {
    let root: Value = serde_json::from_str(raw).ok()?;
    let active = root.get("activePlayer")?;
    let players = root.get("allPlayers")?.as_array()?;
    let me = find_member(players, active)?;

    let scores = me.get("scores")?;
    Some(Live {
        champion: me.get("championName")?.as_str()?.to_string(),
        // Le niveau est dans les deux blocs et ils s'accordent ; celui du bloc
        // du membre fait foi, parce qu'il est vrai même si sa ligne de la liste
        // arrive en retard.
        level: num(active.get("level")).or_else(|| num(me.get("level")))?,
        kills: num(scores.get("kills"))?,
        deaths: num(scores.get("deaths"))?,
        assists: num(scores.get("assists"))?,
        creep_score: num(scores.get("creepScore"))?,
        // `currentGold` est un flottant dans le protocole, pas un entier.
        gold: num(active.get("currentGold"))?,
        // `gameTime` aussi : des secondes fractionnaires depuis le début.
        game_time_seconds: num(root.get("gameData").and_then(|g| g.get("gameTime")))?,
    })
}

/// Un nombre du protocole, entier ou flottant, ramené à l'entier.
///
/// Tronqué, jamais arrondi : à 842,9 secondes de jeu la partie dure 842
/// secondes, elle n'en dure pas encore 843.
fn num(v: Option<&Value>) -> Option<i64> {
    let v = v?;
    if let Some(i) = v.as_i64() {
        return Some(i);
    }
    let f = v.as_f64()?;
    if !f.is_finite() {
        return None;
    }
    Some(f as i64)
}

/// Les façons dont l'API peut nommer un joueur, telles quelles.
///
/// Le protocole a migré des noms d'invocateur vers les Riot ID, et les deux
/// cohabitent : le client sert encore `summonerName` à côté de `riotId`, et
/// l'un ou l'autre peut être vide selon la version. On compare donc tout ce qui
/// est présent des deux côtés, sans supposer laquelle des formes est servie.
///
/// Ces chaînes ne servent qu'à comparer, ici, en mémoire. Aucune ne ressort.
fn identities(p: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["riotId", "summonerName"] {
        if let Some(s) = p.get(key).and_then(Value::as_str) {
            if !s.trim().is_empty() {
                out.push(s.to_string());
            }
        }
    }
    // Le Riot ID recomposé, pour le cas où un côté sert `riotId` entier et
    // l'autre seulement ses deux moitiés.
    let name = p
        .get("riotIdGameName")
        .and_then(Value::as_str)
        .unwrap_or("");
    let tag = p.get("riotIdTagLine").and_then(Value::as_str).unwrap_or("");
    if !name.trim().is_empty() && !tag.trim().is_empty() {
        out.push(format!("{name}#{tag}"));
    }
    out
}

/// La ligne du membre dans la liste des dix joueurs.
///
/// `activePlayer` dit qui est le joueur local : contrairement à Rocket League,
/// le membre n'a rien à saisir. Faute de correspondance on rend `None` : ne
/// rien émettre vaut mieux qu'attribuer au membre les faits d'un autre.
pub fn find_member<'a>(players: &'a [Value], active: &Value) -> Option<&'a Value> {
    let mine = identities(active);
    if mine.is_empty() {
        return None;
    }
    players
        .iter()
        .find(|p| identities(p).iter().any(|id| mine.contains(id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une partie à deux joueurs nommés, dans la forme servie par le client
    /// actuel : Riot ID d'un côté, nom d'invocateur vide de l'autre.
    fn a_game() -> String {
        serde_json::json!({
            "activePlayer": {
                "currentGold": 8350.75,
                "level": 11,
                "summonerName": "",
                "riotId": "Ouranos#KOE",
                "riotIdGameName": "Ouranos",
                "riotIdTagLine": "KOE"
            },
            "allPlayers": [
                {
                    "championName": "Garen",
                    "level": 9,
                    "team": "CHAOS",
                    "summonerName": "",
                    "riotId": "Adversaire#EUW",
                    "riotIdGameName": "Adversaire",
                    "riotIdTagLine": "EUW",
                    "items": [{"displayName": "Doran's Blade"}],
                    "scores": {"assists": 1, "creepScore": 40, "deaths": 5, "kills": 2, "wardScore": 3.0}
                },
                {
                    "championName": "Ahri",
                    "level": 11,
                    "team": "ORDER",
                    "summonerName": "",
                    "riotId": "Ouranos#KOE",
                    "riotIdGameName": "Ouranos",
                    "riotIdTagLine": "KOE",
                    "items": [{"displayName": "Luden's Companion"}],
                    "scores": {"assists": 9, "creepScore": 142, "deaths": 2, "kills": 7, "wardScore": 8.0}
                }
            ],
            "events": {"Events": [{"EventName": "ChampionKill", "KillerName": "Adversaire#EUW"}]},
            "gameData": {"gameMode": "CLASSIC", "gameTime": 843.4, "mapName": "Map11"}
        })
        .to_string()
    }

    #[test]
    fn le_membre_est_retrouve_dans_la_liste_des_joueurs() {
        let l = parse(&a_game()).expect("une partie lisible");
        assert_eq!(l.champion, "Ahri");
        assert_eq!(l.level, 11);
        assert_eq!(l.kills, 7);
        assert_eq!(l.deaths, 2);
        assert_eq!(l.assists, 9);
        assert_eq!(l.creep_score, 142);
        assert_eq!(l.gold, 8350);
        assert_eq!(l.game_time_seconds, 843);
    }

    #[test]
    fn les_faits_dun_autre_joueur_ne_sont_jamais_pris_pour_ceux_du_membre() {
        // La ligne de l'adversaire vient EN PREMIER dans la liste : prendre la
        // première venue donnerait Garen, 2/5/1.
        let l = parse(&a_game()).unwrap();
        assert_ne!(l.champion, "Garen");
        assert_eq!((l.kills, l.deaths, l.assists), (7, 2, 9));
    }

    #[test]
    fn un_membre_introuvable_nemet_rien() {
        // Mieux vaut le silence qu'attribuer au membre les faits d'un autre.
        let mut v: Value = serde_json::from_str(&a_game()).unwrap();
        v["allPlayers"][1]["riotId"] = "PersonneDeConnu#XXX".into();
        v["allPlayers"][1]["riotIdGameName"] = "PersonneDeConnu".into();
        v["allPlayers"][1]["riotIdTagLine"] = "XXX".into();
        assert!(parse(&v.to_string()).is_none());
    }

    #[test]
    fn lancien_protocole_par_nom_dinvocateur_marche_encore() {
        // Avant les Riot ID, les deux côtés ne portaient que `summonerName`.
        // Un client qui n'aurait pas migré doit rester suivi.
        let v = serde_json::json!({
            "activePlayer": {"currentGold": 120.0, "level": 2, "summonerName": "Riot Tuxedo"},
            "allPlayers": [{
                "championName": "Annie",
                "level": 2,
                "summonerName": "Riot Tuxedo",
                "team": "ORDER",
                "scores": {"assists": 0, "creepScore": 11, "deaths": 0, "kills": 1, "wardScore": 0.0}
            }],
            "gameData": {"gameMode": "CLASSIC", "gameTime": 65.9}
        });
        let l = parse(&v.to_string()).expect("l'ancien protocole reste lisible");
        assert_eq!(l.champion, "Annie");
        assert_eq!(l.creep_score, 11);
        assert_eq!(l.game_time_seconds, 65);
    }

    #[test]
    fn une_reponse_tronquee_ne_fait_pas_paniquer() {
        // L'API se coupe au milieu d'une réponse quand la partie se termine.
        let whole = a_game();
        for cut in [0, 1, 5, 40, 120, 400, whole.len() / 2, whole.len() - 1] {
            assert!(parse(&whole[..cut]).is_none(), "coupée à {cut}");
        }
    }

    #[test]
    fn une_reponse_inattendue_ne_fait_pas_paniquer() {
        for raw in [
            "",
            "   ",
            "null",
            "[]",
            "\"Not Found\"",
            "{}",
            "{\"activePlayer\":null}",
            "{\"activePlayer\":{},\"allPlayers\":[]}",
            "{\"activePlayer\":{\"riotId\":\"A#B\"},\"allPlayers\":{}}",
            // La forme est bonne mais les scores manquent.
            "{\"activePlayer\":{\"riotId\":\"A#B\",\"level\":3,\"currentGold\":1.0},\
              \"allPlayers\":[{\"riotId\":\"A#B\",\"championName\":\"Ahri\"}],\
              \"gameData\":{\"gameTime\":10.0}}",
            // Les scores sont là mais pas le temps de jeu.
            "{\"activePlayer\":{\"riotId\":\"A#B\",\"level\":3,\"currentGold\":1.0},\
              \"allPlayers\":[{\"riotId\":\"A#B\",\"championName\":\"Ahri\",\
              \"scores\":{\"kills\":0,\"deaths\":0,\"assists\":0,\"creepScore\":0}}]}",
        ] {
            assert!(parse(raw).is_none(), "aurait dû être refusé : {raw}");
        }
    }

    #[test]
    fn un_bloc_actif_sans_identite_ne_correspond_a_personne() {
        // Sinon un `summonerName` vide des deux côtés ferait correspondre
        // n'importe quelle ligne, donc la première : un autre joueur.
        let v = serde_json::json!({
            "activePlayer": {"currentGold": 0.0, "level": 1, "summonerName": ""},
            "allPlayers": [{
                "championName": "Garen",
                "level": 1,
                "summonerName": "",
                "scores": {"assists": 0, "creepScore": 0, "deaths": 0, "kills": 0}
            }],
            "gameData": {"gameTime": 3.0}
        });
        assert!(parse(&v.to_string()).is_none());
    }

    #[test]
    fn le_temps_de_jeu_est_tronque_pas_arrondi() {
        // À 842,9 secondes la partie dure 842 secondes, pas encore 843.
        let v: Value = serde_json::from_str(&a_game()).unwrap();
        let mut v = v;
        v["gameData"]["gameTime"] = serde_json::json!(842.9);
        assert_eq!(parse(&v.to_string()).unwrap().game_time_seconds, 842);
    }
}
