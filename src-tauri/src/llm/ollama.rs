use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::{json, Value};

use super::{ChatMessage, LlmAdapter};

pub struct OllamaAdapter {
    client: reqwest::Client,
}

impl OllamaAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmAdapter for OllamaAdapter {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        _api_key: Option<&str>,
        model: &str,
        base_url: Option<&str>,
    ) -> Result<String, String> {
        let base = base_url.unwrap_or("http://localhost:11434").trim();
        if !base.starts_with("http://") && !base.starts_with("https://") {
            return Err("Invalid Ollama URL: must begin with http:// or https://".to_string());
        }
        if base.contains('@') || base.contains('\n') || base.contains('\r') {
            return Err("Invalid Ollama URL: forbidden characters detected.".to_string());
        }
        let url = format!("{}/api/chat", base.trim_end_matches('/'));

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let payload_messages: Vec<Value> = messages
            .iter()
            .map(|m| {
                json!({
                    "role": m.role,
                    "content": m.content
                })
            })
            .collect();

        let body = json!({
            "model": model,
            "messages": payload_messages,
            "stream": false
        });

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                format!(
                    "Could not reach Ollama service ({}). Please make sure Ollama is running: {}",
                    url, e
                )
            })?;

        let status = response.status();
        let res_text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        if !status.is_success() {
            return Err(format!("Ollama Error (HTTP {}): {}", status, res_text));
        }

        let parsed: Value = serde_json::from_str(&res_text)
            .map_err(|e| format!("Failed to parse Ollama JSON response: {}", e))?;

        parsed["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Expected 'message.content' not found in Ollama response.".to_string())
    }
}

