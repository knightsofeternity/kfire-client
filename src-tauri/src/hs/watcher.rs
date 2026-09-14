//! Following the game's log while it is running.
//!
//! The game creates ONE DATED FOLDER PER SESSION and writes a fresh Power.log
//! inside it. The watcher follows the newest folder and switches as soon as a
//! newer one appears; a watcher built for a single file would miss every match
//! after the first restart, with nothing to signal it.

use crate::hs::{parser, paths};
use std::path::{Path, PathBuf};

/// How far into the current file we have already read.
#[derive(Default)]
pub struct Cursor {
    read: usize,
}

impl Cursor {
    /// The complete lines appended since the last call.
    ///
    /// A partial trailing line is held back: the game writes while we read, and
    /// half a line would parse as nonsense. It comes back whole next time.
    ///
    /// A file SHORTER than our position is a different file, so it is read from
    /// the start rather than from a meaningless offset.
    pub fn take_new(&mut self, contents: &str) -> String {
        if contents.len() < self.read {
            self.read = 0;
        }
        let fresh = &contents[self.read..];
        let end = match fresh.rfind('\n') {
            Some(i) => i + 1,
            None => return String::new(),
        };
        self.read += end;
        fresh[..end].to_string()
    }

    /// Forget the position, for when we move to another file.
    pub fn reset(&mut self) {
        self.read = 0;
    }
}

/// How often the log is re-read while the game runs.
const POLL: std::time::Duration = std::time::Duration::from_secs(5);

/// The newest session folder inside an install directory, and its start.
fn newest_log(install: &Path) -> Option<(PathBuf, chrono::NaiveDateTime)> {
    let logs = install.join(paths::LOGS_DIR);
    let names: Vec<String> = std::fs::read_dir(&logs)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    let newest = paths::newest_session(&names)?.to_string();
    let start = paths::session_start(&newest)?;
    Some((logs.join(&newest).join("Power.log"), start))
}

/// Follows the log until `stop` is set, handing every finished match to `emit`.
///
/// Failure is SILENT by design: an unreadable or unrecognised log leaves a
/// trace in our own logs and nothing else. Alarming the member over a possibly
/// transient game patch would be worse than saying nothing.
pub fn follow<F: FnMut(parser::Match, chrono::NaiveDateTime)>(
    install: &Path,
    stop: &std::sync::atomic::AtomicBool,
    mut emit: F,
) {
    use std::sync::atomic::Ordering;

    let mut current: Option<PathBuf> = None;
    let mut cursor = Cursor::default();

    while !stop.load(Ordering::Relaxed) {
        if let Some((path, start)) = newest_log(install) {
            if current.as_deref() != Some(path.as_path()) {
                log::info!("hs: following {}", path.display());
                current = Some(path.clone());
                cursor.reset();
            }
            if let Ok(contents) = std::fs::read_to_string(&path) {
                let fresh = cursor.take_new(&contents);
                if !fresh.is_empty() {
                    for m in parser::parse_games(fresh.lines()) {
                        emit(m, start);
                    }
                }
            }
        }
        std::thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_cursor_reads_from_the_start() {
        let mut c = Cursor::default();
        assert_eq!(c.take_new("ligne une\nligne deux\n"), "ligne une\nligne deux\n");
    }

    #[test]
    fn only_what_was_appended_is_returned() {
        let mut c = Cursor::default();
        c.take_new("ligne une\n");
        assert_eq!(c.take_new("ligne une\nligne deux\n"), "ligne deux\n");
    }

    #[test]
    fn nothing_new_yields_nothing() {
        let mut c = Cursor::default();
        c.take_new("ligne une\n");
        assert_eq!(c.take_new("ligne une\n"), "");
    }

    #[test]
    fn a_shorter_file_means_a_new_file_and_is_read_whole() {
        // The game truncated or replaced the file under us; reading from the
        // old offset would return garbage.
        let mut c = Cursor::default();
        c.take_new("une longue premiere ligne\n");
        assert_eq!(c.take_new("court\n"), "court\n");
    }

    #[test]
    fn switching_folders_resets_the_position() {
        let mut c = Cursor::default();
        c.take_new("ligne une\nligne deux\n");
        c.reset();
        assert_eq!(c.take_new("autre\n"), "autre\n");
    }

    #[test]
    fn a_partial_last_line_is_held_back() {
        // The game writes as we read; half a line must not be parsed, it would
        // be parsed again whole on the next pass.
        let mut c = Cursor::default();
        assert_eq!(c.take_new("complete\nincomp"), "complete\n");
        assert_eq!(c.take_new("complete\nincomplete\n"), "incomplete\n");
    }

    #[test]
    fn a_file_with_no_newline_at_all_yields_nothing_yet() {
        let mut c = Cursor::default();
        assert_eq!(c.take_new("pas encore de fin de ligne"), "");
    }

    #[test]
    fn newest_log_follows_the_latest_session_folder() {
        let install = std::env::temp_dir().join(format!(
            "kfire-hs-watcher-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        // No Logs folder at all yet: nothing to follow.
        assert_eq!(newest_log(&install), None);

        let logs = install.join(paths::LOGS_DIR);
        let older = logs.join("Hearthstone_2026_08_02_17_22_29");
        let newer = logs.join("Hearthstone_2026_08_03_09_14_02");
        std::fs::create_dir_all(&older).unwrap();
        std::fs::create_dir_all(&newer).unwrap();

        let (path, start) = newest_log(&install).expect("a newest session");
        assert_eq!(path, newer.join("Power.log"));
        assert_eq!(
            start.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-08-03 09:14:02"
        );

        std::fs::remove_dir_all(&install).unwrap();
    }
}
