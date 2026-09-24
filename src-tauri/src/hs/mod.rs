//! Hearthstone match tracking: the game has no player API, so the desktop
//! client reads the game's own logs and reports a summary.
//!
//! What NEVER leaves this machine, by design: the opponent's name and every
//! card played. Both are in the log. The summary carries facts about the member
//! only.

pub mod config;
pub mod hdt;
pub mod parser;
pub mod paths;
pub mod watcher;

use serde_json::{json, Value};

/// The catalogue slug, the one the server knows this game by.
pub const SLUG: &str = "hearthstone";

/// The instant a local wall-clock time really happened.
///
/// The game's log writes LOCAL time and no zone. Sending it as if it were UTC
/// would place a French member's match two hours in the FUTURE, and the server
/// refuses anything more than five minutes ahead: every match would have been
/// silently rejected.
///
/// Around a daylight-saving change a local time can be ambiguous, happening
/// twice, or skipped entirely. The earliest reading is taken, and a skipped
/// time falls back to reading it as UTC, because a match is worth recording an
/// hour off rather than not at all.
pub fn to_utc(local: chrono::NaiveDateTime) -> chrono::DateTime<chrono::Utc> {
    use chrono::TimeZone;
    chrono::Local
        .from_local_datetime(&local)
        .earliest()
        .map(|t| t.with_timezone(&chrono::Utc))
        .unwrap_or_else(|| local.and_utc())
}

/// The payload sent for one match, and nothing else.
///
/// Built in one place so a test can pin down exactly which fields leave this
/// machine. Absent fields are omitted, never sent as null: the server treats
/// absent as unreadable, which is what it means.
pub fn payload(
    m: &parser::Match,
    slug: &str,
    played_at: chrono::NaiveDateTime,
) -> serde_json::Value {
    let mut o = serde_json::Map::new();
    o.insert("game_slug".into(), slug.into());
    o.insert("mode".into(), m.mode.clone().into());
    o.insert("result".into(), m.result.clone().into());
    o.insert("played_at".into(), to_utc(played_at).to_rfc3339().into());
    if let Some(t) = m.turns {
        o.insert("turns".into(), t.into());
    }
    if let Some(p) = m.placement {
        o.insert("placement".into(), p.into());
    }
    if let Some(h) = &m.hero_card_id {
        o.insert("hero_card_id".into(), h.clone().into());
    }
    serde_json::Value::Object(o)
}

/// Adds HDT's rating to a match payload, when one was found. Two integers and
/// nothing else: the rest of HDT's record never leaves the machine.
pub fn with_rating(p: &mut Value, rating: Option<(i64, i64)>) {
    if let (Some((before, after)), Some(o)) = (rating, p.as_object_mut()) {
        o.insert("rating".into(), before.into());
        o.insert("rating_after".into(), after.into());
    }
}

/// The state broadcast while a match is played, and nothing else.
///
/// Never queued, never replayed, never written down. The log it comes from
/// holds the member's BattleTag, the opponent's name and every card played;
/// none of that has any business being sent to the whole guild, and the fact
/// that this message is not stored would not make it acceptable.
pub fn live_payload(l: &parser::Live, slug: &str) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("game_slug".into(), slug.into());
    o.insert("mode".into(), l.mode.clone().into());
    o.insert("turn".into(), l.turn.into());
    // Omitted, never null: there is no ranking in constructed, and the server
    // refuses a constructed match that carries one.
    if let Some(p) = l.placement {
        o.insert("placement".into(), p.into());
    }
    Value::Object(o)
}

/// The end of a match, broadcast so the card disappears at once.
///
/// Without it the portal only learns a match is over when its own timer expires
/// the state, so a finished match would sit on the guild's live page for
/// seconds after the last turn. Sending `None` on the channel does NOT do this:
/// the consumer skips `None`, so it transmits nothing at all.
///
/// The server understands this shape generically, for every game.
pub fn ended_payload(slug: &str) -> Value {
    json!({ "game_slug": slug, "ended": true })
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// How often at most the live state is published.
const LIVE_EVERY: std::time::Duration = std::time::Duration::from_millis(500);

/// Set while no watcher thread should be running. A single flag is enough: the
/// game runs at most once, and the scanner never reports two starts without a
/// stop between them.
static STOP: AtomicBool = AtomicBool::new(true);

/// Whether the member asked for the rating to be read from HDT.
fn hdt_enabled(db: &crate::db::Db) -> bool {
    db.get_setting("hs_hdt_rating").as_deref() == Some("1")
}

/// Remembers a match locally, for the member's window. `rating_missing` is
/// LOCAL only: it says the rating was expected and not found, and is never
/// sent anywhere.
fn remember(db: &crate::db::Db, p: &Value, rating_missing: bool) {
    let mut remembered = p.clone();
    if rating_missing {
        if let Some(o) = remembered.as_object_mut() {
            o.insert("rating_missing".into(), true.into());
        }
    }
    db.remember_last_match(SLUG, &remembered);
}

/// The timestamp a match is queued under, which also finds its rows again.
fn queued_ts(played_at: chrono::NaiveDateTime) -> String {
    to_utc(played_at).to_rfc3339()
}

/// How long past the end of the wait a held match stays held, so the thread
/// waiting for the rating always releases it before the hold runs out.
const HOLD_MARGIN: std::time::Duration = std::time::Duration::from_secs(5);

/// Queues `p` for every eligible server, on disk, WITHOUT waking the sender,
/// and returns the servers it was queued for. With `hold_until`, no drain
/// sends it before that instant unless it is released first.
fn queue_match(
    db: &crate::db::Db,
    m: &parser::Match,
    played_at: chrono::NaiveDateTime,
    p: &Value,
    hold_until: Option<chrono::DateTime<chrono::Utc>>,
) -> Vec<String> {
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
    let targets = targets(&servers, &catalog, &global, SLUG);
    let (ts, raw) = (queued_ts(played_at), p.to_string());
    for id in &targets {
        match hold_until {
            Some(h) => db.queue_held_event(id, "match_result", SLUG, &ts, &raw, h),
            None => db.queue_event(id, "match_result", SLUG, &ts, Some(&raw)),
        }
    }
    if targets.is_empty() {
        log::info!("hs: a {} went unreported, no eligible server", m.result);
    }
    targets
}

/// Wakes the sender for a match already queued, if it went anywhere.
fn send_queued(notify: &tokio::sync::Notify, m: &parser::Match, targets: &[String]) {
    if targets.is_empty() {
        return;
    }
    notify.notify_one();
    log::info!("hs: queued a {} in {}", m.result, m.mode);
}

/// Records a finished match that expects no rating: remembered locally, queued
/// for every eligible server and sent at once.
fn report(
    db: &crate::db::Db,
    notify: &tokio::sync::Notify,
    m: &parser::Match,
    played_at: chrono::NaiveDateTime,
) {
    let p = payload(m, SLUG, played_at);
    // Remembered before any question of recipients: it is what the member
    // played, shown in the client even if no server gets it.
    remember(db, &p, false);
    let targets = queue_match(db, m, played_at, &p, None);
    send_queued(notify, m, &targets);
}

/// The first half of a match that waits for HDT's rating: remembered and
/// queued at once WITHOUT the rating, so quitting KFIRE during the wait loses
/// nothing, but held back from the queue, so that no drain (the game closing,
/// a reconnection) sends it before the rating can be added. If KFIRE quits
/// during the wait, the hold runs out and the match goes without its rating.
fn queue_before_rating(
    db: &crate::db::Db,
    m: &parser::Match,
    played_at: chrono::NaiveDateTime,
) -> Vec<String> {
    let p = payload(m, SLUG, played_at);
    remember(db, &p, false);
    let hold = chrono::Utc::now()
        + chrono::Duration::from_std(hdt::WAIT_TOTAL + HOLD_MARGIN).unwrap_or_default();
    queue_match(db, m, played_at, &p, Some(hold))
}

/// The rating arrived: added to the rows still waiting (which also releases
/// them), remembered, sent.
fn rating_found(
    db: &crate::db::Db,
    notify: &tokio::sync::Notify,
    m: &parser::Match,
    played_at: chrono::NaiveDateTime,
    targets: &[String],
    rating: (i64, i64),
) {
    let mut p = payload(m, SLUG, played_at);
    with_rating(&mut p, Some(rating));
    let raw = p.to_string();
    let ts = queued_ts(played_at);
    let changed: usize = targets
        .iter()
        .map(|id| db.update_pending_payload(id, "match_result", SLUG, &ts, &raw))
        .sum();
    // The queue drained meanwhile (a reconnection, another event): the match
    // already left without its rating, which is accepted.
    if changed == 0 && !targets.is_empty() {
        log::info!("hs: rating found after the match was sent");
    }
    remember(db, &p, false);
    send_queued(notify, m, targets);
}

/// No rating after the wait: the queued match is released as it is, and only
/// the member's window learns the rating is missing.
fn rating_not_found(
    db: &crate::db::Db,
    notify: &tokio::sync::Notify,
    m: &parser::Match,
    played_at: chrono::NaiveDateTime,
    targets: &[String],
) {
    let p = payload(m, SLUG, played_at);
    let ts = queued_ts(played_at);
    for id in targets {
        db.release_pending(id, "match_result", SLUG, &ts);
    }
    remember(db, &p, true);
    send_queued(notify, m, targets);
}

/// Starts following the log, unless the member has not enabled tracking.
pub fn start_watching(
    db: Arc<crate::db::Db>,
    notify: Arc<tokio::sync::Notify>,
    live: tokio::sync::watch::Sender<Option<String>>,
) {
    if db.get_setting("hs_enabled").as_deref() != Some("1") {
        return;
    }
    let Some(install) = installed_dir(&db) else {
        log::info!("hs: install directory not found, not watching");
        return;
    };
    // swap returns the PREVIOUS value: true means it was stopped, so we start.
    if !STOP.swap(false, Ordering::SeqCst) {
        return; // already running
    }
    std::thread::spawn(move || {
        // Shared by both callbacks, which is why it holds its state behind a
        // RefCell: two closures cannot each borrow the same `mut` variable.
        // Keeping the memory of what was last sent in ONE place is what makes
        // "only when it changed" hold across both the live state and the end
        // of a match.
        let last_sent = std::cell::RefCell::new(None::<String>);
        let broadcast = |payload: serde_json::Value| {
            let raw = payload.to_string();
            if last_sent.borrow().as_deref() == Some(raw.as_str()) {
                return;
            }
            *last_sent.borrow_mut() = Some(raw);
            let env = json!({
                "type": "live_match",
                "ts": chrono::Utc::now().to_rfc3339(),
                "payload": payload,
            });
            let _ = live.send(Some(env.to_string()));
        };
        let mut last_live = std::time::Instant::now() - LIVE_EVERY;

        watcher::follow(
            &install,
            &STOP,
            |m, start| {
                // Tell the portal the match is over instead of letting its
                // timer work it out, so the card goes away at once.
                broadcast(ended_payload(SLUG));
                let played_at = m.played_at(start);
                let wants_rating =
                    hdt_enabled(&db) && m.mode == "battlegrounds" && m.placement.is_some();
                if !wants_rating {
                    report(&db, &notify, &m, played_at);
                    return;
                }
                // On disk at once, so quitting during the wait loses nothing;
                // sent only once the rating is known or given up on.
                let targets = queue_before_rating(&db, &m, played_at);
                // HDT may write its file a few seconds after we saw the end:
                // wait for it in a thread of its own, so the log follower
                // never stalls, and never more than thirty seconds.
                let (db, notify, m) = (db.clone(), notify.clone(), m);
                std::thread::spawn(move || {
                    let placement = m.placement.unwrap_or_default();
                    match hdt::wait_for_rating(placement, to_utc(played_at)) {
                        Ok(r) => rating_found(&db, &notify, &m, played_at, &targets, r),
                        Err(why) => {
                            log::info!("hs: rating not recorded: {why}");
                            rating_not_found(&db, &notify, &m, played_at, &targets);
                        }
                    }
                });
            },
            |state| {
                // Spaced out, and only when something changed: the log is
                // re-read on a timer, so most passes see the very same state.
                if last_live.elapsed() < LIVE_EVERY {
                    return;
                }
                last_live = std::time::Instant::now();
                if let Some(l) = state {
                    broadcast(live_payload(&l, SLUG));
                }
            },
        );
        STOP.store(true, Ordering::SeqCst);
    });
}

/// The servers a match should be queued for.
///
/// Two filters, both matching how a session start is fanned out. A server whose
/// catalogue does not carry the game would answer `unknown_game`, so it is
/// skipped. A server the member has set offline has no session to drain its
/// queue, so enqueuing would pile rows up until they flooded back; and choosing
/// offline is a choice not to be tracked, which applies to a match as much as
/// to a session.
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

/// Asks the watcher thread to finish.
pub fn stop_watching() {
    STOP.store(true, Ordering::SeqCst);
}

/// The install directory: the member's manual setting wins, else the running
/// process tells us, and the answer is remembered.
pub fn installed_dir(db: &crate::db::Db) -> Option<std::path::PathBuf> {
    if let Some(manual) = db.get_setting("hs_install_dir") {
        return Some(std::path::PathBuf::from(manual));
    }
    let found = paths::running_install_dir()?;
    db.set_setting("hs_install_dir", &found.to_string_lossy());
    Some(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    fn a_match() -> parser::Match {
        parser::Match {
            mode: "battlegrounds".into(),
            result: "loss".into(),
            turns: Some(22),
            placement: Some(5),
            hero_card_id: Some("BG28_HERO_400".into()),
            ended_at: chrono::NaiveTime::from_hms_opt(17, 44, 59).unwrap(),
        }
    }

    fn at() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-08-02 17:44:59", "%Y-%m-%d %H:%M:%S").unwrap()
    }

    #[test]
    fn payload_carries_exactly_the_seven_allowed_fields() {
        let v = payload(&a_match(), "hearthstone", at());
        let o = v.as_object().unwrap();
        let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "game_slug",
                "hero_card_id",
                "mode",
                "placement",
                "played_at",
                "result",
                "turns"
            ]
        );
    }

    #[test]
    fn payload_can_never_carry_a_name() {
        // The log holds the member's BattleTag, the opponents' names and every
        // card played. None of it may appear here, ever.
        let raw = payload(&a_match(), "hearthstone", at()).to_string();
        for forbidden in ["PlayerName", "entityName", "Entity=", "#"] {
            assert!(!raw.contains(forbidden), "leaked {forbidden} in {raw}");
        }
    }

    #[test]
    fn absent_fields_are_omitted_not_null() {
        let mut m = a_match();
        m.turns = None;
        m.placement = None;
        m.hero_card_id = None;
        let v = payload(&m, "hearthstone", at());
        assert_eq!(v.as_object().unwrap().len(), 4);
        assert!(v.get("placement").is_none());
        assert!(v.get("turns").is_none());
        assert!(v.get("hero_card_id").is_none());
    }

    #[test]
    fn le_direct_ne_porte_exactement_que_les_quatre_champs_autorises() {
        // THE test of this module: it pins the exact list of keys that leave
        // this machine towards every member of the guild. The log this comes
        // from holds the member's BattleTag, the opponent's name and every card
        // played; if this list ever grows, it grows on purpose.
        let l = parser::Live {
            mode: "battlegrounds".into(),
            turn: 11,
            placement: Some(4),
        };
        let v = live_payload(&l, SLUG);
        let o = v.as_object().unwrap();
        let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, vec!["game_slug", "mode", "placement", "turn"]);
        assert_eq!(o["game_slug"], SLUG);
        assert_eq!(o["mode"], "battlegrounds");
        assert_eq!(o["turn"], 11);
        assert_eq!(o["placement"], 4);
    }

    #[test]
    fn le_direct_en_mode_construit_nemet_aucune_position() {
        // There is no ranking in constructed, and the server refuses a
        // constructed match carrying one.
        let l = parser::Live {
            mode: "constructed".into(),
            turn: 7,
            placement: None,
        };
        let v = live_payload(&l, SLUG);
        let o = v.as_object().unwrap();
        let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, vec!["game_slug", "mode", "turn"]);
        assert!(!v.to_string().contains("placement"));
    }

    #[test]
    fn la_fin_dune_partie_ne_porte_que_le_jeu_et_le_drapeau() {
        let v = ended_payload(SLUG);
        let o = v.as_object().unwrap();
        let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, vec!["ended", "game_slug"]);
        assert_eq!(o["game_slug"], SLUG);
        assert_eq!(o["ended"], true);
    }

    #[test]
    fn a_server_that_does_not_know_the_game_is_skipped() {
        let servers = vec![("a".to_string(), "inherit".to_string()), ("b".into(), "inherit".into())];
        let catalog = vec![("a".to_string(), "hearthstone".to_string()), ("b".into(), "subnautica".into())];
        assert_eq!(targets(&servers, &catalog, "online", "hearthstone"), vec!["a"]);
    }

    #[test]
    fn an_offline_server_is_skipped() {
        // Nothing drains its queue, and offline is a choice not to be tracked.
        let servers = vec![("a".to_string(), "offline".to_string())];
        let catalog = vec![("a".to_string(), "hearthstone".to_string())];
        assert!(targets(&servers, &catalog, "online", "hearthstone").is_empty());
    }

    #[test]
    fn a_global_offline_status_skips_every_server() {
        let servers = vec![("a".to_string(), "inherit".to_string())];
        let catalog = vec![("a".to_string(), "hearthstone".to_string())];
        assert!(targets(&servers, &catalog, "offline", "hearthstone").is_empty());
    }

    #[test]
    fn a_server_overriding_back_to_online_is_kept() {
        let servers = vec![("a".to_string(), "online".to_string())];
        let catalog = vec![("a".to_string(), "hearthstone".to_string())];
        assert_eq!(targets(&servers, &catalog, "offline", "hearthstone"), vec!["a"]);
    }

    #[test]
    fn played_at_is_sent_as_utc_rfc3339() {
        let v = payload(&a_match(), "hearthstone", at());
        let s = v["played_at"].as_str().unwrap();
        let parsed = chrono::DateTime::parse_from_rfc3339(s).expect("RFC 3339");
        assert_eq!(parsed.offset().local_minus_utc(), 0, "must be sent as UTC");
        assert_eq!(parsed.naive_utc(), to_utc(at()).naive_utc());
    }

    #[test]
    fn a_local_time_is_not_mistaken_for_utc() {
        // The log writes local time. Sending it unchanged would put a French
        // member's match two hours in the future, and the server refuses
        // anything more than five minutes ahead.
        use chrono::TimeZone;
        let local = at();
        let expected = chrono::Local
            .from_local_datetime(&local)
            .earliest()
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(to_utc(local), expected);
    }

    #[test]
    fn a_rating_adds_exactly_two_fields() {
        let mut v = payload(&a_match(), "hearthstone", at());
        with_rating(&mut v, Some((5571, 5644)));
        assert_eq!(v["rating"], 5571);
        assert_eq!(v["rating_after"], 5644);
        let mut w = payload(&a_match(), "hearthstone", at());
        with_rating(&mut w, None);
        assert!(w.get("rating").is_none() && w.get("rating_after").is_none());
    }

    fn db_with_a_server() -> (crate::db::Db, String) {
        let db = crate::db::Db::open(std::path::Path::new(":memory:")).unwrap();
        let a = db.add_server("https://a", "r", "A");
        let hs = crate::db::CachedGame {
            slug: SLUG.into(),
            name: "Hearthstone".into(),
            executable_names: vec!["Hearthstone.exe".into()],
        };
        db.replace_games(&a, &[hs]).unwrap();
        (db, a)
    }

    /// The rows the sender would take right now.
    fn queued(db: &crate::db::Db, server: &str) -> Vec<Value> {
        parsed(db.pending_events(server))
    }

    /// Every row, held or not.
    fn queued_or_held(db: &crate::db::Db, server: &str) -> Vec<Value> {
        let later = chrono::Utc::now() + chrono::Duration::days(1);
        parsed(db.pending_events_at(server, later))
    }

    fn parsed(events: Vec<crate::db::PendingEvent>) -> Vec<Value> {
        events
            .iter()
            .map(|e| serde_json::from_str(e.payload.as_deref().unwrap()).unwrap())
            .collect()
    }

    fn was_notified(n: &tokio::sync::Notify) -> bool {
        use futures_util::FutureExt;
        n.notified().now_or_never().is_some()
    }

    #[test]
    fn a_match_waiting_for_its_rating_is_already_on_disk_but_not_sent() {
        // Quitting KFIRE during the wait must not lose the match, and a drain
        // during the wait (the game closed, a reconnection) must not send it.
        let (db, a) = db_with_a_server();
        let targets = queue_before_rating(&db, &a_match(), at());
        assert_eq!(targets, vec![a.clone()]);
        assert!(
            queued(&db, &a).is_empty(),
            "held while the rating is awaited"
        );
        let q = queued_or_held(&db, &a);
        assert_eq!(q.len(), 1);
        assert!(q[0].get("rating").is_none());
        let mem = db.last_match(SLUG).unwrap();
        assert!(mem.get("rating").is_none() && mem.get("rating_missing").is_none());
    }

    #[test]
    fn a_rating_found_in_time_is_added_to_the_queued_match_then_sent() {
        let (db, a) = db_with_a_server();
        let notify = tokio::sync::Notify::new();
        let targets = queue_before_rating(&db, &a_match(), at());
        assert!(!was_notified(&notify));
        rating_found(&db, &notify, &a_match(), at(), &targets, (5571, 5644));
        let q = queued(&db, &a);
        assert_eq!(q.len(), 1);
        assert_eq!(q[0]["rating"], 5571);
        assert_eq!(q[0]["rating_after"], 5644);
        let mem = db.last_match(SLUG).unwrap();
        assert_eq!(mem["rating"], 5571);
        assert!(mem.get("rating_missing").is_none());
        assert!(was_notified(&notify));
    }

    #[test]
    fn a_rating_found_after_the_match_left_touches_nothing() {
        let (db, a) = db_with_a_server();
        let notify = tokio::sync::Notify::new();
        let targets = queue_before_rating(&db, &a_match(), at());
        // Sent meanwhile, or its server unlinked: the rows are gone.
        let later = chrono::Utc::now() + chrono::Duration::days(1);
        for e in db.pending_events_at(&a, later) {
            db.delete_event(e.id);
        }
        rating_found(&db, &notify, &a_match(), at(), &targets, (5571, 5644));
        assert!(db.pending_events(&a).is_empty());
        assert_eq!(db.last_match(SLUG).unwrap()["rating"], 5571);
    }

    #[test]
    fn a_missing_rating_is_flagged_locally_and_never_sent() {
        let (db, a) = db_with_a_server();
        let notify = tokio::sync::Notify::new();
        let targets = queue_before_rating(&db, &a_match(), at());
        rating_not_found(&db, &notify, &a_match(), at(), &targets);
        let q = queued(&db, &a);
        assert_eq!(q.len(), 1);
        assert!(q[0].get("rating_missing").is_none() && q[0].get("rating").is_none());
        assert_eq!(db.last_match(SLUG).unwrap()["rating_missing"], true);
        assert!(was_notified(&notify));
    }

    #[test]
    fn the_hold_outlasts_the_wait_for_the_rating() {
        let (db, a) = db_with_a_server();
        queue_before_rating(&db, &a_match(), at());
        let wait = chrono::Duration::from_std(hdt::WAIT_TOTAL).unwrap();
        let end_of_wait = chrono::Utc::now() + wait;
        assert!(db.pending_events_at(&a, end_of_wait).is_empty());
        let well_after = end_of_wait + chrono::Duration::minutes(1);
        assert_eq!(db.pending_events_at(&a, well_after).len(), 1);
    }

    #[test]
    fn a_match_without_a_rating_wanted_is_queued_and_sent_at_once() {
        let (db, a) = db_with_a_server();
        let notify = tokio::sync::Notify::new();
        report(&db, &notify, &a_match(), at());
        assert_eq!(queued(&db, &a).len(), 1);
        assert!(db.last_match(SLUG).is_some());
        assert!(was_notified(&notify));
    }

    #[test]
    fn a_match_is_never_sent_in_the_future() {
        // The server refuses a time more than five minutes ahead of its own
        // clock, so a match that just ended must land in the past.
        let just_now = chrono::Local::now().naive_local() - chrono::Duration::seconds(30);
        let sent = to_utc(just_now);
        assert!(sent <= chrono::Utc::now(), "sent {sent} is in the future");
    }
}

#[cfg(test)]
mod real_log_payloads {
    use super::*;

    /// Prints the exact payloads the client would queue for a real log, so the
    /// contract with the server can be checked against a running instance.
    /// Ignored by default; the log stays outside this public repository.
    #[test]
    #[ignore = "reads a real log outside the repository"]
    fn payloads_for_a_real_log() {
        let path = std::env::var("HS_REAL_LOG").expect("set HS_REAL_LOG");
        let src = std::fs::read_to_string(path).unwrap();
        let start = paths::session_start("Hearthstone_2026_08_02_17_22_29").unwrap();
        for m in parser::parse_games(src.lines()) {
            println!("{}", payload(&m, "hearthstone", m.played_at(start)));
        }
    }
}
