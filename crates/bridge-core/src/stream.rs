use crate::sanitizer::ToolNameCodec;
use crate::translator::normalize_call_id;
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

pub fn extract_anthropic_usage(resp_usage: Option<&Value>) -> Value {
    let Some(u) = resp_usage.and_then(Value::as_object) else {
        return json!({
            "input_tokens": 0,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 0,
            "output_tokens": 0
        });
    };
    let total_input = u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0);
    let details = u.get("input_tokens_details").and_then(Value::as_object);
    let cached = details
        .and_then(|d| d.get("cached_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_write = details
        .and_then(|d| d.get("cache_write_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let uncached = total_input.saturating_sub(cached).saturating_sub(cache_write);
    let output_tokens = u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0);

    json!({
        "input_tokens": uncached,
        "cache_creation_input_tokens": cache_write,
        "cache_read_input_tokens": cached,
        "output_tokens": output_tokens
    })
}

fn safe_parse_json_object(raw: Option<&str>) -> Value {
    let Some(s) = raw else {
        return json!({});
    };
    if s.is_empty() {
        return json!({});
    }
    match serde_json::from_str::<Value>(s) {
        Ok(Value::Object(map)) => Value::Object(map),
        Ok(other) => json!({ "value": other }),
        Err(_) => json!({}),
    }
}

pub fn convert_responses_json_to_anthropic(
    resp_json: &Value,
    requested_model: &str,
    tool_codec: Option<&ToolNameCodec>,
) -> Value {
    let mut content = Vec::new();
    let mut has_tool_use = false;

    if let Some(outputs) = resp_json.get("output").and_then(Value::as_array) {
        for item in outputs {
            let Some(obj) = item.as_object() else {
                continue;
            };
            match obj.get("type").and_then(Value::as_str).unwrap_or("") {
                "reasoning" => {
                    let summary_text = obj
                        .get("summary")
                        .and_then(Value::as_array)
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|s| s.get("text").and_then(Value::as_str))
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default();
                    if !summary_text.is_empty() {
                        let sig_id = obj
                            .get("id")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .unwrap_or_else(|| Uuid::new_v4().simple().to_string());
                        content.push(json!({
                            "type": "thinking",
                            "thinking": summary_text,
                            "signature": format!("sig_crb_{sig_id}")
                        }));
                    }
                }
                "message" => {
                    if let Some(parts) = obj.get("content").and_then(Value::as_array) {
                        for part in parts {
                            let pt = part.get("type").and_then(Value::as_str).unwrap_or("");
                            if (pt == "output_text" || pt == "text")
                                && part.get("text").and_then(Value::as_str).is_some()
                            {
                                content.push(json!({
                                    "type": "text",
                                    "text": part["text"]
                                }));
                            } else if pt == "refusal" {
                                if let Some(refusal) = part.get("refusal").and_then(Value::as_str) {
                                    content.push(json!({
                                        "type": "text",
                                        "text": refusal
                                    }));
                                }
                            }
                        }
                    }
                }
                "function_call" => {
                    has_tool_use = true;
                    let wire_name = obj.get("name").and_then(Value::as_str).unwrap_or("tool");
                    let orig_name = tool_codec
                        .map(|c| c.to_original_name(wire_name))
                        .unwrap_or(wire_name);
                    let id = normalize_call_id(
                        obj.get("call_id")
                            .or_else(|| obj.get("id"))
                            .and_then(Value::as_str),
                    );
                    content.push(json!({
                        "type": "tool_use",
                        "id": id,
                        "name": orig_name,
                        "input": safe_parse_json_object(obj.get("arguments").and_then(Value::as_str))
                    }));
                }
                _ => {}
            }
        }
    }

    if content.is_empty() {
        if let Some(out_txt) = resp_json.get("output_text").and_then(Value::as_str) {
            if !out_txt.is_empty() {
                content.push(json!({ "type": "text", "text": out_txt }));
            }
        }
    }

    let stop_reason = if has_tool_use {
        "tool_use"
    } else if resp_json.get("status").and_then(Value::as_str) == Some("incomplete")
        && resp_json
            .get("incomplete_details")
            .and_then(|d| d.get("reason"))
            .and_then(Value::as_str)
            == Some("max_output_tokens")
    {
        "max_tokens"
    } else {
        "end_turn"
    };

    let raw_id = resp_json
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let clean_id = raw_id.strip_prefix("resp_").unwrap_or(&raw_id);

    json!({
        "id": format!("msg_{clean_id}"),
        "type": "message",
        "role": "assistant",
        "model": requested_model,
        "content": content,
        "stop_reason": stop_reason,
        "stop_sequence": Value::Null,
        "usage": extract_anthropic_usage(resp_json.get("usage"))
    })
}

#[derive(Debug, Clone)]
pub struct SseFrame {
    pub done: bool,
    pub event_name: Option<String>,
    pub raw_data: String,
    pub data_obj: Option<Value>,
    pub raw_frame: String,
}

#[derive(Debug, Default)]
pub struct SseFrameParser {
    buffer: String,
}

impl SseFrameParser {
    pub fn push(&mut self, chunk: &str) -> Vec<SseFrame> {
        self.buffer.push_str(&chunk.replace("\r\n", "\n"));
        let mut out = Vec::new();
        while let Some(idx) = self.buffer.find("\n\n") {
            let block = self.buffer[..idx].to_string();
            self.buffer.drain(..idx + 2);
            if let Some(frame) = Self::parse_block(&block) {
                out.push(frame);
            }
        }
        out
    }

    pub fn flush(&mut self) -> Vec<SseFrame> {
        let mut out = Vec::new();
        if !self.buffer.trim().is_empty() {
            let remaining = std::mem::take(&mut self.buffer);
            if let Some(frame) = Self::parse_block(&remaining) {
                out.push(frame);
            }
        } else {
            self.buffer.clear();
        }
        out
    }

    fn parse_block(raw_block: &str) -> Option<SseFrame> {
        let normalized = raw_block.replace("\r\n", "\n").trim().to_string();
        if normalized.is_empty() {
            return None;
        }
        let mut event_name = None;
        let mut data_lines = Vec::new();

        for line in normalized.lines() {
            if line.starts_with(':') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("event:") {
                event_name = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("data:") {
                data_lines.push(rest.trim_start().to_string());
            }
        }

        if data_lines.is_empty() {
            return None;
        }
        let raw_data = data_lines.join("\n");
        if raw_data == "[DONE]" {
            return Some(SseFrame {
                done: true,
                event_name,
                raw_data,
                data_obj: None,
                raw_frame: normalized,
            });
        }

        let data_obj = serde_json::from_str::<Value>(&raw_data).ok();
        Some(SseFrame {
            done: false,
            event_name,
            raw_data,
            data_obj,
            raw_frame: normalized,
        })
    }
}

#[derive(Debug, Clone)]
enum BlockKind {
    Thinking,
    Text,
    ToolUse,
}

#[derive(Debug, Clone)]
struct StreamBlock {
    index: usize,
    kind: BlockKind,
    closed: bool,
    signature_sent: bool,
    signature: String,
    emitted_args: bool,
}

pub struct ResponsesToAnthropicStreamProcessor {
    requested_model: String,
    tool_codec: ToolNameCodec,
    started: bool,
    finished: bool,
    next_block_index: usize,
    has_tool_use: bool,
    final_response_obj: Option<Value>,
    block_order: Vec<String>,
    blocks: HashMap<String, StreamBlock>,
    output_index_to_item_id: HashMap<u64, String>,
}

impl ResponsesToAnthropicStreamProcessor {
    pub fn new(requested_model: String, tool_codec: ToolNameCodec) -> Self {
        Self {
            requested_model,
            tool_codec,
            started: false,
            finished: false,
            next_block_index: 0,
            has_tool_use: false,
            final_response_obj: None,
            block_order: Vec::new(),
            blocks: HashMap::new(),
            output_index_to_item_id: HashMap::new(),
        }
    }

    fn ensure_message_start(&mut self, resp_id: Option<&str>, out: &mut Vec<(String, Value)>) {
        if self.started {
            return;
        }
        self.started = true;
        let raw = resp_id
            .map(str::to_string)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let clean_id = raw.strip_prefix("resp_").unwrap_or(&raw);
        out.push((
            "message_start".to_string(),
            json!({
                "type": "message_start",
                "message": {
                    "id": format!("msg_{clean_id}"),
                    "type": "message",
                    "role": "assistant",
                    "model": self.requested_model,
                    "content": [],
                    "stop_reason": Value::Null,
                    "stop_sequence": Value::Null,
                    "usage": {
                        "input_tokens": 0,
                        "cache_creation_input_tokens": 0,
                        "cache_read_input_tokens": 0,
                        "output_tokens": 0
                    }
                }
            }),
        ));
    }

    fn open_thinking_block(
        &mut self,
        key: &str,
        item_id: &str,
        out: &mut Vec<(String, Value)>,
    ) -> usize {
        if let Some(blk) = self.blocks.get(key) {
            return blk.index;
        }
        self.ensure_message_start(None, out);
        let index = self.next_block_index;
        self.next_block_index += 1;
        let blk = StreamBlock {
            index,
            kind: BlockKind::Thinking,
            closed: false,
            signature_sent: false,
            signature: format!("sig_crb_{item_id}"),
            emitted_args: false,
        };
        self.block_order.push(key.to_string());
        self.blocks.insert(key.to_string(), blk);
        out.push((
            "content_block_start".to_string(),
            json!({
                "type": "content_block_start",
                "index": index,
                "content_block": { "type": "thinking", "thinking": "" }
            }),
        ));
        index
    }

    fn open_text_block(&mut self, key: &str, out: &mut Vec<(String, Value)>) -> usize {
        if let Some(blk) = self.blocks.get(key) {
            return blk.index;
        }
        self.ensure_message_start(None, out);
        let index = self.next_block_index;
        self.next_block_index += 1;
        let blk = StreamBlock {
            index,
            kind: BlockKind::Text,
            closed: false,
            signature_sent: false,
            signature: String::new(),
            emitted_args: false,
        };
        self.block_order.push(key.to_string());
        self.blocks.insert(key.to_string(), blk);
        out.push((
            "content_block_start".to_string(),
            json!({
                "type": "content_block_start",
                "index": index,
                "content_block": { "type": "text", "text": "" }
            }),
        ));
        index
    }

    fn open_tool_block(
        &mut self,
        key: &str,
        call_id: Option<&str>,
        wire_name: &str,
        out: &mut Vec<(String, Value)>,
    ) -> usize {
        if let Some(blk) = self.blocks.get(key) {
            return blk.index;
        }
        self.ensure_message_start(None, out);
        self.has_tool_use = true;
        let index = self.next_block_index;
        self.next_block_index += 1;
        let orig_name = self.tool_codec.to_original_name(wire_name).to_string();
        let norm_id = normalize_call_id(call_id);
        let blk = StreamBlock {
            index,
            kind: BlockKind::ToolUse,
            closed: false,
            signature_sent: false,
            signature: String::new(),
            emitted_args: false,
        };
        self.block_order.push(key.to_string());
        self.blocks.insert(key.to_string(), blk);
        out.push((
            "content_block_start".to_string(),
            json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {
                    "type": "tool_use",
                    "id": norm_id,
                    "name": orig_name,
                    "input": {}
                }
            }),
        ));
        index
    }

    fn close_block(&mut self, key: &str, out: &mut Vec<(String, Value)>) {
        let Some(blk) = self.blocks.get_mut(key) else {
            return;
        };
        if blk.closed {
            return;
        }
        match blk.kind {
            BlockKind::Thinking if !blk.signature_sent => {
                blk.signature_sent = true;
                out.push((
                    "content_block_delta".to_string(),
                    json!({
                        "type": "content_block_delta",
                        "index": blk.index,
                        "delta": { "type": "signature_delta", "signature": blk.signature }
                    }),
                ));
            }
            BlockKind::ToolUse if !blk.emitted_args => {
                blk.emitted_args = true;
                out.push((
                    "content_block_delta".to_string(),
                    json!({
                        "type": "content_block_delta",
                        "index": blk.index,
                        "delta": { "type": "input_json_delta", "partial_json": "{}" }
                    }),
                ));
            }
            _ => {}
        }
        blk.closed = true;
        out.push((
            "content_block_stop".to_string(),
            json!({
                "type": "content_block_stop",
                "index": blk.index
            }),
        ));
    }

    pub fn handle_responses_event(&mut self, evt: &Value) -> Vec<(String, Value)> {
        let mut out = Vec::new();
        if self.finished {
            return out;
        }
        let Some(evt_obj) = evt.as_object() else {
            return out;
        };
        let typ = evt_obj.get("type").and_then(Value::as_str).unwrap_or("");

        match typ {
            "response.created" | "response.in_progress" => {
                let rid = evt_obj
                    .get("response")
                    .and_then(|r| r.get("id"))
                    .and_then(Value::as_str);
                self.ensure_message_start(rid, &mut out);
            }
            "response.output_item.added" => {
                self.ensure_message_start(None, &mut out);
                if let Some(item) = evt_obj.get("item").and_then(Value::as_object) {
                    let out_idx = evt_obj.get("output_index").and_then(Value::as_u64);
                    if let (Some(id), Some(oi)) =
                        (item.get("id").and_then(Value::as_str), out_idx)
                    {
                        self.output_index_to_item_id.insert(oi, id.to_string());
                    }
                    if item.get("type").and_then(Value::as_str) == Some("function_call") {
                        let key_suffix = item
                            .get("id")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .unwrap_or_else(|| out_idx.unwrap_or(0).to_string());
                        let key = format!("fc:{key_suffix}");
                        let call_id = item
                            .get("call_id")
                            .or_else(|| item.get("id"))
                            .and_then(Value::as_str);
                        let name = item.get("name").and_then(Value::as_str).unwrap_or("tool");
                        self.open_tool_block(&key, call_id, name, &mut out);
                    }
                }
            }
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                if let Some(delta) = evt_obj.get("delta").and_then(Value::as_str) {
                    if !delta.is_empty() {
                        let out_idx = evt_obj.get("output_index").and_then(Value::as_u64);
                        let item_id = evt_obj
                            .get("item_id")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .or_else(|| {
                                out_idx.and_then(|oi| self.output_index_to_item_id.get(&oi).cloned())
                            })
                            .unwrap_or_else(|| format!("r_{}", out_idx.unwrap_or(0)));
                        let key = format!("thinking:{item_id}");
                        let idx = self.open_thinking_block(&key, &item_id, &mut out);
                        out.push((
                            "content_block_delta".to_string(),
                            json!({
                                "type": "content_block_delta",
                                "index": idx,
                                "delta": { "type": "thinking_delta", "thinking": delta }
                            }),
                        ));
                    }
                }
            }
            "response.output_text.delta" => {
                if let Some(delta) = evt_obj.get("delta").and_then(Value::as_str) {
                    if !delta.is_empty() {
                        let out_idx = evt_obj.get("output_index").and_then(Value::as_u64);
                        let item_id = evt_obj
                            .get("item_id")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .or_else(|| {
                                out_idx.and_then(|oi| self.output_index_to_item_id.get(&oi).cloned())
                            })
                            .unwrap_or_else(|| format!("m_{}", out_idx.unwrap_or(0)));
                        let key = format!("text:{item_id}");
                        let idx = self.open_text_block(&key, &mut out);
                        out.push((
                            "content_block_delta".to_string(),
                            json!({
                                "type": "content_block_delta",
                                "index": idx,
                                "delta": { "type": "text_delta", "text": delta }
                            }),
                        ));
                    }
                }
            }
            "response.function_call_arguments.delta" => {
                if let Some(delta) = evt_obj.get("delta").and_then(Value::as_str) {
                    if !delta.is_empty() {
                        let out_idx = evt_obj.get("output_index").and_then(Value::as_u64);
                        let item_id = evt_obj
                            .get("item_id")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .or_else(|| {
                                out_idx.and_then(|oi| self.output_index_to_item_id.get(&oi).cloned())
                            })
                            .unwrap_or_else(|| out_idx.unwrap_or(0).to_string());
                        let key = format!("fc:{item_id}");
                        let idx = if let Some(b) = self.blocks.get(&key) {
                            b.index
                        } else {
                            let name = evt_obj
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("tool");
                            self.open_tool_block(&key, Some(&item_id), name, &mut out)
                        };
                        if let Some(b) = self.blocks.get_mut(&key) {
                            b.emitted_args = true;
                        }
                        out.push((
                            "content_block_delta".to_string(),
                            json!({
                                "type": "content_block_delta",
                                "index": idx,
                                "delta": { "type": "input_json_delta", "partial_json": delta }
                            }),
                        ));
                    }
                }
            }
            "response.output_item.done" => {
                if let Some(item) = evt_obj.get("item").and_then(Value::as_object) {
                    let out_idx = evt_obj.get("output_index").and_then(Value::as_u64);
                    let item_id = item
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| {
                            out_idx.and_then(|oi| self.output_index_to_item_id.get(&oi).cloned())
                        })
                        .unwrap_or_else(|| out_idx.unwrap_or(0).to_string());

                    match item.get("type").and_then(Value::as_str).unwrap_or("") {
                        "reasoning" => {
                            let key = format!("thinking:{item_id}");
                            if !self.blocks.contains_key(&key) {
                                if let Some(summary) = item.get("summary").and_then(Value::as_array)
                                {
                                    let txt: String = summary
                                        .iter()
                                        .filter_map(|s| s.get("text").and_then(Value::as_str))
                                        .filter(|s| !s.is_empty())
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    if !txt.is_empty() {
                                        let idx =
                                            self.open_thinking_block(&key, &item_id, &mut out);
                                        out.push((
                                            "content_block_delta".to_string(),
                                            json!({
                                                "type": "content_block_delta",
                                                "index": idx,
                                                "delta": { "type": "thinking_delta", "thinking": txt }
                                            }),
                                        ));
                                    }
                                }
                            }
                            self.close_block(&key, &mut out);
                        }
                        "message" => {
                            let key = format!("text:{item_id}");
                            if !self.blocks.contains_key(&key) {
                                if let Some(parts) = item.get("content").and_then(Value::as_array) {
                                    let txt: String = parts
                                        .iter()
                                        .filter_map(|c| c.get("text").and_then(Value::as_str))
                                        .collect::<String>();
                                    if !txt.is_empty() {
                                        let idx = self.open_text_block(&key, &mut out);
                                        out.push((
                                            "content_block_delta".to_string(),
                                            json!({
                                                "type": "content_block_delta",
                                                "index": idx,
                                                "delta": { "type": "text_delta", "text": txt }
                                            }),
                                        ));
                                    }
                                }
                            }
                            self.close_block(&key, &mut out);
                        }
                        "function_call" => {
                            let key = format!("fc:{item_id}");
                            let idx = if let Some(b) = self.blocks.get(&key) {
                                b.index
                            } else {
                                let cid = item
                                    .get("call_id")
                                    .and_then(Value::as_str)
                                    .unwrap_or(&item_id);
                                let name =
                                    item.get("name").and_then(Value::as_str).unwrap_or("tool");
                                self.open_tool_block(&key, Some(cid), name, &mut out)
                            };
                            let already = self
                                .blocks
                                .get(&key)
                                .map(|b| b.emitted_args)
                                .unwrap_or(false);
                            if !already {
                                if let Some(args) = item.get("arguments").and_then(Value::as_str) {
                                    if !args.is_empty() {
                                        if let Some(b) = self.blocks.get_mut(&key) {
                                            b.emitted_args = true;
                                        }
                                        out.push((
                                            "content_block_delta".to_string(),
                                            json!({
                                                "type": "content_block_delta",
                                                "index": idx,
                                                "delta": { "type": "input_json_delta", "partial_json": args }
                                            }),
                                        ));
                                    }
                                }
                            }
                            self.close_block(&key, &mut out);
                        }
                        _ => {}
                    }
                }
            }
            "response.completed" | "response.incomplete" => {
                self.final_response_obj = evt_obj.get("response").cloned();
                self.finalize_into(&mut out);
            }
            _ => {}
        }

        out
    }

    pub fn finalize(&mut self) -> Vec<(String, Value)> {
        let mut out = Vec::new();
        self.finalize_into(&mut out);
        out
    }

    fn finalize_into(&mut self, out: &mut Vec<(String, Value)>) {
        if self.finished {
            return;
        }
        let resp_id = self
            .final_response_obj
            .as_ref()
            .and_then(|r| r.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string);
        self.ensure_message_start(resp_id.as_deref(), out);

        if self.next_block_index == 0 {
            if let Some(ref resp) = self.final_response_obj {
                let synthesized = convert_responses_json_to_anthropic(
                    resp,
                    &self.requested_model,
                    Some(&self.tool_codec),
                );
                if let Some(blocks) = synthesized.get("content").and_then(Value::as_array) {
                    for blk in blocks {
                        let idx = self.next_block_index;
                        self.next_block_index += 1;
                        match blk.get("type").and_then(Value::as_str).unwrap_or("") {
                            "thinking" => {
                                out.push((
                                    "content_block_start".to_string(),
                                    json!({ "type": "content_block_start", "index": idx, "content_block": { "type": "thinking", "thinking": "" } }),
                                ));
                                out.push((
                                    "content_block_delta".to_string(),
                                    json!({ "type": "content_block_delta", "index": idx, "delta": { "type": "thinking_delta", "thinking": blk["thinking"] } }),
                                ));
                                out.push((
                                    "content_block_delta".to_string(),
                                    json!({ "type": "content_block_delta", "index": idx, "delta": { "type": "signature_delta", "signature": blk["signature"] } }),
                                ));
                                out.push((
                                    "content_block_stop".to_string(),
                                    json!({ "type": "content_block_stop", "index": idx }),
                                ));
                            }
                            "text" => {
                                out.push((
                                    "content_block_start".to_string(),
                                    json!({ "type": "content_block_start", "index": idx, "content_block": { "type": "text", "text": "" } }),
                                ));
                                out.push((
                                    "content_block_delta".to_string(),
                                    json!({ "type": "content_block_delta", "index": idx, "delta": { "type": "text_delta", "text": blk["text"] } }),
                                ));
                                out.push((
                                    "content_block_stop".to_string(),
                                    json!({ "type": "content_block_stop", "index": idx }),
                                ));
                            }
                            "tool_use" => {
                                self.has_tool_use = true;
                                let input_json = blk
                                    .get("input")
                                    .cloned()
                                    .unwrap_or_else(|| json!({}))
                                    .to_string();
                                out.push((
                                    "content_block_start".to_string(),
                                    json!({ "type": "content_block_start", "index": idx, "content_block": { "type": "tool_use", "id": blk["id"], "name": blk["name"], "input": {} } }),
                                ));
                                out.push((
                                    "content_block_delta".to_string(),
                                    json!({ "type": "content_block_delta", "index": idx, "delta": { "type": "input_json_delta", "partial_json": input_json } }),
                                ));
                                out.push((
                                    "content_block_stop".to_string(),
                                    json!({ "type": "content_block_stop", "index": idx }),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        let keys = self.block_order.clone();
        for key in keys {
            self.close_block(&key, out);
        }

        let stop_reason = if self.has_tool_use {
            "tool_use"
        } else if self
            .final_response_obj
            .as_ref()
            .and_then(|r| r.get("status"))
            .and_then(Value::as_str)
            == Some("incomplete")
            && self
                .final_response_obj
                .as_ref()
                .and_then(|r| r.get("incomplete_details"))
                .and_then(|d| d.get("reason"))
                .and_then(Value::as_str)
                == Some("max_output_tokens")
        {
            "max_tokens"
        } else {
            "end_turn"
        };

        let usage = extract_anthropic_usage(
            self.final_response_obj
                .as_ref()
                .and_then(|r| r.get("usage")),
        );

        out.push((
            "message_delta".to_string(),
            json!({
                "type": "message_delta",
                "delta": { "stop_reason": stop_reason, "stop_sequence": Value::Null },
                "usage": usage
            }),
        ));
        out.push(("message_stop".to_string(), json!({ "type": "message_stop" })));
        self.finished = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_thinking_text_tool_and_cached_usage() {
        let mut codec = ToolNameCodec::default();
        let long_name = format!("mcp__srv__{}", "fn_".repeat(30));
        let wire = codec.to_wire_name(&long_name);

        let mut proc =
            ResponsesToAnthropicStreamProcessor::new("claude-opus-4-6".to_string(), codec);
        let mut events = Vec::new();
        events.extend(proc.handle_responses_event(&json!({ "type": "response.created", "response": { "id": "resp_1" } })));
        events.extend(proc.handle_responses_event(&json!({ "type": "response.reasoning_summary_text.delta", "output_index": 0, "delta": "step 1" })));
        events.extend(proc.handle_responses_event(&json!({ "type": "response.output_item.done", "output_index": 0, "item": { "type": "reasoning", "id": "r0" } })));
        events.extend(proc.handle_responses_event(&json!({ "type": "response.output_text.delta", "output_index": 1, "delta": "Hello" })));
        events.extend(proc.handle_responses_event(&json!({ "type": "response.output_item.done", "output_index": 1, "item": { "type": "message", "id": "m1" } })));
        events.extend(proc.handle_responses_event(&json!({
            "type": "response.output_item.added",
            "output_index": 2,
            "item": { "type": "function_call", "id": "fc1", "call_id": "call_1", "name": wire }
        })));
        events.extend(proc.handle_responses_event(&json!({
            "type": "response.function_call_arguments.delta",
            "output_index": 2,
            "item_id": "fc1",
            "delta": "{\"a\":1}"
        })));
        events.extend(proc.handle_responses_event(&json!({
            "type": "response.completed",
            "response": {
                "id": "resp_1",
                "usage": { "input_tokens": 100, "output_tokens": 25, "input_tokens_details": { "cached_tokens": 80 } }
            }
        })));

        assert!(events.iter().any(|(n, _)| n == "message_start"));
        assert!(events
            .iter()
            .any(|(n, d)| n == "content_block_delta" && d["delta"]["type"] == "signature_delta"));
        assert!(events.iter().any(|(n, d)| n == "content_block_start"
            && d["content_block"]["type"] == "tool_use"
            && d["content_block"]["name"] == long_name));
        let md = events
            .iter()
            .find(|(n, _)| n == "message_delta")
            .map(|(_, d)| d)
            .unwrap();
        assert_eq!(md["usage"]["input_tokens"], 20);
        assert_eq!(md["usage"]["cache_read_input_tokens"], 80);
        assert_eq!(md["usage"]["output_tokens"], 25);
    }
}
