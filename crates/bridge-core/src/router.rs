use crate::config::BridgeConfig;
use sha2::{Digest, Sha256};

pub fn resolve_upstream_model(
    requested_model: Option<&str>,
    config: &BridgeConfig,
    is_background_title_call: bool,
) -> String {
    let raw = requested_model.unwrap_or("").trim();
    let lower = raw.to_ascii_lowercase();

    if lower.contains("haiku") {
        return config.models.haiku.clone();
    }
    if lower.contains("sonnet") || lower.contains("fable") {
        return config.models.sonnet.clone();
    }
    if lower.contains("opus") {
        return config.models.opus.clone();
    }

    if !raw.is_empty() && !lower.starts_with("claude-") {
        if is_background_title_call && raw == config.models.opus && !config.models.haiku.is_empty()
        {
            return config.models.haiku.clone();
        }
        return raw.to_string();
    }

    config.models.default.clone()
}

pub fn derive_prompt_cache_key(
    session_id: Option<&str>,
    system_prompt: &str,
    upstream_model: &str,
) -> String {
    let seed = if let Some(sid) = session_id.map(str::trim).filter(|s| !s.is_empty()) {
        format!("session:{upstream_model}:{sid}")
    } else {
        let snippet: String = system_prompt.chars().take(512).collect();
        format!("sys:{upstream_model}:{snippet}")
    };
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let digest = hex::encode(hasher.finalize());
    format!("crb-{}", &digest[..24])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_aliases_and_explicit_models() {
        let cfg = BridgeConfig::default();
        assert_eq!(
            resolve_upstream_model(Some("claude-opus-4-6"), &cfg, false),
            "gpt-6.1-sol"
        );
        assert_eq!(
            resolve_upstream_model(Some("claude-sonnet-4-6"), &cfg, false),
            "gpt-6-astra"
        );
        assert_eq!(
            resolve_upstream_model(Some("claude-haiku-4-5-20251001"), &cfg, false),
            "gpt-5.6-sol"
        );
        assert_eq!(
            resolve_upstream_model(Some("gpt-5.6-sol"), &cfg, false),
            "gpt-5.6-sol"
        );
        assert_eq!(
            resolve_upstream_model(Some("gpt-6.1-sol"), &cfg, true),
            "gpt-5.6-sol"
        );
    }
}

