//! Generate a single diagnostics report from preflight results. The
//! customer clicks "Save report" in the UI, the file is written into
//! the installer log folder, and the absolute path is shown back so
//! they can attach it to a support email.

use super::{PreflightCheck, PreflightStatus};
use chrono::Local;
use std::path::PathBuf;

pub fn build(checks: &[PreflightCheck]) -> String {
    let mut s = String::new();

    s.push_str("SmartBridge Preflight Report\n");
    s.push_str("============================\n");
    s.push_str(&format!(
        "Generated: {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S %z")
    ));
    s.push_str(&format!(
        "Setup version: {}\n",
        env!("CARGO_PKG_VERSION")
    ));
    s.push_str(&format!(
        "Host: {} {} ({})\n\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::env::consts::FAMILY
    ));

    let summary = summarise(checks);
    s.push_str(&format!(
        "Summary: {} pass, {} warn, {} fail, {} skipped (total {})\n\n",
        summary.pass,
        summary.warn,
        summary.fail,
        summary.skipped,
        checks.len()
    ));

    for c in checks {
        s.push_str(&format!(
            "[{tag}] {label}\n",
            tag = status_tag(c.status),
            label = c.label,
        ));
        s.push_str(&format!("  id: {}\n", c.id));
        s.push_str(&format!("  {}\n", c.explanation));
        if !c.details.is_empty() {
            s.push_str("  details:\n");
            for d in &c.details {
                s.push_str(&format!("    - {d}\n"));
            }
        }
        s.push('\n');
    }

    s
}

/// Build the report and persist it to disk. Returns the absolute path
/// of the written file so the caller (the Preflight UI) can show it.
///
/// Files land alongside the rolling Setup logs at
/// `<installer_log_dir>/preflight-YYYYMMDD-HHMMSS.txt`. The timestamp
/// in the filename means repeated saves don't overwrite each other —
/// users can compare runs or send the most recent one.
pub fn save(checks: &[PreflightCheck]) -> Result<PathBuf, String> {
    let dir = crate::paths::installer_log_dir()
        .ok_or_else(|| "could not determine installer log directory".to_string())?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create log directory {}: {e}", dir.display()))?;

    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("preflight-{stamp}.txt"));

    let body = build(checks);
    std::fs::write(&path, body)
        .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(path)
}

fn status_tag(s: PreflightStatus) -> &'static str {
    match s {
        PreflightStatus::Pass => "PASS",
        PreflightStatus::Warn => "WARN",
        PreflightStatus::Fail => "FAIL",
        PreflightStatus::Skipped => "SKIP",
    }
}

struct Summary {
    pass: usize,
    warn: usize,
    fail: usize,
    skipped: usize,
}

fn summarise(checks: &[PreflightCheck]) -> Summary {
    let mut out = Summary { pass: 0, warn: 0, fail: 0, skipped: 0 };
    for c in checks {
        match c.status {
            PreflightStatus::Pass => out.pass += 1,
            PreflightStatus::Warn => out.warn += 1,
            PreflightStatus::Fail => out.fail += 1,
            PreflightStatus::Skipped => out.skipped += 1,
        }
    }
    out
}
