//! main-app: detects the SmartBridge.app / SmartBridge.exe.
//!
//! macOS:   `/Applications/SmartBridge.app` (Info.plist → CFBundleShortVersionString
//!          via `defaults read`, no extra crate dep)
//! Windows: per-user-first lookup. Modern installs go to
//!          `%LOCALAPPDATA%\Programs\SmartBridge\SmartBridge.exe`. The
//!          legacy NSIS installer used `%PROGRAMFILES%\SmartBridge\` —
//!          we still check there so a customer who upgrades from the
//!          old installer keeps showing as Ready until they reinstall.
//!          Version is read from the .exe's FileVersionInfo via PowerShell.
//! Linux:   not currently a SmartBridge build target. Always NotInstalled.

use super::DetectionResult;
use std::path::PathBuf;

pub async fn detect() -> DetectionResult {
    #[cfg(target_os = "macos")]
    {
        detect_macos().await
    }

    #[cfg(target_os = "windows")]
    {
        detect_windows().await
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        DetectionResult::not_installed()
            .with_detail("SmartBridge does not currently ship a Linux build.")
    }
}

#[cfg(target_os = "macos")]
async fn detect_macos() -> DetectionResult {
    let app = PathBuf::from("/Applications/SmartBridge.app");
    if !app.exists() {
        return DetectionResult::not_installed();
    }

    let plist = app.join("Contents").join("Info.plist");
    if !plist.exists() {
        return DetectionResult::needs_repair(
            "SmartBridge.app exists but Contents/Info.plist is missing — \
             the bundle is corrupt. Reinstall fixes this.",
        );
    }

    match read_info_plist_version(&plist).await {
        Ok(v) => DetectionResult::ready()
            .with_version(v)
            .with_detail(format!("found at {}", app.display())),
        Err(e) => DetectionResult::ready()
            .with_detail(format!("found at {}", app.display()))
            .with_detail(format!("could not read version: {e}")),
    }
}

#[cfg(target_os = "macos")]
async fn read_info_plist_version(plist: &std::path::Path) -> Result<String, String> {
    let key = "CFBundleShortVersionString";

    // `defaults read <plist-without-extension> <key>` is the standard Apple
    // way to read either binary or XML plists without a third-party crate.
    let plist_noext = plist.with_extension("");

    let out = tokio::process::Command::new("defaults")
        .arg("read")
        .arg(plist_noext)
        .arg(key)
        .output()
        .await
        .map_err(|e| format!("spawn defaults failed: {e}"))?;

    if !out.status.success() {
        return Err(format!(
            "defaults read failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(target_os = "windows")]
async fn detect_windows() -> DetectionResult {
    let candidates = candidate_exe_paths();

    for exe in &candidates {
        if exe.exists() {
            let mut det = DetectionResult::ready()
                .with_detail(format!("found at {}", exe.display()));
            if let Some(v) = read_exe_version(exe).await {
                det = det.with_version(v);
            }
            return det;
        }
    }

    DetectionResult::not_installed()
}

#[cfg(target_os = "windows")]
fn candidate_exe_paths() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(per_user) = crate::paths::windows_main_app_dir() {
        out.push(per_user.join("SmartBridge.exe"));
    }
    for env_var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(base) = std::env::var(env_var) {
            out.push(PathBuf::from(base).join("SmartBridge").join("SmartBridge.exe"));
        }
    }
    out
}

#[cfg(target_os = "windows")]
async fn read_exe_version(exe: &std::path::Path) -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let exe_str = exe.to_string_lossy().to_string();
    let escaped = exe_str.replace('\'', "''");
    let script = format!(
        "(Get-Item -LiteralPath '{escaped}').VersionInfo.ProductVersion"
    );

    let out = tokio::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .await
        .ok()?;

    if !out.status.success() {
        return None;
    }
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}
