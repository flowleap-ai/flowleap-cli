//! Doctor readiness contract (issue #43): `--json doctor` always carries
//! `ready` and actor-tagged pending-only `nextSteps`, exits 0 iff ready, and
//! keeps every pre-existing field — driven through the real binary against a
//! wiremock backend. The contract is recorded in docs/adr/0001.
//!
//! Human rendering (issue #44): bare `flowleap doctor` renders a ✓/✗/•
//! checklist plus a numbered actor-tagged next-steps list instead of raw
//! JSON, under the same exit contract. Human-mode assertions are
//! contains-style on stable lines, never full-output goldens.

mod support;

use serde_json::{json, Value};
use support::{run_cli, stdout_json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Mount a healthy GET /health.
async fn mount_health_ok(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "status": "ok" })))
        .mount(server)
        .await;
}

/// Mount POST /v1/keys/validate answering with the given per-provider verdicts.
async fn mount_validate(server: &MockServer, providers: Value) {
    Mock::given(method("POST"))
        .and(path("/v1/keys/validate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "providers": providers })))
        .mount(server)
        .await;
}

/// Mount GET /api/profile with the normalized subscription entitlement
/// contract consumed by doctor.
async fn mount_subscription(server: &MockServer, subscription: Value) {
    Mock::given(method("GET"))
        .and(path("/api/profile"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "subscription": subscription })),
        )
        .mount(server)
        .await;
}

async fn mount_active_subscription(server: &MockServer) {
    mount_subscription(
        server,
        json!({
            "entitled": true,
            "status": "active",
            "plan": "Basic",
            "action": null,
        }),
    )
    .await;
}

/// The step ids of a report's nextSteps, in order.
fn step_ids(report: &Value) -> Vec<&str> {
    report["nextSteps"]
        .as_array()
        .expect("nextSteps must always be an array")
        .iter()
        .map(|step| step["id"].as_str().expect("every step has an id"))
        .collect()
}

/// Every pre-existing doctor field must still be present (additive-only).
fn assert_existing_fields(report: &Value) {
    assert!(report["ok"].is_boolean(), "ok: {report}");
    assert!(report["baseUrl"].is_string(), "baseUrl: {report}");
    assert!(report["auth"]["available"].is_boolean(), "auth: {report}");
    assert!(
        report["auth"]["source"].is_string(),
        "auth.source: {report}"
    );
    assert!(report["config"]["path"].is_string(), "config: {report}");
    assert!(
        report["providerKeys"]["epo"].is_boolean() && report["providerKeys"]["uspto"].is_boolean(),
        "providerKeys: {report}"
    );
    assert!(
        report["backend"]["reachable"].is_boolean(),
        "backend: {report}"
    );
    assert!(report["cli"]["currentVersion"].is_string(), "cli: {report}");
    assert!(report["skills"].is_object(), "skills: {report}");
}

/// Fully ready machine: reachable, durable fl_pat_ auth, server-covered keys
/// → exit 0, `ready: true`, empty nextSteps, existing fields intact.
#[tokio::test]
async fn ready_machine_exits_0_with_empty_next_steps() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;
    mount_subscription(
        &server,
        json!({
            "entitled": true,
            "status": "active",
            "plan": "Basic",
            "action": null,
        }),
    )
    .await;
    mount_validate(
        &server,
        json!({
            "epo": { "source": "server", "valid": true },
            "uspto": { "source": "server", "valid": true },
        }),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[("FLOWLEAP_API_KEY", "fl_pat_test")],
        &["--json", "doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(0));
    let report = stdout_json(&output);
    assert_eq!(report["ready"], true);
    assert_eq!(report["nextSteps"], json!([]));
    assert_eq!(report["ok"], true);
    assert_eq!(report["backend"]["healthStatus"], 200);
    assert_eq!(report["keyValidation"]["source"], "server");
    assert_eq!(report["subscription"]["source"], "profile");
    assert_eq!(report["subscription"]["entitled"], true);
    assert_eq!(report["subscription"]["status"], "active");
    assert_existing_fields(&report);
}

#[tokio::test]
async fn trialing_subscription_is_entitled_and_ready() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;
    mount_subscription(
        &server,
        json!({
            "entitled": true,
            "status": "trialing",
            "plan": "Basic",
            "action": null,
        }),
    )
    .await;
    mount_validate(
        &server,
        json!({
            "epo": { "source": "server", "valid": true },
            "uspto": { "source": "server", "valid": true },
        }),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[("FLOWLEAP_API_KEY", "fl_pat_test")],
        &["--json", "doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(0));
    let report = stdout_json(&output);
    assert_eq!(report["ready"], true);
    assert_eq!(report["subscription"]["entitled"], true);
    assert_eq!(report["subscription"]["status"], "trialing");
}

#[tokio::test]
async fn inactive_subscription_surfaces_backend_human_action() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;
    mount_subscription(
        &server,
        json!({
            "entitled": false,
            "status": "inactive",
            "plan": "Basic",
            "action": {
                "id": "subscribe-basic",
                "title": "Subscribe to FlowLeap Basic",
                "url": "https://flowleap.co/en/pricing",
            },
        }),
    )
    .await;
    mount_validate(
        &server,
        json!({
            "epo": { "source": "server", "valid": true },
            "uspto": { "source": "server", "valid": true },
        }),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[("FLOWLEAP_API_KEY", "fl_pat_test")],
        &["--json", "doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let report = stdout_json(&output);
    assert_eq!(report["ready"], false);
    assert_eq!(report["subscription"]["source"], "profile");
    assert_eq!(report["subscription"]["entitled"], false);
    assert_eq!(step_ids(&report), ["subscribe-basic"]);
    assert_eq!(report["nextSteps"][0]["actor"], "human");
    assert_eq!(
        report["nextSteps"][0]["url"],
        "https://flowleap.co/en/pricing"
    );
}

#[tokio::test]
async fn unavailable_profile_is_unknown_and_never_assumes_purchase() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;
    Mock::given(method("GET"))
        .and(path("/api/profile"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({ "error": "boom" })))
        .mount(&server)
        .await;
    mount_validate(
        &server,
        json!({
            "epo": { "source": "server", "valid": true },
            "uspto": { "source": "server", "valid": true },
        }),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[
            ("FLOWLEAP_API_KEY", "fl_pat_test"),
            ("FLOWLEAP_MAX_RETRIES", "0"),
        ],
        &["--json", "doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let report = stdout_json(&output);
    assert_eq!(report["ready"], false);
    assert_eq!(report["subscription"]["source"], "unknown");
    assert_eq!(report["subscription"]["entitled"], Value::Null);
    assert_eq!(step_ids(&report), ["verify-subscription"]);
    assert_eq!(report["nextSteps"][0]["actor"], "agent");
    assert!(
        !report.to_string().contains("subscribe-basic"),
        "unknown must not guess a billing action: {report}"
    );
}

#[tokio::test]
async fn malformed_profile_is_unknown_and_never_assumes_purchase() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;
    mount_subscription(
        &server,
        json!({
            "status": "inactive",
            "plan": "Basic",
            "action": null,
        }),
    )
    .await;
    mount_validate(
        &server,
        json!({
            "epo": { "source": "server", "valid": true },
            "uspto": { "source": "server", "valid": true },
        }),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[("FLOWLEAP_API_KEY", "fl_pat_test")],
        &["--json", "doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let report = stdout_json(&output);
    assert_eq!(report["subscription"]["source"], "unknown");
    assert_eq!(step_ids(&report), ["verify-subscription"]);
    assert!(
        !report.to_string().contains("subscribe-basic"),
        "malformed profile must not guess a billing action: {report}"
    );
}

/// Unauthenticated: auth-login comes first with a human actor and a runnable
/// command; provider steps fall back to local presence; exit 1 with the
/// checklist still fully emitted. `ok` keeps its reachability meaning.
#[tokio::test]
async fn unauthenticated_lists_auth_login_first_and_exits_1() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;

    let output = run_cli(&server.uri(), &[], &["--json", "doctor"]).await;

    assert_eq!(output.status.code(), Some(1));
    let report = stdout_json(&output);
    assert_eq!(report["ok"], true, "ok stays reachability: {report}");
    assert_eq!(report["ready"], false);
    assert_eq!(
        step_ids(&report),
        [
            "auth-login",
            "obtain-epo-keys",
            "store-epo-keys",
            "obtain-uspto-key",
            "store-uspto-key",
            "verify-keys",
        ]
    );
    let login = &report["nextSteps"][0];
    assert_eq!(login["actor"], "human");
    assert_eq!(login["run"], "flowleap --json auth login");
    assert!(login["title"].as_str().is_some_and(|t| !t.is_empty()));
    // Unauthenticated → no server validation → local-presence fallback + note.
    assert_eq!(report["keyValidation"]["source"], "local");
    assert!(
        report["keyValidation"]["note"]
            .as_str()
            .is_some_and(|n| !n.is_empty()),
        "fallback must carry a note: {report}"
    );
    assert_existing_fields(&report);
}

/// Session-only auth (Clerk token, no fl_pat_ personal token) pends exactly
/// the mint-personal-token agent step.
#[tokio::test]
async fn session_only_auth_pends_mint_personal_token() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;
    mount_active_subscription(&server).await;
    mount_validate(
        &server,
        json!({
            "epo": { "source": "server", "valid": true },
            "uspto": { "source": "server", "valid": true },
        }),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[("FLOWLEAP_TOKEN", "clerk-session-jwt")],
        &["--json", "doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let report = stdout_json(&output);
    assert_eq!(report["ready"], false);
    assert_eq!(step_ids(&report), ["mint-personal-token"]);
    let mint = &report["nextSteps"][0];
    assert_eq!(mint["actor"], "agent");
    assert_eq!(
        mint["run"],
        "flowleap --json auth create-token --name <n> --store"
    );
}

/// Server-covered providers produce no next steps; only the genuinely
/// blocking provider appears, split into human (obtain) and agent (store)
/// steps plus a final verify.
#[tokio::test]
async fn server_covered_provider_steps_are_omitted() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;
    mount_active_subscription(&server).await;
    mount_validate(
        &server,
        json!({
            "epo": { "source": "server", "valid": true },
            "uspto": { "source": "none", "valid": null },
        }),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[("FLOWLEAP_API_KEY", "fl_pat_test")],
        &["--json", "doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let report = stdout_json(&output);
    assert_eq!(
        step_ids(&report),
        ["obtain-uspto-key", "store-uspto-key", "verify-keys"]
    );
    let obtain = &report["nextSteps"][0];
    assert_eq!(obtain["actor"], "human");
    assert_eq!(obtain["url"], "https://data.uspto.gov/apis/getting-started");
    let store = &report["nextSteps"][1];
    assert_eq!(store["actor"], "agent");
    assert_eq!(store["run"], "flowleap keys set uspto --key <k>");
    let verify = &report["nextSteps"][2];
    assert_eq!(verify["actor"], "agent");
    assert_eq!(verify["run"], "flowleap --json keys test");
}

/// User keys the server rejected are blocking: re-obtain + re-store + verify.
#[tokio::test]
async fn rejected_user_keys_are_blocking() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;
    mount_active_subscription(&server).await;
    mount_validate(
        &server,
        json!({
            "epo": { "source": "user", "valid": false, "message": "rejected" },
            "uspto": { "source": "server", "valid": true },
        }),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[
            ("FLOWLEAP_API_KEY", "fl_pat_test"),
            ("FLOWLEAP_EPO_KEY", "stale-key"),
            ("FLOWLEAP_EPO_SECRET", "stale-secret"),
        ],
        &["--json", "doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let report = stdout_json(&output);
    assert_eq!(
        step_ids(&report),
        ["obtain-epo-keys", "store-epo-keys", "verify-keys"]
    );
    assert_eq!(report["nextSteps"][0]["url"], "https://developers.epo.org");
}

/// A failing /v1/keys/validate never errors doctor: it falls back to local
/// key presence (with a note) — here everything is present locally, so the
/// machine still reads ready and exits 0.
#[tokio::test]
async fn validation_call_failure_falls_back_to_local_presence() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;
    mount_active_subscription(&server).await;
    Mock::given(method("POST"))
        .and(path("/v1/keys/validate"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({ "error": "boom" })))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[
            ("FLOWLEAP_API_KEY", "fl_pat_test"),
            ("FLOWLEAP_EPO_KEY", "local-epo-key"),
            ("FLOWLEAP_EPO_SECRET", "local-epo-secret"),
            ("FLOWLEAP_USPTO_KEY", "local-uspto-key"),
            ("FLOWLEAP_MAX_RETRIES", "0"),
        ],
        &["--json", "doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(0));
    let report = stdout_json(&output);
    assert_eq!(report["ready"], true);
    assert_eq!(report["nextSteps"], json!([]));
    assert_eq!(report["keyValidation"]["source"], "local");
    assert!(
        report["keyValidation"]["note"]
            .as_str()
            .is_some_and(|n| !n.is_empty()),
        "fallback must carry a note: {report}"
    );
}

/// Missing local keys under the same fallback are blocking again — presence
/// is the criterion when the server can't answer.
#[tokio::test]
async fn validation_call_failure_with_missing_local_keys_blocks() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;
    mount_active_subscription(&server).await;
    Mock::given(method("POST"))
        .and(path("/v1/keys/validate"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({ "error": "boom" })))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[
            ("FLOWLEAP_API_KEY", "fl_pat_test"),
            ("FLOWLEAP_MAX_RETRIES", "0"),
        ],
        &["--json", "doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let report = stdout_json(&output);
    assert_eq!(report["ready"], false);
    assert_eq!(
        step_ids(&report),
        [
            "obtain-epo-keys",
            "store-epo-keys",
            "obtain-uspto-key",
            "store-uspto-key",
            "verify-keys",
        ]
    );
}

/// Unreachable backend: the offline checklist is still fully emitted (from
/// local state) and the run exits 1.
#[tokio::test]
async fn unreachable_backend_emits_offline_checklist_and_exits_1() {
    // Port 9 (discard) is closed on any sane test machine; no server started.
    let output = run_cli(
        "http://127.0.0.1:9",
        &[("FLOWLEAP_MAX_RETRIES", "0")],
        &["--json", "doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let report = stdout_json(&output);
    assert_eq!(report["ok"], false);
    assert_eq!(report["ready"], false);
    assert_eq!(report["backend"]["reachable"], false);
    assert_eq!(step_ids(&report)[0], "auth-login");
    assert!(
        step_ids(&report).contains(&"verify-keys"),
        "offline checklist still lists provider steps: {report}"
    );
    assert_existing_fields(&report);
}

/// An authenticated offline machine cannot verify entitlement, so it is
/// not-ready and carries the agent-owned verification step.
#[tokio::test]
async fn unreachable_backend_with_credentials_is_not_ready() {
    let output = run_cli(
        "http://127.0.0.1:9",
        &[
            ("FLOWLEAP_API_KEY", "fl_pat_test"),
            ("FLOWLEAP_EPO_KEY", "k"),
            ("FLOWLEAP_EPO_SECRET", "s"),
            ("FLOWLEAP_USPTO_KEY", "u"),
            ("FLOWLEAP_MAX_RETRIES", "0"),
        ],
        &["--json", "doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let report = stdout_json(&output);
    assert_eq!(step_ids(&report), ["verify-subscription"]);
    assert_eq!(report["subscription"]["source"], "not-checked");
    assert_eq!(report["ready"], false, "unreachable is never ready");
}

// ---------------------------------------------------------------------------
// Human rendering (issue #44) — bare `flowleap doctor`, no --json.
// ---------------------------------------------------------------------------

/// A run's stdout as UTF-8, after asserting it is not raw JSON — human mode
/// must never dump the report object.
fn stdout_human(output: &std::process::Output) -> String {
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is utf8");
    assert!(
        !stdout.trim_start().starts_with('{'),
        "human mode must not emit raw JSON: {stdout}"
    );
    stdout
}

/// Fully ready machine (valid local user keys): every checklist line is ✓,
/// there is no "Next steps:" section at all, and the run exits 0.
#[tokio::test]
async fn human_ready_machine_renders_all_check_marks_and_no_next_steps() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;
    mount_active_subscription(&server).await;
    mount_validate(
        &server,
        json!({
            "epo": { "source": "user", "valid": true },
            "uspto": { "source": "user", "valid": true },
        }),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[
            ("FLOWLEAP_API_KEY", "fl_pat_test"),
            ("FLOWLEAP_EPO_KEY", "k"),
            ("FLOWLEAP_EPO_SECRET", "s"),
            ("FLOWLEAP_USPTO_KEY", "u"),
        ],
        &["doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(0));
    let stdout = stdout_human(&output);
    assert!(stdout.contains("FlowLeap doctor"), "header: {stdout}");
    assert!(stdout.contains("✓ Backend reachable"), "backend: {stdout}");
    assert!(
        stdout.contains("✓ Authenticated (personal token)"),
        "auth: {stdout}"
    );
    assert!(
        stdout.contains("✓ Subscription entitled (active)"),
        "subscription: {stdout}"
    );
    assert!(stdout.contains("✓ EPO keys: set locally"), "epo: {stdout}");
    assert!(
        stdout.contains("✓ USPTO key: set locally"),
        "uspto: {stdout}"
    );
    assert!(stdout.contains("✓ CLI"), "cli: {stdout}");
    assert!(stdout.contains("✓ Skills up to date"), "skills: {stdout}");
    assert!(!stdout.contains("✗"), "ready shows no ✗: {stdout}");
    assert!(!stdout.contains("•"), "ready shows no •: {stdout}");
    assert!(
        !stdout.contains("Next steps:"),
        "ready machine has no next-steps section: {stdout}"
    );
}

/// Unauthenticated: the auth line is ✗ and the numbered next-steps list opens
/// with the [human]-tagged sign-in step carrying its runnable command; the
/// run exits 1.
#[tokio::test]
async fn human_unauthenticated_renders_cross_and_actor_tagged_steps() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;

    let output = run_cli(&server.uri(), &[], &["doctor"]).await;

    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout_human(&output);
    assert!(stdout.contains("✗ Not signed in"), "auth line: {stdout}");
    assert!(stdout.contains("Next steps:"), "section: {stdout}");
    assert!(stdout.contains("1. [human]"), "numbered tag: {stdout}");
    assert!(
        stdout.contains("flowleap --json auth login"),
        "login command: {stdout}"
    );
    assert!(stdout.contains("[agent]"), "agent-tagged steps: {stdout}");
    assert!(
        stdout.contains("https://data.uspto.gov/apis/getting-started"),
        "human step URL on its own line: {stdout}"
    );
}

/// A provider the server covers renders as an informational • line and gets
/// no next step, while the genuinely blocking provider renders ✗ ("not
/// covered" — the server verdict proved it) with its steps listed.
#[tokio::test]
async fn human_server_covered_provider_renders_dot_and_no_step() {
    let server = MockServer::start().await;
    mount_health_ok(&server).await;
    mount_active_subscription(&server).await;
    mount_validate(
        &server,
        json!({
            "epo": { "source": "server", "valid": true },
            "uspto": { "source": "none", "valid": null },
        }),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[("FLOWLEAP_API_KEY", "fl_pat_test")],
        &["doctor"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout_human(&output);
    assert!(
        stdout.contains("• EPO keys: none locally — covered by server"),
        "covered provider is informational: {stdout}"
    );
    assert!(
        stdout.contains("✗ USPTO key: not set, not covered"),
        "blocking provider: {stdout}"
    );
    assert!(
        !stdout.contains("EPO consumer key"),
        "no step for the covered provider: {stdout}"
    );
    assert!(
        stdout.contains("Store the USPTO ODP API key"),
        "blocking provider keeps its steps: {stdout}"
    );
}
