//! help-files install: downloads getting-started, installation guide,
//! and the multilingual interactive manual zip into
//! `<user_data_dir>/help/`.

use super::InstallOutcome;
use crate::detection;
use crate::download::fetch_with_verify;
use crate::manifest::Manifest;
use crate::paths;
use tauri::AppHandle;

const COMPONENT: &str = "help-files";
const ASSET_IDS: &[&str] = &[
    "help-files.getting-started",
    "help-files.installation-guide",
    "help-files.interactive-manual",
];

pub async fn install(app: &AppHandle, manifest: &Manifest) -> InstallOutcome {
    let component = match manifest.component(COMPONENT) {
        Some(c) => c,
        None => return InstallOutcome::err(COMPONENT, vec![format!("manifest missing component {COMPONENT}")]),
    };

    let target_dir = match paths::help_files_dir() {
        Some(d) => d,
        None => return InstallOutcome::err(COMPONENT, vec!["could not resolve help dir".into()]),
    };

    if let Err(e) = std::fs::create_dir_all(&target_dir) {
        return InstallOutcome::err(COMPONENT, vec![format!("create {}: {e}", target_dir.display())]);
    }

    let mut messages: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for asset_id in ASSET_IDS {
        let asset = match component.asset(asset_id) {
            Some(a) => a,
            None => {
                errors.push(format!("manifest missing asset {asset_id}"));
                continue;
            }
        };

        let spec = match super::download_spec_for(asset) {
            Some(s) => s,
            None => {
                errors.push(format!("asset {asset_id} is not downloadable"));
                continue;
            }
        };

        let outcome = match fetch_with_verify(app, &spec).await {
            Ok(o) => o,
            Err(e) => {
                errors.push(format!("{asset_id}: download failed: {e}"));
                continue;
            }
        };

        let target = target_dir.join(&asset.file_name);
        match std::fs::copy(&outcome.local_path, &target) {
            Ok(_) => messages.push(format!("placed {} ({} bytes)", target.display(), outcome.bytes)),
            Err(e) => errors.push(format!("copy {} → {}: {e}", asset.file_name, target.display())),
        }
    }

    let det = detection::help_files::detect().await;

    if errors.is_empty() {
        InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
    } else {
        let mut all = messages;
        all.extend(errors);
        InstallOutcome::err(COMPONENT, all).with_post_state(det)
    }
}

/// Delete the help directory under the user data dir.
pub async fn remove() -> InstallOutcome {
    let target_dir = match paths::help_files_dir() {
        Some(d) => d,
        None => {
            return InstallOutcome::err(COMPONENT, vec!["could not resolve help dir".into()]);
        }
    };
    let mut messages: Vec<String> = Vec::new();
    if target_dir.exists() {
        match std::fs::remove_dir_all(&target_dir) {
            Ok(()) => messages.push(format!("Removed {}", target_dir.display())),
            Err(e) => {
                let det = detection::help_files::detect().await;
                return InstallOutcome::err(
                    COMPONENT,
                    vec![format!("remove {}: {e}", target_dir.display())],
                )
                .with_post_state(det);
            }
        }
    } else {
        messages.push(format!("No help files at {}", target_dir.display()));
    }
    let det = detection::help_files::detect().await;
    InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
}
