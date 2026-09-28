//! Reading a Rocket League match, in pure functions.
//!
//! The full scoresheet, which carries every player's name, lives here and goes
//! no further. This module uses it to compute, and lets out facts about the
//! member, plus the other players' numbers without their names.

use serde_json::Value;

/// The playlists with no opponent: free play, workshop, training. Reporting
/// them would skew every ratio.
pub const TRAINING: &[i64] = &[0, 9, 19, 21, 73];

/// The ranked playlists, according to the Psyonix catalogue.
pub const RANKED: &[i64] = &[10, 11, 13, 27, 28, 29, 30];

/// How many distinct player names we keep at most.
///
/// This is not distrust of the game, which runs on the member's own machine:
/// it is that this set serves only to write ONE log line, and a log line needs
/// no more than that. A real match holds eight at most, substitutes included.
const MAX_NAMES_SEEN: usize = 32;

/// How many other players a summary carries at most: four against four, minus
/// the member. The server refuses more.
const MAX_OTHERS: usize = 7;

/// The match's identifier as it leaves this machine: the SHA-256 of the game's
/// `MatchGuid`, in lowercase hex.
///
/// Every member of the same match computes the same key, which is how the
/// server groups their reports; the raw GUID stays here.
pub fn match_key(guid: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(guid.as_bytes()))
}

/// Another player of the match, reduced to numbers. Their name is only the key
/// `Match` keeps them under, and never leaves it.
#[derive(Debug, Clone, PartialEq)]
pub struct Other {
    pub team: i64,
    pub score: i64,
    pub goals: i64,
    pub assists: i64,
    pub saves: i64,
    pub shots: i64,
    pub demos: i64,
    /// Seen during the match, absent from the latest state: they left.
    pub left: bool,
}

/// Whether this playlist is ranked.
pub fn is_ranked(playlist: i64) -> bool {
    RANKED.contains(&playlist)
}

/// Whether this playlist has no opponent and must never be reported.
pub fn is_training(playlist: i64) -> bool {
    TRAINING.contains(&playlist)
}

/// The reasons a match is not reported.
///
/// `finish()` returns exactly one of them: never a combination, never a silent
/// `None`. Each variant states the truth it observed, not a guess at its
/// cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// No `UpdateState` was ever received: there is nothing to summarise.
    NeverObserved,
    /// One of the two sides never held a single player. Free play and training
    /// have nobody on the other side; a real match does.
    NoOpponent,
    /// The game really did send a playlist, and it is a training playlist
    /// according to the Psyonix catalogue.
    TrainingPlaylist(i64),
    /// The observed team size (the largest roster seen on one side) falls
    /// outside the range that is plausible for Rocket League.
    TeamSizeOutOfRange(i64),
    /// The configured player name matches nobody on the scoresheet.
    MemberNotFound,
    /// The member was found, but on a team that is neither blue nor orange.
    MemberTeamInvalid(i64),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::NeverObserved => write!(f, "no update was ever observed"),
            Refusal::NoOpponent => write!(f, "no opponent was ever seen on the other team"),
            Refusal::TrainingPlaylist(p) => write!(f, "training playlist ({p})"),
            Refusal::TeamSizeOutOfRange(n) => write!(f, "team size out of range ({n})"),
            Refusal::MemberNotFound => write!(f, "the configured member name matched nobody"),
            Refusal::MemberTeamInvalid(t) => write!(f, "the member's team number is invalid ({t})"),
        }
    }
}

/// The summary of a finished match: nothing but facts about the member.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// `None` when the game never sent this playlist. That is the norm: Rocket
    /// League's actual protocol has no `Playlist` field.
    pub playlist: Option<i64>,
    pub team_size: i64,
    pub player_team: i64,
    pub team_blue_score: i64,
    pub team_orange_score: i64,
    pub result: String,
    pub goals: i64,
    pub assists: i64,
    pub saves: i64,
    pub shots: i64,
    pub score: i64,
    pub demos: i64,
    pub mvp: bool,
    pub duration_seconds: i64,
    /// `None` when the game never sent a `MatchGuid`: then `others` is empty
    /// and the match is reported as before the scoreboard existed.
    pub match_key: Option<String>,
    /// The other players, by team then score, never in the stream's order.
    pub others: Vec<Other>,
}

/// A match's current state, for the live feed. Never written, never replayed.
#[derive(Debug, Clone, PartialEq)]
pub struct Live {
    pub team_blue_score: i64,
    pub team_orange_score: i64,
    pub seconds_remaining: i64,
    pub overtime: bool,
    pub goals: i64,
    pub assists: i64,
    pub saves: i64,
    pub shots: i64,
    pub score: i64,
    pub demos: i64,
}

/// One player's statistics, exactly as the game gives them.
#[derive(Debug, Clone, Default)]
struct Stats {
    team: i64,
    goals: i64,
    assists: i64,
    saves: i64,
    shots: i64,
    score: i64,
    demos: i64,
}

/// A match being observed.
pub struct Match {
    member: String,
    guid: Option<String>,
    /// `None` for as long as the game has never sent a `Playlist` field. That
    /// is the normal state: the real protocol has no such field.
    playlist: Option<i64>,
    blue: i64,
    orange: i64,
    seconds: i64,
    overtime: bool,
    /// The largest player count seen on each team, kept separately. A player
    /// who leaves disappears from the last state, so only the maximum tells the
    /// match's true size. Kept apart because either one at zero means "nobody
    /// on the other side", hence free play or training, never a real match:
    /// this is the main guard, more reliable than a playlist the game does not
    /// necessarily send.
    max_players_blue: i64,
    max_players_orange: i64,
    /// The member's statistics, and the best score seen on each team.
    mine: Option<Stats>,
    best_blue: i64,
    best_orange: i64,
    seen: bool,
    /// Every player name met during the match.
    ///
    /// They NEVER leave this machine. They serve one purpose only: when the
    /// member was never found, writing them to the local log so that he can see
    /// for himself under what name the game refers to him, and fix his setting.
    /// Without that, a mistyped name produces nothing at all and stays
    /// impossible to diagnose.
    names_seen: std::collections::BTreeSet<String>,
    /// Every other player's latest line, keyed by name. The names never leave
    /// this struct: `finish()` emits the values only.
    others: std::collections::BTreeMap<String, Other>,
}

fn i(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

/// Whether this name from the scoresheet is the member's.
///
/// The comparison ignores case and surrounding spaces. The exact format of the
/// `Name` field has never been checked against the real game, only read in the
/// `ke-rl-tracker` code, and a case slip in the setting is the likeliest
/// mistake. The tolerance costs nothing and removes a whole class of silent
/// failures.
fn same_player(name: Option<&str>, member: &str) -> bool {
    match name {
        Some(n) => n.trim().eq_ignore_ascii_case(member.trim()),
        None => false,
    }
}

impl Match {
    /// Opens the observation of a match for this member.
    pub fn new(member: &str) -> Self {
        Self {
            member: member.to_string(),
            guid: None,
            playlist: None,
            blue: 0,
            orange: 0,
            seconds: 0,
            overtime: false,
            max_players_blue: 0,
            max_players_orange: 0,
            mine: None,
            best_blue: 0,
            best_orange: 0,
            seen: false,
            names_seen: Default::default(),
            others: Default::default(),
        }
    }

    /// The match's GUID, once it is known.
    pub fn guid(&self) -> Option<String> {
        self.guid.clone()
    }

    /// Takes an `UpdateState` into account.
    pub fn observe(&mut self, data: &Value) {
        if let Some(g) = data.get("MatchGuid").and_then(Value::as_str) {
            // The GUID is the same on every frame of a match: only rewrite it
            // when it has genuinely changed.
            if !g.is_empty() && self.guid.as_deref() != Some(g) {
                self.guid = Some(g.to_string());
            }
        }

        // Borrowed, never cloned: only four numbers are read here, and this
        // function runs thirty times a second for the whole match.
        if let Some(game) = data.get("Game").filter(|g| g.is_object()) {
            // Never invent a playlist: the real protocol has no such field.
            // `i()` would return `0` for a missing key, and `0` is a training
            // playlist; that is exactly the bug that refused every real match.
            if let Some(p) = game.get("Playlist").and_then(Value::as_i64) {
                self.playlist = Some(p);
            }
            self.seconds = i(game, "TimeSeconds");
            self.overtime = game
                .get("bOvertime")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if let Some(teams) = game.get("Teams").and_then(Value::as_array) {
                for t in teams {
                    match i(t, "TeamNum") {
                        0 => self.blue = i(t, "Score"),
                        1 => self.orange = i(t, "Score"),
                        _ => {}
                    }
                }
            }
        }

        // Players sits at the ROOT of Data, not under Game. The original
        // Python agent looks in the wrong place; copying its mistake would read
        // nothing at all.
        let Some(players) = data.get("Players").and_then(Value::as_array) else {
            return;
        };

        let (mut blue_n, mut orange_n) = (0i64, 0i64);
        // Borrowed from the frame, so marking who left costs no allocation.
        let mut present: Vec<&str> = Vec::with_capacity(players.len());
        for p in players {
            let team = i(p, "TeamNum");
            let score = i(p, "Score");
            match team {
                0 => {
                    blue_n += 1;
                    self.best_blue = self.best_blue.max(score);
                }
                1 => {
                    orange_n += 1;
                    self.best_orange = self.best_orange.max(score);
                }
                _ => {}
            }
            let name = p.get("Name").and_then(Value::as_str);
            if let Some(n) = name {
                // contains() takes a &str without allocating; only a genuinely
                // new name pays for a String, so a handful per match instead of
                // one per player per frame.
                if !self.names_seen.contains(n) && self.names_seen.len() < MAX_NAMES_SEEN {
                    self.names_seen.insert(n.to_string());
                }
            }
            if same_player(name, &self.member) {
                self.mine = Some(Stats {
                    team,
                    goals: i(p, "Goals"),
                    assists: i(p, "Assists"),
                    saves: i(p, "Saves"),
                    shots: i(p, "Shots"),
                    score,
                    demos: i(p, "Demos"),
                });
            } else if let Some(n) = name {
                present.push(n);
                let line = Other {
                    team,
                    score,
                    goals: i(p, "Goals"),
                    assists: i(p, "Assists"),
                    saves: i(p, "Saves"),
                    shots: i(p, "Shots"),
                    demos: i(p, "Demos"),
                    left: false,
                };
                if let Some(o) = self.others.get_mut(n) {
                    *o = line;
                } else if self.others.len() < MAX_NAMES_SEEN {
                    self.others.insert(n.to_string(), line);
                }
            }
        }
        for (name, o) in self.others.iter_mut() {
            o.left = !present.contains(&name.as_str());
        }
        self.max_players_blue = self.max_players_blue.max(blue_n);
        self.max_players_orange = self.max_players_orange.max(orange_n);
        self.seen = true;
    }

    /// The names met during the match, for the LOCAL log only.
    pub fn names_seen(&self) -> Vec<String> {
        self.names_seen.iter().cloned().collect()
    }

    /// The other players as the summary carries them.
    ///
    /// Players still present come first when there are more than seven, so a
    /// match with substitutes keeps those who finished it. Then by team and
    /// score: the order players arrived in must not show through.
    fn others_for_summary(&self) -> Vec<Other> {
        let mut v: Vec<Other> = self.others.values().cloned().collect();
        v.sort_by_key(|o| (o.left, std::cmp::Reverse(o.score)));
        v.truncate(MAX_OTHERS);
        v.sort_by_key(|o| (o.team, std::cmp::Reverse(o.score)));
        v
    }

    /// The current state, for the live feed.
    pub fn live(&self) -> Option<Live> {
        let mine = self.mine.as_ref()?;
        Some(Live {
            team_blue_score: self.blue,
            team_orange_score: self.orange,
            seconds_remaining: self.seconds.max(0),
            overtime: self.overtime,
            goals: mine.goals,
            assists: mine.assists,
            saves: mine.saves,
            shots: mine.shots,
            score: mine.score,
            demos: mine.demos,
        })
    }

    /// The summary, if and only if it is COMPLETE.
    ///
    /// Returns the precise reason as soon as anything is missing: never
    /// observed, no opponent, training playlist (only when the game really did
    /// send one), impossible team size, or the member's statistics absent. His
    /// row can be missing from the last state if he leaves before the end, and
    /// `ke-rl-tracker`'s production code keeps `if our_player:` for exactly
    /// that reason. Better a missing match than a wrong one: an invented zero
    /// is indistinguishable from a real zero and would poison the guild's
    /// averages for ever.
    pub fn finish(&self, duration_seconds: i64) -> Result<Summary, Refusal> {
        if !self.seen {
            return Err(Refusal::NeverObserved);
        }
        // The real guard: free play and training have nobody on the other
        // side. A real match does, always, on both sides.
        if self.max_players_blue == 0 || self.max_players_orange == 0 {
            return Err(Refusal::NoOpponent);
        }
        // Kept for the day Psyonix adds this field, but only when it is REALLY
        // there: never invented.
        if let Some(p) = self.playlist {
            if is_training(p) {
                return Err(Refusal::TrainingPlaylist(p));
            }
        }
        let team_size = self.max_players_blue.max(self.max_players_orange);
        if !(1..=4).contains(&team_size) {
            return Err(Refusal::TeamSizeOutOfRange(team_size));
        }
        let mine = self.mine.as_ref().ok_or(Refusal::MemberNotFound)?;
        if mine.team != 0 && mine.team != 1 {
            return Err(Refusal::MemberTeamInvalid(mine.team));
        }

        let (my_score, their_score) = if mine.team == 0 {
            (self.blue, self.orange)
        } else {
            (self.orange, self.blue)
        };
        let result = match my_score.cmp(&their_score) {
            std::cmp::Ordering::Greater => "win",
            std::cmp::Ordering::Less => "loss",
            std::cmp::Ordering::Equal => "draw",
        };

        // The MVP is the best score on the WINNING team, so there is no MVP
        // without a win.
        let best_of_mine = if mine.team == 0 {
            self.best_blue
        } else {
            self.best_orange
        };
        let mvp = result == "win" && mine.score >= best_of_mine;

        let match_key = self.guid.as_deref().map(match_key);
        let others = if match_key.is_some() {
            self.others_for_summary()
        } else {
            Vec::new()
        };

        Ok(Summary {
            playlist: self.playlist,
            team_size,
            player_team: mine.team,
            team_blue_score: self.blue,
            team_orange_score: self.orange,
            result: result.to_string(),
            goals: mine.goals,
            assists: mine.assists,
            saves: mine.saves,
            shots: mine.shots,
            score: mine.score,
            demos: mine.demos,
            mvp,
            duration_seconds: duration_seconds.clamp(0, 7200),
            match_key,
            others,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The stream's real shape, as documented by the Psyonix Stats API:
    /// `Data.Game` NEVER has a `Playlist` field. This test is the proof of the
    /// bug: with the old code, `i(game, "Playlist")` returned `0` for that
    /// missing field, `0` is in `TRAINING`, and the match was refused even
    /// though the member was very much in it, with an opponent and a clear win.
    /// It must fail before the fix and pass after it.
    #[test]
    fn a_real_match_with_no_playlist_field_is_summarised() {
        let state = json!({
            "MatchGuid": "g-real-1",
            "Game": {
                "TimeSeconds": 0,
                "bOvertime": false,
                "bReplay": false,
                "bHasWinner": true,
                "Winner": "Blue",
                "Arena": "Stadium_P",
                "Teams": [
                    {"Name": "Blue", "TeamNum": 0, "Score": 5},
                    {"Name": "Orange", "TeamNum": 1, "Score": 1},
                ],
            },
            "Players": [
                {
                    "Name": "Bushido",
                    "Shortcut": 0,
                    "TeamNum": 0,
                    "PrimaryId": "Steam|76561198000000000|0",
                    "Score": 640,
                    "Goals": 3,
                    "Assists": 1,
                    "Saves": 2,
                    "Shots": 5,
                    "Demos": 1,
                },
                {
                    "Name": "Coequipier",
                    "Shortcut": 1,
                    "TeamNum": 0,
                    "PrimaryId": "Epic|abc123|0",
                    "Score": 300,
                    "Goals": 2,
                    "Assists": 0,
                    "Saves": 0,
                    "Shots": 3,
                    "Demos": 0,
                },
                {
                    "Name": "Adversaire1",
                    "Shortcut": 2,
                    "TeamNum": 1,
                    "PrimaryId": "Steam|76561198000000001|0",
                    "Score": 200,
                    "Goals": 1,
                    "Assists": 0,
                    "Saves": 1,
                    "Shots": 4,
                    "Demos": 0,
                },
                {
                    "Name": "Adversaire2",
                    "Shortcut": 3,
                    "TeamNum": 1,
                    "PrimaryId": "Unknown|0|0",
                    "Score": 150,
                    "Goals": 0,
                    "Assists": 1,
                    "Saves": 0,
                    "Shots": 2,
                    "Demos": 0,
                },
            ],
        });
        let mut m = Match::new("Bushido");
        m.observe(&state);
        let s = m
            .finish(300)
            .expect("a real match with no Playlist must be summarised");
        assert_eq!(s.playlist, None);
        assert_eq!(s.result, "win");
        assert_eq!(s.team_size, 2);
    }

    /// With no opponent, nobody on the other side: free play or training,
    /// never a real match. The member is alone on his team.
    #[test]
    fn a_solo_match_with_nobody_on_the_other_team_has_no_opponent() {
        let mut m = Match::new("Bushido");
        m.observe(&state(-1, 0, 0, json!([me(0, 3)])));
        assert_eq!(m.finish(300), Err(Refusal::NoOpponent));
    }

    /// The field really is there this time: the historical rule still applies,
    /// but only when the data is genuinely present.
    #[test]
    fn a_real_match_with_a_present_training_playlist_is_refused() {
        let mut m = Match::new("Bushido");
        m.observe(&state(73, 4, 2, json!([me(0, 2), them("x", 1, 100)])));
        assert_eq!(m.finish(300), Err(Refusal::TrainingPlaylist(73)));
    }

    /// The member is absent from the scoresheet: this is the only case where
    /// the configured name can really be to blame.
    #[test]
    fn a_real_match_without_the_member_is_member_not_found() {
        let mut m = Match::new("Bushido");
        m.observe(&state(
            13,
            4,
            2,
            json!([them("a", 0, 100), them("b", 1, 100)]),
        ));
        assert_eq!(m.finish(300), Err(Refusal::MemberNotFound));
    }

    fn state(
        playlist: i64,
        blue: i64,
        orange: i64,
        players: serde_json::Value,
    ) -> serde_json::Value {
        json!({
            "MatchGuid": "g-1",
            "Game": {
                "Playlist": playlist,
                "TimeSeconds": 120,
                "bOvertime": false,
                "Teams": [{"TeamNum": 0, "Score": blue}, {"TeamNum": 1, "Score": orange}],
            },
            "Players": players,
        })
    }

    fn me(team: i64, goals: i64) -> serde_json::Value {
        json!({"Name": "Bushido", "TeamNum": team, "Goals": goals,
               "Assists": 1, "Saves": 2, "Shots": 4, "Score": 420, "Demos": 0})
    }

    fn them(name: &str, team: i64, score: i64) -> serde_json::Value {
        json!({"Name": name, "TeamNum": team, "Goals": 0,
               "Assists": 0, "Saves": 0, "Shots": 0, "Score": score, "Demos": 0})
    }

    #[test]
    fn a_finished_match_is_summarised() {
        let mut m = Match::new("Bushido");
        m.observe(&state(13, 4, 2, json!([me(0, 2), them("x", 1, 100)])));
        let s = m.finish(300).expect("a complete match must be summarised");
        assert_eq!(s.playlist, Some(13));
        assert_eq!(s.player_team, 0);
        assert_eq!(s.team_blue_score, 4);
        assert_eq!(s.team_orange_score, 2);
        assert_eq!(s.result, "win");
        assert_eq!(s.goals, 2);
        assert_eq!(s.duration_seconds, 300);
    }

    #[test]
    fn the_orange_side_wins_when_its_score_is_higher() {
        // The mirror of the blue case. Getting it backwards would throw away
        // half the guild's wins with nothing else to show for it.
        let mut m = Match::new("Bushido");
        m.observe(&state(11, 1, 5, json!([me(1, 3), them("x", 0, 100)])));
        let s = m.finish(300).unwrap();
        assert_eq!(s.player_team, 1);
        assert_eq!(s.result, "win");
    }

    #[test]
    fn equal_scores_are_a_draw() {
        let mut m = Match::new("Bushido");
        m.observe(&state(6, 2, 2, json!([me(0, 1), them("x", 1, 100)])));
        assert_eq!(m.finish(300).unwrap().result, "draw");
    }

    #[test]
    fn the_team_size_is_the_largest_ever_seen() {
        // A player who leaves disappears from the last state. Without the
        // maximum, a 3v3 would end up recorded as a 2v2.
        let mut m = Match::new("Bushido");
        m.observe(&state(
            13,
            0,
            0,
            json!([
                me(0, 0),
                them("a", 0, 1),
                them("b", 0, 1),
                them("c", 1, 1),
                them("d", 1, 1),
                them("e", 1, 1)
            ]),
        ));
        m.observe(&state(
            13,
            1,
            0,
            json!([me(0, 1), them("a", 0, 1), them("c", 1, 1), them("d", 1, 1)]),
        ));
        assert_eq!(m.finish(300).unwrap().team_size, 3);
    }

    #[test]
    fn mvp_is_the_best_score_of_the_winning_team() {
        let mut m = Match::new("Bushido");
        m.observe(&state(
            13,
            3,
            1,
            json!([me(0, 2), them("a", 0, 300), them("b", 1, 500)]),
        ));
        // The member has 420, his team-mate 300, the opponent 500 but loses.
        assert!(m.finish(300).unwrap().mvp);
    }

    #[test]
    fn there_is_no_mvp_without_a_win() {
        let mut m = Match::new("Bushido");
        m.observe(&state(13, 1, 3, json!([me(0, 0), them("a", 1, 100)])));
        let s = m.finish(300).unwrap();
        assert_eq!(s.result, "loss");
        assert!(!s.mvp);
    }

    #[test]
    fn a_match_without_the_member_is_refused() {
        // His row can be missing from the last state, typically if he leaves
        // before the end. ke-rl-tracker's production code keeps `if our_player:`
        // for that reason. An invented zero would be indistinguishable from a
        // real one.
        let mut m = Match::new("Bushido");
        m.observe(&state(
            13,
            4,
            2,
            json!([them("a", 0, 100), them("b", 1, 100)]),
        ));
        // Changed: `finish` now returns a `Result`, and the exact variant
        // proves it really is the member's absence that is to blame, not the
        // absence of an opponent (both sides have a player).
        assert_eq!(m.finish(300), Err(Refusal::MemberNotFound));
    }

    #[test]
    fn the_member_is_found_whatever_the_case_and_the_spacing() {
        // The exact format of the Name field has never been checked against the
        // real game. A case slip in the setting is the likeliest mistake, and
        // without the tolerance it would produce NO match at all, saying
        // nothing. Changed: an opponent is added, otherwise the new "there must
        // be an opponent" rule would refuse the match before even looking at
        // the name.
        for written in ["bushido", "BUSHIDO", "  Bushido  "] {
            let mut m = Match::new(written);
            m.observe(&state(13, 4, 2, json!([me(0, 2), them("x", 1, 100)])));
            assert!(m.finish(300).is_ok(), "{written} should have matched");
        }
    }

    #[test]
    fn the_names_seen_are_kept_for_the_local_log_only() {
        // When the member is never found, he must be able to read in his own
        // log under what name the game refers to him. Those names go nowhere:
        // no payload carries them, as the pinning tests in mod.rs prove.
        let mut m = Match::new("Personne");
        m.observe(&state(
            13,
            1,
            0,
            json!([me(0, 1), them("Adversaire", 1, 10)]),
        ));
        // Changed: `finish` now returns a `Result`.
        assert!(m.finish(300).is_err(), "an absent member refuses the match");
        assert_eq!(m.names_seen(), vec!["Adversaire", "Bushido"]);
    }

    #[test]
    fn a_match_never_observed_is_refused() {
        // Changed: the exact variant proves we do tell "never observed" apart
        // from the other refusals, now that `finish` returns a `Result`.
        assert_eq!(
            Match::new("Bushido").finish(300),
            Err(Refusal::NeverObserved)
        );
    }

    #[test]
    fn a_training_playlist_is_refused() {
        // Changed: an opponent is added to the scoresheet. Without him, the new
        // "there must be an opponent" rule would refuse the match with
        // `NoOpponent` before even looking at the playlist, and this test would
        // no longer prove anything about the playlist itself.
        for playlist in [0, 9, 19, 21, 73] {
            let mut m = Match::new("Bushido");
            m.observe(&state(playlist, 0, 0, json!([me(0, 0), them("x", 1, 0)])));
            assert_eq!(
                m.finish(60),
                Err(Refusal::TrainingPlaylist(playlist)),
                "playlist {playlist} had to be refused"
            );
        }
    }

    #[test]
    fn a_ranked_playlist_is_recognised() {
        for playlist in [10, 11, 13, 27, 28, 29, 30] {
            assert!(is_ranked(playlist), "{playlist} is ranked");
        }
        for playlist in [1, 2, 3, 6, 22, 24] {
            assert!(!is_ranked(playlist), "{playlist} is not ranked");
        }
    }

    #[test]
    fn the_live_state_carries_the_member_and_nobody_else() {
        let mut m = Match::new("Bushido");
        m.observe(&state(
            13,
            2,
            1,
            json!([me(0, 1), them("Adversaire", 1, 999)]),
        ));
        let live = m.live().expect("an observed match has a state");
        assert_eq!(live.team_blue_score, 2);
        assert_eq!(live.team_orange_score, 1);
        assert_eq!(live.goals, 1);
        assert_eq!(live.score, 420, "the member's score, not the opponent's");
    }

    #[test]
    fn players_are_read_from_the_root_not_from_game() {
        // The original Python agent looks for Data.Teams and Data.Players in
        // the wrong place. If we copied its mistake, nothing would ever be read.
        //
        // Changed: an opponent is added at the root, otherwise the new "there
        // must be an opponent" rule would refuse the match (a single player, on
        // the blue side, in the root's Players) before proving anything about
        // which location is read.
        let mut m = Match::new("Bushido");
        m.observe(&json!({
            "MatchGuid": "g",
            "Game": {"Playlist": 13, "TimeSeconds": 10, "bOvertime": false,
                     "Teams": [{"TeamNum": 0, "Score": 1}, {"TeamNum": 1, "Score": 0}],
                     "Players": [me(1, 9)]},
            "Players": [me(0, 1), them("x", 1, 0)],
        }));
        let s = m.finish(300).unwrap();
        assert_eq!(s.player_team, 0, "the root's Players is what counts");
        assert_eq!(s.goals, 1);
    }

    #[test]
    fn a_guid_opens_the_match_and_the_same_guid_does_not_reopen_it() {
        let mut m = Match::new("Bushido");
        assert!(m.guid().is_none());
        m.observe(&state(13, 0, 0, json!([me(0, 0)])));
        assert_eq!(m.guid().as_deref(), Some("g-1"));
    }

    #[test]
    fn overtime_is_carried_to_the_live_state() {
        let mut m = Match::new("Bushido");
        let mut s = state(13, 3, 3, json!([me(0, 1)]));
        s["Game"]["bOvertime"] = json!(true);
        m.observe(&s);
        assert!(m.live().unwrap().overtime);
    }

    #[test]
    fn the_names_seen_stop_growing_long_before_anything_silly() {
        let mut m = Match::new("Personne");
        for n in 0..100 {
            m.observe(&state(13, 0, 0, json!([them(&format!("joueur{n}"), 0, 1)])));
        }
        assert_eq!(m.names_seen().len(), MAX_NAMES_SEEN);
    }

    fn named(name: &str, team: i64, score: i64, goals: i64) -> serde_json::Value {
        json!({"Name": name, "TeamNum": team, "Goals": goals,
               "Assists": 1, "Saves": 0, "Shots": 2, "Score": score, "Demos": 0})
    }

    #[test]
    fn les_autres_joueurs_sont_gardes_sans_nom_par_equipe_et_score() {
        let mut m = Match::new("Bushido");
        m.observe(&state(
            -1,
            3,
            1,
            json!([
                me(0, 2),
                named("Zed", 1, 150, 1),
                named("Alpha", 0, 300, 1),
                named("Moe", 1, 400, 0),
            ]),
        ));
        let s = m.finish(300).unwrap();
        assert_eq!(s.match_key.as_deref(), Some(match_key("g-1").as_str()));
        let order: Vec<(i64, i64)> = s.others.iter().map(|o| (o.team, o.score)).collect();
        assert_eq!(order, vec![(0, 300), (1, 400), (1, 150)]);
        assert!(s.others.iter().all(|o| !o.left));
    }

    #[test]
    fn un_joueur_parti_garde_sa_derniere_ligne() {
        let mut m = Match::new("Bushido");
        m.observe(&state(
            -1,
            0,
            0,
            json!([
                me(0, 0),
                named("Fuyard", 1, 80, 0),
                named("Reste", 1, 50, 0)
            ]),
        ));
        m.observe(&state(
            -1,
            1,
            0,
            json!([me(0, 1), named("Reste", 1, 60, 0)]),
        ));
        let s = m.finish(300).unwrap();
        let fuyard = s
            .others
            .iter()
            .find(|o| o.score == 80)
            .expect("le fuyard est garde");
        assert!(fuyard.left);
        let reste = s
            .others
            .iter()
            .find(|o| o.score == 60)
            .expect("la ligne la plus recente");
        assert!(!reste.left);
    }

    #[test]
    fn sans_match_guid_ni_cle_ni_autres_joueurs() {
        let mut frame = state(-1, 1, 0, json!([me(0, 1), named("X", 1, 10, 0)]));
        frame.as_object_mut().unwrap().remove("MatchGuid");
        let mut m = Match::new("Bushido");
        m.observe(&frame);
        let s = m.finish(300).unwrap();
        assert_eq!(s.match_key, None);
        assert!(s.others.is_empty());
    }

    #[test]
    fn la_cle_est_stable_et_distincte() {
        assert_eq!(match_key("abc"), match_key("abc"));
        assert_ne!(match_key("abc"), match_key("abd"));
        let k = match_key("abc");
        assert_eq!(k.len(), 64);
        assert!(k
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
    }

    #[test]
    fn au_plus_sept_autres_joueurs_les_presents_d_abord() {
        // 4v4 : sept autres joueurs a la fois. Deux coequipiers quittent la
        // partie et sont remplaces, neuf autres joueurs vus en tout.
        let mut m = Match::new("Bushido");
        m.observe(&state(
            -1,
            0,
            0,
            json!([
                me(0, 0),
                named("a", 0, 10, 0),
                named("b", 0, 20, 0),
                named("c", 0, 30, 0),
                named("d", 1, 40, 0),
                named("e", 1, 50, 0),
                named("f", 1, 60, 0),
                named("g", 1, 70, 0),
            ]),
        ));
        m.observe(&state(
            -1,
            0,
            0,
            json!([
                me(0, 0),
                named("h", 0, 5, 0),
                named("i", 0, 6, 0),
                named("c", 0, 30, 0),
                named("d", 1, 40, 0),
                named("e", 1, 50, 0),
                named("f", 1, 60, 0),
                named("g", 1, 70, 0),
            ]),
        ));
        let s = m.finish(300).unwrap();
        assert_eq!(s.others.len(), 7);
        assert!(
            s.others.iter().all(|o| !o.left),
            "les partis cedent leur place aux presents"
        );
    }
}
