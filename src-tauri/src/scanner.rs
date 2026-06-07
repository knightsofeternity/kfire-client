//! Local game detection: scans running processes on a fixed interval and
//! matches them against the cached games catalog (exe basename → slug).
//!
//! Detections are pushed as [`GameEvent`]s into a channel consumed by the
//! WebSocket task; the set of currently running games is shared so the WS
//! task can re-announce them after a reconnect.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use tokio::sync::mpsc::UnboundedSender;

/// How often we scan the process table.
const SCAN_INTERVAL: Duration = Duration::from_secs(5);

/// A presence event produced by the scanner.
#[derive(Debug, Clone)]
pub struct GameEvent {
    pub started: bool,
    pub game_slug: String,
    pub ts: chrono::DateTime<chrono::Utc>,
}

/// Shared state between the scanner, the WS task and the UI.
#[derive(Default)]
pub struct ScannerState {
    /// exe basename (lowercase) → game slug. Rebuilt from the SQLite cache.
    pub exe_index: RwLock<HashMap<String, String>>,
    /// slug → display name, for the UI.
    pub names: RwLock<HashMap<String, String>>,
    /// Slugs of games currently detected as running.
    pub running: RwLock<HashSet<String>>,
}

impl ScannerState {
    /// Rebuilds the matching index from the cached catalog.
    pub fn load_catalog(&self, games: &[crate::db::CachedGame]) {
        let mut index = HashMap::new();
        let mut names = HashMap::new();
        for g in games {
            for exe in &g.executable_names {
                index.insert(exe.clone(), g.slug.clone());
            }
            names.insert(g.slug.clone(), g.name.clone());
        }
        log::info!(
            "scanner: catalog loaded ({} games, {} executables)",
            names.len(),
            index.len()
        );
        *self.exe_index.write().unwrap() = index;
        *self.names.write().unwrap() = names;
    }

    pub fn running_slugs(&self) -> Vec<String> {
        self.running.read().unwrap().iter().cloned().collect()
    }
}

/// Starts the scanner on a dedicated background thread.
pub fn spawn(state: Arc<ScannerState>, events: UnboundedSender<GameEvent>) {
    thread::Builder::new()
        .name("kfire-scanner".into())
        .spawn(move || run(state, events))
        .expect("failed to spawn scanner thread");
}

fn run(state: Arc<ScannerState>, events: UnboundedSender<GameEvent>) {
    let mut sys = System::new();

    loop {
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );

        let seen: HashSet<String> = {
            let index = state.exe_index.read().unwrap();
            if index.is_empty() {
                // Catalog not downloaded yet: nothing to match.
                thread::sleep(SCAN_INTERVAL);
                continue;
            }
            sys.processes()
                .values()
                .filter_map(|p| {
                    let name = p.name().to_string_lossy().to_lowercase();
                    index.get(&name).cloned()
                })
                .collect()
        };

        let (started, stopped): (Vec<String>, Vec<String>) = {
            let running = state.running.read().unwrap();
            (
                seen.difference(&running).cloned().collect(),
                running.difference(&seen).cloned().collect(),
            )
        };

        for slug in started {
            log::info!("scanner: game_started {slug}");
            let _ = events.send(GameEvent {
                started: true,
                game_slug: slug,
                ts: chrono::Utc::now(),
            });
        }
        for slug in stopped {
            log::info!("scanner: game_stopped {slug}");
            let _ = events.send(GameEvent {
                started: false,
                game_slug: slug,
                ts: chrono::Utc::now(),
            });
        }

        *state.running.write().unwrap() = seen;
        thread::sleep(SCAN_INTERVAL);
    }
}
