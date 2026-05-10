//! Detect installed DAWs and offer to clear their plugin caches. After a
//! SmartBridge reinstall, the host cache often still says "blacklisted"
//! until the cache is removed and the host rescans on next launch.
//!
//! We never *replace* a host cache, only delete it. Hosts always rebuild
//! their cache on next start — they treat the absence of the cache as a
//! signal to do a fresh full scan, which is exactly what we want.

use super::{FixAction, FixOutcome, PreflightCheck};
use std::path::PathBuf;

const CUBASE_HOST_ID: &str = "cubase";
const STUDIO_ONE_HOST_ID: &str = "studio_one";
const REAPER_HOST_ID: &str = "reaper";
const LIVE_HOST_ID: &str = "live";

pub async fn check_cubase() -> PreflightCheck {
    let files = cubase_cache_files();
    build_check(
        "host.cubase_cache",
        "Cubase / Nuendo plugin cache",
        CUBASE_HOST_ID,
        &files,
        "Cubase",
    )
}

pub async fn check_studio_one() -> PreflightCheck {
    let files = studio_one_cache_files();
    build_check(
        "host.studio_one_cache",
        "Studio One plugin cache",
        STUDIO_ONE_HOST_ID,
        &files,
        "Studio One",
    )
}

pub async fn check_reaper() -> PreflightCheck {
    let files = reaper_cache_files();
    build_check(
        "host.reaper_cache",
        "Reaper plugin cache",
        REAPER_HOST_ID,
        &files,
        "Reaper",
    )
}

pub async fn check_live() -> PreflightCheck {
    let files = live_cache_files();
    build_check(
        "host.live_cache",
        "Ableton Live plugin database",
        LIVE_HOST_ID,
        &files,
        "Ableton Live",
    )
}

/// Apply the ClearHostCache fix.
pub async fn clear(host_id: &str) -> FixOutcome {
    let files = match host_id {
        CUBASE_HOST_ID => cubase_cache_files(),
        STUDIO_ONE_HOST_ID => studio_one_cache_files(),
        REAPER_HOST_ID => reaper_cache_files(),
        LIVE_HOST_ID => live_cache_files(),
        other => {
            return FixOutcome::err(
                FixAction::ClearHostCache { host_id: other.to_string() },
                vec![format!("Unknown host id: {other}")],
            );
        }
    };

    let fix = FixAction::ClearHostCache { host_id: host_id.to_string() };

    if files.is_empty() {
        return FixOutcome::ok(
            fix,
            vec![format!("No {host_id} cache files were found, nothing to clear.")],
        );
    }

    let mut messages: Vec<String> = Vec::new();
    let mut any_failed = false;
    for f in &files {
        if !f.exists() {
            continue;
        }
        match std::fs::remove_file(f) {
            Ok(_) => messages.push(format!("Cleared cache: {}", f.display())),
            Err(e) => {
                any_failed = true;
                messages.push(format!("Could not clear {}: {e} (close {host_id} and try again)", f.display()));
            }
        }
    }

    if messages.is_empty() {
        messages.push(format!("No {host_id} cache files were present."));
    } else {
        messages.push(format!(
            "Next time {host_id} launches it will do a full plugin rescan and pick up SmartBridge."
        ));
    }

    if any_failed {
        FixOutcome::err(fix, messages)
    } else {
        FixOutcome::ok(fix, messages)
    }
}

fn build_check(
    id: &'static str,
    label: &'static str,
    host_id: &'static str,
    files: &[PathBuf],
    display_name: &'static str,
) -> PreflightCheck {
    let existing: Vec<&PathBuf> = files.iter().filter(|p| p.exists()).collect();

    if files.is_empty() {
        return PreflightCheck::skipped(
            id,
            label,
            format!("Cache-clear support for {display_name} is not implemented on this OS yet."),
        );
    }

    if existing.is_empty() {
        return PreflightCheck::skipped(
            id,
            label,
            format!("No {display_name} install detected on this PC, nothing to clean up."),
        );
    }

    let mut details: Vec<String> = existing
        .iter()
        .map(|p| format!("found: {}", p.display()))
        .collect();
    details.push(format!(
        "Clearing these forces {display_name} to rescan all plugins on next launch."
    ));

    PreflightCheck::warn(
        id,
        label,
        format!(
            "{display_name} is installed and has a plugin cache. After installing or repairing SmartBridge, this cache may still mark the plugin as blacklisted from a previous failed scan. Click 'Clear cache' to delete it — {display_name} will rebuild it the next time it launches."
        ),
    )
    .with_details(details)
    .with_fix(FixAction::ClearHostCache { host_id: host_id.to_string() })
}

// =============================================================================
// Per-host cache file enumeration
// =============================================================================

fn cubase_cache_files() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut out: Vec<PathBuf> = Vec::new();
        if let Some(app) = dirs::config_dir() {
            // %APPDATA%\Steinberg\Cubase XX_64\Vst3xPluginScanner\*.xml
            // and Vst2xPlugin.xml — we wipe everything that looks like a
            // plugin cache, keep the rest of the prefs alone.
            let steinberg = app.join("Steinberg");
            for v in 12..=20 {
                let dir = steinberg.join(format!("Cubase {v}_64"));
                if !dir.exists() {
                    continue;
                }
                for fname in [
                    "Vst3xPluginScanner.xml",
                    "Vst3PluginScanner.xml",
                    "Vst2xPlugin.xml",
                    "Vst3xPluginRescanner.xml",
                ] {
                    out.push(dir.join(fname));
                }
                // Also rescan-on-next-launch: Steinberg sometimes uses
                // a subfolder named "Vst3xPluginScanner" containing
                // host-specific xmls.
                let subdir = dir.join("Vst3xPluginScanner");
                if subdir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&subdir) {
                        for e in entries.flatten() {
                            let p = e.path();
                            if p.extension().and_then(|s| s.to_str()) == Some("xml") {
                                out.push(p);
                            }
                        }
                    }
                }
            }
        }
        out
    }

    #[cfg(target_os = "macos")]
    {
        let mut out: Vec<PathBuf> = Vec::new();
        if let Some(home) = dirs::home_dir() {
            let steinberg = home.join("Library").join("Preferences").join("Steinberg");
            for v in 12..=20 {
                let dir = steinberg.join(format!("Cubase {v}"));
                if dir.exists() {
                    out.push(dir.join("Vst3PluginScanner.xml"));
                    out.push(dir.join("Vst2xPlugin.xml"));
                }
            }
        }
        out
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Vec::new()
    }
}

fn studio_one_cache_files() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut out: Vec<PathBuf> = Vec::new();
        if let Some(app) = dirs::config_dir() {
            let presonus = app.join("PreSonus");
            // Studio One 4 .. 8 have the same plugindb.xml layout.
            for v in 4..=8 {
                let dir = presonus.join(format!("Studio One {v}")).join("Plug-In Database");
                if !dir.exists() {
                    continue;
                }
                out.push(dir.join("plugindb.xml"));
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for e in entries.flatten() {
                        let p = e.path();
                        if p.extension().and_then(|s| s.to_str()) == Some("xml") {
                            out.push(p);
                        }
                    }
                }
            }
        }
        out
    }

    #[cfg(target_os = "macos")]
    {
        let mut out: Vec<PathBuf> = Vec::new();
        if let Some(home) = dirs::home_dir() {
            let presonus = home.join("Library").join("Application Support").join("PreSonus Software");
            for v in 4..=8 {
                let dir = presonus.join(format!("Studio One {v}")).join("Plug-In Database");
                if dir.exists() {
                    out.push(dir.join("plugindb.xml"));
                }
            }
        }
        out
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Vec::new()
    }
}

fn reaper_cache_files() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut out: Vec<PathBuf> = Vec::new();
        if let Some(app) = dirs::config_dir() {
            let dir = app.join("REAPER");
            if dir.exists() {
                out.push(dir.join("reaper-vstplugins64.ini"));
                out.push(dir.join("reaper-vstplugins.ini"));
                out.push(dir.join("reaper-vst3plugins64.ini"));
                out.push(dir.join("reaper-vst3plugins.ini"));
            }
        }
        out
    }

    #[cfg(target_os = "macos")]
    {
        let mut out: Vec<PathBuf> = Vec::new();
        if let Some(home) = dirs::home_dir() {
            let dir = home.join("Library").join("Application Support").join("REAPER");
            if dir.exists() {
                out.push(dir.join("reaper-vstplugins64.ini"));
                out.push(dir.join("reaper-vst3plugins64.ini"));
            }
        }
        out
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Vec::new()
    }
}

fn live_cache_files() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut out: Vec<PathBuf> = Vec::new();
        if let Some(app) = dirs::config_dir() {
            let ableton = app.join("Ableton");
            if !ableton.exists() {
                return out;
            }
            // Ableton folders look like "Live 11.3.10"
            if let Ok(entries) = std::fs::read_dir(&ableton) {
                for e in entries.flatten() {
                    let name = e.file_name();
                    let name = name.to_string_lossy();
                    if !name.starts_with("Live ") {
                        continue;
                    }
                    let db_dir = e.path().join("Database");
                    if !db_dir.exists() {
                        continue;
                    }
                    if let Ok(inner) = std::fs::read_dir(&db_dir) {
                        for f in inner.flatten() {
                            let p = f.path();
                            // Live uses .cfg / .db files in the Database dir
                            // for plugin scan caches.
                            if p.extension()
                                .and_then(|s| s.to_str())
                                .map(|s| s == "cfg" || s == "db")
                                .unwrap_or(false)
                            {
                                out.push(p);
                            }
                        }
                    }
                }
            }
        }
        out
    }

    #[cfg(target_os = "macos")]
    {
        let mut out: Vec<PathBuf> = Vec::new();
        if let Some(home) = dirs::home_dir() {
            let ableton = home.join("Library").join("Application Support").join("Ableton");
            if !ableton.exists() {
                return out;
            }
            if let Ok(entries) = std::fs::read_dir(&ableton) {
                for e in entries.flatten() {
                    let name = e.file_name();
                    let name = name.to_string_lossy();
                    if !name.starts_with("Live ") {
                        continue;
                    }
                    let db_dir = e.path().join("Database");
                    if !db_dir.exists() {
                        continue;
                    }
                    if let Ok(inner) = std::fs::read_dir(&db_dir) {
                        for f in inner.flatten() {
                            let p = f.path();
                            if p.extension()
                                .and_then(|s| s.to_str())
                                .map(|s| s == "cfg" || s == "db")
                                .unwrap_or(false)
                            {
                                out.push(p);
                            }
                        }
                    }
                }
            }
        }
        out
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Vec::new()
    }
}
