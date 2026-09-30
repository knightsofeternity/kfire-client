//! World of Warcraft: Blizzard publishes no playtime anywhere, so the client
//! installs a small addon (`wow-addon/KFire`) that records each character's
//! /played, and keeps it up to date in every playable edition of the game,
//! Project Ascension included.
//!
//! This version only installs the addon. Reading what it saved and sending it
//! comes next.

pub mod addon;
pub mod buildinfo;
pub mod install;
pub mod paths;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Setting: "1" when the member wants the addon.
pub const SETTING: &str = "wow_addon";
/// Settings: the last WoW root and Ascension folder found.
const ROOT_SETTING: &str = "wow_root";
const ASCENSION_SETTING: &str = "wow_ascension_dir";
/// Setting: the last sync error, empty when the last sync went fine.
const ERROR_SETTING: &str = "wow_last_error";

/// One sync at a time: a WoW launch is reported once per linked server, and
/// each report triggers a sync.
static SYNC: Mutex<()> = Mutex::new(());

/// Whether a catalogue slug is one of the WoW editions (retail, classic,
/// forever, whatever Blizzard adds under the same name) or Ascension.
pub fn is_wow_slug(slug: &str) -> bool {
    slug == "world-of-warcraft" || slug.starts_with("world-of-warcraft-") || slug == "wow-ascension"
}

/// Whether this OS can run WoW at all.
pub fn supported() -> bool {
    cfg!(windows) || cfg!(target_os = "macos")
}

/// A name the member recognises.
pub fn label(product: &str) -> String {
    match product {
        "wow" => "Retail",
        "wow_classic" => "Classic",
        "wow_classic_era" => "Classic Era",
        "wow_anniversary" => "Anniversary",
        "wow_classic_beta" => "Forever",
        paths::ASCENSION => "Ascension",
        other => other,
    }
    .to_string()
}

/// The interface number of a Blizzard edition: `.build.info` first, the
/// game's own `Config.wtf` otherwise.
pub fn interface_for(root: &Path, ed: &paths::Edition) -> Option<u32> {
    std::fs::read_to_string(root.join(".build.info"))
        .ok()
        .and_then(|bi| buildinfo::product_interface(&bi, &ed.product))
        .or_else(|| config_interface(&ed.dir))
}

fn config_interface(dir: &Path) -> Option<u32> {
    std::fs::read_to_string(dir.join("WTF").join("Config.wtf"))
        .ok()
        .and_then(|c| buildinfo::last_addon_version(&c))
}

/// Every folder the addon goes into, with the interface number its client
/// expects.
pub fn targets(
    root: Option<&Path>,
    ascension: Option<&Path>,
) -> Vec<(paths::Edition, Option<u32>)> {
    let mut out: Vec<(paths::Edition, Option<u32>)> = root
        .map(|r| {
            paths::editions(r)
                .into_iter()
                .map(|ed| {
                    let i = interface_for(r, &ed);
                    (ed, i)
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(dir) = ascension {
        let i = config_interface(dir).unwrap_or(paths::ASCENSION_INTERFACE);
        out.push((
            paths::Edition {
                dir: dir.to_path_buf(),
                product: paths::ASCENSION.to_string(),
            },
            Some(i),
        ));
    }
    out
}

/// The WoW root: a running client tells the truth, then the last one found,
/// then the usual places. The answer is remembered.
fn root(db: &crate::db::Db) -> Option<PathBuf> {
    let saved = db
        .get_setting(ROOT_SETTING)
        .map(PathBuf::from)
        .filter(|p| paths::is_wow_root(p));
    let found = paths::running_root()
        .or(saved)
        .or_else(paths::common_root)?;
    db.set_setting(ROOT_SETTING, &found.to_string_lossy());
    Some(found)
}

/// Same for Ascension's folder.
fn ascension_dir(db: &crate::db::Db) -> Option<PathBuf> {
    let saved = db
        .get_setting(ASCENSION_SETTING)
        .map(PathBuf::from)
        .filter(|p| paths::is_ascension_dir(p));
    let found = paths::running_ascension()
        .or(saved)
        .or_else(paths::common_ascension)?;
    db.set_setting(ASCENSION_SETTING, &found.to_string_lossy());
    Some(found)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EditionStatus {
    pub product: String,
    pub label: String,
    pub dir: String,
    pub interface: Option<u32>,
    /// "installed", "pending", "foreign" or "off".
    pub state: String,
    /// When the game last saved the addon's data (RFC 3339), if ever.
    pub saved_at: Option<String>,
}

/// Installs (enabled) or removes (disabled) the addon in every target.
/// Returns the first error, after trying them all.
pub fn sync_targets(
    targets: &[(paths::Edition, Option<u32>)],
    enabled: bool,
) -> Result<(), String> {
    let mut first_err = None;
    for (ed, interface) in targets {
        let res = if enabled {
            install::install(&ed.dir, *interface)
        } else {
            install::uninstall(&ed.dir)
        };
        match res {
            Ok(outcome) => log::info!("wow: {} -> {:?}", ed.product, outcome),
            Err(e) => {
                log::warn!("wow: {} failed: {e}", ed.product);
                first_err.get_or_insert(format!("{}: {e}", label(&ed.product)));
            }
        }
    }
    first_err.map_or(Ok(()), Err)
}

/// Read-only state of every target.
pub fn status_targets(
    targets: &[(paths::Edition, Option<u32>)],
    enabled: bool,
) -> Vec<EditionStatus> {
    targets
        .iter()
        .map(|(ed, interface)| {
            let state = match install::state(&ed.dir, *interface, enabled) {
                install::State::Installed => "installed",
                install::State::Pending => "pending",
                install::State::Foreign => "foreign",
                install::State::Off => "off",
            };
            EditionStatus {
                product: ed.product.clone(),
                label: label(&ed.product),
                dir: ed.dir.to_string_lossy().to_string(),
                interface: *interface,
                state: state.to_string(),
                saved_at: install::saved_at(&ed.dir)
                    .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()),
            }
        })
        .collect()
}

pub fn enabled(db: &crate::db::Db) -> bool {
    db.get_setting(SETTING).as_deref() == Some("1")
}

/// Brings every target in line with the member's choice. Safe to call from any
/// thread, as often as wanted: it only writes what differs.
pub fn sync(db: &crate::db::Db) {
    let _guard = SYNC.lock().unwrap_or_else(|e| e.into_inner());
    if !supported() {
        return;
    }
    let (root, asc) = (root(db), ascension_dir(db));
    if root.is_none() && asc.is_none() {
        log::info!("wow: neither World of Warcraft nor Ascension found");
        return;
    }
    let err = sync_targets(&targets(root.as_deref(), asc.as_deref()), enabled(db))
        .err()
        .unwrap_or_default();
    db.set_setting(ERROR_SETTING, &err);
}

/// What the interface shows.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Status {
    pub supported: bool,
    pub enabled: bool,
    pub root: Option<String>,
    pub ascension: Option<String>,
    pub error: Option<String>,
    pub editions: Vec<EditionStatus>,
}

pub fn status(db: &crate::db::Db) -> Status {
    let enabled = enabled(db);
    let (root, asc) = if supported() {
        (root(db), ascension_dir(db))
    } else {
        (None, None)
    };
    Status {
        supported: supported(),
        enabled,
        editions: status_targets(&targets(root.as_deref(), asc.as_deref()), enabled),
        root: root.map(|r| r.to_string_lossy().to_string()),
        ascension: asc.map(|a| a.to_string_lossy().to_string()),
        error: db.get_setting(ERROR_SETTING).filter(|e| !e.is_empty()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wow::paths::tests::fake_root;

    fn toc(dir: &Path) -> Option<String> {
        std::fs::read_to_string(dir.join("Interface/AddOns/KFire/KFire.toc")).ok()
    }

    #[test]
    fn wow_slugs() {
        assert!(is_wow_slug("world-of-warcraft"));
        assert!(is_wow_slug("world-of-warcraft-classic"));
        assert!(is_wow_slug("world-of-warcraft-forever"));
        assert!(is_wow_slug("wow-ascension"));
        assert!(!is_wow_slug("warcraft-iii-reign-of-chaos"));
        assert!(!is_wow_slug("world-of-warships"));
    }

    #[test]
    fn labels() {
        assert_eq!(label("wow"), "Retail");
        assert_eq!(label("wow_classic_beta"), "Forever");
        assert_eq!(label("ascension"), "Ascension");
        assert_eq!(label("wow_forever"), "wow_forever");
    }

    #[test]
    fn sync_installs_in_playable_editions_only_with_their_own_interface() {
        let root = fake_root(
            "sync",
            &[
                ("_retail_", Some("wow")),
                ("_classic_beta_", Some("wow_classic_beta")),
                ("_ptr_", Some("wowt")),
            ],
        );
        let t = targets(Some(&root), None);
        sync_targets(&t, true).unwrap();
        assert!(toc(&root.join("_retail_"))
            .unwrap()
            .contains("## Interface: 120100\n"));
        assert!(toc(&root.join("_classic_beta_"))
            .unwrap()
            .contains("## Interface: 16001\n"));
        assert!(
            toc(&root.join("_ptr_")).is_none(),
            "never into a test realm"
        );

        let st = status_targets(&t, true);
        assert_eq!(st.len(), 2);
        assert!(st.iter().all(|e| e.state == "installed"));
        assert_eq!(
            st.iter()
                .find(|e| e.product == "wow_classic_beta")
                .unwrap()
                .label,
            "Forever"
        );

        sync_targets(&t, false).unwrap();
        assert!(toc(&root.join("_retail_")).is_none());
        assert!(status_targets(&t, false).iter().all(|e| e.state == "off"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ascension_gets_the_addon_with_a_3_3_5_interface() {
        let asc = std::env::temp_dir().join(format!("kfire-wow-asc-sync-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&asc).unwrap();
        std::fs::write(asc.join("Ascension.exe"), b"").unwrap();
        let t = targets(None, Some(&asc));
        assert_eq!(t.len(), 1);
        sync_targets(&t, true).unwrap();
        assert!(toc(&asc).unwrap().contains("## Interface: 30300\n"));
        let st = status_targets(&t, true);
        assert_eq!(
            (st[0].label.as_str(), st[0].state.as_str()),
            ("Ascension", "installed")
        );
        std::fs::remove_dir_all(asc).unwrap();
    }

    #[test]
    fn config_wtf_is_the_fallback_interface() {
        let root = fake_root("cfg", &[("_classic_", Some("wow_classic"))]);
        let wtf = root.join("_classic_").join("WTF");
        std::fs::create_dir_all(&wtf).unwrap();
        std::fs::write(wtf.join("Config.wtf"), "SET lastAddonVersion \"50503\"\n").unwrap();
        let ed = paths::editions(&root).pop().unwrap();
        assert_eq!(interface_for(&root, &ed), Some(50503));
        std::fs::remove_dir_all(root).unwrap();
    }
}
