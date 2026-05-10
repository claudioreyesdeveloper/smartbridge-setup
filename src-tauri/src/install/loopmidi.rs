//! loopMIDI install action.
//!
//! Behaviour:
//!
//!   1. If loopMIDI.exe is missing, download
//!      `loopMIDISetup_1_0_16_27.zip` from tobias-erichsen.de, extract it,
//!      and run `loopMIDISetup.exe /SP- /VERYSILENT /SUPPRESSMSGBOX
//!      /NORESTART /SUPPRESSAPPLAUNCH`.
//!   2. Once loopMIDI.exe is present, call it twice with `-new` to create
//!      the two SmartBridge ports the Cubase Rename feature relies on.
//!   3. Add an HKCU `Run` entry so loopMIDI auto-starts at logon (so the
//!      ports are alive whenever Cubase is opened).
//!
//! Strictly Windows. On macOS this returns a friendly "not applicable"
//! outcome — the macOS Cubase Rename feature uses IAC ports, set up
//! separately by the user in Audio MIDI Setup.

use super::InstallOutcome;
use crate::detection;

#[cfg(target_os = "windows")]
use crate::detection::loopmidi::{find_loopmidi_exe, PORT_IN_NAME, PORT_OUT_NAME};

use tauri::AppHandle;

const COMPONENT: &str = "windows-loopmidi";

#[cfg(target_os = "windows")]
const LOOPMIDI_ZIP_URL: &str =
    "https://www.tobias-erichsen.de/wp-content/uploads/2020/01/loopMIDISetup_1_0_16_27.zip";

pub async fn install(_app: &AppHandle) -> InstallOutcome {
    #[cfg(not(target_os = "windows"))]
    {
        InstallOutcome::ok(
            COMPONENT,
            vec![
                "loopMIDI is Windows-only. On macOS the Cubase Rename feature uses Apple IAC \
                 ports — open Audio MIDI Setup → MIDI Studio → IAC Driver and enable two ports \
                 named 'SmartBridge Genos Rename' and 'SmartBridge Genos Rename Reply'."
                    .into(),
            ],
        )
        .with_post_state(detection::DetectionResult::not_available_in_build())
    }

    #[cfg(target_os = "windows")]
    {
        windows_impl::install_windows().await
    }
}

/// Remove the SmartBridge-specific virtual ports and the HKCU autostart
/// entry. We deliberately do NOT uninstall loopMIDI itself — it's a
/// third-party tool the customer may use for other things.
pub async fn remove() -> InstallOutcome {
    #[cfg(not(target_os = "windows"))]
    {
        InstallOutcome::ok(
            COMPONENT,
            vec!["loopMIDI is Windows-only — nothing to remove on this OS.".into()],
        )
        .with_post_state(detection::DetectionResult::not_available_in_build())
    }

    #[cfg(target_os = "windows")]
    {
        windows_impl::remove_windows().await
    }
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use crate::commands::ComponentState;
    use crate::install::zip_util::extract_zip;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    pub async fn install_windows() -> InstallOutcome {
        tracing::info!(target: "install.loopmidi", "begin Windows install");

        let mut messages: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        // Step 1: ensure loopMIDI is installed.
        tracing::info!(target: "install.loopmidi", "step 1: locate or install loopMIDI.exe");
        let exe = match find_loopmidi_exe() {
            Some(p) => {
                tracing::info!(target: "install.loopmidi", path = %p.display(), "loopMIDI already installed");
                messages.push(format!("loopMIDI already installed: {}", p.display()));
                p
            }
            None => {
                tracing::info!(target: "install.loopmidi", url = LOOPMIDI_ZIP_URL, "downloading loopMIDI from tobias-erichsen.de");
                messages.push("loopMIDI not found — downloading from tobias-erichsen.de…".into());
                match download_and_install_loopmidi().await {
                    Ok(p) => {
                        tracing::info!(target: "install.loopmidi", path = %p.display(), "loopMIDI installed");
                        messages.push(format!("loopMIDI installed: {}", p.display()));
                        p
                    }
                    Err(e) => {
                        tracing::error!(target: "install.loopmidi", error = %e, "loopMIDI install failed");
                        errors.push(format!("loopMIDI install failed: {e}"));
                        let det = detection::loopmidi::detect().await;
                        let mut all = messages;
                        all.extend(errors);
                        return InstallOutcome::err(COMPONENT, all).with_post_state(det);
                    }
                }
            }
        };

        // Step 2: ensure loopMIDI is *running*. This is the subtle bit
        // that the legacy NSIS installer got right by accident: it ran
        // loopMIDISetup without `/SUPPRESSAPPLAUNCH`, which means Inno
        // Setup launched loopMIDI on the user's session right after
        // install. Once running, loopMIDI is single-instance: subsequent
        // `loopMIDI.exe -new "<port>"` calls connect to the running tray
        // instance over IPC and the port is created. Without a running
        // instance, the `-new` call exits immediately and the port is
        // silently lost — the symptom the customer sees as "the
        // SmartBridge ports were never created".
        //
        // We use `-autostart` here (loopMIDI's own flag for "launch
        // minimized to tray") and spawn (don't wait), so the install
        // step doesn't block waiting for the GUI to close. A short
        // settle delay gives loopMIDI's IPC server time to come up
        // before we send the port-creation commands.
        tracing::info!(target: "install.loopmidi", "step 2: ensure loopMIDI tray instance is running");
        if let Err(e) = ensure_loopmidi_running(&exe).await {
            tracing::error!(target: "install.loopmidi", error = %e, "ensure_loopmidi_running failed");
            errors.push(format!("could not start loopMIDI: {e}"));
        } else {
            tracing::info!(target: "install.loopmidi", "loopMIDI is running");
            messages.push("loopMIDI is running (background tray instance)".into());
        }

        // Step 3: create the two named virtual ports. loopMIDI dedupes
        // by name, so re-running the create call when the port already
        // exists is harmless. We invoke it twice so an error on one
        // port doesn't prevent the other from being created.
        tracing::info!(target: "install.loopmidi", "step 3: create SmartBridge virtual ports");
        for port in [PORT_OUT_NAME, PORT_IN_NAME] {
            match Command::new(&exe).arg("-new").arg(port).status() {
                Ok(status) if status.success() => {
                    tracing::info!(target: "install.loopmidi", port = %port, "created virtual port");
                    messages.push(format!("created virtual port: {port}"));
                }
                Ok(status) => {
                    tracing::error!(target: "install.loopmidi", port = %port, code = ?status.code(), "loopMIDI -new exited non-zero");
                    errors.push(format!(
                        "loopMIDI -new {port} exited with code {:?}",
                        status.code()
                    ));
                }
                Err(e) => {
                    tracing::error!(target: "install.loopmidi", port = %port, error = %e, "spawn loopMIDI -new failed");
                    errors.push(format!("failed to spawn loopMIDI -new {port}: {e}"));
                }
            }
        }

        // Step 4: register HKCU autostart so the ports survive a reboot.
        // The legacy NSIS section uses `WriteRegStr HKCU "...Run" "loopMIDI" '"<exe>" -autostart'`.
        // We mirror that exactly with `reg add`. HKCU writes don't need
        // elevation so this works from the unelevated Setup process.
        tracing::info!(target: "install.loopmidi", "step 4: register HKCU autostart");
        match register_autostart(&exe) {
            Ok(()) => {
                tracing::info!(target: "install.loopmidi", "loopMIDI registered to start at login");
                messages.push("registered loopMIDI to start at login".into());
            }
            Err(e) => {
                tracing::error!(target: "install.loopmidi", error = %e, "autostart registration failed");
                errors.push(format!("autostart registration failed: {e}"));
            }
        }

        // Final proof: do not claim success unless Windows can actually see
        // the two SmartBridge MIDI ports. If loopMIDI's command-line creation
        // did nothing on this machine, stop here and ask for manual creation.
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        let det = detection::loopmidi::detect().await;
        if det.state != ComponentState::Ready {
            messages.extend(det.details.iter().cloned());
            errors.push(manual_port_creation_message());
        }

        if errors.is_empty() {
            InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
        } else {
            let mut all = messages;
            all.extend(errors);
            InstallOutcome::err(COMPONENT, all).with_post_state(det)
        }
    }


    fn manual_port_creation_message() -> String {
        format!(
            "loopMIDI is installed, but SmartBridge could not create the two ports automatically. Open loopMIDI, add a port named '{out}', add a second port named '{inp}', then run SmartBridge Setup again.",
            out = PORT_OUT_NAME,
            inp = PORT_IN_NAME
        )
    }

    async fn download_and_install_loopmidi() -> Result<PathBuf, String> {
        let tmp_root = std::env::temp_dir().join(format!(
            "smartbridge-setup-loopmidi-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&tmp_root)
            .map_err(|e| format!("create temp dir {}: {e}", tmp_root.display()))?;

        let zip_path = tmp_root.join("loopMIDISetup.zip");

        // Download the zip. We use reqwest directly (rather than
        // download::fetch_with_verify) because the loopMIDI archive isn't
        // published in the SmartBridge release manifest and we can't
        // pin a SHA without baking in a Tobias Erichsen build hash that
        // would silently break if he ever re-releases. Mirrors the
        // unverified `NSISdl::download` call in the NSIS installer.
        let bytes = reqwest::Client::builder()
            .build()
            .map_err(|e| format!("http client: {e}"))?
            .get(LOOPMIDI_ZIP_URL)
            .send()
            .await
            .map_err(|e| format!("download {LOOPMIDI_ZIP_URL}: {e}"))?
            .error_for_status()
            .map_err(|e| format!("download {LOOPMIDI_ZIP_URL}: {e}"))?
            .bytes()
            .await
            .map_err(|e| format!("read body: {e}"))?;

        std::fs::write(&zip_path, &bytes)
            .map_err(|e| format!("write {}: {e}", zip_path.display()))?;

        // Extract. The official zip contains a single file
        // `loopMIDISetup_1_0_16_27.exe` (Inno Setup installer).
        let extract_dir = tmp_root.join("extracted");
        std::fs::create_dir_all(&extract_dir)
            .map_err(|e| format!("create {}: {e}", extract_dir.display()))?;
        extract_zip(&zip_path, &extract_dir)?;

        let setup_exe = find_setup_exe(&extract_dir)
            .ok_or_else(|| "loopMIDISetup_*.exe not found inside zip".to_string())?;

        // Run silent install. Inno Setup honours these flags. The user
        // will see one UAC prompt for the embedded MSI service the
        // installer registers; that's expected and identical to the
        // legacy NSIS installer behaviour.
        let status = Command::new(&setup_exe)
            .args([
                "/SP-",
                "/VERYSILENT",
                "/SUPPRESSMSGBOX",
                "/NORESTART",
                "/SUPPRESSAPPLAUNCH",
            ])
            .status()
            .map_err(|e| format!("spawn loopMIDISetup: {e}"))?;

        if !status.success() {
            return Err(format!(
                "loopMIDISetup exited with code {:?}",
                status.code()
            ));
        }

        // Wait briefly and re-resolve the install path. The Inno Setup
        // installer exits after the on-disk copy finishes but before the
        // shell registers the new file's path entries; a short retry loop
        // avoids a spurious "loopMIDI.exe not found" right after install.
        for _ in 0..10 {
            if let Some(exe) = find_loopmidi_exe() {
                let _ = std::fs::remove_dir_all(&tmp_root);
                return Ok(exe);
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }

        Err("loopMIDI installer ran but loopMIDI.exe still not found".into())
    }

    pub async fn remove_windows() -> InstallOutcome {
        let mut messages: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        // Remove the two SmartBridge virtual ports if loopMIDI is around
        // to do it. If loopMIDI itself is gone, the ports went with it
        // and there's nothing to clean.
        if let Some(exe) = find_loopmidi_exe() {
            for port in [PORT_OUT_NAME, PORT_IN_NAME] {
                match Command::new(&exe).arg("-delete").arg(port).status() {
                    Ok(status) if status.success() => {
                        messages.push(format!("removed virtual port: {port}"));
                    }
                    Ok(status) => {
                        // loopMIDI returns non-zero when the port is
                        // already absent; treat that as fine.
                        messages.push(format!(
                            "loopMIDI -delete {port} returned {:?} (port may already be gone)",
                            status.code()
                        ));
                    }
                    Err(e) => {
                        errors.push(format!("failed to spawn loopMIDI -delete {port}: {e}"));
                    }
                }
            }
        } else {
            messages
                .push("loopMIDI.exe not found — assuming SmartBridge ports are already gone.".into());
        }

        // Remove the HKCU autostart entry. `reg delete` exits 1 if the
        // value already doesn't exist; treat that as success.
        let out = Command::new("reg")
            .args([
                "delete",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "loopMIDI",
                "/f",
            ])
            .output();
        match out {
            Ok(o) if o.status.success() => {
                messages.push("removed loopMIDI autostart entry".into());
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                if stderr.contains("unable to find") || stderr.contains("cannot find") {
                    messages.push("(no loopMIDI autostart entry to remove)".into());
                } else {
                    errors.push(format!(
                        "reg delete loopMIDI exited {:?}: {}",
                        o.status.code(),
                        stderr.trim()
                    ));
                }
            }
            Err(e) => errors.push(format!("spawn reg: {e}")),
        }

        let det = detection::loopmidi::detect().await;
        if errors.is_empty() {
            InstallOutcome::ok(COMPONENT, messages).with_post_state(det)
        } else {
            let mut all = messages;
            all.extend(errors);
            InstallOutcome::err(COMPONENT, all).with_post_state(det)
        }
    }

    /// Check whether a loopMIDI tray instance is already running on the
    /// current Windows session. Uses `tasklist /FI "IMAGENAME eq
    /// loopMIDI.exe" /FO CSV /NH` rather than a Win32 process snapshot
    /// so we don't need to add a windows-specific dependency for one
    /// query. The CSV output is `"loopMIDI.exe","<pid>",…` when a
    /// matching process exists, and `INFO: No tasks…` otherwise.
    fn loopmidi_is_running() -> bool {
        let out = Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq loopMIDI.exe", "/FO", "CSV", "/NH"])
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let s = String::from_utf8_lossy(&o.stdout);
                s.to_lowercase().contains("loopmidi.exe")
            }
            _ => false,
        }
    }

    /// Best-effort: make sure a loopMIDI tray instance is alive on this
    /// Windows session before we try to create ports. If nothing is
    /// running, spawn `loopMIDI.exe -autostart` (loopMIDI's own
    /// "minimized to tray" flag) detached and give it a moment to come
    /// up so its IPC server is ready when the subsequent `-new` calls
    /// arrive. Returns Ok(()) even when loopMIDI was already running —
    /// the only failure path is "could not even spawn the process",
    /// which is what we propagate to the caller.
    async fn ensure_loopmidi_running(exe: &Path) -> Result<(), String> {
        if loopmidi_is_running() {
            return Ok(());
        }
        Command::new(exe)
            .arg("-autostart")
            .spawn()
            .map_err(|e| format!("spawn loopMIDI: {e}"))?;
        // Settle delay: loopMIDI typically registers its single-instance
        // mutex + IPC server within ~500ms of process start. 1.5s gives
        // a comfortable margin on a busy machine without making the
        // install feel sluggish.
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        Ok(())
    }

    fn find_setup_exe(dir: &Path) -> Option<PathBuf> {
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()).unwrap_or("") == "exe" {
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    let lower = name.to_ascii_lowercase();
                    if lower.starts_with("loopmidisetup") {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    fn register_autostart(exe: &Path) -> Result<(), String> {
        // `reg add` with /f overwrites silently if the value already exists.
        // The data string must be quoted on its own to keep the embedded
        // spaces in the program path intact when Windows expands it.
        let value_data = format!("\"{}\" -autostart", exe.display());
        let status = Command::new("reg")
            .args([
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "loopMIDI",
                "/t",
                "REG_SZ",
                "/d",
            ])
            .arg(&value_data)
            .arg("/f")
            .status()
            .map_err(|e| format!("spawn reg: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("reg add exited with code {:?}", status.code()))
        }
    }
}
