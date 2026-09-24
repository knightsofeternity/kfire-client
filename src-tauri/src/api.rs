//! REST control-plane client (login, refresh, games download).
//!
//! Contract: https://github.com/knightsofeternity/kfire-protocol/blob/main/openapi.yaml

use serde::{Deserialize, Serialize};

use crate::db::CachedGame;

#[derive(Debug)]
pub enum ApiError {
    Network(String),
    /// The server answered with a non-2xx `status`. `code` is its JSON error
    /// code, or `unknown` when the body was not one (a proxy's HTML page).
    Server { status: u16, code: String, message: String },
}

impl ApiError {
    /// Machine-readable error code (for the UI to special-case).
    pub fn code(&self) -> &str {
        match self {
            ApiError::Network(_) => "network",
            ApiError::Server { code, .. } => code,
        }
    }

    /// True when the server has definitively refused this refresh token, so
    /// the link is dead. Only the two answers the refresh endpoint gives for
    /// that count: anything else (a proxy's 502 while the server restarts, a
    /// 500, a 429, a Cloudflare 403 page) is transient and must be retried,
    /// never treated as a reason to forget the server.
    pub fn is_definitive_rejection(&self) -> bool {
        matches!(
            self,
            ApiError::Server { status: 401, code, .. } if code == "invalid_refresh_token"
        ) || matches!(
            self,
            ApiError::Server { status: 403, code, .. } if code == "banned"
        )
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Network(e) => write!(f, "network error: {e}"),
            ApiError::Server { message, .. } => write!(f, "{message}"),
        }
    }
}
impl std::error::Error for ApiError {}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

/// Public instance config from `GET /api/v1/config`.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default)]
    pub org_name: String,
}

/// Response to starting a device-pairing flow.
#[derive(Debug, Clone, Deserialize)]
pub struct PairStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_url: String,
    pub interval: u64,
}

/// Response to polling a pairing. `status`: pending | complete | denied | expired.
#[derive(Debug, Clone, Deserialize)]
pub struct PairPoll {
    pub status: String,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ServerError {
    code: String,
    message: String,
}

#[derive(Serialize)]
struct DeviceInfo<'a> {
    device_id: &'a str,
    name: &'a str,
    platform: &'a str,
}

fn platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn device_name() -> String {
    sysinfo::System::host_name().unwrap_or_else(|| "KFIRE Desktop".into())
}

pub struct ApiClient {
    http: reqwest::Client,
    pub base_url: String,
}

impl ApiClient {
    /// `base_url` is the server origin, e.g. `https://kfire.example.org`.
    pub fn new(base_url: &str) -> Self {
        // Trim whitespace BEFORE the trailing slash. A pasted URL with a
        // trailing space/newline is silently stripped by reqwest's WHATWG URL
        // parser (so the REST refresh works), but `ws_url()` concatenates it
        // into the WebSocket URL where http::Uri rejects it with
        // "invalid uri character" - the connection then never establishes.
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim().trim_end_matches('/').to_string(),
        }
    }

    async fn handle<T: serde::de::DeserializeOwned>(
        resp: Result<reqwest::Response, reqwest::Error>,
    ) -> Result<T, ApiError> {
        let resp = resp.map_err(|e| ApiError::Network(e.to_string()))?;
        if resp.status().is_success() {
            resp.json::<T>()
                .await
                .map_err(|e| ApiError::Network(format!("invalid response: {e}")))
        } else {
            let status = resp.status().as_u16();
            let err = resp
                .json::<ServerError>()
                .await
                .unwrap_or_else(|_| ServerError {
                    code: "unknown".into(),
                    message: "unexpected server error".into(),
                });
            Err(ApiError::Server {
                status,
                code: err.code,
                message: err.message,
            })
        }
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
        device_id: &str,
    ) -> Result<TokenPair, ApiError> {
        let body = serde_json::json!({
            "username": username,
            "password": password,
            "device": DeviceInfo { device_id, name: &device_name(), platform: platform() },
        });
        Self::handle(
            self.http
                .post(format!("{}/api/v1/auth/login", self.base_url))
                .json(&body)
                .send()
                .await,
        )
        .await
    }

    /// Starts the browser device-pairing flow.
    pub async fn start_pairing(&self, device_id: &str) -> Result<PairStart, ApiError> {
        let body = serde_json::json!({
            "device_id": device_id,
            "name": device_name(),
            "platform": platform(),
        });
        Self::handle(
            self.http
                .post(format!("{}/api/v1/devices/pair/start", self.base_url))
                .json(&body)
                .send()
                .await,
        )
        .await
    }

    /// Polls a pairing until the user approves it in the browser.
    pub async fn poll_pairing(&self, device_code: &str) -> Result<PairPoll, ApiError> {
        let body = serde_json::json!({ "device_code": device_code });
        Self::handle(
            self.http
                .post(format!("{}/api/v1/devices/pair/poll", self.base_url))
                .json(&body)
                .send()
                .await,
        )
        .await
    }

    /// Public instance config (org name, registration mode). Used to label a
    /// freshly linked server in the UI.
    pub async fn fetch_config(&self) -> Result<ServerConfig, ApiError> {
        Self::handle(
            self.http
                .get(format!("{}/api/v1/config", self.base_url))
                .send()
                .await,
        )
        .await
    }

    pub async fn refresh(
        &self,
        refresh_token: &str,
        device_id: &str,
    ) -> Result<TokenPair, ApiError> {
        let body = serde_json::json!({
            "refresh_token": refresh_token,
            "device_id": device_id,
        });
        Self::handle(
            self.http
                .post(format!("{}/api/v1/auth/refresh", self.base_url))
                .json(&body)
                .send()
                .await,
        )
        .await
    }

    /// Sets the owner's chosen presence status (online | invisible | offline).
    pub async fn set_presence_status(&self, access_token: &str, status: &str) -> Result<(), ApiError> {
        let resp = self
            .http
            .patch(format!("{}/api/v1/users/me", self.base_url))
            .bearer_auth(access_token)
            .json(&serde_json::json!({ "presence_status": status }))
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(ApiError::Server {
                status: resp.status().as_u16(),
                code: "patch_failed".into(),
                message: format!("set presence_status failed: HTTP {}", resp.status()),
            })
        }
    }

    pub async fn logout(&self, access_token: &str) -> Result<(), ApiError> {
        let resp = self
            .http
            .post(format!("{}/api/v1/auth/logout", self.base_url))
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(ApiError::Server {
                status: resp.status().as_u16(),
                code: "logout_failed".into(),
                message: format!("logout failed: HTTP {}", resp.status()),
            })
        }
    }

    pub async fn fetch_games(&self, access_token: &str) -> Result<Vec<CachedGame>, ApiError> {
        #[derive(Deserialize)]
        struct GamesResponse {
            games: Vec<CachedGame>,
        }
        let resp: GamesResponse = Self::handle(
            self.http
                .get(format!("{}/api/v1/games", self.base_url))
                .bearer_auth(access_token)
                .send()
                .await,
        )
        .await?;
        Ok(resp.games)
    }

    /// Converts the HTTP origin to the WebSocket endpoint.
    pub fn ws_url(&self) -> String {
        let ws = if let Some(rest) = self.base_url.strip_prefix("https://") {
            format!("wss://{rest}")
        } else if let Some(rest) = self.base_url.strip_prefix("http://") {
            format!("ws://{rest}")
        } else {
            format!("wss://{}", self.base_url)
        };
        format!("{ws}/ws")
    }
}

#[cfg(test)]
mod tests {
    use super::ApiClient;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Serves exactly one HTTP response on a local port and returns the base
    /// URL to reach it: a real server, so `handle` parses real bytes.
    fn serve_once(status: &str, content_type: &str, body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(response.as_bytes());
        });
        format!("http://{addr}")
    }

    async fn refresh_against(status: &str, content_type: &str, body: &str) -> super::ApiError {
        let api = ApiClient::new(&serve_once(status, content_type, body));
        api.refresh("some-refresh-token", "some-device")
            .await
            .expect_err("a non-2xx refresh must be an error")
    }

    #[tokio::test]
    async fn a_proxy_502_during_a_redeploy_is_not_a_rejection() {
        let e = refresh_against("502 Bad Gateway", "text/html", "<html>502 Bad Gateway</html>").await;
        assert!(!e.is_definitive_rejection(), "a 502 must not unlink the server: {e:?}");
    }

    #[tokio::test]
    async fn a_server_500_is_not_a_rejection() {
        let e = refresh_against(
            "500 Internal Server Error",
            "application/json",
            r#"{"code":"internal","message":"boom"}"#,
        )
        .await;
        assert!(!e.is_definitive_rejection(), "{e:?}");
    }

    #[tokio::test]
    async fn rate_limiting_is_not_a_rejection() {
        let e = refresh_against("429 Too Many Requests", "text/plain", "slow down").await;
        assert!(!e.is_definitive_rejection(), "{e:?}");
    }

    #[tokio::test]
    async fn a_cloudflare_403_page_is_not_a_ban() {
        let e = refresh_against("403 Forbidden", "text/html", "<html>Attention Required</html>").await;
        assert!(!e.is_definitive_rejection(), "{e:?}");
    }

    #[tokio::test]
    async fn an_invalid_refresh_token_is_a_rejection() {
        let e = refresh_against(
            "401 Unauthorized",
            "application/json",
            r#"{"code":"invalid_refresh_token","message":"refresh token is not valid"}"#,
        )
        .await;
        assert!(e.is_definitive_rejection(), "{e:?}");
    }

    #[tokio::test]
    async fn a_ban_is_a_rejection() {
        let e = refresh_against(
            "403 Forbidden",
            "application/json",
            r#"{"code":"banned","message":"this account is banned"}"#,
        )
        .await;
        assert!(e.is_definitive_rejection(), "{e:?}");
    }

    #[test]
    fn new_trims_whitespace_and_trailing_slash() {
        // A pasted URL with surrounding whitespace must not leak into ws_url.
        let c = ApiClient::new("  https://kfire.example.org/  ");
        assert_eq!(c.base_url, "https://kfire.example.org");
        assert_eq!(c.ws_url(), "wss://kfire.example.org/ws");
    }

    #[test]
    fn ws_url_has_no_space_with_trailing_space_input() {
        let c = ApiClient::new("https://kfire.example.org ");
        assert!(!c.ws_url().contains(' '), "ws_url must not contain a space");
    }
}
