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

const CPU_ACTIVE_THRESHOLD: f32 = 1.0; // summed CPU% across the basename's processes
const IDLE_SCANS_TO_DROP: u32 = 3; // ~15s at the 5s scan interval

/// Whether a matched exe counts as "playing". START is never CPU-gated; a
/// process already running is dropped only after IDLE_SCANS_TO_DROP consecutive
/// idle scans. Any CPU activity resets the idle counter.
fn is_active(
    exe: &str,
    was_running: bool,
    cpu: f32,
    idle: &mut std::collections::HashMap<String, u32>,
) -> bool {
    if cpu >= CPU_ACTIVE_THRESHOLD {
        idle.remove(exe);
        return true;
    }
    if !was_running {
        return true;
    }
    let n = idle.entry(exe.to_string()).or_insert(0);
    *n += 1;
    *n < IDLE_SCANS_TO_DROP
}

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
    /// exe basenames currently detected as running (the effective "playing" set).
    pub running: RwLock<HashSet<String>>,
    /// `(server_id, slug)` pairs the user has chosen to ignore (never reported).
    pub ignored: RwLock<HashSet<(String, String)>>,
    /// exe basenames the user has "stopped"; suppressed until the process exits.
    pub suppressed: RwLock<HashSet<String>>,
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

    /// Replaces the ignore set from a `db.list_ignored()` slice.
    pub fn reload_ignored(&self, ignored: &[(String, String)]) {
        *self.ignored.write().unwrap() = ignored.iter().cloned().collect();
    }

    /// Marks every exe that maps to `slug` as suppressed, so it stops being
    /// reported until the underlying process exits. Used when the user "stops"
    /// a running game from the UI.
    pub fn suppress_slug(&self, slug: &str) {
        let index = self.exe_index.read().unwrap();
        let mut suppressed = self.suppressed.write().unwrap();
        for (exe, pairs) in index.iter() {
            if pairs.iter().any(|(_, s)| s == slug) {
                suppressed.insert(exe.clone());
            }
        }
    }

    /// All `(server_id, slug)` pairs that map to this slug (deduplicated).
    pub fn servers_for_slug(&self, slug: &str) -> Vec<(String, String)> {
        let index = self.exe_index.read().unwrap();
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for pairs in index.values() {
            for pair in pairs {
                if pair.1 == slug && seen.insert(pair.clone()) {
                    out.push(pair.clone());
                }
            }
        }
        out
    }

    /// Slugs of the given server's games that are currently running.
    pub fn running_for(&self, server_id: &str) -> Vec<String> {
        let index = self.exe_index.read().unwrap();
        let running = self.running.read().unwrap();
        let ignored = self.ignored.read().unwrap();
        let mut slugs = Vec::new();
        for exe in running.iter() {
            if let Some(pairs) = index.get(exe) {
                for (sid, slug) in pairs {
                    if sid == server_id && !ignored.contains(&(sid.clone(), slug.clone())) {
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
        let ignored = self.ignored.read().unwrap();
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for exe in running.iter() {
            if let Some(pairs) = index.get(exe) {
                // Show the game only via a pair the user has not ignored.
                if let Some((_, slug)) = pairs
                    .iter()
                    .find(|(sid, slug)| !ignored.contains(&(sid.clone(), slug.clone())))
                {
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
    // Per-basename consecutive-idle-scan counter; persists across scans.
    let mut idle: HashMap<String, u32> = HashMap::new();

    loop {
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu(),
        );

        // Summed CPU% per matched basename, plus the set of matched basenames
        // actually present in the process table this scan.
        let (cpu_sum, present): (HashMap<String, f32>, HashSet<String>) = {
            let index = state.exe_index.read().unwrap();
            if index.is_empty() {
                // No catalog downloaded yet: nothing to match.
                thread::sleep(SCAN_INTERVAL);
                continue;
            }
            let mut cpu_sum: HashMap<String, f32> = HashMap::new();
            let mut present: HashSet<String> = HashSet::new();
            for p in sys.processes().values() {
                let name = p.name().to_string_lossy().to_lowercase();
                if index.contains_key(&name) {
                    *cpu_sum.entry(name.clone()).or_insert(0.0) += p.cpu_usage();
                    present.insert(name);
                }
            }
            (cpu_sum, present)
        };

        // Compute the new effective ("playing") set: present + CPU-active +
        // not user-suppressed. START is never CPU-gated (see is_active).
        let seen_effective: HashSet<String> = {
            let prev = state.running.read().unwrap();
            let suppressed = state.suppressed.read().unwrap();
            present
                .iter()
                .filter(|exe| {
                    let cpu = cpu_sum.get(*exe).copied().unwrap_or(0.0);
                    is_active(exe, prev.contains(*exe), cpu, &mut idle)
                        && !suppressed.contains(*exe)
                })
                .cloned()
                .collect()
        };

        // Auto-clear suppression/idle bookkeeping for processes that fully exited.
        {
            let mut suppressed = state.suppressed.write().unwrap();
            suppressed.retain(|exe| present.contains(exe));
        }
        idle.retain(|exe, _| present.contains(exe));

        let (started, stopped): (Vec<String>, Vec<String>) = {
            let running = state.running.read().unwrap();
            (
                seen_effective.difference(&running).cloned().collect(),
                running.difference(&seen_effective).cloned().collect(),
            )
        };

        // Fan each started/stopped executable out to every server that knows it,
        // skipping any (server_id, slug) pair the user has ignored.
        let now = chrono::Utc::now();
        {
            let index = state.exe_index.read().unwrap();
            let ignored = state.ignored.read().unwrap();
            for exe in &started {
                for (server_id, slug) in index.get(exe).into_iter().flatten() {
                    if ignored.contains(&(server_id.clone(), slug.clone())) {
                        continue;
                    }
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
                    if ignored.contains(&(server_id.clone(), slug.clone())) {
                        continue;
                    }
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

        *state.running.write().unwrap() = seen_effective;
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
    fn cpu_gate_drops_after_sustained_idle_not_on_first() {
        let mut idle: std::collections::HashMap<String, u32> = Default::default();
        assert!(is_active("g.exe", true, 0.0, &mut idle)); // running, idle #1 -> keep
        assert!(is_active("g.exe", true, 0.0, &mut idle)); // idle #2 -> keep
        assert!(!is_active("g.exe", true, 0.0, &mut idle)); // idle #3 -> drop
        assert!(is_active("g.exe", true, 5.0, &mut idle)); // CPU back -> active, counter reset
        assert!(is_active("g.exe", true, 0.0, &mut idle)); // idle #1 again (reset worked)
    }

    #[test]
    fn cpu_gate_does_not_gate_start() {
        let mut idle = Default::default();
        assert!(is_active("g.exe", false, 0.0, &mut idle)); // freshly seen, idle -> still starts
    }

    #[test]
    fn ignored_pair_excluded_from_running_for() {
        let s = ScannerState::default();
        s.load_catalog(&[("srv-a".into(), game("a-game", "game.exe"))]);
        s.running.write().unwrap().insert("game.exe".into());
        s.reload_ignored(&[("srv-a".into(), "a-game".into())]);
        assert!(s.running_for("srv-a").is_empty());
    }

    #[test]
    fn suppress_slug_marks_exe() {
        let s = ScannerState::default();
        s.load_catalog(&[("srv-a".into(), game("a-game", "game.exe"))]);
        s.suppress_slug("a-game");
        assert!(s.suppressed.read().unwrap().contains("game.exe"));
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
