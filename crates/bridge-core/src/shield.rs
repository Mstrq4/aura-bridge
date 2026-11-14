use crate::config::{redact_secrets, BridgeConfig};
use crate::stream::{SseFrame, SseFrameParser};
use futures_util::StreamExt;
use regex::Regex;
use reqwest::header::HeaderMap;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, Notify};

#[derive(Debug, Clone)]
pub struct UpstreamBridgeError {
    pub message: String,
    pub status: u16,
    pub is_rate_limit: bool,
    pub retry_after: Option<u64>,
    pub upstream_status: Option<u16>,
    pub code: Option<String>,
}

impl std::fmt::Display for UpstreamBridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for UpstreamBridgeError {}

#[derive(Debug, Clone)]
pub struct FailureClassification {
    pub is_retryable: bool,
    pub is_rate_limit: bool,
    pub mapped_status: u16,
}

pub fn classify_upstream_failure(
    http_status: Option<u16>,
    raw_body: &str,
    err_code: Option<&str>,
) -> FailureClassification {
    static RE_RATE: OnceLock<Regex> = OnceLock::new();
    static RE_TRANSIENT: OnceLock<Regex> = OnceLock::new();
    let re_rate = RE_RATE.get_or_init(|| {
        Regex::new(r"(?i)(too_many_requests|rate[\s_-]?limit|exceeded.*rate limit|quota.*exceeded|tokens per minute|requests per minute|Responses upstream too_many_requests)").unwrap()
    });
    let re_transient = RE_TRANSIENT.get_or_init(|| {
        Regex::new(r"(?i)(overloaded|server_error|bad gateway|service unavailable|gateway timeout|timed?\s*out|ECONNRESET|ETIMEDOUT|socket hang up|fetch failed|upstream.*error|error decoding response body|unexpected eof|connection closed|broken pipe|reset by peer|incomplete message|stream error)").unwrap()
    });

    let code_l = err_code.unwrap_or("").to_ascii_lowercase();
    let combined = format!("{} {raw_body}", err_code.unwrap_or(""));

    if http_status == Some(429)
        || code_l == "too_many_requests"
        || code_l == "rate_limit_exceeded"
        || re_rate.is_match(&combined)
    {
        return FailureClassification {
            is_retryable: true,
            is_rate_limit: true,
            mapped_status: 429,
        };
    }

    let transient_status = matches!(
        http_status,
        Some(408 | 500 | 502 | 503 | 504 | 520 | 522 | 524)
    );
    let transient_text =
        matches!(http_status, None | Some(0 | 422) | Some(500..=599)) && re_transient.is_match(&combined);

    if transient_status || transient_text {
        return FailureClassification {
            is_retryable: true,
            is_rate_limit: false,
            mapped_status: http_status.filter(|&s| s >= 500).unwrap_or(502),
        };
    }

    FailureClassification {
        is_retryable: false,
        is_rate_limit: false,
        mapped_status: http_status.filter(|&s| s >= 400).unwrap_or(500),
    }
}

pub fn extract_retry_after_seconds(headers: Option<&HeaderMap>, raw_body: &str) -> Option<u64> {
    if let Some(hdrs) = headers {
        if let Some(v) = hdrs.get("retry-after").and_then(|v| v.to_str().ok()) {
            if let Ok(secs) = v.trim().parse::<f64>() {
                if secs.is_finite() && secs > 0.0 {
                    return Some(secs.ceil() as u64);
                }
            }
        }
        if let Some(v) = hdrs.get("x-ms-retry-after-ms").and_then(|v| v.to_str().ok()) {
            if let Ok(ms) = v.trim().parse::<f64>() {
                if ms.is_finite() && ms > 0.0 {
                    return Some((ms / 1000.0).ceil() as u64);
                }
            }
        }
    }
    static RE_RETRY: OnceLock<Regex> = OnceLock::new();
    let re = RE_RETRY.get_or_init(|| {
        Regex::new(r"(?i)(?:retry after|try again in)\s+(\d+(?:\.\d+)?)\s*(?:s|sec|seconds?)")
            .unwrap()
    });
    if let Some(caps) = re.captures(raw_body) {
        if let Ok(secs) = caps[1].parse::<f64>() {
            if secs.is_finite() && secs > 0.0 {
                return Some(secs.ceil() as u64);
            }
        }
    }
    None
}

pub fn compute_backoff_delay_ms(
    attempt: u32,
    config: &BridgeConfig,
    retry_after_secs: Option<u64>,
) -> u64 {
    let base = config.retry_base_delay_ms.max(10) as f64;
    let max_delay = config.retry_max_delay_ms.max(base as u64) as f64;
    let exp = (base * 1.85_f64.powi(attempt.saturating_sub(1) as i32)).min(max_delay);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as f64 / 1_000_000_000.0)
        .unwrap_or(0.5);
    let jitter = exp * (0.08 + nanos * 0.14);
    let mut computed = (exp + jitter).min(max_delay).round() as u64;

    if let Some(secs) = retry_after_secs {
        if secs > 0 {
            let hinted = secs.saturating_mul(1000).saturating_add(500);
            computed = computed.max(hinted).min((max_delay as u64).max(60_000));
        }
    }
    computed
}

fn is_stream_commit_event(evt_type: &str) -> bool {
    matches!(
        evt_type,
        "response.output_item.added"
            | "response.content_part.added"
            | "response.output_text.delta"
            | "response.reasoning_summary_text.delta"
            | "response.reasoning_text.delta"
            | "response.function_call_arguments.delta"
            | "response.output_item.done"
            | "response.completed"
            | "response.incomplete"
    )
}

fn extract_sse_error_details(evt: &Value) -> Option<(String, String)> {
    let obj = evt.as_object()?;
    match obj.get("type").and_then(Value::as_str)? {
        "error" => {
            let err = obj.get("error").unwrap_or(evt);
            let code = err
                .get("code")
                .or_else(|| err.get("type"))
                .and_then(Value::as_str)
                .unwrap_or("stream_error")
                .to_string();
            let message = err
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| evt.to_string());
            Some((code, message))
        }
        "response.failed" => {
            let err = obj
                .get("response")
                .and_then(|r| r.get("error"))
                .or_else(|| obj.get("error"))
                .unwrap_or(&Value::Null);
            let code = err
                .get("code")
                .or_else(|| err.get("type"))
                .and_then(Value::as_str)
                .unwrap_or("response_failed")
                .to_string();
            let message = err
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| err.to_string());
            Some((code, message))
        }
        _ => None,
    }
}

#[derive(Default)]
struct GateInner {
    active: usize,
    last_dispatched: Option<Instant>,
}

#[derive(Clone)]
pub struct HeavyRequestGate {
    inner: Arc<Mutex<GateInner>>,
    notify: Arc<Notify>,
}

pub struct GateGuard {
    inner: Arc<Mutex<GateInner>>,
    notify: Arc<Notify>,
    active: bool,
}

impl Drop for GateGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let inner_bg = self.inner.clone();
        let notify_bg = self.notify.clone();
        if let Ok(mut guard) = self.inner.try_lock() {
            guard.active = guard.active.saturating_sub(1);
            self.notify.notify_one();
        } else {
            tokio::spawn(async move {
                let mut guard = inner_bg.lock().await;
                guard.active = guard.active.saturating_sub(1);
                notify_bg.notify_one();
            });
        }
    }
}

impl HeavyRequestGate {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(GateInner::default())),
            notify: Arc::new(Notify::new()),
        }
    }

    pub async fn acquire(&self, is_heavy: bool, config: &BridgeConfig) -> GateGuard {
        if !is_heavy {
            return GateGuard {
                inner: self.inner.clone(),
                notify: self.notify.clone(),
                active: false,
            };
        }
        let max_concurrent = config.max_concurrent_heavy_requests.max(1);
        let min_pacing = Duration::from_millis(config.min_heavy_pacing_ms);

        loop {
            let mut guard = self.inner.lock().await;
            if guard.active < max_concurrent {
                guard.active += 1;
                let wait_dur = guard.last_dispatched.and_then(|last| {
                    let target = last + min_pacing;
                    let now = Instant::now();
                    (target > now).then_some(target - now)
                });
                drop(guard);
                if let Some(d) = wait_dur {
                    tokio::time::sleep(d).await;
                }
                let mut guard2 = self.inner.lock().await;
                guard2.last_dispatched = Some(Instant::now());
                return GateGuard {
                    inner: self.inner.clone(),
                    notify: self.notify.clone(),
                    active: true,
                };
            }
            drop(guard);
            self.notify.notified().await;
        }
    }
}

pub enum StreamCallbackEvent {
    Commit,
    Frame(SseFrame),
    Error(UpstreamBridgeError),
}

#[derive(Clone)]
pub struct UpstreamClient {
    http: Client,
    gate: HeavyRequestGate,
}

fn decode_utf8_chunk(pending: &mut Vec<u8>, chunk: &[u8]) -> String {
    pending.extend_from_slice(chunk);
    let mut out = String::new();
    let mut idx = 0usize;
    while idx < pending.len() {
        match std::str::from_utf8(&pending[idx..]) {
            Ok(valid) => {
                out.push_str(valid);
                idx = pending.len();
            }
            Err(err) => {
                let valid_up_to = err.valid_up_to();
                if valid_up_to > 0 {
                    out.push_str(
                        std::str::from_utf8(&pending[idx..idx + valid_up_to]).unwrap_or_default(),
                    );
                    idx += valid_up_to;
                }
                match err.error_len() {
                    Some(bad_len) => {
                        out.push('\u{FFFD}');
                        idx += bad_len;
                    }
                    None => break, // Incomplete trailing multi-byte UTF-8 sequence; wait for next chunk
                }
            }
        }
    }
    if idx > 0 {
        pending.drain(..idx);
    }
    out
}

fn flush_utf8_pending(pending: &mut Vec<u8>) -> String {
    if pending.is_empty() {
        return String::new();
    }
    let text = String::from_utf8_lossy(pending).into_owned();
    pending.clear();
    text
}

impl UpstreamClient {
    pub fn new() -> Self {
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .tcp_keepalive(Some(Duration::from_secs(15)))
            // Disable stale HTTP keep-alive socket reuse so long SSE/WAF edges never fail on half-closed sockets
            .pool_max_idle_per_host(0)
            .build()
            .expect("reqwest client build failed");
        Self {
            http,
            gate: HeavyRequestGate::new(),
        }
    }

    pub async fn request_json<F>(
        &self,
        config: &BridgeConfig,
        payload: &Value,
        api_key: &str,
        is_heavy: bool,
        mut on_retry: F,
    ) -> Result<Value, UpstreamBridgeError>
    where
        F: FnMut(bool),
    {
        let url = format!("{}/v1/responses", config.upstream_base_url);
        let max_attempts = config.max_upstream_retries.saturating_add(1).max(1);
        let _guard = self.gate.acquire(is_heavy, config).await;

        let mut req_payload = payload.clone();
        if let Some(obj) = req_payload.as_object_mut() {
            obj.insert("stream".to_string(), Value::Bool(false));
        }

        let mut last_err: Option<UpstreamBridgeError> = None;
        for attempt in 1..=max_attempts {
            let mut builder = self
                .http
                .post(&url)
                .timeout(Duration::from_millis(config.upstream_timeout_ms))
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .header("Accept-Encoding", "identity")
                .header("User-Agent", &config.upstream_user_agent);
            if !api_key.is_empty() {
                builder = builder.header("Authorization", format!("Bearer {api_key}"));
            }

            match builder.json(&req_payload).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let hdrs = resp.headers().clone();
                    let text = match resp.text().await {
                        Ok(t) => t,
                        Err(e) => {
                            let msg = e.to_string();
                            let cls = classify_upstream_failure(None, &msg, None);
                            let err = UpstreamBridgeError {
                                message: format!("Response decode error: {}", redact_secrets(&msg)),
                                status: 502,
                                is_rate_limit: cls.is_rate_limit,
                                retry_after: None,
                                upstream_status: Some(status),
                                code: None,
                            };
                            if !cls.is_retryable || attempt == max_attempts {
                                return Err(err);
                            }
                            on_retry(cls.is_rate_limit);
                            let delay = compute_backoff_delay_ms(attempt, config, None);
                            last_err = Some(err);
                            tokio::time::sleep(Duration::from_millis(delay)).await;
                            continue;
                        }
                    };
                    if (200..300).contains(&status) {
                        return serde_json::from_str::<Value>(&text).map_err(|e| {
                            UpstreamBridgeError {
                                message: format!("Invalid upstream JSON: {e}"),
                                status: 502,
                                is_rate_limit: false,
                                retry_after: None,
                                upstream_status: Some(status),
                                code: None,
                            }
                        });
                    }
                    let retry_after = extract_retry_after_seconds(Some(&hdrs), &text);
                    let cls = classify_upstream_failure(Some(status), &text, None);
                    let err = UpstreamBridgeError {
                        message: redact_secrets(
                            &text
                                .chars()
                                .take(800)
                                .collect::<String>()
                                .if_empty(|| format!("Upstream HTTP {status}")),
                        ),
                        status: cls.mapped_status,
                        is_rate_limit: cls.is_rate_limit,
                        retry_after: retry_after.or(cls.is_rate_limit.then_some(15)),
                        upstream_status: Some(status),
                        code: None,
                    };
                    if !cls.is_retryable || attempt == max_attempts {
                        return Err(err);
                    }
                    on_retry(cls.is_rate_limit);
                    let delay = compute_backoff_delay_ms(attempt, config, retry_after);
                    last_err = Some(err);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
                Err(e) => {
                    let msg = e.to_string();
                    let cls = classify_upstream_failure(None, &msg, None);
                    let err = UpstreamBridgeError {
                        message: format!("Upstream network failure: {}", redact_secrets(&msg)),
                        status: 502,
                        is_rate_limit: cls.is_rate_limit,
                        retry_after: None,
                        upstream_status: None,
                        code: None,
                    };
                    if !cls.is_retryable || attempt == max_attempts {
                        return Err(err);
                    }
                    on_retry(cls.is_rate_limit);
                    let delay = compute_backoff_delay_ms(attempt, config, None);
                    last_err = Some(err);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }
        }

        Err(last_err.unwrap_or(UpstreamBridgeError {
            message: "Upstream retries exhausted".to_string(),
            status: 502,
            is_rate_limit: false,
            retry_after: None,
            upstream_status: None,
            code: None,
        }))
    }

    pub async fn request_stream<F>(
        &self,
        config: &BridgeConfig,
        payload: &Value,
        api_key: &str,
        is_heavy: bool,
        tx: mpsc::Sender<StreamCallbackEvent>,
        mut on_retry: F,
    ) where
        F: FnMut(bool) + Send,
    {
        let url = format!("{}/v1/responses", config.upstream_base_url);
        let max_attempts = config.max_upstream_retries.saturating_add(1).max(1);
        let _guard = self.gate.acquire(is_heavy, config).await;

        let mut req_payload = payload.clone();
        if let Some(obj) = req_payload.as_object_mut() {
            obj.insert("stream".to_string(), Value::Bool(true));
        }

        let mut last_err: Option<UpstreamBridgeError> = None;

        let chunk_timeout = Duration::from_millis(config.upstream_timeout_ms.max(15_000));

        for attempt in 1..=max_attempts {
            let mut builder = self
                .http
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Accept", "text/event-stream")
                .header("Accept-Encoding", "identity")
                .header("Connection", "close")
                .header("User-Agent", &config.upstream_user_agent);
            if !api_key.is_empty() {
                builder = builder.header("Authorization", format!("Bearer {api_key}"));
            }

            let mut committed = false;
            let send_fut = tokio::time::timeout(chunk_timeout, builder.json(&req_payload).send());
            match send_fut.await {
                Err(_) => {
                    let err = UpstreamBridgeError {
                        message: "Upstream stream connect timed out".to_string(),
                        status: 504,
                        is_rate_limit: false,
                        retry_after: None,
                        upstream_status: None,
                        code: None,
                    };
                    if attempt == max_attempts {
                        let _ = tx.send(StreamCallbackEvent::Error(err)).await;
                        return;
                    }
                    on_retry(false);
                    let delay = compute_backoff_delay_ms(attempt, config, None);
                    last_err = Some(err);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    continue;
                }
                Ok(Ok(resp)) => {
                    let status = resp.status().as_u16();
                    let hdrs = resp.headers().clone();
                    if !(200..300).contains(&status) {
                        let text = resp.text().await.unwrap_or_default();
                        let retry_after = extract_retry_after_seconds(Some(&hdrs), &text);
                        let cls = classify_upstream_failure(Some(status), &text, None);
                        let err = UpstreamBridgeError {
                            message: redact_secrets(
                                &text
                                    .chars()
                                    .take(800)
                                    .collect::<String>()
                                    .if_empty(|| format!("Upstream HTTP {status}")),
                            ),
                            status: cls.mapped_status,
                            is_rate_limit: cls.is_rate_limit,
                            retry_after: retry_after.or(cls.is_rate_limit.then_some(15)),
                            upstream_status: Some(status),
                            code: None,
                        };
                        if !cls.is_retryable || attempt == max_attempts {
                            let _ = tx.send(StreamCallbackEvent::Error(err)).await;
                            return;
                        }
                        on_retry(cls.is_rate_limit);
                        let delay = compute_backoff_delay_ms(attempt, config, retry_after);
                        last_err = Some(err);
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        continue;
                    }

                    let mut parser = SseFrameParser::default();
                    let mut utf8_pending: Vec<u8> = Vec::new();
                    let mut primed: Vec<SseFrame> = Vec::new();
                    let mut pre_commit_failure: Option<UpstreamBridgeError> = None;
                    let mut terminal_event_seen = false;
                    let mut byte_stream = resp.bytes_stream();

                    loop {
                        let next_item = tokio::time::timeout(chunk_timeout, byte_stream.next()).await;
                        let Some(chunk_res) = (match next_item {
                            Ok(item) => item,
                            Err(_) => {
                                if terminal_event_seen {
                                    break;
                                }
                                let err = UpstreamBridgeError {
                                    message: "Stream read error: chunk timed out".to_string(),
                                    status: 504,
                                    is_rate_limit: false,
                                    retry_after: None,
                                    upstream_status: None,
                                    code: None,
                                };
                                if committed {
                                    let _ = tx.send(StreamCallbackEvent::Error(err)).await;
                                    return;
                                }
                                pre_commit_failure = Some(err);
                                break;
                            }
                        }) else {
                            break;
                        };
                        match chunk_res {
                            Ok(bytes) => {
                                let text = decode_utf8_chunk(&mut utf8_pending, &bytes);
                                for frame in parser.push(&text) {
                                    if pre_commit_failure.is_some() {
                                        continue;
                                    }
                                    if frame.done {
                                        terminal_event_seen = true;
                                    } else if let Some(ref dobj) = frame.data_obj {
                                        let typ = dobj.get("type").and_then(Value::as_str).unwrap_or("");
                                        if matches!(
                                            typ,
                                            "response.completed"
                                                | "response.incomplete"
                                                | "response.failed"
                                                | "message_stop"
                                        ) {
                                            terminal_event_seen = true;
                                        }
                                    }

                                    if !committed {
                                        if frame.done {
                                            committed = true;
                                            let _ = tx.send(StreamCallbackEvent::Commit).await;
                                            for pf in primed.drain(..) {
                                                let _ = tx.send(StreamCallbackEvent::Frame(pf)).await;
                                            }
                                            let _ = tx.send(StreamCallbackEvent::Frame(frame)).await;
                                            continue;
                                        }
                                        if let Some(ref dobj) = frame.data_obj {
                                            if let Some((code, msg)) = extract_sse_error_details(dobj) {
                                                let cls = classify_upstream_failure(
                                                    None,
                                                    &msg,
                                                    Some(&code),
                                                );
                                                let ra = extract_retry_after_seconds(Some(&hdrs), &msg);
                                                pre_commit_failure = Some(UpstreamBridgeError {
                                                    message: redact_secrets(&msg),
                                                    status: cls.mapped_status,
                                                    is_rate_limit: cls.is_rate_limit,
                                                    retry_after: ra,
                                                    upstream_status: Some(if cls.is_rate_limit { 429 } else { 502 }),
                                                    code: Some(code),
                                                });
                                                break;
                                            }
                                        }
                                        let evt_type = frame
                                            .data_obj
                                            .as_ref()
                                            .and_then(|d| d.get("type"))
                                            .and_then(Value::as_str)
                                            .or(frame.event_name.as_deref())
                                            .unwrap_or("");
                                        if is_stream_commit_event(evt_type) {
                                            committed = true;
                                            let _ = tx.send(StreamCallbackEvent::Commit).await;
                                            for pf in primed.drain(..) {
                                                let _ = tx.send(StreamCallbackEvent::Frame(pf)).await;
                                            }
                                            let _ = tx.send(StreamCallbackEvent::Frame(frame)).await;
                                        } else {
                                            primed.push(frame);
                                        }
                                    } else if tx.send(StreamCallbackEvent::Frame(frame)).await.is_err()
                                    {
                                        return;
                                    }
                                }
                                if pre_commit_failure.is_some() {
                                    break;
                                }
                            }
                            Err(e) => {
                                // Ignore benign socket teardown EOF/decode glitches once terminal completion frame arrived
                                if terminal_event_seen {
                                    break;
                                }
                                let msg = e.to_string();
                                let cls = classify_upstream_failure(None, &msg, None);
                                let err = UpstreamBridgeError {
                                    message: format!("Stream read error: {}", redact_secrets(&msg)),
                                    status: 502,
                                    is_rate_limit: cls.is_rate_limit,
                                    retry_after: None,
                                    upstream_status: None,
                                    code: None,
                                };
                                if committed {
                                    let _ = tx.send(StreamCallbackEvent::Error(err)).await;
                                    return;
                                }
                                pre_commit_failure = Some(err);
                                break;
                            }
                        }
                    }

                    if pre_commit_failure.is_none() {
                        let tail = flush_utf8_pending(&mut utf8_pending);
                        if !tail.is_empty() {
                            for frame in parser.push(&tail) {
                                if !committed {
                                    committed = true;
                                    let _ = tx.send(StreamCallbackEvent::Commit).await;
                                    for pf in primed.drain(..) {
                                        let _ = tx.send(StreamCallbackEvent::Frame(pf)).await;
                                    }
                                }
                                let _ = tx.send(StreamCallbackEvent::Frame(frame)).await;
                            }
                        }
                        for frame in parser.flush() {
                            if !committed {
                                committed = true;
                                let _ = tx.send(StreamCallbackEvent::Commit).await;
                                for pf in primed.drain(..) {
                                    let _ = tx.send(StreamCallbackEvent::Frame(pf)).await;
                                }
                            }
                            let _ = tx.send(StreamCallbackEvent::Frame(frame)).await;
                        }
                    }

                    if let Some(err) = pre_commit_failure {
                        let cls = classify_upstream_failure(
                            Some(err.status),
                            &err.message,
                            err.code.as_deref(),
                        );
                        if !cls.is_retryable || attempt == max_attempts {
                            let _ = tx.send(StreamCallbackEvent::Error(err)).await;
                            return;
                        }
                        on_retry(cls.is_rate_limit);
                        let delay = compute_backoff_delay_ms(attempt, config, err.retry_after);
                        last_err = Some(err);
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        continue;
                    }

                    if !committed {
                        let _ = tx.send(StreamCallbackEvent::Commit).await;
                        for pf in primed.drain(..) {
                            let _ = tx.send(StreamCallbackEvent::Frame(pf)).await;
                        }
                    }
                    return;
                }
                Ok(Err(e)) => {
                    let msg = e.to_string();
                    let cls = classify_upstream_failure(None, &msg, None);
                    let err = UpstreamBridgeError {
                        message: format!(
                            "Upstream stream transport failure: {}",
                            redact_secrets(&msg)
                        ),
                        status: 502,
                        is_rate_limit: cls.is_rate_limit,
                        retry_after: None,
                        upstream_status: None,
                        code: None,
                    };
                    if !cls.is_retryable || attempt == max_attempts {
                        let _ = tx.send(StreamCallbackEvent::Error(err)).await;
                        return;
                    }
                    on_retry(cls.is_rate_limit);
                    let delay = compute_backoff_delay_ms(attempt, config, None);
                    last_err = Some(err);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }
        }

        let _ = tx
            .send(StreamCallbackEvent::Error(last_err.unwrap_or(
                UpstreamBridgeError {
                    message: "Upstream stream retries exhausted".to_string(),
                    status: 502,
                    is_rate_limit: false,
                    retry_after: None,
                    upstream_status: None,
                    code: None,
                },
            )))
            .await;
    }

    pub async fn probe_models(&self, config: &BridgeConfig, api_key: &str) -> Vec<Value> {
        let mut targets = Vec::new();
        for m in [
            &config.models.opus,
            &config.models.sonnet,
            &config.models.haiku,
        ] {
            if !m.is_empty() && !targets.contains(m) {
                targets.push(m.clone());
            }
        }

        let mut futs = Vec::new();
        for model_id in targets {
            let http = self.http.clone();
            let url = format!("{}/v1/responses", config.upstream_base_url);
            let ua = config.upstream_user_agent.clone();
            let key = api_key.to_string();
            futs.push(tokio::spawn(async move {
                let t0 = Instant::now();
                let mut builder = http
                    .post(&url)
                    .timeout(Duration::from_secs(15))
                    .header("Content-Type", "application/json")
                    .header("User-Agent", &ua);
                if !key.is_empty() {
                    builder = builder.header("Authorization", format!("Bearer {key}"));
                }
                let body = json!({
                    "model": model_id,
                    "store": false,
                    "stream": false,
                    "max_output_tokens": 32,
                    "reasoning": { "effort": "low" },
                    "input": [{ "role": "user", "content": [{ "type": "input_text", "text": "OK" }] }]
                });
                match builder.json(&body).send().await {
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        let ok = (200..300).contains(&status);
                        let text = resp.text().await.unwrap_or_default();
                        let err_msg = if ok {
                            String::new()
                        } else {
                            serde_json::from_str::<Value>(&text)
                                .ok()
                                .and_then(|j| {
                                    j.get("error")
                                        .and_then(|e| e.get("message"))
                                        .and_then(Value::as_str)
                                        .map(str::to_string)
                                })
                                .unwrap_or_else(|| text.chars().take(180).collect())
                        };
                        json!({
                            "model": model_id,
                            "ok": ok,
                            "status": status,
                            "latencyMs": t0.elapsed().as_millis() as u64,
                            "error": redact_secrets(&err_msg),
                        })
                    }
                    Err(e) => json!({
                        "model": model_id,
                        "ok": false,
                        "status": 0,
                        "latencyMs": t0.elapsed().as_millis() as u64,
                        "error": redact_secrets(&e.to_string()),
                    }),
                }
            }));
        }

        let mut results = Vec::new();
        for f in futs {
            if let Ok(v) = f.await {
                results.push(v);
            }
        }
        results
    }
}

trait StrIfEmpty {
    fn if_empty<F: FnOnce() -> String>(self, f: F) -> String;
}

impl StrIfEmpty for String {
    fn if_empty<F: FnOnce() -> String>(self, f: F) -> String {
        if self.is_empty() {
            f()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::post, Router};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn retries_pre_commit_sse_too_many_requests_before_commit() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_c = attempts.clone();

        let app = Router::new().route(
            "/v1/responses",
            post(move || {
                let c = attempts_c.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    if c == 1 {
                        let body = "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\"}}\n\nevent: error\ndata: {\"type\":\"error\",\"error\":{\"code\":\"too_many_requests\",\"message\":\"Responses upstream too_many_requests: exceeded token rate limit.\"}}\n\n";
                        (
                            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                            body,
                        )
                    } else {
                        let body = "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_2\"}}\n\nevent: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"Recovered!\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_2\"}}\n\n";
                        (
                            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                            body,
                        )
                    }
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let mut cfg = BridgeConfig::default();
        cfg.upstream_base_url = format!("http://{addr}");
        cfg.max_upstream_retries = 3;
        cfg.retry_base_delay_ms = 20;
        cfg.retry_max_delay_ms = 50;

        let client = UpstreamClient::new();
        let (tx, mut rx) = mpsc::channel(32);
        let retries = Arc::new(AtomicUsize::new(0));
        let retries_c = retries.clone();
        client
            .request_stream(&cfg, &json!({ "model": "gpt-6.1-sol" }), "sk-test", false, tx, move |_| {
                retries_c.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        let mut saw_commit = false;
        let mut saw_recovered = false;
        while let Some(ev) = rx.recv().await {
            match ev {
                StreamCallbackEvent::Commit => saw_commit = true,
                StreamCallbackEvent::Frame(f) => {
                    if f.raw_data.contains("Recovered!") {
                        saw_recovered = true;
                    }
                }
                StreamCallbackEvent::Error(e) => panic!("unexpected error: {e}"),
            }
        }

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(retries.load(Ordering::SeqCst), 1);
        assert!(saw_commit);
        assert!(saw_recovered);

        server_task.abort();
    }

    #[test]
    fn classifies_stream_decode_error_as_retryable() {
        let cls = classify_upstream_failure(
            Some(502),
            "Stream read error: error decoding response body",
            None,
        );
        assert!(cls.is_retryable);
        assert!(!cls.is_rate_limit);
    }

    #[test]
    fn decodes_split_multibyte_utf8_chunks_without_replacement() {
        let arabic = "مرحبا بالعالم 🚀";
        let raw = arabic.as_bytes();
        // Split right inside multi-byte Arabic and emoji sequences
        let mut pending = Vec::new();
        let mut reconstructed = String::new();
        reconstructed.push_str(&decode_utf8_chunk(&mut pending, &raw[..3]));
        reconstructed.push_str(&decode_utf8_chunk(&mut pending, &raw[3..raw.len() - 2]));
        reconstructed.push_str(&decode_utf8_chunk(&mut pending, &raw[raw.len() - 2..]));
        reconstructed.push_str(&flush_utf8_pending(&mut pending));
        assert_eq!(reconstructed, arabic);
    }
}
