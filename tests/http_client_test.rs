//! End-to-end tests for the shared HTTP client hardening (issue #14):
//! timeouts, versioned User-Agent, and bounded retry — driven through the real
//! binary against a wiremock backend.

mod support;

use std::time::Duration;

use serde_json::json;
use support::{run_cli, stdout_json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A stalled backend must fail with a timeout error, not hang forever.
#[tokio::test]
async fn stalled_server_times_out_with_clear_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/health"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[("FLOWLEAP_TIMEOUT_SECS", "1")],
        &["api", "request", "get", "/v1/health"],
    )
    .await;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap().to_lowercase();
    assert!(
        stderr.contains("timed out") || stderr.contains("timeout"),
        "expected a timeout error, stderr was: {stderr}"
    );
}

/// A transient 5xx is retried and the follow-up success is returned.
#[tokio::test]
async fn transient_5xx_is_retried_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/thing"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/thing"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "result": "ok" })))
        .expect(1)
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[],
        &["api", "request", "get", "/v1/thing", "--output", "json"],
    )
    .await;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = stdout_json(&output);
    assert_eq!(value["ok"], true);
    assert_eq!(value["status"], 200);
    assert_eq!(value["body"]["result"], "ok");
}

/// A 4xx is a client error and must never be retried.
#[tokio::test]
async fn client_error_is_not_retried() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/nope"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(json!({ "error": "not found", "code": "NOT_FOUND" })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[],
        &["api", "request", "get", "/v1/nope", "--output", "json"],
    )
    .await;

    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(value["ok"], false);
    assert_eq!(value["status"], 404);
    assert_eq!(value["body"]["code"], "NOT_FOUND");
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        1,
        "a 4xx must not be retried"
    );
}

/// Every request carries the versioned User-Agent.
#[tokio::test]
async fn requests_carry_versioned_user_agent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/thing"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[],
        &["api", "request", "get", "/v1/thing", "--output", "json"],
    )
    .await;
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let requests = server.received_requests().await.unwrap();
    let user_agent = requests[0]
        .headers
        .get("user-agent")
        .expect("user-agent header present")
        .to_str()
        .unwrap();
    assert_eq!(
        user_agent,
        format!("flowleap-cli/{}", env!("CARGO_PKG_VERSION")),
    );
}

/// A 429 with a long Retry-After passes through unchanged: the error envelope
/// keeps its shape and surfaces `retryAfterSeconds`, and it is not retried.
#[tokio::test]
async fn rate_limit_envelope_is_preserved() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/thing"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "30")
                .set_body_json(json!({ "error": "rate limited", "code": "RATE_LIMITED" })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[],
        &["api", "request", "get", "/v1/thing", "--output", "json"],
    )
    .await;

    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(value["ok"], false);
    assert_eq!(value["status"], 429);
    assert_eq!(value["retryAfterSeconds"], 30);
    assert_eq!(value["body"]["code"], "RATE_LIMITED");
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        1,
        "a long Retry-After must pass through, not retry in-band"
    );
}

/// A 429 with a short Retry-After is respected and retried in-band.
#[tokio::test]
async fn short_retry_after_429_is_retried() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/thing"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/thing"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "result": "ok" })))
        .expect(1)
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[],
        &["api", "request", "get", "/v1/thing", "--output", "json"],
    )
    .await;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = stdout_json(&output);
    assert_eq!(value["ok"], true);
    assert_eq!(value["status"], 200);
}
