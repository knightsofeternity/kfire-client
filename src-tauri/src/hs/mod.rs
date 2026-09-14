//! Hearthstone match tracking: the game has no player API, so the desktop
//! client reads the game's own logs and reports a summary.
//!
//! What NEVER leaves this machine, by design: the opponent's name and every
//! card played. Both are in the log. The summary carries facts about the member
//! only.

pub mod config;
pub mod parser;
pub mod paths;
