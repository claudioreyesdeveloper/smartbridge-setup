//! Structured logging for SmartBridge Setup.
//!
//! Two sinks:
//!   * stderr (visible during `npm run tauri dev`)
//!   * a daily-rotated file at `<installer_data_dir>/logs/setup.YYYY-MM-DD.log`
//!
//! The Diagnostics tab in the UI exposes a "Open log folder" button that
//! reveals this directory in the OS file manager.
//!
//! IMPORTANT: never log API keys, tokens, or any field whose name contains
//! `apiKey` / `api_key` / `token` / `secret`. Sanitisation is the caller's
//! responsibility — but in practice most call sites should just log a
//! redacted indicator like `apiKey=<redacted>` instead of the value.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Returns a guard that must be kept alive for the lifetime of the app
/// (otherwise the background log-flush thread will exit and we'll drop
/// log lines on shutdown).
pub fn init() -> Option<WorkerGuard> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,smartbridge_setup_lib=debug"));

    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_ansi(true);

    let (file_layer, guard) = match crate::paths::installer_log_dir() {
        Some(dir) => {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!(
                    "smartbridge-setup: cannot create log directory {dir:?}: {e}, file logging disabled"
                );
                (None, None)
            } else {
                let file_appender =
                    tracing_appender::rolling::daily(&dir, "setup.log");
                let (writer, guard) = tracing_appender::non_blocking(file_appender);
                let layer = fmt::layer()
                    .with_writer(writer)
                    .with_target(false)
                    .with_ansi(false);
                (Some(layer), Some(guard))
            }
        }
        None => {
            eprintln!("smartbridge-setup: no installer data dir, file logging disabled");
            (None, None)
        }
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stderr_layer)
        .with(file_layer)
        .init();

    guard
}
