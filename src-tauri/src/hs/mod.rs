//! Hearthstone match tracking: the game has no player API, so the desktop
//! client reads the game's own logs and reports a summary.
//!
//! What NEVER leaves this machine, by design: the opponent's name and every
//! card played. Both are in the log. The summary carries facts about the member
//! only.

pub mod config;
pub mod parser;
pub mod paths;
pub mod watcher;

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
    o.insert("played_at".into(), played_at.and_utc().to_rfc3339().into());
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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Set while no watcher thread should be running. A single flag is enough: the
/// game runs at most once, and the scanner never reports two starts without a
/// stop between them.
static STOP: AtomicBool = AtomicBool::new(true);

/// Starts following the log, unless the member has not enabled tracking.
pub fn start_watching(db: Arc<crate::db::Db>, notify: Arc<tokio::sync::Notify>) {
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
        watcher::follow(&install, &STOP, |m, start| {
            let played_at = m.played_at(start);
            // Every linked server that knows the game gets the match, exactly
            // as a session start does.
            for server in db.list_servers() {
                let p = payload(&m, "hearthstone", played_at);
                db.queue_event(
                    &server.id,
                    "match_result",
                    "hearthstone",
                    &played_at.and_utc().to_rfc3339(),
                    Some(&p.to_string()),
                );
            }
            notify.notify_one();
            log::info!("hs: queued a {} in {}", m.result, m.mode);
        });
        STOP.store(true, Ordering::SeqCst);
    });
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
    fn played_at_is_sent_as_utc_rfc3339() {
        let v = payload(&a_match(), "hearthstone", at());
        let s = v["played_at"].as_str().unwrap();
        assert!(s.starts_with("2026-08-02T17:44:59"));
        assert!(s.ends_with("+00:00") || s.ends_with('Z'));
    }
}
