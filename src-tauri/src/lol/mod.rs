//! Suivi de League of Legends : le jeu ouvre lui-même une API sur la machine du
//! joueur pendant une partie, et le client de bureau la lit.
//!
//! Ce qui NE quitte JAMAIS cette machine, par conception : `allgamedata` nomme
//! les dix participants de la partie, avec leurs Riot ID, leurs équipes, leurs
//! objets et l'historique des évènements. Rien de tout cela ne sort. Le nom du
//! membre lui-même ne sort pas non plus : il sert uniquement, localement, à
//! retrouver sa ligne dans la liste des joueurs (voir `parser::find_member`).
//! Seuls des faits sur le membre sortent, et la liste exacte est épinglée par
//! un test.

pub mod parser;

use serde_json::{json, Value};

/// Le slug du catalogue, celui sous lequel le serveur connaît ce jeu.
pub const SLUG: &str = "league-of-legends";

/// L'état diffusé pendant la partie, et rien d'autre.
///
/// Jamais mis en file, jamais rejoué, jamais écrit. Il ne nomme personne : la
/// réponse dont il est tiré porte le Riot ID des dix joueurs, et le fait que ce
/// message ne soit pas stocké ne rendrait pas acceptable de les diffuser à
/// toute la guilde. La validation du serveur refuse de toute façon tout champ
/// non prévu : une clé de plus ici, et plus rien ne passe.
pub fn live_payload(l: &parser::Live, slug: &str) -> Value {
    json!({
        "game_slug": slug,
        "champion": l.champion,
        "level": l.level,
        "kills": l.kills,
        "deaths": l.deaths,
        "assists": l.assists,
        "creep_score": l.creep_score,
        "gold": l.gold,
        "game_time_seconds": l.game_time_seconds,
    })
}

/// La fin d'une partie, diffusée pour que la carte s'efface tout de suite.
///
/// Sans elle le portail n'apprend la fin qu'à l'expiration de son propre
/// minuteur, et un score figé resterait affiché sur la page en direct de la
/// guilde. Le serveur comprend cette forme génériquement, pour tous les jeux.
pub fn ended_payload(slug: &str) -> Value {
    json!({ "game_slug": slug, "ended": true })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_live() -> parser::Live {
        parser::Live {
            champion: "Ahri".into(),
            level: 11,
            kills: 7,
            deaths: 2,
            assists: 9,
            creep_score: 142,
            gold: 8350,
            game_time_seconds: 843,
        }
    }

    #[test]
    fn le_direct_ne_porte_exactement_que_les_neuf_champs_autorises() {
        // LE test de ce module : il épingle la liste exacte des clés qui
        // quittent cette machine vers tous les membres de la guilde. La réponse
        // dont elles sont tirées nomme les dix joueurs de la partie, porte
        // leurs équipes, leurs objets et l'historique des évènements. Si cette
        // liste s'allonge un jour, elle s'allonge exprès. La validation du
        // serveur refuse de toute façon tout champ non prévu.
        let v = live_payload(&a_live(), SLUG);
        let o = v.as_object().expect("un objet");
        let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "assists",
                "champion",
                "creep_score",
                "deaths",
                "game_slug",
                "game_time_seconds",
                "gold",
                "kills",
                "level",
            ]
        );
        assert_eq!(o["game_slug"], SLUG);
        assert_eq!(o["champion"], "Ahri");
        assert_eq!(o["level"], 11);
        assert_eq!(o["kills"], 7);
        assert_eq!(o["deaths"], 2);
        assert_eq!(o["assists"], 9);
        assert_eq!(o["creep_score"], 142);
        assert_eq!(o["gold"], 8350);
        assert_eq!(o["game_time_seconds"], 843);
    }

    #[test]
    fn le_direct_ne_porte_le_nom_de_personne() {
        // Ni celui du membre, ni celui des neuf autres. Le nom du membre sert
        // uniquement, localement, à retrouver sa ligne.
        let raw = live_payload(&a_live(), SLUG).to_string();
        for forbidden in [
            "riotId",
            "summonerName",
            "riotIdGameName",
            "riotIdTagLine",
            "allPlayers",
            "team",
            "items",
            "events",
            "#",
        ] {
            assert!(!raw.contains(forbidden), "{forbidden} a fuité dans {raw}");
        }
    }

    #[test]
    fn le_direct_construit_depuis_une_vraie_reponse_ne_porte_rien_de_plus() {
        // Bout en bout : du JSON du jeu, qui nomme tout le monde, jusqu'à ce
        // qui part. Aucun nom d'adversaire ne doit survivre au trajet.
        let raw = serde_json::json!({
            "activePlayer": {"currentGold": 500.5, "level": 6, "riotId": "Ouranos#KOE"},
            "allPlayers": [
                {
                    "championName": "Garen",
                    "level": 7,
                    "team": "CHAOS",
                    "riotId": "Adversaire#EUW",
                    "scores": {"assists": 0, "creepScore": 55, "deaths": 1, "kills": 4}
                },
                {
                    "championName": "Ahri",
                    "level": 6,
                    "team": "ORDER",
                    "riotId": "Ouranos#KOE",
                    "scores": {"assists": 2, "creepScore": 61, "deaths": 1, "kills": 3}
                }
            ],
            "events": {"Events": [{"EventName": "ChampionKill", "KillerName": "Adversaire#EUW"}]},
            "gameData": {"gameMode": "CLASSIC", "gameTime": 300.0}
        })
        .to_string();
        let l = parser::parse(&raw).expect("une partie lisible");
        let sent = live_payload(&l, SLUG).to_string();
        for forbidden in [
            "Ouranos",
            "Adversaire",
            "KOE",
            "EUW",
            "Garen",
            "CHAOS",
            "ORDER",
        ] {
            assert!(!sent.contains(forbidden), "{forbidden} a fuité dans {sent}");
        }
        assert!(sent.contains("Ahri"), "le champion du membre doit partir");
    }

    #[test]
    fn la_fin_dune_partie_ne_porte_que_le_jeu_et_le_drapeau() {
        let v = ended_payload(SLUG);
        let o = v.as_object().expect("un objet");
        let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, vec!["ended", "game_slug"]);
        assert_eq!(o["game_slug"], SLUG);
        assert_eq!(o["ended"], true);
    }

    #[test]
    fn le_slug_est_celui_que_le_serveur_connait() {
        assert_eq!(SLUG, "league-of-legends");
    }
}
