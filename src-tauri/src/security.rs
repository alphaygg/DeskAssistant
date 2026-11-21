use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const SERVICE_NAME: &str = "deskassistant";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub active_provider: String,
    pub model_name: String,
    pub ollama_url: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            active_provider: "gemini".to_string(),
            model_name: "gemini-3.8-flash".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyStatus {
    pub provider: String,
    pub has_key: bool,
    pub masked_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub active_provider: String,
    pub model_name: String,
    pub ollama_url: String,
    pub has_openai_key: bool,
    pub has_gemini_key: bool,
    pub has_anthropic_key: bool,
    pub keys: Vec<KeyStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsPayload {
    pub active_provider: String,
    pub model_name: String,
    pub ollama_url: Option<String>,
    pub api_key: Option<String>,
}

pub struct KeyManager;

impl KeyManager {
    fn vault_path(app: &AppHandle) -> Result<PathBuf, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app data directory: {}", e))?;
        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }
        Ok(dir.join(".keys_vault.bin"))
    }

    fn scramble(data: &[u8]) -> Vec<u8> {
        let mask = b"DeskAssistant_Secure_Key_Vault_Salt_v2_2026";
        data.iter()
            .enumerate()
            .map(|(i, b)| b ^ mask[i % mask.len()])
            .collect()
    }

    fn load_vault(app: &AppHandle) -> HashMap<String, String> {
        // Try new scrambled vault (.keys_vault.bin) first
        if let Ok(path) = Self::vault_path(app) {
            if path.exists() {
                if let Ok(bytes) = fs::read(&path) {
                    let unscrambled = Self::scramble(&bytes);
                    if let Ok(content) = String::from_utf8(unscrambled) {
                        if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                            return map;
                        }
                    }
                }
            }
        }

        // Backwards compatibility fallback: check legacy un-scrambled .keys_vault.json
        if let Ok(dir) = app.path().app_data_dir() {
            let legacy_path = dir.join(".keys_vault.json");
            if legacy_path.exists() {
                if let Ok(content) = fs::read_to_string(&legacy_path) {
                    if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                        return map;
                    }
                }
            }
        }

        HashMap::new()
    }

    fn save_to_vault(app: &AppHandle, provider: &str, key: &str) -> Result<(), String> {
        let path = Self::vault_path(app)?;
        let mut map = Self::load_vault(app);
        map.insert(provider.to_string(), key.to_string());
        let data = serde_json::to_string(&map)
            .map_err(|e| format!("Failed to serialize vault: {}", e))?;

        let scrambled = Self::scramble(data.as_bytes());
        fs::write(&path, &scrambled).map_err(|e| format!("Failed to write to vault file: {}", e))?;

        // Restrict file permissions to owner-only read/write (0600) on Unix/macOS
        #[cfg(unix)]
        {
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }

        // Clean up legacy plaintext file if present
        if let Ok(dir) = app.path().app_data_dir() {
            let legacy_path = dir.join(".keys_vault.json");
            if legacy_path.exists() {
                let _ = fs::remove_file(legacy_path);
            }
        }

        Ok(())
    }

    fn delete_from_vault(app: &AppHandle, provider: &str) {
        if let Ok(path) = Self::vault_path(app) {
            let mut map = Self::load_vault(app);
            if map.remove(provider).is_some() {
                if let Ok(data) = serde_json::to_string(&map) {
                    let scrambled = Self::scramble(data.as_bytes());
                    let _ = fs::write(&path, &scrambled);
                    #[cfg(unix)]
                    {
                        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
                    }
                }
            }
        }
    }

    pub fn store_key(app: &AppHandle, provider: &str, key: &str) -> Result<(), String> {
        // 1. Try writing to native OS Keychain
        let keychain_res = Entry::new(SERVICE_NAME, provider)
            .map_err(|e| e.to_string())
            .and_then(|entry| entry.set_password(key).map_err(|e| e.to_string()));

        // 2. Also persist to local protected vault (0600 permissions + scrambled)
        let vault_res = Self::save_to_vault(app, provider, key);

        if keychain_res.is_err() && vault_res.is_err() {
            Err(format!(
                "Failed to store key. Keychain: {:?}, Vault: {:?}",
                keychain_res, vault_res
            ))
        } else {
            Ok(())
        }
    }

    pub fn get_key(app: &AppHandle, provider: &str) -> Result<String, String> {
        // 1. Try reading from native OS Keychain first
        if let Ok(entry) = Entry::new(SERVICE_NAME, provider) {
            if let Ok(pwd) = entry.get_password() {
                let trimmed = pwd.trim().to_string();
                if !trimmed.is_empty() {
                    return Ok(trimmed);
                }
            }
        }

        // 2. Fallback to local secure vault if Keychain fails or is empty
        let map = Self::load_vault(app);
        if let Some(key) = map.get(provider) {
            let trimmed = key.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }

        Err(format!(
            "No registered API key found for {}.",
            provider.to_uppercase()
        ))
    }

    pub fn has_key(app: &AppHandle, provider: &str) -> bool {
        // Silent and fast local check: avoids triggering repetitive OS Keychain dialog pop-ups during UI load
        let vault = Self::load_vault(app);
        if let Some(key) = vault.get(provider) {
            if !key.trim().is_empty() {
                return true;
            }
        }
        false
    }

    pub fn get_masked_preview(app: &AppHandle, provider: &str) -> Option<String> {
        let vault = Self::load_vault(app);
        if let Some(key) = vault.get(provider) {
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                if trimmed.len() <= 8 {
                    return Some("••••••••".to_string());
                }
                let start = &trimmed[..4.min(trimmed.len())];
                let end = &trimmed[trimmed.len().saturating_sub(4)..];
                return Some(format!("{}••••{}", start, end));
            }
        }
        None
    }

    pub fn delete_key(app: &AppHandle, provider: &str) -> Result<(), String> {
        if let Ok(entry) = Entry::new(SERVICE_NAME, provider) {
            let _ = entry.delete_credential();
        }
        Self::delete_from_vault(app, provider);
        Ok(())
    }
}

pub struct SettingsManager;

impl SettingsManager {
    fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app data directory: {}", e))?;
        if !dir.exists() {
            fs::create_dir_all(&dir)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
        Ok(dir.join("settings.json"))
    }

    pub fn load_settings(app: &AppHandle) -> AppSettings {
        if let Ok(path) = Self::config_path(app) {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(settings) = serde_json::from_str::<AppSettings>(&content) {
                        return settings;
                    }
                }
            }
        }
        AppSettings::default()
    }

    pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
        let path = Self::config_path(app)?;
        let data = serde_json::to_string_pretty(settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;
        fs::write(path, data).map_err(|e| format!("Failed to write settings file: {}", e))
    }
}
