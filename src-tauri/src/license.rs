//! License flavor + license.json writer.
//!
//! SmartBridge Setup ships in three flavors selected at compile time via
//! the `SETUP_FLAVOR` env var:
//!
//!   * `release`  - default. Plugin runs without restriction. Setup writes
//!                  `{ "mode": "full" }` so the plugin's gate skips checks.
//!
//!   * `demo`     - 30-day time-limited build. Setup does NOT write a
//!                  license.json at all - the plugin auto-creates one
//!                  with `mode = demo` + `first_run_iso` on first launch.
//!                  Anyone can install without entering anything.
//!
//!   * `beta_0_1` - invitation-only build. Setup shows an activation card:
//!                  user enters email + serial. We validate locally against
//!                  the bundled salt and write `{ "mode": "beta_0_1", ... }`.
//!                  The plugin re-validates on every launch.
//!
//! The Rust salt copy (gitignored) lives at `secrets/serial_salt.rs`.
//! It MUST stay in sync with `secrets/serial_salt.h` (C++) and the admin
//! tool's copy. If the file is missing at compile time the build falls
//! back to a placeholder salt and warns - real builds always have it.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

// ---------- Salt --------------------------------------------------------

include!("../secrets/serial_salt.rs");

// ---------- Flavor ------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Flavor {
    Release,
    Demo,
    Beta_0_1,
}

impl Flavor {
    pub fn from_env() -> Self {
        match option_env!("SETUP_FLAVOR").unwrap_or("release") {
            "demo" => Flavor::Demo,
            "beta_0_1" | "beta-0.1" | "beta01" => Flavor::Beta_0_1,
            _ => Flavor::Release,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Flavor::Release => "SmartBridge Setup",
            Flavor::Demo => "SmartBridge Setup (Demo)",
            Flavor::Beta_0_1 => "SmartBridge Setup (Beta 0.1)",
        }
    }
}

// ---------- Status surfaced to the frontend ----------------------------

#[derive(Debug, Clone, Serialize)]
pub struct LicenseStatus {
    pub flavor: Flavor,
    pub display_name: &'static str,

    /// Beta-only: true once the user has entered a valid email + serial
    /// AND we've persisted license.json to the user data dir. Always
    /// false in Release / Demo flavors (those don't gate installs).
    pub activated: bool,

    /// The activated email, if any.
    pub email: Option<String>,
}

pub fn current_status() -> LicenseStatus {
    let flavor = Flavor::from_env();
    // Always report an existing beta activation, regardless of which Setup
    // flavor is running. This lets customers open Setup later and change
    // their email/serial without needing a special first-run path.
    let (activated, email) = match read_existing_license() {
        Some(rec) if rec.mode == "beta_0_1" && verify_record(&rec) => {
            (true, Some(rec.email))
        }
        _ => (false, None),
    };
    LicenseStatus {
        flavor,
        display_name: flavor.display_name(),
        activated,
        email,
    }
}

// ---------- Serial computation -----------------------------------------

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
pub const SERIAL_BYTES: usize = 10; // 80 bits -> 16 base32 chars

/// Compute the canonical (no-dash) 16-character serial for an email.
/// Caller can format with [`format_serial`] for display.
pub fn compute_serial(email: &str) -> String {
    let normalized = email.trim().to_lowercase();
    let mut h = Sha256::new();
    h.update(SERIAL_SALT);
    h.update(normalized.as_bytes());
    let digest = h.finalize();
    base32_crockford(&digest[..SERIAL_BYTES])
}

/// Insert dashes every 4 chars: AAAA-BBBB-CCCC-DDDD.
pub fn format_serial(canonical: &str) -> String {
    if canonical.len() != 16 {
        return canonical.to_string();
    }
    format!(
        "{}-{}-{}-{}",
        &canonical[..4],
        &canonical[4..8],
        &canonical[8..12],
        &canonical[12..16],
    )
}

/// Strip whitespace and dashes, uppercase, map ambiguous Crockford
/// glyphs (O/I/L) back to 0/1/1 so a user typing the serial with a
/// confused 'O' instead of '0' still works.
pub fn canonicalize_serial(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .map(|c| match c {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect()
}

fn base32_crockford(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 8 / 5 + 1);
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for b in bytes {
        buffer = (buffer << 8) | u32::from(*b);
        bits += 8;
        while bits >= 5 {
            let idx = ((buffer >> (bits - 5)) & 0x1F) as usize;
            out.push(CROCKFORD[idx] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        let idx = ((buffer << (5 - bits)) & 0x1F) as usize;
        out.push(CROCKFORD[idx] as char);
    }
    out
}

/// True iff `serial` matches the expected canonical serial for `email`.
pub fn verify(email: &str, serial: &str) -> bool {
    let expected = compute_serial(email);
    canonicalize_serial(serial) == canonicalize_serial(&expected)
}

// ---------- license.json writer ----------------------------------------

/// Path to the SmartBridge plugin's runtime license file. This must
/// match what the C++ LicenseGate reads (PathResolver::getUserDataDirectory()).
fn license_file() -> Option<PathBuf> {
    crate::paths::user_data_dir().map(|d| d.join("license.json"))
}

/// Outcome of an activation attempt, returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct ActivationOutcome {
    pub ok: bool,
    pub message: String,
    pub status: LicenseStatus,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct LicenseRecord {
    #[serde(default)]
    mode: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    serial: String,
    #[serde(default)]
    issued_at: String,
}

fn read_existing_license() -> Option<LicenseRecord> {
    let path = license_file()?;
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice::<LicenseRecord>(&bytes).ok()
}

fn verify_record(rec: &LicenseRecord) -> bool {
    !rec.email.is_empty()
        && !rec.serial.is_empty()
        && verify(&rec.email, &rec.serial)
}

/// Activate or replace a Beta license. Validates the serial and writes
/// license.json atomically. This is intentionally available from every Setup
/// flavor so a customer can enter/change their serial later from the Setup
/// dashboard.
pub fn activate_beta(email: &str, serial: &str) -> ActivationOutcome {
    let email_clean = email.trim().to_lowercase();
    if email_clean.is_empty() {
        return ActivationOutcome {
            ok: false,
            message: "Email is required.".into(),
            status: current_status(),
        };
    }

    if !verify(&email_clean, serial) {
        return ActivationOutcome {
            ok: false,
            message: "That serial does not match this email. Double-check both \
                       (serials are case-insensitive, dashes optional)."
                .into(),
            status: current_status(),
        };
    }

    let canonical = canonicalize_serial(serial);
    let formatted = format_serial(&canonical);

    let now = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::json!({
        "mode": "beta_0_1",
        "email": email_clean,
        "serial": formatted,
        "issued_at": now,
    });

    if let Err(e) = write_atomic(&payload) {
        return ActivationOutcome {
            ok: false,
            message: format!("Could not write license file: {e}"),
            status: current_status(),
        };
    }

    ActivationOutcome {
        ok: true,
        message: format!("Activated for {email_clean}."),
        status: current_status(),
    }
}

/// Write `{ "mode": "full" }` so the plugin treats this as a Release
/// install (no demo timer, no activation prompt). Called by the Release
/// flavor of Setup as part of the resources component.
pub fn write_full_license() -> Result<(), String> {
    if let Some(rec) = read_existing_license() {
        if rec.mode == "beta_0_1" && verify_record(&rec) {
            return Ok(());
        }
    }

    let payload = serde_json::json!({ "mode": "full" });
    write_atomic(&payload).map_err(|e| e.to_string())
}

fn write_atomic(payload: &serde_json::Value) -> std::io::Result<()> {
    let path = license_file()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no user data dir"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(payload)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

// ---------- Tests -------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_is_case_and_whitespace_insensitive() {
        let a = compute_serial("alice@example.com");
        let b = compute_serial("Alice@Example.COM ");
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn verify_accepts_dashes_and_lowercase() {
        let s = format_serial(&compute_serial("alice@example.com"));
        assert!(verify("alice@example.com", &s));
        assert!(verify("ALICE@example.com", &s.to_lowercase()));
        assert!(verify("alice@example.com", &s.replace('-', "")));
    }

    #[test]
    fn wrong_email_fails() {
        let s = format_serial(&compute_serial("alice@example.com"));
        assert!(!verify("bob@example.com", &s));
    }
}
