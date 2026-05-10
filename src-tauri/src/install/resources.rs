//! smartbridge-resources install: places the seed config.json at the
//! user's SmartBridge data dir, ONLY IF no config.json already exists.
//!
//! This is the single most important user-data invariant in the entire
//! installer: we never, ever overwrite an existing user config. If you
//! find yourself tempted to add a `--force` flag that does, stop.

use super::InstallOutcome;
use crate::detection;
use crate::download::fetch_with_verify;
use crate::manifest::Manifest;
use crate::paths;
use tauri::AppHandle;

const COMPONENT: &str = "smartbridge-resources";
const ASSET_ID: &str = "smartbridge-resources.config-default";

pub async fn install(app: &AppHandle, manifest: &Manifest) -> InstallOutcome {
    let component = match manifest.component(COMPONENT) {
        Some(c) => c,
        None => return InstallOutcome::err(COMPONENT, vec![format!("manifest is missing component {COMPONENT}")]),
    };
    let asset = match component.asset(ASSET_ID) {
        Some(a) => a,
        None => return InstallOutcome::err(COMPONENT, vec![format!("manifest is missing asset {ASSET_ID}")]),
    };

    let target = match paths::user_config_path() {
        Some(p) => p,
        None => return InstallOutcome::err(COMPONENT, vec!["could not resolve user config path".into()]),
    };

    let mut messages: Vec<String> = Vec::new();

    if target.exists() {
        messages.push(format!(
            "config already present at {} — leaving it untouched.",
            target.display()
        ));
        let det = detection::resources::detect().await;
        return InstallOutcome::ok(COMPONENT, messages).with_post_state(det);
    }

    let spec = match super::download_spec_for(asset) {
        Some(s) => s,
        None => return InstallOutcome::err(COMPONENT, vec!["asset is not downloadable".into()]),
    };

    let outcome = match fetch_with_verify(app, &spec).await {
        Ok(o) => o,
        Err(e) => return InstallOutcome::err(COMPONENT, vec![format!("download failed: {e}")]),
    };

    if let Some(parent) = target.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return InstallOutcome::err(COMPONENT, vec![format!("create {}: {e}", parent.display())]);
        }
    }

    if let Err(e) = std::fs::copy(&outcome.local_path, &target) {
        return InstallOutcome::err(COMPONENT, vec![format!("copy to {}: {e}", target.display())]);
    }

    messages.push(format!(
        "placed seed config at {} ({} bytes, sha256 {})",
        target.display(),
        outcome.bytes,
        &outcome.sha256_lc[..16]
    ));

    let det = detection::resources::detect().await;
    InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
}

/// "Remove" semantics for the resources component intentionally do NOT
/// delete the user's `config.json`. The whole point of the seed-only
/// install is that user config is sacred. We only surface a friendly
/// message; if the customer really wants to wipe their config they can
/// do it via the main-app uninstall flow's "also delete my data" toggle.
pub async fn remove() -> InstallOutcome {
    let target = paths::user_config_path();
    let det = detection::resources::detect().await;
    let mut messages: Vec<String> = Vec::new();
    if let Some(p) = target {
        if p.exists() {
            messages.push(format!(
                "Kept your config at {} — Setup never deletes a customer's \
                 configuration on a routine remove. Use the full SmartBridge \
                 uninstaller (Windows Settings → Apps) and tick \"also delete \
                 my data\" if you want it gone.",
                p.display()
            ));
        } else {
            messages.push("No config.json present — nothing to remove.".into());
        }
    }
    InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
}
