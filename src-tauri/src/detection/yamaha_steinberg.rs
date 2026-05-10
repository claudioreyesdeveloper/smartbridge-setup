//! Yamaha Steinberg USB Driver detection (Windows).
//!
//! SmartBridge can talk to Yamaha arranger keyboards over USB MIDI. On
//! Windows that normally requires Yamaha's signed USB driver. Detection is
//! intentionally conservative: if the official installed-program entry is
//! present, we treat the driver as ready. Otherwise the installer can try
//! to install it via winget.

use super::DetectionResult;

pub async fn detect() -> DetectionResult {
    #[cfg(not(target_os = "windows"))]
    {
        DetectionResult::not_available_in_build()
    }

    #[cfg(target_os = "windows")]
    {
        match find_installed_driver() {
            Some(version) => DetectionResult::ready()
                .with_version(version)
                .with_detail("Yamaha Steinberg USB Driver is installed."),
            None => DetectionResult::not_installed()
                .with_detail("Yamaha Steinberg USB Driver was not found in Windows Apps."),
        }
    }
}

#[cfg(target_os = "windows")]
fn find_installed_driver() -> Option<String> {
    let roots = [
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    ];

    for root in roots {
        let out = std::process::Command::new("reg")
            .args(["query", root, "/s"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        if !text.to_lowercase().contains("yamaha steinberg usb driver") {
            continue;
        }

        let version = text
            .lines()
            .find(|line| line.contains("DisplayVersion"))
            .and_then(|line| line.split_whitespace().last())
            .unwrap_or("installed");
        return Some(version.to_string());
    }

    None
}
