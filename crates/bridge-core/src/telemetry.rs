use crate::config::{legacy_app_data_dir, telemetry_file_path, BridgeConfig};
use axum::http::HeaderMap;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BucketStats {
    pub requests: u64,
    pub errors: u64,
    pub in_tokens: u64,
    pub out_tokens: u64,
    pub cached_tokens: u64,
    pub tokens: u64,
    pub total_latency_ms: u64,
    #[serde(default)]
    pub last_seen: String,
    #[serde(default)]
    pub models: BTreeMap<String, ModelSubStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelSubStat {
    pub requests: u64,
    pub tokens: u64,
    pub cached_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogEntry {
    pub id: String,
    pub timestamp: String,
    pub client: String,
    pub model: String,
    pub status: u16,
    pub in_tokens: u64,
    pub out_tokens: u64,
    pub cached_tokens: u64,
    pub tokens: u64,
    pub latency: u64,
    #[serde(default)]
    pub is_stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeMetrics {
    pub requests_total: u64,
    pub messages_requests: u64,
    pub responses_pass_through_requests: u64,
    pub count_tokens_requests: u64,
    pub retries_total: u64,
    pub rate_limit_shield_hits: u64,
    pub errors_total: u64,
    pub input_tokens_total: u64,
    pub output_tokens_total: u64,
    pub cached_tokens_total: u64,
    pub tokens_total: u64,
    #[serde(default)]
    pub client_stats: BTreeMap<String, BucketStats>,
    #[serde(default)]
    pub model_stats: BTreeMap<String, BucketStats>,
}

impl Default for BridgeMetrics {
    fn default() -> Self {
        Self {
            requests_total: 0,
            messages_requests: 0,
            responses_pass_through_requests: 0,
            count_tokens_requests: 0,
            retries_total: 0,
            rate_limit_shield_hits: 0,
            errors_total: 0,
            input_tokens_total: 0,
            output_tokens_total: 0,
            cached_tokens_total: 0,
            tokens_total: 0,
            client_stats: BTreeMap::new(),
            model_stats: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryStore {
    pub metrics: BridgeMetrics,
    pub recent_logs: VecDeque<RequestLogEntry>,
}

impl TelemetryStore {
    pub fn load_or_migrate() -> Self {
        let path = telemetry_file_path();
        if path.exists() {
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(parsed) = serde_json::from_str::<TelemetryStore>(&raw) {
                    return parsed;
                }
            }
        }
        // Migrate telemetry from previous Aura/ClaudeSwitchPro profile or legacy Node bridge if available
        let candidates = [
            legacy_app_data_dir().join("telemetry.json"),
            PathBuf::from(
                r"H:\Projects\Backend-API\Active\claude-responses-bridge\logs\telemetry.json",
            ),
        ];
        for legacy in candidates {
            if legacy.exists() {
                if let Ok(raw) = fs::read_to_string(&legacy) {
                    if let Ok(parsed) = serde_json::from_str::<TelemetryStore>(&raw) {
                        return parsed;
                    }
                }
            }
        }
        Self::default()
    }

    pub fn persist(&self) {
        let path = telemetry_file_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }

    pub fn add_log(
        &mut self,
        id: String,
        client: &str,
        model: &str,
        status: u16,
        in_tokens: u64,
        out_tokens: u64,
        cached_tokens: u64,
        latency: u64,
        is_stream: bool,
    ) {
        let total = in_tokens + out_tokens;
        let now = Utc::now().to_rfc3339();
        let entry = RequestLogEntry {
            id,
            timestamp: now.clone(),
            client: client.to_string(),
            model: model.to_string(),
            status,
            in_tokens,
            out_tokens,
            cached_tokens,
            tokens: total,
            latency,
            is_stream,
        };
        self.recent_logs.push_front(entry);
        while self.recent_logs.len() > 80 {
            self.recent_logs.pop_back();
        }

        self.metrics.input_tokens_total += in_tokens;
        self.metrics.output_tokens_total += out_tokens;
        self.metrics.cached_tokens_total += cached_tokens;
        self.metrics.tokens_total += total;

        let ms = self.metrics.model_stats.entry(model.to_string()).or_default();
        ms.requests += 1;
        if status >= 400 {
            ms.errors += 1;
        }
        ms.in_tokens += in_tokens;
        ms.out_tokens += out_tokens;
        ms.cached_tokens += cached_tokens;
        ms.tokens += total;
        ms.total_latency_ms += latency;

        let cs = self.metrics.client_stats.entry(client.to_string()).or_default();
        cs.requests += 1;
        if status >= 400 {
            cs.errors += 1;
        }
        cs.in_tokens += in_tokens;
        cs.out_tokens += out_tokens;
        cs.cached_tokens += cached_tokens;
        cs.tokens += total;
        cs.total_latency_ms += latency;
        cs.last_seen = now;
        let sub = cs.models.entry(model.to_string()).or_default();
        sub.requests += 1;
        sub.tokens += total;
        sub.cached_tokens += cached_tokens;

        self.persist();
    }

    pub fn settle_stream_usage(&mut self, log_id: &str, resp_usage: &Value) {
        let Some(u) = resp_usage.as_object() else {
            return;
        };
        let Some(entry) = self.recent_logs.iter_mut().find(|l| l.id == log_id) else {
            return;
        };

        let cached = u
            .get("input_tokens_details")
            .and_then(|d| d.get("cached_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let cache_write = u
            .get("input_tokens_details")
            .and_then(|d| d.get("cache_write_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let in_tok = u
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(entry.in_tokens)
            .saturating_sub(cached)
            .saturating_sub(cache_write);
        let out_tok = u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0);

        let d_in = in_tok as i64 - entry.in_tokens as i64;
        let d_out = out_tok as i64 - entry.out_tokens as i64;
        let d_cache = cached as i64 - entry.cached_tokens as i64;
        let d_tot = (in_tok + out_tok) as i64 - entry.tokens as i64;

        entry.in_tokens = in_tok;
        entry.out_tokens = out_tok;
        entry.cached_tokens = cached;
        entry.tokens = in_tok + out_tok;

        let model = entry.model.clone();
        let client = entry.client.clone();

        self.metrics.input_tokens_total = add_signed(self.metrics.input_tokens_total, d_in);
        self.metrics.output_tokens_total = add_signed(self.metrics.output_tokens_total, d_out);
        self.metrics.cached_tokens_total = add_signed(self.metrics.cached_tokens_total, d_cache);
        self.metrics.tokens_total = add_signed(self.metrics.tokens_total, d_tot);

        if let Some(ms) = self.metrics.model_stats.get_mut(&model) {
            ms.in_tokens = add_signed(ms.in_tokens, d_in);
            ms.out_tokens = add_signed(ms.out_tokens, d_out);
            ms.cached_tokens = add_signed(ms.cached_tokens, d_cache);
            ms.tokens = add_signed(ms.tokens, d_tot);
        }
        if let Some(cs) = self.metrics.client_stats.get_mut(&client) {
            cs.in_tokens = add_signed(cs.in_tokens, d_in);
            cs.out_tokens = add_signed(cs.out_tokens, d_out);
            cs.cached_tokens = add_signed(cs.cached_tokens, d_cache);
            cs.tokens = add_signed(cs.tokens, d_tot);
            if let Some(sub) = cs.models.get_mut(&model) {
                sub.tokens = add_signed(sub.tokens, d_tot);
                sub.cached_tokens = add_signed(sub.cached_tokens, d_cache);
            }
        }
        self.persist();
    }

    pub fn clear(&mut self) {
        self.recent_logs.clear();
        self.metrics.requests_total = 0;
        self.metrics.messages_requests = 0;
        self.metrics.responses_pass_through_requests = 0;
        self.metrics.retries_total = 0;
        self.metrics.rate_limit_shield_hits = 0;
        self.metrics.errors_total = 0;
        self.metrics.input_tokens_total = 0;
        self.metrics.output_tokens_total = 0;
        self.metrics.cached_tokens_total = 0;
        self.metrics.tokens_total = 0;
        self.metrics.client_stats.clear();
        for ms in self.metrics.model_stats.values_mut() {
            *ms = BucketStats::default();
        }
        self.persist();
    }
}

fn add_signed(base: u64, delta: i64) -> u64 {
    if delta >= 0 {
        base.saturating_add(delta as u64)
    } else {
        base.saturating_sub((-delta) as u64)
    }
}

pub fn detect_client_agent(
    headers: &HeaderMap,
    requested_model: &str,
    upstream_model: &str,
    is_responses_api: bool,
) -> &'static str {
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let x_app = headers
        .get("x-app")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let has_session = headers.contains_key("x-claude-code-session-id");
    let req_m = requested_model.to_ascii_lowercase();
    let up_m = upstream_model.to_ascii_lowercase();

    if ua.contains("deepseek") || ua.contains("dsh") || x_app.contains("dsh") {
        return "DeepSeek Harness";
    }
    if ua.contains("claude-cli") || has_session {
        // NOTE: Haiku-tier check matches bridge agent routing without leaking config reference here
        if req_m.contains("haiku") || up_m.contains("5.6") {
            return "Claude Code Subagent";
        }
        return "Claude Code CLI";
    }
    if ua.contains("claude") || ua.contains("anthropic") {
        return "Claude Desktop";
    }
    if is_responses_api
        && (ua.contains("openai") || ua.contains("stainless") || ua.contains("node"))
    {
        return "DeepSeek Harness";
    }
    if is_responses_api {
        "Responses Client"
    } else {
        "Claude Code CLI"
    }
}

pub fn detect_client_agent_with_config(
    headers: &HeaderMap,
    requested_model: &str,
    upstream_model: &str,
    is_responses_api: bool,
    config: &BridgeConfig,
) -> &'static str {
    let base = detect_client_agent(headers, requested_model, upstream_model, is_responses_api);
    if base == "Claude Code CLI"
        && !upstream_model.is_empty()
        && upstream_model.eq_ignore_ascii_case(&config.models.haiku)
    {
        return "Claude Code Subagent";
    }
    base
}

