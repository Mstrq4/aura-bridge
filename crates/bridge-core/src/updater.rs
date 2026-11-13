use reqwest::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub const DEFAULT_GITHUB_REPO: &str = "Mstrq4/aura-bridge";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
    pub asset_url: String,
    pub release_notes: String,
    pub published_at: String,
    pub platform_key: String,
    pub repo: String,
}

pub fn current_platform_key() -> String {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        std::env::consts::ARCH
    };
    format!("{os}-{arch}")
}

fn parse_version_lenient(raw: &str) -> Option<Version> {
    let cleaned = raw.trim().trim_start_matches(['v', 'V']);
    Version::parse(cleaned).ok()
}

pub async fn check_github_updates(current_version: &str, repo: Option<&str>) -> UpdateCheckResult {
    let repo_slug = repo
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_GITHUB_REPO);
    let platform_key = current_platform_key();
    let release_page = format!("https://github.com/{repo_slug}/releases/latest");

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(format!("aurabridge/{current_version}"))
        .build()
        .ok();

    let Some(http) = client else {
        return UpdateCheckResult {
            current_version: current_version.to_string(),
            latest_version: current_version.to_string(),
            update_available: false,
            release_url: release_page,
            asset_url: String::new(),
            release_notes: String::new(),
            published_at: String::new(),
            platform_key,
            repo: repo_slug.to_string(),
        };
    };

    // 1) Try Tauri updater latest.json first
    let manifest_url = format!("https://github.com/{repo_slug}/releases/latest/download/latest.json");
    if let Ok(resp) = http.get(&manifest_url).send().await {
        if resp.status().is_success() {
            if let Ok(manifest) = resp.json::<Value>().await {
                if let Some(ver) = manifest.get("version").and_then(Value::as_str) {
                    let is_newer = match (
                        parse_version_lenient(current_version),
                        parse_version_lenient(ver),
                    ) {
                        (Some(cur), Some(lat)) => lat > cur,
                        _ => ver.trim_start_matches('v') != current_version.trim_start_matches('v'),
                    };
                    let asset_url = manifest
                        .get("platforms")
                        .and_then(|p| p.get(&platform_key))
                        .and_then(|entry| entry.get("url"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    return UpdateCheckResult {
                        current_version: current_version.to_string(),
                        latest_version: ver.trim_start_matches('v').to_string(),
                        update_available: is_newer,
                        release_url: release_page,
                        asset_url,
                        release_notes: manifest
                            .get("notes")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        published_at: manifest
                            .get("pub_date")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        platform_key,
                        repo: repo_slug.to_string(),
                    };
                }
            }
        }
    }

    // 2) Fallback: GitHub Releases API
    let api_url = format!("https://api.github.com/repos/{repo_slug}/releases/latest");
    if let Ok(resp) = http.get(&api_url).send().await {
        if resp.status().is_success() {
            if let Ok(rel) = resp.json::<Value>().await {
                let tag = rel
                    .get("tag_name")
                    .and_then(Value::as_str)
                    .unwrap_or(current_version);
                let is_newer = match (
                    parse_version_lenient(current_version),
                    parse_version_lenient(tag),
                ) {
                    (Some(cur), Some(lat)) => lat > cur,
                    _ => false,
                };
                let html_url = rel
                    .get("html_url")
                    .and_then(Value::as_str)
                    .unwrap_or(&release_page)
                    .to_string();
                let asset_url = rel
                    .get("assets")
                    .and_then(Value::as_array)
                    .and_then(|arr| {
                        arr.iter().find_map(|a| {
                            let name = a.get("name").and_then(Value::as_str)?.to_ascii_lowercase();
                            let url = a.get("browser_download_url").and_then(Value::as_str)?;
                            if cfg!(target_os = "windows")
                                && name.ends_with(".exe")
                                && ((cfg!(target_arch = "aarch64") && name.contains("arm64"))
                                    || (cfg!(target_arch = "x86_64")
                                        && (name.contains("x64") || name.contains("x86_64")))
                                    || (cfg!(target_arch = "x86")
                                        && (name.contains("x86") || name.contains("i686"))
                                        && !name.contains("x86_64")))
                            {
                                Some(url.to_string())
                            } else if cfg!(target_os = "macos")
                                && name.ends_with(".dmg")
                                && (name.contains("universal")
                                    || (cfg!(target_arch = "aarch64") && name.contains("aarch64"))
                                    || (cfg!(target_arch = "x86_64") && name.contains("x64")))
                            {
                                Some(url.to_string())
                            } else if cfg!(target_os = "linux")
                                && (name.ends_with(".appimage") || name.ends_with(".deb"))
                                && ((cfg!(target_arch = "aarch64")
                                    && (name.contains("arm64") || name.contains("aarch64")))
                                    || (cfg!(target_arch = "x86_64")
                                        && (name.contains("amd64") || name.contains("x86_64") || name.contains("x64"))))
                            {
                                Some(url.to_string())
                            } else {
                                None
                            }
                        })
                    })
                    .unwrap_or_default();
                return UpdateCheckResult {
                    current_version: current_version.to_string(),
                    latest_version: tag.trim_start_matches(['v', 'V']).to_string(),
                    update_available: is_newer,
                    release_url: html_url,
                    asset_url,
                    release_notes: rel
                        .get("body")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    published_at: rel
                        .get("published_at")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    platform_key,
                    repo: repo_slug.to_string(),
                };
            }
        }
    }

    UpdateCheckResult {
        current_version: current_version.to_string(),
        latest_version: current_version.to_string(),
        update_available: false,
        release_url: release_page,
        asset_url: String::new(),
        release_notes: String::new(),
        published_at: String::new(),
        platform_key,
        repo: repo_slug.to_string(),
    }
}

