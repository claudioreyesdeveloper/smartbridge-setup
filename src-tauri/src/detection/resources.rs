//! smartbridge-resources: today this is just the seed `config.json`.
//!
//! Detection rule:
//!   * config.json exists at the user's SmartBridge data dir → Ready.
//!     (We never overwrite an existing user config; whatever they have is
//!     considered authoritative.)
//!   * config.json does not exist → NotInstalled. Phase 4 places the
//!     sanitised seed config from the release manifest.
//!
//! When the encrypted database is broken out of the main installer in a
//! future release, add the smartbridge.db check here too.

use super::DetectionResult;
use crate::paths;

pub async fn detect() -> DetectionResult {
    let config = match paths::user_config_path() {
        Some(p) => p,
        None => return DetectionResult::error("could not resolve user config path"),
    };

    if config.exists() {
        DetectionResult::ready().with_detail(format!("config: {}", config.display()))
    } else {
        DetectionResult::not_installed()
            .with_detail(format!("config not yet placed at {}", config.display()))
    }
}
