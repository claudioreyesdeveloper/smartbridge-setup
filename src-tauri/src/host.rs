//! Lightweight host introspection for the Diagnostics tab.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct HostInfo {
    pub os: &'static str,
    pub arch: &'static str,
    pub family: &'static str,
}

pub fn current() -> HostInfo {
    HostInfo {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        family: std::env::consts::FAMILY,
    }
}
