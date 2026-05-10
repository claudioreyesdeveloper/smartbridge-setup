//! Tauri IPC commands exposed to the Svelte frontend.
//!
//! Static per-component metadata (display name, plain-language blurb,
//! required/optional, default action label) lives in [`COMPONENT_META`].
//! Live state (`state`, `installed_version`, `details`) comes from the
//! detection module and is merged in at request time.

use crate::detection;
use crate::host;
use crate::install;
use crate::license;
use crate::local_repo;
use crate::manifest;
use crate::preflight;
use std::path::PathBuf;

use serde::Serialize;
use tauri::AppHandle;

/// User-facing component status. Maps 1:1 onto a card colour in the UI.
/// `Unknown` is used by the frontend before the first detection completes;
/// `UpdateAvailable` is constructed by Phase 4 manifest comparison logic.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentState {
    /// Card greyed out — feature is not built into this binary.
    NotAvailableInBuild,
    /// Detection has not run yet.
    Unknown,
    /// Component is installed and verified working.
    Ready,
    /// Component is installed but a newer version exists.
    UpdateAvailable,
    /// Component is not installed; user can install it.
    NotInstalled,
    /// Component was installed but something is wrong (missing file,
    /// failed checksum, missing dependency).
    NeedsRepair,
    /// Detection or install failed in a way the user can act on.
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentCard {
    pub component_id: &'static str,
    pub display_name: &'static str,
    pub plain_language: &'static str,
    pub required: bool,
    pub optional: bool,
    pub state: ComponentState,
    pub installed_version: Option<String>,
    pub available_version: Option<String>,
    pub action_label: &'static str,
    /// Diagnostic detail lines (shown only on the Diagnostics tab).
    pub details: Vec<String>,
}

/// Static per-component metadata. Order matters — this is the order cards
/// appear on the dashboard.
struct ComponentMeta {
    id: &'static str,
    display_name: &'static str,
    plain_language: &'static str,
    required: bool,
    optional: bool,
    action_label: &'static str,
}

const COMPONENT_META: &[ComponentMeta] = &[
    ComponentMeta {
        id: "main-app",
        display_name: "Main app",
        plain_language: "The SmartBridge application itself.",
        required: true,
        optional: false,
        action_label: "Install",
    },
    ComponentMeta {
        id: "cubase-connection",
        display_name: "Cubase connection",
        plain_language:
            "Lets SmartBridge talk to Cubase: installs the MIDI Remote script and the project template.",
        required: false,
        optional: false,
        action_label: "Install",
    },
    ComponentMeta {
        id: "ai-lyrics",
        display_name: "AI Lyrics",
        plain_language:
            "Local lyric generation using a small AI model. Optional. Needs Ollama installed.",
        required: false,
        optional: true,
        action_label: "Install",
    },
    ComponentMeta {
        id: "ai-lyrics-default-model",
        display_name: "AI Lyrics default model",
        plain_language:
            "The single Llama-backend default model (Gemma 4 E4B, ~9.6 GB). Used when Optimized routing is off. Optional. Needs Ollama installed.",
        required: false,
        optional: true,
        action_label: "Install",
    },
    ComponentMeta {
        id: "synthv-connection",
        display_name: "Synthesizer V connection",
        plain_language: "Optional side-panel script for Synthesizer V Studio.",
        required: false,
        optional: true,
        action_label: "Install",
    },
    ComponentMeta {
        id: "smartbridge-resources",
        display_name: "SmartBridge resources",
        plain_language:
            "First-run defaults the app needs (configuration, templates).",
        required: false,
        optional: false,
        action_label: "Install",
    },
    ComponentMeta {
        id: "help-files",
        display_name: "Help files",
        plain_language:
            "Getting-started PDF, installation guide, and the interactive manual.",
        required: false,
        optional: true,
        action_label: "Install",
    },
    ComponentMeta {
        id: "windows-loopmidi",
        display_name: "loopMIDI virtual ports (Windows)",
        plain_language:
            "Creates the two virtual MIDI ports SmartBridge uses to rename Cubase tracks. \
             Free download from Tobias Erichsen — installed silently and configured to start \
             at login. Skipped on macOS (use Apple IAC ports there).",
        required: false,
        optional: true,
        action_label: "Install",
    },
    ComponentMeta {
        id: "yamaha-steinberg-driver",
        display_name: "Yamaha Steinberg USB Driver (Windows)",
        plain_language:
            "Lets Windows talk to Yamaha keyboards over USB MIDI. Installed automatically \
             with winget if it is missing.",
        required: false,
        optional: true,
        action_label: "Install",
    },
];

fn meta_for(id: &str) -> Option<&'static ComponentMeta> {
    COMPONENT_META.iter().find(|m| m.id == id)
}

fn merge(meta: &ComponentMeta, det: detection::DetectionResult) -> ComponentCard {
    ComponentCard {
        component_id: meta.id,
        display_name: meta.display_name,
        plain_language: meta.plain_language,
        required: meta.required,
        optional: meta.optional,
        state: det.state,
        installed_version: det.installed_version,
        available_version: None,
        action_label: meta.action_label,
        details: det.details,
    }
}

#[tauri::command]
pub fn installer_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn host_info() -> host::HostInfo {
    host::current()
}

#[derive(Debug, Clone, Serialize)]
pub struct ManifestUrls {
    pub url: &'static str,
}

#[tauri::command]
pub fn manifest_url() -> ManifestUrls {
    ManifestUrls {
        url: manifest::MANIFEST_URL,
    }
}

#[tauri::command]
pub async fn list_components() -> Vec<ComponentCard> {
    let results = detection::detect_all().await;
    let mut cards: Vec<ComponentCard> = Vec::with_capacity(results.len());
    for (id, det) in results {
        if let Some(meta) = meta_for(id) {
            cards.push(merge(meta, det));
        }
    }
    cards
}

#[tauri::command]
pub async fn recheck_component(component_id: String) -> Option<ComponentCard> {
    let meta = meta_for(component_id.as_str())?;
    let det = detection::detect_one(component_id.as_str()).await?;
    Some(merge(meta, det))
}

/// Fetch the release manifest, with stable→beta→cache fallback.
/// Frontend uses the result to populate `available_version` on cards and
/// to drive Phase 4 install actions.
#[tauri::command]
pub async fn fetch_manifest() -> Result<manifest::FetchedManifest, String> {
    manifest::fetch_with_fallback().await
}

/// Install (or repair) a single component. Fetches the manifest first
/// (cache-aware), dispatches to the per-component install logic, and
/// emits `download://progress` events while assets stream down.
///
/// Returns the install outcome including a fresh detection result so the
/// dashboard can update the card without a separate recheck round trip.
#[tauri::command]
pub async fn install_component(
    app: AppHandle,
    component_id: String,
) -> Result<install::InstallOutcome, String> {
    tracing::info!(target: "ipc", component = %component_id, "install_component");
    let fetched = manifest::fetch_with_fallback().await.map_err(|e| {
        tracing::error!(target: "ipc", component = %component_id, error = %e, "fetch_manifest failed");
        e
    })?;
    Ok(install::install(&app, &fetched.manifest, &component_id).await)
}

/// Inverse of [`install_component`]: remove a single component from the
/// machine. Re-detects after removal so the dashboard updates without a
/// separate recheck round trip. Never destroys user data — the dedicated
/// main-app uninstall flow has its own toggle for that.
#[tauri::command]
pub async fn remove_component(
    component_id: String,
) -> install::InstallOutcome {
    tracing::info!(target: "ipc", component = %component_id, "remove_component");
    install::remove(&component_id).await
}

#[tauri::command]
pub fn open_log_folder() -> Result<String, String> {
    let dir = crate::paths::installer_log_dir()
        .ok_or_else(|| "could not determine installer log directory".to_string())?;
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return Err(format!("could not create log directory: {e}"));
    }
    Ok(dir.to_string_lossy().to_string())
}

/// Snapshot of the offline / local-repo configuration. Surfaced in the
/// dashboard header (small badge) and in the Diagnostics tab.
#[tauri::command]
pub fn get_local_repo_status() -> local_repo::LocalRepoStatus {
    local_repo::current_status()
}

/// Configure SmartBridge Setup to read from a local offline bundle. The
/// directory must already contain `smartbridge-release-manifest.json`
/// plus the asset files referenced by it.
#[tauri::command]
pub fn set_local_repo(path: String) -> Result<local_repo::LocalRepoStatus, String> {
    let p = PathBuf::from(path.trim());
    crate::paths::set_local_repo(Some(p))?;
    Ok(local_repo::current_status())
}

/// Disable offline mode: clears the persisted local repo path. The env
/// variable `SMARTBRIDGE_LOCAL_REPO`, if set, still takes effect.
#[tauri::command]
pub fn clear_local_repo() -> Result<local_repo::LocalRepoStatus, String> {
    crate::paths::set_local_repo(None)?;
    Ok(local_repo::current_status())
}

/// Snapshot the build flavor (release / demo / beta_0_1) and Beta
/// activation state. The frontend uses this to:
///   * adjust the header title and badge,
///   * decide whether to show the Activation card,
///   * gate the rest of the dashboard behind activation when needed.
#[tauri::command]
pub fn get_license_status() -> license::LicenseStatus {
    license::current_status()
}

/// Validate (email, serial) against the bundled salt and, if it matches,
/// write `~/Library/SmartBridge/license.json` so the plugin can re-verify
/// at runtime. Only meaningful for the Beta_0_1 flavor; a no-op on others.
#[tauri::command]
pub fn activate_beta(email: String, serial: String) -> license::ActivationOutcome {
    license::activate_beta(&email, &serial)
}

// =============================================================================
// Preflight
// =============================================================================

/// Run every preflight check in parallel and return them in display order.
/// Used by the Preflight tab as the "Check everything" entry point.
#[tauri::command]
pub async fn run_preflight() -> Vec<preflight::PreflightCheck> {
    preflight::run_all().await
}

/// Re-run a single preflight check by id (for the per-card "Check again"
/// button after a fix).
#[tauri::command]
pub async fn run_preflight_one(check_id: String) -> Option<preflight::PreflightCheck> {
    preflight::run_one(&check_id).await
}

/// Apply a one-click fix for a failing preflight check.
#[tauri::command]
pub async fn apply_preflight_fix(
    app: AppHandle,
    fix: preflight::FixAction,
) -> preflight::FixOutcome {
    preflight::apply_fix(&app, fix).await
}

/// Build the diagnostics report and write it to a timestamped file
/// in the installer log folder. Returns the absolute path so the
/// frontend can show "Saved to <path>". Caller passes in the
/// preflight checks it just rendered so the report and the UI agree
/// on what was found.
#[tauri::command]
pub fn save_preflight_report(checks: Vec<preflight::PreflightCheck>) -> Result<String, String> {
    tracing::info!(target: "ipc", count = checks.len(), "save_preflight_report");
    let path = preflight::report::save(&checks)?;
    Ok(path.to_string_lossy().to_string())
}

// =============================================================================
// Uninstall
// =============================================================================

/// Was Setup launched with `--uninstall`? Read by the frontend on mount
/// so the dashboard can swap to the dedicated uninstall view.
#[tauri::command]
pub fn get_uninstall_mode() -> crate::UninstallMode {
    crate::uninstall_mode()
}

/// Run the SmartBridge main-app uninstall. Always available — the
/// dedicated `--uninstall main-app` flow uses this, and so does the
/// "also delete my data" path. `remove_user_data=true` deletes the
/// user's encrypted database, config.json, and lyrics; default is false.
#[tauri::command]
pub async fn uninstall_main_app(
    remove_user_data: bool,
) -> install::InstallOutcome {
    tracing::info!(target: "ipc", remove_user_data, "uninstall_main_app");
    install::main_app::uninstall(remove_user_data).await
}

/// Clean uninstall: removes every Setup-managed component in one pass.
///
/// `remove_user_data=true` wipes the user's encrypted database, config,
/// lyrics, and saved sessions (`~/Library/SmartBridge` or
/// `%APPDATA%\SmartBridge`). Default in the dashboard UI is ON.
///
/// `remove_ollama_models=true` removes the SmartBridge Optimized triplet
/// (tinyllama / mistral / qwen2.5) and the default Llama model
/// (gemma4:e4b). The Ollama runtime itself is not touched. Default in
/// the dashboard UI is ON.
///
/// The implementation never short-circuits on a sub-step error — every
/// component is attempted and all messages/errors are returned in a
/// single InstallOutcome.
#[tauri::command]
pub async fn clean_uninstall(
    remove_user_data: bool,
    remove_ollama_models: bool,
) -> install::InstallOutcome {
    tracing::info!(
        target: "ipc",
        remove_user_data,
        remove_ollama_models,
        "clean_uninstall",
    );
    install::clean_uninstall::run(remove_user_data, remove_ollama_models).await
}

/// Build a "support bundle" zip on the user's Desktop. Contains the
/// rotated SmartBridge Setup logs, the SmartBridge.exe log (if present),
/// and a metadata.txt with non-secret host/manifest/uninstall-mode
/// information. Reveals the resulting file in the OS file manager and
/// returns its absolute path so the UI can show it.
///
/// The customer is expected to email/upload that one file when something
/// goes wrong — it's the equivalent of an Apple sysdiagnose for SmartBridge
/// Setup.
#[tauri::command]
pub fn save_support_bundle() -> Result<String, String> {
    tracing::info!(target: "ipc", "save_support_bundle");
    let path = crate::support_bundle::build()?;
    Ok(path.to_string_lossy().to_string())
}
