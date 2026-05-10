//! Offline / local-repository support.
//!
//! When SmartBridge Setup is configured with a local repo (env var
//! `SMARTBRIDGE_LOCAL_REPO=/some/dir` or via the Diagnostics tab), every
//! manifest read and every asset download is satisfied from disk instead
//! of the network. SHA256 verification still happens against the local
//! file - we never trust the bundle blindly.
//!
//! Expected on-disk layout for an offline bundle:
//!
//! ```text
//! my-smartbridge-offline-bundle/
//! ├── smartbridge-release-manifest.json
//! ├── SmartBridge_2.0.0.pkg
//! ├── SmartBridge_2.0.0_Setup.exe
//! ├── build_features.macos.json
//! ├── build_features.windows.json
//! ├── config-default.json
//! ├── SmartBridge.cpr
//! ├── SmartBridge_GenosSlotRename.js
//! ├── synthv_smartbridge_sidepanel.lua
//! ├── SmartBridge_Getting_Started_One_Page.txt
//! ├── Installation_guide.zip
//! └── smartbridge_multilingual_manual.zip
//! ```
//!
//! All asset files live flat inside the bundle dir, named exactly as the
//! manifest's `release_asset_name` (or `file_name` for non-GitHub
//! deliveries). The script `scripts/build_offline_bundle.sh` produces
//! exactly this layout.

#![allow(dead_code)]

use crate::manifest::{Delivery, Manifest, ManifestAsset};
use crate::paths;
use serde::Serialize;
use std::path::PathBuf;

/// Filename of the manifest inside an offline bundle. Hard-coded because
/// it has to match what `build_offline_bundle.sh` produces.
pub const MANIFEST_FILENAME: &str = "smartbridge-release-manifest.json";

/// User-visible status describing the current offline-mode configuration.
/// Surfaced to the frontend in the Diagnostics tab and the header badge.
#[derive(Debug, Clone, Serialize)]
pub struct LocalRepoStatus {
    /// True iff `SMARTBRIDGE_LOCAL_REPO` env var or persisted config
    /// resolves to an existing directory.
    pub configured: bool,

    /// The directory path, if configured.
    pub path: Option<String>,

    /// True iff the directory contains a parseable manifest. A configured
    /// path with a missing/broken manifest is reported with `configured =
    /// true` but `manifest_present = false` so the UI can flag it.
    pub manifest_present: bool,

    /// `release_version` from the local manifest, when present.
    pub manifest_version: Option<String>,

    /// Path the offline mode was set via env var (overrides persisted).
    pub from_env: bool,
}

/// Compute the absolute path of an asset inside a local bundle.
/// Matches whatever filename the manifest exposes:
///   - `Delivery::GithubReleaseAsset` -> uses `release_asset_name`
///   - `Delivery::Http` -> uses the URL's basename
///   - `Delivery::R2` -> uses `object_key`'s basename
///   - `Delivery::OllamaPull` -> not file-backed, returns None
pub fn asset_local_path(repo_dir: &PathBuf, asset: &ManifestAsset) -> Option<PathBuf> {
    let basename = match &asset.delivery {
        Delivery::GithubReleaseAsset { release_asset_name, .. } => release_asset_name.clone(),
        Delivery::Http { source_url, .. } => {
            std::path::Path::new(source_url)
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())?
        }
        Delivery::R2 { object_key, .. } => {
            std::path::Path::new(object_key)
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())?
        }
        Delivery::OllamaPull { .. } => return None,
    };
    Some(repo_dir.join(basename))
}

/// Read and parse the manifest from the configured local repo.
/// Returns the parsed `Manifest` plus the absolute path it came from.
pub fn read_local_manifest() -> Option<(Manifest, PathBuf)> {
    let dir = paths::local_repo_dir()?;
    let path = dir.join(MANIFEST_FILENAME);
    let bytes = std::fs::read(&path).ok()?;
    let manifest = serde_json::from_slice::<Manifest>(&bytes).ok()?;
    Some((manifest, path))
}

/// Snapshot the current offline-mode configuration for display.
pub fn current_status() -> LocalRepoStatus {
    let from_env = std::env::var("SMARTBRIDGE_LOCAL_REPO")
        .ok()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let dir = paths::local_repo_dir();
    let configured = dir.is_some();
    let path_str = dir.as_ref().map(|p| p.display().to_string());

    let (manifest_present, manifest_version) = match read_local_manifest() {
        Some((m, _)) => (true, Some(m.release_version)),
        None => (false, None),
    };

    LocalRepoStatus {
        configured,
        path: path_str,
        manifest_present,
        manifest_version,
        from_env,
    }
}
