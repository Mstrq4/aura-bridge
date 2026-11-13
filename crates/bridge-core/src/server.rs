use crate::app_sync::{
    apply_claude_code, apply_claude_desktop, apply_deepseek_harness, get_app_status,
};
use crate::config::{
    load_config, mask_api_key, persist_env_updates, redact_secrets, BridgeConfig,
};
use crate::router::{derive_prompt_cache_key, resolve_upstream_model};
use crate::sanitizer::sanitize_json_schema;
use crate::shield::{StreamCallbackEvent, UpstreamClient};
use crate::stream::{
    convert_responses_json_to_anthropic, extract_anthropic_usage,
    ResponsesToAnthropicStreamProcessor,
};
use crate::telemetry::{detect_client_agent_with_config, TelemetryStore};
use crate::translator::{
    convert_anthropic_to_responses, estimate_anthropic_input_tokens,
    estimate_responses_input_tokens,
};
use crate::updater::check_github_updates;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

const UI_INDEX_HTML: &str = include_str!("../../../ui/index.html");
const UI_STYLES_CSS: &str = include_str!("../../../ui/styles.css");
const UI_APP_JS: &str = include_str!("../../../ui/app.js");
const UI_ICONS_JS: &str = include_str!("../../../ui/icons.js");

pub struct BridgeState {
    pub config: RwLock<BridgeConfig>,
    pub telemetry: Mutex<TelemetryStore>,
    pub upstream: UpstreamClient,
    pub started_at: Instant,
    pub version: String,
}

impl BridgeState {
    pub fn new(version: String) -> Arc<Self> {
        Arc::new(Self {
            config: RwLock::new(load_config()),
            telemetry: Mutex::new(TelemetryStore::load_or_migrate()),
            upstream: UpstreamClient::new(),
            started_at: Instant::now(),
            version,
        })
    }

    pub fn reload_config(&self) {
        let fresh = load_config();
        *self.config.write().unwrap() = fresh;
    }
}

fn resolve_api_key(headers: &HeaderMap, config: &BridgeConfig) -> String {
    let configured =
        if !config.upstream_api_key.is_empty() && config.upstream_api_key != "replace-with-your-api-key"
        {
            config.upstream_api_key.trim().to_string()
        } else {
            String::new()
        };

    let mut header_key = String::new();
    if let Some(v) = headers.get("x-api-key").and_then(|h| h.to_str().ok()) {
        if !v.trim().is_empty() {
            header_key = v.trim().to_string();
        }
    } else if let Some(v) = headers.get(header::AUTHORIZATION).and_then(|h| h.to_str().ok()) {
        let lower = v.to_ascii_lowercase();
        if lower.starts_with("bearer ") {
            header_key = v[7..].trim().to_string();
        }
    }

    if header_key.starts_with("sk-") && header_key.len() >= 20 {
        header_key
    } else if !configured.is_empty() {
        configured
    } else {
        header_key
    }
}

fn map_anthropic_error_type(status: u16, is_rate_limit: bool) -> &'static str {
    if is_rate_limit || status == 429 {
        return "rate_limit_error";
    }
    match status {
        400 => "invalid_request_error",
        401 => "authentication_error",
        403 => "permission_error",
        404 => "not_found_error",
        413 => "request_too_large",
        503 | 529 => "overloaded_error",
        _ => "api_error",
    }
}

fn normalize_passthrough_responses_body(
    body: &Value,
    headers: &HeaderMap,
    config: &BridgeConfig,
) -> (Value, bool) {
    let mut cloned = body.clone();
    let has_tools = cloned
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty());
    let max_out = cloned
        .get("max_output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let is_bg_title = !has_tools && max_out > 0 && max_out <= 256;

    let resolved_model =
        resolve_upstream_model(cloned.get("model").and_then(Value::as_str), config, is_bg_title);
    cloned["model"] = json!(resolved_model);

    if cloned.get("store").is_none() {
        cloned["store"] = json!(false);
    }
    if cloned.get("prompt_cache_key").is_none() {
        let sid = headers
            .get("x-claude-code-session-id")
            .or_else(|| headers.get("x-session-id"))
            .and_then(|v| v.to_str().ok());
        let inst = cloned
            .get("instructions")
            .and_then(Value::as_str)
            .unwrap_or("");
        cloned["prompt_cache_key"] = json!(derive_prompt_cache_key(sid, inst, &resolved_model));
    }

    if let Some(tools) = cloned.get_mut("tools").and_then(Value::as_array_mut) {
        for tool in tools.iter_mut() {
            if let Some(obj) = tool.as_object_mut() {
                if obj.get("type").and_then(Value::as_str) == Some("function") {
                    if let Some(params) = obj.get("parameters").cloned() {
                        obj.insert("parameters".to_string(), sanitize_json_schema(&params));
                    }
                    if !obj.contains_key("strict") {
                        obj.insert("strict".to_string(), json!(false));
                    }
                }
            }
        }
    }

    let eff = cloned
        .get("reasoning")
        .and_then(|r| r.get("effort"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let headroom = config.min_reasoning_token_headroom.max(512);
    if max_out > 0 && max_out < headroom && eff != "none" {
        cloned["max_output_tokens"] = json!(max_out + headroom);
    }

    let serialized_len = cloned
        .get("input")
        .map(Value::to_string)
        .unwrap_or_default()
        .len();
    let is_heavy = has_tools || serialized_len > 30000;
    (cloned, is_heavy)
}

async fn cors_middleware(req: Request<Body>, next: Next) -> Response {
    if req.method() == Method::OPTIONS {
        return (
            StatusCode::NO_CONTENT,
            [
                (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
                (header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, OPTIONS"),
                (
                    header::ACCESS_CONTROL_ALLOW_HEADERS,
                    "Content-Type, Authorization, x-api-key, anthropic-version, anthropic-beta, x-claude-code-session-id",
                ),
            ],
            "",
        )
            .into_response();
    }
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    h.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    h.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static(
            "Content-Type, Authorization, x-api-key, anthropic-version, anthropic-beta, x-claude-code-session-id",
        ),
    );
    resp
}

async fn serve_index() -> Html<&'static str> {
    Html(UI_INDEX_HTML)
}

async fn serve_css() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        UI_STYLES_CSS,
    )
        .into_response()
}

async fn serve_app_js() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        UI_APP_JS,
    )
        .into_response()
}

async fn serve_icons_js() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        UI_ICONS_JS,
    )
        .into_response()
}

async fn handle_health(State(state): State<Arc<BridgeState>>) -> Json<Value> {
    let cfg = state.config.read().unwrap().clone();
    let telem = state.telemetry.lock().unwrap().clone();
    Json(json!({
        "status": "ok",
        "service": "claude-switch-pro-bridge",
        "version": state.version,
        "pid": std::process::id(),
        "uptimeSeconds": state.started_at.elapsed().as_secs(),
        "listen": format!("http://{}:{}", cfg.host, cfg.port),
        "upstreamBaseUrl": cfg.upstream_base_url,
        "apiKeyMasked": mask_api_key(&cfg.upstream_api_key),
        "defaultReasoningEffort": cfg.default_reasoning_effort,
        "reasoningSummary": cfg.reasoning_summary,
        "maxUpstreamRetries": cfg.max_upstream_retries,
        "minReasoningTokenHeadroom": cfg.min_reasoning_token_headroom,
        "retryBaseDelayMs": cfg.retry_base_delay_ms,
        "maxConcurrentHeavyRequests": cfg.max_concurrent_heavy_requests,
        "models": cfg.models,
        "metrics": telem.metrics,
        "logs": telem.recent_logs,
    }))
}

async fn handle_app_status(State(state): State<Arc<BridgeState>>) -> Json<Value> {
    let cfg = state.config.read().unwrap().clone();
    Json(get_app_status(&cfg))
}

async fn handle_apply_desktop(State(state): State<Arc<BridgeState>>) -> Response {
    let cfg = state.config.read().unwrap().clone();
    match apply_claude_desktop(&cfg) {
        Ok(msg) => (StatusCode::OK, Json(json!({ "success": true, "message": msg }))).into_response(),
        Err((code, err)) => (
            StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(json!({ "success": false, "error": err })),
        )
            .into_response(),
    }
}

async fn handle_apply_code(State(state): State<Arc<BridgeState>>) -> Response {
    let cfg = state.config.read().unwrap().clone();
    match apply_claude_code(&cfg) {
        Ok(msg) => (StatusCode::OK, Json(json!({ "success": true, "message": msg }))).into_response(),
        Err((code, err)) => (
            StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(json!({ "success": false, "error": err })),
        )
            .into_response(),
    }
}

async fn handle_apply_dsh(State(state): State<Arc<BridgeState>>) -> Response {
    let cfg = state.config.read().unwrap().clone();
    match apply_deepseek_harness(&cfg) {
        Ok(msg) => (StatusCode::OK, Json(json!({ "success": true, "message": msg }))).into_response(),
        Err((code, err)) => (
            StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(json!({ "success": false, "error": err })),
        )
            .into_response(),
    }
}

async fn handle_save_provider(
    State(state): State<Arc<BridgeState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let mut env_updates = BTreeMap::new();
    let resp_json = {
        let mut cfg = state.config.write().unwrap();
        if let Some(models) = body.get("models").and_then(Value::as_object) {
            if let Some(m) = models.get("opus").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) {
                cfg.models.opus = m.to_string();
                env_updates.insert("MODEL_OPUS".to_string(), m.to_string());
            }
            if let Some(m) = models.get("sonnet").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) {
                cfg.models.sonnet = m.to_string();
                env_updates.insert("MODEL_SONNET".to_string(), m.to_string());
            }
            if let Some(m) = models.get("haiku").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) {
                cfg.models.haiku = m.to_string();
                env_updates.insert("MODEL_HAIKU".to_string(), m.to_string());
            }
            cfg.models.default = cfg.models.opus.clone();
            env_updates.insert("MODEL_DEFAULT".to_string(), cfg.models.default.clone());
        }
        if let Some(u) = body.get("upstreamBaseUrl").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) {
            let clean = u.trim_end_matches('/').to_string();
            cfg.upstream_base_url = clean.clone();
            env_updates.insert("UPSTREAM_BASE_URL".to_string(), clean);
        }
        if let Some(k) = body.get("upstreamApiKey").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) {
            cfg.upstream_api_key = k.to_string();
            env_updates.insert("UPSTREAM_API_KEY".to_string(), k.to_string());
        }
        if let Some(eff) = body.get("defaultReasoningEffort").and_then(Value::as_str) {
            let e = eff.trim().to_ascii_lowercase();
            if matches!(e.as_str(), "none" | "minimal" | "low" | "medium" | "high" | "xhigh") {
                cfg.default_reasoning_effort = e.clone();
                env_updates.insert("DEFAULT_REASONING_EFFORT".to_string(), e);
            }
        }
        if let Some(sum) = body.get("reasoningSummary").and_then(Value::as_str) {
            let s = sum.trim().to_ascii_lowercase();
            if matches!(s.as_str(), "auto" | "concise" | "detailed" | "none") {
                cfg.reasoning_summary = s.clone();
                env_updates.insert("REASONING_SUMMARY".to_string(), s);
            }
        }
        json!({
            "success": true,
            "models": cfg.models,
            "upstreamBaseUrl": cfg.upstream_base_url,
            "defaultReasoningEffort": cfg.default_reasoning_effort,
            "reasoningSummary": cfg.reasoning_summary,
            "apiKeyMasked": mask_api_key(&cfg.upstream_api_key),
        })
    };
    let _ = persist_env_updates(&env_updates);
    Json(resp_json)
}

async fn handle_save_shield(
    State(state): State<Arc<BridgeState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let mut env_updates = BTreeMap::new();
    let parse_num = |v: &Value| -> Option<u64> {
        v.as_u64()
            .or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
    };
    let resp_json = {
        let mut cfg = state.config.write().unwrap();
        if let Some(n) = body.get("maxUpstreamRetries").and_then(parse_num).filter(|&n| n <= 15) {
            cfg.max_upstream_retries = n as u32;
            env_updates.insert("MAX_UPSTREAM_RETRIES".to_string(), n.to_string());
        }
        if let Some(n) = body.get("minReasoningTokenHeadroom").and_then(parse_num).filter(|&n| (512..=32768).contains(&n)) {
            cfg.min_reasoning_token_headroom = n;
            env_updates.insert("MIN_REASONING_TOKEN_HEADROOM".to_string(), n.to_string());
        }
        if let Some(n) = body.get("retryBaseDelayMs").and_then(parse_num).filter(|&n| (500..=30000).contains(&n)) {
            cfg.retry_base_delay_ms = n;
            env_updates.insert("RETRY_BASE_DELAY_MS".to_string(), n.to_string());
        }
        if let Some(n) = body.get("maxConcurrentHeavyRequests").and_then(parse_num).filter(|&n| (1..=10).contains(&n)) {
            cfg.max_concurrent_heavy_requests = n as usize;
            env_updates.insert("MAX_CONCURRENT_HEAVY_REQUESTS".to_string(), n.to_string());
        }
        json!({
            "success": true,
            "maxUpstreamRetries": cfg.max_upstream_retries,
            "minReasoningTokenHeadroom": cfg.min_reasoning_token_headroom,
            "retryBaseDelayMs": cfg.retry_base_delay_ms,
            "maxConcurrentHeavyRequests": cfg.max_concurrent_heavy_requests,
        })
    };
    let _ = persist_env_updates(&env_updates);
    Json(resp_json)
}

async fn handle_probe_models(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
) -> Json<Value> {
    let cfg = state.config.read().unwrap().clone();
    let api_key = resolve_api_key(&headers, &cfg);
    let results = state.upstream.probe_models(&cfg, &api_key).await;
    Json(json!({ "success": true, "results": results }))
}

async fn handle_clear_logs(State(state): State<Arc<BridgeState>>) -> Json<Value> {
    state.telemetry.lock().unwrap().clear();
    Json(json!({ "success": true }))
}

async fn handle_restart(State(state): State<Arc<BridgeState>>) -> Json<Value> {
    state.reload_config();
    Json(json!({ "success": true, "message": "Bridge configuration reloaded in-process" }))
}

async fn handle_update_check(State(state): State<Arc<BridgeState>>) -> Json<Value> {
    let res = check_github_updates(&state.version, None).await;
    Json(serde_json::to_value(res).unwrap_or_else(|_| json!({ "updateAvailable": false })))
}

async fn handle_list_models(State(state): State<Arc<BridgeState>>) -> Json<Value> {
    let cfg = state.config.read().unwrap().clone();
    let mut unique = Vec::new();
    for id in [
        cfg.models.opus.as_str(),
        cfg.models.sonnet.as_str(),
        cfg.models.haiku.as_str(),
        "claude-opus-4-6",
        "claude-sonnet-4-6",
        "claude-haiku-4-5-20251001",
    ] {
        if !id.is_empty() && !unique.contains(&id.to_string()) {
            unique.push(id.to_string());
        }
    }
    let data: Vec<Value> = unique
        .into_iter()
        .map(|id| {
            json!({
                "id": id,
                "object": "model",
                "created": 1767225600,
                "owned_by": "claude-switch-pro"
            })
        })
        .collect();
    Json(json!({ "object": "list", "data": data }))
}

async fn handle_count_tokens(
    State(state): State<Arc<BridgeState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    {
        let mut t = state.telemetry.lock().unwrap();
        t.metrics.requests_total += 1;
        t.metrics.count_tokens_requests += 1;
    }
    Json(json!({ "input_tokens": estimate_anthropic_input_tokens(&body) }))
}

fn error_response_with_retry(
    status: StatusCode,
    retry_after: Option<u64>,
    body: Value,
) -> Response {
    let mut resp = (status, Json(body)).into_response();
    if status == StatusCode::TOO_MANY_REQUESTS {
        let secs = retry_after.unwrap_or(15).to_string();
        if let Ok(hv) = HeaderValue::from_str(&secs) {
            resp.headers_mut()
                .insert(HeaderName::from_static("retry-after"), hv);
        }
    }
    resp
}

async fn handle_messages(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    {
        let mut t = state.telemetry.lock().unwrap();
        t.metrics.requests_total += 1;
        t.metrics.messages_requests += 1;
    }
    let cfg = state.config.read().unwrap().clone();
    let api_key = resolve_api_key(&headers, &cfg);
    let session_id = headers
        .get("x-claude-code-session-id")
        .or_else(|| headers.get("x-session-id"))
        .and_then(|v| v.to_str().ok());

    let conv = convert_anthropic_to_responses(&body, session_id, &cfg);
    let client_agent = detect_client_agent_with_config(
        &headers,
        &conv.requested_model,
        &conv.upstream_model,
        false,
        &cfg,
    );
    let is_stream = body.get("stream").and_then(Value::as_bool).unwrap_or(false);

    if !is_stream {
        let t0 = Instant::now();
        let state_retry = state.clone();
        match state
            .upstream
            .request_json(&cfg, &conv.payload, &api_key, conv.is_heavy_request, |is_rl| {
                let mut t = state_retry.telemetry.lock().unwrap();
                t.metrics.retries_total += 1;
                if is_rl {
                    t.metrics.rate_limit_shield_hits += 1;
                }
            })
            .await
        {
            Ok(resp_json) => {
                let anth = convert_responses_json_to_anthropic(
                    &resp_json,
                    &conv.requested_model,
                    Some(&conv.tool_codec),
                );
                let in_tok = anth["usage"]["input_tokens"].as_u64().unwrap_or(0);
                let out_tok = anth["usage"]["output_tokens"].as_u64().unwrap_or(0);
                let cached = anth["usage"]["cache_read_input_tokens"].as_u64().unwrap_or(0);
                state.telemetry.lock().unwrap().add_log(
                    Uuid::new_v4().to_string(),
                    client_agent,
                    &conv.upstream_model,
                    200,
                    in_tok,
                    out_tok,
                    cached,
                    t0.elapsed().as_millis() as u64,
                    false,
                );
                return (StatusCode::OK, Json(anth)).into_response();
            }
            Err(err) => {
                let status = if err.is_rate_limit { 429 } else { err.status };
                state.telemetry.lock().unwrap().metrics.errors_total += 1;
                return error_response_with_retry(
                    StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
                    err.retry_after,
                    json!({
                        "type": "error",
                        "error": {
                            "type": map_anthropic_error_type(status, err.is_rate_limit),
                            "message": redact_secrets(&err.message),
                        }
                    }),
                );
            }
        }
    }

    // Streaming Anthropic SSE response with pre-commit retry buffering
    let req_start = Instant::now();
    let (up_tx, mut up_rx) = mpsc::channel::<StreamCallbackEvent>(64);
    let upstream = state.upstream.clone();
    let cfg_clone = cfg.clone();
    let payload_clone = conv.payload.clone();
    let api_key_clone = api_key.clone();
    let state_retry = state.clone();
    tokio::spawn(async move {
        upstream
            .request_stream(
                &cfg_clone,
                &payload_clone,
                &api_key_clone,
                conv.is_heavy_request,
                up_tx,
                move |is_rl| {
                    let mut t = state_retry.telemetry.lock().unwrap();
                    t.metrics.retries_total += 1;
                    if is_rl {
                        t.metrics.rate_limit_shield_hits += 1;
                    }
                },
            )
            .await;
    });

    match up_rx.recv().await {
        Some(StreamCallbackEvent::Commit) => {
            let log_id = Uuid::new_v4().to_string();
            let est_in = estimate_anthropic_input_tokens(&body);
            state.telemetry.lock().unwrap().add_log(
                log_id.clone(),
                client_agent,
                &conv.upstream_model,
                200,
                est_in,
                0,
                0,
                req_start.elapsed().as_millis() as u64,
                true,
            );

            let (sse_tx, sse_rx) = mpsc::channel::<Result<Event, Infallible>>(128);
            let state_stream = state.clone();
            let mut proc =
                ResponsesToAnthropicStreamProcessor::new(conv.requested_model, conv.tool_codec);
            tokio::spawn(async move {
                while let Some(ev) = up_rx.recv().await {
                    match ev {
                        StreamCallbackEvent::Frame(frame) => {
                            if !frame.done {
                                if let Some(ref dobj) = frame.data_obj {
                                    let typ =
                                        dobj.get("type").and_then(Value::as_str).unwrap_or("");
                                    if typ == "response.completed" || typ == "response.incomplete" {
                                        if let Some(usage) =
                                            dobj.get("response").and_then(|r| r.get("usage"))
                                        {
                                            state_stream
                                                .telemetry
                                                .lock()
                                                .unwrap()
                                                .settle_stream_usage(&log_id, usage);
                                        }
                                    }
                                    for (ename, data) in proc.handle_responses_event(dobj) {
                                        let event = Event::default().event(ename).data(data.to_string());
                                        if sse_tx.send(Ok(event)).await.is_err() {
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                        StreamCallbackEvent::Error(err) => {
                            let status = if err.is_rate_limit { 429 } else { err.status };
                            let data = json!({
                                "type": "error",
                                "error": {
                                    "type": map_anthropic_error_type(status, err.is_rate_limit),
                                    "message": redact_secrets(&err.message)
                                }
                            });
                            let _ = sse_tx
                                .send(Ok(Event::default().event("error").data(data.to_string())))
                                .await;
                            return;
                        }
                        StreamCallbackEvent::Commit => {}
                    }
                }
                for (ename, data) in proc.finalize() {
                    let _ = sse_tx
                        .send(Ok(Event::default().event(ename).data(data.to_string())))
                        .await;
                }
            });

            Sse::new(ReceiverStream::new(sse_rx))
                .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("ping"))
                .into_response()
        }
        Some(StreamCallbackEvent::Error(err)) => {
            let status = if err.is_rate_limit { 429 } else { err.status };
            state.telemetry.lock().unwrap().metrics.errors_total += 1;
            error_response_with_retry(
                StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
                err.retry_after,
                json!({
                    "type": "error",
                    "error": {
                        "type": map_anthropic_error_type(status, err.is_rate_limit),
                        "message": redact_secrets(&err.message)
                    }
                }),
            )
        }
        _ => {
            state.telemetry.lock().unwrap().metrics.errors_total += 1;
            error_response_with_retry(
                StatusCode::BAD_GATEWAY,
                None,
                json!({
                    "type": "error",
                    "error": {
                        "type": "api_error",
                        "message": "Upstream stream closed unexpectedly before commit"
                    }
                }),
            )
        }
    }
}

async fn handle_responses(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    {
        let mut t = state.telemetry.lock().unwrap();
        t.metrics.requests_total += 1;
        t.metrics.responses_pass_through_requests += 1;
    }
    let cfg = state.config.read().unwrap().clone();
    let api_key = resolve_api_key(&headers, &cfg);
    let (payload, is_heavy) = normalize_passthrough_responses_body(&body, &headers, &cfg);
    let resp_model = payload
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(&cfg.models.default)
        .to_string();
    let req_model = body.get("model").and_then(Value::as_str).unwrap_or("");
    let client_name = detect_client_agent_with_config(&headers, req_model, &resp_model, true, &cfg);
    let is_stream = payload
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if !is_stream {
        let t0 = Instant::now();
        let state_retry = state.clone();
        match state
            .upstream
            .request_json(&cfg, &payload, &api_key, is_heavy, |is_rl| {
                let mut t = state_retry.telemetry.lock().unwrap();
                t.metrics.retries_total += 1;
                if is_rl {
                    t.metrics.rate_limit_shield_hits += 1;
                }
            })
            .await
        {
            Ok(resp_json) => {
                let u = extract_anthropic_usage(resp_json.get("usage"));
                let in_tok = u["input_tokens"].as_u64().unwrap_or(0)
                    + u["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                let out_tok = u["output_tokens"].as_u64().unwrap_or(0);
                let cached = u["cache_read_input_tokens"].as_u64().unwrap_or(0);
                state.telemetry.lock().unwrap().add_log(
                    Uuid::new_v4().to_string(),
                    client_name,
                    &resp_model,
                    200,
                    in_tok,
                    out_tok,
                    cached,
                    t0.elapsed().as_millis() as u64,
                    false,
                );
                return (StatusCode::OK, Json(resp_json)).into_response();
            }
            Err(err) => {
                let status = if err.is_rate_limit { 429 } else { err.status };
                state.telemetry.lock().unwrap().metrics.errors_total += 1;
                return error_response_with_retry(
                    StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
                    err.retry_after,
                    json!({
                        "error": {
                            "message": redact_secrets(&err.message),
                            "type": if status == 429 { "rate_limit_error" } else { "upstream_error" },
                            "code": if status == 429 { "rate_limit_exceeded" } else { "upstream_error" }
                        }
                    }),
                );
            }
        }
    }

    // Streaming pass-through Responses SSE
    let req_start = Instant::now();
    let (up_tx, mut up_rx) = mpsc::channel::<StreamCallbackEvent>(64);
    let upstream = state.upstream.clone();
    let cfg_clone = cfg.clone();
    let payload_clone = payload.clone();
    let api_key_clone = api_key.clone();
    let state_retry = state.clone();
    tokio::spawn(async move {
        upstream
            .request_stream(
                &cfg_clone,
                &payload_clone,
                &api_key_clone,
                is_heavy,
                up_tx,
                move |is_rl| {
                    let mut t = state_retry.telemetry.lock().unwrap();
                    t.metrics.retries_total += 1;
                    if is_rl {
                        t.metrics.rate_limit_shield_hits += 1;
                    }
                },
            )
            .await;
    });

    match up_rx.recv().await {
        Some(StreamCallbackEvent::Commit) => {
            let log_id = Uuid::new_v4().to_string();
            let est_in = estimate_responses_input_tokens(&payload);
            state.telemetry.lock().unwrap().add_log(
                log_id.clone(),
                client_name,
                &resp_model,
                200,
                est_in,
                0,
                0,
                req_start.elapsed().as_millis() as u64,
                true,
            );

            let (raw_tx, raw_rx) = mpsc::channel::<Result<bytes::Bytes, Infallible>>(128);
            let state_stream = state.clone();
            tokio::spawn(async move {
                while let Some(ev) = up_rx.recv().await {
                    match ev {
                        StreamCallbackEvent::Frame(frame) => {
                            if let Some(ref dobj) = frame.data_obj {
                                let typ = dobj.get("type").and_then(Value::as_str).unwrap_or("");
                                if typ == "response.completed" || typ == "response.incomplete" {
                                    if let Some(usage) =
                                        dobj.get("response").and_then(|r| r.get("usage"))
                                    {
                                        state_stream
                                            .telemetry
                                            .lock()
                                            .unwrap()
                                            .settle_stream_usage(&log_id, usage);
                                    }
                                }
                            }
                            let chunk = format!("{}\n\n", frame.raw_frame);
                            if raw_tx.send(Ok(bytes::Bytes::from(chunk))).await.is_err() {
                                return;
                            }
                        }
                        StreamCallbackEvent::Error(err) => {
                            let status = if err.is_rate_limit { 429 } else { err.status };
                            let data = json!({
                                "type": "error",
                                "error": {
                                    "type": if status == 429 { "rate_limit_error" } else { "upstream_error" },
                                    "message": redact_secrets(&err.message)
                                }
                            });
                            let chunk = format!("event: error\ndata: {data}\n\n");
                            let _ = raw_tx.send(Ok(bytes::Bytes::from(chunk))).await;
                            return;
                        }
                        StreamCallbackEvent::Commit => {}
                    }
                }
            });

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
                .header(header::CACHE_CONTROL, "no-cache, no-transform")
                .header("X-Accel-Buffering", "no")
                .body(Body::from_stream(ReceiverStream::new(raw_rx)))
                .unwrap()
        }
        Some(StreamCallbackEvent::Error(err)) => {
            let status = if err.is_rate_limit { 429 } else { err.status };
            state.telemetry.lock().unwrap().metrics.errors_total += 1;
            error_response_with_retry(
                StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
                err.retry_after,
                json!({
                    "error": {
                        "message": redact_secrets(&err.message),
                        "type": if status == 429 { "rate_limit_error" } else { "upstream_error" },
                        "code": if status == 429 { "rate_limit_exceeded" } else { "upstream_error" }
                    }
                }),
            )
        }
        _ => {
            state.telemetry.lock().unwrap().metrics.errors_total += 1;
            error_response_with_retry(
                StatusCode::BAD_GATEWAY,
                None,
                json!({
                    "error": {
                        "message": "Upstream stream closed before commit",
                        "type": "upstream_error"
                    }
                }),
            )
        }
    }
}

async fn not_found_handler(req: Request<Body>) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": {
                "message": format!("Unsupported route: {} {}", req.method(), req.uri().path())
            }
        })),
    )
        .into_response()
}

pub fn build_router(state: Arc<BridgeState>) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/dashboard", get(serve_index))
        .route("/ui", get(serve_index))
        .route("/styles.css", get(serve_css))
        .route("/ui/styles.css", get(serve_css))
        .route("/app.js", get(serve_app_js))
        .route("/ui/app.js", get(serve_app_js))
        .route("/icons.js", get(serve_icons_js))
        .route("/ui/icons.js", get(serve_icons_js))
        .route("/health", get(handle_health))
        .route("/status", get(handle_health))
        .route("/api/stats", get(handle_health))
        .route("/api/health", get(handle_health))
        .route("/api/app-status", get(handle_app_status))
        .route("/app-status", get(handle_app_status))
        .route("/api/apply/claude-desktop", post(handle_apply_desktop))
        .route("/api/apply/claude-code", post(handle_apply_code))
        .route("/api/apply/deepseek-harness", post(handle_apply_dsh))
        .route("/api/config", post(handle_save_provider))
        .route("/api/provider", post(handle_save_provider))
        .route("/api/shield", post(handle_save_shield))
        .route("/api/probe-models", post(handle_probe_models))
        .route("/api/logs/clear", post(handle_clear_logs))
        .route("/api/restart", post(handle_restart))
        .route("/api/update/check", get(handle_update_check))
        .route("/v1/models", get(handle_list_models))
        .route("/models", get(handle_list_models))
        .route("/v1/messages/count_tokens", post(handle_count_tokens))
        .route("/messages/count_tokens", post(handle_count_tokens))
        .route("/v1/messages", post(handle_messages))
        .route("/messages", post(handle_messages))
        .route("/v1/responses", post(handle_responses))
        .route("/responses", post(handle_responses))
        .fallback(not_found_handler)
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .layer(middleware::from_fn(cors_middleware))
        .with_state(state)
}

pub async fn start_bridge_on_listener(
    listener: tokio::net::TcpListener,
    state: Arc<BridgeState>,
) -> std::io::Result<()> {
    let app = build_router(state);
    axum::serve(listener, app).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn serves_embedded_ui_and_health_json() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = BridgeState::new("2.1.0".to_string());
        let srv = tokio::spawn(async move {
            let _ = start_bridge_on_listener(listener, state).await;
        });

        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut resp = String::new();
        stream.read_to_string(&mut resp).await.unwrap();
        assert!(resp.contains("200 OK"));
        assert!(resp.contains("\"version\":\"2.1.0\""));
        assert!(resp.contains("claude-switch-pro-bridge"));

        let mut html_stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        html_stream
            .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut html_resp = String::new();
        html_stream.read_to_string(&mut html_resp).await.unwrap();
        assert!(html_resp.contains("200 OK"));
        assert!(html_resp.contains("Claude Switch Pro"));
        assert!(!html_resp.contains("fonts.googleapis.com"));

        srv.abort();
    }
}
