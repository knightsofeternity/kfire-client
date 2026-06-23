//! Lightweight "is my client up to date?" check.
//!
//! Design constraints (the whole point of this module): it must NOT cost CPU,
//! RAM or network. So:
//!   - no background task, no timer, no polling;
//!   - it only runs when the UI window asks for it (on mount);
//!   - results are cached in-process with a TTL, so repeatedly opening the
//!     window does not re-hit GitHub;
//!   - a single small GET with a short timeout, and a silent fallback offline.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const RELEASES_URL: &str = "https://github.com/knightsofeternity/kfire-client/releases";
const LATEST_API: &str =
    "https://api.github.com/repos/knightsofeternity/kfire-client/releases/latest";

/// How long a successful check stays fresh (no re-fetch within this window).
const TTL_OK: Duration = Duration::from_secs(6 * 60 * 60); // 6h
/// A failed check (offline, rate-limited) is retried sooner, but not spammed.
const TTL_ERR: Duration = Duration::from_secs(30 * 60); // 30min
/// Network timeout for the single request.
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

/// Sent to the UI. `latest`/`update_available` are best-effort: when the check
/// could not run (offline), `latest` is `None` and we simply show the version.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub update_available: bool,
    pub releases_url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
}

struct Cached {
    at: Instant,
    ok: bool,
    info: UpdateInfo,
}

fn cache() -> &'static Mutex<Option<Cached>> {
    static CACHE: OnceLock<Mutex<Option<Cached>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Parse a tag like `v0.3.2`, `0.3.2` or `v0.4.0-beta.1` into (major, minor, patch).
/// Pre-release / build suffixes are dropped (we only compare the release line).
fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim();
    let s = s.strip_prefix('v').or_else(|| s.strip_prefix('V')).unwrap_or(s);
    // drop any pre-release ("-beta") or build ("+meta") suffix
    let core = s.split(['-', '+']).next().unwrap_or(s);
    let mut it = core.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next().unwrap_or("0").parse().ok()?;
    let patch = it.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

/// True when `latest` is a strictly newer release than `current`.
/// Unparseable input is treated conservatively as "not newer".
fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_semver(latest), parse_semver(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Returns the cached value if still fresh.
fn fresh_cached() -> Option<UpdateInfo> {
    let guard = cache().lock().ok()?;
    let c = guard.as_ref()?;
    let ttl = if c.ok { TTL_OK } else { TTL_ERR };
    if c.at.elapsed() < ttl {
        Some(c.info.clone())
    } else {
        None
    }
}

fn store(ok: bool, info: &UpdateInfo) {
    if let Ok(mut guard) = cache().lock() {
        *guard = Some(Cached {
            at: Instant::now(),
            ok,
            info: info.clone(),
        });
    }
}

/// Fetch the latest release tag from GitHub. Best-effort; returns `None` on any
/// error (offline, timeout, rate-limited, malformed) so the UI degrades quietly.
async fn fetch_latest_tag() -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("kfire-client/", env!("CARGO_PKG_VERSION")))
        .timeout(HTTP_TIMEOUT)
        .build()
        .ok()?;
    let rel = client
        .get(LATEST_API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<GithubRelease>()
        .await
        .ok()?;
    Some(rel.tag_name)
}

/// Run (or reuse a cached) update check. Never errors: offline simply yields a
/// result with `latest = None`.
pub async fn check() -> UpdateInfo {
    if let Some(info) = fresh_cached() {
        return info;
    }

    let current = current_version();
    match fetch_latest_tag().await {
        Some(tag) => {
            let info = UpdateInfo {
                update_available: is_newer(&tag, &current),
                current,
                latest: Some(tag),
                releases_url: RELEASES_URL.to_string(),
            };
            store(true, &info);
            info
        }
        None => {
            let info = UpdateInfo {
                current,
                latest: None,
                update_available: false,
                releases_url: RELEASES_URL.to_string(),
            };
            store(false, &info);
            info
        }
    }
}

#[tauri::command]
pub async fn check_for_update() -> UpdateInfo {
    check().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_prefixed() {
        assert_eq!(parse_semver("0.3.2"), Some((0, 3, 2)));
        assert_eq!(parse_semver("v0.3.2"), Some((0, 3, 2)));
        assert_eq!(parse_semver(" v1.0.0 "), Some((1, 0, 0)));
    }

    #[test]
    fn parses_short_and_prerelease() {
        assert_eq!(parse_semver("v1.2"), Some((1, 2, 0)));
        assert_eq!(parse_semver("v1"), Some((1, 0, 0)));
        assert_eq!(parse_semver("v0.4.0-beta.1"), Some((0, 4, 0)));
        assert_eq!(parse_semver("0.4.0+build7"), Some((0, 4, 0)));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_semver("nightly"), None);
        assert_eq!(parse_semver(""), None);
    }

    #[test]
    fn newer_detects_each_component() {
        assert!(is_newer("v0.3.3", "0.3.2"));
        assert!(is_newer("v0.4.0", "0.3.2"));
        assert!(is_newer("v1.0.0", "0.3.2"));
    }

    #[test]
    fn not_newer_when_equal_or_older() {
        assert!(!is_newer("v0.3.2", "0.3.2"));
        assert!(!is_newer("v0.3.1", "0.3.2"));
        assert!(!is_newer("v0.2.9", "0.3.2"));
    }

    #[test]
    fn garbage_is_not_newer() {
        assert!(!is_newer("nightly", "0.3.2"));
        assert!(!is_newer("v0.4.0", "garbage"));
    }
}
