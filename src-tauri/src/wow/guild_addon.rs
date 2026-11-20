//! KFIRE Guilde, the guild addon of WoW Forever, embedded in the client at
//! compile time like the KFire addon. It only goes into Forever editions.
//!
//! Next to the addon's own files the client writes `Propose.lua`, which
//! carries the member's KFIRE username: the addon offers it in its profile
//! form until the member has chosen a pseudo.

use std::path::{Path, PathBuf};

/// Folder name under `Interface/AddOns`.
pub const FOLDER: &str = "KFireGuilde";

pub const TOC: &str = include_str!("../../../wow-addon/KFireGuilde/KFireGuilde.toc");

/// Every Lua file the TOC loads, except `Propose.lua` which the client writes.
/// A test checks this list against the TOC.
pub const LUA: &[(&str, &str)] = &[
    (
        "Socle.lua",
        include_str!("../../../wow-addon/KFireGuilde/Socle.lua"),
    ),
    (
        "Reseau.lua",
        include_str!("../../../wow-addon/KFireGuilde/Reseau.lua"),
    ),
    (
        "Magasin.lua",
        include_str!("../../../wow-addon/KFireGuilde/Magasin.lua"),
    ),
    (
        "Profil.lua",
        include_str!("../../../wow-addon/KFireGuilde/Profil.lua"),
    ),
    (
        "Guilde.lua",
        include_str!("../../../wow-addon/KFireGuilde/Guilde.lua"),
    ),
    (
        "Metiers.lua",
        include_str!("../../../wow-addon/KFireGuilde/Metiers.lua"),
    ),
    (
        "Demandes.lua",
        include_str!("../../../wow-addon/KFireGuilde/Demandes.lua"),
    ),
    (
        "Evenements.lua",
        include_str!("../../../wow-addon/KFireGuilde/Evenements.lua"),
    ),
    (
        "Presence.lua",
        include_str!("../../../wow-addon/KFireGuilde/Presence.lua"),
    ),
    (
        "Quetes.lua",
        include_str!("../../../wow-addon/KFireGuilde/Quetes.lua"),
    ),
    (
        "Interface.lua",
        include_str!("../../../wow-addon/KFireGuilde/Interface.lua"),
    ),
    (
        "PageDemandes.lua",
        include_str!("../../../wow-addon/KFireGuilde/PageDemandes.lua"),
    ),
    (
        "PageEvenements.lua",
        include_str!("../../../wow-addon/KFireGuilde/PageEvenements.lua"),
    ),
    (
        "PageQuetes.lua",
        include_str!("../../../wow-addon/KFireGuilde/PageQuetes.lua"),
    ),
];

pub const PROPOSE: &str = "Propose.lua";

/// The Forever editions: the released one and the old beta.
pub fn is_target(product: &str) -> bool {
    product == "wowf" || product == "wow_classic_beta"
}

pub fn addon_dir(edition: &Path) -> PathBuf {
    edition.join("Interface").join("AddOns").join(FOLDER)
}

/// A Lua string literal, safe whatever the username holds.
fn lua_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' | '\0' => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn propose_lua(username: Option<&str>) -> String {
    let mut out = String::from(
        "-- Written by the KFIRE client: the member's KFIRE username, offered in the profile form.\n",
    );
    match username.map(str::trim).filter(|u| !u.is_empty()) {
        Some(u) => out.push_str(&format!(
            "KFireGuildePropose = {{ pseudo = {} }}\n",
            lua_string(u)
        )),
        None => out.push_str("KFireGuildePropose = nil\n"),
    }
    out
}

/// Every file of the addon, as the client writes it.
pub fn files(interface: Option<u32>, username: Option<&str>) -> Vec<(String, String)> {
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
    out.push((PROPOSE.to_string(), propose_lua(username)));
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
        let embedded: Vec<&str> = LUA.iter().map(|(n, _)| *n).chain([PROPOSE]).collect();
        assert_eq!(
            listed, embedded,
            "the TOC and the embedded files must match, in order"
        );
        assert!(TOC.contains("## SavedVariables: KFireGuildeDB"));
    }

    #[test]
    fn the_username_is_a_safe_lua_string() {
        assert_eq!(
            propose_lua(Some("Ou\"ra\\nos\n")),
            "-- Written by the KFIRE client: the member's KFIRE username, offered in the profile form.\n\
             KFireGuildePropose = { pseudo = \"Ou\\\"ra\\\\nos\" }\n"
        );
        assert!(propose_lua(None).contains("KFireGuildePropose = nil"));
        assert!(propose_lua(Some("  ")).contains("KFireGuildePropose = nil"));
    }

    #[test]
    fn only_forever_editions() {
        assert!(is_target("wowf"));
        assert!(is_target("wow_classic_beta"));
        assert!(!is_target("wow"));
        assert!(!is_target("wow_classic_era"));
        assert!(!is_target("ascension"));
    }

    #[test]
    fn the_toc_takes_the_exact_interface() {
        let f = files(Some(16002), Some("Oura"));
        let toc = &f.iter().find(|(n, _)| n == "KFireGuilde.toc").unwrap().1;
        assert!(toc.contains("## Interface: 16002\n"));
        assert!(toc.ends_with("Propose.lua\n"));
        assert!(f
            .iter()
            .any(|(n, c)| n == ".kfire" && c.contains("KFIRE client")));
    }
}
