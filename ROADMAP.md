# AuraBridge Engineering Roadmap

<div align="center">

[English README](README.md) · [الدليل بالعربية](README.ar.md) · [Live Website](https://mstrq4.github.io/aura-bridge-web/)

</div>

---

## Phase 1 — Core Rust Protocol & Shield Engine *(Completed)*
- [x] Lossless Anthropic `/v1/messages` <-> OpenAI `/v1/responses` translator (`crates/bridge-core/src/translator.rs`)
- [x] SSE streaming synthesizer supporting thinking blocks, text deltas, tool calls & cached token usage metrics (`stream.rs`)
- [x] 230+ MCP tool JSON Schema sanitizer & deterministic `>64-char` name codec (`sanitizer.rs`)
- [x] Multimodal Vision image, PDF document (`input_file`), Audio block (`input_audio`), and `xhigh`/`max` reasoning mapper
- [x] Pre-Commit SSE `429 Too Many Requests` Shield & heavy-concurrency request gate (`shield.rs`)
- [x] Dynamic router supporting `gpt-6.1-sol`, `gpt-6-astra`, `gpt-5.6-sol` tier mapping (`router.rs`)

## Phase 2 — Native Tauri v2 Desktop Studio & Setup Wizard *(Completed — v2.1.0)*
- [x] Zero-Node unified Tauri v2 desktop binary with synchronous socket binding (`<100ms` boot, zero grey startup flash)
- [x] Full-bleed 6-tab **Aura** spatial UI with per-agent telemetry (`Claude Code`, `Subagents`, `DeepSeek Hermes`, `Claude Desktop`)
- [x] System Tray online/offline status indicator, close-to-tray persistence & single-instance guard
- [x] Visual 4-Step **AuraBridge Setup Wizard** with smart `.env`/`telemetry.json` migration & embedded custom window icons
- [x] Multi-architecture GitHub Releases (`Windows x64/ARM64`, `macOS Apple Silicon/Intel/Universal`) + Minisign auto-updater
- [x] Bilingual documentation (`README.md`, `README.ar.md`), brand emblem & static Next.js portal on GitHub Pages

## Phase 3 — Ecosystem Plugins & Real-Time Observability *(Next)*
- [ ] **Linux AppImage / `.deb` / `.rpm`** packages & XDG tray notifications in CI release matrix
- [ ] **Multi-Provider Failover Profiles**: Automatic upstream failover pools (AgentRouter primary -> fallback gateways) with health scoring
- [ ] **Live SSE Payload Inspector**: Expandable request/response diff viewer inside the Logs tab for debugging MCP schemas
- [ ] **Cursor, Windsurf & Continue.dev Sync**: Extended 1-click config injectors inside Connected Apps
- [ ] **Local Audio Transcription Fallback**: Optional local Whisper / SenseVoice bridge adapter for `/voice` workflows

## Phase 4 — Team Policies, Cost Guardrails & Extensions *(Planned)*
- [ ] **Daily Budget & Token Quota Caps**: Soft alerts and hard rate caps per agent profile
- [ ] **Encrypted Secret Store Integration**: Native OS Credential Manager / macOS Keychain backing for upstream API keys
- [ ] **Plugin SDK for Custom Translators**: WebAssembly / Rust trait hooks for specialized reasoning providers

---

Want to help shape Phase 3 or Phase 4? Check out [CONTRIBUTING.md](CONTRIBUTING.md) and open a Discussion or Pull Request!

