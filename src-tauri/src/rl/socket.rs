//! Le client TCP qui écoute Rocket League.
//!
//! Le jeu est le serveur, nous sommes le client. Un fil bloquant dédié, comme
//! le suiveur de logs de Hearthstone : `tokio` est compilé sans la
//! fonctionnalité `net` dans ce dépôt, et ce module n'a aucune raison d'exiger
//! qu'on l'ajoute.

use std::collections::BTreeSet;
use std::io::Read;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;

/// Si un fil de suivi est actuellement connecté à la socket du jeu.
static CONNECTED: AtomicBool = AtomicBool::new(false);

/// Si un fil de suivi est actuellement connecté à la socket du jeu.
pub fn connected() -> bool {
    CONNECTED.load(Ordering::SeqCst)
}

/// Combien de messages du jeu ont été décodés depuis le lancement.
///
/// Un compteur qui monte prouve que la socket, le découpage des trames et le
/// décodage fonctionnent. S'il reste à zéro alors que la connexion est
/// établie, le jeu ne parle pas le protocole qu'on attend, et c'est une
/// information qu'aucune autre trace ne donne.
static DECODED: AtomicU64 = AtomicU64::new(0);

pub fn decoded() -> u64 {
    DECODED.load(Ordering::Relaxed)
}

/// Les noms d'évènements déjà journalisés parce qu'on ne les traite pas.
///
/// Plafonné comme `names_seen` dans `parser.rs` : cet ensemble existe pour
/// écrire une ligne de journal une fois chacun, pas pour grossir.
static UNHANDLED_LOGGED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// Les évènements du cycle de vie qui peuvent ouvrir un match. Le GUID n'est pas
/// disponible à `MatchCreated`, d'où les replis.
const OPENS: &[&str] = &["MatchInitialized", "CountdownBegin", "RoundStarted"];

/// Ce que la boucle rend à son appelant.
pub enum Event {
    /// Un match s'ouvre.
    Open,
    /// Un état de match, à prendre en compte.
    State(Value),
    /// Le match se termine.
    Close,
}

/// Se connecte et suit la socket jusqu'à ce que `stop` passe à vrai.
///
/// `on` est appelé pour chaque évènement utile. La reconnexion est automatique :
/// le jeu peut être lancé avant nous, ou redémarré sous nous.
pub fn follow(port: u16, stop: &AtomicBool, mut on: impl FnMut(Event)) {
    log::info!("rl: watching for the game's stats socket on 127.0.0.1:{port}");
    let mut announced = false;
    while !stop.load(Ordering::SeqCst) {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => {
                log::info!("rl: connected to the stats socket on {port}");
                CONNECTED.store(true, Ordering::SeqCst);
                announced = false;
                read_until_closed(stream, stop, &mut on);
                CONNECTED.store(false, Ordering::SeqCst);
                log::info!("rl: stats socket closed");
            }
            Err(e) => {
                if !announced {
                    log::info!(
                        "rl: the stats socket is not open on port {port}. Rocket League only opens it \
                         when the config block is present AND the game was started afterwards ({e})"
                    );
                    announced = true;
                } else {
                    log::debug!("rl: stats socket not there yet ({e})");
                }
            }
        }
        for _ in 0..50 {
            if stop.load(Ordering::SeqCst) {
                CONNECTED.store(false, Ordering::SeqCst);
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    CONNECTED.store(false, Ordering::SeqCst);
}

fn read_until_closed(mut stream: TcpStream, stop: &AtomicBool, on: &mut impl FnMut(Event)) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));

    let mut buf: Vec<u8> = Vec::with_capacity(1 << 16);
    let mut chunk = [0u8; 8192];

    while !stop.load(Ordering::SeqCst) {
        match stream.read(&mut chunk) {
            Ok(0) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => {
                log::warn!("rl: read failed: {e}");
                return;
            }
        }

        loop {
            let Some((frame, rest)) = super::frames::take_frame(&buf) else {
                break;
            };
            let frame = frame.to_vec();
            buf = rest.to_vec();

            let Some((name, data)) = super::frames::decode(&frame) else {
                continue;
            };
            DECODED.fetch_add(1, Ordering::Relaxed);
            match name.as_str() {
                n if OPENS.contains(&n) => on(Event::Open),
                "UpdateState" => on(Event::State(data)),
                "MatchDestroyed" | "MatchEnded" => on(Event::Close),
                _ => {
                    // Un nom d'évènement qu'on ne traite pas. Journalisé UNE
                    // fois chacun : si le jeu nomme autrement le début ou la
                    // fin d'un match, c'est ici qu'on le verra, et nulle part
                    // ailleurs.
                    if let Ok(mut seen) = UNHANDLED_LOGGED.lock() {
                        if seen.len() < 32 && seen.insert(name.clone()) {
                            log::info!("rl: unhandled event from the game: {name}");
                        }
                    }
                }
            }
        }

        // Un tampon qui enfle sans jamais livrer un objet complet veut dire que
        // le flux n'est pas ce qu'on croit. Mieux vaut repartir propre que
        // grossir sans fin.
        if buf.len() > 8 << 20 {
            log::warn!("rl: buffer grew past 8 MiB without a whole object, dropping it");
            buf.clear();
        }
    }
}
