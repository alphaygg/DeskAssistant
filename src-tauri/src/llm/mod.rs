use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod anthropic;
pub mod gemini;
pub mod ollama;
pub mod openai;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system", "user", "assistant"
    pub content: String,
}

#[async_trait]
pub trait LlmAdapter: Send + Sync {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        api_key: Option<&str>,
        model: &str,
        base_url: Option<&str>,
    ) -> Result<String, String>;
}

pub fn get_adapter(provider: &str) -> Result<Box<dyn LlmAdapter>, String> {
    match provider.to_lowercase().as_str() {
        "openai" => Ok(Box::new(openai::OpenAiAdapter::new())),
        "gemini" => Ok(Box::new(gemini::GeminiAdapter::new())),
        "anthropic" => Ok(Box::new(anthropic::AnthropicAdapter::new())),
        "ollama" => Ok(Box::new(ollama::OllamaAdapter::new())),
        _ => Err(format!("Unsupported provider: '{}'", provider)),
    }
}

