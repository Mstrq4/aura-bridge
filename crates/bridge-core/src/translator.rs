use crate::config::BridgeConfig;
use crate::router::{derive_prompt_cache_key, resolve_upstream_model};
use crate::sanitizer::{sanitize_json_schema, ToolNameCodec};
use serde_json::{json, Map, Value};
use std::collections::HashSet;

pub fn normalize_call_id(raw: Option<&str>) -> String {
    let base = raw.filter(|s| !s.is_empty()).unwrap_or("call_default");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    if cleaned.is_empty() {
        "call_default".to_string()
    } else {
        cleaned
    }
}

fn extract_system_instructions(system: Option<&Value>) -> String {
    match system {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|blk| match blk {
                Value::String(s) if !s.trim().is_empty() => Some(s.as_str()),
                Value::Object(obj) => obj
                    .get("text")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n")
            .trim()
            .to_string(),
        _ => String::new(),
    }
}

fn convert_image_source(source: Option<&Value>, detail: Option<&str>) -> Option<Value> {
    let src = source?.as_object()?;
    let stype = src.get("type").and_then(Value::as_str)?;
    let mut item = Map::new();
    item.insert("type".to_string(), Value::String("input_image".to_string()));

    if stype == "base64" {
        let data = src.get("data").and_then(Value::as_str)?;
        if data.is_empty() {
            return None;
        }
        let mime = src
            .get("media_type")
            .and_then(Value::as_str)
            .unwrap_or("image/png");
        item.insert(
            "image_url".to_string(),
            Value::String(format!("data:{mime};base64,{data}")),
        );
    } else if stype == "url" {
        let url = src.get("url").and_then(Value::as_str)?;
        if url.is_empty() {
            return None;
        }
        item.insert("image_url".to_string(), Value::String(url.to_string()));
    } else {
        return None;
    }

    if let Some(d) = detail {
        if d == "low" || d == "high" || d == "auto" {
            item.insert("detail".to_string(), Value::String(d.to_string()));
        }
    }

    Some(Value::Object(item))
}

fn convert_document_block(block: &Map<String, Value>) -> Option<Value> {
    let src = block.get("source")?.as_object()?;
    let stype = src.get("type").and_then(Value::as_str)?;

    match stype {
        "text" => {
            let data = src.get("data").and_then(Value::as_str)?;
            Some(json!({ "type": "input_text", "text": data }))
        }
        "content" => {
            let arr = src.get("content")?.as_array()?;
            let joined: String = arr
                .iter()
                .filter_map(|c| c.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n");
            if joined.is_empty() {
                None
            } else {
                Some(json!({ "type": "input_text", "text": joined }))
            }
        }
        "base64" => {
            let data = src.get("data").and_then(Value::as_str)?;
            if data.is_empty() {
                return None;
            }
            let mime = src
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or("application/pdf");
            let filename = block
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("attachment.pdf");
            Some(json!({
                "type": "input_file",
                "filename": filename,
                "file_data": format!("data:{mime};base64,{data}"),
            }))
        }
        "url" => {
            let url = src.get("url").and_then(Value::as_str)?;
            if url.is_empty() {
                return None;
            }
            Some(json!({
                "type": "input_file",
                "file_url": url,
            }))
        }
        _ => None,
    }
}

fn convert_audio_block(block: &Map<String, Value>) -> Option<Value> {
    let src = block
        .get("source")
        .or_else(|| block.get("input_audio"))?
        .as_object()?;
    let data = src.get("data").and_then(Value::as_str)?;
    if data.is_empty() {
        return None;
    }
    let mime = src
        .get("media_type")
        .or_else(|| src.get("format"))
        .and_then(Value::as_str)
        .unwrap_or("wav")
        .to_ascii_lowercase();
    let format = if mime.contains("mp3") || mime.contains("mpeg") {
        "mp3"
    } else {
        "wav"
    };
    Some(json!({
        "type": "input_audio",
        "input_audio": { "data": data, "format": format }
    }))
}

fn extract_tool_result_payload(block: &Map<String, Value>) -> (String, Vec<Value>) {
    let mut embedded_images = Vec::new();
    let mut text_parts = Vec::new();

    match block.get("content") {
        Some(Value::String(s)) => text_parts.push(s.clone()),
        Some(Value::Array(arr)) => {
            for part in arr {
                let Some(obj) = part.as_object() else {
                    continue;
                };
                match obj.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text" => {
                        if let Some(t) = obj.get("text").and_then(Value::as_str) {
                            text_parts.push(t.to_string());
                        }
                    }
                    "tool_reference" => {
                        let name = obj
                            .get("tool_name")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown");
                        text_parts.push(format!("[Tool schema available: {name}]"));
                    }
                    "image" => {
                        if let Some(img) = convert_image_source(obj.get("source"), None) {
                            embedded_images.push(img);
                        }
                    }
                    "document" => {
                        if let Some(doc) = convert_document_block(obj) {
                            if doc.get("type").and_then(Value::as_str) == Some("input_text") {
                                if let Some(t) = doc.get("text").and_then(Value::as_str) {
                                    text_parts.push(t.to_string());
                                }
                            }
                        }
                    }
                    _ => {
                        text_parts.push(part.to_string());
                    }
                }
            }
        }
        Some(other) if !other.is_null() => text_parts.push(other.to_string()),
        _ => {}
    }

    let is_error = block
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut output = text_parts.join("\n");
    if output.is_empty() && !embedded_images.is_empty() {
        output = "[Image returned by tool — attached in following message]".to_string();
    }
    if output.is_empty() {
        output = if is_error {
            "[Tool returned an empty error]".to_string()
        } else {
            "(ok)".to_string()
        };
    }
    if is_error && !output.starts_with("[Tool Error]") {
        output = format!("[Tool Error]\n{output}");
    }

    (output, embedded_images)
}

fn reconcile_function_call_pairs(items: Vec<Value>) -> Vec<Value> {
    let mut declared = HashSet::new();
    let mut answered = HashSet::new();

    for item in &items {
        let typ = item.get("type").and_then(Value::as_str);
        let call_id = item.get("call_id").and_then(Value::as_str);
        if typ == Some("function_call") {
            if let Some(id) = call_id {
                declared.insert(id.to_string());
            }
        } else if typ == Some("function_call_output") {
            if let Some(id) = call_id {
                answered.insert(id.to_string());
            }
        }
    }

    let mut reconciled = Vec::with_capacity(items.len());
    for item in items {
        let typ = item.get("type").and_then(Value::as_str).map(str::to_string);
        let call_id = item
            .get("call_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_default();

        if typ.as_deref() == Some("function_call") {
            reconciled.push(item);
            if !call_id.is_empty() && !answered.contains(&call_id) {
                reconciled.push(json!({
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": "[Interrupted before tool execution completed]"
                }));
                answered.insert(call_id);
            }
        } else if typ.as_deref() == Some("function_call_output") {
            if !declared.contains(&call_id) {
                let output_str = item
                    .get("output")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                reconciled.push(json!({
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": format!("[Previous tool output ({call_id})]:\n{output_str}")
                    }]
                }));
            } else {
                reconciled.push(item);
            }
        } else {
            reconciled.push(item);
        }
    }

    reconciled
}

fn convert_messages_to_input(messages: Option<&Value>, tool_codec: &mut ToolNameCodec) -> Vec<Value> {
    let Some(Value::Array(msgs)) = messages else {
        return vec![json!({ "role": "user", "content": [{ "type": "input_text", "text": "Hello" }] })];
    };
    if msgs.is_empty() {
        return vec![json!({ "role": "user", "content": [{ "type": "input_text", "text": "Hello" }] })];
    }

    let mut items = Vec::new();
    for msg in msgs {
        let Some(msg_obj) = msg.as_object() else {
            continue;
        };
        let role = if msg_obj.get("role").and_then(Value::as_str) == Some("assistant") {
            "assistant"
        } else {
            "user"
        };

        if let Some(Value::String(s)) = msg_obj.get("content") {
            if !s.is_empty() || role == "user" {
                let text = if s.is_empty() { " " } else { s.as_str() };
                items.push(json!({
                    "role": role,
                    "content": [{
                        "type": if role == "assistant" { "output_text" } else { "input_text" },
                        "text": text
                    }]
                }));
            }
            continue;
        }

        let Some(Value::Array(blocks)) = msg_obj.get("content") else {
            continue;
        };

        if role == "user" {
            let mut pending: Vec<Value> = Vec::new();
            let flush_user = |pending: &mut Vec<Value>, items: &mut Vec<Value>| {
                if !pending.is_empty() {
                    items.push(json!({ "role": "user", "content": std::mem::take(pending) }));
                }
            };

            for blk in blocks {
                let Some(obj) = blk.as_object() else {
                    continue;
                };
                match obj.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text" => {
                        if let Some(t) = obj.get("text").and_then(Value::as_str) {
                            if !t.is_empty() {
                                pending.push(json!({ "type": "input_text", "text": t }));
                            }
                        }
                    }
                    "image" => {
                        let detail = obj.get("detail").and_then(Value::as_str);
                        if let Some(img) = convert_image_source(obj.get("source"), detail) {
                            pending.push(img);
                        }
                    }
                    "document" => {
                        if let Some(doc) = convert_document_block(obj) {
                            pending.push(doc);
                        }
                    }
                    "audio" | "input_audio" => {
                        if let Some(aud) = convert_audio_block(obj) {
                            pending.push(aud);
                        }
                    }
                    "tool_reference" => {
                        let name = obj
                            .get("tool_name")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown");
                        pending.push(json!({
                            "type": "input_text",
                            "text": format!("[Tool loaded: {name}]")
                        }));
                    }
                    "tool_result" => {
                        flush_user(&mut pending, &mut items);
                        let call_id =
                            normalize_call_id(obj.get("tool_use_id").and_then(Value::as_str));
                        let (output, embedded_images) = extract_tool_result_payload(obj);
                        items.push(json!({
                            "type": "function_call_output",
                            "call_id": call_id,
                            "output": output
                        }));
                        if !embedded_images.is_empty() {
                            let mut follow_up = vec![json!({
                                "type": "input_text",
                                "text": format!("[Visual output from tool call {call_id}]")
                            })];
                            follow_up.extend(embedded_images);
                            items.push(json!({ "role": "user", "content": follow_up }));
                        }
                    }
                    _ => {}
                }
            }
            flush_user(&mut pending, &mut items);
        } else {
            let mut pending: Vec<Value> = Vec::new();
            let flush_assistant = |pending: &mut Vec<Value>, items: &mut Vec<Value>| {
                if !pending.is_empty() {
                    items.push(json!({ "role": "assistant", "content": std::mem::take(pending) }));
                }
            };

            for blk in blocks {
                let Some(obj) = blk.as_object() else {
                    continue;
                };
                match obj.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text" => {
                        if let Some(t) = obj.get("text").and_then(Value::as_str) {
                            if !t.is_empty() {
                                pending.push(json!({ "type": "output_text", "text": t }));
                            }
                        }
                    }
                    "tool_use" => {
                        flush_assistant(&mut pending, &mut items);
                        let call_id = normalize_call_id(obj.get("id").and_then(Value::as_str));
                        let name = obj.get("name").and_then(Value::as_str).unwrap_or("tool");
                        let wire_name = tool_codec.to_wire_name(name);
                        let args_str = match obj.get("input") {
                            Some(Value::String(s)) => s.clone(),
                            Some(v) => v.to_string(),
                            None => "{}".to_string(),
                        };
                        items.push(json!({
                            "type": "function_call",
                            "call_id": call_id,
                            "name": wire_name,
                            "arguments": args_str
                        }));
                    }
                    _ => {}
                }
            }
            flush_assistant(&mut pending, &mut items);
        }
    }

    let reconciled = reconcile_function_call_pairs(items);
    if reconciled.is_empty() {
        vec![json!({ "role": "user", "content": [{ "type": "input_text", "text": " " }] })]
    } else {
        reconciled
    }
}

fn normalize_reasoning_effort(raw: &str, fallback: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "none" | "off" => "none".to_string(),
        "minimal" => "minimal".to_string(),
        "low" => "low".to_string(),
        "medium" => "medium".to_string(),
        "high" => "high".to_string(),
        "xhigh" | "max" => "xhigh".to_string(),
        _ => fallback.to_string(),
    }
}

fn resolve_reasoning_config(body: &Value, config: &BridgeConfig, has_tools: bool) -> Value {
    let max_tokens = body.get("max_tokens").and_then(Value::as_u64).unwrap_or(0);
    if !has_tools && max_tokens > 0 && max_tokens <= 512 {
        return json!({ "effort": "low" });
    }

    let thinking_type = body
        .get("thinking")
        .and_then(|t| t.get("type"))
        .and_then(Value::as_str);

    let requested_effort = body
        .get("output_config")
        .and_then(|o| o.get("effort"))
        .and_then(Value::as_str)
        .or_else(|| body.get("reasoning_effort").and_then(Value::as_str));

    if thinking_type == Some("disabled") && requested_effort.is_none() {
        return json!({ "effort": "none" });
    }

    let mut effort = normalize_reasoning_effort(&config.default_reasoning_effort, "medium");
    if let Some(req_eff) = requested_effort {
        effort = normalize_reasoning_effort(req_eff, &effort);
    } else if thinking_type == Some("enabled") {
        if let Some(budget) = body
            .get("thinking")
            .and_then(|t| t.get("budget_tokens"))
            .and_then(Value::as_u64)
        {
            effort = if budget <= 1024 {
                "minimal".to_string()
            } else if budget <= 3000 {
                "low".to_string()
            } else if budget <= 10000 {
                "medium".to_string()
            } else if budget <= 24000 {
                "high".to_string()
            } else {
                "xhigh".to_string()
            };
        }
    }

    let mut reasoning = Map::new();
    reasoning.insert("effort".to_string(), Value::String(effort.clone()));
    if effort != "none" && !config.reasoning_summary.is_empty() && config.reasoning_summary != "none"
    {
        reasoning.insert(
            "summary".to_string(),
            Value::String(config.reasoning_summary.clone()),
        );
    }
    Value::Object(reasoning)
}

pub struct ConvertedRequest {
    pub payload: Value,
    pub upstream_model: String,
    pub requested_model: String,
    pub tool_codec: ToolNameCodec,
    pub is_heavy_request: bool,
}

pub fn convert_anthropic_to_responses(
    body: &Value,
    session_id: Option<&str>,
    config: &BridgeConfig,
) -> ConvertedRequest {
    let mut tool_codec = ToolNameCodec::default();
    let has_tools = body
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty());
    let max_tokens = body
        .get("max_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(16384);
    let is_bg_title = !has_tools && max_tokens > 0 && max_tokens <= 256;

    let req_model_opt = body.get("model").and_then(Value::as_str);
    let upstream_model = resolve_upstream_model(req_model_opt, config, is_bg_title);
    let requested_model = req_model_opt
        .map(str::to_string)
        .unwrap_or_else(|| upstream_model.clone());
    let instructions = extract_system_instructions(body.get("system"));

    let mut tools = Vec::new();
    if let Some(raw_tools) = body.get("tools").and_then(Value::as_array) {
        for tool in raw_tools {
            let Some(t_obj) = tool.as_object() else {
                continue;
            };
            let is_web_search = t_obj
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|s| s.starts_with("web_search"))
                || (t_obj.get("name").and_then(Value::as_str) == Some("web_search")
                    && !t_obj.contains_key("input_schema"));
            if is_web_search {
                tools.push(json!({ "type": "web_search_preview" }));
                continue;
            }
            let Some(name) = t_obj.get("name").and_then(Value::as_str) else {
                continue;
            };
            let wire_name = tool_codec.to_wire_name(name);
            let desc = t_obj
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("");
            let params = sanitize_json_schema(t_obj.get("input_schema").unwrap_or(&Value::Null));
            tools.push(json!({
                "type": "function",
                "name": wire_name,
                "description": desc,
                "parameters": params,
                "strict": false
            }));
        }
    }

    let input = convert_messages_to_input(body.get("messages"), &mut tool_codec);
    let reasoning = resolve_reasoning_config(body, config, !tools.is_empty());
    let effort_str = reasoning
        .get("effort")
        .and_then(Value::as_str)
        .unwrap_or("medium");
    let headroom = config.min_reasoning_token_headroom.max(512);
    let max_output_tokens = if effort_str == "none" {
        max_tokens.max(16).min(64000)
    } else {
        let base = if max_tokens < headroom {
            max_tokens + headroom
        } else {
            max_tokens
        };
        base.max(headroom).min(64000)
    };

    let mut payload = Map::new();
    payload.insert("model".to_string(), Value::String(upstream_model.clone()));
    payload.insert("store".to_string(), Value::Bool(false));
    payload.insert(
        "stream".to_string(),
        Value::Bool(
            body.get("stream")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    let input_val = Value::Array(input);
    let input_json_len = input_val.to_string().len();
    payload.insert("input".to_string(), input_val);
    payload.insert(
        "max_output_tokens".to_string(),
        Value::Number(max_output_tokens.into()),
    );
    payload.insert("reasoning".to_string(), reasoning);
    payload.insert(
        "prompt_cache_key".to_string(),
        Value::String(derive_prompt_cache_key(
            session_id,
            &instructions,
            &upstream_model,
        )),
    );

    if !instructions.is_empty() {
        payload.insert("instructions".to_string(), Value::String(instructions));
    }

    let tools_non_empty = !tools.is_empty();
    if tools_non_empty {
        payload.insert("tools".to_string(), Value::Array(tools));
        if let Some(tc) = body.get("tool_choice") {
            match tc {
                Value::Object(obj) => {
                    match obj.get("type").and_then(Value::as_str).unwrap_or("") {
                        "auto" => {
                            payload.insert("tool_choice".to_string(), json!("auto"));
                        }
                        "any" | "tool" => {
                            payload.insert("tool_choice".to_string(), json!("required"));
                        }
                        "none" => {
                            payload.insert("tool_choice".to_string(), json!("none"));
                        }
                        _ => {}
                    }
                    if obj.get("disable_parallel_tool_use").and_then(Value::as_bool) == Some(true)
                    {
                        payload.insert("parallel_tool_calls".to_string(), Value::Bool(false));
                    }
                }
                Value::String(s) => {
                    payload.insert("tool_choice".to_string(), Value::String(s.clone()));
                }
                _ => {}
            }
        }
    }

    ConvertedRequest {
        payload: Value::Object(payload),
        upstream_model,
        requested_model,
        tool_codec,
        is_heavy_request: tools_non_empty || input_json_len > 30000,
    }
}

pub fn estimate_anthropic_input_tokens(body: &Value) -> u64 {
    let mut chars: usize = 0;
    if let Some(sys) = body.get("system") {
        match sys {
            Value::String(s) => chars += s.len(),
            Value::Array(_) => chars += sys.to_string().len(),
            _ => {}
        }
    }
    if let Some(Value::Array(msgs)) = body.get("messages") {
        for msg in msgs {
            match msg.get("content") {
                Some(Value::String(s)) => chars += s.len(),
                Some(Value::Array(blks)) => {
                    for blk in blks {
                        if blk.get("type").and_then(Value::as_str) == Some("image") {
                            chars += 3200;
                        } else {
                            chars += blk.to_string().len();
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(tools) = body.get("tools") {
        if tools.is_array() {
            chars += tools.to_string().len();
        }
    }
    ((chars as f64 / 3.5).ceil() as u64).max(1)
}

pub fn estimate_responses_input_tokens(body: &Value) -> u64 {
    let mut chars: usize = 0;
    if let Some(inst) = body.get("instructions").and_then(Value::as_str) {
        chars += inst.len();
    }
    if let Some(inp) = body.get("input") {
        match inp {
            Value::String(s) => chars += s.len(),
            Value::Array(_) => chars += inp.to_string().len(),
            _ => {}
        }
    }
    if let Some(tools) = body.get("tools") {
        if tools.is_array() {
            chars += tools.to_string().len().min(160_000);
        }
    }
    ((chars as f64 / 3.6).round() as u64).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_xhigh_max_minimal_disabled_and_audio_blocks() {
        let cfg = BridgeConfig::default();

        let xhigh_req = json!({
            "model": "claude-opus-4-6",
            "max_tokens": 16000,
            "output_config": { "effort": "max" },
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "text", "text": "Inspect this audio" },
                    { "type": "audio", "source": { "type": "base64", "media_type": "audio/mp3", "data": "SUQz" } }
                ]
            }]
        });
        let r1 = convert_anthropic_to_responses(&xhigh_req, Some("s1"), &cfg);
        assert_eq!(r1.payload["reasoning"]["effort"], "xhigh");
        assert_eq!(r1.payload["input"][0]["content"][1]["type"], "input_audio");
        assert_eq!(
            r1.payload["input"][0]["content"][1]["input_audio"]["format"],
            "mp3"
        );

        let disabled_req = json!({
            "model": "claude-haiku-4-5",
            "max_tokens": 2000,
            "thinking": { "type": "disabled" },
            "messages": [{ "role": "user", "content": "hi" }]
        });
        let r2 = convert_anthropic_to_responses(&disabled_req, None, &cfg);
        assert_eq!(r2.payload["reasoning"]["effort"], "none");
    }

    #[test]
    fn converts_multimodal_and_recovers_orphan_tool_calls() {
        let cfg = BridgeConfig::default();
        let req = json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 2048,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "AAAA" }, "detail": "high" }
                    ]
                },
                {
                    "role": "assistant",
                    "content": [
                        { "type": "tool_use", "id": "call_orphan", "name": "read_file", "input": { "path": "x" } }
                    ]
                }
            ]
        });
        let conv = convert_anthropic_to_responses(&req, None, &cfg);
        let inp = conv.payload["input"].as_array().unwrap();
        assert_eq!(inp[0]["content"][0]["detail"], "high");
        assert_eq!(inp[1]["type"], "function_call");
        assert_eq!(inp[2]["type"], "function_call_output");
        assert_eq!(inp[2]["call_id"], "call_orphan");
    }
}

