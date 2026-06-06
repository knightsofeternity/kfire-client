//! Local game detection: scans running processes on a fixed interval and
//! matches them against a known-games list.
//!
//! Current state: functional stub. It really scans processes via `sysinfo`
//! and logs detections, but:
//!   * the games list is a tiny hardcoded seed — it will be synced from the
//!     server (initial seed: Discord "detectable games" list);
//!   * detections are only logged — `game_started` / `game_stopped` events
//!     will be pushed to kfire-server over WebSocket (see
//!     kfire-protocol/websocket-events.md) with an SQLite offline queue.

use std::{collections::HashSet, thread, time::Duration};

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

/// How often we scan the process table.
const SCAN_INTERVAL: Duration = Duration::from_secs(5);

/// Hardcoded seed: (game slug, process names per platform, lowercase).
const KNOWN_GAMES: &[(&str, &[&str])] = &[
    ("counter-strike-2", &["cs2.exe", "cs2"]),
    ("dota-2", &["dota2.exe", "dota2"]),
    ("rocket-league", &["rocketleague.exe", "rocketleague"]),
];

/// Starts the scanner on a dedicated background thread.
pub fn spawn() {
    thread::Builder::new()
        .name("kfire-scanner".into())
        .spawn(run)
        .expect("failed to spawn scanner thread");
}

fn run() {
    let mut sys = System::new();
    let mut running: HashSet<&'static str> = HashSet::new();

    loop {
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );

        let mut seen: HashSet<&'static str> = HashSet::new();
        for process in sys.processes().values() {
            let name = process.name().to_string_lossy().to_lowercase();
            for (slug, exe_names) in KNOWN_GAMES {
                if exe_names.iter().any(|exe| *exe == name) {
                    seen.insert(slug);
                }
            }
        }

        for slug in seen.difference(&running) {
            // TODO(mvp): push `game_started` to the server, queue in SQLite if offline.
            println!("[scanner] game_started: {slug}");
        }
        for slug in running.difference(&seen) {
            // TODO(mvp): push `game_stopped` to the server, queue in SQLite if offline.
            println!("[scanner] game_stopped: {slug}");
        }

        running = seen;
        thread::sleep(SCAN_INTERVAL);
    }
}
