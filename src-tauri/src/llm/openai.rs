use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};

use super::{ChatMessage, LlmAdapter};

pub struct OpenAiAdapter {
    client: reqwest::Client,
}

impl OpenAiAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmAdapter for OpenAiAdapter {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        api_key: Option<&str>,
        model: &str,
        base_url: Option<&str>,
    ) -> Result<String, String> {
        let key = api_key.ok_or_else(|| "OpenAI API key not found. Please provide it in Settings.".to_string())?;
        let url = base_url.unwrap_or("https://api.openai.com/v1/chat/completions");

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", key))
                .map_err(|e| format!("Invalid API key header: {}", e))?,
        );
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
            "messages": payload_messages
        });

        let response = self
            .client
            .post(url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Could not reach OpenAI network: {}", e))?;

        let status = response.status();
        let res_text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        if !status.is_success() {
            return Err(format!("OpenAI Error (HTTP {}): {}", status, res_text));
        }

        let parsed: Value = serde_json::from_str(&res_text)
            .map_err(|e| format!("Failed to parse OpenAI JSON response: {}", e))?;

        parsed["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Expected text content not found in OpenAI response.".to_string())
    }
}

