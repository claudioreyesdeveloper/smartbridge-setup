//! Yamaha Steinberg USB Driver install action (Windows).
//!
//! We do not redistribute Yamaha's driver. On Windows 10/11 we use winget
//! to install the official package (`Yamaha.SteinbergUSBDriver`) silently
//! when possible. If winget is unavailable or fails, Setup surfaces a
//! plain error and the support bundle contains the exact command output.

use super::InstallOutcome;
use crate::detection;

const COMPONENT: &str = "yamaha-steinberg-driver";

pub async fn install() -> InstallOutcome {
    #[cfg(not(target_os = "windows"))]
    {
        InstallOutcome::ok(
            COMPONENT,
            vec!["Yamaha Steinberg USB Driver is only needed on Windows.".into()],
        )
        .with_post_state(detection::DetectionResult::not_available_in_build())
    }

    #[cfg(target_os = "windows")]
    {
        windows_impl::install_windows().await
    }
}

pub async fn remove() -> InstallOutcome {
    InstallOutcome::ok(
        COMPONENT,
        vec!["Setup does not remove the Yamaha Steinberg USB Driver.".into()],
    )
    .with_post_state(detection::yamaha_steinberg::detect().await)
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use std::process::Command;

    pub async fn install_windows() -> InstallOutcome {
        let before = detection::yamaha_steinberg::detect().await;
        if before.state == crate::ComponentState::Ready {
            return InstallOutcome::ok(
                COMPONENT,
                vec!["Yamaha Steinberg USB Driver is already installed.".into()],
            )
            .with_post_state(before);
        }

        let output = match Command::new("winget")
            .args([
                "install",
                "-e",
                "--id",
                "Yamaha.SteinbergUSBDriver",
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
                "--disable-interactivity",
            ])
            .output()
        {
            Ok(output) => output,
            Err(e) => {
                return InstallOutcome::err(
                    COMPONENT,
                    vec![format!(
                        "Could not start winget to install Yamaha Steinberg USB Driver: {e}"
                    )],
                )
                .with_post_state(before);
            }
        };

        let after = detection::yamaha_steinberg::detect().await;
        if output.status.success() || after.state == crate::ComponentState::Ready {
            return InstallOutcome::ok(
                COMPONENT,
                vec!["Yamaha Steinberg USB Driver is installed.".into()],
            )
            .with_post_state(after);
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        InstallOutcome::err(
            COMPONENT,
            vec![
                "Could not install Yamaha Steinberg USB Driver automatically.".into(),
                format!("winget exit code: {:?}", output.status.code()),
                format!("winget output: {}", stdout.trim()),
                format!("winget error: {}", stderr.trim()),
            ],
        )
        .with_post_state(after)
    }
}
