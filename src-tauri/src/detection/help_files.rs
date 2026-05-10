//! help-files: getting-started, installation guide, and the interactive
//! manual zip. All three live in `<user_data_dir>/help/`.

use super::DetectionResult;
use crate::paths;

const EXPECTED_FILES: &[&str] = &[
    "SmartBridge_Getting_Started_One_Page.txt",
    "Installation_guide.zip",
    "smartbridge_multilingual_manual.zip",
];

pub async fn detect() -> DetectionResult {
    let dir = match paths::help_files_dir() {
        Some(d) => d,
        None => return DetectionResult::error("could not resolve help files directory"),
    };

    if !dir.exists() {
        return DetectionResult::not_installed()
            .with_detail(format!("help directory not present: {}", dir.display()));
    }

    let mut present: Vec<&'static str> = Vec::new();
    let mut missing: Vec<&'static str> = Vec::new();

    for &name in EXPECTED_FILES {
        if dir.join(name).exists() {
            present.push(name);
        } else {
            missing.push(name);
        }
    }

    if missing.is_empty() {
        let mut r = DetectionResult::ready().with_detail(format!("at {}", dir.display()));
        for p in present {
            r = r.with_detail(format!("present: {p}"));
        }
        r
    } else if present.is_empty() {
        DetectionResult::not_installed()
            .with_detail(format!("none of the expected files present in {}", dir.display()))
    } else {
        let mut r = DetectionResult::needs_repair(format!(
            "{} of {} expected file(s) missing",
            missing.len(),
            EXPECTED_FILES.len()
        ));
        for m in missing {
            r = r.with_detail(format!("missing: {m}"));
        }
        for p in present {
            r = r.with_detail(format!("present: {p}"));
        }
        r
    }
}
