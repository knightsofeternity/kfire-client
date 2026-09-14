//! The file that makes Hearthstone write its detailed logs.
//!
//! Without it there is NOTHING to read. It is written only after the member
//! agrees, having seen the exact path and the exact contents, and removing the
//! tracking removes it again.

use std::path::PathBuf;

/// The section Hearthstone reads to decide whether to log its power events.
pub const POWER_BLOCK: &str = "\n[Power]\nLogLevel=1\nFilePrinting=true\nConsolePrinting=false\nScreenPrinting=false\nVerbose=true\n";

/// Where the game looks for that file, per platform.
///
/// It is NOT in the install directory: the game reads it from the user's own
/// configuration folder, while it WRITES its logs next to the install. The two
/// are unrelated paths and confusing them yields a silent no-op.
#[cfg_attr(
    not(any(target_os = "windows", target_os = "macos")),
    allow(clippy::unnecessary_wraps)
)]
pub fn log_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let local = std::env::var_os("LOCALAPPDATA")?;
        Some(
            PathBuf::from(local)
                .join("Blizzard")
                .join("Hearthstone")
                .join("log.config"),
        )
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Library/Preferences/Blizzard/Hearthstone/log.config"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // Hearthstone has no native Linux client, so there is nothing to find.
        None
    }
}

/// Whether the file already asks for power logging, ours or another tracker's.
pub fn contains_power_block(contents: &str) -> bool {
    contents.lines().any(|l| l.trim() == "[Power]")
}

/// The contents with our block appended, or unchanged when a block is present.
///
/// Appending a SECOND [Power] section would not be merged by the game, so an
/// existing one, most likely another tracker's, is left strictly alone.
pub fn with_power_block(contents: &str) -> String {
    if contains_power_block(contents) {
        return contents.to_string();
    }
    format!("{contents}{POWER_BLOCK}")
}

/// The contents without the [Power] section.
///
/// Only that section is dropped; anything a member or another tool put there
/// survives untouched.
pub fn without_power_block(contents: &str) -> String {
    let mut out = String::new();
    let mut in_power = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_power = trimmed == "[Power]";
        }
        if !in_power {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_written_block_asks_for_power_logging_to_a_file() {
        assert!(POWER_BLOCK.contains("[Power]"));
        assert!(POWER_BLOCK.contains("FilePrinting=true"));
        assert!(POWER_BLOCK.contains("Verbose=true"));
        // Printing to the screen would deface the game.
        assert!(POWER_BLOCK.contains("ScreenPrinting=false"));
    }

    #[test]
    fn an_absent_file_is_not_enabled() {
        assert!(!contains_power_block(""));
    }

    #[test]
    fn our_own_block_is_detected() {
        assert!(contains_power_block(POWER_BLOCK));
    }

    #[test]
    fn another_trackers_block_is_detected_too() {
        // A member may already run another tracker. We must not write a second
        // [Power] section, which the game would not merge.
        let existing = "[Zone]\nLogLevel=1\n\n[Power]\nLogLevel=1\nFilePrinting=true\n";
        assert!(contains_power_block(existing));
    }

    #[test]
    fn a_section_merely_mentioning_power_is_not_a_power_section() {
        assert!(!contains_power_block("[PowerTaskList]\nLogLevel=1\n"));
    }

    #[test]
    fn appending_keeps_what_was_already_there() {
        let merged = with_power_block("[Zone]\nLogLevel=1\n");
        assert!(merged.starts_with("[Zone]"));
        assert!(merged.contains("LogLevel=1"));
        assert!(merged.contains("[Power]"));
    }

    #[test]
    fn appending_twice_changes_nothing() {
        let once = with_power_block("");
        assert_eq!(with_power_block(&once), once);
    }

    #[test]
    fn another_trackers_block_is_left_untouched() {
        let existing = "[Power]\nLogLevel=2\nFilePrinting=true\n";
        assert_eq!(with_power_block(existing), existing);
    }

    #[test]
    fn removing_leaves_other_sections_alone() {
        let merged = with_power_block("[Zone]\nLogLevel=1\n");
        let stripped = without_power_block(&merged);
        assert!(stripped.contains("[Zone]"));
        assert!(stripped.contains("LogLevel=1"));
        assert!(!stripped.contains("[Power]"));
        assert!(!stripped.contains("FilePrinting"));
    }

    #[test]
    fn removing_a_middle_section_keeps_the_one_after_it() {
        let src = "[Zone]\nA=1\n[Power]\nB=2\n[Net]\nC=3\n";
        let stripped = without_power_block(src);
        assert!(stripped.contains("[Zone]"));
        assert!(stripped.contains("[Net]"));
        assert!(stripped.contains("C=3"));
        assert!(!stripped.contains("B=2"));
    }

    #[test]
    fn removing_from_a_file_without_the_section_changes_nothing_meaningful() {
        let src = "[Zone]\nA=1\n";
        assert!(without_power_block(src).contains("[Zone]"));
        assert!(without_power_block(src).contains("A=1"));
    }
}
