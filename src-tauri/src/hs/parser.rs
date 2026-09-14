//! Turning log lines into match summaries. Pure functions, no I/O.
//!
//! Line formats verified against a real 120 MB Power.log on 2026-09-14.
//!
//! PRIVACY: this module reads lines carrying the member's BattleTag, the
//! opponents' names and every card played. NONE of that may reach the Match
//! struct, which is what gets sent. The struct has no field that could hold a
//! name, and that is deliberate.

use chrono::{Duration, NaiveDateTime, NaiveTime};
use std::collections::HashMap;

/// One finished match, exactly what the server accepts and nothing more.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub mode: String,
    pub result: String,
    pub turns: Option<i64>,
    pub placement: Option<i64>,
    pub hero_card_id: Option<String>,
    /// Time of day of the final line; the day itself comes from the folder.
    pub ended_at: NaiveTime,
}

impl Match {
    /// The instant the match ended, given the session folder's own start.
    ///
    /// Log lines carry a time of day and no date. A time EARLIER than the
    /// session start means the session crossed midnight, so the day rolls over
    /// rather than travelling backwards.
    pub fn played_at(&self, session_start: NaiveDateTime) -> NaiveDateTime {
        let same_day = session_start.date().and_time(self.ended_at);
        if same_day < session_start {
            same_day + Duration::days(1)
        } else {
            same_day
        }
    }
}

/// State accumulated while reading one game.
#[derive(Default)]
struct Game {
    my_player_id: Option<String>,
    my_name: Option<String>,
    mode: Option<String>,
    result: Option<String>,
    turns: Option<i64>,
    placement: Option<i64>,
    hero_entity: Option<String>,
    ended_at: Option<NaiveTime>,
    /// Entity id -> card id.
    cards: HashMap<String, String>,
}

/// Every finished match in the given lines.
///
/// Unfinished games are dropped: a member who closed the game mid-match has no
/// result, and inventing one would be worse than reporting nothing.
pub fn parse_games<'a, I: Iterator<Item = &'a str>>(lines: I) -> Vec<Match> {
    let mut out = Vec::new();
    let mut cur: Option<Game> = None;

    for line in lines {
        // The game emits every event TWICE, under GameState and PowerTaskList.
        // Keeping both would count every match twice.
        if !line.contains("GameState.") {
            continue;
        }
        if line.contains("CREATE_GAME") {
            if let Some(g) = cur.take() {
                out.extend(g.finish());
            }
            cur = Some(Game::default());
            continue;
        }
        let Some(g) = cur.as_mut() else { continue };
        g.read(line);
    }
    if let Some(g) = cur.take() {
        out.extend(g.finish());
    }
    out
}

impl Game {
    fn read(&mut self, line: &str) {
        // The member is the account with a REAL id: in Battlegrounds the
        // opponent carries hi=0 lo=0, and picking it would report their result.
        if let Some(rest) = line.split("Player EntityID=").nth(1) {
            if let Some(pid) = field(rest, "PlayerID=") {
                let real = rest
                    .split("GameAccountId=[hi=")
                    .nth(1)
                    .map(|s| !s.starts_with("0 lo=0]"))
                    .unwrap_or(false);
                if real {
                    self.my_player_id = Some(pid);
                }
            }
        } else if let Some(rest) = line.split("PlayerID=").nth(1) {
            if let Some((pid, name)) = rest.split_once(", PlayerName=") {
                if Some(pid.trim()) == self.my_player_id.as_deref() {
                    self.my_name = Some(name.trim().to_string());
                }
            }
        }
        if let Some(gt) = field(line, "GameType=") {
            self.mode = Some(match gt.as_str() {
                "GT_BATTLEGROUNDS" => "battlegrounds".to_string(),
                _ => "constructed".to_string(),
            });
        }
        // Card identifiers, kept for every entity so the hero can be resolved
        // whatever its naming: a prefix-based search misses recent heroes.
        if let Some(rest) = line.split("FULL_ENTITY - Creating ID=").nth(1) {
            if let Some(id) = field(rest, "") {
                if let Some(card) = field(rest, "CardID=") {
                    self.cards.insert(id, card);
                }
            }
        }
        if let Some(id) = field(line, " id=") {
            if let Some(card) = field(line, "cardId=") {
                self.cards.insert(id, card);
            }
        }

        let Some(me) = self.my_name.clone() else {
            return;
        };
        let mine = format!("Entity={me} ");

        if line.contains(&mine) {
            if let Some(v) = field(line, "tag=HERO_ENTITY value=") {
                self.hero_entity = Some(v);
            }
            if let Some(v) = field(line, "tag=PLAYSTATE value=") {
                let r = match v.as_str() {
                    "WON" => Some("win"),
                    "LOST" => Some("loss"),
                    "TIED" => Some("draw"),
                    _ => None,
                };
                if let Some(r) = r {
                    self.result = Some(r.to_string());
                    self.ended_at = line_time(line);
                }
            }
        }
        // Turns come from the game itself; a player entity carries a smaller
        // counter that is NOT the number of turns played.
        if line.contains("Entity=GameEntity tag=TURN value=") {
            if let Some(v) = field(line, "tag=TURN value=").and_then(|v| v.parse().ok()) {
                self.turns = Some(self.turns.unwrap_or(0).max(v));
            }
        }
        // Placement changes many times; the LAST value is the result.
        if line.contains("tag=PLAYER_LEADERBOARD_PLACE value=") {
            let is_mine = self
                .my_player_id
                .as_deref()
                .map(|p| line.contains(&format!("player={p}]")))
                .unwrap_or(false);
            if is_mine {
                self.placement = field(line, "tag=PLAYER_LEADERBOARD_PLACE value=")
                    .and_then(|v| v.parse().ok());
            }
        }
    }

    fn finish(self) -> Option<Match> {
        let hero = self
            .hero_entity
            .as_ref()
            .and_then(|id| self.cards.get(id))
            .cloned();
        Some(Match {
            mode: self.mode?,
            result: self.result?,
            turns: self.turns,
            placement: self.placement,
            hero_card_id: hero,
            ended_at: self.ended_at?,
        })
    }
}

/// The value following `key`, up to the next space or closing bracket.
///
/// An empty key returns the first token, which is how `FULL_ENTITY - Creating
/// ID=95 CardID=...` yields its entity id.
fn field(line: &str, key: &str) -> Option<String> {
    let rest = if key.is_empty() {
        line
    } else {
        line.split(key).nth(1)?
    };
    let end = rest
        .find(|c: char| c.is_whitespace() || c == ']')
        .unwrap_or(rest.len());
    let v = rest[..end].trim();
    (!v.is_empty()).then(|| v.to_string())
}

/// The time of day a line was written.
fn line_time(line: &str) -> Option<NaiveTime> {
    let rest = line.strip_prefix("D ")?;
    let stamp = rest.split_whitespace().next()?;
    NaiveTime::parse_from_str(stamp, "%H:%M:%S%.f").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One full Battlegrounds game, hand-written from the real line formats.
    /// Every name here is invented: the real log carries the member's BattleTag
    /// and the opponents' names, and this repository is public.
    const BG_GAME: &str = r#"
D 17:24:13.6058674 GameState.DebugPrintPower() - CREATE_GAME
D 17:24:13.6058674 GameState.DebugPrintPower() -     Player EntityID=20 PlayerID=7 GameAccountId=[hi=144115198130930503 lo=24907941]
D 17:24:13.6058674 GameState.DebugPrintPower() -     Player EntityID=21 PlayerID=15 GameAccountId=[hi=0 lo=0]
D 17:24:13.6058674 GameState.DebugPrintGame() - PlayerID=7, PlayerName=TestPlayer#1234
D 17:24:13.6058674 GameState.DebugPrintGame() - PlayerID=15, PlayerName=Adversaire
D 17:24:13.6058674 GameState.DebugPrintGame() - GameType=GT_BATTLEGROUNDS
D 17:24:32.6289894 GameState.DebugPrintPower() -     TAG_CHANGE Entity=TestPlayer#1234 tag=HERO_ENTITY value=95
D 17:24:13.8338107 GameState.DebugPrintPower() -     FULL_ENTITY - Creating ID=95 CardID=BG28_HERO_400
D 17:39:34.5976165 GameState.DebugPrintPower() - TAG_CHANGE Entity=[entityName=Herosnom id=95 zone=PLAY zonePos=0 cardId=BG28_HERO_400 player=7] tag=PLAYER_LEADERBOARD_PLACE value=2
D 17:42:15.2856843 GameState.DebugPrintPower() - TAG_CHANGE Entity=[entityName=Herosnom id=95 zone=PLAY zonePos=0 cardId=BG28_HERO_400 player=7] tag=PLAYER_LEADERBOARD_PLACE value=5
D 17:44:00.0000000 GameState.DebugPrintPower() -     TAG_CHANGE Entity=GameEntity tag=TURN value=22
D 17:44:59.4684646 GameState.DebugPrintPower() - TAG_CHANGE Entity=TestPlayer#1234 tag=PLAYSTATE value=LOST
"#;

    fn parse_one(src: &str) -> Match {
        let games = parse_games(src.lines());
        assert_eq!(games.len(), 1, "expected exactly one game, got {games:?}");
        games.into_iter().next().unwrap()
    }

    #[test]
    fn reads_a_whole_battlegrounds_game() {
        let m = parse_one(BG_GAME);
        assert_eq!(m.mode, "battlegrounds");
        assert_eq!(m.result, "loss");
        assert_eq!(m.turns, Some(22));
        assert_eq!(m.placement, Some(5));
        assert_eq!(m.hero_card_id.as_deref(), Some("BG28_HERO_400"));
    }

    #[test]
    fn the_doubled_stream_does_not_double_the_games() {
        // The game emits every event twice, under GameState and PowerTaskList.
        // A parser that keeps both counts every match twice.
        let doubled: String = BG_GAME
            .lines()
            .flat_map(|l| {
                vec![
                    l.to_string(),
                    l.replace("GameState.DebugPrintPower", "PowerTaskList.DebugPrintPower")
                        .replace("GameState.DebugPrintGame", "PowerTaskList.DebugPrintGame"),
                ]
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(parse_games(doubled.lines()).len(), 1);
    }

    #[test]
    fn a_win_is_a_win() {
        let src = BG_GAME.replace("tag=PLAYSTATE value=LOST", "tag=PLAYSTATE value=WON");
        assert_eq!(parse_one(&src).result, "win");
    }

    #[test]
    fn a_tie_is_a_draw() {
        let src = BG_GAME.replace("tag=PLAYSTATE value=LOST", "tag=PLAYSTATE value=TIED");
        assert_eq!(parse_one(&src).result, "draw");
    }

    #[test]
    fn the_opponents_result_is_never_taken_for_ours() {
        // The opponent won the same game we lost. Picking the wrong player
        // would report a win.
        let src = format!(
            "{BG_GAME}D 17:44:59.4684646 GameState.DebugPrintPower() - TAG_CHANGE Entity=Adversaire tag=PLAYSTATE value=WON\n"
        );
        assert_eq!(parse_one(&src).result, "loss");
    }

    #[test]
    fn turns_come_from_the_game_entity_not_a_player() {
        // A player entity carries its own smaller counter, which is not the
        // number of turns played.
        let src = format!(
            "{BG_GAME}D 17:44:00.0000000 GameState.DebugPrintPower() - TAG_CHANGE Entity=TestPlayer#1234 tag=TURN value=11\n"
        );
        assert_eq!(parse_one(&src).turns, Some(22));
    }

    #[test]
    fn a_constructed_game_has_no_placement() {
        let src = BG_GAME
            .replace("GT_BATTLEGROUNDS", "GT_RANKED")
            .lines()
            .filter(|l| !l.contains("PLAYER_LEADERBOARD_PLACE"))
            .collect::<Vec<_>>()
            .join("\n");
        let m = parse_one(&src);
        assert_eq!(m.mode, "constructed");
        assert_eq!(m.placement, None);
    }

    #[test]
    fn an_unfinished_game_is_not_reported() {
        // The member closed the game mid-match: no result, so nothing to send.
        let src = BG_GAME.replace("tag=PLAYSTATE value=LOST", "tag=PLAYSTATE value=PLAYING");
        assert!(parse_games(src.lines()).is_empty());
    }

    #[test]
    fn an_unreadable_hero_does_not_lose_the_match() {
        let src = BG_GAME.replace("tag=HERO_ENTITY value=95", "tag=HERO_ENTITY value=999");
        let m = parse_one(&src);
        assert_eq!(m.hero_card_id, None);
        assert_eq!(m.result, "loss");
    }

    #[test]
    fn garbage_yields_nothing_and_does_not_panic() {
        assert!(parse_games("not a log at all\n\n".lines()).is_empty());
        assert!(parse_games("".lines()).is_empty());
    }

    #[test]
    fn two_games_in_one_file_are_two_matches() {
        let src = format!("{BG_GAME}{BG_GAME}");
        assert_eq!(parse_games(src.lines()).len(), 2);
    }

    #[test]
    fn played_at_combines_the_folder_date_with_the_line_time() {
        let m = parse_one(BG_GAME);
        let start = crate::hs::paths::session_start("Hearthstone_2026_08_02_17_22_29").unwrap();
        assert_eq!(
            m.played_at(start).format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-08-02 17:44:59"
        );
    }

    #[test]
    fn a_session_crossing_midnight_rolls_the_day_over() {
        // Lines carry a time of day and no date. A time EARLIER than the
        // session start means the next day, not a trip into the past.
        let src = BG_GAME.replace("D 17:44:59.4684646", "D 00:14:59.4684646");
        let m = parse_one(&src);
        let start = crate::hs::paths::session_start("Hearthstone_2026_08_02_23_40_00").unwrap();
        assert_eq!(
            m.played_at(start).format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-08-03 00:14:59"
        );
    }

    #[test]
    fn a_match_carries_no_name_of_any_kind() {
        // The log holds the member's BattleTag, the opponent's name and every
        // card. None of it may survive into the summary.
        let m = parse_one(BG_GAME);
        let debug = format!("{m:?}");
        for forbidden in ["TestPlayer", "Adversaire", "Herosnom", "#1234"] {
            assert!(!debug.contains(forbidden), "leaked {forbidden} in {debug}");
        }
    }
}
