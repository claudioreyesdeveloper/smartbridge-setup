//! cubase-connection: detects the MIDI Remote driver script and the
//! project template at the canonical Steinberg locations used by the
//! existing native installers.
//!
//! MIDI Remote script (cross-version; folder is intentionally version-
//! agnostic per Steinberg's MIDI Remote API design):
//!   macOS:   ~/Documents/Steinberg/Cubase/MIDI Remote/Driver Scripts/Local/SmartBridge/GenosSlotRename/SmartBridge_GenosSlotRename.js
//!   Windows: %USERPROFILE%\Documents\Steinberg\Cubase\MIDI Remote\Driver Scripts\Local\SmartBridge\GenosSlotRename\SmartBridge_GenosSlotRename.js
//!
//! Project template (per Cubase major version; we look at 14 and 15 today;
//! when 16 ships, add it here):
//!   macOS:   ~/Library/Preferences/Cubase {VER}/Project Templates/SmartBridge.cpr
//!   Windows: %APPDATA%\Steinberg\Cubase {VER}\Project Templates\SmartBridge.cpr
//!
//! Status logic:
//!   * No Cubase folder found at all → NotInstalled with "Cubase not detected".
//!   * Cubase folder found, all expected files present → Ready.
//!   * Cubase folder found, some files missing → NeedsRepair with a
//!     specific list of what's missing.

use super::DetectionResult;
use std::path::PathBuf;

const SUPPORTED_CUBASE_VERSIONS: &[u32] = &[14, 15];

pub async fn detect() -> DetectionResult {
    let docs = match dirs::document_dir() {
        Some(d) => d,
        None => {
            return DetectionResult::error("could not locate user Documents folder")
        }
    };

    let cubase_root = docs.join("Steinberg").join("Cubase");
    let any_cubase_signal = cubase_root.exists() || installed_template_dirs_exist();

    let midi_remote_script = docs
        .join("Steinberg")
        .join("Cubase")
        .join("MIDI Remote")
        .join("Driver Scripts")
        .join("Local")
        .join("SmartBridge")
        .join("GenosSlotRename")
        .join("SmartBridge_GenosSlotRename.js");

    let mut missing: Vec<String> = Vec::new();
    let mut present: Vec<String> = Vec::new();

    if midi_remote_script.exists() {
        present.push(format!("MIDI Remote script: {}", midi_remote_script.display()));
    } else if any_cubase_signal {
        missing.push(format!(
            "MIDI Remote script not found at {}",
            midi_remote_script.display()
        ));
    }

    let mut detected_template_versions: Vec<u32> = Vec::new();
    let mut missing_template_versions: Vec<u32> = Vec::new();

    for &ver in SUPPORTED_CUBASE_VERSIONS {
        let prefs_dir = cubase_prefs_dir(ver);
        if !prefs_dir.exists() {
            continue;
        }
        let template = prefs_dir.join("Project Templates").join("SmartBridge.cpr");
        if template.exists() {
            present.push(format!("Cubase {ver} template: {}", template.display()));
            detected_template_versions.push(ver);
        } else {
            missing.push(format!(
                "Cubase {ver} template missing at {}",
                template.display()
            ));
            missing_template_versions.push(ver);
        }
    }

    if !any_cubase_signal && detected_template_versions.is_empty() {
        return DetectionResult::not_installed()
            .with_detail("Cubase 14 or 15 not detected on this machine.");
    }

    if missing.is_empty() && !present.is_empty() {
        let mut r = DetectionResult::ready();
        for p in present {
            r = r.with_detail(p);
        }
        return r;
    }

    if missing.is_empty() && present.is_empty() {
        return DetectionResult::not_installed()
            .with_detail("Cubase folders exist but no SmartBridge files placed yet.");
    }

    let mut r = DetectionResult::needs_repair(format!(
        "missing {} item(s)",
        missing.len()
    ));
    for m in missing {
        r = r.with_detail(m);
    }
    for p in present {
        r = r.with_detail(p);
    }
    r
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
            .map(|c| c.join("Steinberg").join(format!("Cubase {version}")))
            .unwrap_or_default()
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = version;
        PathBuf::new()
    }
}

fn installed_template_dirs_exist() -> bool {
    SUPPORTED_CUBASE_VERSIONS
        .iter()
        .any(|&v| cubase_prefs_dir(v).exists())
}
