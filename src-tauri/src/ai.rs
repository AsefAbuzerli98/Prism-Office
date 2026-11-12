//! AI service daemon (X-02): Claude Messages API over HTTPS with streaming.
//!
//! The Anthropic API key never reaches the webview: it is read from
//! `ANTHROPIC_API_KEY` or from `ai.json` in the app config dir (written with
//! owner-only permissions) and used only here. Rust has no official
//! Anthropic SDK, so requests go over raw HTTP (`ureq`) to `POST /v1/messages`.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::events::EventBus;
use crate::services::Service;

pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";
/// Server-side refusal fallback (`fallbacks: "default"`).
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AiConfig {
    /// Stored key (empty = use `ANTHROPIC_API_KEY`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    pub model: String,
    /// `low` | `medium` | `high` | `xhigh` | `max`.
    pub effort: String,
}

/// What the UI may see about the configuration (never the key itself).
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub configured: bool,
    pub key_source: &'static str,
    pub model: String,
    pub effort: String,
}

pub struct AiStore {
    path: Option<PathBuf>,
    inner: Mutex<AiConfig>,
}

impl AiStore {
    pub fn load(path: Option<PathBuf>) -> Self {
        let inner: AiConfig = crate::store::load(path.as_deref());
        Self {
            path,
            inner: Mutex::new(inner),
        }
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
        let c = self.inner.lock().unwrap();
        if !c.api_key.is_empty() {
            return Some((c.api_key.clone(), "settings"));
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
        }
    }

    /// Update settings; `api_key: None` keeps the stored key, `Some("")` clears it.
    pub fn update(
        &self,
        api_key: Option<String>,
        model: Option<String>,
        effort: Option<String>,
    ) -> std::io::Result<AiStatus> {
        {
            let mut c = self.inner.lock().unwrap();
            if let Some(k) = api_key {
                c.api_key = k.trim().to_string();
            }
            if let Some(m) = model.filter(|m| !m.trim().is_empty()) {
                c.model = m.trim().to_string();
            }
            if let Some(e) =
                effort.filter(|e| ["low", "medium", "high", "xhigh", "max"].contains(&e.as_str()))
            {
                c.effort = e;
            }
            crate::store::save(self.path.as_ref(), &*c)?;
            #[cfg(unix)]
            if let Some(p) = &self.path {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600));
            }
        }
        Ok(self.status())
    }
}

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
    /// Correlates streamed `aiChunk` events with the request.
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
    #[error("Anthropic API error {status}: {message}")]
    Api { status: u16, message: String },
    #[error("network error: {0}")]
    Network(String),
    #[error("unexpected response: {0}")]
    Protocol(String),
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

/// Accumulate a Messages SSE stream. `on_text` receives each text delta.
pub fn read_stream(
    reader: impl BufRead,
    mut on_text: impl FnMut(&str),
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
        let v: Value = match serde_json::from_str(data.trim()) {
            Ok(v) => v,
            Err(_) => continue,
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
                return Err(AiError::Api {
                    status: 0,
                    message: v["error"]["message"]
                        .as_str()
                        .unwrap_or("stream error")
                        .to_string(),
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

pub fn complete(
    store: &AiStore,
    req: &AiRequest,
    on_text: impl FnMut(&str),
) -> Result<AiResponse, AiError> {
    let (key, _) = store.key().ok_or(AiError::NoKey)?;
    let cfg = store.config();
    let body = request_body(req, &cfg);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .proxy(ureq::Proxy::try_from_env())
        .build()
        .into();
    let resp = agent
        .post(API_URL)
        .header("x-api-key", &key)
        .header("anthropic-version", API_VERSION)
        .header("anthropic-beta", FALLBACK_BETA)
        .header("content-type", "application/json")
        .send(body.to_string())
        .map_err(|e| AiError::Network(e.to_string()))?;
    let status = resp.status().as_u16();
    let mut body = resp.into_body();
    if status >= 400 {
        let text = body.read_to_string().unwrap_or_default();
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
            .unwrap_or(text);
        return Err(AiError::Api { status, message });
    }
    read_stream(BufReader::new(body.as_reader()), on_text)
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

    #[test]
    fn body_shape() {
        let req = AiRequest {
            request_id: "r1".into(),
            system: "sys".into(),
            prompt: "hello".into(),
            images: vec![ImageInput {
                media_type: "image/png".into(),
                data: "AAA".into(),
            }],
            effort: None,
            max_tokens: Some(1000),
        };
        let cfg = AiConfig {
            api_key: String::new(),
            model: DEFAULT_MODEL.into(),
            effort: "high".into(),
        };
        let b = request_body(&req, &cfg);
        assert_eq!(b["model"], "claude-opus-5-5");
        assert_eq!(b["stream"], true);
        assert_eq!(b["fallbacks"], "default");
        assert_eq!(b["output_config"]["effort"], "high");
        assert!(
            b.get("thinking").is_none(),
            "thinking can't be disabled on this model; leave it out"
        );
        assert_eq!(b["messages"][0]["content"][0]["type"], "image");
        assert_eq!(b["messages"][0]["content"][1]["text"], "hello");
    }

    #[test]
    fn parses_sse_stream() {
        let sse = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\"}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n";
        let mut chunks = Vec::new();
        let r = read_stream(sse.as_bytes(), |t| chunks.push(t.to_string())).unwrap();
        assert_eq!(r.text, "Hello");
        assert_eq!(chunks, vec!["Hel", "lo"]);
        assert_eq!(
            (r.stop_reason.as_str(), r.model.as_str()),
            ("end_turn", "claude-opus-5-5")
        );

        let refused = "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"refusal\",\"stop_details\":{\"type\":\"refusal\",\"category\":\"cyber\"}}}\n";
        let r = read_stream(refused.as_bytes(), |_| {}).unwrap();
        assert_eq!(r.refusal.as_deref(), Some("cyber"));
        let err = "data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n";
        assert!(matches!(
            read_stream(err.as_bytes(), |_| {}),
            Err(AiError::Api { .. })
        ));
    }

    #[test]
    fn store_keeps_key_private_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("ai.json");
        let s = AiStore::load(Some(p.clone()));
        let st = s
            .update(Some(" sk-test ".into()), None, Some("medium".into()))
            .unwrap();
        assert!(st.configured);
        assert_eq!(st.key_source, "settings");
        assert_eq!(st.effort, "medium");
        let json = serde_json::to_string(&st).unwrap();
        assert!(!json.contains("sk-test"), "status must not leak the key");
        let again = AiStore::load(Some(p.clone()));
        assert_eq!(again.config().api_key, "sk-test");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        again.update(Some(String::new()), None, None).unwrap();
        assert_eq!(AiStore::load(Some(p)).config().api_key, "");
    }
}
