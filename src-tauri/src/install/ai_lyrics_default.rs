//! ai-lyrics-default-model install: pulls the single default local LLM
//! that SmartBridge's plain "Llama" lyrics backend points at.
//!
//! Mirrors `install/ai_lyrics.rs` for the multi-model Optimized triplet,
//! but specialised to a one-model component so the dashboard can manage
//! the Llama default independently of the Optimized routing models.

use super::InstallOutcome;
use crate::commands::ComponentState;
use crate::detection;
use crate::download::DownloadProgress;
use crate::manifest::{Delivery, Manifest};
use crate::paths;
use std::path::PathBuf;
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};

const COMPONENT: &str = "ai-lyrics-default-model";
const PROGRESS_CHANNEL: &str = "download://progress";

pub async fn install(app: &AppHandle, manifest: &Manifest) -> InstallOutcome {
    // Manifest entry is preferred (gives the published list of models,
    // sizes, license URLs). When the published manifest predates this
    // component we fall back to the hardcoded DEFAULT_LOCAL_MODEL so the
    // tile is functional immediately without a manifest republish.
    let mut ollama_tags: Vec<String> = match manifest.component(COMPONENT) {
        Some(component) => component
            .assets
            .iter()
            .filter_map(|asset| match &asset.delivery {
                Delivery::OllamaPull { ollama_tag, .. } => Some(ollama_tag.clone()),
                _ => None,
            })
            .collect(),
        None => Vec::new(),
    };
    if ollama_tags.is_empty() {
        ollama_tags = vec![detection::ai_lyrics_default::DEFAULT_LOCAL_MODEL.to_string()];
    }

    let ollama_path = match paths::resolve_ollama() {
        Some(p) => p,
        None => {
            return InstallOutcome::err(
                COMPONENT,
                vec![
                    "Ollama is not installed on this machine.".into(),
                    "Install it from https://ollama.com or via `brew install ollama`, then click Install again.".into(),
                ],
            );
        }
    };

    if !ollama_service_up().await {
        return InstallOutcome::err(
            COMPONENT,
            vec![
                format!(
                    "Found Ollama at {} but the local service is not responding.",
                    ollama_path.display()
                ),
                "Run `ollama serve` in a terminal, then click Install again.".into(),
            ],
        );
    }

    let mut messages: Vec<String> = vec![
        format!("Using Ollama at {}", ollama_path.display()),
        format!("Pulling {} default Llama model(s)…", ollama_tags.len()),
    ];

    for tag in ollama_tags {
        messages.push(format!("Pulling model `{tag}` via Ollama…"));
        match run_ollama_pull(app, &ollama_path, &tag).await {
            Ok(lines) => messages.extend(lines),
            Err(e) => {
                messages.push(format!("ollama pull failed for `{tag}`: {e}"));
                let det = detection::ai_lyrics_default::detect().await;
                return InstallOutcome::err(COMPONENT, messages).with_post_state(det);
            }
        }
    }

    let det = detection::ai_lyrics_default::detect().await;
    if det.state == ComponentState::Ready {
        InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
    } else {
        messages.push("Default Llama model still missing after install.".into());
        InstallOutcome::err(COMPONENT, messages).with_post_state(det)
    }
}

async fn ollama_service_up() -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1500))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

async fn run_ollama_pull(
    app: &AppHandle,
    ollama_path: &PathBuf,
    tag: &str,
) -> Result<Vec<String>, String> {
    let mut cmd = tokio::process::Command::new(ollama_path);
    cmd.arg("pull").arg(tag);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("could not spawn ollama: {e}"))?;

    let stdout = child.stdout.take().ok_or("no stdout from ollama")?;
    let stderr = child.stderr.take().ok_or("no stderr from ollama")?;

    let app1 = app.clone();
    let app2 = app.clone();
    let tag1 = tag.to_string();
    let tag2 = tag.to_string();

    let out_handle = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut last = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            emit_progress(&app1, &tag1, &line);
            last = line;
        }
        last
    });

    let err_handle = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        let mut last = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            emit_progress(&app2, &tag2, &line);
            last = line;
        }
        last
    });

    let status = child
        .wait()
        .await
        .map_err(|e| format!("ollama wait failed: {e}"))?;

    let _ = out_handle.await;
    let _ = err_handle.await;

    if !status.success() {
        return Err(format!("ollama pull exited with {status}"));
    }

    emit(
        app,
        DownloadProgress {
            download_id: format!("{COMPONENT}.ollama_pull"),
            bytes_downloaded: 100,
            bytes_total: 100,
            phase: "verified",
        },
    );

    Ok(vec![format!("Successfully pulled `{tag}`.")])
}

fn emit_progress(app: &AppHandle, tag: &str, line: &str) {
    let lower = line.to_lowercase();
    let phase = if lower.contains("success") || lower.contains("done") {
        "verified"
    } else if lower.contains("pulling manifest") || lower.contains("starting") {
        "starting"
    } else if lower.contains("verifying") {
        "verified"
    } else {
        "downloading"
    };

    let pct = extract_percentage(line).unwrap_or(0);

    emit(
        app,
        DownloadProgress {
            download_id: format!("{COMPONENT}.ollama_pull"),
            bytes_downloaded: pct as u64,
            bytes_total: 100,
            phase: match phase {
                "starting" => "starting",
                "verified" => "verified",
                _ => "downloading",
            },
        },
    );

    tracing::debug!(target = "ai_lyrics_default.ollama", tag = %tag, line = %line);
}

fn extract_percentage(line: &str) -> Option<u8> {
    let s = line.trim();
    let percent_idx = s.find('%')?;
    let bytes = s.as_bytes();
    let mut start = percent_idx;
    while start > 0 {
        let prev = bytes[start - 1];
        if prev.is_ascii_digit() || prev == b'.' {
            start -= 1;
        } else {
            break;
        }
    }
    let num_str = &s[start..percent_idx];
    let val: f32 = num_str.parse().ok()?;
    Some(val.clamp(0.0, 100.0) as u8)
}

fn emit(app: &AppHandle, progress: DownloadProgress) {
    if let Err(e) = app.emit(PROGRESS_CHANNEL, progress) {
        tracing::warn!(error = %e, "ai_lyrics_default: failed to emit progress");
    }
}

/// Remove the SmartBridge default Llama model from the local Ollama
/// install. Mirrors `install::ai_lyrics::remove` but only touches the
/// single default model — the Optimized triplet remains untouched.
pub async fn remove() -> InstallOutcome {
    let ollama_path = match paths::resolve_ollama() {
        None => {
            return InstallOutcome::ok(
                COMPONENT,
                vec!["Ollama not installed — nothing to remove.".into()],
            )
            .with_post_state(detection::ai_lyrics_default::detect().await);
        }
        Some(p) => p,
    };

    let mut messages: Vec<String> = vec![format!("Using Ollama at {}", ollama_path.display())];
    let mut errors: Vec<String> = Vec::new();
    let tag = detection::ai_lyrics_default::DEFAULT_LOCAL_MODEL;

    match tokio::process::Command::new(&ollama_path)
        .arg("rm")
        .arg(tag)
        .output()
        .await
    {
        Ok(out) if out.status.success() => {
            messages.push(format!("removed Ollama model `{tag}`"));
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.to_lowercase().contains("not found") {
                messages.push(format!("(Ollama model `{tag}` was not present)"));
            } else {
                errors.push(format!("ollama rm {tag} failed: {}", stderr.trim()));
            }
        }
        Err(e) => errors.push(format!("spawn ollama rm: {e}")),
    }

    let det = detection::ai_lyrics_default::detect().await;
    if errors.is_empty() {
        InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
    } else {
        let mut all = messages;
        all.extend(errors);
        InstallOutcome::err(COMPONENT, all).with_post_state(det)
    }
}
