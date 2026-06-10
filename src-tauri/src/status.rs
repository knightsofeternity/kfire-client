//! Presence status model, shared by the WS tasks (which apply it) and the tray
//! (which displays it). Pure logic only — no Tauri or I/O — so it is unit-tested.
//!
//! A status is one of `online` | `invisible` | `offline`. There is a global
//! status plus an optional per-server override (`inherit` falls back to global):
//! - online    → presence reported and visible to other members
//! - invisible → presence still reported (your stats accrue) but hidden
//! - offline   → nothing reported (the server's session is not even started)

/// Resolves the effective status for a server from the global status and the
/// server's override. Unknown/`inherit` overrides fall back to the global; an
/// empty global defaults to `online`.
pub fn effective_status(global: &str, server_override: &str) -> String {
    match server_override {
        "online" | "invisible" | "offline" => server_override.to_string(),
        _ => {
            if global.is_empty() {
                "online".to_string()
            } else {
                global.to_string()
            }
        }
    }
}

/// Visual state of the tray icon, aggregated across every linked server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconState {
    Idle,
    InGame,
    Invisible,
    Problem,
    Offline,
}

impl IconState {
    /// RGBA color of the status dot overlaid on the tray icon.
    pub fn color(self) -> [u8; 4] {
        match self {
            IconState::Idle => [107, 114, 128, 255],     // grey
            IconState::InGame => [34, 197, 94, 255],      // green
            IconState::Invisible => [249, 115, 22, 255],  // orange
            IconState::Problem => [239, 68, 68, 255],     // red
            IconState::Offline => [75, 85, 99, 255],      // dim grey
        }
    }
}

/// Aggregates the tray icon state. `servers` is `(effective_status, conn_status)`
/// per linked server, where `conn_status` is the last WS status seen
/// (`connecting` | `connected` | `disconnected` | `logged_out`).
pub fn aggregate_icon_state(in_game: bool, servers: &[(String, String)]) -> IconState {
    if servers.is_empty() {
        return IconState::Idle;
    }
    let reporting: Vec<&(String, String)> = servers
        .iter()
        .filter(|(eff, _)| eff == "online" || eff == "invisible")
        .collect();
    if reporting.is_empty() {
        return IconState::Offline;
    }
    if reporting.iter().any(|(_, conn)| conn == "disconnected") {
        return IconState::Problem;
    }
    if !in_game {
        return IconState::Idle;
    }
    if reporting.iter().any(|(eff, _)| eff == "online") {
        IconState::InGame
    } else {
        IconState::Invisible
    }
}

/// Draws a filled status dot in the bottom-right quadrant of an RGBA icon,
/// returning the composited pixels. The base is returned unchanged if its
/// length does not match `width * height * 4`.
pub fn composite_status_dot(base: &[u8], width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let mut out = base.to_vec();
    if base.len() != (width as usize) * (height as usize) * 4 || width == 0 || height == 0 {
        return out;
    }
    let radius = (width.min(height) / 4).max(1) as i64;
    let cx = (width as i64) - radius - 1;
    let cy = (height as i64) - radius - 1;
    let r2 = radius * radius;
    for y in (cy - radius).max(0)..(cy + radius).min(height as i64) {
        for x in (cx - radius).max(0)..(cx + radius).min(width as i64) {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= r2 {
                let i = ((y as usize) * (width as usize) + (x as usize)) * 4;
                out[i] = color[0];
                out[i + 1] = color[1];
                out[i + 2] = color[2];
                out[i + 3] = color[3];
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_status_resolves_override_and_global() {
        assert_eq!(effective_status("online", "inherit"), "online");
        assert_eq!(effective_status("invisible", "inherit"), "invisible");
        assert_eq!(effective_status("online", "offline"), "offline");
        assert_eq!(effective_status("offline", "online"), "online");
        assert_eq!(effective_status("", "inherit"), "online"); // empty global → online
        assert_eq!(effective_status("online", "garbage"), "online"); // unknown → global
    }

    fn s(eff: &str, conn: &str) -> (String, String) {
        (eff.to_string(), conn.to_string())
    }

    #[test]
    fn aggregate_icon_state_cases() {
        assert_eq!(aggregate_icon_state(false, &[]), IconState::Idle);
        assert_eq!(
            aggregate_icon_state(true, &[s("offline", "logged_out")]),
            IconState::Offline
        );
        assert_eq!(
            aggregate_icon_state(true, &[s("online", "connected")]),
            IconState::InGame
        );
        assert_eq!(
            aggregate_icon_state(true, &[s("invisible", "connected")]),
            IconState::Invisible
        );
        assert_eq!(
            aggregate_icon_state(true, &[s("online", "disconnected")]),
            IconState::Problem
        );
        assert_eq!(
            aggregate_icon_state(false, &[s("online", "connected")]),
            IconState::Idle
        );
        // One online (connected) beats one invisible when in game.
        assert_eq!(
            aggregate_icon_state(true, &[s("invisible", "connected"), s("online", "connected")]),
            IconState::InGame
        );
    }

    #[test]
    fn composite_status_dot_draws_and_preserves() {
        let w = 8;
        let h = 8;
        let base = vec![10u8; (w * h * 4) as usize];
        let out = composite_status_dot(&base, w, h, [1, 2, 3, 255]);
        // Bottom-right region got the dot color.
        let br = (((h - 2) * w + (w - 2)) * 4) as usize;
        assert_eq!(&out[br..br + 4], &[1, 2, 3, 255]);
        // Top-left corner is untouched.
        assert_eq!(&out[0..4], &[10, 10, 10, 10]);
    }

    #[test]
    fn composite_status_dot_rejects_bad_size() {
        let base = vec![0u8; 10];
        assert_eq!(composite_status_dot(&base, 8, 8, [1, 2, 3, 4]), base);
    }
}
