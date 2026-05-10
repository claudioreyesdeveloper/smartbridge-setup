//! Microsoft Visual C++ 2015–2022 Redistributable (x64) detection + install.
//!
//! Why this exists: SmartBridge is built with MSVC and links the dynamic
//! C/C++ runtime (MSVCP140.dll, VCRUNTIME140.dll, VCRUNTIME140_1.dll).
//! Customers without the redistributable installed get an opaque
//! "entry point not found" failure when their DAW tries to load the .vst3,
//! and the host blacklists the plugin without a useful error message.
//!
//! Detection reads `HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\X64`
//! via `reg query` (we deliberately avoid the `winreg` crate to keep
//! dependencies small). `Installed=1` and `Major>=14` is what we want.

use super::{FixAction, FixOutcome, PreflightCheck};

const REDIST_URL: &str = "https://aka.ms/vs/17/release/vc_redist.x64.exe";

pub async fn check() -> PreflightCheck {
    #[cfg(not(target_os = "windows"))]
    {
        return PreflightCheck::skipped(
            "windows.vcredist",
            "Visual C++ Runtime (Windows only)",
            "Not applicable on this OS.",
        );
    }

    #[cfg(target_os = "windows")]
    {
        match read_vcredist_version() {
            Some((major, minor, build)) if major >= 14 => PreflightCheck::pass(
                "windows.vcredist",
                "Visual C++ Runtime",
                format!("Microsoft Visual C++ Redistributable {major}.{minor}.{build} (x64) is installed."),
            )
            .with_detail(format!("registry: HKLM\\SOFTWARE\\Microsoft\\VisualStudio\\14.0\\VC\\Runtimes\\X64 Major={major} Minor={minor} Bld={build}")),

            Some((major, minor, build)) => PreflightCheck::fail(
                "windows.vcredist",
                "Visual C++ Runtime",
                format!("Microsoft Visual C++ Redistributable {major}.{minor}.{build} is installed but it is too old. SmartBridge needs the 2015-2022 (v14+) build. Click 'Install VC++ runtime' to get the latest."),
            )
            .with_fix(FixAction::InstallVcRedist),

            None => PreflightCheck::fail(
                "windows.vcredist",
                "Visual C++ Runtime",
                "The Microsoft Visual C++ 2015-2022 Redistributable (x64) is not installed on this PC. Without it, every DAW will silently fail to load SmartBridge and add it to the blacklist. Click 'Install VC++ runtime' to fix this in one step.",
            )
            .with_fix(FixAction::InstallVcRedist),
        }
    }
}

/// Apply the InstallVcRedist fix: download Microsoft's installer and run
/// it silently. Thin wrapper around [`ensure_installed`] that maps the
/// reusable result into the [`FixOutcome`] shape the preflight UI expects.
pub async fn install() -> FixOutcome {
    let mut messages = Vec::new();
    match ensure_installed(&mut messages).await {
        Ok(()) => FixOutcome::ok(FixAction::InstallVcRedist, messages),
        Err(e) => {
            messages.push(e);
            FixOutcome::err(FixAction::InstallVcRedist, messages)
        }
    }
}

/// Make sure the Microsoft Visual C++ 2015–2022 Redistributable (x64) is
/// present on this machine. No-op on non-Windows. No-op on Windows when
/// the registry already shows version 14+ installed. Otherwise downloads
/// `vc_redist.x64.exe` from `aka.ms/vs/17/release/vc_redist.x64.exe` and
/// runs it `/install /quiet /norestart`.
///
/// Used by:
///   * the Preflight tab's `InstallVcRedist` one-click fix,
///   * the main-app install step on Windows (auto-bootstrapped before
///     placing files, so the customer never sees a "missing DLL" failure
///     when their DAW first scans `SmartBridge.vst3`).
///
/// Appends human-readable progress lines to `messages`. On error returns
/// `Err(reason)` with a single line describing what failed; the caller
/// is expected to surface that to the user.
pub async fn ensure_installed(messages: &mut Vec<String>) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = messages;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        if let Some((major, minor, build)) = read_vcredist_version() {
            if major >= 14 {
                tracing::info!(
                    target: "preflight.vcredist",
                    major, minor, build,
                    "VC++ Redistributable already installed; skipping",
                );
                messages.push(format!(
                    "VC++ Redistributable {major}.x already installed — skipped."
                ));
                return Ok(());
            }
            tracing::warn!(
                target: "preflight.vcredist",
                major, minor, build,
                "VC++ Redistributable too old; will install latest",
            );
        } else {
            tracing::warn!(
                target: "preflight.vcredist",
                "VC++ Redistributable not detected; will install latest",
            );
        }

        messages.push("VC++ Redistributable missing or out of date — downloading…".into());
        tracing::info!(target: "preflight.vcredist", url = REDIST_URL, "downloading vc_redist.x64.exe");

        let temp = std::env::temp_dir().join("vc_redist_x64.exe");
        let bytes = reqwest::get(REDIST_URL)
            .await
            .map_err(|e| {
                tracing::error!(target: "preflight.vcredist", error = %e, "download failed");
                format!("Could not download VC++ Redistributable: {e}")
            })?
            .bytes()
            .await
            .map_err(|e| {
                tracing::error!(target: "preflight.vcredist", error = %e, "read body failed");
                format!("Could not download VC++ Redistributable: {e}")
            })?;

        std::fs::write(&temp, &bytes).map_err(|e| {
            tracing::error!(target: "preflight.vcredist", path = %temp.display(), error = %e, "write installer failed");
            format!("Could not save VC++ installer to disk: {e}")
        })?;

        tracing::info!(target: "preflight.vcredist", bytes = bytes.len(), path = %temp.display(), "running silent install");
        messages.push(format!(
            "Downloaded vc_redist.x64.exe ({} bytes); running silent install…",
            bytes.len()
        ));

        // /install /quiet /norestart is the documented Microsoft silent
        // install. Returns 0 on success, 1638 if a newer version is
        // already installed (also fine), 3010 if reboot is required.
        let status = std::process::Command::new(&temp)
            .args(["/install", "/quiet", "/norestart"])
            .status()
            .map_err(|e| {
                tracing::error!(target: "preflight.vcredist", error = %e, "spawn installer failed");
                format!("Could not launch vc_redist.x64.exe: {e}")
            })?;

        let code = status.code().unwrap_or(-1);
        tracing::info!(target: "preflight.vcredist", code, "vc_redist.x64.exe finished");
        match code {
            0 | 1638 => {
                messages.push("Visual C++ Redistributable installed successfully.".into());
                Ok(())
            }
            3010 => {
                messages.push(
                    "Visual C++ Redistributable installed; Windows requests a restart.".into(),
                );
                Ok(())
            }
            _ => {
                tracing::error!(target: "preflight.vcredist", code, "vc_redist.x64.exe failed");
                Err(format!(
                    "vc_redist.x64.exe returned exit code {code}. The customer may need to run it manually from {REDIST_URL}."
                ))
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn read_vcredist_version() -> Option<(u32, u32, u32)> {
    let out = std::process::Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\X64",
            "/v",
            "Installed",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    if !stdout.lines().any(|l| {
        l.contains("Installed")
            && l.split_whitespace().last().map(|v| v == "0x1").unwrap_or(false)
    }) {
        return None;
    }

    let major = read_reg_dword(r"HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\X64", "Major")?;
    let minor = read_reg_dword(r"HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\X64", "Minor").unwrap_or(0);
    let bld = read_reg_dword(r"HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\X64", "Bld").unwrap_or(0);
    Some((major, minor, bld))
}

#[cfg(target_os = "windows")]
fn read_reg_dword(key: &str, value: &str) -> Option<u32> {
    let out = std::process::Command::new("reg")
        .args(["query", key, "/v", value])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if !line.starts_with(value) {
            continue;
        }
        // "ValueName    REG_DWORD    0xNNN"
        let token = line.split_whitespace().last()?;
        let parsed = if let Some(hex) = token.strip_prefix("0x") {
            u32::from_str_radix(hex, 16).ok()
        } else {
            token.parse::<u32>().ok()
        };
        if let Some(v) = parsed {
            return Some(v);
        }
    }
    None
}
