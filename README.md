<div align="center">

# Claude Switch Pro — Aura Bridge Control Center

**Unified Open-Source Tauri v2 Desktop Application & Pure Rust Anthropic <-> OpenAI Responses Bridge**

**تطبيق سطح مكتب مفتوح المصدر فوري الإقلاع وجسر نماذج متكامل (بدون الاعتماد على Node.js)**

[![CI](https://github.com/Mstrq4/claude-switch-pro/actions/workflows/ci.yml/badge.svg)](https://github.com/Mstrq4/claude-switch-pro/actions/workflows/ci.yml)
[![Release](https://github.com/Mstrq4/claude-switch-pro/actions/workflows/release.yml/badge.svg)](https://github.com/Mstrq4/claude-switch-pro/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-pink.svg)](./LICENSE)

</div>

---

## المزايا الرئيسية / Key Features

- **إقلاع فوري بدون شاشة رمادية / Instant Native Launch**: واجهة Tauri v2 عبر WebView2 / WKWebView مع أصول محلية مدمجة بالكامل (صفر طلبات خطوط حظر عند الإقلاع) وتشغيل في صينية النظام.
- **محرك جسر مدمج بلغة Rust / Embedded Pure-Rust Bridge Engine (`127.0.0.1:18787`)**:
  - تحويل فوري بدون فقدان بين **Anthropic Messages API (`/v1/messages`)** و **OpenAI Responses API (`/v1/responses`)** مع بث SSE مباشر.
  - دعم كامل للصور واللقطات (**Vision**)، مستندات **PDF**، ومستويات التفكير المتدرجة حتى **`xhigh`** و **`max`**.
  - **Tool Name Codec & MCP Schema Sanitizer**: تشفير أسماء أدوات MCP الأطول من 64 حرفاً وإصلاح مخططات JSON (إزالة خصائص غير مدعومة وموازنة أزواج `function_call` / `function_call_output`).
  - **Pre-Commit SSE Rate-Limit Shield**: حماية استباقية لإعادة المحاولة عند أخطاء `429` / `too_many_requests` قبل بدء البث للعميل مع حارس تزامن ثقيل (`HeavyRequestGate`).
- **مراقبة وإحصائيات كل وكيل / Per-Agent Analytics**: لوحة تحكم Aura من 6 صفحات كاملة العرض تتبع استهلاك التوكنات وتوفير الكاش وتوقيت كل وكيل (`Claude Code CLI`, `Subagents`, `DeepSeek Harness`, `Claude Desktop`).
- **مزامنة الأدوات بنقرة واحدة / 1-Click Tool Sync**: تهيئة تلقائية لملفات إعدادات **Claude Code CLI** و **DeepSeek Harness** و **Claude Desktop** عبر Windows و macOS.
- **معالج تثبيت متعدد المراحل + تحديثات GitHub التلقائية / Multi-Step Aura Installer & Auto-Updater**:
  - معالج تثبيت تفاعلي برسومات Aura (`ClaudeSwitchPro-Setup.exe`) مع دعم التثبيت الصامت `--silent` / `/S` للترقيات الآلية.
  - تحديث ذاتي موقّع رقمياً (`tauri-plugin-updater` + `latest.json`) مباشرة من إصدارات GitHub.

---

## التحميل والتثبيت / Download & Installation

من صفحة [**GitHub Releases**](https://github.com/Mstrq4/claude-switch-pro/releases/latest):

| النظام / Platform | المعمارية / Architecture | الملفات المتاحة / Artifacts |
|---|---|---|
| **Windows** | `x86_64` (64-bit), `ARM64` | `ClaudeSwitchPro-Setup-windows-*.exe` (معالج التثبيت المخصص)، `.msi`، `-setup.exe` |
| **macOS** | Apple Silicon (`aarch64`), Intel (`x86_64`), Universal | `.dmg`، `.app.tar.gz` |

> للترقية التلقائية عبر سطر الأوامر على ويندوز:
> ```cmd
> ClaudeSwitchPro-Setup.exe --silent
> ```

---

## مسارات البيانات المحلية / Local Data Paths

| Platform | Configuration & Telemetry Directory |
|---|---|
| **Windows** | `%LOCALAPPDATA%\ClaudeSwitchPro\` (`.env`, `telemetry.json`) |
| **macOS** | `~/Library/Application Support/ClaudeSwitchPro/` (`.env`, `telemetry.json`) |
| **Linux** | `~/.local/share/ClaudeSwitchPro/` (`.env`, `telemetry.json`) |

---

## البناء من المصدر / Building from Source

### المتطلبات / Prerequisites
- Rust stable toolchain (`1.85+`)
- Node.js `20+` & npm
- On Windows: Microsoft Edge WebView2 Runtime + LLVM (`llvm-rc`) or MinGW (`windres`) or Windows SDK (`rc.exe`)
- On macOS: Xcode Command Line Tools (`xcode-select --install`)

### أوامر التشغيل والاختبار / Commands

```bash
git clone https://github.com/Mstrq4/claude-switch-pro.git
cd claude-switch-pro
npm ci

# Run workspace unit & integration tests
cargo test --workspace

# Run desktop app in development mode
npx tauri dev

# Build release application & bundles
npx tauri build
```

لبناء معالج التثبيت المخصص على ويندوز (`ClaudeSwitchPro-Setup.exe`):

```powershell
cargo build --release -p claude-switch-pro
cargo build --release -p claude-switch-pro-setup
```

---

## إصدار نسخة جديدة وتحديثها تلقائياً / Publishing Releases

1. حدّث رقم الإصدار في `Cargo.toml`، `package.json`، `src-tauri/Cargo.toml`، و`src-tauri/tauri.conf.json`.
2. أنشئ وسماً جديداً وادفعه إلى GitHub:
   ```bash
   git tag v2.1.0
   git push origin v2.1.0
   ```
3. يتولى `.github/workflows/release.yml` تلقائياً بناء وتوقيع حزم **Windows (x64 & ARM64)** و **macOS (Apple Silicon, Intel, Universal)** وإصدار ملف `latest.json` ليصل التحديث فوراً لجميع المستخدمين.

---

## License

Licensed under the [MIT License](./LICENSE).
