//! Non-Claude-CLI providers: Anthropic API and any OpenAI-compatible endpoint (OpenAI, OpenRouter,
//! Groq, Gemini, DeepSeek, Ollama, LM Studio, ...). Replies are streamed back as `api-delta` events.

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

/// `provider` is "claude-cli" (default, also when empty), "anthropic" or "openai" (OpenAI-compatible).
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Settings {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
}

impl Settings {
    pub fn uses_api(&self) -> bool {
        self.provider == "anthropic" || self.provider == "openai"
    }
}

#[derive(Clone)]
pub struct Msg {
    pub role: String,
    pub text: String,
    /// base64 PNGs; only sent for the latest message to keep requests small.
    pub images: Vec<String>,
}

pub async fn stream_reply(app: &AppHandle, s: &Settings, history: &[Msg]) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let anthropic = s.provider == "anthropic";
    let last = history.len().saturating_sub(1);

    let req = if anthropic {
        let base = if s.base_url.trim().is_empty() { "https://api.anthropic.com" } else { s.base_url.trim() };
        let base = base.trim_end_matches('/').trim_end_matches("/v1");
        let msgs: Vec<Value> = history
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let mut c: Vec<Value> = Vec::new();
                if i == last {
                    for img in &m.images {
                        c.push(json!({ "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": img } }));
                    }
                }
                c.push(json!({ "type": "text", "text": m.text }));
                json!({ "role": m.role, "content": c })
            })
            .collect();
        let mut body = json!({ "model": s.model, "max_tokens": 4096, "stream": true, "messages": msgs });
        if !s.system_prompt.trim().is_empty() {
            body["system"] = json!(s.system_prompt);
        }
        client
            .post(format!("{base}/v1/messages"))
            .header("x-api-key", &s.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
    } else {
        let base = if s.base_url.trim().is_empty() { "https://api.openai.com/v1" } else { s.base_url.trim() };
        let mut msgs: Vec<Value> = Vec::new();
        if !s.system_prompt.trim().is_empty() {
            msgs.push(json!({ "role": "system", "content": s.system_prompt }));
        }
        for (i, m) in history.iter().enumerate() {
            if i == last && !m.images.is_empty() {
                let mut c = vec![json!({ "type": "text", "text": m.text })];
                for img in &m.images {
                    c.push(json!({ "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{img}") } }));
                }
                msgs.push(json!({ "role": m.role, "content": c }));
            } else {
                // Plain string content: the widest compatibility with local servers.
                msgs.push(json!({ "role": m.role, "content": m.text }));
            }
        }
        let body = json!({ "model": s.model, "stream": true, "messages": msgs });
        let mut r = client.post(format!("{}/chat/completions", base.trim_end_matches('/'))).json(&body);
        if !s.api_key.trim().is_empty() {
            r = r.bearer_auth(s.api_key.trim());
        }
        r
    };

    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let st = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("{st}: {}", text.chars().take(300).collect::<String>()));
    }

    // Both APIs stream server-sent events: lines of `data: {json}`.
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    let mut full = String::new();
    while let Some(chunk) = stream.next().await {
        buf.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line);
            let Some(data) = line.trim().strip_prefix("data:") else { continue };
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
            if let Some(e) = v["error"]["message"].as_str() {
                return Err(e.to_string());
            }
            let piece = if anthropic { v["delta"]["text"].as_str() } else { v["choices"][0]["delta"]["content"].as_str() };
            if let Some(t) = piece.filter(|t| !t.is_empty()) {
                full.push_str(t);
                let _ = app.emit("api-delta", t);
            }
        }
    }
    Ok(full)
}
