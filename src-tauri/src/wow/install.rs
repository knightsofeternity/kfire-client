//! Putting the addon into an edition's `Interface/AddOns`, and taking it out.
//! A `KFire` folder without our marker belongs to someone else: never touched.

use super::addon;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    /// At least one file was written.
    Installed,
    /// Everything was already as it should be.
    UpToDate,
    /// A `KFire` folder we did not create: left alone.
    Foreign,
    /// Our folder was deleted.
    Removed,
    /// Nothing to delete.
    Absent,
}

/// Where our addon would sit in this edition.
pub fn addon_dir(edition: &Path) -> PathBuf {
    edition.join("Interface").join("AddOns").join(addon::FOLDER)
}

fn files(interface: Option<u32>) -> [(&'static str, String); 3] {
    [
        (addon::MARKER, addon::MARKER_TEXT.to_string()),
        ("KFire.toc", addon::toc_for(interface)),
        ("KFire.lua", addon::LUA.to_string()),
    ]
}

/// Writes whatever differs. The marker goes first, so an interrupted install
/// is still recognised as ours next time.
pub fn install(edition: &Path, interface: Option<u32>) -> std::io::Result<Outcome> {
    let dir = addon_dir(edition);
    if dir.exists() && !dir.join(addon::MARKER).exists() {
        return Ok(Outcome::Foreign);
    }
    std::fs::create_dir_all(&dir)?;
    let mut changed = false;
    for (name, content) in files(interface) {
        let path = dir.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(content.as_str()) {
            std::fs::write(&path, content)?;
            changed = true;
        }
    }
    Ok(if changed {
        Outcome::Installed
    } else {
        Outcome::UpToDate
    })
}

/// Deletes our folder, only if it carries our marker.
pub fn uninstall(edition: &Path) -> std::io::Result<Outcome> {
    let dir = addon_dir(edition);
    if !dir.exists() {
        return Ok(Outcome::Absent);
    }
    if !dir.join(addon::MARKER).exists() {
        return Ok(Outcome::Foreign);
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(Outcome::Removed)
}

/// Read-only view of an edition, for the interface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    /// Ours, and exactly the files this client would write.
    Installed,
    /// Absent or out of date while the member wants it: the next sync writes it.
    Pending,
    /// Someone else's `KFire` folder.
    Foreign,
    /// Absent and not wanted.
    Off,
}

pub fn state(edition: &Path, interface: Option<u32>, enabled: bool) -> State {
    let dir = addon_dir(edition);
    if dir.exists() && !dir.join(addon::MARKER).exists() {
        return State::Foreign;
    }
    let current = files(interface).iter().all(|(name, content)| {
        std::fs::read_to_string(dir.join(name)).ok().as_deref() == Some(content.as_str())
    });
    match (enabled, dir.exists(), current) {
        (_, true, true) if enabled => State::Installed,
        (true, _, _) => State::Pending,
        (false, true, _) => State::Pending, // ours, still there, will be removed
        (false, false, _) => State::Off,
    }
}

/// The newest `KFire.lua` saved by the game, across every account folder of
/// this edition: proof that the addon ran and the game wrote its data.
pub fn saved_at(edition: &Path) -> Option<std::time::SystemTime> {
    let accounts = std::fs::read_dir(edition.join("WTF").join("Account")).ok()?;
    accounts
        .flatten()
        .filter_map(|a| {
            std::fs::metadata(a.path().join("SavedVariables").join("KFire.lua"))
                .and_then(|m| m.modified())
                .ok()
        })
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edition(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kfire-wow-inst-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn install_writes_the_three_files_then_is_idempotent() {
        let ed = edition("idem");
        assert_eq!(install(&ed, Some(120100)).unwrap(), Outcome::Installed);
        let dir = addon_dir(&ed);
        assert!(dir.join(".kfire").exists());
        assert!(std::fs::read_to_string(dir.join("KFire.toc"))
            .unwrap()
            .contains("## Interface: 120100\n"));
        assert_eq!(
            std::fs::read_to_string(dir.join("KFire.lua")).unwrap(),
            addon::LUA
        );
        assert_eq!(install(&ed, Some(120100)).unwrap(), Outcome::UpToDate);
        assert_eq!(state(&ed, Some(120100), true), State::Installed);
        std::fs::remove_dir_all(ed).unwrap();
    }

    #[test]
    fn a_game_update_rewrites_the_interface_line() {
        let ed = edition("upd");
        install(&ed, Some(120100)).unwrap();
        assert_eq!(state(&ed, Some(120105), true), State::Pending);
        assert_eq!(install(&ed, Some(120105)).unwrap(), Outcome::Installed);
        assert!(std::fs::read_to_string(addon_dir(&ed).join("KFire.toc"))
            .unwrap()
            .contains("120105"));
        std::fs::remove_dir_all(ed).unwrap();
    }

    #[test]
    fn someone_elses_kfire_folder_is_never_touched() {
        let ed = edition("foreign");
        let dir = addon_dir(&ed);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("KFire.lua"), "-- not ours").unwrap();
        assert_eq!(install(&ed, None).unwrap(), Outcome::Foreign);
        assert_eq!(uninstall(&ed).unwrap(), Outcome::Foreign);
        assert_eq!(state(&ed, None, true), State::Foreign);
        assert_eq!(
            std::fs::read_to_string(dir.join("KFire.lua")).unwrap(),
            "-- not ours"
        );
        std::fs::remove_dir_all(ed).unwrap();
    }

    #[test]
    fn uninstall_removes_only_our_folder() {
        let ed = edition("uninst");
        let other = ed.join("Interface").join("AddOns").join("DBM-Core");
        std::fs::create_dir_all(&other).unwrap();
        install(&ed, None).unwrap();
        assert_eq!(uninstall(&ed).unwrap(), Outcome::Removed);
        assert!(!addon_dir(&ed).exists());
        assert!(other.exists());
        assert_eq!(uninstall(&ed).unwrap(), Outcome::Absent);
        assert_eq!(state(&ed, None, false), State::Off);
        std::fs::remove_dir_all(ed).unwrap();
    }

    #[test]
    fn saved_variables_are_found_in_any_account() {
        let ed = edition("sv");
        assert!(saved_at(&ed).is_none());
        let sv = ed
            .join("WTF")
            .join("Account")
            .join("12345#1")
            .join("SavedVariables");
        std::fs::create_dir_all(&sv).unwrap();
        std::fs::write(sv.join("KFire.lua"), "KFirePlayed = {}").unwrap();
        assert!(saved_at(&ed).is_some());
        std::fs::remove_dir_all(ed).unwrap();
    }
}
