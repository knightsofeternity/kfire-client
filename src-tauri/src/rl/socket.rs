//! The TCP client that listens to Rocket League.
//!
//! The game is the server, we are the client. A dedicated blocking thread, like
//! the Hearthstone log follower: `tokio` is built without the `net` feature in
//! this repository, and this module has no business demanding that it be added.

use std::collections::BTreeSet;
use std::io::Read;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;

/// Whether a watcher thread is currently connected to the game's socket.
static CONNECTED: AtomicBool = AtomicBool::new(false);

/// Whether a watcher thread is currently connected to the game's socket.
pub fn connected() -> bool {
    CONNECTED.load(Ordering::SeqCst)
}

/// How many messages from the game have been decoded since startup.
///
/// A counter that climbs proves the socket, the frame cutting and the decoding
/// all work. If it stays at zero while the connection is up, the game is not
/// speaking the protocol we expect, and no other trace tells us that.
static DECODED: AtomicU64 = AtomicU64::new(0);

pub fn decoded() -> u64 {
    DECODED.load(Ordering::Relaxed)
}

/// The event names already logged because we do not handle them.
///
/// Capped like `names_seen` in `parser.rs`: this set exists to write one log
/// line for each of them, not to grow.
static UNHANDLED_LOGGED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// The lifecycle events that may open a match. The GUID is not available at
/// `MatchCreated`, hence the fallbacks.
const OPENS: &[&str] = &["MatchInitialized", "CountdownBegin", "RoundStarted"];

/// What the loop hands back to its caller.
pub enum Event {
    /// A match is opening.
    Open,
    /// A match state, to be taken into account.
    State(Value),
    /// The match is ending.
    Close,
}

/// Connects and follows the socket until `stop` turns true.
///
/// `on` is called for every useful event. Reconnection is automatic: the game
/// may be started before us, or restarted underneath us.
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
                    // An event name we do not handle. Logged ONCE for each of
                    // them: if the game ever names the start or the end of a
                    // match differently, here is where we will see it, and
                    // nowhere else.
                    if let Ok(mut seen) = UNHANDLED_LOGGED.lock() {
                        if seen.len() < 32 && seen.insert(name.clone()) {
                            log::info!("rl: unhandled event from the game: {name}");
                        }
                    }
                }
            }
        }

        // A buffer that swells without ever yielding a whole object means the
        // stream is not what we think it is. Better to start clean than to grow
        // without end.
        if buf.len() > 8 << 20 {
            log::warn!("rl: buffer grew past 8 MiB without a whole object, dropping it");
            buf.clear();
        }
    }
}
