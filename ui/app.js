import { ICONS } from "./icons.js";

let API_BASE =
  (window.location.protocol === "http:" || window.location.protocol === "https:") &&
  window.location.hostname !== "tauri.localhost" &&
  !window.__TAURI_INTERNALS__
    ? ""
    : "http://127.0.0.1:18787";

let latestUpdateInfo = null;

const TRANSLATIONS = {
  ar: {
    appTitle: "Claude Switch Pro",
    appSubtitle: "مركز تحكم Aura وجسر النماذج",
    online: "متصل على 18787",
    offline: "غير متصل",
    themeDark: "داكن",
    themeLight: "فاتح",
    btnLang: "English",
    btnRestart: "إعادة تشغيل الجسر",
    btnProbeModels: "فحص النماذج حياً",
    btnRefreshNow: "تحديث البيانات",
    btnViewAgentStats: "عرض إحصائيات كل وكيل",
    btnResetStats: "تصفير الإحصائيات",
    btnClearLogs: "مسح السجل",
    btnRetry: "إعادة المحاولة",
    btnShow: "إظهار",
    btnHide: "إخفاء",
    btnRerunProbe: "إعادة الفحص",
    orbReady: "جاهز",
    orbProbing: "يفحص...",
    orbOffline: "متوقف",
    tabUsage: "المؤشرات العامة",
    tabAgents: "إحصائيات الوكلاء",
    tabLogs: "سجل الطلبات المباشر",
    tabApps: "التطبيقات المرتبطة",
    tabProviders: "المزود والنماذج",
    tabShield: "الدرع والتشخيص",
    dockUsage: "المؤشرات",
    dockAgents: "الوكلاء",
    dockLogs: "السجل",
    dockApps: "التطبيقات",
    dockProviders: "النماذج",
    dockShield: "الدرع",
    offlineBannerText: "تعذر الاتصال بخادم الجسر المحلي على المنفذ 18787. تأكد من تشغيل الخدمة ثم أعد المحاولة.",
    heroTitle: "مركز جسر الاستجابات المتكامل",
    heroSubtitle: "تحويل فوري بدون فقدان بين Anthropic Messages و OpenAI Responses مع دعم الرؤية والملفات والتفكير حتى xhigh.",
    capImages: "الصور واللقطات",
    capPdf: "ملفات PDF",
    statTotalRequests: "إجمالي الطلبات",
    statTotalRequestsSub: "طلبات مسجلة عبر الجسر",
    statTotalTokens: "إجمالي التوكنات",
    statCachedTokens: "الموفّر بالكاش",
    statShieldSaves: "عمليات إنقاذ الدرع",
    sectionModelUsage: "توزيع استهلاك النماذج",
    sectionModelUsageSub: "الاستهلاك الفعلي المسجل لكل فئة نموذج",
    tierOpus: "Opus Tier (الرئيسي)",
    tierSonnet: "Sonnet Tier (المتوازن)",
    tierHaiku: "Haiku Tier (الوكلاء والمهام الخفيفة)",
    tokensWord: "التوكنات",
    sectionAgentsTitle: "إحصائيات كل وكيل وعميل بالتفصيل",
    sectionAgentsSub: "تتبع استهلاك التوكنات، نسبة الكاش، متوسط زمن الاستجابة، والنماذج المستخدمة لكل وكيل على حدة",
    agentsEmptyTitle: "لم يُسجّل نشاط للوكلاء بعد",
    agentsEmptySub: "عند تشغيل أي طلب عبر Claude Code أو الوكلاء الفرعيين (Subagents) أو DeepSeek Harness أو Claude Desktop ستظهر بطاقة إحصائيات تفصيلية لكل وكيل هنا.",
    agentComparisonTitle: "جدول المقارنة التفصيلي بين الوكلاء",
    colAgent: "الوكيل / التطبيق",
    colReqs: "الطلبات",
    colErrs: "الأخطاء",
    colInTokens: "دخول",
    colOutTokens: "خروج",
    colCacheHit: "توفير الكاش",
    colAvgLatency: "متوسط الزمن",
    colLastSeen: "آخر نشاط",
    sectionRecentLogs: "سجل الطلبات المباشر",
    sectionRecentLogsSub: "مراقبة وتصفية الطلبات المعالجة عبر الجسر لحظة بلحظة",
    filterAllClients: "كل الوكلاء",
    filterAllModels: "كل النماذج",
    logsEmptyTitle: "لا توجد طلبات مسجلة تطابق التصفية",
    logsEmptySub: "عند استخدام Claude Code أو DeepSeek Harness أو تشغيل فحص النماذج المباشر ستظهر السجلات الدقيقة هنا.",
    colTime: "الوقت",
    colClient: "الوكيل",
    colModel: "النموذج",
    colStatus: "الحالة",
    colTokens: "التوكنات (دخول / خروج / كاش)",
    colLatency: "الزمن",
    sectionAppsTitle: "التطبيقات المكتشفة والمزامنة بنقرة واحدة",
    sectionAppsSub: "تطبيق إعدادات النماذج والقدرات والتفكير والوسائط المتعددة تلقائياً",
    appClaudeCodeTitle: "Claude Code CLI",
    appClaudeCodeDesc: "الطرفية وتكامل المطورين",
    appDshTitle: "DeepSeek Harness",
    appDshDesc: "عميل سطح المكتب (dsh / pi-ai)",
    appClaudeDesktopTitle: "Claude Desktop",
    appClaudeDesktopDesc: "تطبيق سطح المكتب (Claude-3p)",
    lblConfigPath: "ملف الإعدادات",
    lblCapabilities: "القدرات المفعّلة",
    lblVoiceNote: "ملاحظة الصوت (/voice)",
    cliVoiceLimit: "يتطلب حساب Claude.ai رسمي",
    lblProtocol: "البروتوكول",
    lblModalities: "الصور ومستويات التفكير",
    lblGateway: "البوابة المباشرة",
    lblGuid: "معرّف التكوين",
    lblDirectMode: "الوضع المباشر",
    directGatewayOk: "مربوط مباشرة بالجسر",
    btnApplyCode: "مزامنة وتطبيق على Claude Code",
    btnApplyDsh: "مزامنة وتطبيق على DeepSeek Harness",
    btnApplyDesktop: "تطبيق الإعدادات على Claude Desktop",
    sectionProvidersTitle: "إدارة المزود والنماذج ومستوى التفكير",
    sectionProvidersSub: "تُحفظ التعديلات تلقائياً في ملف .env الخاص بالجسر وتعمل فوراً",
    lblUpstreamUrl: "عنوان المزود الرئيسي (Upstream Base URL)",
    lblApiKey: "مفتاح API (اتركه فارغاً للإبقاء على الحالي)",
    lblOpusModel: "نموذج فئة Opus (المهام الهندسية المعقدة)",
    lblSonnetModel: "نموذج فئة Sonnet (المهام المتوازنة)",
    lblHaikuModel: "نموذج فئة Haiku (الوكلاء والمهام الخفيفة)",
    lblReasoningEffort: "مستوى التفكير الافتراضي (Reasoning Effort)",
    lblReasoningSummary: "ملخص التفكير (Reasoning Summary)",
    btnSaveProvider: "حفظ وتطبيق الإعدادات",
    sectionShieldTitle: "درع الحماية ومصفوفة القدرات",
    sectionShieldSub: "ضبط إعادة المحاولة التلقائية وحالة دعم الصور والصوت والملفات والتفكير",
    shieldTuningTitle: "ضبط معايير درع Pre-Commit SSE",
    lblMaxRetries: "أقصى عدد محاولات (Retries)",
    lblHeadroom: "هامش توكنات التفكير (Headroom)",
    lblRetryDelay: "تأخير المحاولة الأساسي (ms)",
    lblConcurrency: "أقصى طلبات ثقيلة متزامنة",
    btnSaveShield: "حفظ معايير الدرع",
    capMatrixTitle: "مصفوفة دعم الوسائط والتفكير (الحقيقة الفعلية)",
    matVision: "الصور واللقطات (Vision)",
    matPdf: "مستندات PDF",
    matThinking: "مستويات التفكير (low → xhigh)",
    matAudioApi: "إدخال الصوت المباشر للنموذج (input_audio)",
    matDshVoice: "الإدخال الصوتي المحلي في DeepSeek Harness",
    matSupported: "مدعوم بالكامل",
    matUpstreamNo: "غير متاح في agentrouter.org",
    matLocalStt: "متاح محلياً (SenseVoice)",
    capMatrixNote: "أمر /voice داخل Claude Code مقيد برمجياً بحسابات Claude.ai الرسمية، بينما يدعم DeepSeek Harness الإدخال الصوتي المحلي عبر إضافة Voice Input بعد تنزيل نموذج SenseVoice من مدير الإضافات.",
    probeModalTitle: "نتيجة الفحص الحي للنماذج",
    probeModalSub: "اختبار فعلي مباشر لكل نموذج مع المزود الحالي",
    statusConnected: "مفعّل ومتصل",
    statusPending: "بحاجة للمزامنة",
    statusNotFound: "غير مثبت",
    toastDesktopApplied: "تم تطبيق الإعدادات على Claude Desktop بنجاح.",
    toastCodeApplied: "تمت مزامنة Claude Code بالقدرات الكاملة (الصور + xhigh).",
    toastDshApplied: "تم تحديث DeepSeek Harness بالصور ومستويات التفكير و Responses API.",
    toastProviderSaved: "تم حفظ إعدادات المزود والنماذج ومستوى التفكير وتثبيتها في .env.",
    toastShieldSaved: "تم حفظ معايير درع الحماية وتثبيتها في .env.",
    toastCleared: "تم تصفير السجلات والإحصائيات.",
    toastRestarted: "جاري إعادة تشغيل الجسر...",
    btnCheckUpdate: "تحديثات",
    btnInstallUpdate: "تحديث وتثبيت الآن",
    updatesCardTitle: "الإصدار والتحديث التلقائي عبر GitHub",
    lblCurrentVersion: "الإصدار المثبت حالياً",
    lblLatestVersion: "أحدث إصدار على GitHub",
    lblGithubRepo: "المستودع مفتوح المصدر",
    btnCheckUpdatesFull: "التحقق من التحديثات الآن",
    updateUpToDate: "أنت تستخدم أحدث إصدار بالفعل.",
    updateChecking: "جاري التحقق من تحديثات GitHub...",
    updateInstalling: "جاري تنزيل التحديث وتثبيته...",
  },
  en: {
    appTitle: "Claude Switch Pro",
    appSubtitle: "Aura Bridge Control Center",
    online: "Online on 18787",
    offline: "Offline",
    themeDark: "Dark",
    themeLight: "Light",
    btnLang: "العربية",
    btnRestart: "Restart Bridge",
    btnProbeModels: "Live Probe Models",
    btnRefreshNow: "Refresh Now",
    btnViewAgentStats: "View Per-Agent Stats",
    btnResetStats: "Reset Stats",
    btnClearLogs: "Clear Logs",
    btnRetry: "Retry Connection",
    btnShow: "Show",
    btnHide: "Hide",
    btnRerunProbe: "Re-run Probe",
    orbReady: "Ready",
    orbProbing: "Probing...",
    orbOffline: "Offline",
    tabUsage: "Overview & Models",
    tabAgents: "Agent Analytics",
    tabLogs: "Live Request Logs",
    tabApps: "Connected Apps",
    tabProviders: "Providers & Models",
    tabShield: "Shield & Matrix",
    dockUsage: "Overview",
    dockAgents: "Agents",
    dockLogs: "Logs",
    dockApps: "Apps",
    dockProviders: "Models",
    dockShield: "Shield",
    offlineBannerText: "Cannot reach local bridge on port 18787. Verify the daemon is running and retry.",
    heroTitle: "Unified Crystalline Responses Hub",
    heroSubtitle: "Lossless Anthropic Messages <-> OpenAI Responses translation with Vision, PDF, 200+ tools, and xhigh reasoning.",
    capImages: "Vision & Screenshots",
    capPdf: "PDF Documents",
    statTotalRequests: "Total Requests",
    statTotalRequestsSub: "Recorded requests via bridge",
    statTotalTokens: "Total Tokens",
    statCachedTokens: "Cache-Saved Tokens",
    statShieldSaves: "Shield Recoveries",
    sectionModelUsage: "Model Consumption Breakdown",
    sectionModelUsageSub: "Recorded live usage per model tier",
    tierOpus: "Opus Tier (Primary)",
    tierSonnet: "Sonnet Tier (Balanced)",
    tierHaiku: "Haiku Tier (Subagents & Fast)",
    tokensWord: "Tokens",
    sectionAgentsTitle: "Per-Agent & Client Analytics",
    sectionAgentsSub: "Token usage, cache hit ratio, average latency, and model mix broken down per agent",
    agentsEmptyTitle: "No agent activity recorded yet",
    agentsEmptySub: "Send requests from Claude Code, Subagents, DeepSeek Harness, or Claude Desktop to inspect per-agent metrics here.",
    agentComparisonTitle: "Agent Comparison Table",
    colAgent: "Agent / Client",
    colReqs: "Requests",
    colErrs: "Errors",
    colInTokens: "Input",
    colOutTokens: "Output",
    colCacheHit: "Cache Saved",
    colAvgLatency: "Avg Latency",
    colLastSeen: "Last Seen",
    sectionRecentLogs: "Live Request Telemetry",
    sectionRecentLogsSub: "Filter and inspect live requests handled by the bridge",
    filterAllClients: "All Agents",
    filterAllModels: "All Models",
    logsEmptyTitle: "No requests match the current filter",
    logsEmptySub: "Use Claude Code, DeepSeek Harness, or run a Live Model Probe to see live request telemetry here.",
    colTime: "Time",
    colClient: "Agent",
    colModel: "Model",
    colStatus: "Status",
    colTokens: "Tokens (In / Out / Cached)",
    colLatency: "Latency",
    sectionAppsTitle: "Detected Apps & 1-Click Sync",
    sectionAppsSub: "Automatically apply model routing, multimodal support, and reasoning levels",
    appClaudeCodeTitle: "Claude Code CLI",
    appClaudeCodeDesc: "Terminal coding agent",
    appDshTitle: "DeepSeek Harness",
    appDshDesc: "Desktop harness (dsh / pi-ai)",
    appClaudeDesktopTitle: "Claude Desktop",
    appClaudeDesktopDesc: "Anthropic Desktop App (Claude-3p)",
    lblConfigPath: "Config File",
    lblCapabilities: "Enabled Capabilities",
    lblVoiceNote: "Voice (/voice)",
    cliVoiceLimit: "Requires Claude.ai login",
    lblProtocol: "Protocol",
    lblModalities: "Modalities & Reasoning",
    lblGateway: "Local Gateway",
    lblGuid: "Config ID",
    lblDirectMode: "Direct Mode",
    directGatewayOk: "Connected directly to bridge",
    btnApplyCode: "Sync & Apply to Claude Code",
    btnApplyDsh: "Sync & Apply to DeepSeek Harness",
    btnApplyDesktop: "Apply to Claude Desktop",
    sectionProvidersTitle: "Provider, Models & Reasoning Settings",
    sectionProvidersSub: "Changes persist automatically to .env and take effect immediately",
    lblUpstreamUrl: "Upstream Base URL",
    lblApiKey: "API Key (leave blank to keep current)",
    lblOpusModel: "Opus Tier Model",
    lblSonnetModel: "Sonnet Tier Model",
    lblHaikuModel: "Haiku Tier Model",
    lblReasoningEffort: "Default Reasoning Effort",
    lblReasoningSummary: "Reasoning Summary",
    btnSaveProvider: "Save & Apply Configuration",
    sectionShieldTitle: "Rate-Limit Shield & Capability Matrix",
    sectionShieldSub: "Configure pre-commit SSE retries and inspect verified multimodal limits",
    shieldTuningTitle: "Pre-Commit SSE Shield Parameters",
    lblMaxRetries: "Max Upstream Retries",
    lblHeadroom: "Thinking Token Headroom",
    lblRetryDelay: "Base Retry Delay (ms)",
    lblConcurrency: "Max Concurrent Heavy Requests",
    btnSaveShield: "Save Shield Tuning",
    capMatrixTitle: "Verified Multimodal & Reasoning Matrix",
    matVision: "Images & Screenshots",
    matPdf: "PDF Documents",
    matThinking: "Reasoning Effort (low -> xhigh)",
    matAudioApi: "Direct Model Audio Input (input_audio)",
    matDshVoice: "Local Voice Input in DeepSeek Harness",
    matSupported: "Fully Supported",
    matUpstreamNo: "Unavailable on agentrouter.org",
    matLocalStt: "Local STT (SenseVoice)",
    capMatrixNote: "Claude Code's /voice requires an official Claude.ai OAuth session. DeepSeek Harness supports local voice-to-text via Voice Input once SenseVoice is downloaded in Plugins.",
    probeModalTitle: "Live Model Health Probe",
    probeModalSub: "Direct request check against the configured upstream provider",
    statusConnected: "Connected & Ready",
    statusPending: "Sync Recommended",
    statusNotFound: "Not Installed",
    toastDesktopApplied: "Claude Desktop settings applied.",
    toastCodeApplied: "Claude Code CLI synced with Vision, PDF & xhigh capabilities.",
    toastDshApplied: "DeepSeek Harness synced with Vision, Reasoning & Responses API.",
    toastProviderSaved: "Provider, models & reasoning settings saved and persisted.",
    toastShieldSaved: "Shield parameters saved and persisted.",
    toastCleared: "Logs and telemetry counters reset.",
    toastRestarted: "Restarting bridge daemon...",
    btnCheckUpdate: "Updates",
    btnInstallUpdate: "Update & Install Now",
    updatesCardTitle: "Version & GitHub Auto-Updater",
    lblCurrentVersion: "Installed Version",
    lblLatestVersion: "Latest GitHub Release",
    lblGithubRepo: "Open-Source Repository",
    btnCheckUpdatesFull: "Check for Updates",
    updateUpToDate: "You are already on the latest version.",
    updateChecking: "Checking GitHub Releases...",
    updateInstalling: "Downloading and installing update...",
  },
};

let currentLang = localStorage.getItem("crb_lang") === "en" ? "en" : "ar";
let currentTheme = localStorage.getItem("crb_theme") === "dark" ? "dark" : "light";
let activeTabId = "tab-usage";
let formHydrated = false;
let bridgeState = {
  status: "offline",
  uptimeSeconds: 0,
  models: { opus: "gpt-6.1-sol", sonnet: "gpt-6-astra", haiku: "gpt-5.6-sol" },
  metrics: {},
  logs: [],
};
let appStatusState = {};

function t(key) {
  return TRANSLATIONS[currentLang]?.[key] || key;
}

function renderIcons() {
  document.querySelectorAll("[data-icon]").forEach((el) => {
    const key = el.getAttribute("data-icon");
    if (ICONS[key]) el.innerHTML = ICONS[key];
  });
}

function applyTheme(theme) {
  currentTheme = theme === "dark" ? "dark" : "light";
  localStorage.setItem("crb_theme", currentTheme);
  document.documentElement.setAttribute("data-theme", currentTheme);
  const nextIcon = currentTheme === "dark" ? "sun" : "moon";
  const labelKey = currentTheme === "dark" ? "themeLight" : "themeDark";
  const iconEl = document.getElementById("theme-icon");
  const iconMob = document.getElementById("theme-icon-mobile");
  const lblEl = document.getElementById("theme-label");
  if (iconEl && ICONS[nextIcon]) iconEl.innerHTML = ICONS[nextIcon];
  if (iconMob && ICONS[nextIcon]) iconMob.innerHTML = ICONS[nextIcon];
  if (lblEl) lblEl.textContent = t(labelKey);
}

function applyLanguage(lang) {
  currentLang = lang === "en" ? "en" : "ar";
  localStorage.setItem("crb_lang", currentLang);
  document.documentElement.setAttribute("lang", currentLang);
  document.documentElement.setAttribute("dir", currentLang === "ar" ? "rtl" : "ltr");
  document.querySelectorAll("[data-t]").forEach((el) => {
    const k = el.getAttribute("data-t");
    if (TRANSLATIONS[currentLang][k]) el.textContent = TRANSLATIONS[currentLang][k];
  });
  const activeNav = document.querySelector(`.rail-btn[data-tab="${activeTabId}"]`);
  const pageTitle = document.getElementById("active-page-title");
  if (activeNav && pageTitle) {
    const k = activeNav.querySelector("[data-t]")?.getAttribute("data-t");
    if (k) pageTitle.textContent = t(k);
  }
  applyTheme(currentTheme);
  updateUsageMetrics();
  updateAppStatusUI();
}

function switchTab(tabId) {
  activeTabId = tabId;
  document.querySelectorAll("[data-tab]").forEach((b) => {
    b.classList.toggle("active", b.getAttribute("data-tab") === tabId);
  });
  document.querySelectorAll(".tab-pane").forEach((p) => {
    p.classList.toggle("active", p.id === tabId);
  });
  const activeNav = document.querySelector(`.rail-btn[data-tab="${tabId}"]`);
  const pageTitle = document.getElementById("active-page-title");
  if (activeNav && pageTitle) {
    const k = activeNav.querySelector("[data-t]")?.getAttribute("data-t");
    if (k) pageTitle.textContent = t(k);
  }
}

function showToast(message, isError = false) {
  const container = document.getElementById("toast-container");
  if (!container) return;
  const toast = document.createElement("div");
  toast.className = "toast";
  const color = isError ? "var(--status-danger)" : "var(--status-online)";
  const icon = isError ? ICONS.alert : ICONS.check;
  toast.innerHTML = `<span style="color:${color};display:inline-flex;">${icon}</span><span></span>`;
  toast.querySelector("span:last-child").textContent = message;
  container.appendChild(toast);
  setTimeout(() => toast.remove(), 4200);
}

function hydrateConfigForms(data) {
  if (formHydrated || !data) return;
  formHydrated = true;
  const setVal = (id, v) => {
    const el = document.getElementById(id);
    if (el && v !== undefined && v !== null) el.value = String(v);
  };
  setVal("prov-url", data.upstreamBaseUrl);
  setVal("prov-opus", data.models?.opus);
  setVal("prov-sonnet", data.models?.sonnet);
  setVal("prov-haiku", data.models?.haiku);
  setVal("prov-reasoning", data.defaultReasoningEffort || "medium");
  setVal("prov-summary", data.reasoningSummary || "auto");
  setVal("shield-retries", data.maxUpstreamRetries ?? 6);
  setVal("shield-headroom", data.minReasoningTokenHeadroom ?? 4096);
  setVal("shield-delay", data.retryBaseDelayMs ?? 6000);
  setVal("shield-concurrency", data.maxConcurrentHeavyRequests ?? 2);

  const keyInput = document.getElementById("prov-key");
  if (keyInput && data.apiKeyMasked) {
    keyInput.placeholder = data.apiKeyMasked;
  }
}

async function fetchStatus() {
  try {
    const res = await fetch(`${API_BASE}/health`);
    if (!res.ok) throw new Error("HTTP " + res.status);
    const data = await res.json();
    bridgeState = data;
    hydrateConfigForms(data);
    renderOnline();
    updateUsageMetrics();
  } catch {
    renderOffline();
  }
}

async function fetchAppStatus() {
  try {
    const res = await fetch(`${API_BASE}/api/app-status`);
    if (res.ok) {
      appStatusState = await res.json();
      updateAppStatusUI();
    }
  } catch {}
}

async function checkForUpdates(manual = false) {
  if (manual) showToast(t("updateChecking"));
  try {
    const res = await fetch(`${API_BASE}/api/update/check`);
    if (!res.ok) throw new Error("HTTP " + res.status);
    const data = await res.json();
    latestUpdateInfo = data;
    const curVer = document.getElementById("shield-current-ver");
    const latVer = document.getElementById("shield-latest-ver");
    if (curVer && data.currentVersion) curVer.textContent = `v${data.currentVersion}`;
    if (latVer) latVer.textContent = data.latestVersion ? `v${data.latestVersion}` : "—";

    const banner = document.getElementById("update-banner");
    const bannerTxt = document.getElementById("update-banner-text");
    const installCardBtn = document.getElementById("btn-install-update-card");
    if (data.updateAvailable) {
      if (banner) banner.style.display = "flex";
      if (bannerTxt) {
        bannerTxt.textContent =
          currentLang === "ar"
            ? `يتوفر إصدار جديد v${data.latestVersion} على GitHub (الحالي v${data.currentVersion})`
            : `New release v${data.latestVersion} available on GitHub (current v${data.currentVersion})`;
      }
      if (installCardBtn) installCardBtn.style.display = "inline-flex";
    } else {
      if (banner) banner.style.display = "none";
      if (installCardBtn) installCardBtn.style.display = "none";
      if (manual) showToast(t("updateUpToDate"));
    }
  } catch (err) {
    if (manual) showToast(err.message || "Update check failed", true);
  }
}

async function installAppUpdate() {
  const tauriInvoke = window.__TAURI__?.core?.invoke || window.__TAURI_INTERNALS__?.invoke;
  if (tauriInvoke) {
    showToast(t("updateInstalling"));
    try {
      const msg = await tauriInvoke("install_github_update");
      showToast(msg || "Update installed. Restarting...");
      return;
    } catch (err) {
      if (latestUpdateInfo?.releaseUrl) {
        window.open(latestUpdateInfo.assetUrl || latestUpdateInfo.releaseUrl, "_blank", "noopener");
      }
      showToast(typeof err === "string" ? err : err?.message || "Updater fallback", true);
      return;
    }
  }
  const url = latestUpdateInfo?.assetUrl || latestUpdateInfo?.releaseUrl || "https://github.com/Mstrq4/claude-switch-pro/releases/latest";
  window.open(url, "_blank", "noopener");
}

function renderOnline() {
  document.getElementById("offline-banner")?.style.setProperty("display", "none");
  ["status-indicator", "status-indicator-rail"].forEach((id) => {
    const el = document.getElementById(id);
    if (el) {
      el.className = "status-pill";
      el.innerHTML = `<span class="status-dot"></span><span>${t("online")}</span>`;
    }
  });
  const orb = document.getElementById("hero-orb-btn");
  orb?.classList.remove("offline");
  const orbTxt = document.getElementById("orb-badge-text");
  if (orbTxt && !orb?.classList.contains("probing")) orbTxt.textContent = t("orbReady");

  const meta = document.getElementById("topbar-submeta");
  if (meta) {
    meta.textContent = `${bridgeState.listen || "http://127.0.0.1:18787"} • PID ${bridgeState.pid || "—"}`;
  }
  const verPill = document.getElementById("app-version-pill");
  if (verPill && bridgeState.version) {
    verPill.textContent = `v${bridgeState.version}`;
  }
  const curVer = document.getElementById("shield-current-ver");
  if (curVer && bridgeState.version) {
    curVer.textContent = `v${bridgeState.version}`;
  }
}

function renderOffline() {
  document.getElementById("offline-banner")?.style.setProperty("display", "flex");
  ["status-indicator", "status-indicator-rail"].forEach((id) => {
    const el = document.getElementById(id);
    if (el) {
      el.className = "status-pill offline";
      el.innerHTML = `<span class="status-dot"></span><span>${t("offline")}</span>`;
    }
  });
  document.getElementById("hero-orb-btn")?.classList.add("offline");
  const orbTxt = document.getElementById("orb-badge-text");
  if (orbTxt) orbTxt.textContent = t("orbOffline");
}

function setBadge(el, ok, pending, notInstalled) {
  if (!el) return;
  if (notInstalled) {
    el.className = "chip";
    el.textContent = t("statusNotFound");
  } else if (ok) {
    el.className = "chip chip-ok";
    el.textContent = t("statusConnected");
  } else if (pending) {
    el.className = "chip chip-warn";
    el.textContent = t("statusPending");
  }
}

function updateAppStatusUI() {
  const cc = appStatusState.claudeCode;
  setBadge(
    document.getElementById("badge-claude-code"),
    cc?.applied && cc?.capabilitiesReady,
    cc?.installed,
    cc && !cc.installed,
  );
  const dsh = appStatusState.deepseekHarness;
  setBadge(
    document.getElementById("badge-dsh"),
    dsh?.applied && dsh?.multimodalReady,
    dsh?.installed,
    dsh && !dsh.installed,
  );
  const cd = appStatusState.claudeDesktop;
  setBadge(
    document.getElementById("badge-claude-desktop"),
    cd?.applied,
    cd?.installed,
    cd && !cd.installed,
  );
}

function escapeHtml(str) {
  return String(str)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function renderAgentsPage(clientStats) {
  const entries = Object.entries(clientStats || {}).sort((a, b) => (b[1]?.tokens || 0) - (a[1]?.tokens || 0));
  const emptyEl = document.getElementById("agents-empty");
  const gridEl = document.getElementById("agents-grid");
  const tableCard = document.getElementById("agents-table-card");
  const tbody = document.getElementById("agents-tbody");

  if (entries.length === 0) {
    if (emptyEl) emptyEl.style.display = "flex";
    if (gridEl) gridEl.style.display = "none";
    if (tableCard) tableCard.style.display = "none";
    return;
  }

  if (emptyEl) emptyEl.style.display = "none";
  if (gridEl) gridEl.style.display = "grid";
  if (tableCard) tableCard.style.display = "flex";

  const totalAllTokens = Math.max(1, entries.reduce((acc, [, s]) => acc + Number(s.tokens || 0), 0));

  if (gridEl) {
    gridEl.innerHTML = entries
      .map(([name, s]) => {
        const reqs = Number(s.requests || 0);
        const errs = Number(s.errors || 0);
        const inTok = Number(s.inTokens || 0);
        const outTok = Number(s.outTokens || 0);
        const cacheTok = Number(s.cachedTokens || 0);
        const tok = Number(s.tokens || 0);
        const avgLat = reqs > 0 ? Math.round(Number(s.totalLatencyMs || 0) / reqs) : 0;
        const sharePct = Math.round((tok / totalAllTokens) * 100);
        const cacheDenom = inTok + outTok + cacheTok;
        const cachePct = cacheDenom > 0 ? ((cacheTok / cacheDenom) * 100).toFixed(1) : "0.0";
        const modelTags = Object.entries(s.models || {})
          .map(([mId, ms]) => `<span class="chip mono">${escapeHtml(mId)}: ${ms.requests}</span>`)
          .join("");

        return `<article class="card">
          <div class="card-header">
            <div>
              <strong style="font-size:0.96rem;font-weight:800;">${escapeHtml(name)}</strong>
              <div class="brand-subtitle">${reqs} ${currentLang === "ar" ? "طلب" : "reqs"} • ${avgLat} ms</div>
            </div>
            <span class="chip ${errs > 0 ? "chip-warn" : "chip-ok"}">${sharePct}%</span>
          </div>
          <div class="progress-track">
            <div class="progress-fill fill-sonnet" style="width:${sharePct}%;"></div>
          </div>
          <div class="info-box">
            <div class="info-row">
              <span class="info-key">${currentLang === "ar" ? "المدخلات / المخرجات" : "Input / Output"}</span>
              <span class="info-val mono">${inTok.toLocaleString()} / ${outTok.toLocaleString()}</span>
            </div>
            <div class="info-row">
              <span class="info-key">${currentLang === "ar" ? "توفير الكاش" : "Cache Saved"}</span>
              <span class="info-val mono" style="color:var(--status-online);">${cacheTok.toLocaleString()} (${cachePct}%)</span>
            </div>
            <div class="info-row">
              <span class="info-key">${currentLang === "ar" ? "آخر نشاط" : "Last Seen"}</span>
              <span class="info-val mono">${escapeHtml(s.lastSeen || "—")}</span>
            </div>
          </div>
          <div class="hub-chips">${modelTags}</div>
        </article>`;
      })
      .join("");
  }

  if (tbody) {
    tbody.innerHTML = entries
      .map(([name, s]) => {
        const reqs = Number(s.requests || 0);
        const avgLat = reqs > 0 ? Math.round(Number(s.totalLatencyMs || 0) / reqs) : 0;
        const inTok = Number(s.inTokens || 0);
        const outTok = Number(s.outTokens || 0);
        const cacheTok = Number(s.cachedTokens || 0);
        return `<tr>
          <td><strong>${escapeHtml(name)}</strong></td>
          <td class="mono">${reqs.toLocaleString()}</td>
          <td class="mono">${Number(s.errors || 0).toLocaleString()}</td>
          <td class="mono">${inTok.toLocaleString()}</td>
          <td class="mono">${outTok.toLocaleString()}</td>
          <td class="mono" style="color:var(--status-online);">${cacheTok.toLocaleString()}</td>
          <td class="mono">${avgLat} ms</td>
          <td class="mono">${escapeHtml(s.lastSeen || "—")}</td>
        </tr>`;
      })
      .join("");
  }
}

function updateLogFilters(logs) {
  const clientSel = document.getElementById("filter-client");
  const modelSel = document.getElementById("filter-model");
  if (!clientSel || !modelSel) return;

  const prevClient = clientSel.value || "all";
  const prevModel = modelSel.value || "all";
  const clients = Array.from(new Set(logs.map((l) => l.client).filter(Boolean)));
  const models = Array.from(new Set(logs.map((l) => l.model).filter(Boolean)));

  clientSel.innerHTML =
    `<option value="all">${escapeHtml(t("filterAllClients"))}</option>` +
    clients.map((c) => `<option value="${escapeHtml(c)}">${escapeHtml(c)}</option>`).join("");
  modelSel.innerHTML =
    `<option value="all">${escapeHtml(t("filterAllModels"))}</option>` +
    models.map((m) => `<option value="${escapeHtml(m)}">${escapeHtml(m)}</option>`).join("");

  clientSel.value = clients.includes(prevClient) ? prevClient : "all";
  modelSel.value = models.includes(prevModel) ? prevModel : "all";
}

function renderLogsTable() {
  const logs = Array.isArray(bridgeState.logs) ? bridgeState.logs : [];
  updateLogFilters(logs);
  const fClient = document.getElementById("filter-client")?.value || "all";
  const fModel = document.getElementById("filter-model")?.value || "all";

  const filtered = logs.filter((row) => {
    if (fClient !== "all" && row.client !== fClient) return false;
    if (fModel !== "all" && row.model !== fModel) return false;
    return true;
  });

  const emptyEl = document.getElementById("logs-empty");
  const wrapEl = document.getElementById("logs-table-wrap");
  const tbody = document.getElementById("logs-tbody");

  if (filtered.length === 0) {
    if (emptyEl) emptyEl.style.display = "flex";
    if (wrapEl) wrapEl.style.display = "none";
  } else {
    if (emptyEl) emptyEl.style.display = "none";
    if (wrapEl) wrapEl.style.display = "block";
    if (tbody) {
      tbody.innerHTML = filtered
        .slice(0, 60)
        .map((row) => {
          const ok = Number(row.status) >= 200 && Number(row.status) < 300;
          const statusCls = ok ? "chip chip-ok" : "chip chip-err";
          return `<tr>
            <td class="mono">${escapeHtml(row.time || "")}</td>
            <td>${escapeHtml(row.client || "Client")}</td>
            <td class="mono">${escapeHtml(row.model || "")}</td>
            <td><span class="${statusCls}">${escapeHtml(String(row.status || 200))}</span></td>
            <td class="mono">${Number(row.inTokens || 0).toLocaleString()} / ${Number(row.outTokens || 0).toLocaleString()} / ${Number(row.cachedTokens || 0).toLocaleString()}</td>
            <td class="mono">${Number(row.latency || 0)} ms</td>
          </tr>`;
        })
        .join("");
    }
  }
}

function updateUsageMetrics() {
  const m = bridgeState.metrics || {};
  const totalIn = Number(m.inputTokensTotal || 0);
  const totalOut = Number(m.outputTokensTotal || 0);
  const totalCached = Number(m.cachedTokensTotal || 0);
  const totalTokens = totalIn + totalOut;
  const totalWithCache = totalIn + totalOut + totalCached;
  const cachePct = totalWithCache > 0 ? ((totalCached / totalWithCache) * 100).toFixed(1) : "0.0";

  const setTxt = (id, val) => {
    const el = document.getElementById(id);
    if (el) el.textContent = String(val);
  };

  setTxt("stat-total-req", Number(m.requestsTotal || 0).toLocaleString());
  setTxt("stat-total-tokens", totalTokens.toLocaleString());
  setTxt("stat-inout-sub", `${totalIn.toLocaleString()} in / ${totalOut.toLocaleString()} out`);
  setTxt("stat-cached-tokens", totalCached.toLocaleString());
  setTxt("stat-cache-percent", currentLang === "ar" ? `نسبة الكاش: ${cachePct}%` : `Cache Ratio: ${cachePct}%`);
  setTxt("stat-shield-saves", Number(m.rateLimitShieldHits || 0).toLocaleString());
  setTxt("stat-errors-sub", currentLang === "ar" ? `إعادة المحاولة: ${m.retriesTotal || 0}` : `Retries: ${m.retriesTotal || 0}`);
  setTxt("chip-effort", `Effort: ${bridgeState.defaultReasoningEffort || "medium"}`);
  setTxt("chip-uptime", `Uptime: ${bridgeState.uptimeSeconds || 0}s`);

  const opusId = bridgeState.models?.opus || "gpt-6.1-sol";
  const sonnetId = bridgeState.models?.sonnet || "gpt-6-astra";
  const haikuId = bridgeState.models?.haiku || "gpt-5.6-sol";
  setTxt("model-name-opus", opusId);
  setTxt("model-name-sonnet", sonnetId);
  setTxt("model-name-haiku", haikuId);

  const stats = m.modelStats || {};
  const opusStat = stats[opusId] || { requests: 0, tokens: 0 };
  const sonnetStat = stats[sonnetId] || { requests: 0, tokens: 0 };
  const haikuStat = stats[haikuId] || { requests: 0, tokens: 0 };
  const sumModelTok = opusStat.tokens + sonnetStat.tokens + haikuStat.tokens;

  const pct = (tok) => (sumModelTok > 0 ? Math.round((tok / sumModelTok) * 100) : 0);
  const opusPct = pct(opusStat.tokens);
  const sonnetPct = pct(sonnetStat.tokens);
  const haikuPct = pct(haikuStat.tokens);

  const reqLabel = currentLang === "ar" ? "طلب" : "reqs";
  setTxt("model-opus-reqs", `${opusStat.requests} ${reqLabel}`);
  setTxt("model-sonnet-reqs", `${sonnetStat.requests} ${reqLabel}`);
  setTxt("model-haiku-reqs", `${haikuStat.requests} ${reqLabel}`);
  setTxt("model-opus-tokens", opusStat.tokens.toLocaleString());
  setTxt("model-sonnet-tokens", sonnetStat.tokens.toLocaleString());
  setTxt("model-haiku-tokens", haikuStat.tokens.toLocaleString());
  setTxt("model-opus-pct", `${opusPct}%`);
  setTxt("model-sonnet-pct", `${sonnetPct}%`);
  setTxt("model-haiku-pct", `${haikuPct}%`);

  document.getElementById("model-opus-bar")?.style.setProperty("width", `${opusPct}%`);
  document.getElementById("model-sonnet-bar")?.style.setProperty("width", `${sonnetPct}%`);
  document.getElementById("model-haiku-bar")?.style.setProperty("width", `${haikuPct}%`);

  renderAgentsPage(m.clientStats);
  renderLogsTable();
}

async function runLiveProbe() {
  const modal = document.getElementById("probe-modal");
  const body = document.getElementById("probe-modal-body");
  const orb = document.getElementById("hero-orb-btn");
  const orbTxt = document.getElementById("orb-badge-text");
  modal?.classList.add("open");
  orb?.classList.add("probing");
  if (orbTxt) orbTxt.textContent = t("orbProbing");
  if (body) {
    body.innerHTML = [1, 2, 3]
      .map(() => `<div class="aurora-skeleton" style="height:64px;width:100%;"></div>`)
      .join("");
  }
  try {
    const res = await fetch(`${API_BASE}/api/probe-models`, { method: "POST" });
    const data = await res.json();
    const list = Array.isArray(data.results) ? data.results : [];
    if (body) {
      body.innerHTML = list
        .map((r) => {
          const cls = r.ok ? "chip chip-ok" : r.status === 402 ? "chip chip-warn" : "chip chip-err";
          const label = r.ok ? `200 OK (${r.latencyMs}ms)` : `HTTP ${r.status || "ERR"} (${r.latencyMs}ms)`;
          return `<div class="info-box">
            <div class="info-row">
              <strong class="mono">${escapeHtml(r.model)}</strong>
              <span class="${cls}">${escapeHtml(label)}</span>
            </div>
            ${r.error ? `<div style="font-size:0.78rem;color:var(--status-danger);word-break:break-word;">${escapeHtml(r.error)}</div>` : ""}
          </div>`;
        })
        .join("");
    }
  } catch (err) {
    if (body) {
      body.innerHTML = `<div class="error-banner">${escapeHtml(err.message || "Probe failed")}</div>`;
    }
  } finally {
    orb?.classList.remove("probing");
    if (orbTxt) orbTxt.textContent = t("orbReady");
  }
}

async function postApply(url, btnId, toastKey) {
  const btn = document.getElementById(btnId);
  if (!btn) return;
  btn.disabled = true;
  try {
    const res = await fetch(`${API_BASE}${url}`, { method: "POST" });
    const data = await res.json();
    if (!res.ok || !data.success) throw new Error(data.error || "Request failed");
    showToast(t(toastKey));
    fetchAppStatus();
  } catch (err) {
    showToast(err.message || "Error", true);
  } finally {
    btn.disabled = false;
  }
}

async function clearTelemetry() {
  try {
    const res = await fetch(`${API_BASE}/api/logs/clear`, { method: "POST" });
    if (res.ok) {
      showToast(t("toastCleared"));
      fetchStatus();
    }
  } catch (err) {
    showToast(err.message || "Error", true);
  }
}

function setupEventListeners() {
  document.querySelectorAll("[data-tab]").forEach((btn) => {
    btn.addEventListener("click", () => switchTab(btn.getAttribute("data-tab")));
  });
  document.querySelectorAll("[data-nav-jump]").forEach((btn) => {
    btn.addEventListener("click", () => switchTab(btn.getAttribute("data-nav-jump")));
  });

  const toggleLang = () => applyLanguage(currentLang === "ar" ? "en" : "ar");
  const toggleTheme = () => applyTheme(currentTheme === "dark" ? "light" : "dark");
  document.getElementById("btn-lang")?.addEventListener("click", toggleLang);
  document.getElementById("btn-lang-mobile")?.addEventListener("click", toggleLang);
  document.getElementById("btn-theme")?.addEventListener("click", toggleTheme);
  document.getElementById("btn-theme-mobile")?.addEventListener("click", toggleTheme);

  document.getElementById("btn-apply-desktop")?.addEventListener("click", () =>
    postApply("/api/apply/claude-desktop", "btn-apply-desktop", "toastDesktopApplied"),
  );
  document.getElementById("btn-apply-code")?.addEventListener("click", () =>
    postApply("/api/apply/claude-code", "btn-apply-code", "toastCodeApplied"),
  );
  document.getElementById("btn-apply-dsh")?.addEventListener("click", () =>
    postApply("/api/apply/deepseek-harness", "btn-apply-dsh", "toastDshApplied"),
  );

  document.getElementById("filter-client")?.addEventListener("change", renderLogsTable);
  document.getElementById("filter-model")?.addEventListener("change", renderLogsTable);
  document.getElementById("btn-clear-logs")?.addEventListener("click", clearTelemetry);
  document.getElementById("btn-clear-telemetry")?.addEventListener("click", clearTelemetry);

  document.getElementById("btn-toggle-key")?.addEventListener("click", () => {
    const inp = document.getElementById("prov-key");
    const btn = document.getElementById("btn-toggle-key");
    if (!inp || !btn) return;
    const show = inp.type === "password";
    inp.type = show ? "text" : "password";
    btn.textContent = show ? t("btnHide") : t("btnShow");
  });

  document.getElementById("btn-save-provider")?.addEventListener("click", async () => {
    const btn = document.getElementById("btn-save-provider");
    btn.disabled = true;
    try {
      const payload = {
        upstreamBaseUrl: document.getElementById("prov-url")?.value,
        models: {
          opus: document.getElementById("prov-opus")?.value,
          sonnet: document.getElementById("prov-sonnet")?.value,
          haiku: document.getElementById("prov-haiku")?.value,
        },
        defaultReasoningEffort: document.getElementById("prov-reasoning")?.value,
        reasoningSummary: document.getElementById("prov-summary")?.value,
      };
      const keyVal = document.getElementById("prov-key")?.value?.trim();
      if (keyVal) payload.upstreamApiKey = keyVal;
      const res = await fetch(`${API_BASE}/api/provider`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) throw new Error("Failed to save provider");
      showToast(t("toastProviderSaved"));
      formHydrated = false;
      await fetchStatus();
    } catch (err) {
      showToast(err.message || "Error", true);
    } finally {
      btn.disabled = false;
    }
  });

  document.getElementById("btn-save-shield")?.addEventListener("click", async () => {
    const btn = document.getElementById("btn-save-shield");
    btn.disabled = true;
    try {
      const res = await fetch(`${API_BASE}/api/shield`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          maxUpstreamRetries: document.getElementById("shield-retries")?.value,
          minReasoningTokenHeadroom: document.getElementById("shield-headroom")?.value,
          retryBaseDelayMs: document.getElementById("shield-delay")?.value,
          maxConcurrentHeavyRequests: document.getElementById("shield-concurrency")?.value,
        }),
      });
      if (!res.ok) throw new Error("Failed to save shield settings");
      showToast(t("toastShieldSaved"));
      formHydrated = false;
      await fetchStatus();
    } catch (err) {
      showToast(err.message || "Error", true);
    } finally {
      btn.disabled = false;
    }
  });

  document.getElementById("btn-restart")?.addEventListener("click", async () => {
    try {
      await fetch(`${API_BASE}/api/restart`, { method: "POST" });
      showToast(t("toastRestarted"));
      setTimeout(() => {
        fetchStatus();
        fetchAppStatus();
      }, 1800);
    } catch (err) {
      showToast(err.message || "Error", true);
    }
  });

  document.getElementById("btn-check-update")?.addEventListener("click", () => checkForUpdates(true));
  document.getElementById("btn-check-update-card")?.addEventListener("click", () => checkForUpdates(true));
  document.getElementById("btn-install-update")?.addEventListener("click", installAppUpdate);
  document.getElementById("btn-install-update-card")?.addEventListener("click", installAppUpdate);

  ["hero-orb-btn", "btn-hero-probe", "btn-open-probe", "btn-probe-inline", "btn-rerun-probe"].forEach((id) => {
    document.getElementById(id)?.addEventListener("click", runLiveProbe);
  });

  document.getElementById("btn-hero-refresh")?.addEventListener("click", () => {
    fetchStatus();
    fetchAppStatus();
  });
  document.getElementById("btn-retry-health")?.addEventListener("click", () => {
    fetchStatus();
    fetchAppStatus();
  });

  const modal = document.getElementById("probe-modal");
  document.getElementById("btn-close-probe")?.addEventListener("click", () => modal?.classList.remove("open"));
  modal?.addEventListener("click", (e) => {
    if (e.target === modal) modal.classList.remove("open");
  });
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") modal?.classList.remove("open");
  });
}

async function initApp() {
  renderIcons();
  setupEventListeners();
  applyLanguage(currentLang);
  const tauriInvoke = window.__TAURI__?.core?.invoke || window.__TAURI_INTERNALS__?.invoke;
  if (tauriInvoke) {
    try {
      const bridgeUrl = await tauriInvoke("get_bridge_url");
      if (bridgeUrl) API_BASE = bridgeUrl;
    } catch {}
  }
  fetchStatus();
  fetchAppStatus();
  setTimeout(() => checkForUpdates(false), 2500);
  setInterval(() => {
    fetchStatus();
    fetchAppStatus();
  }, 3500);
}

window.addEventListener("DOMContentLoaded", initApp);
