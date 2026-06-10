//! Local game detection: scans running processes on a fixed interval and
//! matches them against the cached games catalogs (exe basename → games).
//!
//! One scanner is shared across every linked server: you play one game, and it
//! is reported to each server that knows it, using that server's own slug. The
//! match index therefore maps an executable to a list of `(server_id, slug)`
//! pairs, and each detection fans out into one [`GameEvent`] per pair.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use tokio::sync::mpsc::UnboundedSender;

/// How often we scan the process table.
const SCAN_INTERVAL: Duration = Duration::from_secs(5);

/// A presence event produced by the scanner, addressed to one server.
#[derive(Debug, Clone)]
pub struct GameEvent {
    pub started: bool,
    pub server_id: String,
    pub game_slug: String,
    pub ts: chrono::DateTime<chrono::Utc>,
}

/// Shared state between the scanner, the WS tasks and the UI.
#[derive(Default)]
pub struct ScannerState {
    /// exe basename (lowercase) → [(server_id, slug)]. Rebuilt from the cache.
    pub exe_index: RwLock<HashMap<String, Vec<(String, String)>>>,
    /// slug → display name, for the UI (merged across servers).
    pub names: RwLock<HashMap<String, String>>,
    /// exe basenames currently detected as running.
    pub running: RwLock<HashSet<String>>,
}

impl ScannerState {
    /// Rebuilds the matching index from every server's cached catalog.
    pub fn load_catalog(&self, games: &[(String, crate::db::CachedGame)]) {
        let mut index: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut names = HashMap::new();
        for (server_id, g) in games {
            for exe in &g.executable_names {
                index
                    .entry(exe.clone())
                    .or_default()
                    .push((server_id.clone(), g.slug.clone()));
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

    /// Slugs of the given server's games that are currently running.
    pub fn running_for(&self, server_id: &str) -> Vec<String> {
        let index = self.exe_index.read().unwrap();
        let running = self.running.read().unwrap();
        let mut slugs = Vec::new();
        for exe in running.iter() {
            if let Some(pairs) = index.get(exe) {
                for (sid, slug) in pairs {
                    if sid == server_id {
                        slugs.push(slug.clone());
                    }
                }
            }
        }
        slugs
    }

    /// Currently-running games as `(slug, name)` for the UI, one per game.
    pub fn running_games(&self) -> Vec<(String, String)> {
        let index = self.exe_index.read().unwrap();
        let names = self.names.read().unwrap();
        let running = self.running.read().unwrap();
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for exe in running.iter() {
            if let Some(pairs) = index.get(exe) {
                if let Some((_, slug)) = pairs.first() {
                    if seen.insert(slug.clone()) {
                        let name = names.get(slug).cloned().unwrap_or_else(|| slug.clone());
                        out.push((slug.clone(), name));
                    }
                }
            }
        }
        out
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
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());

        let seen: HashSet<String> = {
            let index = state.exe_index.read().unwrap();
            if index.is_empty() {
                // No catalog downloaded yet: nothing to match.
                thread::sleep(SCAN_INTERVAL);
                continue;
            }
            sys.processes()
                .values()
                .filter_map(|p| {
                    let name = p.name().to_string_lossy().to_lowercase();
                    if index.contains_key(&name) {
                        Some(name)
                    } else {
                        None
                    }
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

        // Fan each started/stopped executable out to every server that knows it.
        let now = chrono::Utc::now();
        {
            let index = state.exe_index.read().unwrap();
            for exe in &started {
                for (server_id, slug) in index.get(exe).into_iter().flatten() {
                    log::info!("scanner: game_started {slug} ({server_id})");
                    let _ = events.send(GameEvent {
                        started: true,
                        server_id: server_id.clone(),
                        game_slug: slug.clone(),
                        ts: now,
                    });
                }
            }
            for exe in &stopped {
                for (server_id, slug) in index.get(exe).into_iter().flatten() {
                    log::info!("scanner: game_stopped {slug} ({server_id})");
                    let _ = events.send(GameEvent {
                        started: false,
                        server_id: server_id.clone(),
                        game_slug: slug.clone(),
                        ts: now,
                    });
                }
            }
        }

        *state.running.write().unwrap() = seen;
        thread::sleep(SCAN_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::CachedGame;

    fn game(slug: &str, exe: &str) -> CachedGame {
        CachedGame {
            slug: slug.into(),
            name: format!("Name {slug}"),
            executable_names: vec![exe.into()],
        }
    }

    #[test]
    fn shared_exe_fans_out_to_both_servers() {
        let s = ScannerState::default();
        s.load_catalog(&[
            ("srv-a".into(), game("a-game", "game.exe")),
            ("srv-b".into(), game("b-game", "game.exe")),
        ]);

        let index = s.exe_index.read().unwrap();
        let pairs = index.get("game.exe").unwrap();
        assert_eq!(pairs.len(), 2);
        assert!(pairs.contains(&("srv-a".into(), "a-game".into())));
        assert!(pairs.contains(&("srv-b".into(), "b-game".into())));
    }

    #[test]
    fn running_for_returns_per_server_slugs() {
        let s = ScannerState::default();
        s.load_catalog(&[
            ("srv-a".into(), game("a-game", "game.exe")),
            ("srv-b".into(), game("b-game", "game.exe")),
        ]);
        s.running.write().unwrap().insert("game.exe".into());

        assert_eq!(s.running_for("srv-a"), vec!["a-game".to_string()]);
        assert_eq!(s.running_for("srv-b"), vec!["b-game".to_string()]);
    }

    #[test]
    fn running_games_dedups_per_game() {
        let s = ScannerState::default();
        s.load_catalog(&[
            ("srv-a".into(), game("a-game", "game.exe")),
            ("srv-b".into(), game("b-game", "game.exe")),
        ]);
        s.running.write().unwrap().insert("game.exe".into());

        // One running executable → one UI entry, even across two servers.
        assert_eq!(s.running_games().len(), 1);
    }
}
