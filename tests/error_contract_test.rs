//! The `/v1` error contract as the CLI reads it (backend ADR 0014, PRD 0013
//! Phase 2): the key-gate protocol runs on error CODES and the structured
//! `provider` field alone, and a retired endpoint fails loudly with its
//! successor. Backend message wording is freely editable by policy, so nothing
//! here may assert on it — the mock bodies deliberately carry useless messages.

mod support;

use serde_json::{json, Value};
use support::{run_cli, stdout_json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Message text that would once have triggered the message-fragment matching
/// this contract replaced. Present in every body below to prove it is inert.
const MISLEADING_MESSAGE: &str = "EPO_CLIENT_ID / USPTO_ODP_API_KEY not configured";

/// Serve `body` with `status` on GET /v1/thing and run the raw request through.
async fn error_envelope(status: u16, body: Value) -> (std::process::Output, Value) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/thing"))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[],
        &["api", "request", "get", "/v1/thing", "--output", "json"],
    )
    .await;
    let value = stdout_json(&output);
    (output, value)
}

/// Each patent-data-key code produces the hint the key-gate doctrine keys on,
/// naming the office from the structured `provider` field — never from the
/// message.
#[tokio::test]
async fn key_codes_drive_the_provider_keys_hint() {
    let cases = [
        (
            json!({ "error": {
                "message": MISLEADING_MESSAGE,
                "code": "data_keys_required",
                "provider": "epo",
            }}),
            "provider_keys_required",
            "epo",
        ),
        (
            json!({ "error": {
                "message": MISLEADING_MESSAGE,
                "code": "data_keys_required",
                "provider": "uspto",
            }}),
            "provider_keys_required",
            "uspto",
        ),
        (
            json!({ "error": {
                "message": MISLEADING_MESSAGE,
                "code": "patent_provider_key_invalid",
                "provider": "uspto",
            }}),
            "provider_keys_invalid",
            "uspto",
        ),
        // The ODP taxonomy's own missing-key verdict: the code names the
        // office, so it carries no provider field.
        (
            json!({ "error": { "message": MISLEADING_MESSAGE, "code": "odp_api_key_missing" }}),
            "provider_keys_required",
            "uspto",
        ),
    ];

    for (body, expected_code, expected_provider) in cases {
        let (_, value) = error_envelope(400, body.clone()).await;
        let hint = &value["providerKeysHint"];
        assert_eq!(hint["code"], expected_code, "body: {body}");
        assert_eq!(hint["provider"], expected_provider, "body: {body}");
        assert_eq!(hint["requiresHumanIntervention"], true, "body: {body}");
    }
}

/// The inverse, and the whole point of the change: an error whose MESSAGE
/// names the provider env vars but whose CODE is unrelated is not a key gate.
/// A backend reword can no longer invent — or destroy — a gate.
#[tokio::test]
async fn message_text_alone_is_never_a_key_gate() {
    let bodies = [
        json!({ "error": { "message": MISLEADING_MESSAGE, "code": "upstream_error" }}),
        json!({ "error": { "message": MISLEADING_MESSAGE }}),
        json!({ "error": MISLEADING_MESSAGE }),
    ];

    for body in bodies {
        let (_, value) = error_envelope(500, body.clone()).await;
        assert!(
            value.get("providerKeysHint").is_none(),
            "message text must not raise a key gate; body: {body}"
        );
    }
}

/// A key code without the provider field names no office, so it raises no
/// hint: the office is read from the structured field or not at all.
#[tokio::test]
async fn a_key_code_without_a_provider_field_raises_no_hint() {
    let (_, value) = error_envelope(
        400,
        json!({ "error": { "message": MISLEADING_MESSAGE, "code": "data_keys_required" }}),
    )
    .await;
    assert!(value.get("providerKeysHint").is_none());
}

/// A retired endpoint gets its own exit code and a hint relaying the backend's
/// machine-readable successor, so a stale build says exactly what to call.
#[tokio::test]
async fn retired_endpoint_exits_8_and_names_its_successor() {
    let (output, value) = error_envelope(
        410,
        json!({
            "success": false,
            "error": {
                "code": "endpoint_gone",
                "message": "wording the backend is free to change",
                "successor": "POST /v1/tools/search_patents",
                "reason": "ADR 0013: the tools facade is the single agent surface",
            },
            "status": 410,
        }),
    )
    .await;

    assert_eq!(output.status.code(), Some(8));
    let hint = &value["endpointGoneHint"];
    assert_eq!(hint["code"], "endpoint_gone");
    assert_eq!(hint["successor"], "POST /v1/tools/search_patents");
    assert_eq!(
        hint["reason"],
        "ADR 0013: the tools facade is the single agent surface"
    );
    assert_eq!(hint["requiresUpgrade"], true);
}

/// A 410 whose body names no successor still exits 8 and still hints — the
/// capability was withdrawn outright, which the caller must be told.
#[tokio::test]
async fn retired_endpoint_without_a_successor_still_hints() {
    let (output, value) = error_envelope(
        410,
        json!({ "success": false, "error": { "code": "endpoint_gone", "message": "gone" }}),
    )
    .await;

    assert_eq!(output.status.code(), Some(8));
    assert_eq!(value["endpointGoneHint"]["code"], "endpoint_gone");
    assert!(value["endpointGoneHint"].get("successor").is_none());
}

/// In human mode the retirement renders as a box on stderr, naming the
/// successor, and stdout stays free of it.
#[tokio::test]
async fn human_mode_410_renders_a_box_on_stderr() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/thing"))
        .respond_with(ResponseTemplate::new(410).set_body_json(json!({
            "success": false,
            "error": {
                "code": "endpoint_gone",
                "message": "gone",
                "successor": "POST /v1/tools/get_bibliography",
            },
        })))
        .mount(&server)
        .await;

    let output = run_cli(&server.uri(), &[], &["api", "request", "get", "/v1/thing"]).await;

    assert_eq!(output.status.code(), Some(8));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("Endpoint retired") && stderr.contains("get_bibliography"),
        "expected a retirement box naming the successor, got: {stderr}"
    );
}
