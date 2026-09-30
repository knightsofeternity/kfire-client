//! Reading what the game says about itself: `.build.info` at the root (the
//! version of every installed product), `.flavor.info` in an edition folder
//! (which product it is) and `WTF/Config.wtf` (the interface number the game
//! last used for addons).

use std::collections::HashMap;

/// Blizzard's "bpsv": a header of `Name!TYPE:size` columns separated by `|`,
/// then one row per line. Comment lines start with `#`.
pub fn parse_bpsv(text: &str) -> Vec<HashMap<String, String>> {
    let mut lines = text
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let cols: Vec<String> = header
        .split('|')
        .map(|c| c.split('!').next().unwrap_or("").trim().to_string())
        .collect();
    lines
        .map(|l| {
            cols.iter()
                .cloned()
                .zip(l.split('|').map(|v| v.trim().to_string()))
                .collect()
        })
        .collect()
}

/// `12.1.0.70112` -> 120100. Only the first three parts count.
pub fn interface_of(version: &str) -> Option<u32> {
    let mut parts = version.trim().split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next().map_or(Some(0), |p| p.parse().ok())?;
    let patch: u32 = parts.next().map_or(Some(0), |p| p.parse().ok())?;
    Some(major * 10_000 + minor * 100 + patch)
}

/// The interface number of one product, read from `.build.info`.
pub fn product_interface(build_info: &str, product: &str) -> Option<u32> {
    parse_bpsv(build_info)
        .iter()
        .find(|r| r.get("Product").map(String::as_str) == Some(product))
        .and_then(|r| r.get("Version"))
        .and_then(|v| interface_of(v))
}

/// Whether `.build.info` lists at least one World of Warcraft product. Other
/// Blizzard games (Overwatch has a `_retail_` too) have the same file.
pub fn lists_wow(build_info: &str) -> bool {
    parse_bpsv(build_info)
        .iter()
        .any(|r| r.get("Product").is_some_and(|p| p.starts_with("wow")))
}

/// The product named by `.flavor.info`: either a bpsv with one row, or a bare
/// product code.
pub fn flavor_product(text: &str) -> Option<String> {
    text.lines()
        .map(|l| l.trim())
        .rfind(|l| !l.is_empty() && !l.starts_with('#') && !l.contains('!'))
        .map(|l| l.split('|').next().unwrap_or(l).trim().to_string())
        .filter(|p| !p.is_empty())
}

/// `SET lastAddonVersion "120100"` from `WTF/Config.wtf`.
pub fn last_addon_version(config_wtf: &str) -> Option<u32> {
    config_wtf.lines().find_map(|l| {
        l.trim()
            .strip_prefix("SET lastAddonVersion")
            .and_then(|v| v.trim().trim_matches('"').parse().ok())
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const HEADER: &str = "Branch!STRING:0|Active!DEC:1|Build Key!HEX:16|CDN Key!HEX:16|Install Key!HEX:16|IM Size!DEC:4|CDN Path!STRING:0|CDN Hosts!STRING:0|CDN Servers!STRING:0|Tags!STRING:0|Armadillo!STRING:0|Last Activated!STRING:0|Version!STRING:0|KeyRing!HEX:16|Product!STRING:0";

    /// A real-shaped `.build.info` row: 15 columns, Version 13th, Product 15th.
    pub(crate) fn row(version: &str, product: &str) -> String {
        format!("eu|1|aa|bb|||tpr/wow|eu.cdn.blizzard.com||Windows x86_64 EU? enUS speech?|||{version}||{product}")
    }

    pub(crate) fn build_info(rows: &[(&str, &str)]) -> String {
        let mut s = String::from(HEADER);
        for (v, p) in rows {
            s.push('\n');
            s.push_str(&row(v, p));
        }
        s.push('\n');
        s
    }

    #[test]
    fn versions_become_interface_numbers() {
        assert_eq!(interface_of("12.1.0.70112"), Some(120100));
        assert_eq!(interface_of("1.15.9.70000"), Some(11509));
        assert_eq!(interface_of("1.60.1.69913"), Some(16001));
        assert_eq!(interface_of("5.5.3"), Some(50503));
        assert_eq!(interface_of("12"), Some(120000));
        assert_eq!(interface_of("x.1"), None);
        assert_eq!(interface_of(""), None);
    }

    #[test]
    fn each_product_gets_its_own_interface() {
        let bi = build_info(&[
            ("12.1.0.70112", "wow"),
            ("1.15.9.70000", "wow_classic_era"),
            ("1.60.1.69913", "wow_classic_beta"),
        ]);
        assert_eq!(product_interface(&bi, "wow"), Some(120100));
        assert_eq!(product_interface(&bi, "wow_classic_era"), Some(11509));
        assert_eq!(product_interface(&bi, "wow_classic_beta"), Some(16001));
        assert_eq!(product_interface(&bi, "wow_classic"), None);
    }

    #[test]
    fn crlf_and_comments_are_tolerated() {
        let bi = build_info(&[("12.1.0.1", "wow")]).replace('\n', "\r\n");
        let bi = format!("{bi}## seqn = 1\r\n");
        assert_eq!(product_interface(&bi, "wow"), Some(120100));
    }

    #[test]
    fn only_a_wow_build_info_counts() {
        assert!(lists_wow(&build_info(&[("12.1.0.1", "wow")])));
        assert!(!lists_wow(&build_info(&[("2.10.0.1", "pro")])));
        assert!(!lists_wow(""));
    }

    #[test]
    fn flavor_info_in_both_shapes() {
        assert_eq!(
            flavor_product("Product Flavor!STRING:0\nwow_classic_beta\n").as_deref(),
            Some("wow_classic_beta")
        );
        assert_eq!(flavor_product("wow\r\n").as_deref(), Some("wow"));
        assert_eq!(flavor_product(""), None);
    }

    #[test]
    fn config_wtf_gives_the_last_interface() {
        let cfg = "SET locale \"frFR\"\nSET lastAddonVersion \"120100\"\nSET gxApi \"D3D12\"\n";
        assert_eq!(last_addon_version(cfg), Some(120100));
        assert_eq!(last_addon_version("SET locale \"frFR\"\n"), None);
    }
}
