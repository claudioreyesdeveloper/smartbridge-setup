//! PE import walker: read every DLL the SmartBridge .vst3 / .exe imports
//! and confirm Windows can actually load each one.
//!
//! This is the single most useful preflight check: it catches the real
//! reason hosts blacklist a plugin on Windows. If `sqlcipher.dll`,
//! `libcrypto-3-x64.dll` or one of the VC++ runtime DLLs is missing,
//! `LoadLibraryEx` returns NULL and the host scanner just sees "could
//! not load".
//!
//! Implementation:
//!   1. Open the PE file with `goblin` and enumerate the import table.
//!   2. Skip api-ms-win-* set names (always provided by Windows).
//!   3. Skip kernel32, user32, gdi32, ole32, shell32, advapi32 etc.
//!      (Windows itself; if these are missing, the OS itself is broken).
//!   4. For everything else, try `LoadLibraryExW(LOAD_LIBRARY_AS_DATAFILE)`
//!      with the plugin's directory added to the DLL search path. If it
//!      fails, name that DLL in the failure explanation.

use super::PreflightCheck;
#[cfg(target_os = "windows")]
use super::FixAction;
use std::path::Path;
#[cfg(target_os = "windows")]
use std::path::PathBuf;

pub async fn check_vst3_imports() -> PreflightCheck {
    let inner = match super::basics::first_well_formed_vst3_inner() {
        Some(p) => p,
        None => {
            return PreflightCheck::skipped(
                "vst3.dll_imports",
                "Plugin DLL imports",
                "Cannot inspect imports because the SmartBridge.vst3 bundle is not laid out correctly. Run the bundle layout check first.",
            );
        }
    };
    walk("vst3.dll_imports", "Plugin DLL imports", &inner).await
}

pub async fn check_standalone_imports() -> PreflightCheck {
    let exe = match super::basics::standalone_executable_path() {
        Some(p) => p,
        None => {
            return PreflightCheck::skipped(
                "standalone.dll_imports",
                "Standalone DLL imports",
                "Standalone SmartBridge.exe is not installed yet. Install it first if you want to use SmartBridge outside a DAW.",
            );
        }
    };
    walk("standalone.dll_imports", "Standalone DLL imports", &exe).await
}

async fn walk(id: &str, label: &str, binary: &Path) -> PreflightCheck {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = binary;
        return PreflightCheck::skipped(
            id,
            label,
            "DLL import walking is Windows-only. macOS bundles use a different mechanism (otool -L).",
        );
    }

    #[cfg(target_os = "windows")]
    {
        let bytes = match std::fs::read(binary) {
            Ok(b) => b,
            Err(e) => {
                return PreflightCheck::fail(
                    id,
                    label,
                    format!("Could not read the SmartBridge binary to inspect its imports: {e}. This usually means antivirus is quarantining the file."),
                )
                .with_detail(format!("path: {}", binary.display()))
                .with_fix(FixAction::OpenAvSettings);
            }
        };

        let imports = match list_imports(&bytes) {
            Ok(v) => v,
            Err(e) => {
                return PreflightCheck::warn(
                    id,
                    label,
                    format!("Could not parse the binary's import table ({e}). Skipping DLL probe; reinstalling the plugin usually clears this up."),
                )
                .with_detail(format!("path: {}", binary.display()))
                .with_fix(FixAction::ReinstallPlugin);
            }
        };

        let plugin_dir = binary.parent().map(PathBuf::from);
        let mut missing: Vec<String> = Vec::new();
        let mut details: Vec<String> = Vec::new();
        for dll in &imports {
            if is_system_dll_assumed_present(dll) {
                continue;
            }
            if !try_load(dll, plugin_dir.as_deref()) {
                missing.push(dll.clone());
                details.push(format!("MISSING: {dll}"));
            } else {
                details.push(format!("ok: {dll}"));
            }
        }

        if missing.is_empty() {
            PreflightCheck::pass(
                id,
                label,
                format!(
                    "Walked {} imported DLLs and Windows can load every one. Hosts will not blacklist for missing dependencies.",
                    imports.len()
                ),
            )
            .with_detail(format!("path: {}", binary.display()))
            .with_details(details)
        } else {
            // Decide which fix to recommend based on what's missing.
            let likely_vcredist = missing.iter().any(|d| {
                let d = d.to_ascii_lowercase();
                d.starts_with("vcruntime") || d.starts_with("msvcp") || d.starts_with("concrt")
            });
            let fix = if likely_vcredist {
                FixAction::InstallVcRedist
            } else {
                FixAction::ReinstallPlugin
            };

            let dll_list = missing.join(", ");
            let advice = if likely_vcredist {
                "These look like Visual C++ runtime DLLs — install the VC++ Redistributable to get them in one go."
            } else {
                "Reinstalling the plugin will redeploy the bundled copies of these DLLs next to the .vst3."
            };

            PreflightCheck::fail(
                id,
                label,
                format!(
                    "{} of {} imported DLLs cannot be loaded by Windows: {}. This is the exact reason your DAW marks SmartBridge as blacklisted. {}",
                    missing.len(),
                    imports.len(),
                    dll_list,
                    advice,
                ),
            )
            .with_detail(format!("path: {}", binary.display()))
            .with_details(details)
            .with_fix(fix)
        }
    }
}

#[cfg(target_os = "windows")]
fn list_imports(bytes: &[u8]) -> Result<Vec<String>, String> {
    use goblin::pe::PE;

    let pe = PE::parse(bytes).map_err(|e| format!("PE parse: {e}"))?;
    let mut out: Vec<String> = pe
        .imports
        .iter()
        .map(|i| i.dll.to_string())
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
fn list_imports(_bytes: &[u8]) -> Result<Vec<String>, String> {
    Err("not windows".into())
}

/// We don't probe Windows' own DLLs. They're always present (the OS is
/// running, after all) and probing them just wastes time. The `api-ms-*`
/// API set names are technically forwarders, also always available.
fn is_system_dll_assumed_present(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n.starts_with("api-ms-") || n.starts_with("ext-ms-") {
        return true;
    }
    matches!(
        n.as_str(),
        "kernel32.dll"
            | "user32.dll"
            | "gdi32.dll"
            | "advapi32.dll"
            | "shell32.dll"
            | "ole32.dll"
            | "oleaut32.dll"
            | "comdlg32.dll"
            | "comctl32.dll"
            | "winmm.dll"
            | "ws2_32.dll"
            | "wininet.dll"
            | "winhttp.dll"
            | "ntdll.dll"
            | "msimg32.dll"
            | "version.dll"
            | "imm32.dll"
            | "uxtheme.dll"
            | "dwmapi.dll"
            | "dxgi.dll"
            | "d3d11.dll"
            | "d3d9.dll"
            | "d2d1.dll"
            | "dwrite.dll"
            | "windowscodecs.dll"
            | "shlwapi.dll"
            | "rpcrt4.dll"
            | "secur32.dll"
            | "crypt32.dll"
            | "userenv.dll"
            | "iphlpapi.dll"
            | "psapi.dll"
            | "powrprof.dll"
            | "setupapi.dll"
            | "mf.dll"
            | "mfplat.dll"
            | "mfreadwrite.dll"
            | "mfuuid.dll"
            | "msvcrt.dll"
            | "ntoskrnl.exe"
            | "bcrypt.dll"
            | "ncrypt.dll"
            | "wldap32.dll"
            | "dsound.dll"
            | "dinput8.dll"
            | "wtsapi32.dll"
    )
}

#[cfg(target_os = "windows")]
fn try_load(name: &str, search_dir: Option<&Path>) -> bool {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;

    type HMODULE = *mut std::ffi::c_void;

    extern "system" {
        fn LoadLibraryExW(file_name: *const u16, h_file: *const std::ffi::c_void, flags: u32) -> HMODULE;
        fn FreeLibrary(h: HMODULE) -> i32;
        fn AddDllDirectory(new_directory: *const u16) -> *mut std::ffi::c_void;
        fn RemoveDllDirectory(cookie: *mut std::ffi::c_void) -> i32;
    }

    const LOAD_LIBRARY_AS_DATAFILE: u32 = 0x00000002;
    const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 0x00001000;
    const LOAD_LIBRARY_SEARCH_USER_DIRS: u32 = 0x00000400;
    const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 0x00000100;

    // Tell LoadLibrary to also look next to the .vst3 / .exe — that's
    // where vcpkg-built sqlcipher.dll and libcrypto-3-x64.dll typically
    // sit.
    let cookie = if let Some(dir) = search_dir {
        let wide: Vec<u16> = OsStr::new(dir.as_os_str())
            .encode_wide()
            .chain(once(0))
            .collect();
        unsafe { AddDllDirectory(wide.as_ptr()) }
    } else {
        std::ptr::null_mut()
    };

    let wide: Vec<u16> = OsStr::new(name).encode_wide().chain(once(0)).collect();
    let flags = LOAD_LIBRARY_AS_DATAFILE
        | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS
        | LOAD_LIBRARY_SEARCH_USER_DIRS
        | LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR;

    let result = unsafe {
        let h = LoadLibraryExW(wide.as_ptr(), std::ptr::null(), flags);
        if h.is_null() {
            false
        } else {
            FreeLibrary(h);
            true
        }
    };

    if !cookie.is_null() {
        unsafe { RemoveDllDirectory(cookie) };
    }
    result
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
fn try_load(_name: &str, _search_dir: Option<&Path>) -> bool {
    false
}
