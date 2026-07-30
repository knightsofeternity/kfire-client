//! Local game detection: scans running processes on a fixed interval and
//! matches them against the cached games catalogs.
//!
//! A catalog entry is matched in one of two ways:
//! - a plain **basename** (`subnautica.exe`), compared to the process name;
//! - a **qualified path pattern** (`counter-strike source/hl2.exe`), matched as
//!   a suffix of the process's full path. The server publishes this form for
//!   games whose binary name is too generic to identify anything on its own
//!   (`game.exe`, `hl2.exe`), which is the only way they can be detected.
//!
//! Paths are read locally and compared locally: they never leave the machine,
//! only game slugs are reported to servers.
//!
//! One scanner is shared across every linked server: you play one game, and it
//! is reported to each server that knows it, using that server's own slug. The
//! match index therefore maps a catalog entry to a list of `(server_id, slug)`
//! pairs, and each detection fans out into one [`GameEvent`] per pair.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tokio::sync::mpsc::UnboundedSender;

/// How often we scan the process table.
const SCAN_INTERVAL: Duration = Duration::from_secs(5);

const CPU_ACTIVE_THRESHOLD: f32 = 1.0; // summed CPU% across the basename's processes
const IDLE_SCANS_TO_DROP: u32 = 3; // ~15s at the 5s scan interval

/// Whether a matched exe counts as "playing".
///
/// A process is dropped only after IDLE_SCANS_TO_DROP consecutive scans with a
/// genuine low CPU reading. The following always count as active and reset the
/// idle counter:
/// - CPU at or above the threshold (real activity);
/// - CPU exactly 0.0 on a present process: anti-cheat protected games
///   (BattlEye/EAC, e.g. PUBG) report 0.0 to an unprivileged scanner, so 0.0
///   means "unreadable", not "idle". Counting it as idle shredded real play
///   into hundreds of sub-minute sessions;
/// - a process not present on the previous scan (a genuinely new launch):
///   started ungated. `present_before` is the raw presence of the prior scan,
///   NOT the effective "playing" set, so a process the idle gate just dropped
///   while it is still running is not re-started next scan (no on/off flapping).
fn is_active(
    exe: &str,
    present_before: bool,
    cpu: f32,
    idle: &mut std::collections::HashMap<String, u32>,
) -> bool {
    if cpu >= CPU_ACTIVE_THRESHOLD || cpu == 0.0 || !present_before {
        idle.remove(exe);
        return true;
    }
    let n = idle.entry(exe.to_string()).or_insert(0);
    *n += 1;
    *n < IDLE_SCANS_TO_DROP
}

/// Lowercases a process executable path and unifies separators, so Windows
/// paths compare against the forward-slash patterns the catalog ships.
fn normalize_path(raw: &str) -> String {
    raw.replace('\\', "/").to_lowercase()
}

/// Whether a normalized process `path` ends with a catalog `pattern`, aligned
/// on a directory boundary. The boundary check is what keeps
/// "my dragon ball gekishin squadra/game.exe" from matching the real game.
fn matches_pattern(path: &str, pattern: &str) -> bool {
    if pattern.is_empty() || !path.ends_with(pattern) {
        return false;
    }
    let head = path.len() - pattern.len();
    head == 0 || path.as_bytes()[head - 1] == b'/'
}

/// Catalog keys a running process matches: its basename when the catalog knows
/// it, plus every qualified pattern whose suffix the process path satisfies. A
/// process legitimately yields two keys when one game is listed by basename and
/// another by a pattern ending in the same file name.
///
/// `path` is `None` when the executable path could not be read (permissions),
/// in which case only basename matching applies, exactly as before v0.4.0.
fn process_keys(
    name: &str,
    path: Option<&str>,
    key_index: &HashMap<String, Vec<(String, String)>>,
    patterns_by_basename: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let mut keys = Vec::new();
    if key_index.contains_key(name) {
        keys.push(name.to_string());
    }
    if let (Some(path), Some(candidates)) = (path, patterns_by_basename.get(name)) {
        for pattern in candidates {
            if matches_pattern(path, pattern) {
                keys.push(pattern.clone());
            }
        }
    }
    keys
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
    /// Catalog entry (lowercase basename or path pattern) → [(server_id, slug)].
    /// Rebuilt from the cache; every downstream set is keyed by these strings.
    pub key_index: RwLock<HashMap<String, Vec<(String, String)>>>,
    /// Pattern basename → the patterns ending with it, so a process only ever
    /// suffix-checks the handful of patterns that could possibly match it.
    pub patterns_by_basename: RwLock<HashMap<String, Vec<String>>>,
    /// slug → display name, for the UI (merged across servers).
    pub names: RwLock<HashMap<String, String>>,
    /// Catalog keys currently detected as running (the effective "playing" set).
    pub running: RwLock<HashSet<String>>,
    /// `(server_id, slug)` pairs the user has chosen to ignore (never reported).
    pub ignored: RwLock<HashSet<(String, String)>>,
    /// Catalog keys the user has "stopped"; suppressed until the process exits.
    pub suppressed: RwLock<HashSet<String>>,
}

impl ScannerState {
    /// Rebuilds the matching index from every server's cached catalog.
    pub fn load_catalog(&self, games: &[(String, crate::db::CachedGame)]) {
        let mut index: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut patterns: HashMap<String, Vec<String>> = HashMap::new();
        let mut names = HashMap::new();
        for (server_id, g) in games {
            for exe in &g.executable_names {
                index
                    .entry(exe.clone())
                    .or_default()
                    .push((server_id.clone(), g.slug.clone()));
                if let Some((_, base)) = exe.rsplit_once('/') {
                    let candidates = patterns.entry(base.to_string()).or_default();
                    if !candidates.contains(exe) {
                        candidates.push(exe.clone());
                    }
                }
            }
            names.insert(g.slug.clone(), g.name.clone());
        }
        log::info!(
            "scanner: catalog loaded ({} games, {} entries, {} path patterns)",
            names.len(),
            index.len(),
            patterns.values().map(Vec::len).sum::<usize>()
        );
        *self.key_index.write().unwrap() = index;
        *self.patterns_by_basename.write().unwrap() = patterns;
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
        let index = self.key_index.read().unwrap();
        let mut suppressed = self.suppressed.write().unwrap();
        for (exe, pairs) in index.iter() {
            if pairs.iter().any(|(_, s)| s == slug) {
                suppressed.insert(exe.clone());
            }
        }
    }

    /// All `(server_id, slug)` pairs that map to this slug (deduplicated).
    pub fn servers_for_slug(&self, slug: &str) -> Vec<(String, String)> {
        let index = self.key_index.read().unwrap();
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
        let index = self.key_index.read().unwrap();
        let running = self.running.read().unwrap();
        let ignored = self.ignored.read().unwrap();
        let suppressed = self.suppressed.read().unwrap();
        let mut slugs = Vec::new();
        for exe in running.iter() {
            if suppressed.contains(exe) {
                continue;
            }
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
        let index = self.key_index.read().unwrap();
        let names = self.names.read().unwrap();
        let running = self.running.read().unwrap();
        let ignored = self.ignored.read().unwrap();
        let suppressed = self.suppressed.read().unwrap();
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for exe in running.iter() {
            if suppressed.contains(exe) {
                continue;
            }
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
    // Raw set of matched basenames present on the previous scan. Used to tell a
    // genuinely new launch (start ungated) from a process the idle gate dropped
    // but which is still running (must stay dropped, not flap back on).
    let mut prev_present: HashSet<String> = HashSet::new();

    loop {
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            // The executable path is read once per process and cached, and only
            // consulted for names the catalog knows as a path pattern.
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_exe(UpdateKind::OnlyIfNotSet),
        );

        // Summed CPU% per matched catalog key, plus the set of keys actually
        // present in the process table this scan.
        let (cpu_sum, present): (HashMap<String, f32>, HashSet<String>) = {
            let index = state.key_index.read().unwrap();
            let patterns = state.patterns_by_basename.read().unwrap();
            if index.is_empty() {
                // No catalog downloaded yet: nothing to match.
                thread::sleep(SCAN_INTERVAL);
                continue;
            }
            let mut cpu_sum: HashMap<String, f32> = HashMap::new();
            let mut present: HashSet<String> = HashSet::new();
            for p in sys.processes().values() {
                let name = p.name().to_string_lossy().to_lowercase();
                let path = if patterns.contains_key(&name) {
                    p.exe().map(|e| normalize_path(&e.to_string_lossy()))
                } else {
                    None
                };
                for key in process_keys(&name, path.as_deref(), &index, &patterns) {
                    *cpu_sum.entry(key.clone()).or_insert(0.0) += p.cpu_usage();
                    present.insert(key);
                }
            }
            (cpu_sum, present)
        };

        // Compute the new effective ("playing") set: present + CPU-active +
        // not user-suppressed. START is never CPU-gated (see is_active).
        let seen_effective: HashSet<String> = {
            let suppressed = state.suppressed.read().unwrap();
            present
                .iter()
                .filter(|exe| {
                    let cpu = cpu_sum.get(*exe).copied().unwrap_or(0.0);
                    is_active(exe, prev_present.contains(*exe), cpu, &mut idle)
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
            let index = state.key_index.read().unwrap();
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
        prev_present = present;
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
    fn idle_gate_drops_after_sustained_low_cpu_not_on_first() {
        // Low-but-nonzero CPU (below threshold, not the 0.0 "unreadable" value).
        let mut idle: std::collections::HashMap<String, u32> = Default::default();
        assert!(is_active("g.exe", true, 0.5, &mut idle)); // idle #1 -> keep
        assert!(is_active("g.exe", true, 0.5, &mut idle)); // idle #2 -> keep
        assert!(!is_active("g.exe", true, 0.5, &mut idle)); // idle #3 -> drop
        assert!(is_active("g.exe", true, 5.0, &mut idle)); // CPU back -> active, counter reset
        assert!(is_active("g.exe", true, 0.5, &mut idle)); // idle #1 again (reset worked)
    }

    #[test]
    fn new_process_starts_ungated() {
        // A genuinely new launch (not present last scan) starts regardless of CPU.
        let mut idle = Default::default();
        assert!(is_active("g.exe", false, 0.5, &mut idle));
    }

    #[test]
    fn zero_cpu_is_active_not_idle() {
        // Anti-cheat protected games (PUBG/BattlEye) report 0.0 CPU to an
        // unprivileged scanner; that must count as active, never accrue idle.
        let mut idle = Default::default();
        for _ in 0..10 {
            assert!(is_active("tslgame.exe", true, 0.0, &mut idle));
        }
        assert!(idle.is_empty());
    }

    #[test]
    fn dropped_process_stays_dropped_no_flap() {
        // Regression: once dropped for sustained low CPU, a still-present process
        // must not restart the next scan (present_before stays true), so we never
        // get the on/off flapping that littered the history with micro-sessions.
        let mut idle = Default::default();
        assert!(is_active("g.exe", true, 0.5, &mut idle)); // idle #1
        assert!(is_active("g.exe", true, 0.5, &mut idle)); // idle #2
        assert!(!is_active("g.exe", true, 0.5, &mut idle)); // idle #3 -> drop
        assert!(!is_active("g.exe", true, 0.5, &mut idle)); // stays dropped
        assert!(!is_active("g.exe", true, 0.5, &mut idle)); // stays dropped (no flap)
    }

    #[test]
    fn pattern_matches_on_segment_boundary_only() {
        let p = "dragon ball gekishin squadra/game.exe";
        assert!(matches_pattern(
            "c:/steam/steamapps/common/dragon ball gekishin squadra/game.exe",
            p
        ));
        // The whole path may be exactly the pattern.
        assert!(matches_pattern(p, p));
        // A directory whose name merely ends with the pattern's first segment
        // must not match.
        assert!(!matches_pattern(
            "c:/games/my dragon ball gekishin squadra/game.exe",
            p
        ));
        // Same basename, different game.
        assert!(!matches_pattern("c:/steam/common/some other game/game.exe", p));
        assert!(!matches_pattern("game.exe", p));
    }

    #[test]
    fn process_path_is_normalized_before_matching() {
        assert_eq!(
            normalize_path(r"C:\Program Files\Alien Isolation\AI.exe"),
            "c:/program files/alien isolation/ai.exe"
        );
    }

    #[test]
    fn process_keys_covers_basename_pattern_and_both() {
        let s = ScannerState::default();
        s.load_catalog(&[
            ("srv".into(), game("subnautica", "subnautica.exe")),
            ("srv".into(), game("gekishin", "dragon ball gekishin squadra/game.exe")),
        ]);
        let index = s.key_index.read().unwrap();
        let patterns = s.patterns_by_basename.read().unwrap();

        // Plain basename.
        assert_eq!(
            process_keys("subnautica.exe", Some("d:/games/subnautica/subnautica.exe"), &index, &patterns),
            vec!["subnautica.exe".to_string()]
        );
        // Pattern only: the basename alone is not in the index.
        assert_eq!(
            process_keys(
                "game.exe",
                Some("d:/steamapps/common/dragon ball gekishin squadra/game.exe"),
                &index,
                &patterns
            ),
            vec!["dragon ball gekishin squadra/game.exe".to_string()]
        );
        // Same basename in the wrong directory: no key at all.
        assert!(process_keys("game.exe", Some("d:/games/unrelated/game.exe"), &index, &patterns).is_empty());
        // Unreadable path (permissions): fall back to basename matching only.
        assert!(process_keys("game.exe", None, &index, &patterns).is_empty());
        assert_eq!(
            process_keys("subnautica.exe", None, &index, &patterns),
            vec!["subnautica.exe".to_string()]
        );
    }

    #[test]
    fn a_process_can_match_a_basename_and_a_pattern_at_once() {
        let s = ScannerState::default();
        s.load_catalog(&[
            ("srv".into(), game("plain", "game.exe")),
            ("srv".into(), game("qualified", "some game/game.exe")),
        ]);
        let index = s.key_index.read().unwrap();
        let patterns = s.patterns_by_basename.read().unwrap();
        let keys = process_keys("game.exe", Some("c:/x/some game/game.exe"), &index, &patterns);
        assert_eq!(keys.len(), 2, "expected both keys, got {keys:?}");
        assert!(keys.contains(&"game.exe".to_string()));
        assert!(keys.contains(&"some game/game.exe".to_string()));
    }

    /// End-to-end on a real process: proves the refresh flags actually give us
    /// an executable path, which is the assumption the whole pattern matching
    /// rests on. Unix-only because it needs a binary to copy and run.
    #[cfg(unix)]
    #[test]
    fn detects_a_real_running_process_by_its_install_directory() {
        let root = std::env::temp_dir().join("kfire-scanner-test");
        let dir = root.join("dragon ball gekishin squadra");
        std::fs::create_dir_all(&dir).expect("create fake install dir");
        let exe = dir.join("game.exe");
        std::fs::copy("/bin/sleep", &exe).expect("copy a harmless binary");
        let mut child = std::process::Command::new(&exe)
            .arg("30")
            .spawn()
            .expect("run the fake game");
        thread::sleep(Duration::from_millis(200)); // let it appear in the table

        let state = ScannerState::default();
        state.load_catalog(&[(
            "srv".into(),
            game("gekishin", "dragon ball gekishin squadra/game.exe"),
        )]);

        let mut sys = System::new();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_exe(UpdateKind::OnlyIfNotSet),
        );
        let matched = {
            let index = state.key_index.read().unwrap();
            let patterns = state.patterns_by_basename.read().unwrap();
            sys.processes().values().any(|p| {
                let name = p.name().to_string_lossy().to_lowercase();
                let path = p.exe().map(|e| normalize_path(&e.to_string_lossy()));
                process_keys(&name, path.as_deref(), &index, &patterns)
                    .contains(&"dragon ball gekishin squadra/game.exe".to_string())
            })
        };

        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&root);
        assert!(matched, "the running game.exe should match its qualified pattern");
    }

    #[test]
    fn suppressing_a_pattern_game_suppresses_its_pattern_key() {
        let s = ScannerState::default();
        s.load_catalog(&[(
            "srv".into(),
            game("gekishin", "dragon ball gekishin squadra/game.exe"),
        )]);
        s.suppress_slug("gekishin");
        assert!(s
            .suppressed
            .read()
            .unwrap()
            .contains("dragon ball gekishin squadra/game.exe"));
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

        let index = s.key_index.read().unwrap();
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
