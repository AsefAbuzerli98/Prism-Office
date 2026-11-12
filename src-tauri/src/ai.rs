//! AI service daemon (X-02): Claude Messages API over HTTPS with streaming.
//!
//! The Anthropic API key never reaches the webview. It lives in the OS
//! keychain (Windows Credential Manager, macOS Keychain, Secret Service),
//! falling back to an owner-only `ai.json` when no keychain is available, or
//! comes from `ANTHROPIC_API_KEY`. Rust has no official Anthropic SDK, so
//! requests go over raw HTTP (`ureq`) to `POST /v1/messages` behind the
//! [`AiProvider`] trait, with retries and backoff for rate limits and overload.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::events::EventBus;
use crate::services::Service;

pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";
/// Server-side refusal fallback (`fallbacks: "default"`).
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
pub const MAX_RETRIES: u32 = 3;

// ---- configuration & secrets --------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AiConfig {
    /// Fallback key storage, used only when no OS keychain is available.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    pub model: String,
    /// `low` | `medium` | `high` | `xhigh` | `max`.
    pub effort: String,
    /// Privacy switch: when false, no document content leaves the machine.
    pub enabled: bool,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: String::new(),
            effort: String::new(),
            enabled: true,
        }
    }
}

/// What the UI may see about the configuration (never the key itself).
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub configured: bool,
    /// `keychain`, `file`, `environment` or `none`.
    pub key_source: &'static str,
    pub model: String,
    pub effort: String,
    pub enabled: bool,
    pub provider: &'static str,
}

/// Where the API key is kept.
pub trait SecretStore: Send + Sync {
    fn get(&self) -> Option<String>;
    /// `None` deletes. Returns false when the backend is unavailable.
    fn set(&self, value: Option<&str>) -> bool;
    fn name(&self) -> &'static str;
}

/// OS keychain: Windows Credential Manager, macOS Keychain, Secret Service on Linux.
pub struct KeychainSecret;

impl KeychainSecret {
    fn entry() -> Option<keyring::Entry> {
        keyring::Entry::new("app.prism.office", "anthropic-api-key").ok()
    }
}

impl SecretStore for KeychainSecret {
    fn get(&self) -> Option<String> {
        Self::entry()?.get_password().ok().filter(|k| !k.is_empty())
    }
    fn set(&self, value: Option<&str>) -> bool {
        let Some(e) = Self::entry() else { return false };
        match value {
            Some(v) => e.set_password(v).is_ok() && self.get().as_deref() == Some(v),
            None => matches!(e.delete_credential(), Ok(()) | Err(keyring::Error::NoEntry)),
        }
    }
    fn name(&self) -> &'static str {
        "keychain"
    }
}

/// In-memory secret store (tests); `unavailable()` simulates a missing keychain.
#[derive(Default)]
pub struct MemorySecret(Mutex<Option<String>>, bool);

impl MemorySecret {
    pub fn unavailable() -> Self {
        Self(Mutex::new(None), true)
    }
}

impl SecretStore for MemorySecret {
    fn get(&self) -> Option<String> {
        self.0.lock().unwrap().clone()
    }
    fn set(&self, value: Option<&str>) -> bool {
        if self.1 {
            return false;
        }
        *self.0.lock().unwrap() = value.map(str::to_owned);
        true
    }
    fn name(&self) -> &'static str {
        "keychain"
    }
}

pub struct AiStore {
    path: Option<PathBuf>,
    inner: Mutex<AiConfig>,
    secret: Box<dyn SecretStore>,
}

impl AiStore {
    pub fn load(path: Option<PathBuf>) -> Self {
        Self::with_secret(path, Box::new(KeychainSecret))
    }

    pub fn with_secret(path: Option<PathBuf>, secret: Box<dyn SecretStore>) -> Self {
        let inner: AiConfig = crate::store::load(path.as_deref());
        let store = Self {
            path,
            inner: Mutex::new(inner),
            secret,
        };
        // Migrate a key kept in ai.json into the keychain when one is available.
        let legacy = store.inner.lock().unwrap().api_key.clone();
        if !legacy.is_empty() && store.secret.set(Some(&legacy)) {
            store.inner.lock().unwrap().api_key.clear();
            let _ = store.persist();
        }
        store
    }

    fn persist(&self) -> std::io::Result<()> {
        let c = self.inner.lock().unwrap().clone();
        crate::store::save(self.path.as_ref(), &c)?;
        #[cfg(unix)]
        if let Some(p) = &self.path {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn config(&self) -> AiConfig {
        let mut c = self.inner.lock().unwrap().clone();
        if c.model.is_empty() {
            c.model = DEFAULT_MODEL.into();
        }
        if c.effort.is_empty() {
            c.effort = "high".into();
        }
        c
    }

    fn key(&self) -> Option<(String, &'static str)> {
        if let Some(k) = self.secret.get() {
            return Some((k, self.secret.name()));
        }
        let file = self.inner.lock().unwrap().api_key.clone();
        if !file.is_empty() {
            return Some((file, "file"));
        }
        std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|k| !k.is_empty())
            .map(|k| (k, "environment"))
    }

    pub fn status(&self) -> AiStatus {
        let c = self.config();
        let key = self.key();
        AiStatus {
            configured: key.is_some(),
            key_source: key.map(|k| k.1).unwrap_or("none"),
            model: c.model,
            effort: c.effort,
            enabled: c.enabled,
            provider: AnthropicProvider.name(),
        }
    }

    /// Update settings; `api_key: None` keeps the key, `Some("")` clears it.
    pub fn update(
        &self,
        api_key: Option<String>,
        model: Option<String>,
        effort: Option<String>,
        enabled: Option<bool>,
    ) -> std::io::Result<AiStatus> {
        if let Some(k) = api_key {
            let k = k.trim().to_string();
            let value = (!k.is_empty()).then_some(k.as_str());
            if self.secret.set(value) {
                self.inner.lock().unwrap().api_key.clear();
            } else {
                // No keychain: fall back to the owner-only config file.
                self.inner.lock().unwrap().api_key = k.clone();
            }
        }
        {
            let mut c = self.inner.lock().unwrap();
            if let Some(m) = model.filter(|m| !m.trim().is_empty()) {
                c.model = m.trim().to_string();
            }
            if let Some(e) =
                effort.filter(|e| ["low", "medium", "high", "xhigh", "max"].contains(&e.as_str()))
            {
                c.effort = e;
            }
            if let Some(en) = enabled {
                c.enabled = en;
            }
        }
        self.persist()?;
        Ok(self.status())
    }
}

// ---- requests -------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInput {
    pub media_type: String,
    /// Base64 bytes.
    pub data: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRequest {
    /// Correlates streamed chunk events with the request.
    pub request_id: String,
    pub system: String,
    pub prompt: String,
    #[serde(default)]
    pub images: Vec<ImageInput>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiResponse {
    pub text: String,
    pub stop_reason: String,
    pub model: String,
    /// Refusal category when the whole fallback chain declined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("No Anthropic API key configured. Add one in AI settings or set ANTHROPIC_API_KEY.")]
    NoKey,
    #[error("AI features are turned off in AI settings.")]
    Disabled,
    #[error("Anthropic API error {status}: {message}")]
    Api {
        status: u16,
        message: String,
        /// Seconds from a `retry-after` header.
        retry_after: Option<u64>,
    },
    #[error("network error: {0}")]
    Network(String),
}

impl Serialize for AiError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// JSON body for a streaming Messages request.
pub fn request_body(req: &AiRequest, cfg: &AiConfig) -> Value {
    let mut content: Vec<Value> = req
        .images
        .iter()
        .map(|i| json!({ "type": "image", "source": { "type": "base64", "media_type": i.media_type, "data": i.data } }))
        .collect();
    content.push(json!({ "type": "text", "text": req.prompt }));
    json!({
        "model": cfg.model,
        "max_tokens": req.max_tokens.unwrap_or(64000),
        "stream": true,
        "system": req.system,
        "messages": [{ "role": "user", "content": content }],
        // Thinking is always on for this model; depth is controlled by effort.
        "output_config": { "effort": req.effort.clone().unwrap_or_else(|| cfg.effort.clone()) },
        "fallbacks": "default",
    })
}

/// Accumulate a Messages SSE stream; `on_text` receives each text delta.
pub fn read_stream(
    reader: impl BufRead,
    on_text: &mut dyn FnMut(&str),
) -> Result<AiResponse, AiError> {
    let mut out = AiResponse {
        text: String::new(),
        stop_reason: String::new(),
        model: String::new(),
        refusal: None,
    };
    for line in reader.lines() {
        let line = line.map_err(|e| AiError::Network(e.to_string()))?;
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(data.trim()) else {
            continue;
        };
        match v["type"].as_str() {
            Some("message_start") => {
                if let Some(m) = v["message"]["model"].as_str() {
                    out.model = m.to_string();
                }
            }
            Some("content_block_delta") if v["delta"]["type"] == "text_delta" => {
                if let Some(t) = v["delta"]["text"].as_str() {
                    out.text.push_str(t);
                    on_text(t);
                }
            }
            Some("message_delta") => {
                if let Some(r) = v["delta"]["stop_reason"].as_str() {
                    out.stop_reason = r.to_string();
                }
                if let Some(c) = v["delta"]["stop_details"]["category"].as_str() {
                    out.refusal = Some(c.to_string());
                }
            }
            Some("error") => {
                // Mid-stream overload/api errors are retryable like 529/500.
                let status = match v["error"]["type"].as_str().unwrap_or("") {
                    "overloaded_error" => 529,
                    "rate_limit_error" => 429,
                    "api_error" => 500,
                    _ => 0,
                };
                let message = v["error"]["message"]
                    .as_str()
                    .unwrap_or("stream error")
                    .to_string();
                return Err(AiError::Api {
                    status,
                    message,
                    retry_after: None,
                });
            }
            _ => {}
        }
    }
    if out.stop_reason == "refusal" && out.refusal.is_none() {
        out.refusal = Some("unspecified".into());
    }
    Ok(out)
}

// ---- providers & retries ----------------------------------------------------------------

/// A chat model backend; the default sends Claude Messages API requests.
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &'static str;
    /// One attempt; streamed text goes to `on_text`.
    fn send(
        &self,
        body: &Value,
        key: &str,
        on_text: &mut dyn FnMut(&str),
    ) -> Result<AiResponse, AiError>;
}

pub struct AnthropicProvider;

impl AiProvider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn send(
        &self,
        body: &Value,
        key: &str,
        on_text: &mut dyn FnMut(&str),
    ) -> Result<AiResponse, AiError> {
        #[cfg(any(windows, target_os = "macos"))]
        let tls = ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::NativeTls)
            .build();
        #[cfg(not(any(windows, target_os = "macos")))]
        let tls = ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::Rustls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .tls_config(tls)
            .http_status_as_error(false)
            .proxy(ureq::Proxy::try_from_env())
            .build()
            .into();
        let resp = agent
            .post(API_URL)
            .header("x-api-key", key)
            .header("anthropic-version", API_VERSION)
            .header("anthropic-beta", FALLBACK_BETA)
            .header("content-type", "application/json")
            .send(body.to_string())
            .map_err(|e| AiError::Network(e.to_string()))?;
        let status = resp.status().as_u16();
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let mut body = resp.into_body();
        if status >= 400 {
            let text = body.read_to_string().unwrap_or_default();
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
                .unwrap_or(text);
            return Err(AiError::Api {
                status,
                message,
                retry_after,
            });
        }
        read_stream(BufReader::new(body.as_reader()), on_text)
    }
}

/// Whether a failed attempt may be retried (rate limits, overload, server errors, network).
pub fn retryable(e: &AiError) -> bool {
    match e {
        AiError::Api { status, .. } => {
            matches!(status, 408 | 409 | 429 | 500 | 502 | 503 | 504 | 529)
        }
        AiError::Network(_) => true,
        _ => false,
    }
}

/// Delay before retry `attempt` (0-based): honour `retry-after` (capped at
/// 2 min), else exponential backoff 1s, 2s, 4s … capped at 30s, ±20% jitter.
pub fn retry_delay(attempt: u32, retry_after: Option<u64>, jitter: f64) -> Duration {
    if let Some(s) = retry_after {
        return Duration::from_secs(s.min(120));
    }
    let base = (1u64 << attempt.min(5)).min(30) as f64;
    Duration::from_secs_f64(base * (0.8 + 0.4 * jitter.clamp(0.0, 1.0)))
}

/// Send with retries. Retries happen only before any text has been streamed,
/// so partial output is never duplicated.
pub fn complete_with(
    provider: &dyn AiProvider,
    store: &AiStore,
    req: &AiRequest,
    on_text: &mut dyn FnMut(&str),
    sleep: &mut dyn FnMut(Duration),
) -> Result<AiResponse, AiError> {
    let cfg = store.config();
    if !cfg.enabled {
        return Err(AiError::Disabled);
    }
    let (key, _) = store.key().ok_or(AiError::NoKey)?;
    let body = request_body(req, &cfg);
    let mut attempt = 0;
    loop {
        let mut streamed = false;
        let result = provider.send(&body, &key, &mut |t: &str| {
            streamed = true;
            on_text(t);
        });
        match result {
            Err(e) if retryable(&e) && !streamed && attempt < MAX_RETRIES => {
                let retry_after = if let AiError::Api { retry_after, .. } = &e {
                    *retry_after
                } else {
                    None
                };
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos())
                    .unwrap_or(0);
                sleep(retry_delay(
                    attempt,
                    retry_after,
                    (nanos % 1000) as f64 / 1000.0,
                ));
                attempt += 1;
            }
            other => return other,
        }
    }
}

pub fn complete(
    store: &AiStore,
    req: &AiRequest,
    mut on_text: impl FnMut(&str),
) -> Result<AiResponse, AiError> {
    complete_with(
        &AnthropicProvider,
        store,
        req,
        &mut on_text,
        &mut std::thread::sleep,
    )
}

/// Registry entry: reports whether a key is configured.
pub struct AiService {
    pub configured: bool,
}

impl Service for AiService {
    fn id(&self) -> &'static str {
        "ai"
    }
    fn name(&self) -> &'static str {
        "AI services"
    }
    fn start(&self, _bus: &EventBus) -> Result<Option<String>, String> {
        Ok((!self.configured).then(|| "no API key configured".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> AiStore {
        let s = AiStore::with_secret(None, Box::new(MemorySecret::default()));
        s.update(Some("sk-test".into()), None, None, None).unwrap();
        s
    }

    fn req() -> AiRequest {
        AiRequest {
            request_id: "r".into(),
            system: String::new(),
            prompt: "p".into(),
            images: vec![],
            effort: None,
            max_tokens: None,
        }
    }

    #[test]
    fn body_shape() {
        let mut r = req();
        r.images = vec![ImageInput {
            media_type: "image/png".into(),
            data: "AAA".into(),
        }];
        r.max_tokens = Some(1000);
        let cfg = AiConfig {
            model: DEFAULT_MODEL.into(),
            effort: "high".into(),
            ..Default::default()
        };
        let b = request_body(&r, &cfg);
        assert_eq!(b["model"], "claude-opus-5-5");
        assert_eq!(b["stream"], true);
        assert_eq!(b["fallbacks"], "default");
        assert_eq!(b["output_config"]["effort"], "high");
        assert!(
            b.get("thinking").is_none(),
            "thinking can't be disabled on this model; leave it out"
        );
        assert_eq!(b["messages"][0]["content"][0]["type"], "image");
        assert_eq!(b["messages"][0]["content"][1]["text"], "p");
    }

    #[test]
    fn parses_sse_stream() {
        let sse = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n";
        let mut chunks = Vec::new();
        let r = read_stream(sse.as_bytes(), &mut |t: &str| chunks.push(t.to_string())).unwrap();
        assert_eq!(r.text, "Hello");
        assert_eq!(chunks, vec!["Hel", "lo"]);
        assert_eq!(
            (r.stop_reason.as_str(), r.model.as_str()),
            ("end_turn", "claude-opus-5-5")
        );
        let refused = "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"refusal\",\"stop_details\":{\"type\":\"refusal\",\"category\":\"cyber\"}}}\n";
        assert_eq!(
            read_stream(refused.as_bytes(), &mut |_: &str| {})
                .unwrap()
                .refusal
                .as_deref(),
            Some("cyber")
        );
        let err = "data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n";
        let e = read_stream(err.as_bytes(), &mut |_: &str| {}).unwrap_err();
        assert!(retryable(&e), "mid-stream overload is retryable");
    }

    #[test]
    fn keys_go_to_the_keychain_and_never_into_status_or_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("ai.json");
        std::fs::write(
            &p,
            r#"{"apiKey":"sk-legacy","model":"","effort":"","enabled":true}"#,
        )
        .unwrap();
        let s = AiStore::with_secret(Some(p.clone()), Box::new(MemorySecret::default()));
        assert_eq!(
            (s.status().configured, s.status().key_source),
            (true, "keychain")
        );
        assert!(
            !std::fs::read_to_string(&p).unwrap().contains("sk-legacy"),
            "legacy key migrated out of the file"
        );
        let st = s
            .update(
                Some(" sk-new ".into()),
                None,
                Some("medium".into()),
                Some(false),
            )
            .unwrap();
        assert_eq!((st.effort.as_str(), st.enabled), ("medium", false));
        assert!(!serde_json::to_string(&st).unwrap().contains("sk-new"));
        assert!(!std::fs::read_to_string(&p).unwrap().contains("sk-new"));
        s.update(Some(String::new()), None, None, None).unwrap();
        assert_ne!(s.status().key_source, "keychain");
    }

    #[test]
    fn falls_back_to_an_owner_only_file_without_a_keychain() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("ai.json");
        let s = AiStore::with_secret(Some(p.clone()), Box::new(MemorySecret::unavailable()));
        assert_eq!(
            s.update(Some("sk-file".into()), None, None, None)
                .unwrap()
                .key_source,
            "file"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    struct Flaky {
        fails: Mutex<Vec<AiError>>,
        calls: Mutex<u32>,
        stream_then_fail: bool,
    }
    impl AiProvider for Flaky {
        fn name(&self) -> &'static str {
            "flaky"
        }
        fn send(
            &self,
            _: &Value,
            key: &str,
            on_text: &mut dyn FnMut(&str),
        ) -> Result<AiResponse, AiError> {
            assert_eq!(key, "sk-test");
            *self.calls.lock().unwrap() += 1;
            if let Some(e) = self.fails.lock().unwrap().pop() {
                if self.stream_then_fail {
                    on_text("partial");
                }
                return Err(e);
            }
            on_text("ok");
            Ok(AiResponse {
                text: "ok".into(),
                stop_reason: "end_turn".into(),
                model: DEFAULT_MODEL.into(),
                refusal: None,
            })
        }
    }
    fn flaky(fails: Vec<AiError>) -> Flaky {
        Flaky {
            fails: Mutex::new(fails),
            calls: Mutex::new(0),
            stream_then_fail: false,
        }
    }
    fn api(status: u16, retry_after: Option<u64>) -> AiError {
        AiError::Api {
            status,
            message: "x".into(),
            retry_after,
        }
    }

    #[test]
    fn retries_rate_limits_and_overload_with_backoff() {
        let p = flaky(vec![
            AiError::Network("reset".into()),
            api(529, None),
            api(429, Some(7)),
        ]);
        let mut sleeps = Vec::new();
        let r = complete_with(&p, &store(), &req(), &mut |_| {}, &mut |d| sleeps.push(d)).unwrap();
        assert_eq!(r.text, "ok");
        assert_eq!(*p.calls.lock().unwrap(), 4);
        assert_eq!(sleeps[0], Duration::from_secs(7), "retry-after honoured");
        assert!(
            sleeps[1] >= Duration::from_millis(1600) && sleeps[1] <= Duration::from_millis(2400)
        );
    }

    #[test]
    fn gives_up_after_max_retries_never_retries_client_errors_or_streamed_output() {
        let many = flaky((0..5).map(|_| api(503, Some(0))).collect());
        assert!(complete_with(&many, &store(), &req(), &mut |_| {}, &mut |_| {}).is_err());
        assert_eq!(*many.calls.lock().unwrap(), MAX_RETRIES + 1);
        let bad = flaky(vec![api(400, None)]);
        assert!(complete_with(&bad, &store(), &req(), &mut |_| {}, &mut |_| {}).is_err());
        assert_eq!(*bad.calls.lock().unwrap(), 1);
        let mid = Flaky {
            stream_then_fail: true,
            ..flaky(vec![api(529, None)])
        };
        assert!(complete_with(&mid, &store(), &req(), &mut |_| {}, &mut |_| {}).is_err());
        assert_eq!(
            *mid.calls.lock().unwrap(),
            1,
            "no retry after text was streamed"
        );
    }

    #[test]
    fn privacy_switch_blocks_requests() {
        let s = store();
        s.update(None, None, None, Some(false)).unwrap();
        let p = flaky(vec![]);
        assert!(matches!(
            complete_with(&p, &s, &req(), &mut |_| {}, &mut |_| {}),
            Err(AiError::Disabled)
        ));
        assert_eq!(*p.calls.lock().unwrap(), 0);
    }

    #[test]
    fn backoff_schedule() {
        assert_eq!(retry_delay(0, None, 0.5), Duration::from_secs(1));
        assert_eq!(retry_delay(3, None, 0.5), Duration::from_secs(8));
        assert_eq!(retry_delay(9, None, 0.5), Duration::from_secs(30));
        assert_eq!(retry_delay(0, Some(500), 0.5), Duration::from_secs(120));
    }
}
