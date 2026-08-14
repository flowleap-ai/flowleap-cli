//! Tests for the ergonomic facade verbs (compare, figures, summary,
//! timeline, convert-number). Dry-run mode surfaces the exact request each
//! verb would send, so these assert the facade tool name (URL) and the
//! tool-input JSON shape without a live backend.

use std::process::Command;

fn dry_run(temp_home: &tempfile::TempDir, args: &[&str]) -> serde_json::Value {
    let mut full_args = vec!["--json"];
    full_args.extend_from_slice(args);
    full_args.push("--dry-run");

    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env_remove("FLOWLEAP_API_KEY")
        .env_remove("FLOWLEAP_TOKEN")
        .args(&full_args)
        .output()
        .expect("run flowleap dry-run");

    assert!(
        output.status.success(),
        "dry-run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    serde_json::from_str(&stdout).expect("stdout is json")
}

#[test]
fn facade_verbs_appear_in_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .arg("--help")
        .output()
        .expect("run flowleap --help");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    for verb in [
        "compare",
        "figures",
        "summary",
        "timeline",
        "convert-number",
    ] {
        assert!(stdout.contains(verb), "missing verb {verb}");
    }
}

#[test]
fn facade_verbs_show_examples_in_help() {
    for verb in [
        "compare",
        "figures",
        "summary",
        "timeline",
        "convert-number",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
            .args([verb, "--help"])
            .output()
            .expect("run verb --help");

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
        assert!(
            stdout.contains("Examples:") && stdout.contains(&format!("flowleap {verb}")),
            "{verb} --help lacks examples: {stdout}"
        );
    }
}

#[test]
fn compare_calls_compare_patents_with_number_array() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let value = dry_run(&temp_home, &["compare", "EP1000000", "US5443036"]);

    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "POST");
    assert_eq!(
        value["url"],
        "https://api.flowleap.co/v1/tools/compare_patents"
    );
    assert_eq!(
        value["body"]["patent_numbers"],
        serde_json::json!(["EP1000000", "US5443036"])
    );
}

#[test]
fn compare_requires_at_least_two_documents() {
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .args(["compare", "EP1000000", "--dry-run"])
        .output()
        .expect("run compare with one document");

    assert!(!output.status.success());
}

#[test]
fn figures_calls_get_patent_image() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let value = dry_run(&temp_home, &["figures", "EP1000000"]);

    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "POST");
    assert_eq!(
        value["url"],
        "https://api.flowleap.co/v1/tools/get_patent_image"
    );
    assert_eq!(value["body"]["patent_number"], "EP1000000");
}

/// `--out` no longer straddles two surfaces: metadata and image bytes both
/// come from get_patent_image, which returns the page as base64 (backend PRD
/// 0013 — the /v1/ops/figures byte fetch is gone).
#[test]
fn figures_out_fetches_image_payload_from_the_same_tool() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let out_path = temp_home.path().join("fig.png");
    let out_arg = out_path.to_str().expect("utf8 path");
    let value = dry_run(
        &temp_home,
        &["figures", "EP1000000", "--out", out_arg, "--page", "2"],
    );

    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "POST");
    assert_eq!(
        value["url"],
        "https://api.flowleap.co/v1/tools/get_patent_image"
    );
    assert_eq!(value["body"]["patent_number"], "EP1000000");
    assert_eq!(value["body"]["include_images"], true);
    assert_eq!(value["body"]["pages"], serde_json::json!([2]));
    // .png output is rasterized from the PDF source.
    assert_eq!(value["body"]["format"], "pdf");
    assert_eq!(value["body"]["render"], "png");
    assert!(!out_path.exists(), "dry-run must not write files");
}

#[test]
fn summary_calls_get_patent_summary() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let value = dry_run(&temp_home, &["summary", "EP1000000"]);

    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "POST");
    assert_eq!(
        value["url"],
        "https://api.flowleap.co/v1/tools/get_patent_summary"
    );
    assert_eq!(value["body"]["patent_number"], "EP1000000");
}

#[test]
fn timeline_calls_get_prosecution_timeline() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let value = dry_run(&temp_home, &["timeline", "EP1000000"]);

    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "POST");
    assert_eq!(
        value["url"],
        "https://api.flowleap.co/v1/tools/get_prosecution_timeline"
    );
    assert_eq!(value["body"]["patent_number"], "EP1000000");
}

#[test]
fn convert_number_calls_convert_patent_number_with_target_format() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let value = dry_run(
        &temp_home,
        &["convert-number", "EP1000000", "--to", "docdb"],
    );

    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "POST");
    assert_eq!(
        value["url"],
        "https://api.flowleap.co/v1/tools/convert_patent_number"
    );
    assert_eq!(value["body"]["patent_number"], "EP1000000");
    assert_eq!(value["body"]["to_format"], "docdb");
}

#[test]
fn convert_number_rejects_unknown_format() {
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .args(["convert-number", "EP1000000", "--to", "bogus", "--dry-run"])
        .output()
        .expect("run convert-number with bad format");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr is utf8");
    // clap enumerates the supported formats in the error.
    for format in ["epodoc", "docdb", "original"] {
        assert!(stderr.contains(format), "missing format {format}: {stderr}");
    }
}
