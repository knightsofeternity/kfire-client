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

/// The state of a match still being played: a mode and two numbers.
///
/// Deliberately NOT a `Match`: this is broadcast to the whole guild several
/// times a minute, so it carries the strict minimum a card can display, and it
/// has no field a name could ever land in.
#[derive(Debug, Clone, PartialEq)]
pub struct Live {
    pub mode: String,
    /// The turn being played; 0 until the game has announced one.
    pub turn: i64,
    pub placement: Option<i64>,
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
    /// Set once this game has been reported, so later lines cannot report it
    /// a second time.
    emitted: bool,
}

/// Reads lines as they arrive, emitting each match the moment it ends.
///
/// STATEFUL ON PURPOSE. The watcher hands over whatever the game appended since
/// the last poll, a few seconds' worth, while a match lasts many minutes. Its
/// opening and its result therefore land in DIFFERENT chunks, so a parser that
/// only looked at one chunk at a time would never see a single complete match.
/// Verified on a real log: chunked reading lost matches until this state was
/// carried across calls.
#[derive(Default)]
pub struct Parser {
    cur: Option<Game>,
}

impl Parser {
    pub fn new() -> Self {
        Self::default()
    }

    /// The matches that finished within these lines.
    ///
    /// A match is emitted as soon as its result appears, not when the next one
    /// starts: the last match of a session must not wait for a session that
    /// may never come.
    ///
    /// Unfinished games are dropped: a member who closed the game mid-match has
    /// no result, and inventing one would be worse than reporting nothing.
    pub fn push<'a, I: Iterator<Item = &'a str>>(&mut self, lines: I) -> Vec<Match> {
        let mut out = Vec::new();
        for line in lines {
            // The game emits every event TWICE, under GameState and
            // PowerTaskList. Keeping both would count every match twice.
            if !line.contains("GameState.") {
                continue;
            }
            if line.contains("CREATE_GAME") {
                self.cur = Some(Game::default());
                continue;
            }
            let Some(g) = self.cur.as_mut() else { continue };
            g.read(line);
            if !g.emitted {
                if let Some(m) = g.finish() {
                    g.emitted = true;
                    out.push(m);
                }
            }
        }
        out
    }

    /// The match being played right now, if there is one.
    ///
    /// Nothing is read again here: the fields already exist because `finish`
    /// needs them, and this only exposes the three the live card shows.
    ///
    /// `None` once the match has been emitted, because its card must give way
    /// to the end-of-match message rather than freeze on a final state.
    pub fn live(&self) -> Option<Live> {
        let g = self.cur.as_ref()?;
        if g.emitted {
            return None;
        }
        // Without a mode there is nothing to display, and the mode decides
        // whether a placement means anything at all.
        let mode = g.mode.clone()?;
        // Nor without a turn. The game announces one within seconds of the
        // first mulligan, so this only skips the very start; reporting turn 0
        // meanwhile would be rejected by the server, which refuses anything
        // below the first turn, and every one of those refusals would come
        // back to this client as an error it can do nothing about.
        let turn = g.turns?;
        Some(Live {
            turn,
            // Constructed has no leaderboard, so a placement read there can
            // only be noise; the server refuses a constructed match with one.
            placement: (mode == "battlegrounds")
                .then_some(g.reported_placement())
                .flatten(),
            mode,
        })
    }
}

/// Every finished match in the given lines, read in one go.
///
/// A convenience over `Parser` for callers holding a whole file.
pub fn parse_games<'a, I: Iterator<Item = &'a str>>(lines: I) -> Vec<Match> {
    Parser::new().push(lines)
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
        // The turn is the one the MEMBER sees, and only his own entity carries
        // it. GameEntity counts phases, not turns: in Battlegrounds it ticks
        // once for the recruit phase and once for the combat, so it runs at
        // exactly twice the visible turn. A beta tester who stopped at turn 18
        // was reported at 36, and his log shows GameEntity at 36 against 18 on
        // his own entity.
        //
        // Other player entities carry the same counter, Bob le barman included,
        // so "not GameEntity" is not enough: it has to be the member's.
        if line.contains(&mine) {
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

    /// The member's position, with the one correction Battlegrounds needs.
    ///
    /// PLAYER_LEADERBOARD_PLACE only moves as opponents die, and the game never
    /// emits 1 for the last player standing: the winner's log stops at 2, which
    /// is how a top 1 was recorded as a second place. Winning a Battlegrounds
    /// lobby IS first place, by definition, so the result settles the position.
    /// PLAYSTATE=WON is only ever written for a real win, so this cannot turn a
    /// top 2 into a top 1.
    fn reported_placement(&self) -> Option<i64> {
        if self.mode.as_deref() == Some("battlegrounds") && self.result.as_deref() == Some("win") {
            return Some(1);
        }
        self.placement
    }

    /// The match, once every mandatory field has been read.
    fn finish(&self) -> Option<Match> {
        let hero = self
            .hero_entity
            .as_ref()
            .and_then(|id| self.cards.get(id))
            .cloned();
        Some(Match {
            mode: self.mode.clone()?,
            result: self.result.clone()?,
            turns: self.turns,
            placement: self.reported_placement(),
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
D 17:44:00.0000000 GameState.DebugPrintPower() -     TAG_CHANGE Entity=GameEntity tag=TURN value=36
D 17:44:00.0000000 GameState.DebugPrintPower() -     TAG_CHANGE Entity=TestPlayer#1234 tag=TURN value=18
D 17:44:00.0000000 GameState.DebugPrintPower() -     TAG_CHANGE Entity=Bob le barman tag=TURN value=18
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
        assert_eq!(m.turns, Some(18));
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
    fn le_tour_vient_de_lentite_du_membre_pas_de_gameentity() {
        // This test used to claim the opposite, and pinned the bug: a beta
        // tester who played 18 turns was reported as having played 36. In
        // Battlegrounds GameEntity counts every phase, recruit then combat, so
        // it runs at exactly twice the turn the member sees. The counter on the
        // member's own entity is that visible turn.
        let m = parse_one(BG_GAME);
        assert_eq!(m.turns, Some(18));
    }

    #[test]
    fn lentite_dun_autre_joueur_ne_donne_jamais_le_tour() {
        // Bob le barman is a player entity too, so "anything but GameEntity"
        // would be enough to pick him up. Only the member's entity counts.
        let src = BG_GAME
            .lines()
            .filter(|l| !l.contains("Entity=TestPlayer#1234 tag=TURN"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(parse_one(&src).turns, None);
    }

    #[test]
    fn sans_compteur_du_membre_aucun_tour_nest_annonce() {
        // Falling back on GameEntity would announce double the real turn, which
        // is worse than announcing nothing: the guild would read a number that
        // never happened. A missing turn simply drops the field.
        let src = BG_GAME
            .lines()
            .filter(|l| !l.contains("tag=TURN value=18"))
            .collect::<Vec<_>>()
            .join("\n");
        let m = parse_one(&src);
        assert_eq!(m.turns, None);
        assert_eq!(m.result, "loss");
    }

    #[test]
    fn une_victoire_en_champs_de_bataille_est_une_premiere_place() {
        // PLAYER_LEADERBOARD_PLACE follows the opponents dying and the game
        // never emits 1 for the winner, so the log stops at 2. Winning the
        // lobby IS first place, so the win settles the position.
        let src = BG_GAME
            .replace("tag=PLAYSTATE value=LOST", "tag=PLAYSTATE value=WON")
            .replace(
                "tag=PLAYER_LEADERBOARD_PLACE value=5",
                "tag=PLAYER_LEADERBOARD_PLACE value=2",
            );
        let m = parse_one(&src);
        assert_eq!(m.result, "win");
        assert_eq!(m.placement, Some(1));
    }

    #[test]
    fn une_defaite_garde_la_position_du_journal() {
        let m = parse_one(BG_GAME);
        assert_eq!(m.result, "loss");
        assert_eq!(m.placement, Some(5));
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

    /// The lines of a game up to, but not including, the first one holding
    /// `marker`: what the log already holds while the match is still running.
    fn until<'a>(src: &'a str, marker: &str) -> Vec<&'a str> {
        src.lines().take_while(|l| !l.contains(marker)).collect()
    }

    #[test]
    fn une_partie_en_cours_rend_un_etat() {
        let mut p = Parser::new();
        assert!(p
            .push(until(BG_GAME, "tag=PLAYSTATE").into_iter())
            .is_empty());
        let l = p.live().expect("une partie en cours");
        assert_eq!(l.mode, "battlegrounds");
        assert_eq!(l.turn, 18);
        assert_eq!(l.placement, Some(5));
    }

    #[test]
    fn une_partie_terminee_et_emise_ne_rend_plus_detat() {
        let mut p = Parser::new();
        assert_eq!(p.push(BG_GAME.lines()).len(), 1);
        assert_eq!(p.live(), None);
    }

    #[test]
    fn une_partie_sans_mode_connu_ne_rend_pas_detat() {
        // The game type arrives a few lines after the game opens; until then
        // there is nothing a card could display.
        let mut p = Parser::new();
        p.push(until(BG_GAME, "GameType=").into_iter());
        assert_eq!(p.live(), None);
    }

    #[test]
    fn le_tour_remonte_est_le_tour_en_cours() {
        // The same member line as the real log, with its time and turn moved
        // back: this is what the log holds halfway through the match.
        let mut p = Parser::new();
        p.push(until(BG_GAME, "tag=TURN value=36").into_iter());
        p.push(std::iter::once(
            "D 17:35:00.0000000 GameState.DebugPrintPower() -     TAG_CHANGE Entity=TestPlayer#1234 tag=TURN value=9",
        ));
        assert_eq!(p.live().expect("une partie en cours").turn, 9);

        // And it follows the match forward.
        p.push(until(BG_GAME, "tag=PLAYSTATE").into_iter());
        assert_eq!(p.live().expect("une partie en cours").turn, 18);
    }

    #[test]
    fn letat_en_direct_dune_victoire_annonce_la_premiere_place() {
        // Same correction as the summary: the live card must not show a second
        // place on a lobby the member has just won.
        let src = BG_GAME.replace("tag=PLAYSTATE value=LOST", "tag=PLAYSTATE value=WON");
        let mut p = Parser::new();
        // Everything but the very last line, so the game is read but not yet
        // emitted: `live` gives way to the end-of-match message once it is.
        let lines: Vec<&str> = src.lines().collect();
        p.push(lines[..lines.len() - 1].iter().copied());
        assert_eq!(p.live().expect("une partie en cours").placement, Some(5));
        let mut g = Game {
            mode: Some("battlegrounds".to_string()),
            result: Some("win".to_string()),
            turns: Some(18),
            placement: Some(2),
            ..Default::default()
        };
        g.emitted = false;
        let mut p2 = Parser::new();
        p2.cur = Some(g);
        assert_eq!(p2.live().expect("une partie en cours").placement, Some(1));
    }

    #[test]
    fn en_mode_construit_aucune_position_nest_remontee() {
        // Constructed has no leaderboard. Even when the log carries such a
        // line, the live state must not claim a placement: the server refuses
        // a constructed match that has one.
        let src = BG_GAME.replace("GT_BATTLEGROUNDS", "GT_RANKED");
        let mut p = Parser::new();
        p.push(until(&src, "tag=PLAYSTATE").into_iter());
        let l = p.live().expect("une partie en cours");
        assert_eq!(l.mode, "constructed");
        assert_eq!(l.placement, None);
    }

    #[test]
    fn une_partie_sans_tour_connu_nemet_aucun_etat() {
        // Le serveur refuse tout tour inferieur a 1, et chacun de ces refus
        // reviendrait au client sous forme d'erreur. Tant que le jeu n'a pas
        // annonce de tour, il n'y a rien a montrer.
        let mut g = Game {
            mode: Some("battlegrounds".to_string()),
            ..Default::default()
        };
        g.turns = None;
        let mut p = Parser::new();
        p.cur = Some(g);
        assert!(p.live().is_none());

        p.cur.as_mut().unwrap().turns = Some(1);
        assert_eq!(p.live().map(|l| l.turn), Some(1));
    }

    #[test]
    fn letat_en_direct_ne_porte_aucun_nom() {
        // Same rule as the summary: the log holds the member's BattleTag, the
        // opponent's name and every card.
        let mut p = Parser::new();
        p.push(until(BG_GAME, "tag=PLAYSTATE").into_iter());
        let debug = format!("{:?}", p.live().expect("une partie en cours"));
        for forbidden in ["TestPlayer", "Adversaire", "Herosnom", "#1234", "BG28"] {
            assert!(!debug.contains(forbidden), "leaked {forbidden} in {debug}");
        }
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

#[cfg(test)]
mod real_log {
    use super::*;

    /// Checks the parser against a real Power.log, kept OUTSIDE this public
    /// repository: it carries the member's BattleTag, the opponents' names and
    /// every card played. Ignored by default, driven by an environment
    /// variable, so the suite stays runnable by anyone.
    #[test]
    #[ignore = "reads a real log outside the repository"]
    fn real_log_yields_the_three_known_matches() {
        let path = std::env::var("HS_REAL_LOG").expect("set HS_REAL_LOG");
        let src = std::fs::read_to_string(path).unwrap();
        let games = parse_games(src.lines());
        assert_eq!(games.len(), 3, "wrong match count");
        assert_eq!(
            games.iter().map(|g| g.placement).collect::<Vec<_>>(),
            vec![Some(5), Some(4), Some(5)]
        );
        // Halved on 2026-09-21: these were read off GameEntity, which counts
        // both Battlegrounds phases, so they were twice the turn the member
        // played. The member's own counter gives the visible turn.
        assert_eq!(
            games.iter().map(|g| g.turns).collect::<Vec<_>>(),
            vec![Some(11), Some(13), Some(12)]
        );
        assert_eq!(
            games
                .iter()
                .map(|g| g.hero_card_id.as_deref())
                .collect::<Vec<_>>(),
            vec![
                Some("BG28_HERO_400"),
                Some("BG20_HERO_101_SKIN_G"),
                Some("TB_BaconShop_HERO_45_SKIN_F")
            ]
        );
        assert!(games.iter().all(|g| g.result == "loss"));
        assert!(games.iter().all(|g| g.mode == "battlegrounds"));
    }
}
