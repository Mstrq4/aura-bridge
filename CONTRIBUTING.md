# Contributing to Claude Switch Pro

Thank you for helping improve **Claude Switch Pro**!

## Architecture Overview
- `crates/bridge-core/`: Cross-platform Rust core containing the Anthropic Messages <-> OpenAI Responses translator, MCP schema sanitizer, rate-limit shield, telemetry engine, CLI/app syncers, and GitHub Releases updater.
- `crates/setup-wizard/`: Custom native WebView2 multi-step Aura installation wizard for Windows (`x86_64` & `aarch64`).
- `src-tauri/`: Tauri v2 desktop host with embedded Aura UI, system tray, OS notifications, single-instance guard, and auto-updater integration.
- `ui/`: Pure HTML5/CSS3/ES-Module frontend bundled directly inside the desktop app.

## Local Development

```bash
npm install
cargo test --workspace
npx tauri dev
```

## Building Release Bundles

```bash
npx tauri build
```

## Pull Requests
1. Run `cargo test --workspace` and `cargo clippy --workspace`.
2. Verify both RTL (`ar`) and LTR (`en`) and Light/Dark themes when changing `ui/`.
3. Never commit `.env`, API keys, or private updater signing keys.

