//! The Battlegrounds rating, read from Hearthstone Deck Tracker's own file.
//!
//! The game's logs never carry the rating, and KFIRE does not read the game's
//! memory. HDT, which many players already run, writes the rating to
//! `%AppData%\HearthstoneDeckTracker\BgsLastGames.xml`. As an option, and only
//! for a member who installed HDT, the client reads it there.
//!
//! Only six attributes are read: EndTime, Rating, RatingAfter, Placemenent
//! (HDT's own spelling), Duos and FriendlyGame. `Player` holds account
//! identifiers and is never read.

use chrono::{DateTime, Utc};
use std::path::PathBuf;

/// Where members get HDT, shown when it is not installed.
pub const DOWNLOAD_URL: &str = "https://hsreplay.net/downloads/";

/// How far apart our end of match and HDT's may be to be the same game.
const MATCH_WINDOW: i64 = 3 * 60;

/// How long, and how often, the file is re-read after a match: HDT may write
/// it a few seconds after we saw the end in the game's log.
pub const WAIT_TOTAL: std::time::Duration = std::time::Duration::from_secs(30);
pub const WAIT_STEP: std::time::Duration = std::time::Duration::from_secs(3);

/// One Battlegrounds game as HDT recorded it, reduced to what KFIRE uses.
#[derive(Debug, Clone, PartialEq)]
pub struct HdtGame {
    pub ended_at: DateTime<Utc>,
    pub rating: i64,
    pub rating_after: i64,
    pub placement: i64,
    pub duos: bool,
    pub friendly: bool,
}

/// HDT's file, on Windows. `None` elsewhere: HDT only exists on Windows.
pub fn file_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(appdata)
            .join("HearthstoneDeckTracker")
            .join("BgsLastGames.xml"),
    )
}

/// Whether HDT's file exists on this machine.
pub fn available() -> bool {
    file_path().is_some_and(|p| p.is_file())
}

/// The value of attribute `name` in one tag, requiring whitespace before the
/// name so `Rating` never matches inside `RatingAfter`.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let mut from = 0;
    while let Some(i) = tag[from..].find(&key) {
        let at = from + i;
        let preceded = tag[..at].chars().next_back().is_some_and(char::is_whitespace);
        if preceded {
            let start = at + key.len();
            let end = tag[start..].find('"')? + start;
            return Some(&tag[start..end]);
        }
        from = at + key.len();
    }
    None
}

fn game(tag: &str) -> Option<HdtGame> {
    Some(HdtGame {
        ended_at: DateTime::parse_from_rfc3339(attr(tag, "EndTime")?)
            .ok()?
            .with_timezone(&Utc),
        rating: attr(tag, "Rating")?.parse().ok()?,
        rating_after: attr(tag, "RatingAfter")?.parse().ok()?,
        placement: attr(tag, "Placemenent")?.parse().ok()?,
        duos: attr(tag, "Duos") == Some("true"),
        friendly: attr(tag, "FriendlyGame") == Some("true"),
    })
}

/// Every readable `<Game>` in the file. A game missing a field is skipped: one
/// odd entry must not hide the others.
pub fn parse(xml: &str) -> Vec<HdtGame> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<Game") {
        let after = &rest[i + "<Game".len()..];
        if !after.starts_with(char::is_whitespace) {
            rest = after;
            continue;
        }
        let Some(end) = after.find('>') else { break };
        if let Some(g) = game(&after[..end]) {
            out.push(g);
        }
        rest = &after[end..];
    }
    out
}

/// The solo, non-friendly game that finished in `placement` closest to
/// `ended_at`, within three minutes. Never another game's rating.
pub fn find_match(games: &[HdtGame], placement: i64, ended_at: DateTime<Utc>) -> Option<&HdtGame> {
    games
        .iter()
        .filter(|g| !g.duos && !g.friendly && g.placement == placement)
        .map(|g| (g, (g.ended_at - ended_at).num_seconds().abs()))
        .filter(|(_, d)| *d <= MATCH_WINDOW)
        .min_by_key(|(_, d)| *d)
        .map(|(g, _)| g)
}

/// Why no rating was found, for the client's log.
#[derive(Debug, PartialEq)]
pub enum Missing {
    NoFile,
    Unreadable,
    NotFound,
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Missing::NoFile => "HDT file not found",
            Missing::Unreadable => "HDT file unreadable",
            Missing::NotFound => "no solo game with this place within 3 minutes in HDT file",
        })
    }
}

/// Re-reads HDT's file until the game shows up, for thirty seconds at most.
/// Blocking: call it from its own thread, never from the log follower.
pub fn wait_for_rating(placement: i64, ended_at: DateTime<Utc>) -> Result<(i64, i64), Missing> {
    let Some(path) = file_path() else { return Err(Missing::NoFile) };
    let started = std::time::Instant::now();
    loop {
        let last = match std::fs::read_to_string(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Missing::NoFile,
            Err(_) => Missing::Unreadable,
            Ok(xml) => match find_match(&parse(&xml), placement, ended_at) {
                Some(g) => return Ok((g.rating, g.rating_after)),
                None => Missing::NotFound,
            },
        };
        if started.elapsed() >= WAIT_TOTAL {
            return Err(last);
        }
        std::thread::sleep(WAIT_STEP);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    // Written by hand: a real file carries account identifiers, and this
    // repository is public.
    const XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<BgsLastGames>
  <Game Player="1_2" StartTime="2026-09-21T14:05:58.2640070+02:00" EndTime="2026-09-21T14:44:26.3018310+02:00" Hero="TB_BaconShop_HERO_70_SKIN_H" Rating="5571" RatingAfter="5644" Placemenent="2" FriendlyGame="false" Duos="false">
    <FinalBoard />
  </Game>
  <Game Player="1_2" StartTime="2026-09-21T15:00:00.0000000+02:00" EndTime="2026-09-21T15:30:00.0000000+02:00" Hero="BG28_HERO_400" Rating="7000" RatingAfter="7100" Placemenent="1" FriendlyGame="false" Duos="true" />
  <Game Player="1_2" StartTime="2026-09-21T16:00:00.0000000+02:00" EndTime="2026-09-21T16:30:00.0000000+02:00" Hero="BG28_HERO_400" Rating="5644" RatingAfter="5603" Placemenent="6" FriendlyGame="true" Duos="false" />
</BgsLastGames>"#;

    fn utc(h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 21, h, m, s).unwrap()
    }

    #[test]
    fn reads_every_game_with_its_zone() {
        let games = parse(XML);
        assert_eq!(games.len(), 3);
        let g = &games[0];
        assert_eq!(g.rating, 5571);
        assert_eq!(g.rating_after, 5644);
        assert_eq!(g.placement, 2, "the attribute is spelt Placemenent by HDT");
        assert_eq!(g.ended_at, utc(12, 44, 26) + chrono::Duration::nanoseconds(301_831_000));
        assert!(!g.duos && !g.friendly);
        assert!(games[1].duos);
        assert!(games[2].friendly);
    }

    #[test]
    fn a_game_missing_a_field_is_skipped_not_fatal() {
        let xml = r#"<Game EndTime="2026-09-21T14:44:26+02:00" Rating="5571" Placemenent="2" />
<Game EndTime="2026-09-21T14:50:00+02:00" Rating="1" RatingAfter="2" Placemenent="3" />"#;
        let games = parse(xml);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].placement, 3);
    }

    #[test]
    fn an_unrelated_element_starting_with_game_is_ignored() {
        assert!(parse(r#"<GameStats EndTime="x" />"#).is_empty());
    }

    #[test]
    fn matches_the_solo_game_with_the_same_place_ending_close() {
        let games = parse(XML);
        let found = find_match(&games, 2, utc(12, 45, 10)).expect("found");
        assert_eq!(found.rating_after, 5644);
    }

    #[test]
    fn never_takes_a_duo_a_friendly_or_another_place() {
        let games = parse(XML);
        assert!(find_match(&games, 1, utc(13, 30, 0)).is_none(), "duo");
        assert!(find_match(&games, 6, utc(14, 30, 0)).is_none(), "friendly");
        assert!(find_match(&games, 3, utc(12, 44, 26)).is_none(), "other place");
    }

    #[test]
    fn never_takes_a_game_ended_more_than_three_minutes_away() {
        let games = parse(XML);
        assert!(find_match(&games, 2, utc(12, 48, 0)).is_none());
    }

    #[test]
    fn the_closest_candidate_wins() {
        let xml = r#"<Game EndTime="2026-09-21T12:40:00Z" Rating="1" RatingAfter="10" Placemenent="2" FriendlyGame="false" Duos="false" />
<Game EndTime="2026-09-21T12:44:00Z" Rating="1" RatingAfter="20" Placemenent="2" FriendlyGame="false" Duos="false" />"#;
        let games = parse(xml);
        assert_eq!(find_match(&games, 2, utc(12, 43, 30)).unwrap().rating_after, 20);
    }

    #[test]
    fn older_files_without_the_duos_attribute_read_as_solo() {
        let xml = r#"<Game EndTime="2026-09-21T12:44:00Z" Rating="1" RatingAfter="2" Placemenent="2" />"#;
        let games = parse(xml);
        assert!(!games[0].duos && !games[0].friendly);
    }
}
