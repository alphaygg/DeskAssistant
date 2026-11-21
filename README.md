# DeskAssistant

A lightweight desktop AI assistant built with **Tauri**, **Rust**, and **TypeScript**.

DeskAssistant sits on your desktop as a floating widget, allowing you to interact with AI models and execute basic system tasks.

## Features

- **Multiple AI Providers (BYOK):** Use your own API keys for Google Gemini, OpenAI, Claude, or run local models via Ollama.
- **Secure Key Storage:** API keys are encrypted directly in your OS Keychain (macOS Keychain / Windows Credential Manager) and never stored in plaintext.
- **System Actions:** Execute terminal commands and perform local file operations through safe Rust bindings.
- **Minimalist UI:** Clean, floating dark-themed interface with dynamic window resizing and status indicators.

## Tech Stack

- **Backend:** Rust, Tauri v2, `keyring`
- **Frontend:** TypeScript, HTML, CSS
- **Tooling:** Vite

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/)
- Tauri dependencies for your operating system

### Running Locally

1. Clone the repository:
   ```bash
   git clone https://github.com/YOUR_USERNAME/DeskAssistant.git
   cd DeskAssistant
   ```

2. Install dependencies:
   ```bash
   npm install
   ```

3. Run in development mode:
   ```bash
   npm run tauri dev
   ```

4. Build executable:
   ```bash
   npm run tauri build
   ```
