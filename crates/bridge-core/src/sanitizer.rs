use serde_json::{json, Map, Value};
use sha1::{Digest, Sha1};
use std::collections::HashMap;

const DISALLOWED_SCHEMA_KEYS: &[&str] = &[
    "$schema",
    "$id",
    "defer_loading",
    "cache_control",
    "eager_input_streaming",
];

pub fn sanitize_json_schema(schema: &Value) -> Value {
    match schema {
        Value::Null => json!({ "type": "object", "properties": {} }),
        Value::Array(items) => Value::Array(items.iter().map(sanitize_json_schema).collect()),
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                if DISALLOWED_SCHEMA_KEYS.contains(&k.as_str()) {
                    continue;
                }
                if k == "properties" {
                    if let Value::Object(props) = v {
                        let mut clean_props = Map::new();
                        for (pn, pv) in props {
                            clean_props.insert(pn.clone(), sanitize_json_schema(pv));
                        }
                        out.insert("properties".to_string(), Value::Object(clean_props));
                        continue;
                    }
                }
                if v.is_object() || v.is_array() {
                    out.insert(k.clone(), sanitize_json_schema(v));
                } else {
                    out.insert(k.clone(), v.clone());
                }
            }
            if out.get("type").and_then(Value::as_str) == Some("object")
                && !out.contains_key("properties")
            {
                out.insert("properties".to_string(), Value::Object(Map::new()));
            }
            if !out.contains_key("type") && out.contains_key("properties") {
                out.insert("type".to_string(), Value::String("object".to_string()));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

#[derive(Debug, Default, Clone)]
pub struct ToolNameCodec {
    original_to_wire: HashMap<String, String>,
    wire_to_original: HashMap<String, String>,
}

impl ToolNameCodec {
    pub fn to_wire_name(&mut self, original: &str) -> String {
        if let Some(existing) = self.original_to_wire.get(original) {
            return existing.clone();
        }
        let base = if original.is_empty() { "tool" } else { original };
        let clean: String = base
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        if clean.len() <= 64 && !self.wire_to_original.contains_key(&clean) {
            self.original_to_wire
                .insert(original.to_string(), clean.clone());
            self.wire_to_original.insert(clean.clone(), original.to_string());
            return clean;
        }

        let mut hasher = Sha1::new();
        hasher.update(original.as_bytes());
        let suffix = &hex::encode(hasher.finalize())[..10];
        let prefix: String = clean.chars().take(52).collect();
        let short_name = format!("{prefix}_{suffix}");
        self.original_to_wire
            .insert(original.to_string(), short_name.clone());
        self.wire_to_original
            .insert(short_name.clone(), original.to_string());
        short_name
    }

    pub fn to_original_name<'a>(&'a self, wire_name: &'a str) -> &'a str {
        self.wire_to_original
            .get(wire_name)
            .map(String::as_str)
            .unwrap_or(wire_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_230_tools_and_long_names() {
        let mut codec = ToolNameCodec::default();
        let long_name = format!("mcp__ultra_server__{}", "tool_segment_".repeat(8));
        let wire = codec.to_wire_name(&long_name);
        assert!(wire.len() <= 64);
        assert_eq!(codec.to_original_name(&wire), long_name);

        let schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "defer_loading": true,
            "properties": {
                "q": { "type": "string", "$id": "bad" }
            }
        });
        let cleaned = sanitize_json_schema(&schema);
        assert!(cleaned.get("$schema").is_none());
        assert!(cleaned.get("defer_loading").is_none());
        assert!(cleaned["properties"]["q"].get("$id").is_none());
    }
}

