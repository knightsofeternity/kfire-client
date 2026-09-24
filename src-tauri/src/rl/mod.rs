//! Rocket League match tracking: the game has no public player API, so the
//! desktop client reads the stats socket the game opens locally and reports
//! nothing but a summary of it.
//!
//! What NEVER leaves this machine, by design: the names of the other players in
//! the match, team-mates and opponents alike. The stream carries them all. The
//! client uses them to compute, then emits nothing but facts about the member.

pub mod config;
pub mod frames;
pub mod gate;
pub mod parser;
pub mod paths;
pub mod socket;

use serde_json::{json, Value};

/// The catalogue slug, checked against the production database on 2026-09-16.
pub const SLUG: &str = "rocket-league";

/// The payload sent for a match, and nothing else.
///
/// Built in ONE single place so that a test can pin down exactly what leaves
/// this machine. The game's stream carries every player's name; all that is
/// left here is numbers.
pub fn payload(s: &parser::Summary, slug: &str, played_at: chrono::DateTime<chrono::Utc>) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("game_slug".into(), slug.into());
    // Omitted, never `null`, when the game sent no playlist: that is the norm
    // for the real protocol, not an exception.
    if let Some(p) = s.playlist {
        o.insert("playlist".into(), p.into());
    }
    o.insert("team_size".into(), s.team_size.into());
    o.insert("player_team".into(), s.player_team.into());
    o.insert("team_blue_score".into(), s.team_blue_score.into());
    o.insert("team_orange_score".into(), s.team_orange_score.into());
    o.insert("result".into(), s.result.clone().into());
    o.insert("goals".into(), s.goals.into());
    o.insert("assists".into(), s.assists.into());
    o.insert("saves".into(), s.saves.into());
    o.insert("shots".into(), s.shots.into());
    o.insert("score".into(), s.score.into());
    o.insert("demos".into(), s.demos.into());
    o.insert("mvp".into(), s.mvp.into());
    o.insert("duration_seconds".into(), s.duration_seconds.into());
    o.insert("played_at".into(), played_at.to_rfc3339().into());
    Value::Object(o)
}

/// The end of a match, broadcast so the card disappears at once.
///
/// Without it the portal only learns a match is over when the server's timer
/// expires the state, so a frozen score sits on the whole guild's live page
/// for up to fifteen seconds after the match ended. Sending `None` on the
/// channel does NOT do this: the consumer skips `None`, so it transmits
/// nothing at all.
///
/// The server understands this shape generically, for every game.
pub fn ended_payload(slug: &str) -> Value {
    json!({ "game_slug": slug, "ended": true })
}

/// The state broadcast during the match, and nothing else.
///
/// Never queued, never replayed, never written. It names nobody: the fact that
/// it is not stored would not make it acceptable to broadcast an opponent's
/// player name to the whole guild.
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

/// The servers a match must be addressed to.
///
/// Taken word for word from `hs::targets`, for the same reason: a server whose
/// catalogue does not know the game would answer `unknown_game`, and a server
/// the member has taken offline has no session to drain its queue.
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

/// True for as long as no watcher thread should be running. A single flag is
/// enough: the game runs at most once, and the scanner never reports two starts
/// without a stop in between.
static STOP: AtomicBool = AtomicBool::new(true);

/// How often, at most, the live state is published.
const LIVE_EVERY: std::time::Duration = std::time::Duration::from_millis(500);

/// The install directory: the member's manual setting wins, otherwise the
/// running process tells us, otherwise a common location, and the answer is
/// remembered.
pub fn installed_dir(db: &crate::db::Db) -> Option<std::path::PathBuf> {
    if let Some(manual) = db.get_setting("rl_install_dir") {
        return Some(std::path::PathBuf::from(manual));
    }
    let found = paths::running_install_dir().or_else(paths::common_install_dir)?;
    db.set_setting("rl_install_dir", &found.to_string_lossy());
    Some(found)
}

/// Asks the watcher thread to stop.
pub fn stop_watching() {
    STOP.store(true, Ordering::SeqCst);
}

/// Whether a watcher thread is running right now.
pub fn is_watching() -> bool {
    !STOP.load(Ordering::SeqCst)
}

/// Whether that thread is actually connected to the game's socket.
pub fn is_socket_connected() -> bool {
    socket::connected()
}

/// How many messages from the game have been decoded.
pub fn decoded_messages() -> u64 {
    socket::decoded()
}

/// Starts following the socket, unless the member has not turned tracking on
/// or has not declared his player name.
pub fn start_watching(
    db: Arc<crate::db::Db>,
    notify: Arc<tokio::sync::Notify>,
    live: tokio::sync::watch::Sender<Option<String>>,
) {
    if db.get_setting("rl_enabled").as_deref() != Some("1") {
        log::info!("rl: tracking is off in the settings, not watching");
        return;
    }
    let name = db.get_setting("rl_player_name").unwrap_or_default();
    if name.trim().is_empty() {
        log::info!("rl: no player name set, not watching");
        return;
    }
    let Some(install) = installed_dir(&db) else {
        log::info!("rl: install directory not found, not watching");
        return;
    };
    let config_path = paths::config_path_in(&install.to_string_lossy());
    let port = std::fs::read_to_string(&config_path)
        .map(|c| config::port_in(&c))
        .unwrap_or(config::DEFAULT_PORT);

    // swap returns the PREVIOUS value: true means we were stopped, so we are
    // the ones starting.
    if !STOP.swap(false, Ordering::SeqCst) {
        return;
    }

    // The player name itself is never logged: it is the member's own handle,
    // and it has no business being in a log file. Only the fact that it is set,
    // and its length.
    let name_or_placeholder = format!("name set ({} chars)", name.trim().chars().count());
    log::info!(
        "rl: tracking {} on port {} (config: {})",
        name_or_placeholder,
        port,
        config_path
    );

    std::thread::spawn(move || {
        let mut current: Option<parser::Match> = None;
        let mut started_at = std::time::Instant::now();
        let mut last_live = std::time::Instant::now() - LIVE_EVERY;
        let mut last_guid: Option<String> = None;
        // Keeps the end-of-match frames from building a second, ghost match.
        // See `gate.rs`.
        let mut gate = gate::MatchGate::new();

        socket::follow(port, &STOP, |ev| match ev {
            socket::Event::Open => {
                gate.opened();
                if current.is_none() {
                    // Read for every match, not once per game launch: a player
                    // name fixed mid-session applies to the NEXT match, not to
                    // the next launch.
                    let name = db.get_setting("rl_player_name").unwrap_or_default();
                    current = Some(parser::Match::new(&name));
                    started_at = std::time::Instant::now();
                }
            }
            socket::Event::State(data) => {
                if current.is_none() && !gate.may_start() {
                    return;
                }
                let m = current.get_or_insert_with(|| {
                    started_at = std::time::Instant::now();
                    let name = db.get_setting("rl_player_name").unwrap_or_default();
                    parser::Match::new(&name)
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
                gate.closed();
                // Tell the portal the match is over instead of letting its
                // timer work it out: a frozen score would otherwise stay on
                // the guild's live page for seconds after the final whistle.
                let env = serde_json::json!({
                    "type": "live_match",
                    "ts": chrono::Utc::now().to_rfc3339(),
                    "payload": ended_payload(SLUG),
                });
                let _ = live.send(Some(env.to_string()));
                let Some(m) = current.take() else { return };

                // The same GUID twice means the game sent again the end of a
                // match that has already been handled.
                if m.guid().is_some() && m.guid() == last_guid {
                    return;
                }
                last_guid = m.guid();

                let seconds = started_at.elapsed().as_secs() as i64;
                let names = m.names_seen();
                let summary = match m.finish(seconds) {
                    Ok(summary) => summary,
                    // The only case where the configured name can really be to
                    // blame: it matches nobody on the scoresheet. We write the
                    // names seen to the LOCAL log, which does not leave this
                    // machine, so the member can fix it himself. Without that
                    // he would see nothing but silence.
                    Err(parser::Refusal::MemberNotFound) => {
                        log::info!(
                            "rl: a match went unreported, the summary was incomplete \
                             (guid={:?}, {}s): {}",
                            last_guid,
                            seconds,
                            parser::Refusal::MemberNotFound
                        );
                        if !names.is_empty() {
                            log::info!(
                                "rl: the configured player name matched nobody. \
                                 Names in that match: {}. Set yours in the settings.",
                                names.join(", ")
                            );
                            // Written where the member actually looks. A log
                            // line is no use: nobody opens a log file. The
                            // settings screen, they do.
                            db.set_setting("rl_last_mismatch", &names.join(", "));
                        }
                        return;
                    }
                    // Any other reason: the member WAS found, so his player
                    // name is the right one. Showing the player-name warning
                    // here would be a lie. We log the real reason, and we clear
                    // a warning that might date from an earlier match: it no
                    // longer concerns him.
                    Err(other) => {
                        log::info!(
                            "rl: a match went unreported, the summary was incomplete \
                             (guid={:?}, {}s): {}",
                            last_guid,
                            seconds,
                            other
                        );
                        db.set_setting("rl_last_mismatch", "");
                        return;
                    }
                };

                // The summary was produced, so the configured player name did
                // find its player: the warning has no reason to stand. We clear
                // it HERE, before any question of who the recipients are.
                // Clearing it further down would leave it on screen whenever no
                // server is eligible, and the member would watch the screen go
                // on accusing him when he had done exactly what was asked.
                db.set_setting("rl_last_mismatch", "");

                let played_at = chrono::Utc::now();
                // Remembered before any question of recipients: it is what the
                // member played, shown in the client even if no server gets it.
                db.remember_last_match(SLUG, &payload(&summary, SLUG, played_at));
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
                    "rl: queued a {} on playlist {:?}",
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

    #[test]
    fn la_fin_dun_match_ne_porte_que_le_jeu_et_le_drapeau() {
        // This message goes out to every member of the guild: it must carry
        // nothing else, and above all no leftover of the match that just ended.
        let p = ended_payload(SLUG);
        let o = p.as_object().expect("an object");
        let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, vec!["ended", "game_slug"]);
        assert_eq!(o["game_slug"], SLUG);
        assert_eq!(o["ended"], true);
    }
    use crate::rl::parser::Summary;

    fn a_summary() -> Summary {
        Summary {
            playlist: Some(13),
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
    fn the_payload_carries_exactly_the_sixteen_allowed_fields_when_a_playlist_is_present() {
        // Pins down the whole set when the summary carries a playlist (the day
        // Psyonix adds one, or in the tests that set it).
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
    fn the_payload_omits_playlist_entirely_when_the_game_never_sent_one() {
        // This is the norm, not the exception: the real protocol has no
        // Playlist field. The key must be ABSENT, never sent as `null`: it is
        // the same set as the test above, minus "playlist", not an "at most"
        // that would let anything else through.
        let mut s = a_summary();
        s.playlist = None;
        let v = payload(&s, SLUG, at());
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
                "result",
                "saves",
                "score",
                "shots",
                "team_blue_score",
                "team_orange_score",
                "team_size"
            ]
        );
        assert!(!v.to_string().contains("playlist"));
    }

    #[test]
    fn the_payload_can_never_carry_a_name() {
        // The game's stream carries the names of ALL the players in the match.
        // None of them may ever appear here.
        let raw = payload(&a_summary(), SLUG, at()).to_string();
        for forbidden in ["Name", "Bushido", "Players", "PlayerName"] {
            assert!(!raw.contains(forbidden), "{forbidden} leaked into {raw}");
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
        assert_eq!(parsed.offset().local_minus_utc(), 0, "must go out in UTC");
    }

    #[test]
    fn the_slug_is_the_one_the_server_knows() {
        // Checked against the production database on 2026-09-16.
        assert_eq!(SLUG, "rocket-league");
    }
}
