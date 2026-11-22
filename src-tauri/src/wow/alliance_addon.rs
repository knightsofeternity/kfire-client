//! Appel d'Alliance, the group call addon shared by Knights of Eternity and
//! Les Cassos de la Lumière in WoW Forever. It lives in its own repo
//! (`wow-addon/appel-alliance`, the wow-appel-alliance submodule) and is
//! embedded at compile time like KFIRE Guilde. It only goes into Forever
//! editions, the same ones as KFIRE Guilde.

use std::path::{Path, PathBuf};

/// Folder name under `Interface/AddOns`.
pub const FOLDER: &str = "AppelAlliance";

pub const TOC: &str =
    include_str!("../../../wow-addon/appel-alliance/AppelAlliance/AppelAlliance.toc");

/// Every Lua file the TOC loads, in order. A test checks this list against the TOC.
pub const LUA: &[(&str, &str)] = &[
    (
        "Config.lua",
        include_str!("../../../wow-addon/appel-alliance/AppelAlliance/Config.lua"),
    ),
    (
        "Socle.lua",
        include_str!("../../../wow-addon/appel-alliance/AppelAlliance/Socle.lua"),
    ),
    (
        "Reseau.lua",
        include_str!("../../../wow-addon/appel-alliance/AppelAlliance/Reseau.lua"),
    ),
    (
        "Presence.lua",
        include_str!("../../../wow-addon/appel-alliance/AppelAlliance/Presence.lua"),
    ),
    (
        "Annonces.lua",
        include_str!("../../../wow-addon/appel-alliance/AppelAlliance/Annonces.lua"),
    ),
    (
        "Interface.lua",
        include_str!("../../../wow-addon/appel-alliance/AppelAlliance/Interface.lua"),
    ),
    (
        "Toast.lua",
        include_str!("../../../wow-addon/appel-alliance/AppelAlliance/Toast.lua"),
    ),
    (
        "Formulaire.lua",
        include_str!("../../../wow-addon/appel-alliance/AppelAlliance/Formulaire.lua"),
    ),
    (
        "Accueil.lua",
        include_str!("../../../wow-addon/appel-alliance/AppelAlliance/Accueil.lua"),
    ),
];

pub use super::guild_addon::is_target;

pub fn addon_dir(edition: &Path) -> PathBuf {
    edition.join("Interface").join("AddOns").join(FOLDER)
}

/// Every file of the addon, as the client writes it.
pub fn files(interface: Option<u32>) -> Vec<(String, String)> {
    let mut out = vec![
        (
            super::addon::MARKER.to_string(),
            super::addon::MARKER_TEXT.to_string(),
        ),
        (
            format!("{FOLDER}.toc"),
            super::addon::set_interface(TOC, interface),
        ),
    ];
    out.extend(LUA.iter().map(|(n, c)| (n.to_string(), c.to_string())));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_file_of_the_toc_is_embedded() {
        let listed: Vec<&str> = TOC
            .lines()
            .map(str::trim)
            .filter(|l| l.ends_with(".lua") && !l.starts_with('#'))
            .collect();
        let embedded: Vec<&str> = LUA.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            listed, embedded,
            "the TOC and the embedded files must match, in order"
        );
        assert!(TOC.contains("## SavedVariablesPerCharacter: AppelAllianceDB"));
    }

    #[test]
    fn the_toc_takes_the_exact_interface() {
        let f = files(Some(16002));
        let toc = &f.iter().find(|(n, _)| n == "AppelAlliance.toc").unwrap().1;
        assert!(toc.contains("## Interface: 16002\n"));
        assert!(f.iter().any(|(n, _)| n == "Accueil.lua"));
        assert!(f
            .iter()
            .any(|(n, c)| n == ".kfire" && c.contains("KFIRE client")));
    }
}
