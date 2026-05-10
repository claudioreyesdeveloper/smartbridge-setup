//! Preflight: deep validation that SmartBridge is actually loadable in DAWs.
//!
//! Where the [`crate::detection`] module just answers "is the file there?",
//! Preflight answers "will Cubase / Studio One / Reaper actually load it?".
//! This is the layer that catches the silent failures that lead to a host
//! marking SmartBridge as blacklisted (missing DLLs, broken VST3 bundle
//! layout, missing VC++ Redistributable, antivirus quarantine, stale
//! plugin caches in the host).
//!
//! Each check returns a [`PreflightCheck`] with a status, a plain-language
//! explanation, and (optionally) a [`FixAction`] the frontend can offer
//! the customer as a one-click button.
//!
//! Cross-platform: most of the meaningful work is Windows-only, but every
//! check is callable on every platform — non-applicable checks return
//! [`PreflightStatus::Skipped`] so the structure of the dashboard stays
//! identical and we can layer macOS-specific checks on top later.

#![allow(dead_code)]

pub mod basics;
pub mod dll_walk;
pub mod host_caches;
pub mod report;
pub mod selftest;
pub mod vcredist;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreflightStatus {
    /// Check ran and found everything is fine.
    Pass,
    /// Check ran and found a non-blocking issue (e.g. unsigned binary,
    /// optional dependency missing). DAW will still load SmartBridge.
    Warn,
    /// Check ran and found a hard problem that will cause the host to
    /// blacklist SmartBridge. Customer should run the Fix action.
    Fail,
    /// Check is not applicable on this platform (most Windows-specific
    /// checks return Skipped on macOS) or could not run for an
    /// environmental reason (e.g. plugin not installed yet).
    Skipped,
}

/// One-click remediation the frontend can offer the customer. Mapped to a
/// Tauri command (`preflight_fix`) so the same Rust code does both the
/// detection and the fix.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FixAction {
    /// Nothing to fix automatically.
    None,
    /// Download and silently install the latest Microsoft VC++
    /// Redistributable (x64).
    InstallVcRedist,
    /// Re-run the main-app installer (this re-deploys the .vst3 bundle
    /// + bundled DLLs).
    ReinstallPlugin,
    /// Re-deploy the encrypted database to %APPDATA%\SmartBridge.
    ReinstallDatabase,
    /// Delete the named DAW's plugin cache so it forces a clean rescan
    /// next time it launches.
    ClearHostCache { host_id: String },
    /// Open Windows Defender / antivirus exclusion settings (just opens
    /// the relevant Settings app page; we never touch the AV ourselves).
    OpenAvSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightCheck {
    pub id: String,
    pub label: String,
    pub status: PreflightStatus,
    /// Free-form, plain-language explanation. Always safe to show to a
    /// non-technical customer; never contains absolute paths that would
    /// leak identity beyond what's already on the host.
    pub explanation: String,
    /// Extra technical lines for the Diagnostics report (raw paths,
    /// version numbers, missing DLL names). Hidden behind a "Show
    /// details" disclosure in the UI.
    pub details: Vec<String>,
    /// One-click fix the frontend can offer. `FixAction::None` if this
    /// check is informational only.
    pub fix: FixAction,
}

impl PreflightCheck {
    pub fn pass(id: impl Into<String>, label: impl Into<String>, explanation: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status: PreflightStatus::Pass,
            explanation: explanation.into(),
            details: Vec::new(),
            fix: FixAction::None,
        }
    }

    pub fn warn(id: impl Into<String>, label: impl Into<String>, explanation: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status: PreflightStatus::Warn,
            explanation: explanation.into(),
            details: Vec::new(),
            fix: FixAction::None,
        }
    }

    pub fn fail(id: impl Into<String>, label: impl Into<String>, explanation: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status: PreflightStatus::Fail,
            explanation: explanation.into(),
            details: Vec::new(),
            fix: FixAction::None,
        }
    }

    pub fn skipped(id: impl Into<String>, label: impl Into<String>, explanation: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status: PreflightStatus::Skipped,
            explanation: explanation.into(),
            details: Vec::new(),
            fix: FixAction::None,
        }
    }

    pub fn with_detail(mut self, line: impl Into<String>) -> Self {
        self.details.push(line.into());
        self
    }

    pub fn with_details<I, S>(mut self, lines: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.details.extend(lines.into_iter().map(Into::into));
        self
    }

    pub fn with_fix(mut self, fix: FixAction) -> Self {
        self.fix = fix;
        self
    }
}

/// Outcome of running a [`FixAction`]. Same shape as [`crate::install::InstallOutcome`]
/// so the frontend can render it with the existing banner component.
#[derive(Debug, Clone, Serialize)]
pub struct FixOutcome {
    pub fix: FixAction,
    pub success: bool,
    pub messages: Vec<String>,
}

impl FixOutcome {
    pub fn ok(fix: FixAction, messages: Vec<String>) -> Self {
        Self { fix, success: true, messages }
    }
    pub fn err(fix: FixAction, messages: Vec<String>) -> Self {
        Self { fix, success: false, messages }
    }
}

/// All the preflight check IDs in display order. Stable strings so the
/// frontend can call `run_preflight_one(id)` to re-run a single check
/// after a fix.
pub fn all_check_ids() -> Vec<&'static str> {
    vec![
        // basics
        "os.architecture",
        "smartbridge.database",
        "vst3.bundle_layout",
        // VC++ runtime
        "windows.vcredist",
        // DLL imports
        "vst3.dll_imports",
        "standalone.dll_imports",
        // Self-test
        "standalone.selftest",
        // Host plugin caches
        "host.cubase_cache",
        "host.studio_one_cache",
        "host.reaper_cache",
        "host.live_cache",
    ]
}

/// Run every check, in parallel where it makes sense. Returns checks in
/// the order [`all_check_ids`] declares.
pub async fn run_all() -> Vec<PreflightCheck> {
    let (
        arch,
        db,
        bundle,
        vcr,
        vst_imports,
        exe_imports,
        selft,
        cubase,
        studio_one,
        reaper,
        live,
    ) = tokio::join!(
        basics::check_os_architecture(),
        basics::check_database(),
        basics::check_vst3_bundle_layout(),
        vcredist::check(),
        dll_walk::check_vst3_imports(),
        dll_walk::check_standalone_imports(),
        selftest::check(),
        host_caches::check_cubase(),
        host_caches::check_studio_one(),
        host_caches::check_reaper(),
        host_caches::check_live(),
    );

    vec![arch, db, bundle, vcr, vst_imports, exe_imports, selft, cubase, studio_one, reaper, live]
}

/// Re-run a single check by id.
pub async fn run_one(id: &str) -> Option<PreflightCheck> {
    Some(match id {
        "os.architecture" => basics::check_os_architecture().await,
        "smartbridge.database" => basics::check_database().await,
        "vst3.bundle_layout" => basics::check_vst3_bundle_layout().await,
        "windows.vcredist" => vcredist::check().await,
        "vst3.dll_imports" => dll_walk::check_vst3_imports().await,
        "standalone.dll_imports" => dll_walk::check_standalone_imports().await,
        "standalone.selftest" => selftest::check().await,
        "host.cubase_cache" => host_caches::check_cubase().await,
        "host.studio_one_cache" => host_caches::check_studio_one().await,
        "host.reaper_cache" => host_caches::check_reaper().await,
        "host.live_cache" => host_caches::check_live().await,
        _ => return None,
    })
}

/// Apply a one-click fix. The caller is expected to re-run the relevant
/// check afterwards and refresh the UI.
pub async fn apply_fix(app: &AppHandle, fix: FixAction) -> FixOutcome {
    match &fix {
        FixAction::None => FixOutcome::ok(fix, vec!["Nothing to fix.".into()]),
        FixAction::InstallVcRedist => vcredist::install().await,
        FixAction::ReinstallPlugin => {
            // Re-run the existing main-app install path. We can't import
            // the manifest here (no AppHandle in scope for sync code), so
            // delegate to the existing pipeline.
            let manifest = match crate::manifest::fetch_with_fallback().await {
                Ok(m) => m,
                Err(e) => {
                    return FixOutcome::err(fix, vec![format!("Could not fetch release manifest: {e}")]);
                }
            };
            let outcome = crate::install::install(app, &manifest.manifest, "main-app").await;
            if outcome.success {
                FixOutcome::ok(fix, outcome.messages)
            } else {
                FixOutcome::err(fix, outcome.messages)
            }
        }
        FixAction::ReinstallDatabase => {
            let manifest = match crate::manifest::fetch_with_fallback().await {
                Ok(m) => m,
                Err(e) => {
                    return FixOutcome::err(fix, vec![format!("Could not fetch release manifest: {e}")]);
                }
            };
            // The current resources component places config.json; the
            // database itself is shipped inside the main-app installer.
            // Pretend the user clicked "main-app" since that is what
            // re-seeds the encrypted DB on Windows.
            let outcome = crate::install::install(app, &manifest.manifest, "main-app").await;
            if outcome.success {
                FixOutcome::ok(fix, outcome.messages)
            } else {
                FixOutcome::err(fix, outcome.messages)
            }
        }
        FixAction::ClearHostCache { host_id } => {
            host_caches::clear(host_id).await
        }
        FixAction::OpenAvSettings => {
            #[cfg(target_os = "windows")]
            {
                match std::process::Command::new("cmd")
                    .args(["/c", "start", "", "windowsdefender:"])
                    .spawn()
                {
                    Ok(_) => FixOutcome::ok(fix, vec!["Opened Windows Security so you can add an exclusion for SmartBridge.".into()]),
                    Err(e) => FixOutcome::err(fix, vec![format!("Could not open Windows Security: {e}")]),
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                FixOutcome::err(fix, vec!["Antivirus settings shortcut is Windows-only.".into()])
            }
        }
    }
}
