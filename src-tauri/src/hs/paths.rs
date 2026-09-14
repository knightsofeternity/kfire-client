//! Where Hearthstone lives on this machine, and where it writes.
//!
//! The install path is NOT fixed: a real install observed on 2026-09-14 sits
//! under C:\Battle.net\Hearthstone rather than the default folder. So the
//! install directory is derived from the RUNNING PROCESS, which is exact, with
//! a manual setting as the fallback.

use chrono::NaiveDateTime;
use std::path::{Path, PathBuf};

/// The game's own log folder, one dated directory per session inside it.
pub const LOGS_DIR: &str = "Logs";

/// Splits off the final path component, treating BOTH `/` and `\` as
/// separators regardless of the host platform.
///
/// A path observed on the member's machine keeps the separator that machine
/// used: a Windows path handed to `std::path::Path` on Linux (our CI, and
/// this dev machine) is not split on `\` at all, since Unix treats it as an
/// ordinary character. Recovering the parent has to work the same way
/// whether we are compiled for Windows or not, so we parse the raw string
/// ourselves instead of leaning on `Path::parent`.
fn parent_str(s: &str) -> Option<&str> {
    let idx = s.rfind(['/', '\\'])?;
    Some(&s[..idx])
}

fn file_name_str(s: &str) -> Option<&str> {
    let name = match s.rfind(['/', '\\']) {
        Some(idx) => &s[idx + 1..],
        None => s,
    };
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// The install directory holding the executable.
///
/// On macOS the executable sits deep inside the bundle
/// (Hearthstone.app/Contents/MacOS/Hearthstone); the install directory is the
/// folder CONTAINING the bundle, so we walk up past it.
pub fn install_dir_from_exe(exe: &Path) -> Option<PathBuf> {
    let exe_str = exe.to_str()?;
    let mut dir = parent_str(exe_str)?;
    loop {
        let name = file_name_str(dir)?;
        if name.ends_with(".app") {
            return parent_str(dir).map(PathBuf::from);
        }
        if name == "Contents" || name == "MacOS" {
            dir = parent_str(dir)?;
            continue;
        }
        return Some(PathBuf::from(dir));
    }
}

/// The most recent session folder among the given directory names.
///
/// The game creates ONE FOLDER PER SESSION, not one file it truncates. A
/// watcher built for a single file would miss every match after the first
/// restart, silently. The names sort chronologically because the date is
/// zero-padded and ordered from year to second.
pub fn newest_session(names: &[String]) -> Option<&str> {
    names
        .iter()
        .filter(|n| session_start(n).is_some())
        .max()
        .map(String::as_str)
}

/// The instant a session folder was created, read from its own name.
///
/// This is the date the log lines lack: they carry a time of day and nothing
/// else, so the folder is the only source for the day itself.
pub fn session_start(name: &str) -> Option<NaiveDateTime> {
    let rest = name.strip_prefix("Hearthstone_")?;
    NaiveDateTime::parse_from_str(rest, "%Y_%m_%d_%H_%M_%S").ok()
}

/// The install directory of a running Hearthstone, or None when it is not
/// running. Called once when the watcher starts, then cached in settings.
pub fn running_install_dir() -> Option<PathBuf> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );
    sys.processes().values().find_map(|p| {
        let name = p.name().to_string_lossy().to_lowercase();
        if name == "hearthstone.exe" || name == "hearthstone" {
            install_dir_from_exe(p.exe()?)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn install_dir_is_the_executable_parent() {
        assert_eq!(
            install_dir_from_exe(Path::new(r"C:\Battle.net\Hearthstone\Hearthstone.exe")),
            Some(Path::new(r"C:\Battle.net\Hearthstone").to_path_buf())
        );
    }

    #[test]
    fn macos_bundle_walks_up_to_the_app_folder() {
        assert_eq!(
            install_dir_from_exe(Path::new(
                "/Applications/Hearthstone/Hearthstone.app/Contents/MacOS/Hearthstone"
            )),
            Some(Path::new("/Applications/Hearthstone").to_path_buf())
        );
    }

    #[test]
    fn newest_session_is_the_latest_date() {
        let dirs = vec![
            "Hearthstone_2026_08_02_17_22_29".to_string(),
            "Hearthstone_2026_08_03_09_14_02".to_string(),
            "Hearthstone_2026_07_31_23_00_00".to_string(),
        ];
        assert_eq!(newest_session(&dirs), Some("Hearthstone_2026_08_03_09_14_02"));
    }

    #[test]
    fn unrelated_folders_are_ignored() {
        let dirs = vec!["Logs".to_string(), "Hearthstone_2026_08_02_17_22_29".into()];
        assert_eq!(newest_session(&dirs), Some("Hearthstone_2026_08_02_17_22_29"));
    }

    #[test]
    fn no_session_folder_yields_nothing() {
        let dirs = vec!["Logs".to_string()];
        assert_eq!(newest_session(&dirs), None);
    }

    #[test]
    fn session_start_parses_the_folder_date() {
        let ts = session_start("Hearthstone_2026_08_02_17_22_29").unwrap();
        assert_eq!(ts.format("%Y-%m-%d %H:%M:%S").to_string(), "2026-08-02 17:22:29");
    }

    #[test]
    fn a_malformed_folder_has_no_start() {
        assert!(session_start("Hearthstone_nope").is_none());
        assert!(session_start("autre_chose").is_none());
    }
}
