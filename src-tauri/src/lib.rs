//! SmartBridge Setup - Rust backend entry point.
//!
//! All real work lives in submodules. This file only:
//!   * sets up logging,
//!   * parses `--uninstall <component>` CLI args (so the SAME Setup.exe
//!     binary serves both as installer and uninstaller - that's what the
//!     UninstallString in HKCU\Software\...\Uninstall\SmartBridge points at),
//!   * registers Tauri commands,
//!   * starts the Tauri runtime.
//!
//! The frontend that ships in this repo (the rewritten "pensioner-friendly"
//! wizard) talks to a small surface: `wizard::*`, `license::*`,
//! `commands::get_uninstall_mode`, `commands::installer_version`. The
//! older per-component dashboard commands are still defined and registered
//! so the same backend binary keeps working with any external automation
//! / smoke tests / future debug surface that calls them - they just have
//! no UI any more.

mod commands;
mod host;
mod logging;
mod paths;
mod support_bundle;
mod wizard;

pub mod detection;
pub mod download;
pub mod install;
pub mod license;
pub mod local_repo;
pub mod manifest;
pub mod preflight;

pub use commands::{ComponentCard, ComponentState};

use once_cell::sync::OnceCell;
use serde::Serialize;

/// Resolved at process startup from `argv`. Read from the frontend via
/// `get_uninstall_mode` so the wizard router can swap to the uninstall
/// flow when the user clicks "Uninstall" in Windows Settings -> Apps.
static UNINSTALL_MODE: OnceCell<UninstallMode> = OnceCell::new();

#[derive(Debug, Clone, Default, Serialize)]
pub struct UninstallMode {
    pub active: bool,
    /// Optional component name passed after `--uninstall`. When `None`
    /// in active mode, the wizard offers the full uninstall (everything).
    pub component: Option<String>,
}

pub fn uninstall_mode() -> UninstallMode {
    UNINSTALL_MODE.get().cloned().unwrap_or_default()
}

fn parse_uninstall_args(args: &[String]) -> UninstallMode {
    let mut iter = args.iter().skip(1); // skip exe path
    while let Some(a) = iter.next() {
        if a == "--uninstall" || a == "-u" {
            let component = iter.next().cloned();
            return UninstallMode {
                active: true,
                component,
            };
        }
    }
    UninstallMode::default()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _log_guard = logging::init();

    let argv: Vec<String> = std::env::args().collect();
    let mode = parse_uninstall_args(&argv);
    let _ = UNINSTALL_MODE.set(mode.clone());

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        uninstall = mode.active,
        component = ?mode.component,
        "SmartBridge Setup starting",
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // ---- New wizard surface (the only thing the frontend uses) ----
            wizard::check_internet_connection,
            wizard::install_plan,
            wizard::install_all,
            wizard::uninstall_all,
            wizard::compose_help_email,
            // ---- Identity / mode ----
            commands::installer_version,
            commands::host_info,
            commands::get_uninstall_mode,
            // ---- License (used by the wizard's beta-activation screen) ----
            commands::get_license_status,
            commands::activate_beta,
            // ---- Legacy / debug surface (no UI, kept callable) ----
            commands::manifest_url,
            commands::list_components,
            commands::recheck_component,
            commands::fetch_manifest,
            commands::install_component,
            commands::remove_component,
            commands::open_log_folder,
            commands::save_support_bundle,
            commands::get_local_repo_status,
            commands::set_local_repo,
            commands::clear_local_repo,
            commands::run_preflight,
            commands::run_preflight_one,
            commands::apply_preflight_fix,
            commands::save_preflight_report,
            commands::uninstall_main_app,
            commands::clean_uninstall,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SmartBridge Setup");
}
