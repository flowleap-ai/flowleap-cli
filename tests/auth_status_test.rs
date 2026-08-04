//! `flowleap auth status` (issue #60): the command must distinguish a
//! credential that is *present* from one that *works*.
//!
//! Reporting presence as "Authenticated" is what let an expired token read as
//! healthy until the next data command 401'd, so these tests pin all four
//! states — absent, valid, rejected, and could-not-verify — in both output
//! modes, along with the exit code each one carries.

mod support;

use serde_json::json;
use support::{run_cli, stdout_json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const API_KEY_ENV: (&str, &str) = ("FLOWLEAP_API_KEY", "fl_pat_test_key");

async fn mount_profile(server: &MockServer, template: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/api/profile"))
        .respond_with(template)
        .mount(server)
        .await;
}

fn profile_body() -> serde_json::Value {
    json!({ "email": "inventor@example.com", "name": "Ada Lovelace" })
}

/*
 * ── valid ───────────────────────────────────────────────────────────────────
 */

#[tokio::test]
async fn valid_credential_is_reported_as_verified_and_exits_zero() {
    let server = MockServer::start().await;
    mount_profile(
        &server,
        ResponseTemplate::new(200).set_body_json(profile_body()),
    )
    .await;

    let output = run_cli(&server.uri(), &[API_KEY_ENV], &["auth", "status"]).await;

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("Valid"));
    assert!(stdout.contains("verified against the backend"));
    // The identity the probe returned is shown, since it proves the check ran.
    assert!(stdout.contains("inventor@example.com"));
    assert!(stdout.contains("Ada Lovelace"));
}

#[tokio::test]
async fn valid_credential_json_reports_state_and_checked() {
    let server = MockServer::start().await;
    mount_profile(
        &server,
        ResponseTemplate::new(200).set_body_json(profile_body()),
    )
    .await;

    let output = run_cli(&server.uri(), &[API_KEY_ENV], &["--json", "auth", "status"]).await;

    assert_eq!(output.status.code(), Some(0));
    let value = stdout_json(&output);
    assert_eq!(value["verification"]["state"], "valid");
    assert_eq!(value["verification"]["checked"], true);
    assert_eq!(value["credential"]["present"], true);
    assert_eq!(value["credential"]["source"], "env-api-key");
    assert_eq!(value["profile"]["email"], "inventor@example.com");
}

/// A paywall answers a different question than authentication: the credential
/// was accepted, so it is valid — reporting it as rejected would send the
/// user to `auth login`, which does not fix an unpaid subscription.
#[tokio::test]
async fn subscription_paywall_still_counts_as_a_valid_credential() {
    let server = MockServer::start().await;
    mount_profile(
        &server,
        ResponseTemplate::new(402).set_body_json(json!({ "error": "subscription_required" })),
    )
    .await;

    let output = run_cli(&server.uri(), &[API_KEY_ENV], &["--json", "auth", "status"]).await;

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout_json(&output)["verification"]["state"], "valid");
}

/*
 * ── rejected ────────────────────────────────────────────────────────────────
 */

/// The regression this issue exists for: an expired or revoked credential
/// must never read as authenticated.
#[tokio::test]
async fn rejected_credential_says_so_instead_of_authenticated() {
    let server = MockServer::start().await;
    mount_profile(
        &server,
        ResponseTemplate::new(401).set_body_json(json!({ "error": "unauthorized" })),
    )
    .await;

    let output = run_cli(&server.uri(), &[API_KEY_ENV], &["auth", "status"]).await;

    assert_eq!(output.status.code(), Some(3));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("Rejected"));
    assert!(stdout.contains("the backend refused this credential (HTTP 401)"));
    assert!(stdout.contains("expired, revoked, or wrong"));
    assert!(stdout.contains("flowleap auth login"));
    // The word that caused the bug must not appear for a dead credential.
    assert!(!stdout.contains("Authenticated"));
}

#[tokio::test]
async fn rejected_credential_json_reports_the_rejected_state() {
    let server = MockServer::start().await;
    mount_profile(
        &server,
        ResponseTemplate::new(401).set_body_json(json!({ "error": "unauthorized" })),
    )
    .await;

    let output = run_cli(&server.uri(), &[API_KEY_ENV], &["--json", "auth", "status"]).await;

    assert_eq!(output.status.code(), Some(3));
    let value = stdout_json(&output);
    assert_eq!(value["verification"]["state"], "rejected");
    assert_eq!(value["verification"]["checked"], true);
    // Present but not working — the distinction the whole command exists for.
    assert_eq!(value["credential"]["present"], true);
    assert!(value.get("profile").is_none());
}

/*
 * ── absent ──────────────────────────────────────────────────────────────────
 */

#[tokio::test]
async fn absent_credential_is_reported_without_a_network_call() {
    // No mock is mounted: reaching the network at all would fail the run.
    let output = run_cli("http://127.0.0.1:9", &[], &["auth", "status"]).await;

    assert_eq!(output.status.code(), Some(3));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("Not authenticated"));
    assert!(stdout.contains("flowleap auth login"));
}

#[tokio::test]
async fn absent_credential_json_reports_absent_and_unchecked() {
    let output = run_cli("http://127.0.0.1:9", &[], &["--json", "auth", "status"]).await;

    assert_eq!(output.status.code(), Some(3));
    let value = stdout_json(&output);
    assert_eq!(value["verification"]["state"], "absent");
    assert_eq!(value["verification"]["checked"], false);
    assert_eq!(value["credential"]["present"], false);
    assert_eq!(value["credential"]["source"], "missing");
}

/*
 * ── could not verify ────────────────────────────────────────────────────────
 */

/// An unreachable backend proves nothing about the credential. Reporting it
/// as invalid would send a user to re-authenticate over a dropped connection.
#[tokio::test]
async fn unreachable_backend_reports_could_not_verify_never_invalid() {
    let output = run_cli("http://127.0.0.1:9", &[API_KEY_ENV], &["auth", "status"]).await;

    assert_eq!(output.status.code(), Some(7));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("could not verify"));
    assert!(stdout.contains("not proof of a working one"));
    // Never states or implies a verdict it did not reach.
    assert!(!stdout.contains("Rejected"));
    assert!(!stdout.contains("Valid"));
    assert!(!stdout.contains("Authenticated"));
}

#[tokio::test]
async fn unreachable_backend_json_reports_unverified_with_a_reason() {
    let output = run_cli(
        "http://127.0.0.1:9",
        &[API_KEY_ENV],
        &["--json", "auth", "status"],
    )
    .await;

    assert_eq!(output.status.code(), Some(7));
    let value = stdout_json(&output);
    assert_eq!(value["verification"]["state"], "unverified");
    // Present, but explicitly NOT checked — an agent must not read this as ok.
    assert_eq!(value["verification"]["checked"], false);
    assert_eq!(value["credential"]["present"], true);
    assert!(value["verification"]["reason"].is_string());
    // No HTTP status: the backend was never reached.
    assert!(value["verification"].get("httpStatus").is_none());
}

/// A backend that answers, but not usefully, is also an unreached verdict —
/// distinct from a refusal, and carrying the status that explains it.
#[tokio::test]
async fn backend_error_is_unverified_rather_than_rejected() {
    let server = MockServer::start().await;
    mount_profile(
        &server,
        ResponseTemplate::new(500).set_body_json(json!({ "error": "internal" })),
    )
    .await;

    let output = run_cli(&server.uri(), &[API_KEY_ENV], &["--json", "auth", "status"]).await;

    assert_eq!(output.status.code(), Some(1));
    let value = stdout_json(&output);
    assert_eq!(value["verification"]["state"], "unverified");
    assert_eq!(value["verification"]["httpStatus"], 500);
}

/// A dry run sends nothing, so it can prove nothing — it says so plainly and
/// keeps the success exit, matching `doctor`'s dry-run contract.
#[tokio::test]
async fn dry_run_reports_unverified_and_keeps_the_success_exit() {
    let output = run_cli(
        "http://127.0.0.1:9",
        &[API_KEY_ENV],
        &["--json", "--dry-run", "auth", "status"],
    )
    .await;

    assert_eq!(output.status.code(), Some(0));
    let value = stdout_json(&output);
    assert_eq!(value["verification"]["state"], "unverified");
    assert_eq!(value["verification"]["checked"], false);
    assert!(value["verification"]["reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("Dry run")));
}
