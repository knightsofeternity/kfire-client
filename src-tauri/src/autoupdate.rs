//! Real self-update, through Tauri's updater plugin.
//!
//! Until 0.9.1 the client only *told* the member a new version existed (see
//! `update.rs`), and every member had to fetch the installer by hand. Now the
//! client fetches `latest.json` from the latest GitHub release, checks the
//! signature of the new build against the public key in `tauri.conf.json`,
//! installs it and restarts.
//!
//! Cost stays negligible: one small GET a minute after start, then every 6 h
//! (KFIRE lives in the tray for days, so a start-only check is not enough).
//! An update never interrupts a game: while one is running it waits and tries
//! again a little later.
//!
//! Linux .deb/.rpm installs cannot replace themselves (only the AppImage can),
//! so outside an AppImage nothing runs and the footer link to the release page
//! stays the way to update.

use std::sync::Arc;
use std::time::Duration;

use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::scanner::ScannerState;

/// Let the app settle (servers, scanner) before the first check.
const FIRST_CHECK: Duration = Duration::from_secs(60);
/// Regular check while KFIRE stays open.
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Retry delay when a game was running.
const RETRY: Duration = Duration::from_secs(15 * 60);
/// Retry delay after an error (offline, GitHub down, bad signature).
const AFTER_ERROR: Duration = Duration::from_secs(60 * 60);

enum Outcome {
    UpToDate,
    /// An update exists but a game is running: try again soon.
    Deferred,
}

pub fn spawn(app: AppHandle, scanner: Arc<ScannerState>) {
    if cfg!(target_os = "linux") && std::env::var_os("APPIMAGE").is_none() {
        log::info!("auto-update: off (Linux package, not an AppImage)");
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            let wait = match try_update(&app, &scanner).await {
                Ok(Outcome::UpToDate) => EVERY,
                Ok(Outcome::Deferred) => RETRY,
                Err(e) => {
                    log::warn!("auto-update: {e}");
                    AFTER_ERROR
                }
            };
            tokio::time::sleep(wait).await;
        }
    });
}

async fn try_update(
    app: &AppHandle,
    scanner: &ScannerState,
) -> tauri_plugin_updater::Result<Outcome> {
    let Some(update) = app.updater()?.check().await? else {
        return Ok(Outcome::UpToDate);
    };
    if !scanner.running_games().is_empty() {
        log::info!(
            "auto-update: {} found, waiting for the game to close",
            update.version
        );
        return Ok(Outcome::Deferred);
    }
    log::info!(
        "auto-update: installing {} (from {})",
        update.version,
        update.current_version
    );
    update.download_and_install(|_, _| {}, || {}).await?;
    log::info!("auto-update: {} installed, restarting", update.version);
    app.restart();
}
