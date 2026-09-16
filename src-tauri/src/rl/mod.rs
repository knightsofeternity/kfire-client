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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Vrai tant qu'aucun fil de suivi ne doit tourner. Un seul drapeau suffit : le
/// jeu tourne au plus une fois, et le scanner ne signale jamais deux démarrages
/// sans un arrêt entre les deux.
static STOP: AtomicBool = AtomicBool::new(true);

/// À quelle cadence au plus l'état du direct est publié.
const LIVE_EVERY: std::time::Duration = std::time::Duration::from_millis(500);

/// Le dossier d'installation : le réglage manuel du membre gagne, sinon le
/// process en cours nous le dit, sinon un emplacement courant, et la réponse
/// est retenue.
pub fn installed_dir(db: &crate::db::Db) -> Option<std::path::PathBuf> {
    if let Some(manual) = db.get_setting("rl_install_dir") {
        return Some(std::path::PathBuf::from(manual));
    }
    let found = paths::running_install_dir().or_else(paths::common_install_dir)?;
    db.set_setting("rl_install_dir", &found.to_string_lossy());
    Some(found)
}

/// Demande au fil de suivi de s'arrêter.
pub fn stop_watching() {
    STOP.store(true, Ordering::SeqCst);
}

/// Commence à suivre la socket, sauf si le membre n'a pas activé le suivi ou
/// n'a pas déclaré son pseudo.
pub fn start_watching(
    db: Arc<crate::db::Db>,
    notify: Arc<tokio::sync::Notify>,
    live: tokio::sync::watch::Sender<Option<String>>,
) {
    if db.get_setting("rl_enabled").as_deref() != Some("1") {
        return;
    }
    let Some(member) = db
        .get_setting("rl_player_name")
        .filter(|n| !n.trim().is_empty())
    else {
        log::info!("rl: no player name set, not watching");
        return;
    };
    let Some(install) = installed_dir(&db) else {
        log::info!("rl: install directory not found, not watching");
        return;
    };
    let port = std::fs::read_to_string(paths::config_path_in(&install.to_string_lossy()))
        .map(|c| config::port_in(&c))
        .unwrap_or(config::DEFAULT_PORT);

    // swap rend la valeur PRÉCÉDENTE : true veut dire qu'on était arrêté, donc
    // on démarre.
    if !STOP.swap(false, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(move || {
        let mut current: Option<parser::Match> = None;
        let mut started_at = std::time::Instant::now();
        let mut last_live = std::time::Instant::now() - LIVE_EVERY;
        let mut last_guid: Option<String> = None;

        socket::follow(port, &STOP, |ev| match ev {
            socket::Event::Open => {
                if current.is_none() {
                    current = Some(parser::Match::new(&member));
                    started_at = std::time::Instant::now();
                }
            }
            socket::Event::State(data) => {
                let m = current.get_or_insert_with(|| {
                    started_at = std::time::Instant::now();
                    parser::Match::new(&member)
                });
                m.observe(&data);
                if last_live.elapsed() >= LIVE_EVERY {
                    last_live = std::time::Instant::now();
                    if let Some(l) = m.live() {
                        let env = serde_json::json!({
                            "type": "live_match",
                            "ts": chrono::Utc::now().to_rfc3339(),
                            "payload": live_payload(&l, SLUG),
                        });
                        let _ = live.send(Some(env.to_string()));
                    }
                }
            }
            socket::Event::Close => {
                let _ = live.send(None);
                let Some(m) = current.take() else { return };

                // Le même GUID deux fois veut dire que le jeu a renvoyé la fin
                // d'un match déjà traité.
                if m.guid().is_some() && m.guid() == last_guid {
                    return;
                }
                last_guid = m.guid();

                let seconds = started_at.elapsed().as_secs() as i64;
                let names = m.names_seen();
                let Some(summary) = m.finish(seconds) else {
                    // Le contrat de complétude : mieux vaut un match manquant
                    // qu'un match faux. Dire pourquoi, sinon personne ne saura.
                    log::info!(
                        "rl: a match went unreported, the summary was incomplete \
                         (guid={:?}, {}s)",
                        last_guid,
                        seconds
                    );
                    // Le cas de loin le plus probable : le pseudo réglé ne
                    // correspond à personne. On écrit les noms vus dans le
                    // journal LOCAL, qui ne quitte pas cette machine, pour que
                    // le membre se corrige tout seul. Sans cela il ne verrait
                    // qu'un silence.
                    if !names.is_empty() {
                        log::info!(
                            "rl: the configured player name matched nobody. \
                             Names in that match: {}. Set yours in the settings.",
                            names.join(", ")
                        );
                    }
                    return;
                };

                let played_at = chrono::Utc::now();
                let servers: Vec<(String, String)> = db
                    .list_servers()
                    .into_iter()
                    .map(|s| (s.id, s.status_override))
                    .collect();
                let catalog: Vec<(String, String)> = db
                    .load_games()
                    .into_iter()
                    .map(|(id, g)| (id, g.slug))
                    .collect();
                let global = db.get_setting("global_status").unwrap_or_default();
                let ids = targets(&servers, &catalog, &global, SLUG);
                if ids.is_empty() {
                    log::info!(
                        "rl: a {} went unreported, no eligible server",
                        summary.result
                    );
                    return;
                }
                for id in &ids {
                    let p = payload(&summary, SLUG, played_at);
                    db.queue_event(
                        id,
                        "match_result",
                        SLUG,
                        &played_at.to_rfc3339(),
                        Some(&p.to_string()),
                    );
                }
                notify.notify_one();
                log::info!(
                    "rl: queued a {} on playlist {}",
                    summary.result,
                    summary.playlist
                );
            }
        });
        STOP.store(true, Ordering::SeqCst);
    });
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
