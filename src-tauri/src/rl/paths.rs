//! Where Rocket League lives on this machine, and where it reads its config.
//!
//! Unlike Hearthstone, whose configuration file lives in LOCALAPPDATA, this one
//! lives INSIDE the install directory. So nothing can be written until the
//! installation has been found.

use std::path::PathBuf;

/// The game's executable, as the scanner sees it.
pub const EXE: &str = "RocketLeague.exe";

/// The common install locations, taken from `ke-rl-tracker` where they are
/// proven in production. Only probed when the process is not there to tell us
/// the truth.
pub const COMMON_DIRS: &[&str] = &[
    r"C:\Program Files (x86)\Steam\steamapps\common\rocketleague",
    r"C:\Program Files\Steam\steamapps\common\rocketleague",
    r"C:\Program Files\Epic Games\rocketleague",
    r"C:\Program Files (x86)\Epic Games\rocketleague",
];

/// Strips the last element off a path, treating `/` AND `\` as separators,
/// whatever the host platform.
///
/// A Windows path picked up on a member's machine keeps that machine's
/// separators. Handed to `std::path::Path` on Linux, which is our CI and this
/// development machine, it would not be split at all, since Unix sees `\` as an
/// ordinary character. Same reason and same remedy as in `hs/paths.rs`.
fn parent_str(s: &str) -> Option<&str> {
    let idx = s.rfind(['/', '\\'])?;
    Some(&s[..idx])
}

/// The install directory that holds this executable.
///
/// The game runs from `<install>/Binaries/Win64/RocketLeague.exe`, so the
/// installation is three levels above it.
pub fn install_dir_of(exe: &str) -> Option<String> {
    let win64 = parent_str(exe)?;
    let binaries = parent_str(win64)?;
    let install = parent_str(binaries)?;
    if install.is_empty() {
        return None;
    }
    Some(install.to_string())
}

/// The configuration file inside an install directory.
///
/// The directory's separator is kept: we do not mix styles in a path we are
/// going to show the member for him to recognise.
pub fn config_path_in(install: &str) -> String {
    let sep = if install.contains('\\') { '\\' } else { '/' };
    format!("{install}{sep}TAGame{sep}Config{sep}DefaultStatsAPI.ini")
}

/// The install directory worked out from the running process, if it is up.
pub fn running_install_dir() -> Option<PathBuf> {
    use sysinfo::{ProcessRefreshKind, RefreshKind, System, UpdateKind};
    let sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_processes(ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet)),
    );
    for p in sys.processes().values() {
        if p.name().to_string_lossy().eq_ignore_ascii_case(EXE) {
            let exe = p.exe()?.to_string_lossy().to_string();
            return install_dir_of(&exe).map(PathBuf::from);
        }
    }
    None
}

/// The first common location that really exists.
pub fn common_install_dir() -> Option<PathBuf> {
    COMMON_DIRS.iter().map(PathBuf::from).find(|d| d.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_install_dir_is_three_levels_above_the_executable() {
        // The game runs from <install>/Binaries/Win64/RocketLeague.exe
        let exe = r"C:\Program Files (x86)\Steam\steamapps\common\rocketleague\Binaries\Win64\RocketLeague.exe";
        assert_eq!(
            install_dir_of(exe).as_deref(),
            Some(r"C:\Program Files (x86)\Steam\steamapps\common\rocketleague")
        );
    }

    #[test]
    fn a_windows_path_is_split_on_backslashes_even_on_unix() {
        // This test runs on Linux in CI. Path::parent would split nothing there.
        let exe = r"D:\Games\rocketleague\Binaries\Win64\RocketLeague.exe";
        assert_eq!(
            install_dir_of(exe).as_deref(),
            Some(r"D:\Games\rocketleague")
        );
    }

    #[test]
    fn a_unix_path_works_too() {
        let exe = "/home/x/rocketleague/Binaries/Win64/RocketLeague.exe";
        assert_eq!(install_dir_of(exe).as_deref(), Some("/home/x/rocketleague"));
    }

    #[test]
    fn a_path_too_shallow_yields_nothing() {
        assert_eq!(install_dir_of("RocketLeague.exe"), None);
        assert_eq!(install_dir_of(r"C:\RocketLeague.exe"), None);
    }

    #[test]
    fn the_config_path_hangs_off_the_install_dir() {
        assert_eq!(
            config_path_in(r"C:\Games\rocketleague"),
            r"C:\Games\rocketleague\TAGame\Config\DefaultStatsAPI.ini"
        );
    }

    #[test]
    fn a_unix_install_dir_keeps_unix_separators() {
        assert_eq!(
            config_path_in("/home/x/rocketleague"),
            "/home/x/rocketleague/TAGame/Config/DefaultStatsAPI.ini"
        );
    }
}
