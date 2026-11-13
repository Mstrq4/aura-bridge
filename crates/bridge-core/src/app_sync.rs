use crate::config::{user_home_dir, BridgeConfig};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const CLAUDE_CAPABILITIES: &str =
    "effort,max_effort,xhigh_effort,thinking,adaptive_thinking,interleaved_thinking,mid_conversation_system";
const CLAUDE_GLOBAL_CAPABILITIES: &str =
    "effort,max_effort,xhigh_effort,thinking,adaptive_thinking,interleaved_thinking,mid_conv_system,mid_conv_tool_change,mm_images,mm_pdf";
const GATEWAY_ID: &str = "00000000-0000-4000-8000-000000187870";

pub fn claude_desktop_config_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| user_home_dir().join("AppData").join("Local"))
            .join("Claude-3p")
            .join("configLibrary")
    }
    #[cfg(target_os = "macos")]
    {
        user_home_dir()
            .join("Library")
            .join("Application Support")
            .join("Claude-3p")
            .join("configLibrary")
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| user_home_dir().join(".config"))
            .join("Claude-3p")
            .join("configLibrary")
    }
}

pub fn claude_code_settings_path() -> PathBuf {
    user_home_dir().join(".claude").join("settings.json")
}

pub fn deepseek_harness_patch_path() -> PathBuf {
    user_home_dir()
        .join(".dsh")
        .join("profiles")
        .join("desktop")
        .join("cordis.patch.yml")
}

pub fn get_app_status(config: &BridgeConfig) -> Value {
    let claude_3p_dir = claude_desktop_config_dir();
    let desktop_installed = claude_3p_dir.exists();
    let mut desktop_applied = false;
    let meta_path = claude_3p_dir.join("_meta.json");
    if let Ok(raw) = fs::read_to_string(&meta_path) {
        if let Ok(meta) = serde_json::from_str::<Value>(&raw) {
            desktop_applied = meta.get("appliedId").and_then(Value::as_str) == Some(GATEWAY_ID);
        }
    }

    let cli_path = claude_code_settings_path();
    let cli_installed = cli_path.exists();
    let mut cli_applied = false;
    let mut cli_capabilities_ready = false;
    if let Ok(raw) = fs::read_to_string(&cli_path) {
        if let Ok(s) = serde_json::from_str::<Value>(&raw) {
            let port_str = config.port.to_string();
            cli_applied = s
                .get("env")
                .and_then(|e| e.get("ANTHROPIC_BASE_URL"))
                .and_then(Value::as_str)
                .is_some_and(|u| u.contains(&port_str));
            cli_capabilities_ready = s
                .get("env")
                .and_then(|e| e.get("ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES"))
                .is_some()
                && s.get("env")
                    .and_then(|e| e.get("CLAUDE_CODE_MODEL_CAPABILITIES"))
                    .is_some();
        }
    }

    let dsh_path = deepseek_harness_patch_path();
    let dsh_installed = dsh_path.exists();
    let mut dsh_applied = false;
    let mut dsh_multimodal_ready = false;
    if let Ok(text) = fs::read_to_string(&dsh_path) {
        dsh_applied =
            text.contains(&format!("127.0.0.1:{}", config.port)) && text.contains("openai-responses");
        dsh_multimodal_ready = text.contains("- image") && text.contains("reasoningEfforts:");
    }

    json!({
        "claudeDesktop": { "installed": desktop_installed, "applied": desktop_applied },
        "claudeCode": {
            "installed": cli_installed,
            "applied": cli_applied,
            "capabilitiesReady": cli_capabilities_ready
        },
        "deepseekHarness": {
            "installed": dsh_installed,
            "applied": dsh_applied,
            "multimodalReady": dsh_multimodal_ready
        },
        "bridgeUrl": format!("http://{}:{}", config.host, config.port),
        "models": config.models,
    })
}

pub fn apply_claude_desktop(config: &BridgeConfig) -> Result<String, (u16, String)> {
    let dir = claude_desktop_config_dir();
    if !dir.exists() {
        return Err((404, "Claude-3p configLibrary not found".to_string()));
    }
    let gateway_path = dir.join(format!("{GATEWAY_ID}.json"));
    let api_key = if config.upstream_api_key.is_empty() {
        "sk-bridge-local"
    } else {
        config.upstream_api_key.as_str()
    };
    let payload = json!({
        "inferenceGatewayBaseUrl": format!("http://{}:{}", config.host, config.port),
        "inferenceGatewayManagedClaudeCode": true,
        "inferenceGatewayApiKey": api_key,
        "inferenceModels": [
            { "name": "claude-fable-5", "labelOverride": config.models.sonnet },
            { "name": "claude-haiku-4-5", "labelOverride": config.models.haiku },
            { "name": "claude-opus-5", "labelOverride": config.models.opus },
            { "name": "claude-sonnet-5", "labelOverride": config.models.sonnet },
        ],
        "coworkEgressAllowedHosts": ["*"],
        "disableDeploymentModeChooser": true,
        "inferenceProvider": "gateway",
        "inferenceCredentialKind": "static",
        "inferenceGatewayAuthScheme": "bearer"
    });
    fs::write(
        &gateway_path,
        serde_json::to_string_pretty(&payload).map_err(|e| (500, e.to_string()))?,
    )
    .map_err(|e| (500, e.to_string()))?;

    let meta_path = dir.join("_meta.json");
    let mut meta = fs::read_to_string(&meta_path)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or_else(|| json!({ "appliedId": GATEWAY_ID, "entries": [] }));
    meta["appliedId"] = json!(GATEWAY_ID);
    if !meta.get("entries").is_some_and(Value::is_array) {
        meta["entries"] = json!([]);
    }
    let has_entry = meta["entries"]
        .as_array()
        .is_some_and(|arr| arr.iter().any(|e| e.get("id").and_then(Value::as_str) == Some(GATEWAY_ID)));
    if !has_entry {
        if let Some(arr) = meta["entries"].as_array_mut() {
            arr.push(json!({ "id": GATEWAY_ID, "name": "Claude Switch Pro Bridge" }));
        }
    }
    fs::write(
        &meta_path,
        serde_json::to_string_pretty(&meta).map_err(|e| (500, e.to_string()))?,
    )
    .map_err(|e| (500, e.to_string()))?;

    Ok("Applied directly to Claude Desktop".to_string())
}

pub fn apply_claude_code(config: &BridgeConfig) -> Result<String, (u16, String)> {
    let cli_path = claude_code_settings_path();
    if let Some(parent) = cli_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut s = fs::read_to_string(&cli_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));

    if !s.get("env").is_some_and(Value::is_object) {
        s["env"] = json!({});
    }

    let effort = if config.default_reasoning_effort == "none" || config.default_reasoning_effort.is_empty() {
        "high"
    } else {
        config.default_reasoning_effort.as_str()
    };

    if let Some(env) = s.get_mut("env").and_then(Value::as_object_mut) {
        env.insert(
            "ANTHROPIC_BASE_URL".to_string(),
            json!(format!("http://{}:{}", config.host, config.port)),
        );
        if !config.upstream_api_key.is_empty()
            && config.upstream_api_key != "replace-with-your-api-key"
        {
            env.insert(
                "ANTHROPIC_AUTH_TOKEN".to_string(),
                json!(config.upstream_api_key),
            );
        }
        env.insert("ANTHROPIC_MODEL".to_string(), json!(config.models.opus));
        env.insert(
            "ANTHROPIC_DEFAULT_OPUS_MODEL".to_string(),
            json!(config.models.opus),
        );
        env.insert(
            "ANTHROPIC_DEFAULT_SONNET_MODEL".to_string(),
            json!(config.models.sonnet),
        );
        env.insert(
            "ANTHROPIC_DEFAULT_HAIKU_MODEL".to_string(),
            json!(config.models.haiku),
        );
        env.insert(
            "CLAUDE_CODE_SUBAGENT_MODEL".to_string(),
            json!(config.models.haiku),
        );
        env.insert(
            "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES".to_string(),
            json!(CLAUDE_CAPABILITIES),
        );
        env.insert(
            "ANTHROPIC_DEFAULT_SONNET_MODEL_SUPPORTED_CAPABILITIES".to_string(),
            json!(CLAUDE_CAPABILITIES),
        );
        env.insert(
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_SUPPORTED_CAPABILITIES".to_string(),
            json!(CLAUDE_CAPABILITIES),
        );
        env.insert(
            "CLAUDE_CODE_MODEL_CAPABILITIES".to_string(),
            json!(CLAUDE_GLOBAL_CAPABILITIES),
        );
        env.insert("ENABLE_TOOL_SEARCH".to_string(), json!("false"));
    }

    s["model"] = json!("opus[1m]");
    s["alwaysThinkingEnabled"] = json!(true);
    s["showThinkingSummaries"] = json!(true);
    s["effortLevel"] = json!(if effort == "xhigh" { "high" } else { effort });

    let mut ms = s
        .get("modelSettings")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    ms.insert(config.models.opus.clone(), json!({ "effortLevel": effort }));
    ms.insert(
        config.models.sonnet.clone(),
        json!({ "effortLevel": effort }),
    );
    ms.insert(
        config.models.haiku.clone(),
        json!({ "effortLevel": "medium" }),
    );
    s["modelSettings"] = Value::Object(ms);

    fs::write(
        &cli_path,
        serde_json::to_string_pretty(&s).map_err(|e| (500, e.to_string()))?,
    )
    .map_err(|e| (500, e.to_string()))?;

    Ok("Applied full capabilities & settings to Claude Code CLI".to_string())
}

fn build_dsh_provider_block(config: &BridgeConfig) -> Vec<String> {
    let effort = if config.default_reasoning_effort == "none" {
        "off"
    } else if config.default_reasoning_effort.is_empty() {
        "high"
    } else {
        config.default_reasoning_effort.as_str()
    };

    let mut models = Vec::new();
    for m in [
        &config.models.opus,
        &config.models.sonnet,
        &config.models.haiku,
    ] {
        if !m.is_empty() && !models.contains(m) {
            models.push(m.clone());
        }
    }

    let mut lines = vec![
        "- id: llm-pi-ai".to_string(),
        "  name: \"@deepseek-ai/dsh-llm-pi-ai\"".to_string(),
        "  config:".to_string(),
        "    providers:".to_string(),
        "      agentraoter:".to_string(),
        "        apiKeyEnv: AGENTRAOTER_API_KEY".to_string(),
        "        api: openai-responses".to_string(),
        format!("        baseURL: http://{}:{}/v1", config.host, config.port),
        "        defaultContextWindow: 400000".to_string(),
        "        defaultMaxTokens: 64000".to_string(),
        "        defaultInput:".to_string(),
        "          - text".to_string(),
        "          - image".to_string(),
        format!("        reasoning: {effort}"),
        "        requestImagePixelBudget: 4194304".to_string(),
        "        requestImageMaxBytes: 1048576".to_string(),
        "        maxRequestImageBytes: 20971520".to_string(),
        "        maxRequestImages: 50".to_string(),
        "        compat:".to_string(),
        "          supportsDeveloperRole: true".to_string(),
        "          supportsMaxOutputTokens: true".to_string(),
        "          supportsStrictMode: false".to_string(),
        "        retryPolicy:".to_string(),
        "          mode: normal".to_string(),
        "          maxRetries: 3".to_string(),
        "        models:".to_string(),
    ];

    for id in models {
        lines.push(format!("          - id: {id}"));
        lines.push(format!("            name: {id}"));
        lines.push("            contextWindow: 400000".to_string());
        lines.push("            maxTokens: 64000".to_string());
        lines.push("            input:".to_string());
        lines.push("              - text".to_string());
        lines.push("              - image".to_string());
        lines.push("            reasoningEfforts:".to_string());
        lines.push("              off:".to_string());
        lines.push("              low: low".to_string());
        lines.push("              medium: medium".to_string());
        lines.push("              high: high".to_string());
        lines.push("              xhigh: xhigh".to_string());
    }
    lines
}

pub fn apply_dsh_patch_file(patch_path: &Path, config: &BridgeConfig) -> std::io::Result<()> {
    let raw = fs::read_to_string(patch_path)?;
    let block_lines = build_dsh_provider_block(config);
    let mut out = Vec::new();
    let mut skipping = false;
    let mut replaced = false;

    for line in raw.lines() {
        let trimmed = line.trim_start();
        if line.starts_with("- id:") {
            if skipping {
                skipping = false;
            }
            if trimmed.starts_with("- id: llm-pi-ai") {
                skipping = true;
                replaced = true;
                out.extend(block_lines.clone());
                continue;
            }
        }
        if !skipping {
            out.push(line.to_string());
        }
    }
    if !replaced {
        out.extend(block_lines);
    }

    let mut final_lines = Vec::with_capacity(out.len());
    let mut in_default_model = false;
    for mut line in out {
        if line.starts_with("- id:") {
            in_default_model = line.starts_with("- id: agent-default-model");
        }
        if in_default_model && line.trim_start().starts_with("model:") {
            line = format!("    model: {}", config.models.opus);
        }
        final_lines.push(line);
    }

    let mut content = final_lines.join("\n");
    while content.ends_with('\n') {
        content.pop();
    }
    content.push('\n');
    fs::write(patch_path, content)
}

pub fn apply_deepseek_harness(config: &BridgeConfig) -> Result<String, (u16, String)> {
    let patch_path = deepseek_harness_patch_path();
    if !patch_path.exists() {
        return Err((404, "DeepSeek Harness cordis.patch.yml not found".to_string()));
    }
    apply_dsh_patch_file(&patch_path, config).map_err(|e| (500, e.to_string()))?;
    Ok("Applied multimodal vision, reasoningEfforts, and Responses API to DeepSeek Harness".to_string())
}

pub fn apply_all_detected(config: &BridgeConfig) -> Value {
    let cc = apply_claude_code(config).is_ok();
    let dsh = if deepseek_harness_patch_path().exists() {
        apply_deepseek_harness(config).is_ok()
    } else {
        false
    };
    let desktop = if claude_desktop_config_dir().exists() {
        apply_claude_desktop(config).is_ok()
    } else {
        false
    };
    json!({
        "claudeCode": cc,
        "deepseekHarness": dsh,
        "claudeDesktop": desktop
    })
}

