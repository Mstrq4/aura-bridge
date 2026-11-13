use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    pub opus: String,
    pub sonnet: String,
    pub haiku: String,
    pub default: String,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            opus: "gpt-6.1-sol".to_string(),
            sonnet: "gpt-6-astra".to_string(),
            haiku: "gpt-5.6-sol".to_string(),
            default: "gpt-6.1-sol".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeConfig {
    pub host: String,
    pub port: u16,
    pub upstream_base_url: String,
    pub upstream_api_key: String,
    pub upstream_user_agent: String,
    pub models: ModelConfig,
    pub default_reasoning_effort: String,
    pub reasoning_summary: String,
    pub min_reasoning_token_headroom: u64,
    pub max_upstream_retries: u32,
    pub retry_base_delay_ms: u64,
    pub retry_max_delay_ms: u64,
    pub upstream_timeout_ms: u64,
    pub max_concurrent_heavy_requests: usize,
    pub min_heavy_pacing_ms: u64,
    pub log_level: String,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 18787,
            upstream_base_url: "https://agentrouter.org".to_string(),
            upstream_api_key: String::new(),
            upstream_user_agent: "claude-cli/2.1.302 (external, sdk-cli, agent-sdk/0.3.316)"
                .to_string(),
            models: ModelConfig::default(),
            default_reasoning_effort: "medium".to_string(),
            reasoning_summary: "auto".to_string(),
            min_reasoning_token_headroom: 4096,
            max_upstream_retries: 6,
            retry_base_delay_ms: 6000,
            retry_max_delay_ms: 45000,
            upstream_timeout_ms: 240000,
            max_concurrent_heavy_requests: 2,
            min_heavy_pacing_ms: 1200,
            log_level: "info".to_string(),
        }
    }
}

pub fn user_home_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Users\Default"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }
}

pub fn legacy_app_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| user_home_dir().join("AppData").join("Local"))
            .join("ClaudeSwitchPro")
    }
    #[cfg(target_os = "macos")]
    {
        user_home_dir()
            .join("Library")
            .join("Application Support")
            .join("ClaudeSwitchPro")
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| user_home_dir().join(".local").join("share"))
            .join("ClaudeSwitchPro")
    }
}

pub fn app_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| user_home_dir().join("AppData").join("Local"))
            .join("AuraBridge")
    }
    #[cfg(target_os = "macos")]
    {
        user_home_dir()
            .join("Library")
            .join("Application Support")
            .join("AuraBridge")
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| user_home_dir().join(".local").join("share"))
            .join("AuraBridge")
    }
}

pub fn config_env_path() -> PathBuf {
    app_data_dir().join(".env")
}

pub fn telemetry_file_path() -> PathBuf {
    app_data_dir().join("telemetry.json")
}

fn parse_dotenv_file(path: &Path) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Ok(raw) = fs::read_to_string(path) else {
        return map;
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim();
            let mut val = v.trim();
            if (val.starts_with('"') && val.ends_with('"'))
                || (val.starts_with('\'') && val.ends_with('\''))
            {
                if val.len() >= 2 {
                    val = &val[1..val.len() - 1];
                }
            }
            map.insert(key.to_string(), val.to_string());
        }
    }
    map
}

fn read_claude_settings_token_fallback() -> String {
    let settings_path = user_home_dir().join(".claude").join("settings.json");
    let Ok(raw) = fs::read_to_string(settings_path) else {
        return String::new();
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return String::new();
    };
    let token = parsed
        .get("env")
        .and_then(|e| {
            e.get("ANTHROPIC_AUTH_TOKEN")
                .or_else(|| e.get("ANTHROPIC_API_KEY"))
        })
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    token.to_string()
}

pub fn maybe_migrate_legacy_env() {
    let dest = config_env_path();
    if dest.exists() {
        return;
    }
    let _ = fs::create_dir_all(app_data_dir());
    // Migrate seamlessly from previous ClaudeSwitchPro profile, install dir, or legacy Node bridge
    let mut candidates = vec![legacy_app_data_dir().join(".env")];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let l = PathBuf::from(local);
        candidates.push(l.join("Programs").join("AuraBridge").join(".env"));
        candidates.push(l.join("Programs").join("ClaudeSwitchPro").join(".env"));
    }
    candidates.push(PathBuf::from(r"H:\Projects\Backend-API\Active\claude-responses-bridge\.env"));
    for legacy in candidates {
        if legacy.exists() {
            let _ = fs::copy(&legacy, &dest);
            break;
        }
    }
}

pub fn load_config() -> BridgeConfig {
    maybe_migrate_legacy_env();
    let file_env = parse_dotenv_file(&config_env_path());

    let get_str = |key: &str, default: &str| -> String {
        if let Ok(v) = std::env::var(key) {
            if !v.trim().is_empty() {
                return v;
            }
        }
        if let Some(v) = file_env.get(key) {
            if !v.trim().is_empty() {
                return v.clone();
            }
        }
        default.to_string()
    };

    let get_u64 = |key: &str, default: u64| -> u64 {
        get_str(key, "")
            .parse::<u64>()
            .ok()
            .unwrap_or(default)
    };

    let explicit_key = get_str("UPSTREAM_API_KEY", "");
    let upstream_api_key = if !explicit_key.is_empty() && explicit_key != "replace-with-your-api-key"
    {
        explicit_key
    } else {
        read_claude_settings_token_fallback()
    };

    let opus = get_str("MODEL_OPUS", "gpt-6.1-sol");
    let mut cfg = BridgeConfig {
        host: get_str("BRIDGE_HOST", "127.0.0.1"),
        port: get_u64("BRIDGE_PORT", 18787) as u16,
        upstream_base_url: get_str("UPSTREAM_BASE_URL", "https://agentrouter.org")
            .trim_end_matches('/')
            .to_string(),
        upstream_api_key,
        upstream_user_agent: get_str(
            "UPSTREAM_USER_AGENT",
            "claude-cli/2.1.302 (external, sdk-cli, agent-sdk/0.3.316)",
        ),
        models: ModelConfig {
            opus: opus.clone(),
            sonnet: get_str("MODEL_SONNET", "gpt-6-astra"),
            haiku: get_str("MODEL_HAIKU", "gpt-5.6-sol"),
            default: get_str("MODEL_DEFAULT", &opus),
        },
        default_reasoning_effort: get_str("DEFAULT_REASONING_EFFORT", "medium"),
        reasoning_summary: get_str("REASONING_SUMMARY", "auto"),
        min_reasoning_token_headroom: get_u64("MIN_REASONING_TOKEN_HEADROOM", 4096),
        max_upstream_retries: get_u64("MAX_UPSTREAM_RETRIES", 6) as u32,
        retry_base_delay_ms: get_u64("RETRY_BASE_DELAY_MS", 6000),
        retry_max_delay_ms: get_u64("RETRY_MAX_DELAY_MS", 45000),
        upstream_timeout_ms: get_u64("UPSTREAM_TIMEOUT_MS", 240000),
        max_concurrent_heavy_requests: (get_u64("MAX_CONCURRENT_HEAVY_REQUESTS", 2) as usize).max(1),
        min_heavy_pacing_ms: get_u64("MIN_HEAVY_PACING_MS", 1200),
        log_level: get_str("LOG_LEVEL", "info"),
    };
    if cfg.models.default.is_empty() {
        cfg.models.default = cfg.models.opus.clone();
    }
    cfg
}

pub fn persist_env_updates(updates: &BTreeMap<String, String>) -> std::io::Result<()> {
    let dir = app_data_dir();
    fs::create_dir_all(&dir)?;
    let env_path = config_env_path();
    let existing = fs::read_to_string(&env_path).unwrap_or_default();
    let mut remaining = updates.clone();
    let mut next_lines = Vec::new();

    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            next_lines.push(line.to_string());
            continue;
        }
        if let Some((k, _)) = trimmed.split_once('=') {
            let key = k.trim();
            if let Some(val) = remaining.remove(key) {
                next_lines.push(format!("{key}={val}"));
                continue;
            }
        }
        next_lines.push(line.to_string());
    }

    for (k, v) in remaining {
        next_lines.push(format!("{k}={v}"));
    }

    let mut content = next_lines.join("\n");
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    fs::write(env_path, content)
}

pub fn mask_api_key(key: &str) -> String {
    let s = key.trim();
    if s.is_empty() || s == "replace-with-your-api-key" {
        return String::new();
    }
    if s.chars().count() <= 10 {
        return "••••".to_string();
    }
    let head: String = s.chars().take(7).collect();
    let tail: String = s
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head}••••••{tail}")
}

pub fn redact_secrets(input: &str) -> String {
    static RE_SK: OnceLock<Regex> = OnceLock::new();
    static RE_BEARER: OnceLock<Regex> = OnceLock::new();
    let re_sk = RE_SK.get_or_init(|| Regex::new(r"sk-[A-Za-z0-9_-]{8,}").unwrap());
    let re_bearer =
        RE_BEARER.get_or_init(|| Regex::new(r"(?i)(Bearer\s+)[A-Za-z0-9._-]{8,}").unwrap());

    let step1 = re_sk.replace_all(input, "sk-***REDACTED***");
    re_bearer
        .replace_all(&step1, "${1}***REDACTED***")
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_tokens_and_bearers() {
        let msg = "Authorization: Bearer sk-abc1234567890xyz failed for sk-test99999999";
        let clean = redact_secrets(msg);
        assert!(!clean.contains("sk-abc1234567890xyz"));
        assert!(!clean.contains("sk-test99999999"));
        assert!(clean.contains("***REDACTED***"));
    }
}
