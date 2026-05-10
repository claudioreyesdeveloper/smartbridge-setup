//! synthv-connection: detects Synthesizer V Studio 1 and 2 and the
//! side-panel script in each one's scripts folder.
//!
//! macOS:
//!   ~/Library/Application Support/Dreamtonics/Synthesizer V Studio[ 2]/scripts/synthv_smartbridge_sidepanel.lua
//!   /Applications/Synthesizer V Studio[ 2].app           (host detection)
//!
//! Windows:
//!   %USERPROFILE%\Documents\Dreamtonics\Synthesizer V Studio[ 2]\scripts\synthv_smartbridge_sidepanel.lua
//!   %PROGRAMFILES%\Dreamtonics\Synthesizer V Studio[ 2]  (host detection)

use super::DetectionResult;
use std::path::PathBuf;

const SCRIPT_FILE: &str = "synthv_smartbridge_sidepanel.lua";

#[derive(Debug, Clone, Copy)]
enum Studio {
    One,
    Two,
}

impl Studio {
    fn dir_name(self) -> &'static str {
        match self {
            Studio::One => "Synthesizer V Studio",
            Studio::Two => "Synthesizer V Studio 2",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Studio::One => "SynthV Studio 1",
            Studio::Two => "SynthV Studio 2",
        }
    }
}

pub async fn detect() -> DetectionResult {
    let mut present: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut any_studio_detected = false;

    for studio in [Studio::One, Studio::Two] {
        let installed = studio_installed(studio);
        let script_path = studio_script_path(studio);

        if !installed {
            continue;
        }

        any_studio_detected = true;
        if script_path.exists() {
            present.push(format!(
                "{}: script at {}",
                studio.label(),
                script_path.display()
            ));
        } else {
            missing.push(format!(
                "{}: script not installed at {}",
                studio.label(),
                script_path.display()
            ));
        }
    }

    if !any_studio_detected && present.is_empty() {
        return DetectionResult::not_installed()
            .with_detail("Synthesizer V Studio not detected on this machine.");
    }

    if missing.is_empty() && !present.is_empty() {
        let mut r = DetectionResult::ready();
        for p in present {
            r = r.with_detail(p);
        }
        return r;
    }

    if missing.is_empty() && present.is_empty() {
        return DetectionResult::not_installed();
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

fn studio_installed(studio: Studio) -> bool {
    studio_app_dir(studio)
        .map(|p| p.exists())
        .unwrap_or(false)
}

fn studio_app_dir(studio: Studio) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        Some(PathBuf::from("/Applications").join(format!("{}.app", studio.dir_name())))
    }

    #[cfg(target_os = "windows")]
    {
        let bases = [
            std::env::var("ProgramFiles").ok().map(PathBuf::from),
            std::env::var("ProgramFiles(x86)").ok().map(PathBuf::from),
        ];
        bases
            .into_iter()
            .flatten()
            .map(|b| b.join("Dreamtonics").join(studio.dir_name()))
            .find(|p| p.exists())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = studio;
        None
    }
}

fn studio_data_dir(studio: Studio) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("Dreamtonics")
                .join(studio.dir_name())
        })
    }

    #[cfg(target_os = "windows")]
    {
        dirs::document_dir().map(|d| d.join("Dreamtonics").join(studio.dir_name()))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = studio;
        None
    }
}

fn studio_script_path(studio: Studio) -> PathBuf {
    studio_data_dir(studio)
        .map(|d| d.join("scripts").join(SCRIPT_FILE))
        .unwrap_or_else(|| PathBuf::from(SCRIPT_FILE))
}
