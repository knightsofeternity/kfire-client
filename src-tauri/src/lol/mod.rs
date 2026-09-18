//! Suivi de League of Legends : le jeu ouvre lui-même une API sur la machine du
//! joueur pendant une partie, et le client de bureau la lit.
//!
//! Ce qui NE quitte JAMAIS cette machine, par conception : `allgamedata` nomme
//! les dix participants de la partie, avec leurs Riot ID, leurs équipes, leurs
//! objets et l'historique des évènements. Rien de tout cela ne sort. Le nom du
//! membre lui-même ne sort pas non plus : il sert uniquement, localement, à
//! retrouver sa ligne dans la liste des joueurs (voir `parser::find_member`).
//! Seuls des faits sur le membre sortent, et la liste exacte est épinglée par
//! un test.

pub mod api;
pub mod parser;

use serde_json::{json, Value};

/// Le slug du catalogue, celui sous lequel le serveur connaît ce jeu.
pub const SLUG: &str = "league-of-legends";

/// L'état diffusé pendant la partie, et rien d'autre.
///
/// Jamais mis en file, jamais rejoué, jamais écrit. Il ne nomme personne : la
/// réponse dont il est tiré porte le Riot ID des dix joueurs, et le fait que ce
/// message ne soit pas stocké ne rendrait pas acceptable de les diffuser à
/// toute la guilde. La validation du serveur refuse de toute façon tout champ
/// non prévu : une clé de plus ici, et plus rien ne passe.
pub fn live_payload(l: &parser::Live, slug: &str) -> Value {
    json!({
        "game_slug": slug,
        "champion": l.champion,
        "level": l.level,
        "kills": l.kills,
        "deaths": l.deaths,
        "assists": l.assists,
        "creep_score": l.creep_score,
        "gold": l.gold,
        "game_time_seconds": l.game_time_seconds,
    })
}

/// La fin d'une partie, diffusée pour que la carte s'efface tout de suite.
///
/// Sans elle le portail n'apprend la fin qu'à l'expiration de son propre
/// minuteur, et un score figé resterait affiché sur la page en direct de la
/// guilde. Le serveur comprend cette forme génériquement, pour tous les jeux.
pub fn ended_payload(slug: &str) -> Value {
    json!({ "game_slug": slug, "ended": true })
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// À quelle cadence au plus l'état du direct est publié.
///
/// C'est aussi la cadence de sondage de l'API : chaque publication coûte une
/// requête HTTP au jeu, et rien dans la charge utile ne bouge plus vite qu'une
/// seconde (le temps de jeu est en secondes, l'or et les scores changent moins
/// souvent que ça).
const LIVE_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

/// Vrai tant qu'aucun fil de suivi ne doit tourner. Un seul drapeau suffit : le
/// jeu tourne au plus une fois, et le scanner ne signale jamais deux démarrages
/// sans un arrêt entre les deux.
static STOP: AtomicBool = AtomicBool::new(true);

/// Si un fil de suivi tourne en ce moment.
pub fn is_watching() -> bool {
    !STOP.load(Ordering::SeqCst)
}

/// Demande au fil de suivi de s'arrêter.
pub fn stop_watching() {
    STOP.store(true, Ordering::SeqCst);
}

/// Commence à sonder l'API locale, sauf si le membre n'a pas activé le suivi.
///
/// Contrairement à Rocket League, aucun pseudo n'est demandé : `activePlayer`
/// dit qui est le joueur local, donc le membre n'a rien à saisir et rien à se
/// tromper.
pub fn start_watching(db: Arc<crate::db::Db>, live: tokio::sync::watch::Sender<Option<String>>) {
    if db.get_setting("lol_enabled").as_deref() != Some("1") {
        log::info!("lol: tracking is off in the settings, not watching");
        return;
    }
    // swap rend la valeur PRÉCÉDENTE : true veut dire qu'on était arrêté, donc
    // on démarre.
    if !STOP.swap(false, Ordering::SeqCst) {
        return; // déjà en train de tourner
    }
    let Some(client) = api::client() else {
        STOP.store(true, Ordering::SeqCst);
        return;
    };
    log::info!("lol: watching for the game's live API on {}", api::BASE);

    // Une tâche asynchrone, contrairement aux fils bloquants de Hearthstone et
    // Rocket League : ici on parle HTTP, et le client HTTP du projet est
    // asynchrone.
    tauri::async_runtime::spawn(async move {
        let mut last_sent: Option<String> = None;
        // Vrai dès qu'une partie a été vue, pour ne pas annoncer une fin qui
        // n'a jamais eu de début (le jeu tourne depuis le menu, où l'API
        // n'existe pas).
        let mut in_game = false;

        // Garde en UN seul endroit la mémoire de ce qui a été envoyé, ce qui
        // est ce qui fait tenir le « seulement si ça a changé » aussi bien pour
        // l'état du direct que pour la fin de la partie.
        let broadcast = |payload: Value, last_sent: &mut Option<String>| {
            let raw = payload.to_string();
            // Le jeu est sondé sur un minuteur, donc la plupart des passages
            // voient exactement le même état.
            if last_sent.as_deref() == Some(raw.as_str()) {
                return;
            }
            *last_sent = Some(raw);
            let env = json!({
                "type": "live_match",
                "ts": chrono::Utc::now().to_rfc3339(),
                "payload": payload,
            });
            let _ = live.send(Some(env.to_string()));
        };

        while !STOP.load(Ordering::SeqCst) {
            match api::fetch_all_game_data(&client).await {
                Some(raw) => match parser::parse(&raw) {
                    Some(l) => {
                        in_game = true;
                        broadcast(live_payload(&l, SLUG), &mut last_sent);
                    }
                    // La forme attendue n'y est pas encore : l'API répond dès
                    // l'écran de chargement, avant que la liste des joueurs
                    // soit remplie. Rien à signaler.
                    None => log::debug!("lol: the live API answered without a usable game state"),
                },
                None => {
                    if in_game {
                        // L'API a disparu : la partie est finie. On le dit tout
                        // de suite plutôt que de laisser le minuteur du serveur
                        // s'en apercevoir.
                        broadcast(ended_payload(SLUG), &mut last_sent);
                        in_game = false;
                        log::info!("lol: the live API is gone, the game is over");
                    }
                }
            }
            sleep_until_stopped(LIVE_EVERY).await;
        }
        STOP.store(true, Ordering::SeqCst);
    });
}

/// Attend, en revenant tout de suite si on nous demande de nous arrêter.
///
/// Découpé en petits pas pour que quitter le jeu, ou couper le suivi dans les
/// réglages, arrête la boucle sans attendre le sondage suivant.
async fn sleep_until_stopped(total: std::time::Duration) {
    let step = std::time::Duration::from_millis(100);
    let mut slept = std::time::Duration::ZERO;
    while slept < total {
        if STOP.load(Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(step).await;
        slept += step;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_live() -> parser::Live {
        parser::Live {
            champion: "Ahri".into(),
            level: 11,
            kills: 7,
            deaths: 2,
            assists: 9,
            creep_score: 142,
            gold: 8350,
            game_time_seconds: 843,
        }
    }

    #[test]
    fn le_direct_ne_porte_exactement_que_les_neuf_champs_autorises() {
        // LE test de ce module : il épingle la liste exacte des clés qui
        // quittent cette machine vers tous les membres de la guilde. La réponse
        // dont elles sont tirées nomme les dix joueurs de la partie, porte
        // leurs équipes, leurs objets et l'historique des évènements. Si cette
        // liste s'allonge un jour, elle s'allonge exprès. La validation du
        // serveur refuse de toute façon tout champ non prévu.
        let v = live_payload(&a_live(), SLUG);
        let o = v.as_object().expect("un objet");
        let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "assists",
                "champion",
                "creep_score",
                "deaths",
                "game_slug",
                "game_time_seconds",
                "gold",
                "kills",
                "level",
            ]
        );
        assert_eq!(o["game_slug"], SLUG);
        assert_eq!(o["champion"], "Ahri");
        assert_eq!(o["level"], 11);
        assert_eq!(o["kills"], 7);
        assert_eq!(o["deaths"], 2);
        assert_eq!(o["assists"], 9);
        assert_eq!(o["creep_score"], 142);
        assert_eq!(o["gold"], 8350);
        assert_eq!(o["game_time_seconds"], 843);
    }

    #[test]
    fn le_direct_ne_porte_le_nom_de_personne() {
        // Ni celui du membre, ni celui des neuf autres. Le nom du membre sert
        // uniquement, localement, à retrouver sa ligne.
        let raw = live_payload(&a_live(), SLUG).to_string();
        for forbidden in [
            "riotId",
            "summonerName",
            "riotIdGameName",
            "riotIdTagLine",
            "allPlayers",
            "team",
            "items",
            "events",
            "#",
        ] {
            assert!(!raw.contains(forbidden), "{forbidden} a fuité dans {raw}");
        }
    }

    #[test]
    fn le_direct_construit_depuis_une_vraie_reponse_ne_porte_rien_de_plus() {
        // Bout en bout : du JSON du jeu, qui nomme tout le monde, jusqu'à ce
        // qui part. Aucun nom d'adversaire ne doit survivre au trajet.
        let raw = serde_json::json!({
            "activePlayer": {"currentGold": 500.5, "level": 6, "riotId": "Ouranos#KOE"},
            "allPlayers": [
                {
                    "championName": "Garen",
                    "level": 7,
                    "team": "CHAOS",
                    "riotId": "Adversaire#EUW",
                    "scores": {"assists": 0, "creepScore": 55, "deaths": 1, "kills": 4}
                },
                {
                    "championName": "Ahri",
                    "level": 6,
                    "team": "ORDER",
                    "riotId": "Ouranos#KOE",
                    "scores": {"assists": 2, "creepScore": 61, "deaths": 1, "kills": 3}
                }
            ],
            "events": {"Events": [{"EventName": "ChampionKill", "KillerName": "Adversaire#EUW"}]},
            "gameData": {"gameMode": "CLASSIC", "gameTime": 300.0}
        })
        .to_string();
        let l = parser::parse(&raw).expect("une partie lisible");
        let sent = live_payload(&l, SLUG).to_string();
        for forbidden in [
            "Ouranos",
            "Adversaire",
            "KOE",
            "EUW",
            "Garen",
            "CHAOS",
            "ORDER",
        ] {
            assert!(!sent.contains(forbidden), "{forbidden} a fuité dans {sent}");
        }
        assert!(sent.contains("Ahri"), "le champion du membre doit partir");
    }

    #[test]
    fn la_fin_dune_partie_ne_porte_que_le_jeu_et_le_drapeau() {
        let v = ended_payload(SLUG);
        let o = v.as_object().expect("un objet");
        let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, vec!["ended", "game_slug"]);
        assert_eq!(o["game_slug"], SLUG);
        assert_eq!(o["ended"], true);
    }

    #[test]
    fn le_slug_est_celui_que_le_serveur_connait() {
        assert_eq!(SLUG, "league-of-legends");
    }
}
