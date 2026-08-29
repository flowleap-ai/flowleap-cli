//! Exit-code contract + structured 402/429 hints (issue #20): each HTTP
//! status class must produce its documented exit code, and the 402/429
//! envelopes must carry their additive hints — driven through the real binary
//! against a wiremock backend. The contract table lives in AGENTS.md.

mod support;

use std::time::Duration;

use serde_json::json;
use support::{run_cli, stdout_json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Mount a GET /v1/thing mock answering `template`, exactly once expected.
async fn mount_thing(server: &MockServer, template: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/v1/thing"))
        .respond_with(template)
        .mount(server)
        .await;
}

/// Run `api request get /v1/thing --output json` against the server.
async fn request_thing_json(server: &MockServer) -> std::process::Output {
    run_cli(
        &server.uri(),
        &[],
        &["api", "request", "get", "/v1/thing", "--output", "json"],
    )
    .await
}

/// HTTP 401 → exit 3 (auth required).
#[tokio::test]
async fn auth_required_401_exits_3() {
    let server = MockServer::start().await;
    mount_thing(
        &server,
        ResponseTemplate::new(401).set_body_json(json!({ "error": "unauthorized" })),
    )
    .await;

    let output = request_thing_json(&server).await;

    assert_eq!(output.status.code(), Some(3));
    let value = stdout_json(&output);
    assert_eq!(value["ok"], false);
    assert_eq!(value["status"], 401);
}

/// HTTP 402 → exit 4, with a subscription hint carrying the upgrade URL the
/// backend sent.
#[tokio::test]
async fn subscription_required_402_exits_4_with_hint() {
    let server = MockServer::start().await;
    mount_thing(
        &server,
        ResponseTemplate::new(402).set_body_json(json!({
            "error": "subscription_required",
            "upgradeUrl": "https://flowleap.co/upgrade-here",
        })),
    )
    .await;

    let output = request_thing_json(&server).await;

    assert_eq!(output.status.code(), Some(4));
    let value = stdout_json(&output);
    assert_eq!(value["ok"], false);
    assert_eq!(value["status"], 402);
    let hint = &value["subscriptionHint"];
    assert_eq!(hint["requiresHumanIntervention"], true);
    // No plan name: the backend sells one flat License, and naming a plan
    // here went stale once already ("Basic").
    assert!(hint.get("plan").is_none());
    assert_eq!(hint["upgradeUrl"], "https://flowleap.co/upgrade-here");
    // The message must fit the no-card trial (backend ADR 0018): the trial
    // ran at sign-up, so the ask is subscribe — never "start your trial".
    let message = hint["message"].as_str().expect("hint message");
    assert!(message.contains("subscribe"), "message: {message}");
    assert!(!message.contains("Basic"), "message: {message}");
    assert!(
        hint["message"].as_str().is_some_and(|m| !m.is_empty()),
        "hint must carry a message: {hint}"
    );
}

/// A 402 body without an upgrade URL falls back to the pricing page.
#[tokio::test]
async fn subscription_hint_falls_back_to_pricing_url() {
    let server = MockServer::start().await;
    mount_thing(
        &server,
        ResponseTemplate::new(402).set_body_json(json!({ "error": "subscription_required" })),
    )
    .await;

    let output = request_thing_json(&server).await;

    assert_eq!(output.status.code(), Some(4));
    let value = stdout_json(&output);
    assert_eq!(
        value["subscriptionHint"]["upgradeUrl"],
        "https://flowleap.co/pricing"
    );
}

/// In human mode the 402 hint renders as an upgrade box on stderr, and stdout
/// stays free of it.
#[tokio::test]
async fn human_mode_402_renders_upgrade_box_on_stderr() {
    let server = MockServer::start().await;
    mount_thing(
        &server,
        ResponseTemplate::new(402).set_body_json(json!({ "error": "subscription_required" })),
    )
    .await;

    let output = run_cli(&server.uri(), &[], &["api", "request", "get", "/v1/thing"]).await;

    assert_eq!(output.status.code(), Some(4));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("subscription required") && stderr.contains("https://flowleap.co/pricing"),
        "expected an upgrade box on stderr, got: {stderr}"
    );
}

/// HTTP 404 → exit 5 (not found).
#[tokio::test]
async fn not_found_404_exits_5() {
    let server = MockServer::start().await;
    mount_thing(
        &server,
        ResponseTemplate::new(404).set_body_json(json!({ "error": "not found" })),
    )
    .await;

    let output = request_thing_json(&server).await;

    assert_eq!(output.status.code(), Some(5));
    let value = stdout_json(&output);
    assert_eq!(value["status"], 404);
}

/// HTTP 429 → exit 6, with a rate-limit hint carrying retryAfterSeconds.
#[tokio::test]
async fn rate_limited_429_exits_6_with_hint() {
    let server = MockServer::start().await;
    mount_thing(
        &server,
        ResponseTemplate::new(429)
            .insert_header("retry-after", "30")
            .set_body_json(json!({ "error": "rate limited" })),
    )
    .await;

    let output = request_thing_json(&server).await;

    assert_eq!(output.status.code(), Some(6));
    let value = stdout_json(&output);
    assert_eq!(value["status"], 429);
    let hint = &value["rateLimitHint"];
    assert_eq!(hint["retryAfterSeconds"], 30);
    assert!(
        hint["message"].as_str().is_some_and(|m| !m.is_empty()),
        "hint must carry a message: {hint}"
    );
}

/// A 429 without Retry-After still exits 6 and still carries the hint.
#[tokio::test]
async fn rate_limited_without_retry_after_still_hints() {
    let server = MockServer::start().await;
    mount_thing(
        &server,
        ResponseTemplate::new(429).set_body_json(json!({ "error": "rate limited" })),
    )
    .await;

    let output = request_thing_json(&server).await;

    assert_eq!(output.status.code(), Some(6));
    let value = stdout_json(&output);
    let hint = &value["rateLimitHint"];
    assert!(hint["retryAfterSeconds"].is_null());
    assert!(
        hint["message"].as_str().is_some_and(|m| !m.is_empty()),
        "hint must carry a message: {hint}"
    );
}

/// A request timeout → exit 7 (network).
#[tokio::test]
async fn timeout_exits_7() {
    let server = MockServer::start().await;
    mount_thing(
        &server,
        ResponseTemplate::new(200).set_delay(Duration::from_secs(5)),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[("FLOWLEAP_TIMEOUT_SECS", "1")],
        &["api", "request", "get", "/v1/thing", "--output", "json"],
    )
    .await;

    assert_eq!(output.status.code(), Some(7));
}

/// A connection failure → exit 7 (network).
#[tokio::test]
async fn connection_refused_exits_7() {
    // Port 9 (discard) is closed on any sane test machine; no server started.
    let output = run_cli(
        "http://127.0.0.1:9",
        &[("FLOWLEAP_MAX_RETRIES", "0")],
        &["api", "request", "get", "/v1/thing", "--output", "json"],
    )
    .await;

    assert_eq!(output.status.code(), Some(7));
}

/// A 5xx without a dedicated code stays a generic failure → exit 1.
#[tokio::test]
async fn server_error_500_exits_1() {
    let server = MockServer::start().await;
    mount_thing(
        &server,
        ResponseTemplate::new(500).set_body_json(json!({ "error": "boom" })),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[("FLOWLEAP_MAX_RETRIES", "0")],
        &["api", "request", "get", "/v1/thing", "--output", "json"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let value = stdout_json(&output);
    assert_eq!(value["status"], 500);
}

/// The 402 hint fields are additive: the pre-existing envelope shape
/// (ok/status/contentType/body) is untouched.
#[tokio::test]
async fn hint_fields_are_additive_to_the_envelope() {
    let server = MockServer::start().await;
    mount_thing(
        &server,
        ResponseTemplate::new(402).set_body_json(json!({ "error": "subscription_required" })),
    )
    .await;

    let output = request_thing_json(&server).await;
    let value = stdout_json(&output);

    assert_eq!(value["ok"], false);
    assert_eq!(value["status"], 402);
    assert_eq!(value["body"]["error"], "subscription_required");
    assert!(value["contentType"].is_string());
}

/// The patent-data-key gate gets its own code (9), not the generic 1 the
/// backend's 400 status would otherwise map to. It is the most likely
/// first-run failure and the only one whose fix is a human doing a browser
/// signup, so an agent has to be able to tell it from a bad query on `$?`
/// alone.
#[tokio::test]
async fn a_patent_data_key_gate_exits_9_with_its_hint() {
    for (code, provider) in [
        ("data_keys_required", "epo"),
        ("patent_provider_key_invalid", "uspto"),
    ] {
        let server = MockServer::start().await;
        mount_thing(
            &server,
            ResponseTemplate::new(400).set_body_json(json!({
                "error": { "message": "…", "code": code, "provider": provider },
            })),
        )
        .await;

        let output = request_thing_json(&server).await;

        assert_eq!(output.status.code(), Some(9), "code {code}");
        let value = stdout_json(&output);
        assert_eq!(value["status"], 400);
        assert_eq!(value["providerKeysHint"]["provider"], provider);
        assert_eq!(value["providerKeysHint"]["requiresHumanIntervention"], true);
    }
}

/// Backend ADR 0017: trial-budget exhaustion (429 `trial_data_budget_exhausted`)
/// rides the key-gate exit (9), not the rate-limit exit (6) — the durable
/// recovery is the same human key setup, and the hint carries the reset.
#[tokio::test]
async fn trial_budget_exhaustion_exits_9_not_6() {
    let server = MockServer::start().await;
    mount_thing(
        &server,
        ResponseTemplate::new(429).set_body_json(json!({
            "error": {
                "message": "…",
                "code": "trial_data_budget_exhausted",
                "provider": "epo",
                "remaining": 0,
                "resets_at": "2026-08-29T00:00:00.000Z",
            },
        })),
    )
    .await;

    let output = request_thing_json(&server).await;

    assert_eq!(output.status.code(), Some(9));
    let value = stdout_json(&output);
    assert_eq!(value["providerKeysHint"]["code"], "trial_budget_exhausted");
    assert_eq!(
        value["providerKeysHint"]["resetsAt"],
        "2026-08-29T00:00:00.000Z"
    );
}

/// A 400 that is NOT a key gate keeps the generic failure code — exit 9 means
/// the key gate and nothing else.
#[tokio::test]
async fn a_plain_400_still_exits_1() {
    let server = MockServer::start().await;
    mount_thing(
        &server,
        ResponseTemplate::new(400).set_body_json(json!({
            "error": { "message": "bad query", "code": "INVALID_INPUT" },
        })),
    )
    .await;

    let output = request_thing_json(&server).await;

    assert_eq!(output.status.code(), Some(1));
    assert!(stdout_json(&output).get("providerKeysHint").is_none());
}

/// The local auth guard is the one failure that never reaches the backend, so
/// nothing else can name it: it exits 3 like a rejected 401 and its envelope
/// carries the machine-readable code.
#[tokio::test]
async fn missing_credentials_exit_3_with_an_unauthenticated_code() {
    let output = run_cli(
        "http://127.0.0.1:9",
        &[],
        &["--json", "patent", "search", "--query", "ti=battery"],
    )
    .await;

    assert_eq!(output.status.code(), Some(3));
    let value = stdout_json(&output);
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "unauthenticated");
    assert!(
        value["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("flowleap auth login")),
        "message names the fix: {value}"
    );
}

/// Only failures with a code in the closed registry carry one; everything
/// else keeps the historical message-only envelope, so `error.code` is never
/// something an agent has to second-guess.
#[tokio::test]
async fn failures_without_a_registry_code_carry_none() {
    let output = run_cli(
        "http://127.0.0.1:9",
        &[("FLOWLEAP_API_KEY", "fl_pat_test")],
        &["--json", "patent", "search", "--query", "ti=battery"],
    )
    .await;

    assert_eq!(output.status.code(), Some(7), "network failure");
    let value = stdout_json(&output);
    assert!(
        value["error"]["code"].is_null(),
        "no invented code: {value}"
    );
}
