//! ai-lyrics-default-model: detects the default local LLM that
//! SmartBridge's plain "Llama" lyrics backend points at.
//!
//! Separate from the AI Lyrics Optimized triplet (tinyllama / mistral /
//! qwen2.5) because this model serves a different role: it is the
//! single-model fallback used when the user picks "Llama" without
//! enabling Optimized routing.
//!
//! Detection strategy mirrors ai_lyrics.rs:
//!   1. ollama binary on PATH
//!   2. ollama service answering GET /api/tags
//!   3. the returned model tags contain DEFAULT_LOCAL_MODEL
//!
//! Status mapping:
//!   * ollama not installed                 → NotInstalled
//!   * installed but service down           → NeedsRepair (start ollama)
//!   * service up, default model missing    → NeedsRepair (pull it)
//!   * service up, default model present    → Ready

use super::DetectionResult;
use crate::paths;
use std::time::Duration;

const OLLAMA_DEFAULT_ENDPOINT: &str = "http://localhost:11434";
const OLLAMA_TAGS_PATH: &str = "/api/tags";
pub const DEFAULT_LOCAL_MODEL: &str = "gemma4:e4b";
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

    let lower_names: Vec<String> = models
        .iter()
        .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
        .map(|n| n.to_lowercase())
        .collect();

    let target_lc = DEFAULT_LOCAL_MODEL.to_lowercase();
    let present = lower_names.iter().any(|n| n == &target_lc);

    if present {
        DetectionResult::ready()
            .with_detail(format!("Ollama binary: {}", ollama_path.display()))
            .with_detail(format!("Ollama service up at {OLLAMA_DEFAULT_ENDPOINT}"))
            .with_detail(format!("Default local model ready: {DEFAULT_LOCAL_MODEL}"))
    } else {
        DetectionResult::needs_repair(format!(
            "Ollama is running, but the SmartBridge default Llama model `{DEFAULT_LOCAL_MODEL}` \
             is not pulled. Click Install to pull it."
        ))
        .with_detail(format!("Ollama binary: {}", ollama_path.display()))
        .with_detail(format!("missing model: {DEFAULT_LOCAL_MODEL}"))
    }
}
