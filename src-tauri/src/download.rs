//! Generic streaming HTTP download with SHA256 verification and progress
//! events.
//!
//! Used by every install action that needs to fetch a file (the main-app
//! installer, the Cubase script & template, the SynthV script, the help
//! files, the seed config). Not used for ollama_pull (that has its own
//! flow because Ollama owns the download progress).
//!
//! Progress is reported via Tauri events on channel `download://progress`.
//! The event payload is [`DownloadProgress`].
//!
//! Cache: downloads are stored under `<installer_data_dir>/downloads/`
//! using the file's SHA256 as the on-disk name plus the original
//! extension. If a cached file's SHA matches the expected one, we skip
//! the network entirely.

use crate::paths;
use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub download_id: String,
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    pub phase: &'static str,
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("checksum mismatch (expected {expected}, got {actual})")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("size mismatch (expected {expected}, got {actual})")]
    SizeMismatch { expected: u64, actual: u64 },
    #[error("no download cache directory available")]
    NoCacheDir,
}

impl From<reqwest::Error> for DownloadError {
    fn from(e: reqwest::Error) -> Self {
        DownloadError::Http(e.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct DownloadSpec {
    pub url: String,
    pub expected_sha256_lc: String,
    pub expected_size_bytes: u64,
    pub file_name: String,
    pub download_id: String,
    /// If set and the file exists, the download is satisfied from this
    /// local path (still SHA256-verified) instead of HTTP. Populated by
    /// `install::download_spec_for` when a local / offline repo is
    /// configured. Strict offline: if `local_source` points at a missing
    /// file we error out rather than silently falling back to network.
    pub local_source: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct DownloadOutcome {
    pub local_path: PathBuf,
    pub sha256_lc: String,
    pub bytes: u64,
    pub used_cache: bool,
}

pub async fn fetch_with_verify(
    app: &AppHandle,
    spec: &DownloadSpec,
) -> Result<DownloadOutcome, DownloadError> {
    // Offline / local-source short-circuit. SHA256 verification still
    // applies. We do NOT copy the file into the download cache - the
    // local repo IS the source of truth in offline mode.
    if let Some(local) = spec.local_source.as_ref() {
        if !local.exists() {
            return Err(DownloadError::Http(format!(
                "offline mode: expected file not found in local repo: {}",
                local.display()
            )));
        }
        emit(
            app,
            DownloadProgress {
                download_id: spec.download_id.clone(),
                bytes_downloaded: 0,
                bytes_total: spec.expected_size_bytes,
                phase: "verifying_local",
            },
        );
        let actual_sha = sha256_of_file(local).await?;
        if actual_sha != spec.expected_sha256_lc {
            return Err(DownloadError::ChecksumMismatch {
                expected: spec.expected_sha256_lc.clone(),
                actual: actual_sha,
            });
        }
        let bytes = std::fs::metadata(local).map(|m| m.len()).unwrap_or(0);
        if spec.expected_size_bytes > 0 && bytes != spec.expected_size_bytes {
            return Err(DownloadError::SizeMismatch {
                expected: spec.expected_size_bytes,
                actual: bytes,
            });
        }
        emit(
            app,
            DownloadProgress {
                download_id: spec.download_id.clone(),
                bytes_downloaded: bytes,
                bytes_total: bytes,
                phase: "verified_local",
            },
        );
        return Ok(DownloadOutcome {
            local_path: local.clone(),
            sha256_lc: actual_sha,
            bytes,
            used_cache: true,
        });
    }

    let cache_dir = paths::installer_download_cache_dir().ok_or(DownloadError::NoCacheDir)?;
    std::fs::create_dir_all(&cache_dir)?;

    let cached = cache_dir.join(cached_name(&spec.expected_sha256_lc, &spec.file_name));

    if cached.exists() {
        if let Ok(actual) = sha256_of_file(&cached).await {
            if actual == spec.expected_sha256_lc {
                emit(
                    app,
                    DownloadProgress {
                        download_id: spec.download_id.clone(),
                        bytes_downloaded: spec.expected_size_bytes,
                        bytes_total: spec.expected_size_bytes,
                        phase: "cache_hit",
                    },
                );
                return Ok(DownloadOutcome {
                    local_path: cached,
                    sha256_lc: actual,
                    bytes: spec.expected_size_bytes,
                    used_cache: true,
                });
            }
            // Stale cache: remove and re-download.
            let _ = tokio::fs::remove_file(&cached).await;
        }
    }

    emit(
        app,
        DownloadProgress {
            download_id: spec.download_id.clone(),
            bytes_downloaded: 0,
            bytes_total: spec.expected_size_bytes,
            phase: "starting",
        },
    );

    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "smartbridge-setup/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()?;

    let resp = client.get(&spec.url).send().await?;
    if !resp.status().is_success() {
        return Err(DownloadError::Http(format!(
            "GET {}: HTTP {}",
            spec.url,
            resp.status()
        )));
    }

    let total = resp
        .content_length()
        .unwrap_or(spec.expected_size_bytes);

    let tmp_path = cached.with_extension("download.partial");
    let mut tmp_file = tokio::fs::File::create(&tmp_path).await?;
    let mut hasher = Sha256::new();
    let mut bytes_downloaded: u64 = 0;
    let mut last_emit: u64 = 0;
    let emit_interval: u64 = (total / 100).max(64 * 1024); // every 1% or 64 KB

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| DownloadError::Http(e.to_string()))?;
        hasher.update(&chunk);
        tmp_file.write_all(&chunk).await?;
        bytes_downloaded += chunk.len() as u64;

        if bytes_downloaded - last_emit >= emit_interval {
            last_emit = bytes_downloaded;
            emit(
                app,
                DownloadProgress {
                    download_id: spec.download_id.clone(),
                    bytes_downloaded,
                    bytes_total: total,
                    phase: "downloading",
                },
            );
        }
    }
    tmp_file.flush().await?;
    drop(tmp_file);

    let actual_sha = hex::encode(hasher.finalize()).to_lowercase();
    if actual_sha != spec.expected_sha256_lc {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(DownloadError::ChecksumMismatch {
            expected: spec.expected_sha256_lc.clone(),
            actual: actual_sha,
        });
    }

    if spec.expected_size_bytes > 0 && bytes_downloaded != spec.expected_size_bytes {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(DownloadError::SizeMismatch {
            expected: spec.expected_size_bytes,
            actual: bytes_downloaded,
        });
    }

    tokio::fs::rename(&tmp_path, &cached).await?;

    emit(
        app,
        DownloadProgress {
            download_id: spec.download_id.clone(),
            bytes_downloaded,
            bytes_total: total,
            phase: "verified",
        },
    );

    Ok(DownloadOutcome {
        local_path: cached,
        sha256_lc: actual_sha,
        bytes: bytes_downloaded,
        used_cache: false,
    })
}

fn cached_name(sha_lc: &str, file_name: &str) -> String {
    let ext = std::path::Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    format!("{}.{}", &sha_lc[..16], ext)
}

async fn sha256_of_file(path: &std::path::Path) -> Result<String, DownloadError> {
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()).to_lowercase())
}

fn emit(app: &AppHandle, progress: DownloadProgress) {
    if let Err(e) = app.emit("download://progress", progress) {
        tracing::warn!(error = %e, "failed to emit download progress");
    }
}
