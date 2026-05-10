//! Shared zip-extraction helpers.
//!
//! Used by:
//!   * `install::main_app` (Windows zip payload — SmartBridge.exe + VST3 bundle
//!     + encrypted DB + default config)
//!   * `install::loopmidi` (Tobias Erichsen ships the loopMIDI installer as a
//!     zip containing a single Inno Setup .exe)
//!
//! The `zip` crate is pure-Rust and cross-platform, so these helpers compile
//! everywhere even though only Windows actually invokes them today.
//!
//! Both helpers defend against zip-slip: any entry whose path contains `..`,
//! a drive prefix, or other non-`Normal` components is flattened into a
//! single safe basename so the archive cannot escape `dest`.

#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use std::path::{Component, Path, PathBuf};

pub fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let f = std::fs::File::open(zip_path)
        .map_err(|e| format!("open {}: {e}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(f)
        .map_err(|e| format!("read zip {}: {e}", zip_path.display()))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry {i}: {e}"))?;
        let entry_name = entry.name().to_string();
        let out_path = dest.join(sanitize_entry_path(&entry_name));

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| format!("mkdir {}: {e}", out_path.display()))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        let mut out = std::fs::File::create(&out_path)
            .map_err(|e| format!("create {}: {e}", out_path.display()))?;
        std::io::copy(&mut entry, &mut out)
            .map_err(|e| format!("extract {}: {e}", out_path.display()))?;
    }
    Ok(())
}

pub fn sanitize_entry_path(name: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in Path::new(name).components() {
        match comp {
            Component::Normal(s) => out.push(s),
            Component::CurDir => {}
            _ => return PathBuf::from(name.replace('/', "_").replace('\\', "_")),
        }
    }
    out
}
