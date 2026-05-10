//! Smoke test: exercise every detector on the host's real filesystem.
//!
//! This is NOT a correctness test — it just guarantees no detector panics
//! on the developer's actual machine, and prints results so the dev can
//! eyeball whether the states make sense before launching the GUI.
//!
//! Run with: cargo test --test detection_smoke -- --nocapture

use smartbridge_setup_lib::detection;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn detect_all_returns_for_every_component() {
    let results = detection::detect_all().await;
    assert_eq!(results.len(), 7, "expected 7 components");

    println!();
    println!("=== detect_all() smoke results ===");
    for (id, det) in &results {
        println!("  [{:<25}] state={:?} version={:?}", id, det.state, det.installed_version);
        for d in &det.details {
            println!("      · {d}");
        }
    }
    println!("=== end ===");
    println!();

    let ids: Vec<&str> = results.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&"main-app"));
    assert!(ids.contains(&"cubase-connection"));
    assert!(ids.contains(&"ai-lyrics"));
    assert!(ids.contains(&"synthv-connection"));
    assert!(ids.contains(&"smartbridge-resources"));
    assert!(ids.contains(&"help-files"));
    assert!(ids.contains(&"windows-loopmidi"));
}
