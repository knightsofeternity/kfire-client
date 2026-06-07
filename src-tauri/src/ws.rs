//! WebSocket presence task: connects to the server, authenticates with the
//! `hello` handshake, pushes queued game events, heartbeats every 30 s and
//! reconnects with exponential backoff + jitter.
//!
//! Protocol: https://github.com/knightsofeternity/kfire-protocol/blob/main/websocket-events.md
//!
//! Design notes:
//! - Every scanner event goes through the SQLite queue first (see lib.rs);
//!   this task drains the queue whenever connected. Offline resilience falls
//!   out naturally: no connection, no drain, events wait in SQLite.
//! - This task owns token refresh: it is the only place a refresh token is
//!   spent, which avoids racing the single-use rotation.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::{mpsc::UnboundedSender, watch, Notify};
use tokio_tungstenite::tungstenite::Message;

use crate::api::{ApiClient, ApiError};
use crate::db::Db;
use crate::scanner::ScannerState;

const PROTOCOL_VERSION: u64 = 1;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const HELLO_ACK_TIMEOUT: Duration = Duration::from_secs(10);
/// Refresh the games catalog when older than this.
const CATALOG_MAX_AGE: Duration = Duration::from_secs(24 * 3600);

/// Updates surfaced to the UI layer (lib.rs forwards them as Tauri events).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Notification {
    Status { status: String, detail: String },
    Presence { update: serde_json::Value },
}

pub struct WsTask {
    pub db: Arc<Db>,
    pub scanner: Arc<ScannerState>,
    /// Woken whenever the scanner queued a new event.
    pub queue_notify: Arc<Notify>,
    /// Status / presence notifications for the UI.
    pub notifications: UnboundedSender<Notification>,
    /// Last known access token, shared so logout can revoke server-side.
    pub access_token: Arc<std::sync::Mutex<Option<String>>>,
    /// Flips to true when the session ends (logout).
    pub shutdown: watch::Receiver<bool>,
}

impl WsTask {
    fn status(&self, status: &str, detail: &str) {
        let _ = self.notifications.send(Notification::Status {
            status: status.into(),
            detail: detail.into(),
        });
    }

    fn shutting_down(&self) -> bool {
        *self.shutdown.borrow()
    }

    /// Runs until logout or until the refresh token is rejected.
    pub async fn run(mut self) {
        let Some(server_url) = self.db.get_setting("server_url") else {
            self.status("logged_out", "no server configured");
            return;
        };
        let api = ApiClient::new(&server_url);
        let device_id = self.db.device_id();
        let mut attempt: u32 = 0;

        loop {
            if self.shutting_down() {
                return;
            }
            self.status("connecting", "");

            // --- fresh access token (refresh rotation) ----------------------
            let Some(refresh_token) = self.db.get_setting("refresh_token") else {
                self.status("logged_out", "");
                return;
            };
            let access = match api.refresh(&refresh_token, &device_id).await {
                Ok(tokens) => {
                    self.db.set_setting("refresh_token", &tokens.refresh_token);
                    *self.access_token.lock().unwrap() = Some(tokens.access_token.clone());
                    tokens.access_token
                }
                Err(ApiError::Server { .. }) => {
                    // The refresh token is dead (revoked, expired, rotated
                    // elsewhere): the session is over.
                    self.db.delete_setting("refresh_token");
                    self.status("logged_out", "session expired, please sign in again");
                    return;
                }
                Err(ApiError::Network(e)) => {
                    log::warn!("ws: refresh unreachable: {e}");
                    attempt += 1;
                    self.status("disconnected", "server unreachable");
                    if self.backoff(attempt).await {
                        return;
                    }
                    continue;
                }
            };

            // --- connect + serve -------------------------------------------
            match self.serve(&api, &access, &device_id).await {
                ServeEnd::Shutdown => return,
                ServeEnd::ConnectFailed(e) => {
                    log::warn!("ws: connect failed: {e}");
                    attempt += 1;
                }
                ServeEnd::Dropped(e) => {
                    log::warn!("ws: connection dropped: {e}");
                    attempt = 0; // we did connect: restart backoff from small
                }
            }
            self.status("disconnected", "reconnecting…");
            if self.backoff(attempt.max(1)).await {
                return;
            }
        }
    }

    /// Sleeps `min(2^attempt s + jitter, 60 s)`. Returns true on shutdown.
    async fn backoff(&mut self, attempt: u32) -> bool {
        let base = 2u64.saturating_pow(attempt.min(6));
        let jitter = rand::thread_rng().gen_range(0..1000);
        let delay = Duration::from_millis((base * 1000 + jitter).min(60_000));
        log::info!("ws: retrying in {delay:?}");
        tokio::select! {
            _ = tokio::time::sleep(delay) => false,
            _ = self.shutdown.changed() => self.shutting_down(),
        }
    }

    async fn serve(&mut self, api: &ApiClient, access: &str, device_id: &str) -> ServeEnd {
        let url = api.ws_url();
        let (mut stream, _) = match tokio_tungstenite::connect_async(&url).await {
            Ok(ok) => ok,
            Err(e) => return ServeEnd::ConnectFailed(e.to_string()),
        };

        // --- hello handshake -------------------------------------------------
        let hello = envelope(
            "hello",
            json!({
                "protocol_version": PROTOCOL_VERSION,
                "access_token": access,
                "device_id": device_id,
                "client": format!("kfire-client/{} ({})", env!("CARGO_PKG_VERSION"), std::env::consts::OS),
            }),
        );
        if let Err(e) = stream.send(Message::Text(hello)).await {
            return ServeEnd::ConnectFailed(e.to_string());
        }

        match tokio::time::timeout(HELLO_ACK_TIMEOUT, stream.next()).await {
            Ok(Some(Ok(Message::Text(txt)))) => {
                let env: Envelope = match serde_json::from_str(&txt) {
                    Ok(env) => env,
                    Err(e) => return ServeEnd::ConnectFailed(format!("bad hello_ack: {e}")),
                };
                if env.r#type != "hello_ack" {
                    return ServeEnd::ConnectFailed(format!("expected hello_ack, got {}", env.r#type));
                }
            }
            Ok(_) | Err(_) => return ServeEnd::ConnectFailed("no hello_ack".into()),
        }
        log::info!("ws: connected to {url}");
        self.status("connected", "");

        // --- catalog refresh (uses the still-fresh access token) -------------
        self.maybe_sync_catalog(api, access).await;

        // --- re-announce running games (server dedups open sessions) ---------
        for slug in self.scanner.running_slugs() {
            let msg = envelope("game_started", json!({ "game_slug": slug }));
            if stream.send(Message::Text(msg)).await.is_err() {
                return ServeEnd::Dropped("send failed".into());
            }
        }
        // Anything queued while offline goes out now.
        if let Err(e) = self.drain_queue(&mut stream).await {
            return ServeEnd::Dropped(e);
        }

        // --- main loop --------------------------------------------------------
        let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = self.shutdown.changed() => {
                    if self.shutting_down() {
                        let _ = stream.close(None).await;
                        return ServeEnd::Shutdown;
                    }
                }
                _ = self.queue_notify.notified() => {
                    if let Err(e) = self.drain_queue(&mut stream).await {
                        return ServeEnd::Dropped(e);
                    }
                }
                _ = heartbeat.tick() => {
                    let msg = envelope("heartbeat", json!({}));
                    if stream.send(Message::Text(msg)).await.is_err() {
                        return ServeEnd::Dropped("heartbeat send failed".into());
                    }
                }
                incoming = stream.next() => {
                    match incoming {
                        Some(Ok(Message::Text(txt))) => self.handle_incoming(&txt),
                        Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                        Some(Ok(Message::Close(frame))) => {
                            return ServeEnd::Dropped(format!("server closed: {frame:?}"));
                        }
                        Some(Ok(_)) => {}
                        Some(Err(e)) => return ServeEnd::Dropped(e.to_string()),
                        None => return ServeEnd::Dropped("stream ended".into()),
                    }
                }
            }
        }
    }

    fn handle_incoming(&self, txt: &str) {
        let Ok(env) = serde_json::from_str::<Envelope>(txt) else {
            return;
        };
        match env.r#type.as_str() {
            "presence_update" => {
                let _ = self.notifications.send(Notification::Presence {
                    update: env.payload,
                });
            }
            "error" => log::warn!("ws: server error notice: {}", env.payload),
            // Unknown types ignored for forward compatibility.
            _ => {}
        }
    }

    /// Sends every queued event in order, deleting each one once sent.
    async fn drain_queue(&self, stream: &mut WsStream) -> Result<(), String> {
        for ev in self.db.pending_events() {
            let msg = envelope(
                &ev.event_type,
                json!({ "game_slug": ev.game_slug, "started_at": ev.ts }),
            );
            stream
                .send(Message::Text(msg))
                .await
                .map_err(|e| e.to_string())?;
            self.db.delete_event(ev.id);
        }
        Ok(())
    }

    /// Downloads the games catalog when missing or stale.
    async fn maybe_sync_catalog(&self, api: &ApiClient, access: &str) {
        let stale = match self.db.get_setting("games_synced_at") {
            None => true,
            Some(ts) => chrono::DateTime::parse_from_rfc3339(&ts)
                .map(|t| chrono::Utc::now().signed_duration_since(t.with_timezone(&chrono::Utc))
                    > chrono::Duration::from_std(CATALOG_MAX_AGE).unwrap())
                .unwrap_or(true),
        };
        if !stale && self.db.games_count() > 0 {
            return;
        }
        match api.fetch_games(access).await {
            Ok(games) => {
                if let Err(e) = self.db.replace_games(&games) {
                    log::error!("ws: cache games: {e}");
                    return;
                }
                self.scanner.load_catalog(&games);
                self.db
                    .set_setting("games_synced_at", &chrono::Utc::now().to_rfc3339());
                log::info!("ws: games catalog synced ({} games)", games.len());
            }
            Err(e) => log::warn!("ws: games sync failed: {e}"),
        }
    }
}

enum ServeEnd {
    /// Logout requested: stop for good.
    Shutdown,
    /// Could not establish/authenticate: keep growing the backoff.
    ConnectFailed(String),
    /// Established connection dropped: reconnect quickly.
    Dropped(String),
}

type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

#[derive(Deserialize)]
struct Envelope {
    r#type: String,
    #[serde(default)]
    payload: serde_json::Value,
}

fn envelope(typ: &str, payload: serde_json::Value) -> String {
    json!({
        "type": typ,
        "ts": chrono::Utc::now().to_rfc3339(),
        "payload": payload,
    })
    .to_string()
}
