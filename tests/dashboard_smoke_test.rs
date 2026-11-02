//! Runs the recipe-custom-dashboard template smoke test (a zero-dependency
//! Node script) inside `cargo test` so it rides the existing CI job. The
//! script copies each of the four dashboard templates into a temp bundle,
//! puts recorded tool answers in its data/ folder, and asserts the product
//! invariants (numbers-from-code rendering, the input check of each saved
//! answer, provenance footer, inline SVG, offline-safe HTML, guardrail on
//! ambiguous applicants). See
//! skills/recipe-custom-dashboard/references/smoke.mjs for the assertions.
//!
//! Node >= 18 is required. GitHub's ubuntu-latest runner ships Node, so this
//! always runs in CI; on a machine with no `node` on PATH it prints a notice
//! and skips rather than failing a Rust-only checkout.

use std::path::PathBuf;
use std::process::Command;

fn smoke_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("skills/recipe-custom-dashboard/references/smoke.mjs")
}

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn dashboard_templates_smoke() {
    if !node_available() {
        eprintln!(
            "SKIP dashboard_templates_smoke: `node` not found on PATH (Node >= 18 required)."
        );
        return;
    }

    let script = smoke_script();
    assert!(
        script.exists(),
        "smoke script missing at {}",
        script.display()
    );

    let output = Command::new("node")
        .arg(&script)
        .output()
        .expect("failed to spawn node for the dashboard smoke test");

    // The Node script prints a per-check log and exits non-zero on any failure.
    if !output.status.success() {
        panic!(
            "dashboard template smoke test failed\n--- stdout ---\n{}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
