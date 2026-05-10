//! synthv-connection install: places the side-panel lua script in the
//! scripts folder of every detected SynthV Studio install (1 and 2).
//!
//! If SynthV is not installed, we skip this component cleanly. Re-running
//! Setup later will install or refresh the SmartBridge script once SynthV is
//! present.

use super::InstallOutcome;
use crate::detection;
use crate::download::fetch_with_verify;
use crate::manifest::Manifest;
use std::path::PathBuf;
use tauri::AppHandle;

const COMPONENT: &str = "synthv-connection";
const ASSET_ID: &str = "synthv-connection.sidepanel-script";
const SCRIPT_FILE: &str = "synthv_smartbridge_sidepanel.lua";

pub async fn install(app: &AppHandle, manifest: &Manifest) -> InstallOutcome {
    let component = match manifest.component(COMPONENT) {
        Some(c) => c,
        None => return InstallOutcome::err(COMPONENT, vec![format!("manifest missing component {COMPONENT}")]),
    };
    let mut targets: Vec<PathBuf> = Vec::new();
    let (s1, s2) = (studio_data_dir("Synthesizer V Studio"), studio_data_dir("Synthesizer V Studio 2"));

    if let Some(p) = s1.filter(|_| studio_app_present("Synthesizer V Studio")) {
        targets.push(p.join("scripts").join(SCRIPT_FILE));
    }
    if let Some(p) = s2.filter(|_| studio_app_present("Synthesizer V Studio 2")) {
        targets.push(p.join("scripts").join(SCRIPT_FILE));
    }

    let mut messages: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    remove_legacy_appdata_scripts(&mut messages, &mut errors);

    if targets.is_empty() {
        let det = detection::synthv::detect().await;
        messages.push("Synthesizer V Studio was not found. Skipped the SmartBridge Synthesizer V script.".into());
        return if errors.is_empty() {
            InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
        } else {
            let mut all = messages;
            all.extend(errors);
            InstallOutcome::err(COMPONENT, all).with_post_state(det)
        };
    }

    let asset = match component.asset(ASSET_ID) {
        Some(a) => a,
        None => return InstallOutcome::err(COMPONENT, vec![format!("manifest missing asset {ASSET_ID}")]),
    };

    let spec = match super::download_spec_for(asset) {
        Some(s) => s,
        None => return InstallOutcome::err(COMPONENT, vec!["asset not downloadable".into()]),
    };
    let outcome = match fetch_with_verify(app, &spec).await {
        Ok(o) => o,
        Err(e) => return InstallOutcome::err(COMPONENT, vec![format!("download failed: {e}")]),
    };

    for target in targets {
        if let Some(parent) = target.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                errors.push(format!("create {}: {e}", parent.display()));
                continue;
            }
        }
        match std::fs::copy(&outcome.local_path, &target) {
            Ok(_) => messages.push(format!("placed {}", target.display())),
            Err(e) => errors.push(format!("copy → {}: {e}", target.display())),
        }
    }

    let det = detection::synthv::detect().await;

    if errors.is_empty() {
        InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
    } else {
        let mut all = messages;
        all.extend(errors);
        InstallOutcome::err(COMPONENT, all).with_post_state(det)
    }
}

/// Delete the side-panel script from every Synthesizer V Studio scripts
/// folder we know about. Idempotent — paths that don't exist are
/// silently skipped.
pub async fn remove() -> InstallOutcome {
    let mut messages: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for studio_name in ["Synthesizer V Studio", "Synthesizer V Studio 2"] {
        if let Some(base) = studio_data_dir(studio_name) {
            remove_script_at(base.join("scripts").join(SCRIPT_FILE), &mut messages, &mut errors);
        }
    }
    remove_legacy_appdata_scripts(&mut messages, &mut errors);

    let det = detection::synthv::detect().await;
    if errors.is_empty() {
        if messages.is_empty() {
            messages.push("No SynthV side-panel scripts to remove.".into());
        }
        InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
    } else {
        let mut all = messages;
        all.extend(errors);
        InstallOutcome::err(COMPONENT, all).with_post_state(det)
    }
}

fn remove_legacy_appdata_scripts(messages: &mut Vec<String>, errors: &mut Vec<String>) {
    for studio_name in ["Synthesizer V Studio", "Synthesizer V Studio 2"] {
        if let Some(base) = legacy_appdata_dir(studio_name) {
            remove_script_at(base.join("scripts").join(SCRIPT_FILE), messages, errors);
        }
    }
}

fn remove_script_at(target: PathBuf, messages: &mut Vec<String>, errors: &mut Vec<String>) {
    if target.exists() {
        match std::fs::remove_file(&target) {
            Ok(()) => messages.push(format!("Removed {}", target.display())),
            Err(e) => errors.push(format!("remove {}: {e}", target.display())),
        }
    }
}

fn legacy_appdata_dir(dir_name: &str) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir().map(|c| c.join("Dreamtonics").join(dir_name))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = dir_name;
        None
    }
}

fn studio_data_dir(dir_name: &str) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("Dreamtonics")
                .join(dir_name)
        })
    }

    #[cfg(target_os = "windows")]
    {
        dirs::document_dir().map(|d| d.join("Dreamtonics").join(dir_name))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = dir_name;
        None
    }
}

fn studio_app_present(dir_name: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        std::path::Path::new("/Applications")
            .join(format!("{dir_name}.app"))
            .exists()
    }

    #[cfg(target_os = "windows")]
    {
        let bases = [
            std::env::var("ProgramFiles").ok().map(PathBuf::from),
            std::env::var("ProgramFiles(x86)").ok().map(PathBuf::from),
        ];
        bases.into_iter().flatten().any(|b| b.join("Dreamtonics").join(dir_name).exists())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = dir_name;
        false
    }
}
