//! ai-lyrics: detects whether the local Ollama runtime + the exact three
//! models SmartBridge needs for Optimized lyrics routing are available.
//!
//! Three checks, in order:
//!   1. ollama binary is on PATH
//!   2. ollama service answers GET http://localhost:11434/api/tags
//!   3. the returned model tags contain the required small / medium / large
//!      Optimized model set.
//!
//! Status mapping:
//!   * ollama not installed                  → NotInstalled
//!   * installed but service down            → NeedsRepair (start ollama)
//!   * service up, any model missing         → NeedsRepair (pull missing models)
//!   * service up, all models present        → Ready
//!
//! The HTTP call is bounded by a tight timeout so the dashboard does not
//! hang behind a misconfigured local Ollama. SmartBridge's own LlmClient
//! settings (`lyrics_localEndpoint`, `lyrics_localModel`) are read from
//! the user config in Phase 4 to override the defaults below.

use super::DetectionResult;
use crate::paths;
use std::time::Duration;

const OLLAMA_DEFAULT_ENDPOINT: &str = "http://localhost:11434";
const OLLAMA_TAGS_PATH: &str = "/api/tags";
pub const OPTIMIZED_MODELS: &[&str] = &[
    "tinyllama:latest",
    "mistral:7b-instruct-q4_K_M",
    "qwen2.5:14b-instruct-q4_K_M",
];
const HTTP_TIMEOUT: Duration = Duration::from_millis(1500);

pub async fn detect() -> DetectionResult {
    let ollama_path = paths::resolve_ollama();
    if ollama_path.is_none() {
        return DetectionResult::not_installed()
            .with_detail("Ollama not found in PATH or any known install location.")
            .with_detail("Install from https://ollama.com or via `brew install ollama`.");
    }
    let ollama_path = ollama_path.unwrap();

    let client = match reqwest::Client::builder().timeout(HTTP_TIMEOUT).build() {
        Ok(c) => c,
        Err(e) => return DetectionResult::error(format!("HTTP client init failed: {e}")),
    };

    let url = format!("{OLLAMA_DEFAULT_ENDPOINT}{OLLAMA_TAGS_PATH}");
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => {
            return DetectionResult::needs_repair(
                "Ollama is installed but the local service is not responding. \
                 Start it with: ollama serve",
            )
            .with_detail(format!("tried {url}"));
        }
    };

    if !resp.status().is_success() {
        return DetectionResult::needs_repair(format!(
            "Ollama responded with HTTP {} at {url}",
            resp.status()
        ));
    }

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return DetectionResult::error(format!("could not parse Ollama response: {e}")),
    };

    let models = body
        .get("models")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();

    let model_names: Vec<String> = models
        .iter()
        .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect();

    let lower_names: Vec<String> = model_names
        .iter()
        .map(|n| n.to_lowercase())
        .collect();

    let missing: Vec<&str> = OPTIMIZED_MODELS
            .iter()
        .copied()
        .filter(|required| !lower_names.iter().any(|n| n == &required.to_lowercase()))
            .collect();

    if missing.is_empty() {
        let mut r = DetectionResult::ready();
        r = r.with_detail(format!("Ollama binary: {}", ollama_path.display()));
        r = r.with_detail(format!("Ollama service up at {OLLAMA_DEFAULT_ENDPOINT}"));
        for n in OPTIMIZED_MODELS {
            r = r.with_detail(format!("Optimized model ready: {n}"));
        }
        r
    } else {
        let mut r = DetectionResult::needs_repair(
            "Ollama is running, but Optimized lyrics needs three models. \
             Click Install to pull the missing models.",
        )
        .with_detail(format!("Ollama binary: {}", ollama_path.display()));
        for n in missing {
            r = r.with_detail(format!("missing model: {n}"));
        }
        r
    }
}
