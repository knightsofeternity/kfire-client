pub mod api;
pub mod db;
pub mod scanner;
pub mod ws;

use std::collections::HashMap;
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
    /// Wakes the WS tasks when the scanner queued an event.
    pub queue_notify: Arc<Notify>,
    /// Latest access token per server (for best-effort server-side logout).
    pub access_tokens: Arc<Mutex<HashMap<String, String>>>,
    /// Stop senders, one per running server session (keyed by server_id).
    sessions: Mutex<HashMap<String, watch::Sender<bool>>>,
    notifications: mpsc::UnboundedSender<Notification>,
}

impl AppState {
    /// Starts (or restarts) the WebSocket session task for one server.
    fn start_session(&self, server_id: &str) {
        self.stop_session(server_id);
        let (stop_tx, stop_rx) = watch::channel(false);
        self.sessions
            .lock()
            .unwrap()
            .insert(server_id.to_string(), stop_tx);

        let task = WsTask {
            server_id: server_id.to_string(),
            db: self.db.clone(),
            scanner: self.scanner.clone(),
            queue_notify: self.queue_notify.clone(),
            notifications: self.notifications.clone(),
            access_tokens: self.access_tokens.clone(),
            shutdown: stop_rx,
        };
        tauri::async_runtime::spawn(task.run());
    }

    /// Starts a session for every linked server.
    fn start_all(&self) {
        for s in self.db.list_servers() {
            self.start_session(&s.id);
        }
    }

    fn stop_session(&self, server_id: &str) {
        if let Some(stop) = self.sessions.lock().unwrap().remove(server_id) {
            let _ = stop.send(true);
        }
    }
}

// --- Tauri commands ---------------------------------------------------------

#[derive(serde::Serialize)]
pub struct UiServer {
    id: String,
    url: String,
    org_name: String,
    status_override: String,
}

#[derive(serde::Serialize)]
pub struct UiState {
    servers: Vec<UiServer>,
    logged_in: bool,
    games_count: i64,
    running: Vec<RunningGame>,
}

#[derive(serde::Serialize)]
pub struct RunningGame {
    slug: String,
    name: String,
}

#[derive(serde::Serialize)]
pub struct LinkInfo {
    user_code: String,
    verification_url: String,
}

/// Starts browser-based device linking for an additional server: registers a
/// pairing, opens the browser to approve, and polls in the background until
/// tokens arrive (then links the server and starts its session).
#[tauri::command]
async fn start_link(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    server_url: String,
) -> Result<LinkInfo, String> {
    let api = ApiClient::new(&server_url);

    // Refuse linking the same server twice (additive pairing is per-server).
    if state.db.find_server_by_url(&api.base_url).is_some() {
        return Err("this server is already linked".into());
    }

    let device_id = state.db.device_id();
    let start = api
        .start_pairing(&device_id)
        .await
        .map_err(|e| e.to_string())?;

    // Open the browser so the user can approve on the web app.
    use tauri_plugin_opener::OpenerExt;
    if let Err(e) = app
        .opener()
        .open_url(start.verification_url.clone(), None::<&str>)
    {
        log::warn!("could not open browser: {e}");
    }

    let app2 = app.clone();
    let base = api.base_url.clone();
    let device_code = start.device_code.clone();
    let interval = start.interval.max(1);
    tauri::async_runtime::spawn(poll_until_linked(app2, base, device_code, interval));

    Ok(LinkInfo {
        user_code: start.user_code,
        verification_url: start.verification_url,
    })
}

/// Polls the pairing until the user approves it (or it fails), then links the
/// server and starts its presence session.
async fn poll_until_linked(
    app: tauri::AppHandle,
    base: String,
    device_code: String,
    interval: u64,
) {
    use tauri::Manager;

    // Clone the shared handles so we never hold a State borrow across awaits.
    let (db, scanner, access_tokens, notifications) = {
        let s = app.state::<AppState>();
        (
            s.db.clone(),
            s.scanner.clone(),
            s.access_tokens.clone(),
            s.notifications.clone(),
        )
    };
    let api = ApiClient::new(&base);

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        let poll = match api.poll_pairing(&device_code).await {
            Ok(p) => p,
            Err(e) => {
                log::warn!("pairing poll error: {e}");
                continue;
            }
        };
        match poll.status.as_str() {
            "pending" => continue,
            "complete" => {
                if let (Some(access), Some(refresh)) = (poll.access_token, poll.refresh_token) {
                    // Label the server with its org name (best-effort).
                    let org = api
                        .fetch_config()
                        .await
                        .map(|c| c.org_name)
                        .unwrap_or_default();
                    let server_id = db.add_server(&api.base_url, &refresh, &org);
                    access_tokens
                        .lock()
                        .unwrap()
                        .insert(server_id.clone(), access.clone());

                    if let Ok(games) = api.fetch_games(&access).await {
                        if db.replace_games(&server_id, &games).is_ok() {
                            db.set_setting(
                                &format!("games_synced_at:{server_id}"),
                                &chrono::Utc::now().to_rfc3339(),
                            );
                            scanner.load_catalog(&db.load_games());
                        }
                    }
                    app.state::<AppState>().start_session(&server_id);

                    // First successful link: default to launch-at-login (set once).
                    if db.get_setting("autostart_configured").is_none() {
                        use tauri_plugin_autostart::ManagerExt;
                        if app.autolaunch().enable().is_ok() {
                            db.set_setting("autostart_configured", "1");
                        }
                    }
                }
                return;
            }
            _ => {
                let _ = notifications.send(Notification::Status {
                    server_id: String::new(),
                    status: "logged_out".into(),
                    detail: "linking was denied or expired - try again".into(),
                });
                return;
            }
        }
    }
}

/// Unlinks one server: stops its session, best-effort server-side token
/// revocation, and drops its local data.
#[tauri::command]
async fn unlink_server(state: tauri::State<'_, AppState>, server_id: String) -> Result<(), String> {
    state.stop_session(&server_id);

    let access = state.access_tokens.lock().unwrap().remove(&server_id);
    if let (Some(access), Some(server)) = (access, state.db.get_server(&server_id)) {
        let api = ApiClient::new(&server.url);
        if let Err(e) = api.logout(&access).await {
            log::warn!("unlink: server revocation failed: {e}");
        }
    }

    state.db.remove_server(&server_id);
    Ok(())
}

/// Whether the app is registered to launch at login.
#[tauri::command]
fn get_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

/// Register/unregister the app to launch at login. The choice is remembered so
/// the first-link default never overrides what the user picked here.
#[tauri::command]
fn set_autostart(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let res = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    res.map_err(|e| e.to_string())?;
    state.db.set_setting("autostart_configured", "1");
    Ok(())
}

#[tauri::command]
fn get_state(state: tauri::State<'_, AppState>) -> UiState {
    let servers = state
        .db
        .list_servers()
        .into_iter()
        .map(|s| UiServer {
            id: s.id,
            url: s.url,
            org_name: s.org_name,
            status_override: s.status_override,
        })
        .collect::<Vec<_>>();

    let running = state
        .scanner
        .running_games()
        .into_iter()
        .map(|(slug, name)| RunningGame { slug, name })
        .collect();

    UiState {
        logged_in: !servers.is_empty(),
        games_count: state.db.games_count(),
        servers,
        running,
    }
}

// --- App setup ----------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: None,
                    }),
                ])
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            start_link,
            unlink_server,
            get_state,
            get_autostart,
            set_autostart
        ])
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
                access_tokens: Arc::new(Mutex::new(HashMap::new())),
                sessions: Mutex::new(HashMap::new()),
                notifications: notif_tx,
            };

            // --- scanner events → SQLite queue → WS tasks ----------------------
            let queue_db = db.clone();
            let queue_notify = state.queue_notify.clone();
            let running_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(ev) = event_rx.recv().await {
                    let event_type = if ev.started { "game_started" } else { "game_stopped" };
                    queue_db.queue_event(&ev.server_id, event_type, &ev.game_slug, &ev.ts.to_rfc3339());
                    queue_notify.notify_one();
                    let _ = running_handle.emit("kfire://detection", event_type);
                }
            });

            // Auto-resume every linked server's session. A presence app runs in
            // the background, so default to launch-at-login the first time we're
            // linked - but only set it once, then the user's toggle
            // (autostart_configured) is respected forever after.
            if !db.list_servers().is_empty() {
                state.start_all();
                if db.get_setting("autostart_configured").is_none() {
                    use tauri_plugin_autostart::ManagerExt;
                    if app.autolaunch().enable().is_ok() {
                        db.set_setting("autostart_configured", "1");
                    }
                }
            }
            app.manage(state);

            // --- system tray ----------------------------------------------------
            let show = MenuItem::with_id(app, "show", "Show KFIRE", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            TrayIconBuilder::with_id("kfire-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("KFIRE - gaming presence")
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
