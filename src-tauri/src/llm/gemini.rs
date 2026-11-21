use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::{json, Value};
use std::time::Duration;

use super::{ChatMessage, LlmAdapter};

pub struct GeminiAdapter {
    client: reqwest::Client,
}

impl GeminiAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmAdapter for GeminiAdapter {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        api_key: Option<&str>,
        model: &str,
        base_url: Option<&str>,
    ) -> Result<String, String> {
        let key = api_key.ok_or_else(|| "Gemini API key not found. Please provide it in Settings.".to_string())?;

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let mut system_text = String::new();
        let mut contents: Vec<Value> = Vec::new();

        for m in messages {
            if m.role == "system" {
                if !system_text.is_empty() {
                    system_text.push('\n');
                }
                system_text.push_str(&m.content);
            } else {
                let role = if m.role == "assistant" { "model" } else { "user" };
                contents.push(json!({
                    "role": role,
                    "parts": [{ "text": m.content }]
                }));
            }
        }

        let mut body = json!({
            "contents": contents
        });

        if !system_text.is_empty() {
            body["systemInstruction"] = json!({
                "parts": [{ "text": system_text }]
            });
        }

        // Eğer kullanıcı bir model seçtiyse veya model 404/503 verirse denenecek güncel adaylar
        let mut candidate_models = vec![model];
        if !candidate_models.contains(&"gemini-3.8-flash") {
            candidate_models.push("gemini-3.8-flash");
        }
        if !candidate_models.contains(&"gemini-3.5-flash-lite") {
            candidate_models.push("gemini-3.5-flash-lite");
        }

        let mut last_error = String::new();

        // Otomatik Yeniden Deneme ve Model Fallback Döngüsü
        for (attempt, current_model) in candidate_models.iter().enumerate() {
            let url = match base_url {
                Some(u) => format!("{}:generateContent?key={}", u, key),
                None => format!(
                    "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                    current_model, key
                ),
            };

            let send_res = self
                .client
                .post(&url)
                .headers(headers.clone())
                .json(&body)
                .send()
                .await;

            let response = match send_res {
                Ok(r) => r,
                Err(e) => {
                    last_error = format!("Could not reach Gemini network: {}", e);
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
            };

            let status = response.status();
            let res_text = response.text().await.unwrap_or_default();

            if status.is_success() {
                let parsed: Value = serde_json::from_str(&res_text)
                    .map_err(|e| format!("Failed to parse Gemini JSON response: {}", e))?;

                return parsed["candidates"][0]["content"]["parts"][0]["text"]
                    .as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| "Expected text content not found in Gemini response.".to_string());
            }

            // 404 (Model no longer available), 503 (High demand) or 429 (Rate limit)
            if status.as_u16() == 404 || status.as_u16() == 503 || status.as_u16() == 429 {
                last_error = format!(
                    "Model '{}' unavailable (HTTP {}): {}. Trying fallback model...",
                    current_model,
                    status.as_u16(),
                    res_text
                );
                // Backoff and proceed to fallback model
                std::thread::sleep(Duration::from_millis(800 * (attempt as u64 + 1)));
                continue;
            }

            // Client errors (400 Invalid Argument, 401 Unauthorized API Key)
            return Err(format!("Gemini Error (HTTP {}): {}", status, res_text));
        }

        Err(format!(
            "{} Please try again in a few moments or change the model in Settings.",
            last_error
        ))
    }
}
