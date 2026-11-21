use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAction {
    pub tool: String, // "create_folder", "write_file", "list_desktop", "open_url"
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    #[serde(default)]
    pub thought: Option<String>,
    #[serde(default)]
    pub actions: Vec<ToolAction>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub tool: String,
    pub details: String,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOutput {
    pub message: String,
    pub actions: Vec<ActionResult>,
}

pub struct ToolExecutor;

impl ToolExecutor {
    /// Resolves a secure path inside the user's Desktop directory.
    /// Strictly prevents directory traversal attacks ("../") and absolute escapes.
    pub fn resolve_desktop_path(app: &AppHandle, relative_path: &str) -> Result<PathBuf, String> {
        let desktop = app
            .path()
            .desktop_dir()
            .map_err(|e| format!("Desktop directory not found: {}", e))?;

        let cleaned = relative_path
            .trim()
            .trim_start_matches('/')
            .trim_start_matches('\\');

        if cleaned.is_empty() {
            return Err("Target path cannot be empty.".to_string());
        }

        // Strict Path Traversal Defense:
        // Ensure no component is ParentDir (".."), RootDir ("/" or "\\"), or Prefix (Windows drive letters)
        let path_obj = std::path::Path::new(cleaned);
        for component in path_obj.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Err(format!(
                        "Security Boundary: Directory traversal ('..') is not permitted in path '{}'.",
                        relative_path
                    ));
                }
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                    return Err(format!(
                        "Security Boundary: Absolute paths are not permitted in path '{}'.",
                        relative_path
                    ));
                }
                _ => {}
            }
        }

        // Normalized target path
        let target = desktop.join(cleaned);

        // Security check: Target must stay inside the Desktop directory
        if target.starts_with(&desktop) {
            Ok(target)
        } else {
            Err(format!(
                "Security Boundary: Target ('{}') cannot escape the Desktop directory.",
                relative_path
            ))
        }
    }

    /// Create folder (std::fs::create_dir_all)
    pub fn create_folder(app: &AppHandle, path: &str) -> Result<String, String> {
        let target = Self::resolve_desktop_path(app, path)?;
        fs::create_dir_all(&target)
            .map_err(|e| format!("Failed to create folder ({}): {}", target.display(), e))?;
        Ok(format!("Folder created on Desktop: {}", path))
    }

    /// Write text to file (std::fs::write)
    /// Prevents silent overwriting of existing files by creating safe versioned copies.
    pub fn write_file(app: &AppHandle, path: &str, content: &str) -> Result<String, String> {
        let target = Self::resolve_desktop_path(app, path)?;

        // Ensure parent directory exists if nested
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent folder: {}", e))?;
        }

        // Prevent silent destruction of existing files
        let (final_path, version_note) = if target.exists() {
            let parent = target.parent().unwrap_or(&target);
            let stem = target.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
            let ext = target.extension().and_then(|e| e.to_str());

            let mut counter = 1;
            let mut unique_target;
            loop {
                let new_name = match ext {
                    Some(extension) => format!("{} ({}).{}", stem, counter, extension),
                    None => format!("{} ({})", stem, counter),
                };
                unique_target = parent.join(&new_name);
                if !unique_target.exists() {
                    break;
                }
                counter += 1;
            }
            let unique_name = unique_target.file_name().and_then(|n| n.to_str()).unwrap_or(path).to_string();
            (unique_target, format!(" (original preserved, saved as '{}')", unique_name))
        } else {
            (target, String::new())
        };

        fs::write(&final_path, content)
            .map_err(|e| format!("Failed to write file ({}): {}", final_path.display(), e))?;

        Ok(format!(
            "File saved: {}{} ({} bytes)",
            path,
            version_note,
            content.len()
        ))
    }

    /// List Desktop files and folders (std::fs::read_dir)
    pub fn list_desktop(app: &AppHandle) -> Result<String, String> {
        let desktop = app
            .path()
            .desktop_dir()
            .map_err(|e| format!("Desktop directory not found: {}", e))?;

        let entries = fs::read_dir(&desktop)
            .map_err(|e| format!("Failed to read Desktop directory: {}", e))?;

        let mut items = Vec::new();
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                // Filter out hidden files (.DS_Store, etc.)
                if !name.starts_with('.') {
                    let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    let prefix = if is_dir { "[Folder]" } else { "[File]" };
                    items.push(format!("{} {}", prefix, name));
                }
            }
        }

        if items.is_empty() {
            Ok("Desktop is currently empty.".to_string())
        } else {
            Ok(items.join(", "))
        }
    }

    /// Open web URL in user's default browser
    pub fn open_url(url_str: &str) -> Result<String, String> {
        let trimmed = url_str.trim();
        let target = if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            format!("https://{}", trimmed)
        } else {
            trimmed.to_string()
        };

        tauri_plugin_opener::open_url(&target, None::<&str>)
            .map_err(|e| format!("Failed to open browser: {}", e))?;

        Ok(format!("Opened in browser: {}", target))
    }

    /// Parse LLM raw text response into message and structured actions
    pub fn parse_llm_response(raw: &str) -> (String, Vec<ToolAction>) {
        let trimmed = raw.trim();

        // 1. Look for JSON inside markdown blocks (```json ... ``` or ``` ... ```)
        let json_str = if let Some(start) = trimmed.find("```json") {
            let after_start = &trimmed[start + 7..];
            if let Some(end) = after_start.find("```") {
                after_start[..end].trim()
            } else {
                after_start.trim()
            }
        } else if let Some(start) = trimmed.find("```") {
            let after_start = &trimmed[start + 3..];
            if let Some(end) = after_start.find("```") {
                after_start[..end].trim()
            } else {
                after_start.trim()
            }
        } else if trimmed.starts_with('{') && trimmed.ends_with('}') {
            trimmed
        } else {
            ""
        };

        // 2. Attempt to parse JSON into AgentResponse
        if !json_str.is_empty() {
            if let Ok(agent_resp) = serde_json::from_str::<AgentResponse>(json_str) {
                return (agent_resp.message, agent_resp.actions);
            }
        }

        // 3. Fallback to plaintext if not JSON
        (trimmed.to_string(), Vec::new())
    }

    /// Physically executes actions in the OS
    pub fn execute_actions(app: &AppHandle, actions: &[ToolAction]) -> Vec<ActionResult> {
        let mut results = Vec::new();

        for action in actions {
            match action.tool.as_str() {
                "create_folder" => {
                    let path = action.path.as_deref().unwrap_or("NewFolder");
                    match Self::create_folder(app, path) {
                        Ok(msg) => results.push(ActionResult {
                            tool: "create_folder".to_string(),
                            details: msg,
                            success: true,
                        }),
                        Err(err) => results.push(ActionResult {
                            tool: "create_folder".to_string(),
                            details: err,
                            success: false,
                        }),
                    }
                }
                "write_file" => {
                    let path = action.path.as_deref().unwrap_or("file.txt");
                    let content = action.content.as_deref().unwrap_or("");
                    match Self::write_file(app, path, content) {
                        Ok(msg) => results.push(ActionResult {
                            tool: "write_file".to_string(),
                            details: msg,
                            success: true,
                        }),
                        Err(err) => results.push(ActionResult {
                            tool: "write_file".to_string(),
                            details: err,
                            success: false,
                        }),
                    }
                }
                "list_desktop" => match Self::list_desktop(app) {
                    Ok(list) => results.push(ActionResult {
                        tool: "list_desktop".to_string(),
                        details: list,
                        success: true,
                    }),
                    Err(err) => results.push(ActionResult {
                        tool: "list_desktop".to_string(),
                        details: err,
                        success: false,
                    }),
                },
                "open_url" => {
                    let url = action
                        .url
                        .as_deref()
                        .or(action.path.as_deref())
                        .unwrap_or("https://google.com");
                    match Self::open_url(url) {
                        Ok(msg) => results.push(ActionResult {
                            tool: "open_url".to_string(),
                            details: msg,
                            success: true,
                        }),
                        Err(err) => results.push(ActionResult {
                            tool: "open_url".to_string(),
                            details: err,
                            success: false,
                        }),
                    }
                }
                unknown => results.push(ActionResult {
                    tool: unknown.to_string(),
                    details: format!("Unknown tool: '{}'", unknown),
                    success: false,
                }),
            }
        }

        results
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_text() {
        let raw = "Merhaba! Size bugün nasıl yardımcı olabilirim?";
        let (msg, actions) = ToolExecutor::parse_llm_response(raw);
        assert_eq!(msg, raw);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_parse_markdown_json() {
        let raw = r#"İsteğinizi hemen yerine getiriyorum.
```json
{
  "thought": "Masaüstünde Projeler klasörü açıyorum.",
  "actions": [
    { "tool": "create_folder", "path": "Projeler" },
    { "tool": "write_file", "path": "Projeler/notlar.txt", "content": "Harika bir proje" }
  ],
  "message": "Projeler klasörü ve notlar dosyası oluşturuldu!"
}
```"#;
        let (msg, actions) = ToolExecutor::parse_llm_response(raw);
        assert_eq!(msg, "Projeler klasörü ve notlar dosyası oluşturuldu!");
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].tool, "create_folder");
        assert_eq!(actions[0].path.as_deref(), Some("Projeler"));
        assert_eq!(actions[1].tool, "write_file");
        assert_eq!(actions[1].content.as_deref(), Some("Harika bir proje"));
    }

    #[test]
    fn test_parse_open_url() {
        let raw = r#"
```json
{
  "thought": "GitHub açılıyor",
  "actions": [
    { "tool": "open_url", "url": "https://github.com" }
  ],
  "message": "GitHub açılıyor!"
}
```"#;
        let (msg, actions) = ToolExecutor::parse_llm_response(raw);
        assert_eq!(msg, "GitHub açılıyor!");
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].tool, "open_url");
        assert_eq!(actions[0].url.as_deref(), Some("https://github.com"));
    }

    #[test]
    fn test_path_traversal_components() {
        let bad_paths = vec![
            "../secret.txt",
            "test/../../.bashrc",
            "a/b/../../../etc/passwd",
            "/root/file.txt",
        ];
        for bad in bad_paths {
            let path_obj = std::path::Path::new(bad);
            let has_escape = path_obj.components().any(|c| match c {
                std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_) => true,
                _ => false,
            });
            assert!(has_escape, "Failed to catch traversal in: {}", bad);
        }
    }
}
