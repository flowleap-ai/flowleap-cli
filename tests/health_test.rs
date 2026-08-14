//! `flowleap health` human output.
//!
//! The backend exposes two public probes: liveness at `/health` and readiness
//! at `/v1/health`, and only the readiness one carries `apiVersion`. Bare
//! `health` therefore prints no version at all, which reads as "this backend
//! doesn't report one" rather than "you asked the probe that doesn't". These
//! pin the pointer that closes that gap, and the readiness probe's own line.

mod support;

use serde_json::json;
use support::run_cli;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MOCK_API_VERSION: &str = "1.4.2+abc1234";

async fn mount_probe(server: &MockServer, probe: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(probe))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

/// Liveness carries no apiVersion, so it names the probe that does.
#[tokio::test]
async fn liveness_points_at_the_readiness_probe_for_the_api_version() {
    let server = MockServer::start().await;
    mount_probe(&server, "/health", json!({ "status": "ok" })).await;

    let output = run_cli(&server.uri(), &[], &["health"]).await;

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(
        stdout.contains("flowleap health api"),
        "names the readiness probe: {stdout}"
    );
}

/// Readiness reports the server build itself — no pointer needed.
#[tokio::test]
async fn readiness_prints_the_api_version_itself() {
    let server = MockServer::start().await;
    mount_probe(
        &server,
        "/v1/health",
        json!({ "status": "ok", "apiVersion": MOCK_API_VERSION }),
    )
    .await;

    let output = run_cli(&server.uri(), &[], &["health", "api"]).await;

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(
        stdout.contains(&format!("apiVersion: {MOCK_API_VERSION}")),
        "{stdout}"
    );
    assert!(
        !stdout.contains("flowleap health api"),
        "no pointer when the version is right there: {stdout}"
    );
}
