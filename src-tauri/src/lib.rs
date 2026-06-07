pub mod api;
pub mod db;
pub mod scanner;
pub mod ws;

use std::sync::{Arc, Mutex};

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};
use tokio::sync::{mpsc, watch, Notify};

use crate::api::ApiClient;
use crate::db::Db;
use crate::scanner::ScannerState;
use crate::ws::{Notification, WsTask};

/// Shared application state behind Tauri's state manager.
pub struct AppState {
    pub db: Arc<Db>,
    pub scanner: Arc<ScannerState>,
    /// Wakes the WS task when the scanner queued an event.
    pub queue_notify: Arc<Notify>,
    /// Last access token (for best-effort server-side logout).
    pub access_token: Arc<Mutex<Option<String>>>,
    /// Stops the running WS task when flipped to true.
    session_stop: Mutex<Option<watch::Sender<bool>>>,
    notifications: mpsc::UnboundedSender<Notification>,
}

impl AppState {
    /// Starts (or restarts) the WebSocket session task.
    fn start_session(&self) {
        self.stop_session();
        let (stop_tx, stop_rx) = watch::channel(false);
        *self.session_stop.lock().unwrap() = Some(stop_tx);

        let task = WsTask {
            db: self.db.clone(),
            scanner: self.scanner.clone(),
            queue_notify: self.queue_notify.clone(),
            notifications: self.notifications.clone(),
            access_token: self.access_token.clone(),
            shutdown: stop_rx,
        };
        tauri::async_runtime::spawn(task.run());
    }

    fn stop_session(&self) {
        if let Some(stop) = self.session_stop.lock().unwrap().take() {
            let _ = stop.send(true);
        }
    }
}

// --- Tauri commands ---------------------------------------------------------

#[derive(serde::Serialize)]
pub struct UiState {
    server_url: Option<String>,
    username: Option<String>,
    logged_in: bool,
    games_count: i64,
    running: Vec<RunningGame>,
}

#[derive(serde::Serialize)]
pub struct RunningGame {
    slug: String,
    name: String,
}

#[tauri::command]
async fn login(
    state: tauri::State<'_, AppState>,
    server_url: String,
    username: String,
    password: String,
) -> Result<String, String> {
    let api = ApiClient::new(&server_url);
    let device_id = state.db.device_id();

    let tokens = api
        .login(&username, &password, &device_id)
        .await
        .map_err(|e| e.to_string())?;

    state.db.set_setting("server_url", &api.base_url);
    state.db.set_setting("username", &username);
    state.db.set_setting("refresh_token", &tokens.refresh_token);
    *state.access_token.lock().unwrap() = Some(tokens.access_token.clone());

    // Initial catalog download with the fresh access token; the WS task
    // keeps it up to date afterwards.
    match api.fetch_games(&tokens.access_token).await {
        Ok(games) => {
            if state.db.replace_games(&games).is_ok() {
                state.scanner.load_catalog(&games);
                state
                    .db
                    .set_setting("games_synced_at", &chrono::Utc::now().to_rfc3339());
            }
        }
        Err(e) => log::warn!("login: games sync failed (will retry on connect): {e}"),
    }

    state.start_session();
    Ok(username)
}

#[tauri::command]
async fn logout(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.stop_session();

    // Best-effort server-side revocation of this device's refresh token.
    let access = state.access_token.lock().unwrap().take();
    if let (Some(access), Some(server_url)) = (access, state.db.get_setting("server_url")) {
        let api = ApiClient::new(&server_url);
        if let Err(e) = api.logout(&access).await {
            log::warn!("logout: server revocation failed: {e}");
        }
    }

    state.db.delete_setting("refresh_token");
    state.db.delete_setting("username");
    Ok(())
}

#[tauri::command]
fn get_state(state: tauri::State<'_, AppState>) -> UiState {
    let names = state.scanner.names.read().unwrap();
    let running = state
        .scanner
        .running_slugs()
        .into_iter()
        .map(|slug| RunningGame {
            name: names.get(&slug).cloned().unwrap_or_else(|| slug.clone()),
            slug,
        })
        .collect();

    UiState {
        server_url: state.db.get_setting("server_url"),
        username: state.db.get_setting("username"),
        logged_in: state.db.get_setting("refresh_token").is_some(),
        games_count: state.db.games_count(),
        running,
    }
}

// --- App setup ----------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![login, logout, get_state])
        .setup(|app| {
            // --- local cache -------------------------------------------------
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = Arc::new(Db::open(&data_dir.join("kfire.db"))?);

            // --- scanner ------------------------------------------------------
            let scanner_state = Arc::new(ScannerState::default());
            scanner_state.load_catalog(&db.load_games());
            let (event_tx, mut event_rx) = mpsc::unbounded_channel();
            scanner::spawn(scanner_state.clone(), event_tx);

            // --- notifications → webview events --------------------------------
            let (notif_tx, mut notif_rx) = mpsc::unbounded_channel::<Notification>();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(n) = notif_rx.recv().await {
                    let event = match &n {
                        Notification::Status { .. } => "kfire://status",
                        Notification::Presence { .. } => "kfire://presence",
                    };
                    let _ = handle.emit(event, &n);
                }
            });

            let state = AppState {
                db: db.clone(),
                scanner: scanner_state,
                queue_notify: Arc::new(Notify::new()),
                access_token: Arc::new(Mutex::new(None)),
                session_stop: Mutex::new(None),
                notifications: notif_tx,
            };

            // --- scanner events → SQLite queue → WS task -----------------------
            let queue_db = db.clone();
            let queue_notify = state.queue_notify.clone();
            let running_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(ev) = event_rx.recv().await {
                    let event_type = if ev.started { "game_started" } else { "game_stopped" };
                    queue_db.queue_event(event_type, &ev.game_slug, &ev.ts.to_rfc3339());
                    queue_notify.notify_one();
                    let _ = running_handle.emit("kfire://detection", event_type);
                }
            });

            // Auto-resume the session when we have a refresh token.
            if db.get_setting("refresh_token").is_some() {
                state.start_session();
            }
            app.manage(state);

            // --- system tray ----------------------------------------------------
            let show = MenuItem::with_id(app, "show", "Show KFIRE", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            TrayIconBuilder::with_id("kfire-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("KFIRE — gaming presence")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        // Closing the window hides it to the tray instead of quitting.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
