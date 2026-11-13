# Contributing to AuraBridge

Thank you for helping make **AuraBridge** faster, cleaner, and compatible across every developer workflow and OS architecture!

## Repository Structure

| Path | Purpose | Stack |
| :--- | :--- | :--- |
| `crates/bridge-core/` | HTTP/SSE protocol translator, MCP sanitizer, Pre-Commit 429 Shield, telemetry, app sync & updater | Rust (`axum`, `tokio`, `reqwest`) |
| `crates/setup-wizard/` | Standalone 4-step Aura GUI installer & silent CLI (`--silent`) | Rust (`wry`, `tao`) + `ui/installer.html` |
| `src-tauri/` | Native desktop shell, System Tray, autostart, single-instance & Minisign updater | Tauri v2 |
| `ui/` | Embedded Aura Control Center & Setup Wizard HTML/CSS/JS (zero external render-blocking assets) | Vanilla ESM + Spatial Aura CSS |
| `docs/assets/` | Scalable SVG brand & architecture diagrams | SVG |

> The official Next.js web portal lives in its own dedicated repository: [`Mstrq4/aura-bridge-web`](https://github.com/Mstrq4/aura-bridge-web) ([Live Site](https://mstrq4.github.io/aura-bridge-web/)).

## Local Development Workflow

### 1. Prerequisites
- **Rust stable** (`rustup toolchain install stable`) with `clippy`
- **Node.js >= 22** & **pnpm >= 10** (`napp` CLI installed on Windows)
- On Windows: Microsoft Edge WebView2 Runtime & LLVM (`llvm-rc`) or Windows SDK (`rc.exe`)
- On macOS: Xcode Command Line Tools (`xcode-select --install`)

### 2. Running Tests & Quality Gates

```powershell
# Rust core, setup wizard & Tauri workspace tests
cargo test --workspace
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets
```

For web portal contributions in the sibling [`aura-bridge-web`](https://github.com/Mstrq4/aura-bridge-web) repository, run `napp check`.

## Coding & Contribution Rules

1. **Zero Secrets in Commits**: Never commit `.env`, `*.key`, `telemetry.json`, personal config backups, tokens (`sk-...`), or compiled `*.exe` / `*.dmg` binaries.
2. **Preserve External Reference Safety**: Follow `AGENTS.md` strictly regarding isolated projects.
3. **Bilingual & RTL-First UI**: New UI controls in both `ui/` and `aura-bridge-web` must define both English (`en`, LTR) and Arabic (`ar`, RTL) translations using logical CSS (`ps-*`, `pe-*`, `text-start`).
4. **Tests First**: Include focused unit tests in `crates/bridge-core` for protocol translation changes, schema sanitization, or bridge regressions.

## Submitting a Pull Request

1. Fork `https://github.com/Mstrq4/aura-bridge` and create a feature branch (`feat/...` or `fix/...`).
2. Verify `cargo test --workspace` passes locally.
3. Open a concise PR explaining the protocol change, platform tested, and verification output.
