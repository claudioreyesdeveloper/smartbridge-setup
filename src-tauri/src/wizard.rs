//! Wizard orchestrator.
//!
//! The new SmartBridge Setup frontend is a one-screen-at-a-time wizard
//! aimed at users with very low computer skills. This module exposes the
//! handful of high-level Tauri commands that the wizard talks to:
//!
//!   * `install_plan`       - given the user's three yes/no answers,
//!                             returns the ordered list of components to
//!                             install on the current host. Pure data:
//!                             the frontend uses it to render "We will
//!                             install N things" and the step labels.
//!   * `install_all`        - runs that plan, emits one `WizardStepEvent`
//!                             per step on `wizard://step`, and returns
//!                             a single aggregated outcome.
//!   * `uninstall_all`      - thin wrapper around the existing
//!                             `clean_uninstall::run` so the uninstall
//!                             wizard has a single command to call.
//!   * `compose_help_email` - builds the support bundle on the user's
//!                             Desktop and returns a ready-to-open
//!                             `mailto:` URL pointing at the help inbox.
//!
//! The per-component install logic in `install/*` and the manifest
//! fetcher are reused as-is. Nothing here duplicates that work.
//!
//! NOTE: this file is intentionally pure ASCII. German strings use
//! `\u{xxxx}` escapes for umlauts so the source is byte-identical across
//! editors / write paths / git transforms.

use crate::install;
use crate::manifest;
use crate::support_bundle;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// Where the in-app "Get help by email" button sends. Centralised so it
/// can be changed in one place without touching the frontend.
pub const HELP_EMAIL_ADDRESS: &str = "claudio.private@gmail.com";

// =============================================================================
// Plan
// =============================================================================

/// The three yes/no answers the wizard collects on its profile screen.
/// Anything not represented here is decided automatically.
#[derive(Debug, Clone, Deserialize)]
pub struct ProfileChoice {
    pub use_cubase: bool,
    pub use_synthv: bool,
    pub use_ai_lyrics: bool,
}

/// One row of the install plan. The label fields are deliberately short,
/// plain-language sentences (no version numbers, no "MIDI Remote Script"
/// jargon). The frontend picks one based on locale.
#[derive(Debug, Clone, Serialize)]
pub struct PlanStep {
    pub component_id: &'static str,
    pub label_en: &'static str,
    pub label_de: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallPlan {
    pub steps: Vec<PlanStep>,
}

/// The full universe of steps the wizard knows how to render. The order
/// here is the order they will be installed in. `install_plan` filters
/// this list down based on the user's profile answers and the host OS.
const ALL_STEPS: &[PlanStep] = &[
    PlanStep {
        component_id: "main-app",
        label_en: "Installing the main program",
        label_de: "Hauptprogramm wird installiert",
    },
    PlanStep {
        component_id: "smartbridge-resources",
        label_en: "Setting up your songs folder",
        label_de: "Ihr Lieder-Ordner wird vorbereitet",
    },
    PlanStep {
        component_id: "help-files",
        label_en: "Adding the help guide",
        label_de: "Anleitung wird hinzugef\u{fc}gt",
    },
    PlanStep {
        component_id: "cubase-connection",
        label_en: "Connecting to Cubase",
        label_de: "Verbindung zu Cubase wird eingerichtet",
    },
    PlanStep {
        component_id: "windows-loopmidi",
        label_en: "Adding the music cable Cubase needs",
        label_de: "Audio-Verbindung f\u{fc}r Cubase wird eingerichtet",
    },
    PlanStep {
        component_id: "synthv-connection",
        label_en: "Connecting to Synthesizer V",
        label_de: "Verbindung zu Synthesizer V wird eingerichtet",
    },
    PlanStep {
        component_id: "ai-lyrics",
        label_en: "Setting up lyric suggestions",
        label_de: "Liedtext-Vorschl\u{e4}ge werden eingerichtet",
    },
    PlanStep {
        component_id: "ai-lyrics-default-model",
        label_en: "Downloading the lyric helper (this is the slow part, ~10 GB)",
        label_de: "Liedtext-Helfer wird heruntergeladen (dauert lange, ca. 10 GB)",
    },
];

fn step_for(id: &str) -> Option<PlanStep> {
    ALL_STEPS.iter().find(|s| s.component_id == id).cloned()
}

#[tauri::command]
pub fn install_plan(profile: ProfileChoice) -> InstallPlan {
    let host_is_windows = std::env::consts::OS == "windows";

    let mut ids: Vec<&'static str> = vec![
        "main-app",
        "smartbridge-resources",
        "help-files",
    ];

    if profile.use_cubase {
        ids.push("cubase-connection");
        // loopMIDI is Windows-only - on macOS the IAC driver does the
        // same job and the per-component install dispatcher would just
        // error out, which would needlessly fail the whole wizard.
        if host_is_windows {
            ids.push("windows-loopmidi");
        }
    }

    if profile.use_synthv {
        ids.push("synthv-connection");
    }

    if profile.use_ai_lyrics {
        ids.push("ai-lyrics");
        ids.push("ai-lyrics-default-model");
    }

    let steps: Vec<PlanStep> = ids.into_iter().filter_map(step_for).collect();
    InstallPlan { steps }
}

// =============================================================================
// Run plan
// =============================================================================

/// Per-step progress event broadcast on `wizard://step`. The frontend
/// uses `step_index` / `step_count` to drive the single big progress
/// bar and `component_id` to pick the right "Installing the X..." line.
/// `status` is one of `"starting"`, `"ok"`, `"failed"`.
#[derive(Debug, Clone, Serialize)]
pub struct WizardStepEvent {
    pub step_index: u32,
    pub step_count: u32,
    pub component_id: String,
    pub status: &'static str,
    /// Only set when `status == "failed"`. Plain-language one-liner
    /// already mapped from raw error text by `friendly_failure_message`.
    pub failure_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallAllOutcome {
    pub success: bool,
    pub failed_step_index: Option<u32>,
    pub failed_component_id: Option<String>,
    /// Pre-mapped, plain-language sentence the wizard can render verbatim.
    /// Empty on success.
    pub failure_message: String,
    /// Detailed messages per step (in plan order). Always present even
    /// on success - mostly useful for the support bundle / debugging.
    pub step_messages: Vec<Vec<String>>,
}

#[tauri::command]
pub async fn install_all(
    app: AppHandle,
    profile: ProfileChoice,
) -> Result<InstallAllOutcome, String> {
    let plan = install_plan(profile);
    let total = plan.steps.len() as u32;

    tracing::info!(target: "wizard", steps = total, "install_all starting");

    let fetched = manifest::fetch_with_fallback().await.map_err(|e| {
        tracing::error!(target: "wizard", error = %e, "fetch_manifest failed before install_all");
        // The frontend translates this into the same "no internet"
        // message as a per-step network failure - we just propagate the
        // raw string here so it lands in the logs.
        e
    })?;

    let mut step_messages: Vec<Vec<String>> = Vec::with_capacity(plan.steps.len());

    for (i, step) in plan.steps.iter().enumerate() {
        let idx = i as u32;

        emit_step(
            &app,
            WizardStepEvent {
                step_index: idx,
                step_count: total,
                component_id: step.component_id.to_string(),
                status: "starting",
                failure_message: None,
            },
        );

        let outcome = install::install(&app, &fetched.manifest, step.component_id).await;
        step_messages.push(outcome.messages.clone());

        if !outcome.success {
            let msg = friendly_failure_message(step.component_id, &outcome.messages);
            tracing::error!(
                target: "wizard",
                component = step.component_id,
                step_index = idx,
                "install_all aborting on first failure",
            );
            emit_step(
                &app,
                WizardStepEvent {
                    step_index: idx,
                    step_count: total,
                    component_id: step.component_id.to_string(),
                    status: "failed",
                    failure_message: Some(msg.clone()),
                },
            );
            return Ok(InstallAllOutcome {
                success: false,
                failed_step_index: Some(idx),
                failed_component_id: Some(step.component_id.to_string()),
                failure_message: msg,
                step_messages,
            });
        }

        emit_step(
            &app,
            WizardStepEvent {
                step_index: idx,
                step_count: total,
                component_id: step.component_id.to_string(),
                status: "ok",
                failure_message: None,
            },
        );
    }

    tracing::info!(target: "wizard", "install_all completed successfully");
    Ok(InstallAllOutcome {
        success: true,
        failed_step_index: None,
        failed_component_id: None,
        failure_message: String::new(),
        step_messages,
    })
}

fn emit_step(app: &AppHandle, ev: WizardStepEvent) {
    if let Err(e) = app.emit("wizard://step", &ev) {
        tracing::warn!(error = %e, "failed to emit wizard://step");
    }
}

/// Translate raw install error text (which often quotes a `reqwest`
/// error or a `std::io::Error`) into a single plain-English sentence
/// the wizard can show without scaring the user. Patterns are matched
/// against the lower-cased text of the LAST non-empty message line --
/// `InstallOutcome::err` puts the actual cause last by convention.
fn friendly_failure_message(component_id: &str, messages: &[String]) -> String {
    let raw = messages
        .iter()
        .rev()
        .find(|m| !m.trim().is_empty())
        .map(|s| s.as_str())
        .unwrap_or("");

    let lower = raw.to_lowercase();

    if lower.contains("dns")
        || lower.contains("resolve")
        || lower.contains("network is unreachable")
        || lower.contains("no such host")
        || lower.contains("connection refused")
        || lower.contains("timed out")
        || lower.contains("timeout")
    {
        return "We could not reach the internet. Please check your wifi or \
                network cable, then click Try again."
            .into();
    }

    if lower.contains("checksum") || lower.contains("size mismatch") {
        return "A file did not download correctly. This usually fixes itself \
                if you click Try again."
            .into();
    }

    if lower.contains("permission") || lower.contains("denied") || lower.contains("access is denied") {
        if cfg!(target_os = "windows") {
            return "Your computer would not let us write a file. Please close \
                    SmartBridge Setup, then right-click it and choose \
                    'Run as administrator'."
                .into();
        }
        return "Your computer would not let us write a file. Please make sure \
                you are signed in as the main user of this computer, then \
                click Try again."
            .into();
    }

    if lower.contains("disk") && (lower.contains("space") || lower.contains("full")) {
        return "Your computer is running low on free space. Please make some \
                room and click Try again."
            .into();
    }

    if lower.contains("offline") || lower.contains("local repo") {
        return "We could not find the offline installer files. Please try \
                again with an internet connection."
            .into();
    }

    let label = step_for(component_id)
        .map(|s| s.label_en)
        .unwrap_or("install");

    format!(
        "Something went wrong while we were busy with: {label}. \
         You can click Try again, or send us the help report so we can fix it for you."
    )
}

// =============================================================================
// Uninstall
// =============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct UninstallAllOutcome {
    pub success: bool,
    pub messages: Vec<String>,
}

/// Single-call uninstall used by the new uninstall wizard. `keep_user_data`
/// is the inverse of the legacy `remove_user_data`: `true` means "keep
/// the user's songs and settings", which maps to `remove_user_data =
/// false` and `remove_ollama_models = false` (Ollama models are large
/// and the user might still want them for other Ollama apps).
#[tauri::command]
pub async fn uninstall_all(keep_user_data: bool) -> UninstallAllOutcome {
    let remove_user_data = !keep_user_data;
    let remove_ollama_models = !keep_user_data;
    tracing::info!(
        target: "wizard",
        keep_user_data,
        remove_user_data,
        remove_ollama_models,
        "uninstall_all dispatch",
    );
    let outcome =
        install::clean_uninstall::run(remove_user_data, remove_ollama_models).await;
    UninstallAllOutcome {
        success: outcome.success,
        messages: outcome.messages,
    }
}

// =============================================================================
// Help by email
// =============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct HelpEmailOutcome {
    /// Absolute path of the support bundle zip we just wrote (always on
    /// the user's Desktop when possible). The frontend shows this so
    /// the user knows where to find the file if mail composing fails.
    pub bundle_path: String,
    /// Pre-built `mailto:` URL the frontend can hand to the opener
    /// plugin. Subject and body are pre-filled; the body explicitly
    /// instructs the user to drag the file from their Desktop into the
    /// email (cross-platform `mailto:` attachments are not reliable).
    pub mailto_url: String,
    /// The plain email address - shown as a fallback "or write to us
    /// at X" line so users whose default mail handler is broken can
    /// still reach support.
    pub help_email: &'static str,
}

#[tauri::command]
pub fn compose_help_email() -> Result<HelpEmailOutcome, String> {
    let path = support_bundle::build()?;
    let path_str = path.display().to_string();

    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "SmartBridge-Setup-Diagnostics.zip".to_string());

    let body = format!(
        "Hello SmartBridge support,\n\
         \n\
         Something went wrong while I was installing SmartBridge.\n\
         \n\
         A help file called \"{file_name}\" has been saved to my Desktop.\n\
         Please drag that file into this email before sending it - \
         that way you will be able to see what happened on my computer \
         and help me fix it.\n\
         \n\
         Thank you.\n"
    );

    let mailto = format!(
        "mailto:{to}?subject={subject}&body={body}",
        to = HELP_EMAIL_ADDRESS,
        subject = url_encode("SmartBridge installation problem"),
        body = url_encode(&body),
    );

    Ok(HelpEmailOutcome {
        bundle_path: path_str,
        mailto_url: mailto,
        help_email: HELP_EMAIL_ADDRESS,
    })
}

/// Minimal RFC-3986 percent-encoder, restricted to characters safe in
/// `mailto:` query parameters across Windows mail handlers (Outlook,
/// Mail), macOS Mail, and the major webmail handler registrations.
/// Spaces become `%20`, not `+` (the `+` form trips Outlook).
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}
