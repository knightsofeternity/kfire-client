//! The KFire addon, embedded in the client at compile time so that client and
//! addon always ship together.

pub const LUA: &str = include_str!("../../../wow-addon/KFire/KFire.lua");
pub const TOC: &str = include_str!("../../../wow-addon/KFire/KFire.toc");

/// Folder name under `Interface/AddOns`.
pub const FOLDER: &str = "KFire";

/// Proves a `KFire` folder is ours. A folder of that name without it belongs to
/// someone else and is never touched.
pub const MARKER: &str = ".kfire";
pub const MARKER_TEXT: &str = "Installed and kept up to date by the KFIRE client.\n\
Turn the addon off in KFIRE, or delete this folder, to remove it.\n";

/// The TOC with its `## Interface:` line set to exactly this client's number,
/// so the game never flags the addon as out of date. Without a number the
/// bundled multi-version line is kept.
pub fn toc_for(interface: Option<u32>) -> String {
    let Some(i) = interface else {
        return TOC.to_string();
    };
    let mut out: String = TOC
        .lines()
        .map(|l| {
            if l.starts_with("## Interface:") {
                format!("## Interface: {i}")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_addon_is_the_real_one() {
        assert!(LUA.contains("KFirePlayed"));
        assert!(TOC.contains("## SavedVariables: KFirePlayed"));
    }

    #[test]
    fn the_interface_line_is_replaced_by_the_exact_number() {
        let toc = toc_for(Some(120100));
        assert!(toc.contains("## Interface: 120100\n"));
        assert!(!toc.contains("11509"));
        assert!(toc.contains("KFire.lua"));
    }

    #[test]
    fn without_a_number_the_bundled_toc_is_kept() {
        assert_eq!(toc_for(None), TOC);
    }
}
