//! Local SQLite cache: settings, linked servers, games catalogs, offline queue.
//!
//! Everything the client needs to keep working without a server: the games
//! list for process matching, and a queue of presence events to flush on
//! reconnect. The client can be linked to several KFIRE servers at once, so the
//! games catalog and the event queue are keyed by `server_id`.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use uuid::Uuid;

pub struct Db {
    conn: Mutex<Connection>,
}

/// One linked KFIRE server.
#[derive(Debug, Clone)]
pub struct ServerRow {
    pub id: String,
    pub url: String,
    pub refresh_token: String,
    pub org_name: String,
    /// `inherit` | `online` | `invisible` | `offline` (used by the status model).
    pub status_override: String,
}

/// One catalog entry, as cached from `GET /api/v1/games`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedGame {
    pub slug: String,
    pub name: String,
    pub executable_names: Vec<String>,
}

/// A presence event queued while offline, for one server.
#[derive(Debug, Clone)]
pub struct PendingEvent {
    pub id: i64,
    pub event_type: String, // "game_started" | "game_stopped"
    pub game_slug: String,
    pub ts: String, // RFC 3339
    pub payload: Option<String>,
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS settings (
                 key   TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS servers (
                 id              TEXT PRIMARY KEY,
                 url             TEXT NOT NULL,
                 refresh_token   TEXT NOT NULL,
                 org_name        TEXT NOT NULL DEFAULT '',
                 status_override TEXT NOT NULL DEFAULT 'inherit',
                 created_at      TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS ignored_games (
                 server_id TEXT NOT NULL,
                 slug      TEXT NOT NULL,
                 PRIMARY KEY (server_id, slug)
             );",
        )?;

        // Schema v2: the games catalog and the offline queue are keyed by
        // server_id. The catalog is re-downloadable and the queue is transient,
        // so it is safe to rebuild these tables when upgrading.
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 2 {
            conn.execute_batch(
                "DROP TABLE IF EXISTS games;
                 DROP TABLE IF EXISTS pending_events;
                 CREATE TABLE games (
                     server_id TEXT NOT NULL,
                     slug      TEXT NOT NULL,
                     name      TEXT NOT NULL,
                     exe_names TEXT NOT NULL,  -- JSON array of lowercase basenames
                     PRIMARY KEY (server_id, slug)
                 );
                 CREATE TABLE pending_events (
                     id        INTEGER PRIMARY KEY AUTOINCREMENT,
                     server_id TEXT NOT NULL,
                     type      TEXT NOT NULL,
                     game_slug TEXT NOT NULL,
                     ts        TEXT NOT NULL  -- RFC 3339
                 );
                 PRAGMA user_version = 2;",
            )?;

            // Migrate a legacy single-server install into the servers table.
            let legacy_url: Option<String> = conn
                .query_row(
                    "SELECT value FROM settings WHERE key = 'server_url'",
                    [],
                    |r| r.get(0),
                )
                .ok();
            let legacy_refresh: Option<String> = conn
                .query_row(
                    "SELECT value FROM settings WHERE key = 'refresh_token'",
                    [],
                    |r| r.get(0),
                )
                .ok();
            let server_count: i64 =
                conn.query_row("SELECT count(*) FROM servers", [], |r| r.get(0))?;
            if server_count == 0 {
                if let (Some(url), Some(refresh)) = (legacy_url, legacy_refresh) {
                    let org = conn
                        .query_row(
                            "SELECT value FROM settings WHERE key = 'username'",
                            [],
                            |r| r.get::<_, String>(0),
                        )
                        .unwrap_or_default();
                    conn.execute(
                        "INSERT INTO servers (id, url, refresh_token, org_name, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![Uuid::new_v4().to_string(), url, refresh, org, now()],
                    )?;
                }
            }
            conn.execute(
                "DELETE FROM settings WHERE key IN ('server_url', 'refresh_token', 'games_synced_at')",
                [],
            )?;
        }

        // v2 -> v3: a queued event can now carry its own JSON payload. Session
        // events leave it NULL and keep the payload ws.rs has always built for
        // them; a match result stores the whole object it must send.
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 3 {
            conn.execute_batch(
                "ALTER TABLE pending_events ADD COLUMN payload TEXT;
                 PRAGMA user_version = 3;",
            )?;
        }

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // --- settings ----------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .ok()
    }

    pub fn set_setting(&self, key: &str, value: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        );
    }

    pub fn delete_setting(&self, key: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute("DELETE FROM settings WHERE key = ?1", params![key]);
    }

    /// Stable per-installation device ID, generated on first use.
    /// TODO(post-mvp): move the refresh token to the OS keychain (keyring).
    pub fn device_id(&self) -> String {
        if let Some(id) = self.get_setting("device_id") {
            return id;
        }
        let id = Uuid::new_v4().to_string();
        self.set_setting("device_id", &id);
        id
    }

    // --- servers -----------------------------------------------------------

    /// Links a new server, returning its generated id.
    pub fn add_server(&self, url: &str, refresh_token: &str, org_name: &str) -> String {
        let id = Uuid::new_v4().to_string();
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO servers (id, url, refresh_token, org_name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, url, refresh_token, org_name, now()],
        );
        // A fresh link supersedes any "session expired" address kept below.
        let _ = conn.execute("DELETE FROM settings WHERE key = 'expired_server_url'", []);
        id
    }

    pub fn list_servers(&self) -> Vec<ServerRow> {
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, url, refresh_token, org_name, status_override
             FROM servers ORDER BY created_at",
        ) else {
            return Vec::new();
        };
        let rows = stmt.query_map([], row_to_server);
        match rows {
            Ok(it) => it.flatten().collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn get_server(&self, id: &str) -> Option<ServerRow> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, url, refresh_token, org_name, status_override
             FROM servers WHERE id = ?1",
            params![id],
            row_to_server,
        )
        .ok()
    }

    pub fn find_server_by_url(&self, url: &str) -> Option<ServerRow> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, url, refresh_token, org_name, status_override
             FROM servers WHERE url = ?1",
            params![url],
            row_to_server,
        )
        .ok()
    }

    pub fn update_server_refresh(&self, id: &str, refresh_token: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "UPDATE servers SET refresh_token = ?2 WHERE id = ?1",
            params![id, refresh_token],
        );
    }

    /// Sets a server's status override (`inherit` | `online` | `invisible` | `offline`).
    pub fn set_server_status_override(&self, id: &str, status: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "UPDATE servers SET status_override = ?2 WHERE id = ?1",
            params![id, status],
        );
    }

    pub fn set_server_org_name(&self, id: &str, org_name: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "UPDATE servers SET org_name = ?2 WHERE id = ?1",
            params![id, org_name],
        );
    }

    /// Unlinks a server and drops its cached catalog and queued events.
    pub fn remove_server(&self, id: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute("DELETE FROM games WHERE server_id = ?1", params![id]);
        let _ = conn.execute("DELETE FROM pending_events WHERE server_id = ?1", params![id]);
        let _ = conn.execute("DELETE FROM servers WHERE id = ?1", params![id]);
    }

    /// Drops a link the server definitively refused, but keeps its address so
    /// the link form can offer it back: the member re-links in one click
    /// instead of facing an empty form as if KFIRE had never been set up.
    pub fn forget_rejected_server(&self, id: &str) {
        if let Some(server) = self.get_server(id) {
            self.set_setting("expired_server_url", &server.url);
        }
        self.remove_server(id);
    }

    /// The address of the last link the server refused, until the next link.
    pub fn expired_server_url(&self) -> Option<String> {
        self.get_setting("expired_server_url")
    }

    /// Keeps the last match a game module recorded, for the client window only.
    /// The queue forgets a match once it is sent; the member still wants to see
    /// what was counted. One per game, the payload exactly as it was queued.
    pub fn remember_last_match(&self, slug: &str, payload: &serde_json::Value) {
        self.set_setting(&format!("last_match:{slug}"), &payload.to_string());
    }

    /// The last match remembered for `slug`, if any and still readable.
    pub fn last_match(&self, slug: &str) -> Option<serde_json::Value> {
        self.get_setting(&format!("last_match:{slug}"))
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    // --- games catalog (per server) ----------------------------------------

    pub fn replace_games(&self, server_id: &str, games: &[CachedGame]) -> rusqlite::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM games WHERE server_id = ?1", params![server_id])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO games (server_id, slug, name, exe_names) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for g in games {
                let exes = serde_json::to_string(&g.executable_names).unwrap_or_default();
                stmt.execute(params![server_id, g.slug, g.name, exes])?;
            }
        }
        tx.commit()
    }

    /// Loads every server's catalog, tagged with the owning `server_id`.
    pub fn load_games(&self) -> Vec<(String, CachedGame)> {
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare("SELECT server_id, slug, name, exe_names FROM games")
        else {
            return Vec::new();
        };
        let rows = stmt.query_map([], |r| {
            let server_id: String = r.get(0)?;
            let exes: String = r.get(3)?;
            Ok((
                server_id,
                CachedGame {
                    slug: r.get(1)?,
                    name: r.get(2)?,
                    executable_names: serde_json::from_str(&exes).unwrap_or_default(),
                },
            ))
        });
        match rows {
            Ok(it) => it.flatten().collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn games_count(&self) -> i64 {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT count(*) FROM games", [], |r| r.get(0))
            .unwrap_or(0)
    }

    // --- ignored games (per server) ----------------------------------------

    /// Every `(server_id, slug)` pair the user has chosen to ignore.
    pub fn list_ignored(&self) -> Vec<(String, String)> {
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare("SELECT server_id, slug FROM ignored_games") else {
            return Vec::new();
        };
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)));
        match rows {
            Ok(it) => it.flatten().collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn add_ignored(&self, server_id: &str, slug: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT OR IGNORE INTO ignored_games (server_id, slug) VALUES (?1, ?2)",
            params![server_id, slug],
        );
    }

    pub fn remove_ignored(&self, server_id: &str, slug: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "DELETE FROM ignored_games WHERE server_id = ?1 AND slug = ?2",
            params![server_id, slug],
        );
    }

    // --- offline event queue (per server) ----------------------------------

    pub fn queue_event(
        &self,
        server_id: &str,
        event_type: &str,
        game_slug: &str,
        ts: &str,
        payload: Option<&str>,
    ) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO pending_events (server_id, type, game_slug, ts, payload)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![server_id, event_type, game_slug, ts, payload],
        );
    }

    pub fn pending_events(&self, server_id: &str) -> Vec<PendingEvent> {
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, type, game_slug, ts, payload FROM pending_events
             WHERE server_id = ?1 ORDER BY id",
        ) else {
            return Vec::new();
        };
        let rows = stmt.query_map(params![server_id], |r| {
            Ok(PendingEvent {
                id: r.get(0)?,
                event_type: r.get(1)?,
                game_slug: r.get(2)?,
                ts: r.get(3)?,
                payload: r.get(4)?,
            })
        });
        match rows {
            Ok(it) => it.flatten().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Rewrites the payload of an event still waiting to be sent, and says how
    /// many rows it touched: 0 means it already left (or was never queued).
    pub fn update_pending_payload(
        &self,
        server_id: &str,
        event_type: &str,
        game_slug: &str,
        ts: &str,
        payload: &str,
    ) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE pending_events SET payload = ?5
             WHERE server_id = ?1 AND type = ?2 AND game_slug = ?3 AND ts = ?4",
            params![server_id, event_type, game_slug, ts, payload],
        )
        .unwrap_or(0)
    }

    pub fn delete_event(&self, id: i64) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute("DELETE FROM pending_events WHERE id = ?1", params![id]);
    }
}

fn row_to_server(r: &rusqlite::Row) -> rusqlite::Result<ServerRow> {
    Ok(ServerRow {
        id: r.get(0)?,
        url: r.get(1)?,
        refresh_token: r.get(2)?,
        org_name: r.get(3)?,
        status_override: r.get(4)?,
    })
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Db {
        Db::open(Path::new(":memory:")).unwrap()
    }

    fn game(slug: &str, exe: &str) -> CachedGame {
        CachedGame {
            slug: slug.into(),
            name: slug.into(),
            executable_names: vec![exe.into()],
        }
    }

    #[test]
    fn a_rejected_link_remembers_its_address() {
        let db = mem();
        let a = db.add_server("https://kfire.example.org", "ra", "Guild A");
        db.forget_rejected_server(&a);
        assert!(db.get_server(&a).is_none(), "the dead link is gone");
        assert_eq!(
            db.expired_server_url().as_deref(),
            Some("https://kfire.example.org"),
            "but its address is kept to pre-fill the link form"
        );
    }

    #[test]
    fn linking_again_clears_the_expired_address() {
        let db = mem();
        let a = db.add_server("https://kfire.example.org", "ra", "Guild A");
        db.forget_rejected_server(&a);
        db.add_server("https://kfire.example.org", "rb", "Guild A");
        assert_eq!(db.expired_server_url(), None);
    }

    #[test]
    fn a_manual_unlink_does_not_offer_the_address_back() {
        let db = mem();
        let a = db.add_server("https://kfire.example.org", "ra", "Guild A");
        db.remove_server(&a);
        assert_eq!(db.expired_server_url(), None);
    }

    #[test]
    fn the_last_match_is_remembered_per_game() {
        let db = mem();
        assert!(db.last_match("hearthstone").is_none());
        db.remember_last_match("hearthstone", &serde_json::json!({"placement": 3}));
        db.remember_last_match("hearthstone", &serde_json::json!({"placement": 1}));
        db.remember_last_match("rocket-league", &serde_json::json!({"result": "win"}));
        assert_eq!(db.last_match("hearthstone").unwrap()["placement"], 1, "the last one wins");
        assert_eq!(db.last_match("rocket-league").unwrap()["result"], "win");
    }

    #[test]
    fn a_corrupt_last_match_reads_as_none() {
        let db = mem();
        db.set_setting("last_match:hearthstone", "not json");
        assert!(db.last_match("hearthstone").is_none());
    }

    #[test]
    fn servers_crud_and_lookup() {
        let db = mem();
        let a = db.add_server("https://a.example", "ra", "Guild A");
        let b = db.add_server("https://b.example", "rb", "Guild B");

        assert_eq!(db.list_servers().len(), 2);
        assert_eq!(db.get_server(&a).unwrap().org_name, "Guild A");
        assert_eq!(db.find_server_by_url("https://b.example").unwrap().id, b);

        db.update_server_refresh(&a, "ra2");
        assert_eq!(db.get_server(&a).unwrap().refresh_token, "ra2");
    }

    #[test]
    fn games_and_queue_are_per_server() {
        let db = mem();
        let a = db.add_server("https://a.example", "ra", "A");
        let b = db.add_server("https://b.example", "rb", "B");

        db.replace_games(&a, &[game("game-a", "a.exe")]).unwrap();
        db.replace_games(&b, &[game("game-b", "b.exe")]).unwrap();
        assert_eq!(db.load_games().len(), 2);

        db.queue_event(&a, "game_started", "game-a", "t1", None);
        db.queue_event(&b, "game_started", "game-b", "t2", None);
        assert_eq!(db.pending_events(&a).len(), 1);
        assert_eq!(db.pending_events(&b).len(), 1);
        assert_eq!(db.pending_events(&a)[0].game_slug, "game-a");
    }

    #[test]
    fn remove_server_cascades() {
        let db = mem();
        let a = db.add_server("https://a.example", "ra", "A");
        db.replace_games(&a, &[game("game-a", "a.exe")]).unwrap();
        db.queue_event(&a, "game_started", "game-a", "t1", None);

        db.remove_server(&a);
        assert!(db.get_server(&a).is_none());
        assert_eq!(db.load_games().len(), 0);
        assert_eq!(db.pending_events(&a).len(), 0);
    }

    #[test]
    fn queue_keeps_payload_and_existing_events_stay_payloadless() {
        let db = mem();
        let a = db.add_server("https://a", "r", "A");
        db.queue_event(&a, "game_started", "game-a", "t1", None);
        db.queue_event(
            &a,
            "match_result",
            "hearthstone",
            "t2",
            Some(r#"{"mode":"battlegrounds"}"#),
        );

        let events = db.pending_events(&a);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].payload, None);
        assert_eq!(
            events[1].payload.as_deref(),
            Some(r#"{"mode":"battlegrounds"}"#)
        );
    }

    #[test]
    fn a_pending_payload_can_be_rewritten_until_it_is_sent() {
        let db = mem();
        let a = db.add_server("https://a", "r", "A");
        let b = db.add_server("https://b", "r", "B");
        db.queue_event(&a, "match_result", "hearthstone", "t1", Some("old"));
        db.queue_event(&b, "match_result", "hearthstone", "t1", Some("old"));

        assert_eq!(
            db.update_pending_payload(&a, "match_result", "hearthstone", "t1", "new"),
            1
        );
        assert_eq!(db.pending_events(&a)[0].payload.as_deref(), Some("new"));
        // Another server's row is its own.
        assert_eq!(db.pending_events(&b)[0].payload.as_deref(), Some("old"));

        // Once sent (deleted), there is nothing left to rewrite.
        let id = db.pending_events(&a)[0].id;
        db.delete_event(id);
        assert_eq!(
            db.update_pending_payload(&a, "match_result", "hearthstone", "t1", "newer"),
            0
        );
    }
}
