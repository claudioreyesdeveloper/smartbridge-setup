//! Per-component detection.
//!
//! Each detector inspects the user's machine and returns a [`DetectionResult`]
//! describing whether the component is present, broken, or missing. Detection
//! never modifies the machine; only Phase 4 install actions do that.
//!
//! The aggregate [`detect_all`] function fans out across components in
//! parallel, so the dashboard can render in a few hundred milliseconds even
//! when one detector (e.g. AI Lyrics, which makes a network call to Ollama)
//! is slow.

pub mod ai_lyrics;
pub mod ai_lyrics_default;
pub mod cubase;
pub mod help_files;
pub mod loopmidi;
pub mod main_app;
pub mod resources;
pub mod synthv;

use crate::commands::ComponentState;
use serde::Serialize;

/// One component's detection outcome.
#[derive(Debug, Clone, Serialize)]
pub struct DetectionResult {
    pub state: ComponentState,
    /// Best-effort installed version string, when discoverable.
    pub installed_version: Option<String>,
    /// Free-form diagnostic lines for the Diagnostics tab. Never includes
    /// secrets — caller must not pass through API keys, tokens, or paths
    /// that would leak user identity beyond what's already public on the
    /// host (the home dir name is fine; OS account UUIDs are not).
    pub details: Vec<String>,
}

impl DetectionResult {
    pub fn ready() -> Self {
        Self {
            state: ComponentState::Ready,
            installed_version: None,
            details: Vec::new(),
        }
    }

    pub fn not_installed() -> Self {
        Self {
            state: ComponentState::NotInstalled,
            installed_version: None,
            details: Vec::new(),
        }
    }

    pub fn needs_repair(reason: impl Into<String>) -> Self {
        Self {
            state: ComponentState::NeedsRepair,
            installed_version: None,
            details: vec![reason.into()],
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            state: ComponentState::Error,
            installed_version: None,
            details: vec![message.into()],
        }
    }

    pub fn not_available_in_build() -> Self {
        Self {
            state: ComponentState::NotAvailableInBuild,
            installed_version: None,
            details: Vec::new(),
        }
    }

    pub fn with_version(mut self, v: impl Into<String>) -> Self {
        self.installed_version = Some(v.into());
        self
    }

    pub fn with_detail(mut self, d: impl Into<String>) -> Self {
        self.details.push(d.into());
        self
    }
}

/// Run every detector concurrently. Returns a vec in the order the
/// dashboard expects (matching `commands::list_components`).
pub async fn detect_all() -> Vec<(&'static str, DetectionResult)> {
    let (main, cubase, ai, ai_default, synthv, resources, help, loopmidi) = tokio::join!(
        main_app::detect(),
        cubase::detect(),
        ai_lyrics::detect(),
        ai_lyrics_default::detect(),
        synthv::detect(),
        resources::detect(),
        help_files::detect(),
        loopmidi::detect(),
    );

    vec![
        ("main-app", main),
        ("cubase-connection", cubase),
        ("ai-lyrics", ai),
        ("ai-lyrics-default-model", ai_default),
        ("synthv-connection", synthv),
        ("smartbridge-resources", resources),
        ("help-files", help),
        ("windows-loopmidi", loopmidi),
    ]
}

/// Re-run a single detector. Used by the "Check again" button on a card.
pub async fn detect_one(component_id: &str) -> Option<DetectionResult> {
    Some(match component_id {
        "main-app" => main_app::detect().await,
        "cubase-connection" => cubase::detect().await,
        "ai-lyrics" => ai_lyrics::detect().await,
        "ai-lyrics-default-model" => ai_lyrics_default::detect().await,
        "synthv-connection" => synthv::detect().await,
        "smartbridge-resources" => resources::detect().await,
        "help-files" => help_files::detect().await,
        "windows-loopmidi" => loopmidi::detect().await,
        _ => return None,
    })
}
