//! The file that makes Rocket League open its stats socket.
//!
//! Without it there is NOTHING to read. It is only written once the member has
//! agreed, having seen the exact path and the exact contents, and taking the
//! tracking away takes it away too.
//!
//! CAREFUL: the `ke-rl-tracker` README describes a `[StatsAPI]` section with
//! `bEnabled` and `ListenPort`. That is WRONG. The code actually running at its
//! author's writes what follows, and that is what counts.

/// The default port of the stats socket.
pub const DEFAULT_PORT: u16 = 49123;

/// The section the game reads to decide whether to export its stats.
///
/// PacketSendRate accepts 30, 60, 90 or 120. We ask for the MINIMUM: we
/// aggregate locally, so thirty states per second are plenty, and we take less
/// machine time from the game than the original tracker, which asked for
/// sixty.
pub const STATS_BLOCK: &str = "\n[TAGame.MatchStatsExporter_TA]\nPort=49123\nPacketSendRate=30\n";

/// The name of the section, lowercased, for a case-insensitive comparison.
const SECTION: &str = "[tagame.matchstatsexporter_ta]";

/// Whether the file already asks for the export, ours or another tracker's.
pub fn has_stats_section(contents: &str) -> bool {
    contents
        .lines()
        .any(|l| l.trim().to_ascii_lowercase() == SECTION)
}

/// The contents with our section added, or unchanged if there is one already.
///
/// Adding a SECOND section would not be merged by the game, so an existing
/// section, most likely another tracker's, is left strictly alone.
pub fn with_stats_block(contents: &str) -> String {
    if has_stats_section(contents) {
        return contents.to_string();
    }
    format!("{contents}{STATS_BLOCK}")
}

/// The contents without the section, provided that section is ours.
///
/// The removal works line by line rather than by exact match, because a file
/// opened in Notepad comes back in CRLF. With an exact match, turning the
/// tracking off would then do NOTHING: the screen would confirm to the member
/// that it is off while the game kept on exporting. Someone who asks to stop
/// being tracked must really stop being tracked.
///
/// We only take back what we wrote: breaking another tracker's configuration
/// by turning ourselves off would be unacceptable. The section is recognised
/// as ours only if its settings are exactly ours.
pub fn without_stats_block(contents: &str) -> String {
    let eol = if contents.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let lines: Vec<&str> = contents
        .split('\n')
        .map(|l| l.trim_end_matches('\r'))
        .collect();

    let Some(start) = lines
        .iter()
        .position(|l| l.trim().to_ascii_lowercase() == SECTION)
    else {
        return contents.to_string();
    };
    // The section runs until the next one, or until the end.
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.trim().starts_with('['))
        .map(|i| start + 1 + i)
        .unwrap_or(lines.len());

    if !is_ours(&lines[start + 1..end]) {
        return contents.to_string();
    }

    let mut kept: Vec<&str> = Vec::with_capacity(lines.len());
    kept.extend_from_slice(&lines[..start]);
    kept.extend_from_slice(&lines[end..]);
    // The blank line our block had added before itself leaves with it.
    while kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    if kept.is_empty() {
        return String::new();
    }
    let mut out = kept.join(eol);
    out.push_str(eol);
    out
}

/// Whether this section's settings are exactly the ones we write.
///
/// That is what tells our block apart from another tracker's, now that the
/// comparison is no longer literal.
fn is_ours(body: &[&str]) -> bool {
    let mut port = false;
    let mut rate = false;
    for l in body {
        let t = l.trim();
        if t.is_empty() {
            continue;
        }
        let Some((k, v)) = t.split_once('=') else {
            return false;
        };
        match (k.trim().to_ascii_lowercase().as_str(), v.trim()) {
            ("port", "49123") => port = true,
            ("packetsendrate", "30") => rate = true,
            _ => return false,
        }
    }
    port && rate
}

/// The port the game will listen on, according to the file.
///
/// If a section is already there, it is ITS port that counts, not ours: that
/// is the one the game will open. A value that cannot be read, or one outside
/// the user ports, falls back to the default.
pub fn port_in(contents: &str) -> u16 {
    for line in contents.lines() {
        let l = line.trim();
        let Some((k, v)) = l.split_once('=') else {
            continue;
        };
        if !k.trim().eq_ignore_ascii_case("port") {
            continue;
        }
        if let Ok(p) = v.trim().parse::<u32>() {
            if (1024..=65535).contains(&p) {
                return p as u16;
            }
        }
    }
    DEFAULT_PORT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_file_gets_the_whole_block() {
        let out = with_stats_block("");
        assert!(out.contains("[TAGame.MatchStatsExporter_TA]"));
        assert!(out.contains("Port=49123"));
        assert!(out.contains("PacketSendRate=30"));
    }

    #[test]
    fn an_existing_section_is_left_strictly_alone() {
        // Most likely another tracker's. We do not touch it, and above all we
        // do not add a SECOND section: the game does not merge them.
        let theirs = "[TAGame.MatchStatsExporter_TA]\nPort=50000\nPacketSendRate=120\n";
        assert_eq!(with_stats_block(theirs), theirs);
    }

    #[test]
    fn other_sections_survive() {
        let other = "[SomethingElse]\nKey=1\n";
        let out = with_stats_block(other);
        assert!(out.starts_with(other));
        assert!(out.contains("[TAGame.MatchStatsExporter_TA]"));
    }

    #[test]
    fn removing_takes_back_only_our_block() {
        let other = "[SomethingElse]\nKey=1\n";
        let with = with_stats_block(other);
        assert_eq!(without_stats_block(&with).trim_end(), other.trim_end());
    }

    #[test]
    fn removing_leaves_someone_elses_block_alone() {
        // We only take back what we wrote. Breaking another tracker's
        // configuration by turning ourselves off would be unacceptable.
        let theirs = "[TAGame.MatchStatsExporter_TA]\nPort=50000\nPacketSendRate=120\n";
        assert_eq!(without_stats_block(theirs), theirs);
    }

    #[test]
    fn the_port_is_read_from_the_file_when_it_is_there() {
        let theirs = "[TAGame.MatchStatsExporter_TA]\nPort=50000\nPacketSendRate=120\n";
        assert_eq!(port_in(theirs), 50000);
    }

    #[test]
    fn the_port_falls_back_to_the_default() {
        assert_eq!(port_in(""), DEFAULT_PORT);
        assert_eq!(
            port_in("[TAGame.MatchStatsExporter_TA]\nPacketSendRate=60\n"),
            DEFAULT_PORT
        );
    }

    #[test]
    fn a_port_with_spaces_and_case_is_still_read() {
        assert_eq!(
            port_in("[TAGame.MatchStatsExporter_TA]\n port = 49999 \n"),
            49999
        );
    }

    #[test]
    fn an_absurd_port_is_refused_and_falls_back() {
        assert_eq!(
            port_in("[TAGame.MatchStatsExporter_TA]\nPort=0\n"),
            DEFAULT_PORT
        );
        assert_eq!(
            port_in("[TAGame.MatchStatsExporter_TA]\nPort=99999999\n"),
            DEFAULT_PORT
        );
    }

    #[test]
    fn detecting_our_own_block_is_exact() {
        assert!(!has_stats_section(""));
        assert!(has_stats_section("[TAGame.MatchStatsExporter_TA]\n"));
        assert!(has_stats_section("x\n[tagame.matchstatsexporter_ta]\ny\n"));
    }

    #[test]
    fn removing_works_on_a_file_that_came_back_from_notepad() {
        // Notepad rewrites the whole file in CRLF. With an exact match, the
        // removal did nothing at all.
        let unix = with_stats_block("[SomethingElse]\nKey=1\n");
        let crlf = unix.replace('\n', "\r\n");
        let out = without_stats_block(&crlf);
        assert!(!has_stats_section(&out), "our block had to go: {out:?}");
        assert!(out.contains("Key=1"), "the rest of the file had to stay");
    }

    #[test]
    fn removing_a_crlf_file_keeps_it_in_crlf() {
        let crlf = with_stats_block("[SomethingElse]\nKey=1\n").replace('\n', "\r\n");
        let out = without_stats_block(&crlf);
        assert!(
            out.contains("\r\n"),
            "the member's line endings are his own"
        );
        assert!(!out.contains("\n\n"), "no stray blank line");
    }

    #[test]
    fn removing_still_leaves_someone_elses_block_alone_in_crlf_too() {
        let theirs = "[TAGame.MatchStatsExporter_TA]\r\nPort=50000\r\nPacketSendRate=120\r\n";
        assert_eq!(without_stats_block(theirs), theirs);
    }

    #[test]
    fn a_section_with_an_extra_key_is_not_ours() {
        // Someone added a setting to ours: it is not ours any more, we do not
        // touch it.
        let mixed =
            "[TAGame.MatchStatsExporter_TA]\nPort=49123\nPacketSendRate=30\nSomethingElse=1\n";
        assert_eq!(without_stats_block(mixed), mixed);
    }

    #[test]
    fn removing_our_block_from_a_file_that_holds_nothing_else_empties_it() {
        let only_ours = with_stats_block("");
        assert_eq!(without_stats_block(&only_ours), "");
    }
}
