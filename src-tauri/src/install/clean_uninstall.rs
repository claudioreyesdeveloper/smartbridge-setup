//! Clean uninstall: remove every Setup-managed SmartBridge artifact in
//! one operation.
//!
//! The dashboard exposes a per-component "Remove" button on every card.
//! That's fine for surgical fixes ("my SynthV script is busted, reinstall
//! just that one"), but a customer who wants SmartBridge gone shouldn't
//! have to click Remove eight times in a specific order. This module
//! drives the full uninstall:
//!
//!   1. Optional components in dashboard order
//!        cubase-connection
//!        ai-lyrics              (only if remove_ollama_models)
//!        ai-lyrics-default-model (only if remove_ollama_models)
//!        synthv-connection
//!        smartbridge-resources
//!        help-files
//!        windows-loopmidi
//!   2. main-app last, with the user-data toggle threaded through so
//!      ~/Library/SmartBridge or %APPDATA%\SmartBridge is wiped iff
//!      remove_user_data is set.
//!
//! Each step's outcome is appended to one combined InstallOutcome so
//! the dashboard can show a single result panel. We never short-circuit
//! on a sub-step error — the goal is to make as much progress as the
//! environment allows and surface every failure together. The caller
//! gets exit_ok iff every sub-step succeeded.

use super::{main_app, InstallOutcome};
use crate::detection;

const COMPONENT: &str = "clean-uninstall";

/// Components removed in clean uninstall. Order matches the dashboard
/// tile order with main-app deliberately omitted (it's handled
/// separately at the end so we can pass the user-data toggle).
const COMPONENT_ORDER: &[&str] = &[
    "cubase-connection",
    "ai-lyrics",
    "ai-lyrics-default-model",
    "synthv-connection",
    "smartbridge-resources",
    "help-files",
    "windows-loopmidi",
];

fn is_ollama_component(component_id: &str) -> bool {
    component_id == "ai-lyrics" || component_id == "ai-lyrics-default-model"
}

pub async fn run(remove_user_data: bool, remove_ollama_models: bool) -> InstallOutcome {
    let mut messages: Vec<String> = Vec::new();
    let mut had_error = false;

    messages.push(format!(
        "Clean uninstall starting (user_data={}, ollama_models={}).",
        remove_user_data, remove_ollama_models,
    ));

    // 1. Optional components.
    for cid in COMPONENT_ORDER {
        if is_ollama_component(cid) && !remove_ollama_models {
            messages.push(format!(
                "Skipped `{cid}` — Ollama removal not requested."
            ));
            continue;
        }

        messages.push(format!("Removing `{cid}`…"));
        let outcome = super::remove(cid).await;
        if !outcome.success {
            had_error = true;
        }
        for line in outcome.messages {
            messages.push(format!("  [{cid}] {line}"));
        }
    }

    // 2. main-app last. Pass remove_user_data through so the same call
    //    handles the binary, plug-ins, registry/start menu (Windows),
    //    .app + VST3 + AU (macOS), and optionally the user-data dir.
    messages.push("Removing `main-app`…".to_string());
    let outcome = main_app::uninstall(remove_user_data).await;
    if !outcome.success {
        had_error = true;
    }
    for line in outcome.messages {
        messages.push(format!("  [main-app] {line}"));
    }

    // 3. One final detection pass to refresh the dashboard so the cards
    //    flip to NotInstalled where appropriate.
    let det = detection::main_app::detect().await;

    if had_error {
        let mut tail = vec![
            "One or more steps reported errors. See messages above for the \
             specific path / component that failed."
                .into(),
        ];
        messages.append(&mut tail);
        InstallOutcome::err(COMPONENT, messages).with_post_state(det)
    } else {
        messages.push("Clean uninstall finished without errors.".into());
        InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
    }
}
