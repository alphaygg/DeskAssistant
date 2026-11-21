pub mod llm;
pub mod security;
pub mod tools;

use llm::{get_adapter, ChatMessage};
use security::{AppSettings, KeyManager, SettingsManager, SettingsPayload, SettingsResponse};
use tauri::{AppHandle, Window};
use tools::{ExecutionOutput, ToolExecutor};

const AGENT_SYSTEM_PROMPT: &str = include_str!("../../prompts/system_prompt.md");

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! DeskAssistant is ready and listening.", name)
}

#[tauri::command]
fn minimize_window(window: Window) {
    let _ = window.minimize();
}

#[tauri::command]
fn close_window(window: Window) {
    let _ = window.close();
}

#[tauri::command]
fn start_drag(window: Window) {
    let _ = window.start_dragging();
}

#[tauri::command]
fn set_window_size(window: Window, width: f64, height: f64) {
    let _ = window.set_size(tauri::LogicalSize::new(width, height));
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Result<SettingsResponse, String> {
    let settings = SettingsManager::load_settings(&app);
    let providers = ["gemini", "openai", "anthropic"];
    let keys = providers
        .iter()
        .map(|&p| {
            let has_key = KeyManager::has_key(&app, p);
            let masked_key = if has_key {
                KeyManager::get_masked_preview(&app, p)
            } else {
                None
            };
            security::KeyStatus {
                provider: p.to_string(),
                has_key,
                masked_key,
            }
        })
        .collect();

    Ok(SettingsResponse {
        active_provider: settings.active_provider,
        model_name: settings.model_name,
        ollama_url: settings.ollama_url,
        has_openai_key: KeyManager::has_key(&app, "openai"),
        has_gemini_key: KeyManager::has_key(&app, "gemini"),
        has_anthropic_key: KeyManager::has_key(&app, "anthropic"),
        keys,
    })
}

#[tauri::command]
fn save_settings(app: AppHandle, payload: SettingsPayload) -> Result<(), String> {
    let mut raw_ollama = payload.ollama_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    raw_ollama = raw_ollama.trim().to_string();
    if raw_ollama.is_empty() {
        raw_ollama = "http://localhost:11434".to_string();
    }

    // SSRF / URL Validation Defense
    if !raw_ollama.starts_with("http://") && !raw_ollama.starts_with("https://") {
        return Err("Security Boundary: Ollama URL must start with 'http://' or 'https://'".to_string());
    }
    if raw_ollama.contains('@') || raw_ollama.contains('\n') || raw_ollama.contains('\r') {
        return Err("Security Boundary: Forbidden characters detected in Ollama URL.".to_string());
    }

    let settings = AppSettings {
        active_provider: payload.active_provider.clone(),
        model_name: payload.model_name,
        ollama_url: raw_ollama,
    };

    SettingsManager::save_settings(&app, &settings)?;

    if let Some(key) = payload.api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            KeyManager::store_key(&app, &payload.active_provider, trimmed)?;
        }
    }

    Ok(())
}

#[tauri::command]
fn delete_api_key(app: AppHandle, provider: String) -> Result<(), String> {
    KeyManager::delete_key(&app, &provider)
}

#[tauri::command]
async fn send_prompt(app: AppHandle, prompt: String) -> Result<ExecutionOutput, String> {
    let settings = SettingsManager::load_settings(&app);
    let provider = settings.active_provider.to_lowercase();

    let adapter = get_adapter(&provider)?;

    let api_key = if provider == "ollama" {
        None
    } else {
        match KeyManager::get_key(&app, &provider) {
            Ok(k) if !k.trim().is_empty() => Some(k),
            _ => {
                return Err(format!(
                    "No registered API key found for {}. Please enter your API key in ⚙ Settings.",
                    provider.to_uppercase()
                ));
            }
        }
    };

    let base_url = if provider == "ollama" {
        Some(settings.ollama_url.as_str())
    } else {
        None
    };

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: AGENT_SYSTEM_PROMPT.to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: prompt,
        },
    ];

    let raw_response = adapter
        .chat(&messages, api_key.as_deref(), &settings.model_name, base_url)
        .await?;

    // Parse Action or Text
    let (message, actions) = ToolExecutor::parse_llm_response(&raw_response);

    // Physically execute actions if required
    let action_results = if !actions.is_empty() {
        ToolExecutor::execute_actions(&app, &actions)
    } else {
        Vec::new()
    };

    Ok(ExecutionOutput {
        message,
        actions: action_results,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            minimize_window,
            close_window,
            start_drag,
            set_window_size,
            load_settings,
            save_settings,
            delete_api_key,
            send_prompt
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
