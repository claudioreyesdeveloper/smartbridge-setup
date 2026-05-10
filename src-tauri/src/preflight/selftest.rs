//! Self-test: spawn `SmartBridge.exe --selftest` and inspect the JSON
//! report it prints to stdout. This is the strongest possible signal —
//! if the standalone can complete its own bootstrap (PathResolver,
//! ConfigManager, SmartBridgeContext, SQLCipher unlock, Tracktion
//! Engine) then a DAW can almost certainly load the .vst3 too, since
//! they share the same compiled code paths.

use super::{FixAction, PreflightCheck};
use std::time::Duration;

/// Time budget for the self-test. SmartBridge's startup includes
/// SQLCipher unlocking the encrypted DB plus Tracktion engine init,
/// which on a cold cache can take a few seconds. 30s gives us plenty
/// of headroom and still bounds the worst case.
const SELFTEST_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn check() -> PreflightCheck {
    let exe = match super::basics::standalone_executable_path() {
        Some(p) => p,
        None => {
            return PreflightCheck::skipped(
                "standalone.selftest",
                "SmartBridge self-test",
                "Self-test needs the standalone SmartBridge.exe / SmartBridge.app installed. Install the main app first.",
            );
        }
    };

    // The Windows standalone is a GUI subsystem build (WinMain), so
    // stdout/stderr from the child go nowhere. We give it an explicit
    // file path to dump its JSON to, then read it back here. macOS still
    // works because we always write to the file too.
    let report_path = std::env::temp_dir().join(format!(
        "smartbridge-selftest-{}.json",
        std::process::id()
    ));
    // Ensure no stale file from a previous run skews the result.
    let _ = std::fs::remove_file(&report_path);

    let result = tokio::time::timeout(
        SELFTEST_TIMEOUT,
        tokio::process::Command::new(&exe)
            .arg(format!("--selftest={}", report_path.display()))
            .output(),
    )
    .await;

    let output = match result {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            return PreflightCheck::fail(
                "standalone.selftest",
                "SmartBridge self-test",
                format!("Could not launch SmartBridge for the self-test: {e}. The most common reason on Windows is missing DLLs — run the DLL imports check, or click 'Reinstall plugin'."),
            )
            .with_detail(format!("path: {}", exe.display()))
            .with_fix(FixAction::ReinstallPlugin);
        }
        Err(_) => {
            return PreflightCheck::fail(
                "standalone.selftest",
                "SmartBridge self-test",
                format!("SmartBridge took longer than {} seconds to finish its self-test, which means it is hanging during startup. This usually points to an antivirus scan blocking the encrypted database, or a corrupt config file. Try reinstalling the plugin to reset.", SELFTEST_TIMEOUT.as_secs()),
            )
            .with_fix(FixAction::ReinstallPlugin);
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Prefer the JSON file the standalone wrote — that's the only
    // reliable source on Windows. Fall back to scanning stdout for the
    // command-line case.
    let file_payload = std::fs::read_to_string(&report_path).ok();
    let _ = std::fs::remove_file(&report_path);
    let report_line = file_payload
        .as_deref()
        .map(|s| s.trim().to_string())
        .or_else(|| {
            stdout
                .lines()
                .rev()
                .find(|l| l.trim_start().starts_with('{') && l.trim_end().ends_with('}'))
                .map(|s| s.to_string())
        });

    let exit_ok = output.status.success();

    if exit_ok {
        let mut details = vec![format!("path: {}", exe.display())];
        if let Some(line) = report_line.as_deref() {
            details.push(format!("report: {line}"));
        }
        if !stderr.trim().is_empty() {
            // Standalone is chatty on stderr; we only show last 5 lines so
            // the diagnostics section stays readable.
            for l in stderr.lines().rev().take(5).collect::<Vec<_>>().into_iter().rev() {
                details.push(format!("stderr: {l}"));
            }
        }
        PreflightCheck::pass(
            "standalone.selftest",
            "SmartBridge self-test",
            "SmartBridge launched, finished its bootstrap (paths, config, encrypted database, audio engine) and exited cleanly. DAWs should load the plugin without issue.",
        )
        .with_details(details)
    } else {
        let code = output.status.code().unwrap_or(-1);
        let summary = report_line
            .clone()
            .unwrap_or_else(|| "no JSON report produced".to_string());

        // Surface the most useful stderr line as the human explanation.
        let last_stderr = stderr
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("(no stderr)");

        let mut details = vec![
            format!("path: {}", exe.display()),
            format!("exit code: {code}"),
            format!("stdout report: {summary}"),
        ];
        for l in stderr.lines().rev().take(10).collect::<Vec<_>>().into_iter().rev() {
            details.push(format!("stderr: {l}"));
        }

        PreflightCheck::fail(
            "standalone.selftest",
            "SmartBridge self-test",
            format!(
                "SmartBridge failed its own bootstrap (exit code {code}). Last error: '{last}'. This is almost always either a missing DLL, a corrupt encrypted database, or antivirus interference. Reinstalling the plugin fixes the first two; check the antivirus check for the third.",
                code = code,
                last = last_stderr,
            ),
        )
        .with_details(details)
        .with_fix(FixAction::ReinstallPlugin)
    }
}
