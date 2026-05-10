//! "Save log bundle" — produce a single self-contained zip the customer
//! can email to support when something goes wrong.
//!
//! What goes in:
//!   * `setup-logs/` — every `setup.YYYY-MM-DD.log` we still have on
//!     disk (the rotated tracing-appender output). Trimmed to the most
//!     recent 14 files so a customer who's been using Setup for months
//!     doesn't email a 50 MB zip.
//!   * `smartbridge-app-log/` — the standalone SmartBridge.exe log
//!     (`%APPDATA%\SmartBridge\SmartBridge.log` on Windows,
//!     `~/Library/Application Support/SmartBridge/SmartBridge.log` on
//!     macOS — the path JUCE's userApplicationDataDirectory resolves to,
//!     same as `SmartBridgeLogger`). Optional; included only if present.
//!   * `metadata.txt` — non-secret diagnostics: Setup version, host
//!     OS/arch, manifest URLs, local-repo state, uninstall-mode flag,
//!     and the on-disk file listing of the SmartBridge user-data dir
//!     (filenames + sizes only, no contents).
//!
//! What does NOT go in:
//!   * `config.json` — contains the customer's lyrics_apiKey.
//!   * The encrypted `smartbridge.db` itself — irrelevant to most
//!     install/setup issues and unnecessarily large.
//!   * Anything from outside the Setup data dir or the SmartBridge
//!     user-data dir.
//!
//! The bundle lands on the user's Desktop with a timestamped filename
//! and the OS file manager opens highlighting it, so the customer can
//! drag-and-drop it into an email without going hunting.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

const MAX_SETUP_LOGS: usize = 14;

/// Build a support-bundle zip and return its absolute path.
pub fn build() -> Result<PathBuf, String> {
    let dest = bundle_destination()?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create {}: {e}", parent.display()))?;
    }

    let f = File::create(&dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    let mut zip = zip::ZipWriter::new(f);
    let opts = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    // 1. metadata.txt
    let metadata = build_metadata();
    zip.start_file("metadata.txt", opts)
        .map_err(|e| format!("zip start metadata: {e}"))?;
    zip.write_all(metadata.as_bytes())
        .map_err(|e| format!("zip write metadata: {e}"))?;
    tracing::info!(
        target: "support_bundle",
        bytes = metadata.len(),
        "wrote metadata.txt",
    );

    // 2. Setup logs (most recent N).
    if let Some(log_dir) = crate::paths::installer_log_dir() {
        let added = add_recent_logs(&mut zip, &log_dir, "setup-logs", MAX_SETUP_LOGS, opts);
        tracing::info!(
            target: "support_bundle",
            log_dir = %log_dir.display(),
            files = added,
            "added Setup logs",
        );
    }

    // 3. SmartBridge.log if present.
    if let Some(app_log) = smartbridge_app_log_path() {
        if app_log.exists() {
            match add_file(&mut zip, &app_log, "smartbridge-app-log/SmartBridge.log", opts) {
                Ok(bytes) => tracing::info!(
                    target: "support_bundle",
                    path = %app_log.display(),
                    bytes,
                    "added SmartBridge.log",
                ),
                Err(e) => tracing::warn!(
                    target: "support_bundle",
                    path = %app_log.display(),
                    error = %e,
                    "could not add SmartBridge.log to bundle",
                ),
            }
        }
    }

    zip.finish().map_err(|e| format!("finalise zip: {e}"))?;

    tracing::info!(
        target: "support_bundle",
        path = %dest.display(),
        "support bundle written",
    );

    // Reveal in OS file manager (Explorer / Finder). Best-effort —
    // failure here is fine, the path is returned to the UI which shows
    // it to the user anyway.
    let _ = reveal_in_file_manager(&dest);

    Ok(dest)
}

fn bundle_destination() -> Result<PathBuf, String> {
    let stamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S");
    let name = format!("SmartBridge-Setup-Diagnostics-{stamp}.zip");

    // Prefer Desktop. If we can't resolve it (locked-down corp setup,
    // unusual home dir layout) fall back to the Setup data dir, which
    // is guaranteed to exist because the logger creates it.
    if let Some(desktop) = dirs::desktop_dir() {
        return Ok(desktop.join(name));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(name));
    }
    if let Some(data) = crate::paths::installer_data_dir() {
        return Ok(data.join(name));
    }
    Err("could not resolve a destination directory for the support bundle".into())
}

fn build_metadata() -> String {
    let mut out = String::new();
    out.push_str("SmartBridge Setup — support bundle\n");
    out.push_str("===================================\n");
    out.push_str(&format!(
        "generated_at: {}\n",
        chrono::Local::now().to_rfc3339()
    ));
    out.push_str(&format!("setup_version: {}\n", env!("CARGO_PKG_VERSION")));
    out.push_str(&format!("os: {}\n", std::env::consts::OS));
    out.push_str(&format!("arch: {}\n", std::env::consts::ARCH));
    out.push_str(&format!("family: {}\n", std::env::consts::FAMILY));

    let host = crate::host::current();
    out.push_str(&format!("host_label: {}\n", host.os));

    let mode = crate::uninstall_mode();
    out.push_str(&format!("uninstall_mode_active: {}\n", mode.active));
    if let Some(c) = &mode.component {
        out.push_str(&format!("uninstall_mode_component: {c}\n"));
    }

    out.push('\n');
    out.push_str("manifest URL:\n");
    out.push_str(&format!("  {}\n", crate::manifest::MANIFEST_URL));

    out.push('\n');
    out.push_str("local repo (offline mode):\n");
    let local = crate::local_repo::current_status();
    out.push_str(&format!("  configured: {}\n", local.configured));
    out.push_str(&format!("  manifest_present: {}\n", local.manifest_present));
    if let Some(p) = &local.path {
        out.push_str(&format!("  path: {p}\n"));
    }
    if let Some(v) = &local.manifest_version {
        out.push_str(&format!("  manifest_version: {v}\n"));
    }
    out.push_str(&format!("  from_env: {}\n", local.from_env));

    out.push('\n');
    out.push_str("Setup paths:\n");
    if let Some(p) = crate::paths::installer_data_dir() {
        out.push_str(&format!("  installer_data_dir: {}\n", p.display()));
    }
    if let Some(p) = crate::paths::installer_log_dir() {
        out.push_str(&format!("  installer_log_dir:  {}\n", p.display()));
    }
    if let Some(p) = crate::paths::user_data_dir() {
        out.push_str(&format!("  smartbridge_user_data_dir: {}\n", p.display()));
    }

    out.push('\n');
    out.push_str("SmartBridge user-data dir contents (names + sizes, no file bodies):\n");
    match crate::paths::user_data_dir() {
        Some(dir) if dir.exists() => {
            match list_dir_shallow(&dir) {
                Ok(lines) if lines.is_empty() => out.push_str("  (empty)\n"),
                Ok(lines) => {
                    for l in lines {
                        out.push_str(&format!("  {l}\n"));
                    }
                }
                Err(e) => out.push_str(&format!("  (could not list: {e})\n")),
            }
        }
        Some(dir) => out.push_str(&format!("  (does not exist: {})\n", dir.display())),
        None => out.push_str("  (cannot resolve user data dir on this OS)\n"),
    }

    out
}

/// Lists `dir` one level deep. Returns `name (size)` lines for files and
/// `name/` lines for subdirectories. Never recurses — the bundle is for
/// triage, not forensics.
fn list_dir_shallow(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let ty = entry.file_type()?;
        if ty.is_dir() {
            out.push(format!("{name}/"));
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(format!("{name}  ({size} bytes)"));
        }
    }
    out.sort();
    Ok(out)
}

/// Where the standalone SmartBridge.exe / SmartBridge.app writes its log
/// (`SmartBridgeLogger.cpp`). NOT under `~/Library/SmartBridge` — the
/// JUCE logger uses `userApplicationDataDirectory`, i.e.
/// `~/Library/Application Support/SmartBridge/SmartBridge.log` on macOS
/// and `%APPDATA%\SmartBridge\SmartBridge.log` on Windows.
fn smartbridge_app_log_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("SmartBridge")
                .join("SmartBridge.log")
        })
    }

    #[cfg(target_os = "windows")]
    {
        dirs::config_dir().map(|c| c.join("SmartBridge").join("SmartBridge.log"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs::config_dir().map(|c| c.join("SmartBridge").join("SmartBridge.log"))
    }
}

fn add_recent_logs<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    log_dir: &Path,
    zip_prefix: &str,
    keep: usize,
    opts: SimpleFileOptions,
) -> usize {
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    let rd = match std::fs::read_dir(log_dir) {
        Ok(rd) => rd,
        Err(_) => return 0,
    };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let mtime = e
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        entries.push((p, mtime));
    }
    // newest first, then keep only N.
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.truncate(keep);

    let mut added = 0usize;
    for (path, _) in entries {
        let name = match path.file_name() {
            Some(n) => n.to_string_lossy().into_owned(),
            None => continue,
        };
        let zip_path = format!("{zip_prefix}/{name}");
        if add_file(zip, &path, &zip_path, opts).is_ok() {
            added += 1;
        }
    }
    added
}

fn add_file<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    src: &Path,
    zip_path: &str,
    opts: SimpleFileOptions,
) -> Result<u64, String> {
    let mut f = File::open(src).map_err(|e| format!("open {}: {e}", src.display()))?;
    zip.start_file(zip_path, opts)
        .map_err(|e| format!("zip start {zip_path}: {e}"))?;
    let mut buf = [0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("read {}: {e}", src.display()))?;
        if n == 0 {
            break;
        }
        zip.write_all(&buf[..n])
            .map_err(|e| format!("zip write {zip_path}: {e}"))?;
        total += n as u64;
    }
    Ok(total)
}

#[cfg(target_os = "windows")]
fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("spawn explorer: {e}"))
}

#[cfg(target_os = "macos")]
fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
    std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("spawn open: {e}"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
    let dir = path.parent().unwrap_or(path);
    std::process::Command::new("xdg-open")
        .arg(dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("spawn xdg-open: {e}"))
}
