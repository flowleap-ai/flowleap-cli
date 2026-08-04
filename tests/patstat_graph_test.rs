//! `flowleap patstat graph resolve` (issue #54, PRD 0002): the three success
//! kinds, the ambiguity interaction step, and the typed graph error family —
//! driven through the real binary against a wiremock backend, matching the
//! exit-code contract's test harness (see tests/exit_codes_test.rs).
//!
//! Two invariants this file locks for the whole graph family (#55, #56 copy
//! them): `--json` is ALWAYS the backend body unmodified — success kinds and
//! typed error bodies alike — and ambiguity never exits 0.

mod support;

use serde_json::json;
use support::{run_cli, stdout_json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const API_KEY_ENV: (&str, &str) = ("FLOWLEAP_API_KEY", "fl_pat_test_key");

/// Canned `kind: "patent"` body, shaped like the real backend response
/// (flowleap-backend src/lib/patstat-graph/resolve.ts → ResolveResult).
fn patent_body() -> serde_json::Value {
    json!({
        "success": true,
        "kind": "patent",
        "input": "EP3477840",
        "anchor": {
            "node": "pat:56123456",
            "appln_id": 56123456,
            "application": "EP18000829 (A)",
            "title": "Method for operating a wind turbine",
            "granted": true,
            "filing_year": 2018,
            "docdb_family_id": 65432100,
            "publications": [
                {
                    "publn": "EP3477840A1",
                    "kind": "A1",
                    "date": "2019-05-01",
                    "first_grant": false,
                    "at": "tls211:530028653",
                },
                {
                    "publn": "EP3477840B1",
                    "kind": "B1",
                    "date": "2021-03-17",
                    "first_grant": true,
                    "at": "tls211:530028654",
                },
            ],
            "confidence": "EXTRACTED",
            "at": "tls201:56123456",
        },
    })
}

/// Canned `kind: "ambiguous"` body — one number, several distinct
/// applications. HTTP 200: the backend resolved fine, the caller must pick.
fn ambiguous_body() -> serde_json::Value {
    json!({
        "success": true,
        "kind": "ambiguous",
        "input": "US5960411",
        "candidates": [
            {
                "node": "pat:11111111",
                "appln_id": 11111111,
                "application": "US08655468 (A)",
                "title": "Method and system for placing a purchase order",
                "granted": true,
                "filing_year": 1996,
                "docdb_family_id": 22222222,
                "publications": [
                    { "publn": "US5960411A", "kind": "A", "date": "1999-09-28",
                      "first_grant": true, "at": "tls211:1" },
                ],
                "confidence": "EXTRACTED",
                "at": "tls201:11111111",
            },
            {
                "node": "pat:33333333",
                "appln_id": 33333333,
                "application": "US09123456 (A)",
                "title": null,
                "granted": false,
                "filing_year": null,
                "docdb_family_id": 44444444,
                "publications": [
                    { "publn": "US5960411B1", "kind": "B1", "date": "2001-02-02",
                      "first_grant": false, "at": "tls211:2" },
                ],
                "confidence": "EXTRACTED",
                "at": "tls201:33333333",
            },
        ],
    })
}

/// Canned `kind: "entities"` body — free text → ranked harmonized entities,
/// truncated against a larger true total.
fn entities_body() -> serde_json::Value {
    json!({
        "success": true,
        "kind": "entities",
        "input": "Siemens",
        "candidates": [
            {
                "node": "person:98765",
                "psn_id": 98765,
                "name": "SIEMENS AG",
                "applications": 184532,
                "person_variants": 412,
                "confidence": "INFERRED",
                "at": "tls206:psn/98765",
            },
            {
                "node": "person:98766",
                "psn_id": 98766,
                "name": "SIEMENS HEALTHCARE GMBH",
                "applications": 12045,
                "person_variants": 37,
                "confidence": "INFERRED",
                "at": "tls206:psn/98766",
            },
        ],
        "truncated": true,
        "total": 137,
    })
}

/// Canned typed graph error body (unified FlowLeap envelope: the backend
/// spreads `details` into `error`, so candidates ride at `error.candidates`).
fn error_body(code: &str, message: &str, status: u16) -> serde_json::Value {
    json!({
        "success": false,
        "error": { "code": code, "message": message },
        "status": status,
    })
}

async fn mount_resolve(server: &MockServer, template: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/v1/patstat/graph/resolve"))
        .respond_with(template)
        .mount(server)
        .await;
}

#[tokio::test]
async fn resolve_sends_the_query_as_the_q_parameter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/patstat/graph/resolve"))
        .and(query_param("q", "EP3477840"))
        .respond_with(ResponseTemplate::new(200).set_body_json(patent_body()))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "graph", "resolve", "EP3477840"],
    )
    .await;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A company name carries spaces and punctuation — it must reach the backend
/// percent-encoded, not as a broken query string. Asserted through `--dry-run`
/// so the encoding is checked without depending on a server's own parsing.
#[tokio::test]
async fn resolve_url_encodes_free_text_queries() {
    let output = run_cli(
        "http://127.0.0.1:9",
        &[API_KEY_ENV],
        &[
            "--json",
            "--dry-run",
            "patstat",
            "graph",
            "resolve",
            "Kia Motors & Co",
        ],
    )
    .await;

    assert!(output.status.success());
    let value = stdout_json(&output);
    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "GET");
    assert_eq!(
        value["url"],
        "http://127.0.0.1:9/v1/patstat/graph/resolve?q=Kia%20Motors%20%26%20Co"
    );
}

#[tokio::test]
async fn patent_kind_renders_the_anchor_in_human_mode() {
    let server = MockServer::start().await;
    mount_resolve(
        &server,
        ResponseTemplate::new(200).set_body_json(patent_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "resolve", "EP3477840"],
    )
    .await;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");

    assert!(stdout.contains("Anchor: pat:56123456"));
    assert!(stdout.contains("Application: EP18000829 (A)"));
    assert!(stdout.contains("Title: Method for operating a wind turbine"));
    assert!(stdout.contains("Filed: 2018"));
    assert!(stdout.contains("Granted: yes"));
    // Both publications of the one application, with their provenance refs.
    assert!(stdout.contains("EP3477840A1"));
    assert!(stdout.contains("EP3477840B1"));
    assert!(stdout.contains("at=tls211:530028654"));
    assert!(stdout.contains("EXTRACTED"));
    // Rendered, not dumped.
    assert!(!stdout.contains("\"anchor\""));
}

#[tokio::test]
async fn patent_kind_json_mode_emits_the_backend_body_untouched() {
    let server = MockServer::start().await;
    mount_resolve(
        &server,
        ResponseTemplate::new(200).set_body_json(patent_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "graph", "resolve", "EP3477840"],
    )
    .await;

    assert!(output.status.success());
    let value = stdout_json(&output);

    assert_eq!(value, patent_body());
    // json mode passes the endpoint body through untouched — no CLI wrapper.
    assert!(value.get("ok").is_none());
}

/// Free text resolves to a ranked pick-one list. That is a successful
/// resolution (exit 0), unlike an ambiguous NUMBER — the caller asked an
/// open question and got the ranked answer to it.
#[tokio::test]
async fn entities_kind_ranks_candidates_and_states_the_true_total() {
    let server = MockServer::start().await;
    mount_resolve(
        &server,
        ResponseTemplate::new(200).set_body_json(entities_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "resolve", "Siemens"],
    )
    .await;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");

    assert!(stdout.contains("Applicant entities matching \"Siemens\""));
    // Truncation honesty: the shown count is never presented as the total.
    assert!(stdout.contains("Showing 2 of 137 matching entities."));
    assert!(stdout.contains("SIEMENS AG"));
    assert!(stdout.contains("98765"));
    assert!(stdout.contains("184532"));
    // The psn_id is the anchor the applicant landscape takes.
    assert!(stdout.contains("graph applicant"));
}

#[tokio::test]
async fn entities_kind_json_mode_emits_the_backend_body_untouched() {
    let server = MockServer::start().await;
    mount_resolve(
        &server,
        ResponseTemplate::new(200).set_body_json(entities_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "graph", "resolve", "Siemens"],
    )
    .await;

    assert!(output.status.success());
    assert_eq!(stdout_json(&output), entities_body());
}

/// One number behind several distinct applications: the candidates are shown
/// with the kind codes that discriminate them, nothing is picked, and the run
/// exits non-zero so a script cannot read a pick-one prompt as an anchor.
#[tokio::test]
async fn ambiguous_kind_prints_candidates_and_exits_non_zero() {
    let server = MockServer::start().await;
    mount_resolve(
        &server,
        ResponseTemplate::new(200).set_body_json(ambiguous_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "resolve", "US5960411"],
    )
    .await;

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");

    assert!(stdout.contains("Ambiguous publication number"));
    assert!(stdout.contains("matches 2 distinct applications"));
    assert!(stdout.contains("pat:11111111"));
    assert!(stdout.contains("US08655468 (A)"));
    assert!(stdout.contains("pat:33333333"));
    assert!(stdout.contains("US5960411A"));
    assert!(stdout.contains("US5960411B1"));
    // The 9999 unknown-year sentinel is never quoted as a year.
    assert!(stdout.contains("filed unknown"));
    assert!(!stdout.contains("9999"));
    assert!(stdout.contains("None is picked automatically."));
}

#[tokio::test]
async fn ambiguous_kind_json_mode_emits_the_body_untouched_and_exits_non_zero() {
    let server = MockServer::start().await;
    mount_resolve(
        &server,
        ResponseTemplate::new(200).set_body_json(ambiguous_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "graph", "resolve", "US5960411"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout_json(&output), ambiguous_body());
}

/// A parseable number with no row in the loaded edition. The backend message
/// carries the recovery action (country prefix, snapshot recency) and is
/// relayed verbatim; 404 maps to the documented not-found exit code.
#[tokio::test]
async fn patent_not_found_404_relays_the_message_and_exits_not_found() {
    let server = MockServer::start().await;
    let body = error_body(
        "patstat_patent_not_found",
        "No publication in the loaded PATSTAT edition matches \"EP9999999\". Check the number, \
         or include the country prefix (e.g. EP3477840, US5960411) — very recent publications \
         may postdate the edition snapshot.",
        404,
    );
    mount_resolve(&server, ResponseTemplate::new(404).set_body_json(body)).await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "resolve", "EP9999999"],
    )
    .await;

    assert_eq!(output.status.code(), Some(5));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("No such publication in the loaded PATSTAT edition."));
    assert!(stdout.contains("may postdate the edition snapshot"));
}

#[tokio::test]
async fn entity_not_found_404_json_mode_emits_the_error_body_untouched() {
    let server = MockServer::start().await;
    let body = error_body(
        "patstat_entity_not_found",
        "No harmonized (PSN) applicant entity matches \"Zzzz\". Try a shorter name prefix.",
        404,
    );
    mount_resolve(
        &server,
        ResponseTemplate::new(404).set_body_json(body.clone()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "graph", "resolve", "Zzzz"],
    )
    .await;

    assert_eq!(output.status.code(), Some(5));
    // Typed graph errors relay the backend body unmodified, same as successes.
    assert_eq!(stdout_json(&output), body);
}

/// `patstat_patent_ambiguous` (422) is the composites' ambiguity error — the
/// same interaction step, reached through the error envelope. #56's verbs
/// inherit this rendering from the shared graph error path.
#[tokio::test]
async fn patent_ambiguous_422_renders_the_error_candidates() {
    let server = MockServer::start().await;
    let mut body = error_body(
        "patstat_patent_ambiguous",
        "\"US5960411\" matches 2 distinct applications: US08655468 (A), US09123456 (A). Add the \
         kind code (e.g. A1 vs B1) or use a fuller number form to disambiguate.",
        422,
    );
    body["error"]["candidates"] = ambiguous_body()["candidates"].clone();
    mount_resolve(&server, ResponseTemplate::new(422).set_body_json(body)).await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "resolve", "US5960411"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("Ambiguous publication number"));
    assert!(stdout.contains("US08655468 (A)"));
    assert!(stdout.contains("US09123456 (A)"));
    assert!(stdout.contains("None is picked automatically."));
}

#[tokio::test]
async fn invalid_request_400_relays_the_backend_message() {
    let server = MockServer::start().await;
    let body = error_body(
        "patstat_invalid_request",
        "`q` must be a publication number (e.g. EP3477840) or an applicant name of at least 2 \
         characters.",
        400,
    );
    mount_resolve(&server, ResponseTemplate::new(400).set_body_json(body)).await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "resolve", "x"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("Invalid graph request."));
    assert!(stdout.contains("an applicant name of at least 2"));
}

#[tokio::test]
async fn patstat_unavailable_503_renders_plainly() {
    let server = MockServer::start().await;
    let body = error_body(
        "patstat_unavailable",
        "The PATSTAT analytics layer is not configured on this deployment.",
        503,
    );
    mount_resolve(&server, ResponseTemplate::new(503).set_body_json(body)).await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "resolve", "EP3477840"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout
        .contains("PATSTAT analytics unavailable: backend has no PATSTAT dataset configured."));
}

/// Anything outside the typed graph family keeps the CLI-wide envelope +
/// hint-box rendering and its documented exit code — the graph verbs do not
/// invent their own handling for auth, rate limits, or upstream failures.
#[tokio::test]
async fn untyped_failure_falls_back_to_the_shared_envelope() {
    let server = MockServer::start().await;
    mount_resolve(
        &server,
        ResponseTemplate::new(401).set_body_json(json!({ "error": "unauthorized" })),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "graph", "resolve", "EP3477840"],
    )
    .await;

    assert_eq!(output.status.code(), Some(3));
    let value = stdout_json(&output);
    assert_eq!(value["ok"], false);
    assert_eq!(value["status"], 401);
}

/// With no credentials configured at all, the command fails fast locally
/// (never reaches the network) — the `require_auth` guard `patstat` applies
/// before dispatching to any subcommand, graph included.
#[tokio::test]
async fn missing_credentials_fails_locally_without_a_network_call() {
    let output = run_cli(
        "http://127.0.0.1:9",
        &[],
        &["--json", "patstat", "graph", "resolve", "EP3477840"],
    )
    .await;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stderr.contains("Not authenticated") || stdout.contains("Not authenticated"),
        "stdout: {stdout}\nstderr: {stderr}"
    );
}
