//! Local SQLite cache: settings, games catalog, offline event queue.
//!
//! Everything the client needs to keep working without the server: the games
//! list for process matching, and a queue of presence events to flush on
//! reconnect.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use uuid::Uuid;

pub struct Db {
    conn: Mutex<Connection>,
}

/// One catalog entry, as cached from `GET /api/v1/games`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedGame {
    pub slug: String,
    pub name: String,
    pub executable_names: Vec<String>,
}

/// A presence event queued while offline.
#[derive(Debug, Clone)]
pub struct PendingEvent {
    pub id: i64,
    pub event_type: String, // "game_started" | "game_stopped"
    pub game_slug: String,
    pub ts: String, // RFC 3339
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
             CREATE TABLE IF NOT EXISTS games (
                 slug      TEXT PRIMARY KEY,
                 name      TEXT NOT NULL,
                 exe_names TEXT NOT NULL  -- JSON array of lowercase basenames
             );
             CREATE TABLE IF NOT EXISTS pending_events (
                 id        INTEGER PRIMARY KEY AUTOINCREMENT,
                 type      TEXT NOT NULL,
                 game_slug TEXT NOT NULL,
                 ts        TEXT NOT NULL  -- RFC 3339
             );",
        )?;
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

    // --- games catalog -----------------------------------------------------

    pub fn replace_games(&self, games: &[CachedGame]) -> rusqlite::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM games", [])?;
        {
            let mut stmt =
                tx.prepare("INSERT INTO games (slug, name, exe_names) VALUES (?1, ?2, ?3)")?;
            for g in games {
                let exes = serde_json::to_string(&g.executable_names).unwrap_or_default();
                stmt.execute(params![g.slug, g.name, exes])?;
            }
        }
        tx.commit()
    }

    pub fn load_games(&self) -> Vec<CachedGame> {
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare("SELECT slug, name, exe_names FROM games") else {
            return Vec::new();
        };
        let rows = stmt.query_map([], |r| {
            let exes: String = r.get(2)?;
            Ok(CachedGame {
                slug: r.get(0)?,
                name: r.get(1)?,
                executable_names: serde_json::from_str(&exes).unwrap_or_default(),
            })
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

    // --- offline event queue -------------------------------------------------

    pub fn queue_event(&self, event_type: &str, game_slug: &str, ts: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO pending_events (type, game_slug, ts) VALUES (?1, ?2, ?3)",
            params![event_type, game_slug, ts],
        );
    }

    pub fn pending_events(&self) -> Vec<PendingEvent> {
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) =
            conn.prepare("SELECT id, type, game_slug, ts FROM pending_events ORDER BY id")
        else {
            return Vec::new();
        };
        let rows = stmt.query_map([], |r| {
            Ok(PendingEvent {
                id: r.get(0)?,
                event_type: r.get(1)?,
                game_slug: r.get(2)?,
                ts: r.get(3)?,
            })
        });
        match rows {
            Ok(it) => it.flatten().collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn delete_event(&self, id: i64) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute("DELETE FROM pending_events WHERE id = ?1", params![id]);
    }
}
