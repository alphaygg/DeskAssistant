import { invoke } from "@tauri-apps/api/core";

interface KeyStatus {
  provider: string;
  has_key: boolean;
  masked_key?: string;
}

interface SettingsResponse {
  active_provider: string;
  model_name: string;
  ollama_url: string;
  has_openai_key: boolean;
  has_gemini_key: boolean;
  has_anthropic_key: boolean;
  keys?: KeyStatus[];
}

interface ActionResult {
  tool: string;
  details: string;
  success: boolean;
}

interface ExecutionOutput {
  message: string;
  actions: ActionResult[];
}

const DEFAULT_MODELS: Record<string, string> = {
  gemini: "gemini-3.8-flash",
  openai: "gpt-5-mini",
  anthropic: "claude-sonnet-5-5",
  ollama: "llama3.2:3b",
};

interface ModelOption {
  value: string;
  label: string;
}

const PROVIDER_MODELS: Record<string, ModelOption[]> = {
  gemini: [
    { value: "gemini-3.8-flash", label: "Gemini 3.8 Flash (Recommended & Latest)" },
    { value: "gemini-3.5-flash-lite", label: "Gemini 3.5 Flash-Lite (Fast & Lightweight)" },
    { value: "gemini-3.8-pro", label: "Gemini 3.8 Pro (Advanced Reasoning)" },
    { value: "custom", label: "⚙ Custom Model Name..." },
  ],
  openai: [
    { value: "gpt-5-mini", label: "GPT-5 Mini (Fast & Modern)" },
    { value: "gpt-5", label: "GPT-5 (Most Capable)" },
    { value: "o3-mini", label: "o3-mini (Reasoning)" },
    { value: "custom", label: "⚙ Custom Model Name..." },
  ],
  anthropic: [
    { value: "claude-sonnet-5-5", label: "Claude Sonnet 5.5 (Recommended)" },
    { value: "claude-haiku-5-5", label: "Claude Haiku 5.5 (Ultra Fast)" },
    { value: "claude-opus-5-5", label: "Claude Opus 5.5 (Deep Reasoning)" },
    { value: "custom", label: "⚙ Custom Model Name..." },
  ],
  ollama: [
    { value: "llama3.2:3b", label: "Llama 3.2 3B (Fast & Lightweight)" },
    { value: "llama3.2:1b", label: "Llama 3.2 1B (Ultra Light)" },
    { value: "llama3:8b", label: "Llama 3 8B (Balanced)" },
    { value: "qwen2.5:3b", label: "Qwen 2.5 3B (Multilingual)" },
    { value: "qwen2.5:7b", label: "Qwen 2.5 7B (Code & Reasoning)" },
    { value: "mistral:7b", label: "Mistral 7B (Classic)" },
    { value: "custom", label: "⚙ Custom Model Name..." },
  ],
};

// DOM Elements - Chat View
const chatView = document.getElementById("chat-view") as HTMLElement | null;
const userInput = document.getElementById("user-input") as HTMLInputElement | null;
const promptForm = document.getElementById("prompt-form") as HTMLFormElement | null;
const messageContent = document.getElementById("message-content") as HTMLElement | null;
const actionFeedback = document.getElementById("action-feedback") as HTMLElement | null;
const statusIndicator = document.getElementById("status-indicator") as HTMLElement | null;

// DOM Elements - Settings View
const settingsView = document.getElementById("settings-view") as HTMLElement | null;
const settingProvider = document.getElementById("setting-provider") as HTMLSelectElement | null;
const settingModelSelect = document.getElementById("setting-model-select") as HTMLSelectElement | null;
const customModelGroup = document.getElementById("custom-model-group") as HTMLElement | null;
const settingModelCustom = document.getElementById("setting-model-custom") as HTMLInputElement | null;
const keysVaultList = document.getElementById("keys-vault-list") as HTMLElement | null;
const apiKeyLabel = document.getElementById("api-key-label") as HTMLElement | null;
const settingApiKey = document.getElementById("setting-api-key") as HTMLInputElement | null;
const apiKeyGroup = document.getElementById("api-key-group") as HTMLElement | null;
const settingOllamaUrl = document.getElementById("setting-ollama-url") as HTMLInputElement | null;
const ollamaUrlGroup = document.getElementById("ollama-url-group") as HTMLElement | null;
const btnSaveSettings = document.getElementById("btn-save-settings") as HTMLButtonElement | null;
const btnCancelSettings = document.getElementById("btn-cancel-settings") as HTMLButtonElement | null;
const settingsStatus = document.getElementById("settings-status") as HTMLElement | null;

// DOM Elements - Window & Navigation Controls
const btnSettings = document.getElementById("btn-settings") as HTMLButtonElement | null;
const btnClose = document.getElementById("btn-close") as HTMLButtonElement | null;
const btnMinimize = document.getElementById("btn-minimize") as HTMLButtonElement | null;
const dragHeader = document.getElementById("drag-header") as HTMLElement | null;
const characterAnchor = document.getElementById("character-anchor") as HTMLElement | null;
const rootContainer = document.querySelector(".assistant-root") as HTMLElement | null;

// State Cache
let currentSettings: SettingsResponse | null = null;
let currentWindowHeight = 0;

// System-language adaptive greeting lookup
const GREETINGS: Record<string, string> = {
  tr: "Merhaba!",
  en: "Hello!",
  es: "¡Hola!",
  de: "Hallo!",
  fr: "Bonjour!",
  it: "Ciao!",
  pt: "Olá!",
  ja: "こんにちは！",
  zh: "你好！",
  ko: "안녕하세요!",
  ru: "Привет!",
  ar: "مرحباً!",
};

function getSystemGreeting(): string {
  const code = (navigator.language || navigator.languages?.[0] || "en")
    .slice(0, 2)
    .toLowerCase();
  return GREETINGS[code] || "Hello!";
}

// Dynamic Window Auto-Resizing (Auto-Resize Pattern)
async function syncWindowHeight() {
  if (!rootContainer) return;
  const rect = rootContainer.getBoundingClientRect();
  const measuredHeight = Math.ceil(rect.height);

  const targetHeight = Math.max(180, measuredHeight);
  if (Math.abs(targetHeight - currentWindowHeight) >= 3) {
    currentWindowHeight = targetHeight;
    try {
      await invoke("set_window_size", {
        width: 380,
        height: targetHeight,
      });
    } catch (err) {
      console.error("Failed to update window size:", err);
    }
  }
}

const resizeObserver = new ResizeObserver(() => {
  syncWindowHeight();
});

if (rootContainer) {
  resizeObserver.observe(rootContainer);
}

// Window Dragging
function setupDraggable(el: HTMLElement | null) {
  if (!el) return;
  el.addEventListener("mousedown", async (e: MouseEvent) => {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement | null;
    if (
      target?.closest("button") ||
      target?.closest("input") ||
      target?.closest("select") ||
      target?.closest(".window-controls") ||
      target?.closest(".input-container") ||
      target?.closest(".settings-field")
    ) {
      return;
    }

    try {
      await invoke("start_drag");
    } catch (err) {
      console.error("Failed to start window drag:", err);
    }
  });
}

setupDraggable(dragHeader);
setupDraggable(characterAnchor);

// Window Buttons
btnClose?.addEventListener("click", () => invoke("close_window"));
btnMinimize?.addEventListener("click", () => invoke("minimize_window"));

// Helper: Escape HTML
function escapeHtml(text: string): string {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

// Update Chat Bubble Message
function updateMessage(text: string) {
  if (messageContent) {
    messageContent.innerHTML = `<p class="agent-text">${escapeHtml(text)}</p>`;
    requestAnimationFrame(syncWindowHeight);
  }
}

// Display Executed OS Actions
function displayActions(actions: ActionResult[]) {
  if (!actionFeedback) return;
  if (!actions || actions.length === 0) {
    actionFeedback.classList.add("hidden");
    actionFeedback.innerHTML = "";
    return;
  }

  const itemsHtml = actions
    .map((act) => {
      const icon = act.success ? "✓" : "✗";
      const badgeClass = act.success ? "action-ok" : "action-err";
      let toolLabel = act.tool;
      if (act.tool === "create_folder") toolLabel = "📁 Folder";
      else if (act.tool === "write_file") toolLabel = "📝 File";
      else if (act.tool === "list_desktop") toolLabel = "📋 Desktop";
      else if (act.tool === "open_url") toolLabel = "🌐 Browser";

      return `<div class="action-item ${badgeClass}">
        <span class="action-badge">${toolLabel} ${icon}</span>
        <span class="action-text">${escapeHtml(act.details)}</span>
      </div>`;
    })
    .join("");

  actionFeedback.innerHTML = itemsHtml;
  actionFeedback.classList.remove("hidden");
  requestAnimationFrame(syncWindowHeight);
}

// View Toggle (Chat <-> Settings)
function toggleSettingsView(open?: boolean) {
  const shouldOpen = open !== undefined ? open : settingsView?.classList.contains("hidden");
  if (shouldOpen) {
    chatView?.classList.add("hidden");
    settingsView?.classList.remove("hidden");
    loadAndApplySettings();
  } else {
    settingsView?.classList.add("hidden");
    chatView?.classList.remove("hidden");
  }
  requestAnimationFrame(syncWindowHeight);
}

btnSettings?.addEventListener("click", () => toggleSettingsView());
btnCancelSettings?.addEventListener("click", () => toggleSettingsView(false));

const PROVIDER_NAMES: Record<string, string> = {
  gemini: "Google Gemini",
  openai: "OpenAI (ChatGPT)",
  anthropic: "Anthropic (Claude)",
  ollama: "Ollama (Local LLM)",
};

// Render Saved API Keys Vault List
function renderKeysVaultList() {
  if (!keysVaultList || !currentSettings) return;

  const currentProvider = settingProvider?.value || currentSettings.active_provider;
  const cloudProviders = ["gemini", "openai", "anthropic"];

  keysVaultList.innerHTML = "";

  for (const prov of cloudProviders) {
    const provName = PROVIDER_NAMES[prov] || prov.toUpperCase();
    const keyInfo = currentSettings.keys?.find((k) => k.provider === prov);
    const hasKey = keyInfo?.has_key ?? false;
    const masked = keyInfo?.masked_key;
    const isSelected = prov === currentProvider;

    const card = document.createElement("div");
    card.className = `vault-key-card ${isSelected ? "active-selected" : ""}`;

    card.innerHTML = `
      <div class="vault-key-info">
        <div class="vault-key-header">
          <span class="vault-provider-name">${escapeHtml(provName)}</span>
          <span class="vault-key-badge ${hasKey ? "saved" : "missing"}">
            ${hasKey ? "Active ✓" : "Not Added"}
          </span>
        </div>
        <div class="vault-key-preview ${hasKey ? "" : "empty"}">
          ${hasKey ? escapeHtml(masked || "••••••••••••••••") : "No key saved"}
        </div>
      </div>
      ${
        hasKey
          ? `<button type="button" class="btn-vault-delete" data-provider="${prov}" title="Delete ${escapeHtml(provName)} key">
              🗑️ Delete
            </button>`
          : `<button type="button" class="btn-vault-add" data-provider="${prov}" title="Add key for ${escapeHtml(provName)}">
              + Add
            </button>`
      }
    `;

    // Click on card selects provider
    card.addEventListener("click", () => {
      if (settingProvider && settingProvider.value !== prov) {
        settingProvider.value = prov;
        settingProvider.dispatchEvent(new Event("change"));
      }
    });

    // Delete Button handler (Inline 2-Step Confirmation without native popups)
    const btnDel = card.querySelector(".btn-vault-delete") as HTMLButtonElement | null;
    let confirmTimer: number | null = null;

    btnDel?.addEventListener("click", async (e) => {
      e.stopPropagation();

      if (!btnDel.classList.contains("confirm-delete")) {
        // Step 1: Switch to visual confirmation state
        btnDel.classList.add("confirm-delete");
        btnDel.innerHTML = `⚠️ Sure?`;
        btnDel.title = `Click again to permanently delete ${escapeHtml(provName)} key`;

        if (confirmTimer) clearTimeout(confirmTimer);
        confirmTimer = window.setTimeout(() => {
          btnDel.classList.remove("confirm-delete");
          btnDel.innerHTML = `🗑️ Delete`;
          btnDel.title = `Delete ${escapeHtml(provName)} key`;
        }, 3500);
        return;
      }

      // Step 2: Confirmed! Execute deletion
      if (confirmTimer) clearTimeout(confirmTimer);
      btnDel.classList.remove("confirm-delete");
      btnDel.disabled = true;
      btnDel.innerHTML = `Deleting...`;

      try {
        await invoke("delete_api_key", { provider: prov });
        if (settingsStatus) {
          settingsStatus.classList.remove("hidden", "error");
          settingsStatus.classList.add("success");
          settingsStatus.textContent = `${provName} key deleted ✓`;
          setTimeout(() => settingsStatus?.classList.add("hidden"), 2000);
        }
        await loadAndApplySettings();
      } catch (err) {
        if (settingsStatus) {
          settingsStatus.classList.remove("hidden", "success");
          settingsStatus.classList.add("error");
          settingsStatus.textContent = `Delete error: ${String(err)}`;
        }
        btnDel.disabled = false;
        btnDel.innerHTML = `🗑️ Delete`;
      }
    });

    // Add Button handler
    const btnAdd = card.querySelector(".btn-vault-add") as HTMLButtonElement | null;
    btnAdd?.addEventListener("click", (e) => {
      e.stopPropagation();
      if (settingProvider) {
        settingProvider.value = prov;
        settingProvider.dispatchEvent(new Event("change"));
      }
      settingApiKey?.focus();
    });

    keysVaultList.appendChild(card);
  }
}

// Update Key Badge according to active provider
function updateKeyBadge(provider: string) {
  if (!currentSettings) return;

  const provName = PROVIDER_NAMES[provider] || provider.toUpperCase();
  if (apiKeyLabel) {
    apiKeyLabel.textContent = `Enter / Update Key for ${provName}`;
  }

  if (provider === "ollama") {
    apiKeyGroup?.classList.add("hidden");
    ollamaUrlGroup?.classList.remove("hidden");
  } else {
    apiKeyGroup?.classList.remove("hidden");
    ollamaUrlGroup?.classList.add("hidden");
  }

  renderKeysVaultList();
}

// Helper to populate model select dropdown
function populateModelSelect(provider: string, selectedModel?: string) {
  if (!settingModelSelect) return;
  const models = PROVIDER_MODELS[provider] || [
    { value: DEFAULT_MODELS[provider] || "", label: DEFAULT_MODELS[provider] || "Default" },
    { value: "custom", label: "⚙ Custom Model Name..." },
  ];

  settingModelSelect.innerHTML = "";
  let matched = false;
  const activeModel = selectedModel || DEFAULT_MODELS[provider] || "";

  for (const item of models) {
    const opt = document.createElement("option");
    opt.value = item.value;
    opt.textContent = item.label;
    if (item.value === activeModel) {
      opt.selected = true;
      matched = true;
    }
    settingModelSelect.appendChild(opt);
  }

  if (!matched && activeModel) {
    settingModelSelect.value = "custom";
    if (customModelGroup) customModelGroup.classList.remove("hidden");
    if (settingModelCustom) settingModelCustom.value = activeModel;
  } else if (settingModelSelect.value === "custom") {
    if (customModelGroup) customModelGroup.classList.remove("hidden");
  } else {
    if (customModelGroup) customModelGroup.classList.add("hidden");
  }
}

// Model select change handler
settingModelSelect?.addEventListener("change", () => {
  if (settingModelSelect.value === "custom") {
    customModelGroup?.classList.remove("hidden");
    settingModelCustom?.focus();
  } else {
    customModelGroup?.classList.add("hidden");
  }
  requestAnimationFrame(syncWindowHeight);
});

// Provider select change handler
settingProvider?.addEventListener("change", () => {
  const provider = settingProvider.value;
  populateModelSelect(provider, DEFAULT_MODELS[provider]);
  if (settingApiKey) settingApiKey.value = "";
  updateKeyBadge(provider);
  requestAnimationFrame(syncWindowHeight);
});

// Load Settings from Rust
async function loadAndApplySettings() {
  try {
    currentSettings = await invoke<SettingsResponse>("load_settings");
    if (settingProvider) settingProvider.value = currentSettings.active_provider;
    populateModelSelect(currentSettings.active_provider, currentSettings.model_name);
    if (settingOllamaUrl) settingOllamaUrl.value = currentSettings.ollama_url;
    if (settingApiKey) settingApiKey.value = "";

    updateKeyBadge(currentSettings.active_provider);
  } catch (err) {
    console.error("Failed to load settings:", err);
  }
}

// Save Settings to Rust
btnSaveSettings?.addEventListener("click", async () => {
  const provider = settingProvider?.value || "gemini";
  let model = "";
  if (settingModelSelect?.value === "custom") {
    model = settingModelCustom?.value.trim() || DEFAULT_MODELS[provider] || "";
  } else {
    model = settingModelSelect?.value || DEFAULT_MODELS[provider] || "";
  }
  const ollamaUrl = settingOllamaUrl?.value.trim() || "http://localhost:11434";
  const apiKey = settingApiKey?.value.trim() || undefined;

  if (settingsStatus) {
    settingsStatus.classList.remove("hidden", "success", "error");
    settingsStatus.textContent = "Saving...";
  }

  try {
    await invoke("save_settings", {
      payload: {
        active_provider: provider,
        model_name: model,
        ollama_url: ollamaUrl,
        api_key: apiKey,
      },
    });

    if (settingApiKey) settingApiKey.value = "";

    if (settingsStatus) {
      settingsStatus.classList.add("success");
      settingsStatus.textContent = "Settings saved to Keychain ✓";
    }

    await loadAndApplySettings();

    setTimeout(() => {
      if (settingsStatus) settingsStatus.classList.add("hidden");
      toggleSettingsView(false);
    }, 900);
  } catch (err) {
    if (settingsStatus) {
      settingsStatus.classList.add("error");
      settingsStatus.textContent = `Error: ${String(err)}`;
    }
  }
});

// Prompt Form Submit (Chat & Agent Execution)
promptForm?.addEventListener("submit", async (e) => {
  e.preventDefault();
  const text = userInput?.value.trim();
  if (!text) return;

  if (userInput) userInput.value = "";

  statusIndicator?.classList.add("busy");
  statusIndicator?.classList.remove("error");
  displayActions([]); // Clear previous actions
  updateMessage(`Working on it... ("${text}")`);

  try {
    const response = await invoke<ExecutionOutput>("send_prompt", { prompt: text });
    updateMessage(response.message);
    displayActions(response.actions);
  } catch (err) {
    statusIndicator?.classList.remove("busy");
    statusIndicator?.classList.add("error");
    updateMessage(`[Error] ${String(err)}`);
  } finally {
    statusIndicator?.classList.remove("busy");
  }
});

// Initialize on DOM load
window.addEventListener("DOMContentLoaded", async () => {
  updateMessage(getSystemGreeting());
  await loadAndApplySettings();
  setTimeout(syncWindowHeight, 50);
});
