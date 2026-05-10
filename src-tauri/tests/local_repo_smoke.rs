//! Offline / local-repo end-to-end test.
//!
//! Builds a minimal offline bundle in a tempdir (manifest + one asset),
//! points `SMARTBRIDGE_LOCAL_REPO` at it, and verifies that:
//!
//!   1. `manifest::fetch_with_fallback` reads from disk (no network)
//!      and surfaces the local path in `source_url`.
//!   2. `install::download_spec_for` populates `local_source` on the
//!      DownloadSpec.
//!   3. The local file's SHA256 matches what the manifest claims (the
//!      same check the downloader performs at install time).
//!
//! Pure offline: this test never hits the network. Run with:
//!   cargo test --test local_repo_smoke -- --nocapture

use sha2::{Digest, Sha256};
use smartbridge_setup_lib::{install, manifest};
use std::fs;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_repo_satisfies_manifest_and_download() {
    let tmp = tempfile::tempdir().expect("mktempdir");
    let bundle_dir = tmp.path().to_path_buf();

    let asset_bytes = b"{\n  \"hello\": \"offline\"\n}\n";
    let asset_name = "config-default.json";
    fs::write(bundle_dir.join(asset_name), asset_bytes).expect("write asset");

    let mut hasher = Sha256::new();
    hasher.update(asset_bytes);
    let asset_sha = hex::encode(hasher.finalize()).to_lowercase();
    let asset_size = asset_bytes.len() as u64;

    let manifest_json = serde_json::json!({
        "manifest_version": 1,
        "product": "SmartBridge",
        "release_version": "9.9.9-test",
        "release_channel": "test",
        "generated_at": "2026-05-01T00:00:00Z",
        "asset_provider": { "type": "local" },
        "components": [{
            "id": "smartbridge-resources",
            "display_name": "SmartBridge resources",
            "required": false,
            "optional": false,
            "status": "included",
            "depends_on": [],
            "assets": [{
                "asset_id": "smartbridge-resources.config-default",
                "component_id": "smartbridge-resources",
                "file_name": asset_name,
                "version": "9.9.9",
                "platform": "any",
                "architecture": "any",
                "content_type": "application/json",
                "install_action": "copy",
                "requires_admin": false,
                "signature_required": false,
                "signature_status": "n/a",
                "delivery": {
                    "method": "github_release_asset",
                    "release_tag": "v9.9.9-test",
                    "release_asset_name": asset_name,
                    "download_url": "https://example.invalid/should-not-be-fetched",
                    "sha256": asset_sha,
                    "file_size_bytes": asset_size,
                }
            }]
        }]
    });

    fs::write(
        bundle_dir.join("smartbridge-release-manifest.json"),
        serde_json::to_vec_pretty(&manifest_json).unwrap(),
    )
    .expect("write manifest");

    std::env::set_var("SMARTBRIDGE_LOCAL_REPO", &bundle_dir);

    let fetched = manifest::fetch_with_fallback()
        .await
        .expect("manifest fetch from local repo should succeed");

    println!();
    println!("=== fetched (offline) manifest ===");
    println!("  source     : {}", fetched.source_url);
    println!("  cached at  : {}", fetched.cached_path);
    println!("  from net   : {}", fetched.fetched_from_network);
    println!("  version    : {}", fetched.manifest.release_version);

    assert!(
        fetched.source_url.starts_with("local:"),
        "source_url should mark the local origin, got: {}",
        fetched.source_url
    );
    assert!(!fetched.fetched_from_network, "must NOT touch the network");
    assert_eq!(fetched.manifest.release_version, "9.9.9-test");

    let asset = fetched
        .manifest
        .component("smartbridge-resources")
        .and_then(|c| c.asset("smartbridge-resources.config-default"))
        .expect("synthetic asset must be present");
    let spec = install::download_spec_for(asset).expect("downloadable spec");

    println!();
    println!("=== download spec ===");
    println!("  url          : {}", spec.url);
    println!("  local_source : {:?}", spec.local_source);
    println!("  expected sha : {}", spec.expected_sha256_lc);
    println!("  expected size: {}", spec.expected_size_bytes);

    let local = spec
        .local_source
        .as_ref()
        .expect("local_source MUST be populated when SMARTBRIDGE_LOCAL_REPO is set");
    assert!(local.exists(), "local source path must exist on disk");
    assert_eq!(local, &bundle_dir.join(asset_name));

    let on_disk = fs::read(local).expect("read local asset");
    let mut h = Sha256::new();
    h.update(&on_disk);
    let actual_sha = hex::encode(h.finalize()).to_lowercase();
    assert_eq!(
        actual_sha, spec.expected_sha256_lc,
        "local file SHA must match manifest"
    );
    assert_eq!(on_disk.len() as u64, spec.expected_size_bytes);

    std::env::remove_var("SMARTBRIDGE_LOCAL_REPO");
    println!();
    println!("OK: offline mode end-to-end (no network)");
}
