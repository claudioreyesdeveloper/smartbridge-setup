//! loopMIDI virtual MIDI ports for the Cubase Rename feature.
//!
//! SmartBridge needs two virtual MIDI ports for Cubase Rename to work on
//! Windows. The install action downloads loopMIDI silently from
//! tobias-erichsen.de and creates the named ports. This module just
//! detects whether they're already there.
//!
//! Detection rules:
//!   * macOS: NotAvailableInBuild — the Cubase Rename feature uses
//!     IAC ports on macOS, not loopMIDI.
//!   * Windows, loopMIDI.exe missing → NotInstalled.
//!   * Windows, loopMIDI.exe present, both named ports detected via
//!     winmm midiOutGetDevCapsW → Ready.
//!   * Windows, loopMIDI.exe present, one or both ports missing →
//!     NeedsRepair (install action will create them).

use super::DetectionResult;

pub const PORT_OUT_NAME: &str = "SmartBridge Genos Rename";
pub const PORT_IN_NAME: &str = "SmartBridge Genos Rename Reply";

pub async fn detect() -> DetectionResult {
    #[cfg(not(target_os = "windows"))]
    {
        DetectionResult::not_available_in_build()
    }

    #[cfg(target_os = "windows")]
    {
        match find_loopmidi_exe() {
            None => DetectionResult::not_installed()
                .with_detail("loopMIDI.exe not found at the standard Tobias Erichsen install path"),
            Some(exe) => {
                let outs = list_midi_output_names().unwrap_or_default();
                let ins = list_midi_input_names().unwrap_or_default();
                let has_out = outs.iter().any(|n| n.contains(PORT_OUT_NAME));
                let has_in = ins.iter().any(|n| n.contains(PORT_IN_NAME));

                let mut det = if has_out && has_in {
                    DetectionResult::ready()
                } else {
                    DetectionResult::needs_repair(format!(
                        "loopMIDI is installed but the SmartBridge ports are missing ({}). \
                         Click Install to create them.",
                        match (has_out, has_in) {
                            (false, false) => "neither output nor input port",
                            (false, true) => "output port missing",
                            (true, false) => "input port missing",
                            (true, true) => unreachable!(),
                        }
                    ))
                };
                det = det.with_detail(format!("loopMIDI.exe: {}", exe.display()));
                det = det.with_detail(format!("output port present: {has_out}"));
                det = det.with_detail(format!("input port present: {has_in}"));
                det
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn find_loopmidi_exe() -> Option<std::path::PathBuf> {
    let candidates = [
        std::env::var("ProgramFiles").ok().map(std::path::PathBuf::from),
        std::env::var("ProgramFiles(x86)").ok().map(std::path::PathBuf::from),
    ];
    for base in candidates.into_iter().flatten() {
        let exe = base
            .join("Tobias Erichsen")
            .join("loopMIDI")
            .join("loopMIDI.exe");
        if exe.exists() {
            return Some(exe);
        }
    }
    None
}

// =============================================================================
// winmm MIDI device enumeration (Windows only)
// =============================================================================

#[cfg(target_os = "windows")]
fn list_midi_output_names() -> Option<Vec<String>> {
    #[repr(C)]
    struct MIDIOUTCAPSW {
        w_mid: u16,
        w_pid: u16,
        v_driver_version: u32,
        sz_pname: [u16; 32],
        w_technology: u16,
        w_voices: u16,
        w_notes: u16,
        w_channel_mask: u16,
        dw_support: u32,
    }

    extern "system" {
        fn midiOutGetNumDevs() -> u32;
        fn midiOutGetDevCapsW(u_device_id: usize, pmoc: *mut MIDIOUTCAPSW, cbmoc: u32) -> u32;
    }

    unsafe {
        let n = midiOutGetNumDevs();
        let mut out = Vec::with_capacity(n as usize);
        for i in 0..n {
            let mut caps: MIDIOUTCAPSW = std::mem::zeroed();
            let rc = midiOutGetDevCapsW(
                i as usize,
                &mut caps as *mut _,
                std::mem::size_of::<MIDIOUTCAPSW>() as u32,
            );
            if rc != 0 {
                continue;
            }
            out.push(wide_to_string(&caps.sz_pname));
        }
        Some(out)
    }
}

#[cfg(target_os = "windows")]
fn list_midi_input_names() -> Option<Vec<String>> {
    #[repr(C)]
    struct MIDIINCAPSW {
        w_mid: u16,
        w_pid: u16,
        v_driver_version: u32,
        sz_pname: [u16; 32],
        dw_support: u32,
    }

    extern "system" {
        fn midiInGetNumDevs() -> u32;
        fn midiInGetDevCapsW(u_device_id: usize, pmic: *mut MIDIINCAPSW, cbmic: u32) -> u32;
    }

    unsafe {
        let n = midiInGetNumDevs();
        let mut out = Vec::with_capacity(n as usize);
        for i in 0..n {
            let mut caps: MIDIINCAPSW = std::mem::zeroed();
            let rc = midiInGetDevCapsW(
                i as usize,
                &mut caps as *mut _,
                std::mem::size_of::<MIDIINCAPSW>() as u32,
            );
            if rc != 0 {
                continue;
            }
            out.push(wide_to_string(&caps.sz_pname));
        }
        Some(out)
    }
}

#[cfg(target_os = "windows")]
fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}
