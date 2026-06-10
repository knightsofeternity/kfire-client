//! WebSocket presence task: connects to one server, authenticates with the
//! `hello` handshake, pushes that server's queued game events, heartbeats every
//! 30 s and reconnects with exponential backoff + jitter.
//!
//! Protocol: https://github.com/knightsofeternity/kfire-protocol/blob/main/websocket-events.md
//!
//! Design notes:
//! - One task per linked server (keyed by `server_id`); they share one scanner.
//! - Every scanner event goes through the SQLite queue first (see lib.rs);
//!   this task drains its own server's queue whenever connected. Offline
//!   resilience falls out naturally: no connection, no drain, events wait.
//! - This task owns its server's token refresh: it is the only place that
//!   server's refresh token is spent, which avoids racing the single-use
//!   rotation.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
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
/// Each update is attributed to the server it came from.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Notification {
    Status {
        server_id: String,
        status: String,
        detail: String,
    },
    Presence {
        server_id: String,
        update: serde_json::Value,
    },
}

pub struct WsTask {
    pub server_id: String,
    pub db: Arc<Db>,
    pub scanner: Arc<ScannerState>,
    /// Woken whenever the scanner queued a new event (for any server).
    pub queue_notify: Arc<Notify>,
    /// Status / presence notifications for the UI.
    pub notifications: UnboundedSender<Notification>,
    /// Latest access token per server, shared so unlink can revoke server-side.
    pub access_tokens: Arc<Mutex<HashMap<String, String>>>,
    /// Flips to true when this server's session ends (unlink).
    pub shutdown: watch::Receiver<bool>,
}

impl WsTask {
    fn status(&self, status: &str, detail: &str) {
        let _ = self.notifications.send(Notification::Status {
            server_id: self.server_id.clone(),
            status: status.into(),
            detail: detail.into(),
        });
    }

    fn shutting_down(&self) -> bool {
        *self.shutdown.borrow()
    }

    /// Runs until unlink or until the refresh token is rejected.
    pub async fn run(mut self) {
        let Some(server) = self.db.get_server(&self.server_id) else {
            self.status("logged_out", "no server configured");
            return;
        };
        let api = ApiClient::new(&server.url);
        let device_id = self.db.device_id();
        let mut attempt: u32 = 0;

        loop {
            if self.shutting_down() {
                return;
            }
            self.status("connecting", "");

            // --- fresh access token (refresh rotation) ----------------------
            let Some(server) = self.db.get_server(&self.server_id) else {
                self.status("logged_out", "");
                return;
            };
            let access = match api.refresh(&server.refresh_token, &device_id).await {
                Ok(tokens) => {
                    self.db
                        .update_server_refresh(&self.server_id, &tokens.refresh_token);
                    self.access_tokens
                        .lock()
                        .unwrap()
                        .insert(self.server_id.clone(), tokens.access_token.clone());
                    tokens.access_token
                }
                Err(ApiError::Server { .. }) => {
                    // The refresh token is dead (revoked, expired, rotated
                    // elsewhere): unlink this server.
                    self.db.remove_server(&self.server_id);
                    self.status("logged_out", "session expired, please link again");
                    return;
                }
                Err(ApiError::Network(e)) => {
                    log::warn!("ws[{}]: refresh unreachable: {e}", self.server_id);
                    self.status("disconnected", &format!("server unreachable: {e}"));
                    attempt += 1;
                    if self.backoff(attempt).await {
                        return;
                    }
                    continue;
                }
            };

            // --- connect + serve -------------------------------------------
            let reason = match self.serve(&api, &access, &device_id).await {
                ServeEnd::Shutdown => return,
                ServeEnd::ConnectFailed(e) => {
                    log::warn!("ws[{}]: connect failed: {e}", self.server_id);
                    attempt += 1;
                    e
                }
                ServeEnd::Dropped(e) => {
                    log::warn!("ws[{}]: connection dropped: {e}", self.server_id);
                    attempt = 0; // we did connect: restart backoff from small
                    e
                }
            };
            self.status("disconnected", &reconnect_detail(&reason));
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
        log::info!("ws[{}]: retrying in {delay:?}", self.server_id);
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
        log::info!("ws[{}]: connected to {url}", self.server_id);
        self.status("connected", "");

        // --- catalog refresh (uses the still-fresh access token) -------------
        self.maybe_sync_catalog(api, access).await;

        // --- re-announce running games (server dedups open sessions) ---------
        for slug in self.scanner.running_for(&self.server_id) {
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
                    server_id: self.server_id.clone(),
                    update: env.payload,
                });
            }
            "error" => log::warn!("ws[{}]: server error notice: {}", self.server_id, env.payload),
            // Unknown types ignored for forward compatibility.
            _ => {}
        }
    }

    /// Sends every queued event for this server in order, deleting each once sent.
    async fn drain_queue(&self, stream: &mut WsStream) -> Result<(), String> {
        for ev in self.db.pending_events(&self.server_id) {
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

    /// Downloads this server's games catalog when missing or stale.
    async fn maybe_sync_catalog(&self, api: &ApiClient, access: &str) {
        let key = format!("games_synced_at:{}", self.server_id);
        let stale = match self.db.get_setting(&key) {
            None => true,
            Some(ts) => chrono::DateTime::parse_from_rfc3339(&ts)
                .map(|t| {
                    chrono::Utc::now().signed_duration_since(t.with_timezone(&chrono::Utc))
                        > chrono::Duration::from_std(CATALOG_MAX_AGE).unwrap()
                })
                .unwrap_or(true),
        };
        if !stale {
            return;
        }
        match api.fetch_games(access).await {
            Ok(games) => {
                if let Err(e) = self.db.replace_games(&self.server_id, &games) {
                    log::error!("ws[{}]: cache games: {e}", self.server_id);
                    return;
                }
                // Rebuild the shared index from every server's catalog.
                self.scanner.load_catalog(&self.db.load_games());
                self.db.set_setting(&key, &chrono::Utc::now().to_rfc3339());
                log::info!(
                    "ws[{}]: games catalog synced ({} games)",
                    self.server_id,
                    games.len()
                );
            }
            Err(e) => log::warn!("ws[{}]: games sync failed: {e}", self.server_id),
        }
    }
}

enum ServeEnd {
    /// Unlink requested: stop for good.
    Shutdown,
    /// Could not establish/authenticate: keep growing the backoff.
    ConnectFailed(String),
    /// Established connection dropped: reconnect quickly.
    Dropped(String),
}

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

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

/// Builds the user-facing "disconnected" detail from the reason the connection
/// ended. An empty reason (e.g. a clean shutdown path) yields a bare
/// "reconnecting…"; otherwise the real cause is shown so a stuck client is
/// self-diagnosable instead of an opaque spinner.
fn reconnect_detail(reason: &str) -> String {
    if reason.is_empty() {
        "reconnecting…".to_string()
    } else {
        format!("reconnecting… ({reason})")
    }
}

#[cfg(test)]
mod tests {
    use super::reconnect_detail;

    #[test]
    fn reconnect_detail_includes_reason() {
        assert_eq!(
            reconnect_detail("no hello_ack"),
            "reconnecting… (no hello_ack)"
        );
    }

    #[test]
    fn reconnect_detail_empty_reason_is_bare() {
        assert_eq!(reconnect_detail(""), "reconnecting…");
    }
}
