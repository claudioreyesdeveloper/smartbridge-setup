//! SmartBridge release manifest model.
//!
//! Mirrors the JSON published at:
//!   https://github.com/claudioreyesdeveloper/smartbridge-releases/releases/.../smartbridge-release-manifest.json
//!
//! The shape here MUST match what `scripts/release/build_manifest.py`
//! produces (driven by `scripts/release/components.json`). If you
//! change one, change the other.
//!
//! Fetch + cache lives at the bottom of this file.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Bootstrap URL
// ---------------------------------------------------------------------------

/// Repo that hosts SmartBridge release artifacts and the manifest.
/// This is a separate, public repo so unauthenticated downloads work.
pub const RELEASES_REPO: &str = "claudioreyesdeveloper/smartbridge-releases";

/// The Setup app reads the manifest from
/// `releases/latest/download/smartbridge-release-manifest.json`. GitHub
/// resolves `/latest` to the most recent non-prerelease release, which
/// is why `release.yml` publishes with `prerelease: false`. The
/// `release_channel` field inside the manifest body (e.g. "beta") drives
/// the badge in the UI — there is no longer a separate "beta URL".
pub const MANIFEST_URL: &str =
    "https://github.com/claudioreyesdeveloper/smartbridge-releases/releases/latest/download/smartbridge-release-manifest.json";

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub manifest_version: u32,
    pub product: String,
    pub release_version: String,
    pub release_channel: String,
    pub generated_at: String,
    pub asset_provider: serde_json::Value,
    pub components: Vec<ManifestComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestComponent {
    /// JSON key is `id`; we surface it as `component_id` in Rust to match
    /// the rest of the codebase.
    #[serde(rename = "id")]
    pub component_id: String,
    pub display_name: String,
    pub required: bool,
    pub optional: bool,
    pub status: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub assets: Vec<ManifestAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestAsset {
    pub asset_id: String,
    pub component_id: String,
    pub file_name: String,
    pub version: String,
    pub platform: String,
    pub architecture: String,
    pub content_type: String,
    pub install_action: String,
    pub requires_admin: bool,
    pub signature_required: bool,
    pub signature_status: String,
    #[serde(default)]
    pub signature_waiver_reason: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub delivery: Delivery,
}

impl Manifest {
    pub fn component(&self, id: &str) -> Option<&ManifestComponent> {
        self.components.iter().find(|c| c.component_id == id)
    }
}

impl ManifestComponent {
    pub fn asset(&self, id: &str) -> Option<&ManifestAsset> {
        self.assets.iter().find(|a| a.asset_id == id)
    }
}

/// Tagged on `method` to match the manifest exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum Delivery {
    #[serde(rename = "github_release_asset")]
    GithubReleaseAsset {
        release_tag: String,
        release_asset_name: String,
        download_url: String,
        #[serde(default)]
        object_key: Option<String>,
        sha256: String,
        file_size_bytes: u64,
    },
    #[serde(rename = "r2")]
    R2 {
        object_key: String,
        sha256: String,
        file_size_bytes: u64,
    },
    Http {
        source_url: String,
        sha256: String,
        file_size_bytes: u64,
    },
    OllamaPull {
        ollama_tag: String,
        approx_size_bytes: u64,
        #[serde(default)]
        license_url: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Fetch + cache
// ---------------------------------------------------------------------------

/// Manifest fetch outcome along with the URL we ultimately used.
#[derive(Debug, Clone, Serialize)]
pub struct FetchedManifest {
    pub manifest: Manifest,
    pub source_url: String,
    pub cached_path: String,
    pub fetched_from_network: bool,
}

const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

/// Fetch the release manifest with this priority:
///   0. **Local / offline repo** (env `SMARTBRIDGE_LOCAL_REPO` or persisted
///      setting). When set, we never touch the network for the manifest.
///   1. The single `MANIFEST_URL` (`releases/latest/download/...`). CI
///      publishes with `prerelease: false` so this always resolves.
///   2. Disk cache, if any prior fetch succeeded.
///
/// On any successful network fetch we update the disk cache.
pub async fn fetch_with_fallback() -> Result<FetchedManifest, String> {
    // Offline / local repo wins. Strict: if the user configured a local
    // repo but the manifest there is missing/broken, surface that as an
    // error rather than silently falling through to the network — that
    // would defeat the point of opting into offline mode.
    if let Some(dir) = crate::paths::local_repo_dir() {
        let manifest_path = dir.join(crate::local_repo::MANIFEST_FILENAME);
        match std::fs::read(&manifest_path) {
            Ok(bytes) => match serde_json::from_slice::<Manifest>(&bytes) {
                Ok(manifest) => {
                    tracing::info!(
                        path = %manifest_path.display(),
                        "manifest loaded from local repo"
                    );
                    return Ok(FetchedManifest {
                        manifest,
                        source_url: format!("local:{}", manifest_path.display()),
                        cached_path: manifest_path.to_string_lossy().to_string(),
                        fetched_from_network: false,
                    });
                }
                Err(e) => {
                    return Err(format!(
                        "local repo configured at {} but manifest is unparseable: {e}",
                        dir.display()
                    ));
                }
            },
            Err(e) => {
                return Err(format!(
                    "local repo configured at {} but {} is missing or unreadable: {e}",
                    dir.display(),
                    crate::local_repo::MANIFEST_FILENAME
                ));
            }
        }
    }

    let client = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .user_agent(concat!(
            "smartbridge-setup/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|e| format!("HTTP client init: {e}"))?;

    match try_fetch(&client, MANIFEST_URL).await {
        Ok(manifest) => {
            let cache_path = write_cache(&manifest)
                .unwrap_or_else(|_| "<no cache>".to_string());
            tracing::info!(url = MANIFEST_URL, "manifest fetched");
            return Ok(FetchedManifest {
                manifest,
                source_url: MANIFEST_URL.to_string(),
                cached_path: cache_path,
                fetched_from_network: true,
            });
        }
        Err(e) => {
            tracing::warn!(url = MANIFEST_URL, error = %e, "manifest fetch failed");

            if let Some(cached) = read_cache() {
                tracing::info!("network manifest unreachable, using disk cache");
                let path = crate::paths::installer_manifest_cache_path()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                return Ok(FetchedManifest {
                    manifest: cached,
                    source_url: "<cache>".to_string(),
                    cached_path: path,
                    fetched_from_network: false,
                });
            }

            Err(format!(
                "no manifest reachable and no cache present; last error: {e}"
            ))
        }
    }
}

async fn try_fetch(client: &reqwest::Client, url: &str) -> Result<Manifest, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("GET {url}: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }

    resp.json::<Manifest>()
        .await
        .map_err(|e| format!("parse manifest from {url}: {e}"))
}

fn write_cache(m: &Manifest) -> Result<String, String> {
    let path = crate::paths::installer_manifest_cache_path()
        .ok_or_else(|| "no cache path".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir cache: {e}"))?;
    }
    let json = serde_json::to_vec_pretty(m).map_err(|e| format!("serialize: {e}"))?;

    // Atomic-ish: write to a temp file in the same dir, then rename.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| format!("write tmp cache: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("rename cache: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

fn read_cache() -> Option<Manifest> {
    let path = crate::paths::installer_manifest_cache_path()?;
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice::<Manifest>(&bytes).ok()
}
