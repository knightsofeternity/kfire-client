//! Where World of Warcraft lives: one root (`World of Warcraft/`) holding one
//! folder per edition (`_retail_`, `_classic_era_`, `_classic_beta_`...).

use super::buildinfo;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Edition {
    pub dir: PathBuf,
    pub product: String,
}

/// Editions the addon goes into. Test realms (PTR, beta, alpha) and Blizzard's
/// internal folders are left alone: their characters are copies, their /played
/// would be wrong. Forever lives in `wow_classic_beta` while it is in beta.
pub fn is_playable(product: &str) -> bool {
    match product {
        "wow" | "wow_classic" | "wow_classic_era" | "wow_anniversary" | "wow_classic_beta" => true,
        p => {
            p.starts_with("wow_")
                && ![
                    "ptr",
                    "beta",
                    "alpha",
                    "dev",
                    "vendor",
                    "test",
                    "event",
                    "submission",
                ]
                .iter()
                .any(|x| p.contains(x))
        }
    }
}

/// Fallback when an edition folder has no `.flavor.info`.
fn product_of_subfolder(name: &str) -> Option<&'static str> {
    Some(match name.to_ascii_lowercase().as_str() {
        "_retail_" => "wow",
        "_classic_" => "wow_classic",
        "_classic_era_" => "wow_classic_era",
        "_anniversary_" => "wow_anniversary",
        "_classic_beta_" => "wow_classic_beta",
        "_ptr_" => "wowt",
        "_beta_" => "wow_beta",
        "_xptr_" => "wowxptr",
        "_classic_ptr_" => "wow_classic_ptr",
        "_classic_era_ptr_" => "wow_classic_era_ptr",
        _ => return None,
    })
}

/// The playable editions under a root, sorted by folder.
pub fn editions(root: &Path) -> Vec<Edition> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let dir = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if !dir.is_dir() || name.len() < 3 || !name.starts_with('_') || !name.ends_with('_') {
            continue;
        }
        let product = std::fs::read_to_string(dir.join(".flavor.info"))
            .ok()
            .and_then(|t| buildinfo::flavor_product(&t))
            .or_else(|| product_of_subfolder(&name).map(str::to_string));
        if let Some(product) = product.filter(|p| is_playable(p)) {
            out.push(Edition { dir, product });
        }
    }
    out.sort_by(|a, b| a.dir.cmp(&b.dir));
    out
}

/// Whether this folder is a World of Warcraft root.
pub fn is_wow_root(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join(".build.info"))
        .map(|t| buildinfo::lists_wow(&t))
        .unwrap_or(false)
}

/// Strips the last element off a path, on `/` AND `\`, whatever the host: a
/// Windows path handed to `Path` on Linux (our CI) would not split at all.
fn parent_str(s: &str) -> Option<&str> {
    let idx = s.rfind(['/', '\\'])?;
    Some(&s[..idx])
}

/// The root of an edition executable: `<root>/<_edition_>/Wow.exe`.
pub fn root_of_exe(exe: &str) -> Option<String> {
    let edition = parent_str(exe)?;
    let root = parent_str(edition)?;
    if root.is_empty() {
        None
    } else {
        Some(root.to_string())
    }
}

/// The root of a WoW client running right now, if any. Every edition's
/// executable starts with "wow" (Wow.exe, WowClassic.exe, WowB.exe; on macOS
/// "World of Warcraft" inside an .app bundle, handled by the common roots).
pub fn running_root() -> Option<PathBuf> {
    use sysinfo::{ProcessRefreshKind, RefreshKind, System, UpdateKind};
    let sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_processes(ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet)),
    );
    sys.processes().values().find_map(|p| {
        let name = p.name().to_string_lossy().to_lowercase();
        if !name.starts_with("wow") {
            return None;
        }
        let exe = p.exe()?.to_string_lossy().to_string();
        let root = PathBuf::from(root_of_exe(&exe)?);
        is_wow_root(&root).then_some(root)
    })
}

/// The usual install locations, first one that is really a WoW root.
pub fn common_root() -> Option<PathBuf> {
    candidates().into_iter().find(|d| is_wow_root(d))
}

fn candidates() -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        return vec![PathBuf::from("/Applications/World of Warcraft")];
    }
    if !cfg!(windows) {
        return Vec::new();
    }
    let subs = [
        r"Program Files (x86)\World of Warcraft",
        r"Program Files\World of Warcraft",
        r"World of Warcraft",
        r"Games\World of Warcraft",
        r"Jeux\World of Warcraft",
        r"Blizzard\World of Warcraft",
    ];
    ('C'..='Z')
        .flat_map(|d| {
            subs.iter()
                .map(move |s| PathBuf::from(format!(r"{d}:\{s}")))
        })
        .collect()
}

/// Project Ascension, an unofficial server on a 3.3.5 client: no edition
/// folders and no `.build.info`. `Ascension.exe`, `Interface` and `WTF` sit in
/// one folder, `...\Ascension Launcher\resources\ascension-live` (formerly
/// `resources\client`).
pub const ASCENSION: &str = "ascension";
/// The interface number of a 3.3.5 client.
pub const ASCENSION_INTERFACE: u32 = 30300;

pub fn is_ascension_dir(dir: &Path) -> bool {
    ["Ascension.exe", "ascension.exe"]
        .iter()
        .any(|f| dir.join(f).is_file())
}

/// Ascension's folder, from its running client.
pub fn running_ascension() -> Option<PathBuf> {
    use sysinfo::{ProcessRefreshKind, RefreshKind, System, UpdateKind};
    let sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_processes(ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet)),
    );
    sys.processes().values().find_map(|p| {
        if !p
            .name()
            .to_string_lossy()
            .eq_ignore_ascii_case("ascension.exe")
        {
            return None;
        }
        let exe = p.exe()?.to_string_lossy().to_string();
        let dir = PathBuf::from(parent_str(&exe)?);
        is_ascension_dir(&dir).then_some(dir)
    })
}

/// The launcher's usual folders (Windows only: Ascension has no other client).
pub fn common_ascension() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    [
        r"C:\Program Files\Ascension Launcher",
        r"C:\Program Files (x86)\Ascension Launcher",
    ]
    .iter()
    .flat_map(|base| {
        [r"resources\ascension-live", r"resources\client"]
            .iter()
            .map(move |sub| PathBuf::from(format!(r"{base}\{sub}")))
    })
    .find(|d| is_ascension_dir(d))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::wow::buildinfo::tests::build_info;

    /// A fake WoW root in the temp dir: `.build.info` plus the given editions,
    /// each as (folder, Some(product in .flavor.info) or None).
    pub(crate) fn fake_root(tag: &str, editions: &[(&str, Option<&str>)]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("kfire-wow-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(".build.info"),
            build_info(&[
                ("12.1.0.70112", "wow"),
                ("1.15.9.70000", "wow_classic_era"),
                ("1.60.1.69913", "wow_classic_beta"),
                ("12.1.5.70200", "wowt"),
            ]),
        )
        .unwrap();
        for (folder, flavor) in editions {
            let d = root.join(folder);
            std::fs::create_dir_all(&d).unwrap();
            if let Some(p) = flavor {
                std::fs::write(
                    d.join(".flavor.info"),
                    format!("Product Flavor!STRING:0\n{p}\n"),
                )
                .unwrap();
            }
        }
        root
    }

    #[test]
    fn playable_products() {
        for p in [
            "wow",
            "wow_classic",
            "wow_classic_era",
            "wow_anniversary",
            "wow_classic_beta",
            "wow_forever",
        ] {
            assert!(is_playable(p), "{p} should be playable");
        }
        for p in [
            "wowt",
            "wow_beta",
            "wowxptr",
            "wow_classic_ptr",
            "wow_classic_era_ptr",
            "wowdev",
            "wowv3",
            "wow_vendor",
        ] {
            assert!(!is_playable(p), "{p} should be skipped");
        }
    }

    #[test]
    fn editions_are_found_by_flavor_info_then_by_folder_name() {
        let root = fake_root(
            "eds",
            &[
                ("_retail_", Some("wow")),
                ("_classic_era_", None),
                ("_classic_beta_", Some("wow_classic_beta")),
                ("_ptr_", Some("wowt")),
                ("Data", None),
                ("_unknown_", None),
            ],
        );
        let found: Vec<(String, String)> = editions(&root)
            .into_iter()
            .map(|e| {
                (
                    e.dir.file_name().unwrap().to_string_lossy().to_string(),
                    e.product,
                )
            })
            .collect();
        assert_eq!(
            found,
            vec![
                ("_classic_beta_".to_string(), "wow_classic_beta".to_string()),
                ("_classic_era_".to_string(), "wow_classic_era".to_string()),
                ("_retail_".to_string(), "wow".to_string()),
            ]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_root_is_recognised_by_its_build_info() {
        let root = fake_root("root", &[]);
        assert!(is_wow_root(&root));
        assert!(!is_wow_root(&root.join("nope")));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_ascension_folder_is_recognised_by_its_executable() {
        let d = std::env::temp_dir().join(format!("kfire-wow-asc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        assert!(!is_ascension_dir(&d));
        std::fs::write(d.join("Ascension.exe"), b"").unwrap();
        assert!(is_ascension_dir(&d));
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn the_root_is_two_levels_above_the_executable() {
        assert_eq!(
            root_of_exe(r"C:\Program Files (x86)\World of Warcraft\_retail_\Wow.exe").as_deref(),
            Some(r"C:\Program Files (x86)\World of Warcraft")
        );
        assert_eq!(
            root_of_exe("/Applications/World of Warcraft/_classic_era_/WowClassic.exe").as_deref(),
            Some("/Applications/World of Warcraft")
        );
        assert_eq!(root_of_exe("Wow.exe"), None);
        assert_eq!(root_of_exe(r"\Wow.exe"), None);
    }
}
