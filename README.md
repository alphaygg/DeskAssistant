# DeskAssistant

A lightweight desktop AI assistant built with **Tauri**, **Rust**, and **TypeScript**.

DeskAssistant sits quietly on your desktop as a floating widget, allowing you to chat with multiple AI models and execute simple system tasks directly from your desktop.

---

## Features

- **Multiple AI Providers (BYOK):** Use your own API keys for Google Gemini, OpenAI, Claude, or run locally with Ollama.
- **Secure Key Storage:** API keys are stored directly in your OS Keychain (macOS Keychain / Windows Credential Manager) rather than configuration files or plaintext.
- **System Actions:** Capable of running terminal commands and managing local files through safe Rust bindings.
- **Minimalist Floating UI:** Dark glassmorphic floating interface with real-time status indicators and dynamic resizing.

---

## Preview

<!-- Add a screenshot of your app here -->
<p align="center">
  <img src="screenshot.png" alt="DeskAssistant Screenshot" width="500" />
</p>

---

## Tech Stack

- **Backend:** Rust, Tauri v2, `keyring` crate
- **Frontend:** TypeScript, HTML5, CSS
- **Bundler:** Vite

---

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/)
- Tauri build tools for your OS ([Tauri Prerequisites Guide](https://tauri.app/start/prerequisites/))

### Running Locally

1. Clone the repo:
   ```bash
   git clone https://github.com/YOUR_USERNAME/DeskAssistant.git
   cd DeskAssistant
   ```

2. Install dependencies:
   ```bash
   npm install
   ```

3. Start in development mode:
   ```bash
   npm run tauri dev
   ```

4. Build for release:
   ```bash
   npm run tauri build
   ```

---

## License

[MIT](LICENSE)
