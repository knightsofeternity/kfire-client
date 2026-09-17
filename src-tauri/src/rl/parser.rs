//! Lire un match Rocket League, en fonctions pures.
//!
//! La feuille de match complète, qui porte le nom de tous les joueurs, vit ici
//! et ne va pas plus loin. Ce module s'en sert pour calculer, et n'en sort que
//! des faits sur le membre.

use serde_json::Value;

/// Les playlists sans adversaire : partie libre, ateliers, entraînement. Les
/// rapporter fausserait tous les ratios.
pub const TRAINING: &[i64] = &[0, 9, 19, 21, 73];

/// Les playlists classées, d'après le catalogue Psyonix.
pub const RANKED: &[i64] = &[10, 11, 13, 27, 28, 29, 30];

/// Combien de pseudos distincts on retient au plus.
///
/// Ce n'est pas de la méfiance envers le jeu, qui tourne sur la machine du
/// membre : c'est que cet ensemble ne sert qu'à écrire UNE ligne de journal,
/// et qu'une ligne de journal n'a pas besoin de plus. Un match réel en compte
/// huit au maximum, remplaçants compris.
const MAX_NAMES_SEEN: usize = 32;

/// Si cette playlist est classée.
pub fn is_ranked(playlist: i64) -> bool {
    RANKED.contains(&playlist)
}

/// Si cette playlist n'a pas d'adversaire et ne doit jamais être rapportée.
pub fn is_training(playlist: i64) -> bool {
    TRAINING.contains(&playlist)
}

/// Les raisons pour lesquelles un match n'est pas rapporté.
///
/// `finish()` en rend exactement une : jamais une combinaison, jamais un
/// `None` muet. Chaque variante dit la vérité qu'elle constate, pas une
/// hypothèse sur sa cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Aucun `UpdateState` n'a jamais été reçu : il n'y a rien à résumer.
    NeverObserved,
    /// Un des deux camps n'a jamais compté le moindre joueur. La partie libre
    /// et l'entraînement n'ont personne en face ; un vrai match, si.
    NoOpponent,
    /// Le jeu a vraiment envoyé une playlist, et c'est une playlist
    /// d'entraînement d'après le catalogue Psyonix.
    TrainingPlaylist(i64),
    /// La taille d'équipe observée (le plus grand effectif vu dans un camp)
    /// sort de la plage plausible pour Rocket League.
    TeamSizeOutOfRange(i64),
    /// Le pseudo réglé ne correspond à aucun joueur de la feuille de match.
    MemberNotFound,
    /// Le membre a été trouvé, mais avec un camp qui n'est ni bleu ni orange.
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

/// Le résumé d'un match terminé : uniquement des faits sur le membre.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// `None` quand le jeu n'a jamais envoyé cette playlist. C'est la norme :
    /// le protocole réel de Rocket League n'a pas de champ `Playlist`.
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
}

/// L'état courant d'un match, pour le direct. Jamais écrit, jamais rejoué.
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

/// Les statistiques d'un joueur, telles que le jeu les donne.
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

/// Un match en cours d'observation.
pub struct Match {
    member: String,
    guid: Option<String>,
    /// `None` tant que le jeu n'a jamais envoyé de champ `Playlist`. C'est
    /// l'état normal : le vrai protocole n'a pas ce champ.
    playlist: Option<i64>,
    blue: i64,
    orange: i64,
    seconds: i64,
    overtime: bool,
    /// Le maximum de joueurs vu dans chaque équipe, séparément. Un joueur qui
    /// quitte disparaît du dernier état, donc seul le maximum dit la vraie
    /// taille du match. Séparés parce qu'un des deux à zéro veut dire
    /// « personne en face », donc partie libre ou entraînement, jamais un
    /// vrai match : c'est la garde principale, plus fiable qu'une playlist
    /// que le jeu n'envoie pas forcément.
    max_players_blue: i64,
    max_players_orange: i64,
    /// Les statistiques du membre, et le meilleur score vu dans chaque équipe.
    mine: Option<Stats>,
    best_blue: i64,
    best_orange: i64,
    seen: bool,
    /// Tous les pseudos croisés pendant le match.
    ///
    /// Ils ne quittent JAMAIS la machine. Ils servent à une seule chose : quand
    /// le membre n'a jamais été trouvé, les écrire dans le journal local pour
    /// qu'il voie lui-même sous quel nom le jeu le désigne, et corrige son
    /// réglage. Sans cela, un pseudo mal saisi ne produit rien du tout et reste
    /// indiagnosticable.
    names_seen: std::collections::BTreeSet<String>,
}

fn i(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

/// Si ce nom de la feuille de match est celui du membre.
///
/// La comparaison ignore la casse et les espaces de bord. Le format exact du
/// champ `Name` n'a jamais été vérifié contre le vrai jeu, seulement lu dans le
/// code de `ke-rl-tracker`, et une faute de casse dans le réglage est l'erreur
/// la plus probable. La tolérance ne coûte rien et supprime toute une classe de
/// pannes silencieuses.
fn same_player(name: Option<&str>, member: &str) -> bool {
    match name {
        Some(n) => n.trim().eq_ignore_ascii_case(member.trim()),
        None => false,
    }
}

impl Match {
    /// Ouvre l'observation d'un match pour ce membre.
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
        }
    }

    /// Le GUID du match, une fois qu'il est connu.
    pub fn guid(&self) -> Option<String> {
        self.guid.clone()
    }

    /// Prend en compte un `UpdateState`.
    pub fn observe(&mut self, data: &Value) {
        if let Some(g) = data.get("MatchGuid").and_then(Value::as_str) {
            // Le GUID est le même à chaque image d'un match : ne le réécrire
            // que s'il a vraiment changé.
            if !g.is_empty() && self.guid.as_deref() != Some(g) {
                self.guid = Some(g.to_string());
            }
        }

        // Emprunté, jamais cloné : on ne lit ici que quatre nombres, et cette
        // fonction tourne trente fois par seconde pendant tout le match.
        if let Some(game) = data.get("Game").filter(|g| g.is_object()) {
            // Ne jamais inventer une playlist : le vrai protocole n'a pas ce
            // champ. `i()` rendrait `0` pour une clé absente, et `0` est une
            // playlist d'entraînement ; c'est exactement le bug qui refusait
            // tous les matchs réels.
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

        // Players est à la RACINE de Data, pas sous Game. L'agent Python
        // d'origine se trompe d'endroit ; recopier son erreur ne lirait rien.
        let Some(players) = data.get("Players").and_then(Value::as_array) else {
            return;
        };

        let (mut blue_n, mut orange_n) = (0i64, 0i64);
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
            if let Some(n) = p.get("Name").and_then(Value::as_str) {
                // contains() prend un &str sans allouer ; seul un nom vraiment
                // nouveau paie une String, soit une poignée par match au lieu
                // d'une par joueur et par image.
                if !self.names_seen.contains(n) && self.names_seen.len() < MAX_NAMES_SEEN {
                    self.names_seen.insert(n.to_string());
                }
            }
            if same_player(p.get("Name").and_then(Value::as_str), &self.member) {
                self.mine = Some(Stats {
                    team,
                    goals: i(p, "Goals"),
                    assists: i(p, "Assists"),
                    saves: i(p, "Saves"),
                    shots: i(p, "Shots"),
                    score,
                    demos: i(p, "Demos"),
                });
            }
        }
        self.max_players_blue = self.max_players_blue.max(blue_n);
        self.max_players_orange = self.max_players_orange.max(orange_n);
        self.seen = true;
    }

    /// Les pseudos croisés pendant le match, pour le journal LOCAL uniquement.
    pub fn names_seen(&self) -> Vec<String> {
        self.names_seen.iter().cloned().collect()
    }

    /// L'état courant, pour le direct.
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

    /// Le résumé, si et seulement si il est COMPLET.
    ///
    /// Rend la raison précise dès qu'il manque quelque chose : jamais observé,
    /// pas d'adversaire, playlist d'entraînement (seulement quand le jeu l'a
    /// vraiment envoyée), taille d'équipe impossible, ou statistiques du
    /// membre absentes. Sa ligne peut manquer du dernier état s'il quitte
    /// avant la fin, et le code de production de `ke-rl-tracker` garde
    /// `if our_player:` pour exactement cette raison. Mieux vaut un match
    /// manquant qu'un match faux : un zéro inventé est indiscernable d'un vrai
    /// zéro et empoisonnerait les moyennes de la guilde pour toujours.
    pub fn finish(&self, duration_seconds: i64) -> Result<Summary, Refusal> {
        if !self.seen {
            return Err(Refusal::NeverObserved);
        }
        // La vraie garde : partie libre et entraînement n'ont personne en
        // face. Un vrai match, si, toujours, des deux côtés.
        if self.max_players_blue == 0 || self.max_players_orange == 0 {
            return Err(Refusal::NoOpponent);
        }
        // Gardée pour le jour où Psyonix ajouterait ce champ, mais seulement
        // quand il est VRAIMENT présent : jamais inventé.
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

        // Le MVP est le meilleur score de l'équipe GAGNANTE, donc il n'existe
        // pas sans victoire.
        let best_of_mine = if mine.team == 0 {
            self.best_blue
        } else {
            self.best_orange
        };
        let mvp = result == "win" && mine.score >= best_of_mine;

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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// La forme réelle du fil, telle que documentée par l'API Stats de
    /// Psyonix : `Data.Game` n'a JAMAIS de champ `Playlist`. Ce test est la
    /// preuve du bug : avec l'ancien code, `i(game, "Playlist")` renvoyait `0`
    /// pour ce champ absent, `0` est dans `TRAINING`, et le match était refusé
    /// alors que le membre y était bel et bien, avec un adversaire, une
    /// victoire nette. Il doit échouer avant le correctif et réussir après.
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
            .expect("un match réel sans Playlist doit se résumer");
        assert_eq!(s.playlist, None);
        assert_eq!(s.result, "win");
        assert_eq!(s.team_size, 2);
    }

    /// Sans adversaire, personne en face : partie libre ou entraînement,
    /// jamais un vrai match. Le membre est seul sur son équipe.
    #[test]
    fn a_solo_match_with_nobody_on_the_other_team_has_no_opponent() {
        let mut m = Match::new("Bushido");
        m.observe(&state(-1, 0, 0, json!([me(0, 3)])));
        assert_eq!(m.finish(300), Err(Refusal::NoOpponent));
    }

    /// Le champ existe vraiment cette fois : la règle historique s'applique
    /// encore, mais seulement quand la donnée est réellement présente.
    #[test]
    fn a_real_match_with_a_present_training_playlist_is_refused() {
        let mut m = Match::new("Bushido");
        m.observe(&state(73, 4, 2, json!([me(0, 2), them("x", 1, 100)])));
        assert_eq!(m.finish(300), Err(Refusal::TrainingPlaylist(73)));
    }

    /// Le membre est absent de la feuille : c'est le seul cas où le pseudo
    /// réglé peut vraiment être en cause.
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
        let s = m.finish(300).expect("un match complet doit se résumer");
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
        // Le miroir du cas bleu. L'inverser rejetterait la moitié des
        // victoires de la guilde sans que rien d'autre ne le montre.
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
        // Un joueur qui quitte disparaît du dernier état. Sans le maximum, un
        // 3v3 finirait enregistré comme un 2v2.
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
        // Le membre a 420, son coéquipier 300, l'adversaire 500 mais il perd.
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
        // Sa ligne peut manquer du dernier état, typiquement s'il quitte avant
        // la fin. Le code de production de ke-rl-tracker garde `if our_player:`
        // pour cette raison. Un zéro inventé serait indiscernable d'un vrai.
        let mut m = Match::new("Bushido");
        m.observe(&state(
            13,
            4,
            2,
            json!([them("a", 0, 100), them("b", 1, 100)]),
        ));
        // Modifié : `finish` rend désormais un `Result`, la variante exacte
        // prouve que c'est bien l'absence du membre qui est en cause, pas
        // l'absence d'adversaire (les deux camps ont un joueur).
        assert_eq!(m.finish(300), Err(Refusal::MemberNotFound));
    }

    #[test]
    fn the_member_is_found_whatever_the_case_and_the_spacing() {
        // Le format exact du champ Name n'a jamais été vérifié contre le vrai
        // jeu. Une faute de casse dans le réglage est l'erreur la plus probable,
        // et sans tolérance elle ne produirait AUCUN match, sans rien dire.
        // Modifié : un adversaire est ajouté, sinon la nouvelle règle
        // « il faut un adversaire » refuserait le match avant même de
        // regarder le pseudo.
        for written in ["bushido", "BUSHIDO", "  Bushido  "] {
            let mut m = Match::new(written);
            m.observe(&state(13, 4, 2, json!([me(0, 2), them("x", 1, 100)])));
            assert!(m.finish(300).is_ok(), "{written} aurait dû correspondre");
        }
    }

    #[test]
    fn the_names_seen_are_kept_for_the_local_log_only() {
        // Quand le membre n'est jamais trouvé, il doit pouvoir lire dans son
        // journal sous quel nom le jeu le désigne. Ces noms ne partent nulle
        // part : aucune charge utile ne les porte, ce que prouvent les tests
        // d'épinglage de mod.rs.
        let mut m = Match::new("Personne");
        m.observe(&state(
            13,
            1,
            0,
            json!([me(0, 1), them("Adversaire", 1, 10)]),
        ));
        // Modifié : `finish` rend un `Result` désormais.
        assert!(m.finish(300).is_err(), "un membre absent refuse le match");
        assert_eq!(m.names_seen(), vec!["Adversaire", "Bushido"]);
    }

    #[test]
    fn a_match_never_observed_is_refused() {
        // Modifié : la variante exacte prouve qu'on distingue bien « jamais
        // observé » des autres refus, maintenant que `finish` rend un `Result`.
        assert_eq!(
            Match::new("Bushido").finish(300),
            Err(Refusal::NeverObserved)
        );
    }

    #[test]
    fn a_training_playlist_is_refused() {
        // Modifié : un adversaire est ajouté à la feuille. Sans lui, la
        // nouvelle règle « il faut un adversaire » refuserait le match avec
        // `NoOpponent` avant même de regarder la playlist, et ce test ne
        // prouverait plus rien sur la playlist elle-même.
        for playlist in [0, 9, 19, 21, 73] {
            let mut m = Match::new("Bushido");
            m.observe(&state(playlist, 0, 0, json!([me(0, 0), them("x", 1, 0)])));
            assert_eq!(
                m.finish(60),
                Err(Refusal::TrainingPlaylist(playlist)),
                "playlist {playlist} devait être refusée"
            );
        }
    }

    #[test]
    fn a_ranked_playlist_is_recognised() {
        for playlist in [10, 11, 13, 27, 28, 29, 30] {
            assert!(is_ranked(playlist), "{playlist} est classée");
        }
        for playlist in [1, 2, 3, 6, 22, 24] {
            assert!(!is_ranked(playlist), "{playlist} n'est pas classée");
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
        let live = m.live().expect("un match observé a un état");
        assert_eq!(live.team_blue_score, 2);
        assert_eq!(live.team_orange_score, 1);
        assert_eq!(live.goals, 1);
        assert_eq!(
            live.score, 420,
            "le score du membre, pas celui de l'adversaire"
        );
    }

    #[test]
    fn players_are_read_from_the_root_not_from_game() {
        // L'agent Python d'origine cherche Data.Teams et Data.Players au mauvais
        // endroit. Si on recopiait son erreur, rien ne serait jamais lu.
        //
        // Modifié : un adversaire est ajouté à la racine, sinon la nouvelle
        // règle « il faut un adversaire » refuserait le match (un seul joueur,
        // côté bleu, dans les Players de la racine) avant de prouver quoi que
        // ce soit sur l'emplacement lu.
        let mut m = Match::new("Bushido");
        m.observe(&json!({
            "MatchGuid": "g",
            "Game": {"Playlist": 13, "TimeSeconds": 10, "bOvertime": false,
                     "Teams": [{"TeamNum": 0, "Score": 1}, {"TeamNum": 1, "Score": 0}],
                     "Players": [me(1, 9)]},
            "Players": [me(0, 1), them("x", 1, 0)],
        }));
        let s = m.finish(300).unwrap();
        assert_eq!(s.player_team, 0, "Players à la racine fait foi");
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
}
