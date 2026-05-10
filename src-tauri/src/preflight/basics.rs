//! Cheap, high-signal checks: OS arch, encrypted DB, VST3 bundle layout.

use super::{FixAction, PreflightCheck};
use std::path::PathBuf;

/// Step 1: confirm we're on a 64-bit Windows / macOS host. SmartBridge
/// is x64-only; legacy 32-bit hosts (FL Studio 32, Cubase 32) cannot
/// load the plugin and would mark it as missing in their scanner.
pub async fn check_os_architecture() -> PreflightCheck {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let label = "Operating system";

    let supported_os = matches!(os, "windows" | "macos");
    let supported_arch = matches!(arch, "x86_64" | "aarch64");

    if supported_os && supported_arch {
        PreflightCheck::pass(
            "os.architecture",
            label,
            format!(
                "Detected {os} on {arch}. SmartBridge ships a native 64-bit build for this combination."
            ),
        )
        .with_detail(format!("os = {os}"))
        .with_detail(format!("arch = {arch}"))
    } else if supported_os {
        PreflightCheck::fail(
            "os.architecture",
            label,
            format!("Detected {os} on {arch}, which is not a supported architecture. SmartBridge only ships an x86_64 build for Windows and a Universal binary for macOS."),
        )
    } else {
        PreflightCheck::fail(
            "os.architecture",
            label,
            format!("Detected operating system '{os}' is not supported. SmartBridge ships only macOS and Windows builds."),
        )
    }
}

/// Step 2: confirm the encrypted SmartBridge database is present at the
/// expected per-user location and is non-empty. This catches the
/// classic "installer never ran / user moved the file out".
pub async fn check_database() -> PreflightCheck {
    let db = match crate::paths::user_database_path() {
        Some(p) => p,
        None => {
            return PreflightCheck::fail(
                "smartbridge.database",
                "Encrypted database",
                "Could not work out where the SmartBridge user data folder lives on this OS. This is a SmartBridge Setup bug, please send the diagnostics report.",
            );
        }
    };

    if !db.exists() {
        return PreflightCheck::fail(
            "smartbridge.database",
            "Encrypted database",
            "The SmartBridge database is not installed yet. Click 'Reinstall database' to copy it back into place — this is the file SmartBridge reads every voice, MIDI clip and lyric template from.",
        )
        .with_detail(format!("expected at: {}", db.display()))
        .with_fix(FixAction::ReinstallDatabase);
    }

    let meta = match std::fs::metadata(&db) {
        Ok(m) => m,
        Err(e) => {
            return PreflightCheck::fail(
                "smartbridge.database",
                "Encrypted database",
                format!("The database file exists but cannot be read: {e}. Antivirus or file permissions may be blocking it."),
            )
            .with_detail(format!("path: {}", db.display()))
            .with_fix(FixAction::OpenAvSettings);
        }
    };

    let size = meta.len();
    if size < 64 * 1024 {
        return PreflightCheck::fail(
            "smartbridge.database",
            "Encrypted database",
            format!("The database file is suspiciously small ({size} bytes). It is probably corrupt or only half-downloaded. Click 'Reinstall database' to replace it."),
        )
        .with_detail(format!("path: {}", db.display()))
        .with_fix(FixAction::ReinstallDatabase);
    }

    PreflightCheck::pass(
        "smartbridge.database",
        "Encrypted database",
        format!("Database is in place ({} MB)", size / (1024 * 1024)),
    )
    .with_detail(format!("path: {}", db.display()))
    .with_detail(format!("size_bytes: {size}"))
}

/// Step 3: confirm at least one of the well-known VST3 install locations
/// contains a properly-formed SmartBridge bundle.
///
/// A real Windows VST3 must be a folder named `SmartBridge.vst3` containing
/// `Contents\x86_64-win\SmartBridge.vst3` (the actual DLL). A flat .dll
/// renamed to .vst3 is a common Tauri/installer mistake and modern hosts
/// (Cubase 12+, Live 11+, Studio One 6+) silently refuse it.
///
/// On macOS the layout is `SmartBridge.vst3/Contents/MacOS/SmartBridge`.
pub async fn check_vst3_bundle_layout() -> PreflightCheck {
    let candidates = vst3_candidate_paths();
    if candidates.is_empty() {
        return PreflightCheck::skipped(
            "vst3.bundle_layout",
            "VST3 bundle layout",
            "VST3 layout check is not implemented for this OS yet.",
        );
    }

    let mut found_any = false;
    let mut details: Vec<String> = Vec::new();
    let mut malformed: Vec<PathBuf> = Vec::new();
    let mut found_well_formed: Option<PathBuf> = None;

    for c in &candidates {
        if !c.exists() {
            details.push(format!("not present: {}", c.display()));
            continue;
        }
        found_any = true;

        // A correct VST3 is always a directory bundle.
        if !c.is_dir() {
            malformed.push(c.clone());
            details.push(format!("malformed (flat file, not a folder): {}", c.display()));
            continue;
        }

        let inner = inner_binary_path(c);
        if inner.exists() && inner.is_file() {
            found_well_formed = Some(c.clone());
            details.push(format!("ok: {}", c.display()));
            details.push(format!("inner binary: {}", inner.display()));
        } else {
            malformed.push(c.clone());
            details.push(format!(
                "missing inner binary at {}",
                inner.display()
            ));
        }
    }

    if !found_any {
        return PreflightCheck::fail(
            "vst3.bundle_layout",
            "VST3 bundle layout",
            "SmartBridge.vst3 is not installed in any of the standard plugin folders. Click 'Reinstall plugin' to lay it down properly.",
        )
        .with_details(details)
        .with_fix(FixAction::ReinstallPlugin);
    }

    if found_well_formed.is_some() && malformed.is_empty() {
        return PreflightCheck::pass(
            "vst3.bundle_layout",
            "VST3 bundle layout",
            "SmartBridge.vst3 is installed in the standard plugin folder with the correct internal layout.",
        )
        .with_details(details);
    }

    if found_well_formed.is_some() && !malformed.is_empty() {
        // One copy is fine, another is wrong. Worth flagging — many DAWs
        // scan multiple folders and one bad copy can cause a blacklist
        // even when the good copy exists.
        return PreflightCheck::warn(
            "vst3.bundle_layout",
            "VST3 bundle layout",
            "Found a working SmartBridge.vst3, but one of the other plugin folders contains a broken copy. Some DAWs scan every folder and may blacklist the broken one. Click 'Reinstall plugin' to overwrite the broken copies.",
        )
        .with_details(details)
        .with_fix(FixAction::ReinstallPlugin);
    }

    PreflightCheck::fail(
        "vst3.bundle_layout",
        "VST3 bundle layout",
        "SmartBridge.vst3 is present but the bundle layout is wrong — DAWs will refuse it. The .vst3 must be a folder containing Contents/x86_64-win/SmartBridge.vst3 (Windows) or Contents/MacOS/SmartBridge (macOS). Click 'Reinstall plugin' to fix.",
    )
    .with_details(details)
    .with_fix(FixAction::ReinstallPlugin)
}

/// Public helper used by other preflight modules to find the inner DLL
/// they need to inspect.
pub fn first_well_formed_vst3_inner() -> Option<PathBuf> {
    for c in vst3_candidate_paths() {
        if c.is_dir() {
            let inner = inner_binary_path(&c);
            if inner.is_file() {
                return Some(inner);
            }
        }
    }
    None
}

/// Public helper used by the DLL walk + self-test modules.
pub fn standalone_executable_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let candidates = [
            std::env::var("ProgramFiles").ok().map(PathBuf::from),
            std::env::var("ProgramFiles(x86)").ok().map(PathBuf::from),
        ];
        for base in candidates.into_iter().flatten() {
            let exe = base.join("SmartBridge").join("SmartBridge.exe");
            if exe.exists() {
                return Some(exe);
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    {
        let p = PathBuf::from("/Applications/SmartBridge.app/Contents/MacOS/SmartBridge");
        if p.exists() { Some(p) } else { None }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        None
    }
}

fn vst3_candidate_paths() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut out: Vec<PathBuf> = Vec::new();
        if let Ok(common) = std::env::var("CommonProgramFiles") {
            out.push(PathBuf::from(common).join("VST3").join("SmartBridge.vst3"));
        } else if let Ok(pf) = std::env::var("ProgramFiles") {
            // CommonProgramFiles is *almost* always set, but fall back to
            // the canonical "C:\Program Files\Common Files\VST3" via PF.
            out.push(PathBuf::from(pf).join("Common Files").join("VST3").join("SmartBridge.vst3"));
        }
        if let Some(local) = dirs::data_local_dir() {
            // Per-user VST3 path recognised by Cubase 12+ and Studio One 6+.
            out.push(local.join("Programs").join("Common").join("VST3").join("SmartBridge.vst3"));
        }
        out
    }

    #[cfg(target_os = "macos")]
    {
        let mut out: Vec<PathBuf> = Vec::new();
        out.push(PathBuf::from("/Library/Audio/Plug-Ins/VST3/SmartBridge.vst3"));
        if let Some(home) = dirs::home_dir() {
            out.push(home.join("Library").join("Audio").join("Plug-Ins").join("VST3").join("SmartBridge.vst3"));
        }
        out
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Vec::new()
    }
}

fn inner_binary_path(bundle: &PathBuf) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        bundle
            .join("Contents")
            .join("x86_64-win")
            .join("SmartBridge.vst3")
    }

    #[cfg(target_os = "macos")]
    {
        bundle.join("Contents").join("MacOS").join("SmartBridge")
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        bundle.join("Contents").join("x86_64-linux").join("SmartBridge.so")
    }
}
