//! REST control-plane client (login, refresh, games download).
//!
//! Contract: https://github.com/knightsofeternity/kfire-protocol/blob/main/openapi.yaml

use serde::{Deserialize, Serialize};

use crate::db::CachedGame;

#[derive(Debug)]
pub enum ApiError {
    Network(String),
    Server { code: String, message: String },
}

impl ApiError {
    /// Machine-readable error code (for the UI to special-case).
    pub fn code(&self) -> &str {
        match self {
            ApiError::Network(_) => "network",
            ApiError::Server { code, .. } => code,
        }
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
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
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
            let err = resp
                .json::<ServerError>()
                .await
                .unwrap_or_else(|_| ServerError {
                    code: "unknown".into(),
                    message: "unexpected server error".into(),
                });
            Err(ApiError::Server {
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
