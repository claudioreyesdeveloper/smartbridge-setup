//! cubase-connection install: places the MIDI Remote driver script in
//! the version-agnostic Driver Scripts folder, and the project template
//! in every detected Cubase 12-15 prefs folder.
//!
//! If no Cubase prefs folder is detected, we still install the MIDI
//! Remote script (it's harmless until Cubase is installed) and queue the
//! template into the user's Documents folder as a manual-install bundle
//! the user can drag in later.

use super::InstallOutcome;
use crate::detection;
use crate::download::fetch_with_verify;
use crate::manifest::Manifest;
use std::path::PathBuf;
use tauri::AppHandle;

const COMPONENT: &str = "cubase-connection";
const SCRIPT_ASSET_ID: &str = "cubase-connection.midi-remote-script";
const TEMPLATE_ASSET_ID: &str = "cubase-connection.project-template";
const SUPPORTED_CUBASE_VERSIONS: &[u32] = &[12, 13, 14, 15];

pub async fn install(app: &AppHandle, manifest: &Manifest) -> InstallOutcome {
    let component = match manifest.component(COMPONENT) {
        Some(c) => c,
        None => return InstallOutcome::err(COMPONENT, vec![format!("manifest missing component {COMPONENT}")]),
    };

    let mut messages: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    // 1. MIDI Remote script (cross-version)
    if let Some(asset) = component.asset(SCRIPT_ASSET_ID) {
        if let Some(spec) = super::download_spec_for(asset) {
            match fetch_with_verify(app, &spec).await {
                Ok(outcome) => {
                    let target = midi_remote_script_path();
                    if let Some(parent) = target.parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            errors.push(format!("create {}: {e}", parent.display()));
                        }
                    }
                    match std::fs::copy(&outcome.local_path, &target) {
                        Ok(_) => messages.push(format!("placed MIDI Remote script: {}", target.display())),
                        Err(e) => errors.push(format!("copy MIDI script → {}: {e}", target.display())),
                    }
                }
                Err(e) => errors.push(format!("MIDI script download: {e}")),
            }
        }
    } else {
        errors.push(format!("manifest missing asset {SCRIPT_ASSET_ID}"));
    }

    // 2. Project template (per detected Cubase version)
    if let Some(asset) = component.asset(TEMPLATE_ASSET_ID) {
        if let Some(spec) = super::download_spec_for(asset) {
            match fetch_with_verify(app, &spec).await {
                Ok(outcome) => {
                    let mut placed_any = false;
                    for &ver in SUPPORTED_CUBASE_VERSIONS {
                        let prefs_dir = cubase_prefs_dir(ver);
                        if !prefs_dir.exists() {
                            continue;
                        }
                        let template_dir = prefs_dir.join("Project Templates");
                        if let Err(e) = std::fs::create_dir_all(&template_dir) {
                            errors.push(format!("create {}: {e}", template_dir.display()));
                            continue;
                        }
                        let target = template_dir.join("SmartBridge.cpr");
                        match std::fs::copy(&outcome.local_path, &target) {
                            Ok(_) => {
                                messages.push(format!("placed Cubase {ver} template: {}", target.display()));
                                placed_any = true;
                            }
                            Err(e) => errors.push(format!("copy template → {}: {e}", target.display())),
                        }
                    }
                    if !placed_any {
                        if let Some(docs) = dirs::document_dir() {
                            let manual_dir = docs.join("SmartBridge").join("Cubase Manual Install");
                            if let Err(e) = std::fs::create_dir_all(&manual_dir) {
                                errors.push(format!("create {}: {e}", manual_dir.display()));
                            } else {
                                let target = manual_dir.join("SmartBridge.cpr");
                                match std::fs::copy(&outcome.local_path, &target) {
                                    Ok(_) => messages.push(format!(
                                        "no Cubase 12-15 prefs detected — placed template at {} for manual install",
                                        target.display()
                                    )),
                                    Err(e) => errors.push(format!("copy manual template: {e}")),
                                }
                            }
                        }
                    }
                }
                Err(e) => errors.push(format!("template download: {e}")),
            }
        }
    } else {
        errors.push(format!("manifest missing asset {TEMPLATE_ASSET_ID}"));
    }

    let det = detection::cubase::detect().await;

    if errors.is_empty() {
        InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
    } else {
        let mut all = messages;
        all.extend(errors);
        InstallOutcome::err(COMPONENT, all).with_post_state(det)
    }
}

fn midi_remote_script_path() -> PathBuf {
    let docs = dirs::document_dir().unwrap_or_else(|| PathBuf::from("."));
    docs.join("Steinberg")
        .join("Cubase")
        .join("MIDI Remote")
        .join("Driver Scripts")
        .join("Local")
        .join("SmartBridge")
        .join("GenosSlotRename")
        .join("SmartBridge_GenosSlotRename.js")
}

/// Delete the MIDI Remote driver script and the per-version project
/// templates that the install action placed. Templates only get deleted
/// in the Cubase 12-15 prefs locations Setup wrote them into; templates
/// in unrelated Cubase prefs that the customer dragged there manually
/// are not touched. The Documents\SmartBridge\Cubase Manual Install
/// fallback bundle is also removed.
pub async fn remove() -> InstallOutcome {
    let mut messages: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    let script = midi_remote_script_path();
    if script.exists() {
        match std::fs::remove_file(&script) {
            Ok(()) => messages.push(format!("Removed {}", script.display())),
            Err(e) => errors.push(format!("remove {}: {e}", script.display())),
        }
    }
    // Tidy empty per-script subfolders we created (best-effort).
    if let Some(parent) = script.parent() {
        let _ = std::fs::remove_dir(parent);
        if let Some(grand) = parent.parent() {
            let _ = std::fs::remove_dir(grand);
        }
    }

    for &ver in SUPPORTED_CUBASE_VERSIONS {
        let template = cubase_prefs_dir(ver)
            .join("Project Templates")
            .join("SmartBridge.cpr");
        if template.exists() {
            match std::fs::remove_file(&template) {
                Ok(()) => messages.push(format!("Removed {}", template.display())),
                Err(e) => errors.push(format!("remove {}: {e}", template.display())),
            }
        }
    }

    if let Some(docs) = dirs::document_dir() {
        let manual_dir = docs.join("SmartBridge").join("Cubase Manual Install");
        if manual_dir.exists() {
            match std::fs::remove_dir_all(&manual_dir) {
                Ok(()) => messages.push(format!("Removed {}", manual_dir.display())),
                Err(e) => errors.push(format!("remove {}: {e}", manual_dir.display())),
            }
        }
    }

    let det = detection::cubase::detect().await;
    if errors.is_empty() {
        if messages.is_empty() {
            messages.push("No Cubase connection files to remove.".into());
        }
        InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
    } else {
        let mut all = messages;
        all.extend(errors);
        InstallOutcome::err(COMPONENT, all).with_post_state(det)
    }
}

fn cubase_prefs_dir(version: u32) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .map(|h| {
                h.join("Library")
                    .join("Preferences")
                    .join(format!("Cubase {version}"))
            })
            .unwrap_or_default()
    }

    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .map(|c| c.join("Steinberg").join(format!("Cubase {version}_64")))
            .unwrap_or_default()
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = version;
        PathBuf::new()
    }
}
