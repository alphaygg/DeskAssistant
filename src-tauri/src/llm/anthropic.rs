use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::{json, Value};

use super::{ChatMessage, LlmAdapter};

pub struct AnthropicAdapter {
    client: reqwest::Client,
}

impl AnthropicAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmAdapter for AnthropicAdapter {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        api_key: Option<&str>,
        model: &str,
        base_url: Option<&str>,
    ) -> Result<String, String> {
        let key = api_key.ok_or_else(|| "Anthropic API key not found. Please provide it in Settings.".to_string())?;
        let url = base_url.unwrap_or("https://api.anthropic.com/v1/messages");

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(key)
                .map_err(|e| format!("Invalid API key header: {}", e))?,
        );
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let mut system_text = String::new();
        let mut req_messages: Vec<Value> = Vec::new();

        for m in messages {
            if m.role == "system" {
                if !system_text.is_empty() {
                    system_text.push('\n');
                }
                system_text.push_str(&m.content);
            } else {
                let role = if m.role == "assistant" { "assistant" } else { "user" };
                req_messages.push(json!({
                    "role": role,
                    "content": m.content
                }));
            }
        }

        let mut body = json!({
            "model": model,
            "max_tokens": 1024,
            "messages": req_messages
        });

        if !system_text.is_empty() {
            body["system"] = json!(system_text);
        }

        let response = self
            .client
            .post(url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Could not reach Anthropic network: {}", e))?;

        let status = response.status();
        let res_text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        if !status.is_success() {
            return Err(format!("Anthropic Error (HTTP {}): {}", status, res_text));
        }

        let parsed: Value = serde_json::from_str(&res_text)
            .map_err(|e| format!("Failed to parse Anthropic JSON response: {}", e))?;

        parsed["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Expected text content not found in Anthropic response.".to_string())
    }
}

