//! Le client TCP qui écoute Rocket League.
//!
//! Le jeu est le serveur, nous sommes le client. Un fil bloquant dédié, comme
//! le suiveur de logs de Hearthstone : `tokio` est compilé sans la
//! fonctionnalité `net` dans ce dépôt, et ce module n'a aucune raison d'exiger
//! qu'on l'ajoute.

use std::io::Read;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::Value;

/// Si un fil de suivi est actuellement connecté à la socket du jeu.
static CONNECTED: AtomicBool = AtomicBool::new(false);

/// Si un fil de suivi est actuellement connecté à la socket du jeu.
pub fn connected() -> bool {
    CONNECTED.load(Ordering::SeqCst)
}

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
            match name.as_str() {
                n if OPENS.contains(&n) => on(Event::Open),
                "UpdateState" => on(Event::State(data)),
                "MatchDestroyed" | "MatchEnded" => on(Event::Close),
                _ => {}
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
