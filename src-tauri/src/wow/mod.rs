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
pub mod savedvars;

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
    /// How many characters the addon has recorded in this edition.
    pub characters: usize,
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
                characters: read_characters(&ed.dir).len(),
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
    fn every_edition_reports_to_its_catalogue_game() {
        assert_eq!(slug_for("wow"), Some("world-of-warcraft"));
        assert_eq!(
            slug_for("wow_classic_era"),
            Some("world-of-warcraft-classic")
        );
        assert_eq!(
            slug_for("wow_anniversary"),
            Some("world-of-warcraft-classic")
        );
        assert_eq!(
            slug_for("wow_classic_beta"),
            Some("world-of-warcraft-forever")
        );
        assert_eq!(slug_for("ascension"), Some("wow-ascension"));
        assert_eq!(slug_for("wowt"), None);
    }

    fn write_sv(edition: &Path, account: &str, body: &str) {
        let dir = edition
            .join("WTF")
            .join("Account")
            .join(account)
            .join("SavedVariables");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("KFire.lua"), body).unwrap();
    }

    #[test]
    fn accounts_merge_and_the_newest_record_wins() {
        let ed = std::env::temp_dir().join(format!("kfire-wow-read-{}", uuid::Uuid::new_v4()));
        let ch = |name: &str, played: i64, at: i64| {
            format!("[\"eu/R/{name}\"] = {{ [\"name\"] = \"{name}\", [\"realm\"] = \"R\", [\"realmNorm\"] = \"R\", [\"region\"] = \"eu\", [\"played\"] = {played}, [\"at\"] = {at}, }},")
        };
        write_sv(
            &ed,
            "111#1",
            &format!(
                "KFirePlayed = {{ [\"v\"] = 1, [\"chars\"] = {{ {} {} }}, }}",
                ch("Ouranos", 100, 10),
                ch("Alt", 50, 10)
            ),
        );
        write_sv(
            &ed,
            "222#1",
            &format!(
                "KFirePlayed = {{ [\"v\"] = 1, [\"chars\"] = {{ {} }}, }}",
                ch("Ouranos", 900, 20)
            ),
        );
        write_sv(&ed, "333#1", "garbage {{{");
        let mut got = read_characters(&ed);
        got.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(got.len(), 2, "two characters, the broken file skipped");
        assert_eq!(
            (got[1].name.as_str(), got[1].played_seconds),
            ("Ouranos", 900),
            "newest record wins"
        );
        let p = payload("world-of-warcraft", &got);
        assert_eq!(p["game_slug"], "world-of-warcraft");
        let c = &p["characters"][1];
        let mut keys: Vec<&str> = c.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "name",
                "played_seconds",
                "realm",
                "realm_norm",
                "recorded_at",
                "region"
            ]
        );
        assert_eq!(c["recorded_at"], "1970-01-01T00:00:20+00:00");
        std::fs::remove_dir_all(ed).unwrap();
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

// --- Reading and sending what the addon recorded ---------------------------

/// The catalogue game of an edition, as the servers know it.
pub fn slug_for(product: &str) -> Option<&'static str> {
    Some(match product {
        "wow" => "world-of-warcraft",
        "wow_classic" | "wow_classic_era" | "wow_anniversary" => "world-of-warcraft-classic",
        "wow_classic_beta" => "world-of-warcraft-forever",
        paths::ASCENSION => "wow-ascension",
        _ => return None,
    })
}

/// Every character the addon recorded in one edition, across all the game
/// accounts of this licence. A character found twice keeps its newest record.
pub fn read_characters(edition: &Path) -> Vec<savedvars::Character> {
    let mut by_key: std::collections::BTreeMap<String, savedvars::Character> = Default::default();
    let Ok(accounts) = std::fs::read_dir(edition.join("WTF").join("Account")) else {
        return Vec::new();
    };
    for acc in accounts.flatten() {
        let file = acc.path().join("SavedVariables").join("KFire.lua");
        let Ok(src) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Some(chars) = savedvars::characters(&src) else {
            log::warn!("wow: unreadable {}", file.display());
            continue;
        };
        for c in chars {
            let key = format!("{}/{}/{}", c.region, c.realm_norm, c.name);
            if by_key.get(&key).is_none_or(|old| c.at > old.at) {
                by_key.insert(key, c);
            }
        }
    }
    by_key.into_values().collect()
}

/// The match_result payload of one edition. Facts about the member's own
/// characters, nothing else.
pub fn payload(slug: &str, chars: &[savedvars::Character]) -> serde_json::Value {
    let list: Vec<serde_json::Value> = chars
        .iter()
        .map(|c| {
            let at = chrono::DateTime::from_timestamp(c.at, 0).unwrap_or_default();
            let mut m = serde_json::json!({
                "region": c.region, "realm": c.realm, "realm_norm": c.realm_norm, "name": c.name,
                "played_seconds": c.played_seconds, "recorded_at": at.to_rfc3339(),
            });
            if let Some(l) = c.level {
                m["level"] = l.into();
            }
            if let Some(cl) = &c.class {
                m["class"] = cl.clone().into();
            }
            m
        })
        .collect();
    serde_json::json!({ "game_slug": slug, "characters": list })
}

/// A short fingerprint of what was sent, to send it only once.
fn fingerprint(v: &serde_json::Value) -> String {
    use sha2::{Digest, Sha256};
    let d = Sha256::digest(v.to_string().as_bytes());
    d.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// The servers a payload goes to: those whose catalogue knows the edition,
/// and not set offline (same rule as Hearthstone and Rocket League).
fn servers_for(db: &crate::db::Db, slug: &str) -> Vec<String> {
    let global = db.get_setting("global_status").unwrap_or_default();
    let catalog: Vec<(String, String)> = db
        .load_games()
        .into_iter()
        .map(|(id, g)| (id, g.slug))
        .collect();
    db.list_servers()
        .into_iter()
        .filter(|s| {
            crate::status::effective_status(&global, &s.status_override) != "offline"
                && catalog.iter().any(|(sid, g)| sid == &s.id && g == slug)
        })
        .map(|s| s.id)
        .collect()
}

/// Reads what the addon recorded in every edition and queues it for the
/// servers that have not received this exact list yet. Returns whether
/// anything was queued, for the caller to wake the sender.
pub fn report(db: &crate::db::Db) -> bool {
    let _guard = SYNC.lock().unwrap_or_else(|e| e.into_inner());
    if !supported() || !enabled(db) {
        return false;
    }
    let (root, asc) = (root(db), ascension_dir(db));
    let mut by_slug: std::collections::BTreeMap<&str, Vec<savedvars::Character>> =
        Default::default();
    for (ed, _) in targets(root.as_deref(), asc.as_deref()) {
        let Some(slug) = slug_for(&ed.product) else {
            continue;
        };
        by_slug
            .entry(slug)
            .or_default()
            .extend(read_characters(&ed.dir));
    }
    let mut queued = false;
    let now = chrono::Utc::now().to_rfc3339();
    for (slug, chars) in by_slug {
        if chars.is_empty() {
            continue;
        }
        let p = payload(slug, &chars);
        let fp = fingerprint(&p);
        for server in servers_for(db, slug) {
            let key = format!("wow_played_sent:{server}:{slug}");
            if db.get_setting(&key).as_deref() == Some(fp.as_str()) {
                continue;
            }
            db.queue_event(&server, "match_result", slug, &now, Some(&p.to_string()));
            db.set_setting(&key, &fp);
            queued = true;
            log::info!(
                "wow: queued {} characters of {slug} for {server}",
                chars.len()
            );
        }
    }
    queued
}
