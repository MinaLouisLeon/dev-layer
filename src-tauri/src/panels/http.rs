//! The HTTP panel's backend — the piece that replaces reaching for Postman on
//! a quick call.
//!
//! Deliberately thin: build a request, send it, hand back everything about the
//! response including how long it took. No collections, no environments, no
//! scripting — those are the reasons Postman is heavy.

use std::path::Path;
use std::str::FromStr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use base64::Engine;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Requests kept in history, newest first.
const HISTORY_LIMIT: usize = 25;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<Header>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<Header>,
    /// Text when the body decodes as UTF-8, base64 otherwise.
    pub body: String,
    pub body_is_base64: bool,
    pub size: usize,
    pub elapsed_ms: u64,
    /// Where the request ended up, after redirects.
    pub final_url: String,
}

fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent(concat!("dev-layer/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("could not build HTTP client")
    })
}

pub async fn send_request(request: HttpRequest) -> Result<HttpResponse> {
    let method = Method::from_str(&request.method.to_uppercase())
        .map_err(|_| Error::Config(format!("unsupported method {:?}", request.method)))?;

    let mut headers = HeaderMap::new();
    for header in &request.headers {
        let name = header.name.trim();
        if name.is_empty() {
            continue;
        }
        let name = HeaderName::from_str(name)
            .map_err(|_| Error::Config(format!("invalid header name {name:?}")))?;
        let value = HeaderValue::from_str(&header.value)
            .map_err(|_| Error::Config(format!("invalid value for header {name}")))?;
        headers.insert(name, value);
    }

    let timeout = request
        .timeout_ms
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_TIMEOUT);

    let mut builder = client()
        .request(method, &request.url)
        .headers(headers)
        .timeout(timeout);
    if let Some(body) = request.body.filter(|b| !b.is_empty()) {
        builder = builder.body(body);
    }

    // Timed around the whole exchange, including reading the body — that is
    // the number a developer actually cares about.
    let started = Instant::now();
    let response = builder
        .send()
        .await
        .map_err(|e| Error::Platform(format!("request failed: {e}")))?;

    let status = response.status();
    let final_url = response.url().to_string();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| Header {
            name: name.to_string(),
            value: value.to_str().unwrap_or("<binary>").to_string(),
        })
        .collect();

    let bytes = response
        .bytes()
        .await
        .map_err(|e| Error::Platform(format!("could not read body: {e}")))?;
    let elapsed_ms = started.elapsed().as_millis() as u64;

    let (body, body_is_base64) = match std::str::from_utf8(&bytes) {
        Ok(text) => (text.to_string(), false),
        Err(_) => (
            base64::engine::general_purpose::STANDARD.encode(&bytes),
            true,
        ),
    };

    Ok(HttpResponse {
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("").to_string(),
        headers,
        body,
        body_is_base64,
        size: bytes.len(),
        elapsed_ms,
        final_url,
    })
}

/// Recent requests, so the panel is useful on the second call as well as the
/// first. Stored beside the config; deliberately capped and never includes
/// response bodies.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RequestHistory {
    pub entries: Vec<HttpRequest>,
}

impl RequestHistory {
    fn path_in(dir: &Path) -> std::path::PathBuf {
        dir.join("requests.json")
    }

    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(Self::path_in(dir))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// Adds a request, de-duplicating by method+URL so re-running a call moves
    /// it to the top instead of filling the list.
    pub fn record(&mut self, request: &HttpRequest) {
        self.entries
            .retain(|e| !(e.method == request.method && e.url == request.url));
        self.entries.insert(0, request.clone());
        self.entries.truncate(HISTORY_LIMIT);
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir).map_err(|e| Error::Config(e.to_string()))?;
        let raw = serde_json::to_string_pretty(self).map_err(|e| Error::Config(e.to_string()))?;
        std::fs::write(Self::path_in(dir), raw).map_err(|e| Error::Config(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: &str, url: &str) -> HttpRequest {
        HttpRequest {
            method: method.into(),
            url: url.into(),
            headers: Vec::new(),
            body: None,
            timeout_ms: None,
        }
    }

    #[test]
    fn history_moves_a_repeated_request_to_the_top_without_duplicating() {
        let mut history = RequestHistory::default();
        history.record(&request("GET", "http://localhost:3000/health"));
        history.record(&request("POST", "http://localhost:3000/login"));
        history.record(&request("GET", "http://localhost:3000/health"));

        assert_eq!(history.entries.len(), 2);
        assert_eq!(history.entries[0].url, "http://localhost:3000/health");
        assert_eq!(history.entries[1].method, "POST");
    }

    #[test]
    fn history_is_capped() {
        let mut history = RequestHistory::default();
        for i in 0..40 {
            history.record(&request("GET", &format!("http://localhost/{i}")));
        }
        assert_eq!(history.entries.len(), HISTORY_LIMIT);
        assert_eq!(history.entries[0].url, "http://localhost/39");
    }
}
