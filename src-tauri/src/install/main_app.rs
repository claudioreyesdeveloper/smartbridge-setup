//! main-app install: places the SmartBridge application natively. No
//! second-stage installer.
//!
//! ===== Windows (per-user, no UAC) =====
//!   1. Download main-app.windows asset (zip) and verify SHA256.
//!   2. Extract to a temp folder.
//!   3. Place files at canonical per-user paths:
//!        SmartBridge.exe          -> %LOCALAPPDATA%\Programs\SmartBridge\
//!        *.dll (sqlcipher, libcrypto, plus any other runtime deps the
//!         build pipeline placed next to SmartBridge.exe)
//!                                 -> %LOCALAPPDATA%\Programs\SmartBridge\
//!        SmartBridge.vst3\        -> %LOCALAPPDATA%\Programs\Common\VST3\
//!        smartbridge.enc.db       -> %APPDATA%\SmartBridge\smartbridge.db
//!                                    (always — this is read-only seed data
//!                                     encrypted with this release's
//!                                     SQLCipher key; an out-of-date file
//!                                     here is the #1 cause of "no DB
//!                                     connection" at app launch)
//!        config.default.json      -> %APPDATA%\SmartBridge\config.json
//!                                    (only if missing — config.json holds
//!                                     the customer's lyrics_apiKey and
//!                                     feature toggles, never overwrite)
//!   4. Write HKCU registry: SOFTWARE\VST3\SmartBridge + the standard
//!      Add/Remove Programs Uninstall key. UninstallString points back at
//!      Setup.exe with `--uninstall main-app` so the customer gets a
//!      one-click uninstall from Windows Settings.
//!   5. Create per-user Start Menu shortcuts under `…\Programs\SmartBridge\`.
//!   6. Re-detect.
//!
//! All Windows registry writes are HKCU only — per-user install means no
//! UAC, no machine-wide changes, no risk of breaking another user account
//! on the same PC.
//!
//! ===== macOS (unchanged) =====
//! Download .pkg, hand to Apple Installer.app via `/usr/bin/open`. The
//! Installer GUI handles auth, scope, and the actual file placement. We
//! deliberately do NOT shell to `installer(8)` — that would require sudo.
//! macOS install is fire-and-forget; the customer closes Installer when
//! done and clicks "Check again".

use super::InstallOutcome;
use crate::detection;
use crate::download::{fetch_with_verify, DownloadOutcome};
use crate::manifest::{Manifest, ManifestAsset};
use tauri::AppHandle;

const COMPONENT: &str = "main-app";

pub async fn install(app: &AppHandle, manifest: &Manifest) -> InstallOutcome {
    let asset_id = match platform_asset_id() {
        Some(id) => id,
        None => {
            return InstallOutcome::err(
                COMPONENT,
                vec!["No SmartBridge installer is published for this platform.".into()],
            );
        }
    };

    let component = match manifest.component(COMPONENT) {
        Some(c) => c,
        None => {
            return InstallOutcome::err(
                COMPONENT,
                vec![format!("manifest missing component {COMPONENT}")],
            );
        }
    };
    let asset = match component.asset(asset_id) {
        Some(a) => a,
        None => {
            return InstallOutcome::err(
                COMPONENT,
                vec![format!("manifest missing asset {asset_id}")],
            );
        }
    };

    let spec = match super::download_spec_for(asset) {
        Some(s) => s,
        None => {
            return InstallOutcome::err(
                COMPONENT,
                vec!["main-app asset is not downloadable".into()],
            );
        }
    };

    let outcome = match fetch_with_verify(app, &spec).await {
        Ok(o) => o,
        Err(e) => return InstallOutcome::err(COMPONENT, vec![format!("download failed: {e}")]),
    };

    #[cfg(target_os = "macos")]
    {
        return install_macos(asset, &outcome).await;
    }

    #[cfg(target_os = "windows")]
    {
        let _ = asset;
        return windows_impl::install_windows(manifest, &outcome).await;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = (asset, outcome);
        InstallOutcome::err(
            COMPONENT,
            vec!["No SmartBridge installer is published for this platform.".into()],
        )
    }
}

fn platform_asset_id() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("main-app.macos")
    } else if cfg!(target_os = "windows") {
        Some("main-app.windows")
    } else {
        None
    }
}

/// Uninstall SmartBridge. Inverse of [`install`].
///
/// Windows: removes the per-user install dir, the VST3 bundle, the
/// Start Menu shortcuts, and the HKCU registry entries that the
/// installer wrote. User data (`%APPDATA%\SmartBridge\smartbridge.db`,
/// `config.json`, etc.) is kept by default — pass `remove_user_data=true`
/// to delete it.
///
/// macOS: not yet implemented; surfaces a friendly "drag SmartBridge.app
/// to the Trash" message. macOS apps don't typically own an uninstall
/// flow.
pub async fn uninstall(remove_user_data: bool) -> InstallOutcome {
    #[cfg(target_os = "macos")]
    {
        macos_impl::uninstall_macos(remove_user_data).await
    }

    #[cfg(target_os = "windows")]
    {
        windows_impl::uninstall_windows(remove_user_data).await
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = remove_user_data;
        InstallOutcome::err(
            COMPONENT,
            vec!["Uninstall is not implemented for this platform.".into()],
        )
    }
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use crate::paths;
    use std::path::Path;
    use std::process::Command;

    /// Real macOS uninstall, mirroring the Windows side.
    ///
    /// Removes:
    ///   * /Applications/SmartBridge.app
    ///   * ~/Library/Audio/Plug-Ins/VST3/SmartBridge.vst3
    ///   * ~/Library/Audio/Plug-Ins/Components/SmartBridge.component (AU)
    ///   * (optional) ~/Library/SmartBridge user data
    ///
    /// The .pkg installer can write the .app as root:wheel; `rm -rf` from
    /// the user account fails with EACCES in that case. We try the
    /// no-sudo path first and fall back to `sudo rm -rf` only when
    /// permission requires it (which will prompt the user via
    /// AuthorizationServices the first time).
    pub async fn uninstall_macos(remove_user_data: bool) -> InstallOutcome {
        let mut messages: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        // 1. Best-effort terminate the running standalone app so file
        //    handles release before we delete the bundle. Plug-in copies
        //    loaded into a DAW will keep using the in-memory image until
        //    the host restarts — we surface a hint about that below.
        let _ = Command::new("/usr/bin/killall")
            .arg("SmartBridge")
            .status();

        // 2. Remove /Applications/SmartBridge.app.
        let app = paths::macos_main_app_path();
        match remove_path(&app) {
            RemoveOutcome::Removed => messages.push(format!("Removed {}", app.display())),
            RemoveOutcome::Absent => messages.push(format!("(no app at {})", app.display())),
            RemoveOutcome::Error(e) => errors.push(format!("remove {}: {e}", app.display())),
        }

        // 3. Remove the VST3 bundle.
        if let Some(vst3_root) = paths::macos_vst3_dir() {
            let vst3 = vst3_root.join("SmartBridge.vst3");
            match remove_path(&vst3) {
                RemoveOutcome::Removed => messages.push(format!("Removed {}", vst3.display())),
                RemoveOutcome::Absent => {}
                RemoveOutcome::Error(e) => errors.push(format!("remove {}: {e}", vst3.display())),
            }
        }

        // 4. Remove the Audio Unit component.
        if let Some(au_root) = paths::macos_audio_unit_dir() {
            let au = au_root.join("SmartBridge.component");
            match remove_path(&au) {
                RemoveOutcome::Removed => messages.push(format!("Removed {}", au.display())),
                RemoveOutcome::Absent => {}
                RemoveOutcome::Error(e) => errors.push(format!("remove {}: {e}", au.display())),
            }
        }

        // 5. Optionally remove the user-data dir. Same default-OFF
        //    contract as the Windows path: routine uninstalls preserve
        //    the customer's library + config + lyrics; the caller has
        //    to opt in explicitly.
        if let Some(data) = paths::user_data_dir() {
            if remove_user_data {
                match remove_path(&data) {
                    RemoveOutcome::Removed => messages.push(format!(
                        "Deleted SmartBridge user data at {}",
                        data.display()
                    )),
                    RemoveOutcome::Absent => {}
                    RemoveOutcome::Error(e) => {
                        errors.push(format!("remove user data {}: {e}", data.display()))
                    }
                }
            } else if data.exists() {
                messages.push(format!(
                    "Kept your SmartBridge data at {} (database, config, lyrics). \
                     Re-install will pick it up automatically.",
                    data.display()
                ));
            }
        }

        // Heads-up about plug-in caches that this step doesn't clear —
        // running DAWs hold the bundle in-memory and may have cached
        // module info that points at a now-missing path.
        messages.push(
            "If a DAW (Cubase, Logic, Live, Studio One, …) is running, \
             quit and restart it so it drops SmartBridge from its \
             plug-in scanner cache."
                .into(),
        );

        let det = detection::main_app::detect().await;
        if errors.is_empty() {
            InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
        } else {
            let mut all = messages;
            all.extend(errors);
            InstallOutcome::err(COMPONENT, all).with_post_state(det)
        }
    }

    enum RemoveOutcome {
        Removed,
        Absent,
        Error(String),
    }

    /// Remove a file or directory. Tries a plain `std::fs::remove*`
    /// first; if that fails with permission denied (the .pkg installer
    /// runs as root and may have left the .app owned by root:wheel),
    /// falls back to `sudo rm -rf` so the customer can authenticate
    /// once via the system prompt.
    fn remove_path(p: &Path) -> RemoveOutcome {
        if !p.exists() {
            return RemoveOutcome::Absent;
        }
        let plain = if p.is_dir() {
            std::fs::remove_dir_all(p)
        } else {
            std::fs::remove_file(p)
        };
        match plain {
            Ok(()) => RemoveOutcome::Removed,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                let status = Command::new("/usr/bin/sudo")
                    .args(["-n", "rm", "-rf", "--"])
                    .arg(p)
                    .status();
                match status {
                    Ok(s) if s.success() => RemoveOutcome::Removed,
                    Ok(s) => RemoveOutcome::Error(format!(
                        "sudo rm -rf exited with {:?} (run Setup from Terminal \
                         after `sudo -v` if the password prompt didn't appear)",
                        s.code()
                    )),
                    Err(e) => RemoveOutcome::Error(format!("spawn sudo: {e}")),
                }
            }
            Err(e) => RemoveOutcome::Error(e.to_string()),
        }
    }
}

#[cfg(target_os = "macos")]
async fn install_macos(asset: &ManifestAsset, outcome: &DownloadOutcome) -> InstallOutcome {
    let mut messages: Vec<String> = vec![
        format!(
            "Downloaded and verified the SmartBridge installer ({} bytes, sha256 {}…)",
            outcome.bytes,
            &outcome.sha256_lc[..16],
        ),
        if asset.signature_required {
            "This installer is digitally signed and was verified.".into()
        } else {
            "Heads-up: this installer is not code-signed. Integrity is enforced \
             by SHA256 verification against the release manifest. Your OS will \
             show a Gatekeeper warning — that is expected."
                .into()
        },
    ];

    match std::process::Command::new("/usr/bin/open")
        .arg(&outcome.local_path)
        .spawn()
    {
        Ok(_) => {
            messages.push(
                "Launched Apple Installer. Complete it in the dialog that opened, \
                 then click \"Check again\" on the Main app card to re-detect."
                    .into(),
            );
        }
        Err(e) => {
            messages.push(format!("Could not launch the installer: {e}"));
            return InstallOutcome::err(COMPONENT, messages);
        }
    }

    let det = detection::main_app::detect().await;
    InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use crate::install::zip_util::extract_zip;
    use crate::paths;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    // What we expect to find at the top level of the zip payload. The
    // build pipeline that produces `SmartBridge-<v>-windows-x64.zip` is
    // responsible for laying these names out exactly.
    const FILE_EXE: &str = "SmartBridge.exe";
    const DIR_VST3: &str = "SmartBridge.vst3";
    const FILE_DB: &str = "smartbridge.enc.db";
    const FILE_CONFIG_DEFAULT: &str = "config.default.json";

    const REG_VST3: &str = r"HKCU\SOFTWARE\VST3\SmartBridge";
    const REG_UNINSTALL: &str =
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\SmartBridge";

    pub async fn install_windows(manifest: &Manifest, outcome: &DownloadOutcome) -> InstallOutcome {
        tracing::info!(
            target: "install.main_app",
            release = %manifest.release_version,
            payload_bytes = outcome.bytes,
            payload_sha256 = %outcome.sha256_lc,
            payload_path = %outcome.local_path.display(),
            "begin Windows install",
        );

        let mut messages: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        messages.push(format!(
            "Downloaded and verified the SmartBridge payload ({} bytes, sha256 {}…)",
            outcome.bytes,
            &outcome.sha256_lc[..16],
        ));

        // 0. Prerequisite: VC++ 2015–2022 Redistributable (x64).
        //
        //    SmartBridge.exe and the VST3 are MSVC-built and link the dynamic
        //    C/C++ runtime. Without this redistributable the customer's DAW
        //    will silently fail to load SmartBridge with an opaque
        //    "entry point not found" error and add it to the blacklist —
        //    which then needs a host-cache wipe to recover from. So we treat
        //    it as a hard prerequisite of placing files: if the redistributable
        //    can't be installed we abort the whole main-app step rather than
        //    leave a half-installed product.
        tracing::info!(target: "install.main_app", "step 0: ensure VC++ Redistributable");
        if let Err(e) = crate::preflight::vcredist::ensure_installed(&mut messages).await {
            tracing::error!(target: "install.main_app", error = %e, "VC++ Redistributable install failed");
            messages.push(e);
            messages.push(
                "Aborting main-app install — the VC++ Redistributable is required \
                 before SmartBridge can be loaded by any DAW."
                    .into(),
            );
            let det = detection::main_app::detect().await;
            return InstallOutcome::err(COMPONENT, messages).with_post_state(det);
        }

        tracing::info!(target: "install.main_app", "step 1: resolve target paths");
        // 1. Resolve target paths (per-user; no UAC needed).
        let install_dir = match paths::windows_main_app_dir() {
            Some(d) => d,
            None => {
                return InstallOutcome::err(
                    COMPONENT,
                    vec!["could not resolve %LOCALAPPDATA%".into()],
                );
            }
        };
        let vst3_root = match paths::windows_vst3_dir() {
            Some(d) => d,
            None => {
                return InstallOutcome::err(
                    COMPONENT,
                    vec!["could not resolve %LOCALAPPDATA%\\Programs\\Common\\VST3".into()],
                );
            }
        };
        let user_data = match paths::user_data_dir() {
            Some(d) => d,
            None => {
                return InstallOutcome::err(
                    COMPONENT,
                    vec!["could not resolve %APPDATA%\\SmartBridge".into()],
                );
            }
        };
        let start_menu = match paths::windows_start_menu_dir() {
            Some(d) => d,
            None => {
                return InstallOutcome::err(
                    COMPONENT,
                    vec!["could not resolve Start Menu folder".into()],
                );
            }
        };
        tracing::info!(
            target: "install.main_app",
            install_dir = %install_dir.display(),
            vst3_root = %vst3_root.display(),
            user_data = %user_data.display(),
            start_menu = %start_menu.display(),
            "resolved per-user install paths",
        );

        // 2. Extract the zip into a process-scoped temp folder.
        let tmp_root = std::env::temp_dir().join(format!(
            "smartbridge-setup-main-app-{}",
            std::process::id()
        ));
        if let Err(e) = std::fs::create_dir_all(&tmp_root) {
            return InstallOutcome::err(
                COMPONENT,
                vec![format!("create temp dir {}: {e}", tmp_root.display())],
            );
        }
        tracing::info!(target: "install.main_app", tmp = %tmp_root.display(), "step 2: extract zip");
        if let Err(e) = extract_zip(&outcome.local_path, &tmp_root) {
            tracing::error!(target: "install.main_app", error = %e, "extract zip failed");
            let _ = std::fs::remove_dir_all(&tmp_root);
            return InstallOutcome::err(COMPONENT, vec![format!("extract zip: {e}")]);
        }
        // Log the top-level zip layout — this is invaluable when the
        // CI pipeline produces a zip that doesn't match what we expect
        // (missing DLL, wrong DB filename, etc.).
        if let Ok(rd) = std::fs::read_dir(&tmp_root) {
            let entries: Vec<String> = rd
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            tracing::info!(
                target: "install.main_app",
                entry_count = entries.len(),
                entries = ?entries,
                "zip top-level layout",
            );
        }
        messages.push(format!("Extracted payload to {}", tmp_root.display()));

        // 3. Place SmartBridge.exe + every runtime DLL the build pipeline
        //    bundled next to it (sqlcipher.dll, libcrypto-3-x64.dll today).
        //    These DLLs MUST live in the same directory as SmartBridge.exe —
        //    Windows' DLL search order resolves the .exe's own folder first
        //    and there's no per-user place we could put them where every
        //    customer would already have them on PATH. Without them, launching
        //    SmartBridge.exe pops the classic
        //      "The code execution cannot proceed because sqlcipher.dll
        //       was not found"
        //    error dialog and the customer is dead in the water.
        tracing::info!(target: "install.main_app", "step 3: place SmartBridge.exe + runtime DLLs");
        if let Err(e) = std::fs::create_dir_all(&install_dir) {
            tracing::error!(target: "install.main_app", dir = %install_dir.display(), error = %e, "create install_dir failed");
            errors.push(format!("create {}: {e}", install_dir.display()));
        }
        let exe_src = tmp_root.join(FILE_EXE);
        let exe_dst = install_dir.join(FILE_EXE);
        if !exe_src.exists() {
            tracing::error!(target: "install.main_app", expected = %exe_src.display(), "payload missing SmartBridge.exe");
            errors.push(format!("payload missing {FILE_EXE}"));
        } else if let Err(e) = replace_file(&exe_src, &exe_dst) {
            tracing::error!(target: "install.main_app", dst = %exe_dst.display(), error = %e, "place SmartBridge.exe failed");
            errors.push(format!("place {}: {e}", exe_dst.display()));
        } else {
            tracing::info!(target: "install.main_app", dst = %exe_dst.display(), "placed SmartBridge.exe");
            messages.push(format!("Placed {}", exe_dst.display()));
        }

        // Copy any *.dll the pipeline staged at the zip root next to
        // SmartBridge.exe. We deliberately do not whitelist specific names:
        // if a future vcpkg revision drags in extra transitive deps, this
        // step keeps shipping them without code changes here.
        match std::fs::read_dir(&tmp_root) {
            Ok(rd) => {
                let mut dll_count = 0usize;
                for entry in rd.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|s| s.to_str()).map(|s| s.eq_ignore_ascii_case("dll")) != Some(true) {
                        continue;
                    }
                    let name = match p.file_name() {
                        Some(n) => n.to_owned(),
                        None => continue,
                    };
                    let dst = install_dir.join(&name);
                    if let Err(e) = replace_file(&p, &dst) {
                        tracing::error!(target: "install.main_app", dll = %name.to_string_lossy(), error = %e, "place DLL failed");
                        errors.push(format!("place {}: {e}", dst.display()));
                    } else {
                        tracing::info!(target: "install.main_app", dll = %name.to_string_lossy(), "placed DLL");
                        dll_count += 1;
                    }
                }
                if dll_count > 0 {
                    messages.push(format!(
                        "Placed {dll_count} runtime DLL(s) alongside {}",
                        exe_dst.display()
                    ));
                } else {
                    tracing::error!(target: "install.main_app", "zip payload contains no runtime DLLs");
                    errors.push(
                        "Payload contains no runtime DLLs at the zip root. \
                         SmartBridge.exe will fail to launch with \
                         'sqlcipher.dll was not found'. Re-publish the \
                         main-app.windows zip from a fresh CI build."
                            .into(),
                    );
                }
            }
            Err(e) => {
                tracing::error!(target: "install.main_app", dir = %tmp_root.display(), error = %e, "scan zip for DLLs failed");
                errors.push(format!("scan {} for DLLs: {e}", tmp_root.display()));
            }
        }

        tracing::info!(target: "install.main_app", "step 4: place VST3 bundle");
        // 4. Place SmartBridge.vst3 directory bundle.
        if let Err(e) = std::fs::create_dir_all(&vst3_root) {
            tracing::error!(target: "install.main_app", dir = %vst3_root.display(), error = %e, "create vst3_root failed");
            errors.push(format!("create {}: {e}", vst3_root.display()));
        }
        let vst3_src = tmp_root.join(DIR_VST3);
        let vst3_dst = vst3_root.join(DIR_VST3);
        if !vst3_src.exists() {
            tracing::error!(target: "install.main_app", expected = %vst3_src.display(), "payload missing VST3 bundle");
            errors.push(format!("payload missing {DIR_VST3}/"));
        } else {
            if vst3_dst.exists() {
                if let Err(e) = std::fs::remove_dir_all(&vst3_dst) {
                    tracing::error!(target: "install.main_app", dst = %vst3_dst.display(), error = %e, "remove old VST3 failed");
                    errors.push(format!("remove existing {}: {e}", vst3_dst.display()));
                }
            }
            if let Err(e) = copy_dir_all(&vst3_src, &vst3_dst) {
                tracing::error!(target: "install.main_app", dst = %vst3_dst.display(), error = %e, "copy VST3 failed");
                errors.push(format!("copy VST3 bundle to {}: {e}", vst3_dst.display()));
            } else {
                tracing::info!(target: "install.main_app", dst = %vst3_dst.display(), "placed VST3 bundle");
                messages.push(format!("Placed VST3 bundle at {}", vst3_dst.display()));
            }
        }

        // 5. User-data dir: replace seed data, preserve real user state.
        //
        //    smartbridge.db is the SQLCipher-encrypted *library* (chord
        //    progressions, voice maps, MIDI clips, syllables) — read-only
        //    seed data the customer never writes to. Its encryption key
        //    is baked into SmartBridge.exe at compile time and the
        //    encrypted DB is built with the same key in the same CI run,
        //    so the file on disk MUST come from the same release as
        //    SmartBridge.exe. The legacy NSIS installer always overwrote
        //    it (NSIS `File` is unconditional); we mirror that behaviour
        //    or the customer ends up with a SmartBridge.exe expecting
        //    one key and a DB encrypted with another, which surfaces as
        //    "no database connection" the moment the app boots.
        //
        //    config.json IS user data — it contains the customer's
        //    lyrics_apiKey and per-feature toggles — so we only seed
        //    the default when no config.json exists yet, and never
        //    overwrite an existing one.
        //
        //    Same for any future user-state files (lyrics/, edits/) the
        //    app writes under %APPDATA%\SmartBridge\ — those stay
        //    untouched on install/upgrade.
        tracing::info!(target: "install.main_app", "step 5: seed user-data dir (DB + default config)");
        if let Err(e) = std::fs::create_dir_all(&user_data) {
            tracing::error!(target: "install.main_app", dir = %user_data.display(), error = %e, "create user_data failed");
            errors.push(format!("create {}: {e}", user_data.display()));
        }
        let db_dst = user_data.join("smartbridge.db");
        let db_src = tmp_root.join(FILE_DB);
        if db_src.exists() {
            let src_size = std::fs::metadata(&db_src).map(|m| m.len()).unwrap_or(0);
            if let Err(e) = replace_file(&db_src, &db_dst) {
                tracing::error!(target: "install.main_app", dst = %db_dst.display(), error = %e, "seed DB failed");
                errors.push(format!("seed DB to {}: {e}", db_dst.display()));
            } else {
                tracing::info!(
                    target: "install.main_app",
                    dst = %db_dst.display(),
                    bytes = src_size,
                    "seeded encrypted DB",
                );
                messages.push(format!(
                    "Installed library DB at {} (this release's SQLCipher key).",
                    db_dst.display()
                ));
            }
        } else {
            tracing::error!(target: "install.main_app", expected = %db_src.display(), "payload missing encrypted DB");
            errors.push(
                "Payload contains no encrypted DB. SmartBridge.exe will fail \
                 to connect; re-publish the main-app.windows zip from a fresh \
                 CI build."
                    .into(),
            );
        }

        let config_dst = user_data.join("config.json");
        let config_src = tmp_root.join(FILE_CONFIG_DEFAULT);
        if config_dst.exists() {
            messages.push(format!(
                "Existing config.json present at {} — left untouched.",
                config_dst.display()
            ));
        } else if config_src.exists() {
            if let Err(e) = std::fs::copy(&config_src, &config_dst) {
                errors.push(format!("seed config.json to {}: {e}", config_dst.display()));
            } else {
                messages.push(format!("Seeded default config at {}", config_dst.display()));
            }
        } else {
            messages.push("Payload contains no default config to seed; skipped.".into());
        }

        // 6. Registry writes (HKCU only).
        let release_version = manifest.release_version.clone();
        let setup_exe = std::env::current_exe()
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let uninstall_cmd = if setup_exe.is_empty() {
            "smartbridge-setup --uninstall main-app".to_string()
        } else {
            format!("\"{setup_exe}\" --uninstall main-app")
        };

        tracing::info!(target: "install.main_app", uninstall_cmd = %uninstall_cmd, "step 6: write HKCU registry");
        if let Err(e) = reg_set_string(REG_VST3, "Path", &vst3_dst.to_string_lossy()) {
            tracing::error!(target: "install.main_app", error = %e, "registry write VST3 Path failed");
            errors.push(format!("registry SOFTWARE\\VST3\\SmartBridge Path: {e}"));
        }
        if let Err(e) = reg_set_string(REG_VST3, "Version", &release_version) {
            errors.push(format!("registry SOFTWARE\\VST3\\SmartBridge Version: {e}"));
        }

        let install_loc_str = install_dir.to_string_lossy().to_string();
        let exe_str = exe_dst.to_string_lossy().to_string();
        let uninstall_pairs: [(&str, &str); 6] = [
            ("DisplayName", "SmartBridge"),
            ("DisplayVersion", release_version.as_str()),
            ("Publisher", "SmartBridge"),
            ("InstallLocation", install_loc_str.as_str()),
            ("DisplayIcon", exe_str.as_str()),
            ("UninstallString", uninstall_cmd.as_str()),
        ];
        for (value, data) in uninstall_pairs {
            if let Err(e) = reg_set_string(REG_UNINSTALL, value, data) {
                errors.push(format!("registry Uninstall {value}: {e}"));
            }
        }
        for (value, data) in [("NoModify", 1u32), ("NoRepair", 1u32)] {
            if let Err(e) = reg_set_dword(REG_UNINSTALL, value, data) {
                errors.push(format!("registry Uninstall {value}: {e}"));
            }
        }
        messages.push("Wrote HKCU registry entries (VST3 + Add/Remove Programs).".into());

        tracing::info!(target: "install.main_app", "step 7: create Start Menu shortcuts");
        // 7. Start Menu shortcuts (per-user).
        let sb_dir = start_menu.join("SmartBridge");
        if let Err(e) = std::fs::create_dir_all(&sb_dir) {
            errors.push(format!("create {}: {e}", sb_dir.display()));
        } else {
            let main_lnk = sb_dir.join("SmartBridge.lnk");
            match create_shortcut(&main_lnk, &exe_dst, &[], "SmartBridge") {
                Ok(()) => messages.push(format!("Created shortcut {}", main_lnk.display())),
                Err(e) => errors.push(format!("create {}: {e}", main_lnk.display())),
            }
            if !setup_exe.is_empty() {
                let unin_lnk = sb_dir.join("Uninstall SmartBridge.lnk");
                let setup_path = PathBuf::from(&setup_exe);
                match create_shortcut(
                    &unin_lnk,
                    &setup_path,
                    &["--uninstall", "main-app"],
                    "Uninstall SmartBridge",
                ) {
                    Ok(()) => messages.push(format!("Created shortcut {}", unin_lnk.display())),
                    Err(e) => errors.push(format!("create {}: {e}", unin_lnk.display())),
                }
            }
        }

        // 8. Clean up the extraction tmp.
        let _ = std::fs::remove_dir_all(&tmp_root);

        tracing::info!(target: "install.main_app", "step 9: re-detect");
        // 9. Re-detect.
        let det = detection::main_app::detect().await;
        tracing::info!(
            target: "install.main_app",
            error_count = errors.len(),
            "Windows install finished",
        );

        if errors.is_empty() {
            InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
        } else {
            let mut all = messages;
            all.extend(errors);
            InstallOutcome::err(COMPONENT, all).with_post_state(det)
        }
    }

    pub async fn uninstall_windows(remove_user_data: bool) -> InstallOutcome {
        let mut messages: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        let install_dir = paths::windows_main_app_dir();
        let vst3_root = paths::windows_vst3_dir();
        let user_data = paths::user_data_dir();
        let start_menu = paths::windows_start_menu_dir();

        // 1. Remove the install dir (SmartBridge.exe + .old + any other
        //    files the installer dropped there).
        if let Some(dir) = install_dir.as_ref() {
            if dir.exists() {
                match std::fs::remove_dir_all(dir) {
                    Ok(()) => messages.push(format!("Removed {}", dir.display())),
                    Err(e) => errors.push(format!("remove {}: {e}", dir.display())),
                }
            } else {
                messages.push(format!("(no install dir at {})", dir.display()));
            }
        }

        // 2. Remove the VST3 bundle.
        if let Some(root) = vst3_root.as_ref() {
            let vst3_dst = root.join(DIR_VST3);
            if vst3_dst.exists() {
                match std::fs::remove_dir_all(&vst3_dst) {
                    Ok(()) => messages.push(format!("Removed {}", vst3_dst.display())),
                    Err(e) => errors.push(format!("remove {}: {e}", vst3_dst.display())),
                }
            }
        }

        // 3. Remove HKCU registry entries.
        for key in [REG_VST3, REG_UNINSTALL] {
            match reg_delete_key(key) {
                Ok(true) => messages.push(format!("Removed registry {key}")),
                Ok(false) => messages.push(format!("(no registry key {key})")),
                Err(e) => errors.push(format!("remove registry {key}: {e}")),
            }
        }

        // 4. Remove Start Menu shortcuts.
        if let Some(sm) = start_menu.as_ref() {
            let sb_dir = sm.join("SmartBridge");
            if sb_dir.exists() {
                match std::fs::remove_dir_all(&sb_dir) {
                    Ok(()) => messages.push(format!("Removed Start Menu folder {}", sb_dir.display())),
                    Err(e) => errors.push(format!("remove start menu {}: {e}", sb_dir.display())),
                }
            }
        }

        // 5. Optionally remove user data. Default is OFF — destroying a
        //    customer's database/lyrics on a routine uninstall would be
        //    a worse mistake than leaving it behind. The UI ticks the
        //    user-data checkbox off by design.
        if let Some(data) = user_data.as_ref() {
            if remove_user_data {
                if data.exists() {
                    match std::fs::remove_dir_all(data) {
                        Ok(()) => messages.push(format!(
                            "Deleted SmartBridge user data at {}",
                            data.display()
                        )),
                        Err(e) => errors.push(format!("remove user data {}: {e}", data.display())),
                    }
                }
            } else if data.exists() {
                messages.push(format!(
                    "Kept your SmartBridge data at {} (database, config, lyrics). \
                     Re-install will pick it up automatically.",
                    data.display()
                ));
            }
        }

        let det = detection::main_app::detect().await;
        if errors.is_empty() {
            InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
        } else {
            let mut all = messages;
            all.extend(errors);
            InstallOutcome::err(COMPONENT, all).with_post_state(det)
        }
    }

    /// Returns Ok(true) if the key was deleted, Ok(false) if it didn't
    /// exist (`reg delete` exits with code 1 in that case).
    fn reg_delete_key(key: &str) -> Result<bool, String> {
        let out = Command::new("reg")
            .args(["delete", key, "/f"])
            .output()
            .map_err(|e| format!("spawn reg: {e}"))?;
        if out.status.success() {
            Ok(true)
        } else {
            // `reg delete` returns 1 when the key doesn't exist. Inspect
            // stderr to distinguish that from a real failure (e.g. ACL
            // problem).
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("unable to find") || stderr.contains("cannot find") {
                Ok(false)
            } else {
                Err(format!(
                    "reg delete exited with code {:?}: {}",
                    out.status.code(),
                    stderr.trim()
                ))
            }
        }
    }

    /// Best-effort overwrite of a file that may currently be open by
    /// another process (the running SmartBridge.exe, the loaded
    /// sqlcipher.dll while the standalone is running, a host scanning
    /// the VST3, etc.). Falls back to "rename existing aside, then
    /// write" so the new file lands even when the Win32 file lock
    /// blocks the straight overwrite. The aside file (`<name>.old`) is
    /// harmless and Windows cleans it up the next time the user
    /// restarts the host process / reboots.
    fn replace_file(src: &Path, dst: &Path) -> std::io::Result<()> {
        match std::fs::copy(src, dst) {
            Ok(_) => Ok(()),
            Err(_) => {
                // Append `.old` to the *full* destination filename so the
                // suffix preserves the original extension. Using
                // `with_extension` here would replace the extension
                // entirely (e.g. sqlcipher.dll -> sqlcipher.old) which is
                // confusing and could collide between file families.
                let aside = match dst.file_name() {
                    Some(name) => {
                        let mut s = name.to_os_string();
                        s.push(".old");
                        dst.with_file_name(s)
                    }
                    None => dst.with_extension("old"),
                };
                let _ = std::fs::remove_file(&aside);
                if dst.exists() {
                    std::fs::rename(dst, &aside)?;
                }
                std::fs::copy(src, dst).map(|_| ())
            }
        }
    }

    fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let from = entry.path();
            let to = dst.join(entry.file_name());
            if ty.is_dir() {
                copy_dir_all(&from, &to)?;
            } else {
                std::fs::copy(&from, &to)?;
            }
        }
        Ok(())
    }

    fn reg_set_string(key: &str, value: &str, data: &str) -> Result<(), String> {
        let status = Command::new("reg")
            .args(["add", key, "/v", value, "/t", "REG_SZ", "/d", data, "/f"])
            .status()
            .map_err(|e| format!("spawn reg: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("reg add exited with code {:?}", status.code()))
        }
    }

    fn reg_set_dword(key: &str, value: &str, data: u32) -> Result<(), String> {
        let data_str = data.to_string();
        let status = Command::new("reg")
            .args([
                "add", key, "/v", value, "/t", "REG_DWORD", "/d", &data_str, "/f",
            ])
            .status()
            .map_err(|e| format!("spawn reg: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("reg add exited with code {:?}", status.code()))
        }
    }

    /// Single-quote a string for embedding in a PowerShell command.
    /// PowerShell single-quoted strings are literal, so the only
    /// character we need to escape is the single quote itself, which
    /// doubles to `''` (PowerShell convention).
    fn ps_quote(s: &str) -> String {
        format!("'{}'", s.replace('\'', "''"))
    }

    /// Create a Windows .lnk shortcut via WScript.Shell. Used for both
    /// the main "SmartBridge" shortcut and the "Uninstall SmartBridge"
    /// shortcut. We deliberately avoid pulling a `winapi`/`windows`
    /// crate just to call IShellLink — PowerShell + WScript.Shell is
    /// already on every supported Windows host and matches the legacy
    /// installer's mechanism.
    fn create_shortcut(
        lnk_path: &Path,
        target: &Path,
        args: &[&str],
        description: &str,
    ) -> Result<(), String> {
        let args_joined = args
            .iter()
            .map(|a| {
                if a.contains(' ') {
                    format!("\"{a}\"")
                } else {
                    (*a).to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        let working_dir = target
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let script = format!(
            "$ws = New-Object -ComObject WScript.Shell; \
             $lnk = $ws.CreateShortcut({lnk}); \
             $lnk.TargetPath = {tgt}; \
             $lnk.Arguments = {args}; \
             $lnk.WorkingDirectory = {wd}; \
             $lnk.Description = {desc}; \
             $lnk.Save()",
            lnk = ps_quote(&lnk_path.to_string_lossy()),
            tgt = ps_quote(&target.to_string_lossy()),
            args = ps_quote(&args_joined),
            wd = ps_quote(&working_dir),
            desc = ps_quote(description),
        );
        let status = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .status()
            .map_err(|e| format!("spawn powershell: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("powershell exited with code {:?}", status.code()))
        }
    }
}
