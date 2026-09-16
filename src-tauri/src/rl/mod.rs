//! Suivi des matchs Rocket League : le jeu n'a pas d'API joueur publique, donc
//! le client de bureau lit la socket de statistiques qu'il ouvre en local et
//! n'en rapporte qu'un résumé.
//!
//! Ce qui NE quitte JAMAIS cette machine, par conception : le nom des autres
//! joueurs du match, coéquipiers comme adversaires. Le flux les porte tous. Le
//! client s'en sert pour calculer, puis n'émet que des faits sur le membre.

pub mod config;
pub mod frames;
pub mod parser;
pub mod paths;
pub mod socket;

use serde_json::{json, Value};

/// Le slug du catalogue, vérifié en base de production le 2026-09-16.
pub const SLUG: &str = "rocket-league";

/// La charge utile envoyée pour un match, et rien d'autre.
///
/// Construite en UN seul endroit pour qu'un test puisse épingler exactement ce
/// qui quitte cette machine. Le flux du jeu porte le nom de tous les joueurs ;
/// il n'en reste ici que des nombres.
pub fn payload(s: &parser::Summary, slug: &str, played_at: chrono::DateTime<chrono::Utc>) -> Value {
    json!({
        "game_slug": slug,
        "playlist": s.playlist,
        "team_size": s.team_size,
        "player_team": s.player_team,
        "team_blue_score": s.team_blue_score,
        "team_orange_score": s.team_orange_score,
        "result": s.result,
        "goals": s.goals,
        "assists": s.assists,
        "saves": s.saves,
        "shots": s.shots,
        "score": s.score,
        "demos": s.demos,
        "mvp": s.mvp,
        "duration_seconds": s.duration_seconds,
        "played_at": played_at.to_rfc3339(),
    })
}

/// L'état diffusé pendant le match, et rien d'autre.
///
/// Jamais mis en file, jamais rejoué, jamais écrit. Il ne nomme personne : le
/// fait qu'il ne soit pas stocké ne rendrait pas acceptable de diffuser le
/// pseudonyme d'un adversaire à toute la guilde.
pub fn live_payload(l: &parser::Live, slug: &str) -> Value {
    json!({
        "game_slug": slug,
        "team_blue_score": l.team_blue_score,
        "team_orange_score": l.team_orange_score,
        "seconds_remaining": l.seconds_remaining,
        "overtime": l.overtime,
        "goals": l.goals,
        "assists": l.assists,
        "saves": l.saves,
        "shots": l.shots,
        "score": l.score,
        "demos": l.demos,
    })
}

/// Les serveurs auxquels un match doit être adressé.
///
/// Reprise mot pour mot de `hs::targets`, pour la même raison : un serveur dont
/// le catalogue ignore le jeu répondrait `unknown_game`, et un serveur que le
/// membre a mis hors ligne n'a aucune session pour vider sa file.
fn targets(
    servers: &[(String, String)],
    catalog: &[(String, String)],
    global: &str,
    slug: &str,
) -> Vec<String> {
    servers
        .iter()
        .filter(|(id, over)| {
            crate::status::effective_status(global, over) != "offline"
                && catalog.iter().any(|(sid, s)| sid == id && s == slug)
        })
        .map(|(id, _)| id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rl::parser::Summary;

    fn a_summary() -> Summary {
        Summary {
            playlist: 13,
            team_size: 3,
            player_team: 0,
            team_blue_score: 4,
            team_orange_score: 2,
            result: "win".into(),
            goals: 2,
            assists: 1,
            saves: 3,
            shots: 5,
            score: 640,
            demos: 1,
            mvp: true,
            duration_seconds: 330,
        }
    }

    fn at() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-09-16T17:44:59Z")
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    #[test]
    fn the_payload_carries_exactly_the_sixteen_allowed_fields() {
        let v = payload(&a_summary(), SLUG, at());
        let o = v.as_object().unwrap();
        let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "assists",
                "demos",
                "duration_seconds",
                "game_slug",
                "goals",
                "mvp",
                "played_at",
                "player_team",
                "playlist",
                "result",
                "saves",
                "score",
                "shots",
                "team_blue_score",
                "team_orange_score",
                "team_size"
            ]
        );
    }

    #[test]
    fn the_payload_can_never_carry_a_name() {
        // Le flux du jeu porte le nom de TOUS les joueurs du match. Aucun ne
        // doit apparaître ici, jamais.
        let raw = payload(&a_summary(), SLUG, at()).to_string();
        for forbidden in ["Name", "Bushido", "Players", "PlayerName"] {
            assert!(!raw.contains(forbidden), "{forbidden} a fuité dans {raw}");
        }
    }

    #[test]
    fn the_live_payload_carries_exactly_the_eleven_allowed_fields() {
        let live = crate::rl::parser::Live {
            team_blue_score: 2,
            team_orange_score: 1,
            seconds_remaining: 143,
            overtime: false,
            goals: 1,
            assists: 0,
            saves: 2,
            shots: 3,
            score: 310,
            demos: 0,
        };
        let v = live_payload(&live, SLUG);
        let o = v.as_object().unwrap();
        let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "assists",
                "demos",
                "game_slug",
                "goals",
                "overtime",
                "saves",
                "score",
                "seconds_remaining",
                "shots",
                "team_blue_score",
                "team_orange_score"
            ]
        );
    }

    #[test]
    fn played_at_is_sent_as_utc_rfc3339() {
        let v = payload(&a_summary(), SLUG, at());
        let s = v["played_at"].as_str().unwrap();
        let parsed = chrono::DateTime::parse_from_rfc3339(s).expect("RFC 3339");
        assert_eq!(parsed.offset().local_minus_utc(), 0, "doit partir en UTC");
    }

    #[test]
    fn the_slug_is_the_one_the_server_knows() {
        // Vérifié en base de production le 2026-09-16.
        assert_eq!(SLUG, "rocket-league");
    }
}
