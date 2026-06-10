//! End-to-end integration test against a live kfire-server.
//!
//! Skipped unless KFIRE_TEST_SERVER is set (e.g. http://127.0.0.1:8091).
//! A second account ("bob") observes the WebSocket broadcasts triggered by
//! the client library ("alice") to verify the full presence pipeline.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc, watch, Notify};
use tokio_tungstenite::tungstenite::Message;

use std::collections::HashMap;

use kfire_client_lib::api::ApiClient;
use kfire_client_lib::db::Db;
use kfire_client_lib::scanner::ScannerState;
use kfire_client_lib::ws::{Notification, WsTask};

const PASSWORD: &str = "a-very-long-password";

async fn register(server: &str, username: &str) {
    let resp = reqwest::Client::new()
        .post(format!("{server}/api/v1/auth/register"))
        .json(&json!({
            "username": username,
            "email": format!("{username}@example.org"),
            "password": PASSWORD,
        }))
        .send()
        .await
        .expect("register request");
    assert!(
        resp.status() == 201 || resp.status() == 409,
        "register {username}: HTTP {}",
        resp.status()
    );
}

/// Raw observer connection (bypasses the client library on purpose).
struct Observer {
    stream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
}

impl Observer {
    async fn connect(server: &str, access_token: &str, device_id: &str) -> Self {
        let ws_url = format!("{}/ws", server.replace("http", "ws"));
        let (mut stream, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .expect("observer ws connect");
        let hello = json!({
            "type": "hello",
            "ts": chrono::Utc::now().to_rfc3339(),
            "payload": {
                "protocol_version": 1,
                "access_token": access_token,
                "device_id": device_id,
                "client": "integration-observer",
            },
        });
        stream
            .send(Message::Text(hello.to_string()))
            .await
            .expect("send hello");
        let ack = tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .expect("hello_ack timeout")
            .expect("stream ended")
            .expect("ws error");
        let ack: Value = serde_json::from_str(ack.to_text().unwrap()).unwrap();
        assert_eq!(ack["type"], "hello_ack", "observer handshake");
        Self { stream }
    }

    /// Waits for a presence_update matching (username, status).
    async fn wait_presence(&mut self, username: &str, status: &str) -> Value {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
        loop {
            let msg = tokio::time::timeout_at(deadline, self.stream.next())
                .await
                .unwrap_or_else(|_| panic!("timed out waiting for {username}={status}"))
                .expect("stream ended")
                .expect("ws error");
            let Ok(env) = serde_json::from_str::<Value>(msg.to_text().unwrap_or("")) else {
                continue;
            };
            if env["type"] == "presence_update"
                && env["payload"]["username"] == username
                && env["payload"]["status"] == status
            {
                return env["payload"].clone();
            }
        }
    }
}

#[tokio::test]
async fn full_presence_pipeline() {
    let Ok(server) = std::env::var("KFIRE_TEST_SERVER") else {
        eprintln!("KFIRE_TEST_SERVER not set: skipping integration test");
        return;
    };

    // --- accounts -----------------------------------------------------------
    register(&server, "alice").await;
    register(&server, "bob").await;

    // --- bob: raw observer ----------------------------------------------------
    let bob_api = ApiClient::new(&server);
    let bob_tokens = bob_api
        .login("bob", PASSWORD, "00000000-0000-0000-0000-0000000000b0")
        .await
        .expect("bob login");
    let mut observer = Observer::connect(
        &server,
        &bob_tokens.access_token,
        "00000000-0000-0000-0000-0000000000b0",
    )
    .await;

    // --- alice: the real client library ---------------------------------------
    let dir = std::env::temp_dir().join(format!("kfire-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Arc::new(Db::open(&dir.join("kfire.db")).unwrap());
    let scanner = Arc::new(ScannerState::default());

    let api = ApiClient::new(&server);
    let device_id = db.device_id();
    let tokens = api
        .login("alice", PASSWORD, &device_id)
        .await
        .expect("alice login");

    let server_id = db.add_server(&api.base_url, &tokens.refresh_token, "Test Org");

    // Catalog download + matching index (same path as the link command).
    let games = api
        .fetch_games(&tokens.access_token)
        .await
        .expect("fetch games");
    assert!(games.len() > 10_000, "catalog too small: {}", games.len());
    db.replace_games(&server_id, &games).unwrap();
    let catalog: Vec<_> = games.iter().map(|g| (server_id.clone(), g.clone())).collect();
    scanner.load_catalog(&catalog);
    assert!(
        scanner
            .exe_index
            .read()
            .unwrap()
            .contains_key("cs2.exe"),
        "cs2.exe missing from matching index"
    );

    // --- run the WS task --------------------------------------------------------
    let queue_notify = Arc::new(Notify::new());
    let (notif_tx, mut notif_rx) = mpsc::unbounded_channel::<Notification>();
    let (stop_tx, stop_rx) = watch::channel(false);

    let task = WsTask {
        server_id: server_id.clone(),
        db: db.clone(),
        scanner: scanner.clone(),
        queue_notify: queue_notify.clone(),
        notifications: notif_tx,
        access_tokens: Arc::new(std::sync::Mutex::new(HashMap::new())),
        shutdown: stop_rx,
    };
    let task_handle = tokio::spawn(task.run());

    // Wait for "connected".
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        let n = tokio::time::timeout_at(deadline, notif_rx.recv())
            .await
            .expect("timed out waiting for connected status")
            .expect("notification channel closed");
        if let Notification::Status { status, .. } = &n {
            if status == "connected" {
                break;
            }
        }
    }

    // Bob sees alice come online.
    observer.wait_presence("alice", "online").await;

    // --- simulate a detection: queued event flows out over WS --------------------
    db.queue_event(
        &server_id,
        "game_started",
        "counter-strike-2",
        &chrono::Utc::now().to_rfc3339(),
    );
    queue_notify.notify_one();

    let payload = observer.wait_presence("alice", "in_game").await;
    assert_eq!(payload["game"]["slug"], "counter-strike-2");

    db.queue_event(
        &server_id,
        "game_stopped",
        "counter-strike-2",
        &chrono::Utc::now().to_rfc3339(),
    );
    queue_notify.notify_one();
    observer.wait_presence("alice", "online").await;

    // The offline queue must be fully drained.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(db.pending_events(&server_id).len(), 0, "queue not drained");

    // --- logout: bob sees alice go offline ----------------------------------------
    stop_tx.send(true).unwrap();
    observer.wait_presence("alice", "offline").await;

    let _ = tokio::time::timeout(Duration::from_secs(3), task_handle).await;
    std::fs::remove_dir_all(&dir).ok();
}
