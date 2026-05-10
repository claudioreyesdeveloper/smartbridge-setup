//! Cross-platform paths used by SmartBridge Setup.
//!
//! Centralised here so the rest of the codebase never branches on OS for
//! "where does config.json live" or "where do we cache downloaded assets".
//! When we discover the Windows / Linux defaults are wrong, we fix them
//! exactly once, here.
//!
//! NOTE on the Windows config path: the existing JUCE `ConfigManager` code
//! in the SmartBridge C++ app currently writes config.json to a Library-style
//! path on every platform, but the NSIS installer puts the seed file at
//! `%APPDATA%\SmartBridge\config.json`. This is a known mismatch; the Tauri
//! installer follows the NSIS convention on Windows and the JUCE convention
//! on macOS/Linux. The Diagnostics tab will warn about the mismatch until
//! the C++ app is patched.
//!
//! The dead-code allow is intentional during scaffolding; Phase 3 and 4
//! exercise these paths.

#![allow(dead_code)]

use std::path::PathBuf;

/// Where the user's SmartBridge config.json lives.
pub fn user_config_path() -> Option<PathBuf> {
    user_data_dir().map(|d| d.join("config.json"))
}

/// Where the user's encrypted SmartBridge database lives.
pub fn user_database_path() -> Option<PathBuf> {
    user_data_dir().map(|d| d.join("smartbridge.db"))
}

/// Per-user SmartBridge data directory (config, db, help files, logs).
///
/// macOS:   ~/Library/SmartBridge/
/// Windows: %APPDATA%\SmartBridge\           (matches NSIS installer)
/// Linux:   ~/.local/share/SmartBridge/      (XDG)
pub fn user_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|h| h.join("Library").join("SmartBridge"))
    }

    #[cfg(target_os = "windows")]
    {
        dirs::config_dir().map(|c| c.join("SmartBridge"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs::data_local_dir().map(|d| d.join("SmartBridge"))
    }
}

/// Where the help files component places its content.
pub fn help_files_dir() -> Option<PathBuf> {
    user_data_dir().map(|d| d.join("help"))
}

/// Per-user SmartBridge program install directory on Windows.
///
/// `%LOCALAPPDATA%\Programs\SmartBridge\` — the modern per-user install
/// convention used by VS Code, Cursor, Spotify, etc. No UAC required.
/// Both detection and the new native installer write here.
#[cfg(target_os = "windows")]
pub fn windows_main_app_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("Programs").join("SmartBridge"))
}

/// Canonical install location for SmartBridge.app on macOS.
///
/// `/Applications/SmartBridge.app` — where the .pkg installer drops it
/// and where Spotlight, Launch Services, and Finder expect it. We only
/// touch this path on uninstall.
#[cfg(target_os = "macos")]
pub fn macos_main_app_path() -> PathBuf {
    PathBuf::from("/Applications/SmartBridge.app")
}

/// Per-user VST3 plug-in directory on macOS.
///
/// `~/Library/Audio/Plug-Ins/VST3/` — the per-user VST3 location every
/// macOS DAW (Logic, Cubase, Live, Studio One, Reaper, …) scans. The
/// build pipeline drops `SmartBridge.vst3` inside.
#[cfg(target_os = "macos")]
pub fn macos_vst3_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| {
        h.join("Library")
            .join("Audio")
            .join("Plug-Ins")
            .join("VST3")
    })
}

/// Per-user Audio Unit (AU) component directory on macOS.
///
/// `~/Library/Audio/Plug-Ins/Components/` — Logic Pro, GarageBand and
/// other AU hosts only see plug-ins from this directory (or the
/// system-wide `/Library/...` equivalent). The build pipeline drops
/// `SmartBridge.component` inside.
#[cfg(target_os = "macos")]
pub fn macos_audio_unit_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| {
        h.join("Library")
            .join("Audio")
            .join("Plug-Ins")
            .join("Components")
    })
}

/// Per-user VST3 plug-in directory on Windows.
///
/// `%LOCALAPPDATA%\Programs\Common\VST3\` — recognised by Cubase 12+,
/// Studio One 6+, Live 11+, FL Studio 21+. SmartBridge's bundle goes
/// inside this folder as `SmartBridge.vst3\`.
#[cfg(target_os = "windows")]
pub fn windows_vst3_dir() -> Option<PathBuf> {
    dirs::data_local_dir()
        .map(|d| d.join("Programs").join("Common").join("VST3"))
}

/// Per-user Start Menu Programs folder on Windows.
///
/// `%APPDATA%\Microsoft\Windows\Start Menu\Programs\`. Per-user shortcuts
/// (no admin) appear here for the current Windows account only.
#[cfg(target_os = "windows")]
pub fn windows_start_menu_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| {
        d.join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
    })
}

/// Per-user data directory for SmartBridge Setup itself
/// (download cache, manifest cache, logs).
///
/// macOS:   ~/Library/Application Support/SmartBridge Setup/
/// Windows: %APPDATA%\SmartBridge Setup\
/// Linux:   ~/.local/share/SmartBridge Setup/
pub fn installer_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("SmartBridge Setup")
        })
    }

    #[cfg(target_os = "windows")]
    {
        dirs::config_dir().map(|c| c.join("SmartBridge Setup"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs::data_local_dir().map(|d| d.join("SmartBridge Setup"))
    }
}

pub fn installer_log_dir() -> Option<PathBuf> {
    installer_data_dir().map(|d| d.join("logs"))
}

pub fn installer_download_cache_dir() -> Option<PathBuf> {
    installer_data_dir().map(|d| d.join("downloads"))
}

pub fn installer_manifest_cache_path() -> Option<PathBuf> {
    installer_data_dir().map(|d| d.join("smartbridge-release-manifest.json"))
}

/// Path to SmartBridge Setup's small persistent settings file.
/// Currently holds the configured offline / local-repo directory.
pub fn setup_config_path() -> Option<PathBuf> {
    installer_data_dir().map(|d| d.join("setup-config.json"))
}

/// Resolve the offline / local-repo directory the user has configured, if any.
///
/// Search order:
///   1. `SMARTBRIDGE_LOCAL_REPO` environment variable (for IT-managed
///      / scripted deployments).
///   2. `local_repo` value persisted in `setup-config.json`.
///
/// Returns `None` if neither is set, or if the resolved path doesn't exist
/// on disk. Returns `Some(path)` only if the directory is real and
/// readable; the caller still has to validate the manifest is present
/// inside it.
pub fn local_repo_dir() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("SMARTBRIDGE_LOCAL_REPO") {
        let p = PathBuf::from(env_path.trim());
        if p.is_dir() {
            return Some(p);
        }
    }
    if let Some(cfg) = read_setup_config() {
        if let Some(p) = cfg.local_repo.as_deref() {
            let p = PathBuf::from(p);
            if p.is_dir() {
                return Some(p);
            }
        }
    }
    None
}

/// Persist the local repo path. Pass `None` to clear.
/// Returns the path actually written (canonicalized when possible).
pub fn set_local_repo(new: Option<PathBuf>) -> Result<Option<PathBuf>, String> {
    let mut cfg = read_setup_config().unwrap_or_default();
    let canonical = match new {
        Some(p) => {
            if !p.is_dir() {
                return Err(format!("not a directory: {}", p.display()));
            }
            let canon = std::fs::canonicalize(&p).unwrap_or(p);
            cfg.local_repo = Some(canon.to_string_lossy().to_string());
            Some(canon)
        }
        None => {
            cfg.local_repo = None;
            None
        }
    };
    write_setup_config(&cfg)?;
    Ok(canonical)
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SetupConfig {
    /// Absolute path to a folder containing a SmartBridge offline bundle
    /// (a manifest plus all referenced assets, named exactly as they
    /// appear in the manifest's `release_asset_name` / `file_name`).
    pub local_repo: Option<String>,
}

fn read_setup_config() -> Option<SetupConfig> {
    let path = setup_config_path()?;
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice::<SetupConfig>(&bytes).ok()
}

fn write_setup_config(cfg: &SetupConfig) -> Result<(), String> {
    let path = setup_config_path().ok_or_else(|| "no setup config path".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir setup config: {e}"))?;
    }
    let json = serde_json::to_vec_pretty(cfg).map_err(|e| format!("serialize: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| format!("write tmp: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
    Ok(())
}

/// Resolve the absolute path to the `ollama` binary, with fallbacks for the
/// common case where SmartBridge Setup is launched from Finder/Dock and
/// inherits the stripped system PATH (no Homebrew, no /usr/local/bin).
///
/// Search order:
///   1. The user's PATH (via `which` / `where.exe`).
///   2. Known per-platform install locations.
///
/// Returns None if ollama is not found anywhere we look.
pub fn resolve_ollama() -> Option<PathBuf> {
    if let Some(p) = ollama_via_path() {
        return Some(p);
    }
    for candidate in known_ollama_locations() {
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn ollama_via_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let probe = "where.exe";
    #[cfg(not(target_os = "windows"))]
    let probe = "/usr/bin/which";

    let out = std::process::Command::new(probe)
        .arg("ollama")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if line.is_empty() {
        None
    } else {
        Some(PathBuf::from(line))
    }
}

fn known_ollama_locations() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let mut v = vec![
            PathBuf::from("/opt/homebrew/bin/ollama"),
            PathBuf::from("/usr/local/bin/ollama"),
            PathBuf::from("/Applications/Ollama.app/Contents/Resources/ollama"),
        ];
        if let Some(home) = dirs::home_dir() {
            v.push(home.join(".local").join("bin").join("ollama"));
        }
        v
    }

    #[cfg(target_os = "windows")]
    {
        let mut v: Vec<PathBuf> = Vec::new();
        if let Some(local) = dirs::data_local_dir() {
            v.push(local.join("Programs").join("Ollama").join("ollama.exe"));
        }
        if let Ok(pf) = std::env::var("ProgramFiles") {
            v.push(PathBuf::from(pf).join("Ollama").join("ollama.exe"));
        }
        v
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut v = vec![
            PathBuf::from("/usr/local/bin/ollama"),
            PathBuf::from("/usr/bin/ollama"),
        ];
        if let Some(home) = dirs::home_dir() {
            v.push(home.join(".local").join("bin").join("ollama"));
        }
        v
    }
}
