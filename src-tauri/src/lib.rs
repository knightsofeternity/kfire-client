pub mod api;
pub mod db;
pub mod scanner;
pub mod status;
pub mod ws;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    Emitter, Manager,
};
use tokio::sync::{mpsc, watch, Notify};

use crate::api::ApiClient;
use crate::db::Db;
use crate::scanner::ScannerState;
use crate::status::{aggregate_icon_state, composite_status_dot, effective_status, IconState};
use crate::ws::{Notification, WsTask};

/// Shared application state behind Tauri's state manager.
pub struct AppState {
    pub db: Arc<Db>,
    pub scanner: Arc<ScannerState>,
    /// Wakes the WS tasks when the scanner queued an event.
    pub queue_notify: Arc<Notify>,
    /// Latest access token per server (for best-effort server-side logout).
    pub access_tokens: Arc<Mutex<HashMap<String, String>>>,
    /// Last WS connection status seen per server (for the tray icon).
    pub conn_status: Arc<Mutex<HashMap<String, String>>>,
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

    /// Starts a session for every linked server (honoring offline status).
    fn start_all(&self) {
        for s in self.db.list_servers() {
            self.apply_server_status(&s.id);
        }
    }

    fn stop_session(&self, server_id: &str) {
        if let Some(stop) = self.sessions.lock().unwrap().remove(server_id) {
            let _ = stop.send(true);
        }
        self.conn_status.lock().unwrap().remove(server_id);
    }

    /// Effective status for a server (its override, else the global status).
    fn effective_status(&self, server_id: &str) -> String {
        let global = self.db.get_setting("global_status").unwrap_or_default();
        let over = self
            .db
            .get_server(server_id)
            .map(|s| s.status_override)
            .unwrap_or_default();
        crate::status::effective_status(&global, &over)
    }

    /// Pushes a server's effective presence status to the server over REST,
    /// using its stored access token. Lets a status change take effect
    /// immediately even when the WS is already open, and persists `offline`
    /// server-side before the socket closes.
    fn push_presence_status(&self, server_id: &str) {
        let status = self.effective_status(server_id);
        let server = match self.db.get_server(server_id) {
            Some(s) => s,
            None => return,
        };
        let token = match self.access_tokens.lock().unwrap().get(server_id).cloned() {
            Some(t) => t,
            None => return,
        };
        let server_id = server_id.to_string();
        tauri::async_runtime::spawn(async move {
            let api = crate::api::ApiClient::new(&server.url);
            if let Err(e) = api.set_presence_status(&token, &status).await {
                log::warn!("push presence_status[{server_id}] failed: {e}");
            }
        });
    }

    /// (Re)starts or stops a server's session to match its effective status.
    /// Offline → stop; online/invisible → (re)start so the WS task re-applies
    /// the presence status on connect. Pushes the new status over REST first so
    /// the change takes effect immediately (and offline persists before close).
    fn apply_server_status(&self, server_id: &str) {
        self.push_presence_status(server_id);
        if self.effective_status(server_id) == "offline" {
            self.stop_session(server_id);
        } else {
            self.start_session(server_id);
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
    global_status: String,
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
                            scanner.reload_ignored(&db.list_ignored());
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
        global_status: state
            .db
            .get_setting("global_status")
            .unwrap_or_else(|| "online".into()),
        logged_in: !servers.is_empty(),
        games_count: state.db.games_count(),
        servers,
        running,
    }
}

/// Sets the global status and re-applies it to every server that inherits it.
#[tauri::command]
fn set_global_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    status: String,
) -> Result<(), String> {
    if !matches!(status.as_str(), "online" | "invisible" | "offline") {
        return Err("invalid status".into());
    }
    state.db.set_setting("global_status", &status);
    for s in state.db.list_servers() {
        if s.status_override == "inherit" {
            state.apply_server_status(&s.id);
        }
    }
    rebuild_tray(&app);
    Ok(())
}

/// Sets one server's status override and re-applies it.
#[tauri::command]
fn set_server_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    server_id: String,
    status: String,
) -> Result<(), String> {
    if !matches!(status.as_str(), "inherit" | "online" | "invisible" | "offline") {
        return Err("invalid status".into());
    }
    state.db.set_server_status_override(&server_id, &status);
    state.apply_server_status(&server_id);
    rebuild_tray(&app);
    Ok(())
}

// --- system tray ---------------------------------------------------------------

/// Human label for a server row's current state in the tray menu.
fn server_state_word(effective: &str, conn: Option<&String>) -> &'static str {
    match effective {
        "offline" => "offline",
        "invisible" => "invisible",
        _ => match conn.map(String::as_str) {
            Some("connected") => "online",
            Some("disconnected") => "reconnecting",
            _ => "connecting",
        },
    }
}

/// Rebuilds the tray menu, tooltip and status icon from the current state.
/// Safe to call from any thread that has the `AppHandle`.
fn rebuild_tray(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let global = state
        .db
        .get_setting("global_status")
        .unwrap_or_else(|| "online".into());
    let servers = state.db.list_servers();
    let conn = state.conn_status.lock().unwrap().clone();
    let running = state.scanner.running_games();
    let in_game = !running.is_empty();

    // --- global status submenu ---
    let checked = |v: &str| global == v;
    let g_on = CheckMenuItem::with_id(app, "g:online", "Online", true, checked("online"), None::<&str>);
    let g_inv = CheckMenuItem::with_id(app, "g:invisible", "Invisible", true, checked("invisible"), None::<&str>);
    let g_off = CheckMenuItem::with_id(app, "g:offline", "Offline", true, checked("offline"), None::<&str>);
    let (g_on, g_inv, g_off) = match (g_on, g_inv, g_off) {
        (Ok(a), Ok(b), Ok(c)) => (a, b, c),
        _ => return,
    };
    let Ok(status_sub) = Submenu::with_items(app, "Status", true, &[&g_on, &g_inv, &g_off]) else {
        return;
    };

    // --- per-server submenus ---
    let mut server_subs: Vec<Submenu<tauri::Wry>> = Vec::new();
    for s in &servers {
        let eff = effective_status(&global, &s.status_override);
        let ov = |v: &str| s.status_override == v;
        let mk = |id: &str, label: &str, on: bool| {
            CheckMenuItem::with_id(app, format!("s:{}:{id}", s.id), label, true, on, None::<&str>)
        };
        let (inh, on, inv, off, unlink, sep) = match (
            mk("inherit", "Use global", ov("inherit")),
            mk("online", "Online", ov("online")),
            mk("invisible", "Invisible", ov("invisible")),
            mk("offline", "Offline", ov("offline")),
            MenuItem::with_id(app, format!("u:{}", s.id), "Unlink", true, None::<&str>),
            PredefinedMenuItem::separator(app),
        ) {
            (Ok(a), Ok(b), Ok(c), Ok(d), Ok(e), Ok(f)) => (a, b, c, d, e, f),
            _ => return,
        };
        let name = if s.org_name.is_empty() { &s.url } else { &s.org_name };
        let label = format!("{name} — {}", server_state_word(&eff, conn.get(&s.id)));
        let items: [&dyn IsMenuItem<tauri::Wry>; 6] = [&inh, &on, &inv, &off, &sep, &unlink];
        if let Ok(sub) = Submenu::with_items(app, label, true, &items) {
            server_subs.push(sub);
        }
    }

    // --- assemble ---
    let (add, show, quit, sep1, sep2) = match (
        MenuItem::with_id(app, "add", "Add a server…", true, None::<&str>),
        MenuItem::with_id(app, "show", "Show KFIRE", true, None::<&str>),
        MenuItem::with_id(app, "quit", "Quit", true, None::<&str>),
        PredefinedMenuItem::separator(app),
        PredefinedMenuItem::separator(app),
    ) {
        (Ok(a), Ok(b), Ok(c), Ok(d), Ok(e)) => (a, b, c, d, e),
        _ => return,
    };

    let mut items: Vec<&dyn IsMenuItem<tauri::Wry>> = vec![&status_sub, &sep1];
    for sub in &server_subs {
        items.push(sub);
    }
    items.push(&sep2);
    items.push(&add);
    items.push(&show);
    items.push(&quit);

    let Ok(menu) = Menu::with_items(app, &items) else {
        return;
    };

    // --- tooltip + icon ---
    let server_states: Vec<(String, String)> = servers
        .iter()
        .map(|s| {
            (
                effective_status(&global, &s.status_override),
                conn.get(&s.id).cloned().unwrap_or_default(),
            )
        })
        .collect();
    let icon_state = aggregate_icon_state(in_game, &server_states);
    let word = match icon_state {
        IconState::InGame | IconState::Idle => "Online",
        IconState::Invisible => "Invisible",
        IconState::Problem => "Reconnecting",
        IconState::Offline => "Offline (not recording)",
    };
    let tooltip = if in_game {
        let names: Vec<String> = running.iter().map(|(_, n)| n.clone()).collect();
        format!("KFIRE — Playing {} · {word}", names.join(", "))
    } else {
        format!("KFIRE — {word}")
    };

    if let Some(tray) = app.tray_by_id("kfire-tray") {
        let _ = tray.set_menu(Some(menu));
        let _ = tray.set_tooltip(Some(&tooltip));
        if let Some(base) = app.default_window_icon() {
            let rgba = composite_status_dot(
                base.rgba(),
                base.width(),
                base.height(),
                icon_state.color(),
            );
            let img = tauri::image::Image::new_owned(rgba, base.width(), base.height());
            let _ = tray.set_icon(Some(img));
        }
    }
}

/// Handles a tray menu click: status changes, unlink, window, quit.
fn handle_tray_menu(app: &tauri::AppHandle, id: &str) {
    match id {
        "show" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "quit" => app.exit(0),
        "add" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        id if id.starts_with("g:") => {
            let status = id[2..].to_string();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                state.db.set_setting("global_status", &status);
                for s in state.db.list_servers() {
                    if s.status_override == "inherit" {
                        state.apply_server_status(&s.id);
                    }
                }
                rebuild_tray(&app);
            });
        }
        id if id.starts_with("s:") => {
            // s:{server_id}:{status}
            let rest = &id[2..];
            if let Some(pos) = rest.rfind(':') {
                let server_id = rest[..pos].to_string();
                let status = rest[pos + 1..].to_string();
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<AppState>();
                    state.db.set_server_status_override(&server_id, &status);
                    state.apply_server_status(&server_id);
                    rebuild_tray(&app);
                });
            }
        }
        id if id.starts_with("u:") => {
            let server_id = id[2..].to_string();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                state.stop_session(&server_id);
                let access = state.access_tokens.lock().unwrap().remove(&server_id);
                if let (Some(access), Some(server)) = (access, state.db.get_server(&server_id)) {
                    let api = ApiClient::new(&server.url);
                    let _ = api.logout(&access).await;
                }
                state.db.remove_server(&server_id);
                rebuild_tray(&app);
            });
        }
        _ => {}
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
            set_global_status,
            set_server_status,
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
            scanner_state.reload_ignored(&db.list_ignored());
            let (event_tx, mut event_rx) = mpsc::unbounded_channel();
            scanner::spawn(scanner_state.clone(), event_tx);

            // Last WS status per server, shared with the notification loop.
            let conn_status: Arc<Mutex<HashMap<String, String>>> =
                Arc::new(Mutex::new(HashMap::new()));

            // --- notifications → conn status + tray + webview events ----------
            let (notif_tx, mut notif_rx) = mpsc::unbounded_channel::<Notification>();
            let handle = app.handle().clone();
            let conn_for_notif = conn_status.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(n) = notif_rx.recv().await {
                    if let Notification::Status { server_id, status, .. } = &n {
                        if !server_id.is_empty() {
                            conn_for_notif
                                .lock()
                                .unwrap()
                                .insert(server_id.clone(), status.clone());
                            rebuild_tray(&handle);
                        }
                    }
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
                conn_status,
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
                    // Offline servers have no running session to drain the
                    // queue, so we never enqueue for them (no flood on return).
                    let global = queue_db.get_setting("global_status").unwrap_or_default();
                    let over = queue_db
                        .get_server(&ev.server_id)
                        .map(|s| s.status_override)
                        .unwrap_or_default();
                    if crate::status::effective_status(&global, &over) != "offline" {
                        queue_db.queue_event(&ev.server_id, event_type, &ev.game_slug, &ev.ts.to_rfc3339());
                        queue_notify.notify_one();
                    }
                    let _ = running_handle.emit("kfire://detection", event_type);
                    rebuild_tray(&running_handle);
                }
            });

            app.manage(state);

            // Auto-resume every linked server's session (offline ones stay
            // stopped). A presence app runs in the background, so default to
            // launch-at-login the first time we're linked - but only set it
            // once, then the user's toggle (autostart_configured) wins forever.
            if !db.list_servers().is_empty() {
                app.state::<AppState>().start_all();
                if db.get_setting("autostart_configured").is_none() {
                    use tauri_plugin_autostart::ManagerExt;
                    if app.autolaunch().enable().is_ok() {
                        db.set_setting("autostart_configured", "1");
                    }
                }
            }

            // --- system tray (menu/icon/tooltip filled by rebuild_tray) -------
            TrayIconBuilder::with_id("kfire-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("KFIRE - gaming presence")
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| handle_tray_menu(app, event.id.as_ref()))
                .build(app)?;
            rebuild_tray(&app.handle());

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
