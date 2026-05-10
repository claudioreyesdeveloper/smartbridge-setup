//! Per-component install actions.
//!
//! Each module here implements one component's install/repair logic. The
//! file-copy components (smartbridge-resources, help-files, synthv,
//! cubase) all follow the same shape:
//!
//!   1. Resolve manifest assets needed for this component.
//!   2. Download each asset via [`crate::download::fetch_with_verify`]
//!      (cache-aware, SHA256-verified).
//!   3. Place the file at its canonical OS location.
//!   4. Re-detect to confirm Ready.
//!
//! Heavyweight components (main-app native installer, ai-lyrics ollama
//! pull) live in their own modules with bespoke logic.
//!
//! All install actions are idempotent: running them on an already-Ready
//! component re-verifies and either no-ops (cache hit + same checksum)
//! or repairs (replaces the file with a fresh download).

pub mod ai_lyrics;
pub mod ai_lyrics_default;
pub mod clean_uninstall;
pub mod cubase;
pub mod help_files;
pub mod loopmidi;
pub mod main_app;
pub mod resources;
pub mod synthv;
pub mod yamaha_steinberg;
pub mod zip_util;

use crate::detection::DetectionResult;
use crate::download::DownloadSpec;
use crate::manifest::{Delivery, Manifest, ManifestAsset};

use serde::Serialize;
use tauri::AppHandle;

/// Convert a manifest asset into the shape the downloader expects.
/// Returns None for delivery methods that aren't downloadable as-is
/// (e.g. ollama_pull).
pub fn download_spec_for(asset: &ManifestAsset) -> Option<DownloadSpec> {
    let (url, sha, size) = match &asset.delivery {
        Delivery::GithubReleaseAsset {
            download_url,
            sha256,
            file_size_bytes,
            ..
        } => (download_url.clone(), sha256.clone(), *file_size_bytes),
        Delivery::Http {
            source_url,
            sha256,
            file_size_bytes,
        } => (source_url.clone(), sha256.clone(), *file_size_bytes),
        Delivery::R2 { .. } | Delivery::OllamaPull { .. } => return None,
    };

    // Offline / local repo: if a bundle dir is configured AND it
    // physically contains this asset, point the downloader at it. Strict
    // mode - if the user opted into offline but the file is missing in
    // the bundle, the downloader surfaces a clear error instead of
    // silently fetching from the internet.
    let local_source = crate::paths::local_repo_dir()
        .and_then(|dir| crate::local_repo::asset_local_path(&dir, asset));

    Some(DownloadSpec {
        url,
        expected_sha256_lc: sha.to_lowercase(),
        expected_size_bytes: size,
        file_name: asset.file_name.clone(),
        download_id: asset.asset_id.clone(),
        local_source,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallOutcome {
    pub component_id: String,
    pub success: bool,
    pub messages: Vec<String>,
    pub post_state: Option<DetectionResult>,
}

impl InstallOutcome {
    pub fn ok(component_id: impl Into<String>, messages: Vec<String>) -> Self {
        let component_id = component_id.into();
        for m in &messages {
            tracing::info!(target: "install", component = %component_id, "{}", m);
        }
        tracing::info!(target: "install", component = %component_id, "outcome=ok");
        Self {
            component_id,
            success: true,
            messages,
            post_state: None,
        }
    }

    pub fn err(component_id: impl Into<String>, messages: Vec<String>) -> Self {
        let component_id = component_id.into();
        // Only the LAST line of an err outcome is typically the actual
        // failure cause; log every line at warn so the order is preserved
        // and the support bundle contains the full chain of attempts.
        for m in &messages {
            tracing::warn!(target: "install", component = %component_id, "{}", m);
        }
        tracing::error!(target: "install", component = %component_id, "outcome=err");
        Self {
            component_id,
            success: false,
            messages,
            post_state: None,
        }
    }

    pub fn with_post_state(mut self, det: DetectionResult) -> Self {
        self.post_state = Some(det);
        self
    }
}

/// Dispatch by component_id.
pub async fn install(
    app: &AppHandle,
    manifest: &Manifest,
    component_id: &str,
) -> InstallOutcome {
    tracing::info!(
        target: "install",
        component = component_id,
        manifest_version = %manifest.release_version,
        "install dispatch",
    );
    match component_id {
        "main-app" => main_app::install(app, manifest).await,
        "cubase-connection" => cubase::install(app, manifest).await,
        "ai-lyrics" => ai_lyrics::install(app, manifest).await,
        "ai-lyrics-default-model" => ai_lyrics_default::install(app, manifest).await,
        "synthv-connection" => synthv::install(app, manifest).await,
        "smartbridge-resources" => resources::install(app, manifest).await,
        "help-files" => help_files::install(app, manifest).await,
        "windows-loopmidi" => loopmidi::install(app).await,
        "yamaha-steinberg-driver" => yamaha_steinberg::install().await,
        other => InstallOutcome::err(
            other,
            vec![format!("unknown component: {other}")],
        ),
    }
}

/// Inverse of [`install`]: remove just this component's artifacts and
/// re-detect. Used by:
///   * the per-card "Remove" button on the dashboard,
///   * the dedicated `--uninstall main-app` flow when the customer
///     uninstalls SmartBridge from Windows Settings → Apps (which also
///     calls [`main_app::uninstall`] directly with `remove_user_data`).
///
/// User data is never destroyed by this entry point. The dedicated
/// main-app uninstall flow has its own checkbox for that.
pub async fn remove(component_id: &str) -> InstallOutcome {
    tracing::info!(target: "install", component = component_id, "remove dispatch");
    match component_id {
        "main-app" => main_app::uninstall(false).await,
        "cubase-connection" => cubase::remove().await,
        "ai-lyrics" => ai_lyrics::remove().await,
        "ai-lyrics-default-model" => ai_lyrics_default::remove().await,
        "synthv-connection" => synthv::remove().await,
        "smartbridge-resources" => resources::remove().await,
        "help-files" => help_files::remove().await,
        "windows-loopmidi" => loopmidi::remove().await,
        "yamaha-steinberg-driver" => yamaha_steinberg::remove().await,
        other => InstallOutcome::err(
            other,
            vec![format!("unknown component: {other}")],
        ),
    }
}
