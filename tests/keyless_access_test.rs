//! Keyless access (backend ADR 0026): the publication-index search of
//! `patent search` (the default, and `--provider index`), the dated answers of the four tools with
//! a hosted copy, and the "not now" refusals.
//!
//! The backend has all of this behind one flag. These tests pin the CLI
//! against BOTH states: the flag on (the index answers, a dated answer has
//! `live: false`) and the flag off (the schema refuses provider `index`).
//! `--json` stdout stays the backend payload verbatim.

mod support;

use serde_json::{json, Value};
use support::{run_cli, stdout_json, tool_ok};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn run(server: &MockServer, args: &[&str]) -> std::process::Output {
    run_cli(&server.uri(), &[("FLOWLEAP_API_KEY", "fl_pat_test")], args).await
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is utf8")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf8")
}

async fn mount_tool_for(server: &MockServer, tool: &str, expected_input: Value, data: Value) {
    Mock::given(method("POST"))
        .and(path(format!("/v1/tools/{tool}")))
        .and(body_json(expected_input))
        .respond_with(ResponseTemplate::new(200).set_body_json(tool_ok(tool, data)))
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_failure(server: &MockServer, tool: &str, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path(format!("/v1/tools/{tool}")))
        .respond_with(response)
        .mount(server)
        .await;
}

const ATTRIBUTION: &str = "Contains data from Google Patents Public Datasets (IFI CLAIMS Patent Services and Google), licensed under CC BY 4.0.";

/// A publication-index answer in the served shape.
fn index_answer() -> Value {
    json!({
        "query": "\"solid state\" battery",
        "total": 1234,
        "returned": 1,
        "range": { "begin": 1, "end": 1 },
        "sort": "date",
        "docs": [{
            "docId": "EP4000001B1",
            "title": "Solid state battery",
            "abstract": "A battery.",
            "applicants": ["TOYOTA MOTOR CORP"],
            "publicationDate": "2025-03-12",
            "kind": "B1",
            "cpc": ["H01M10/0562"]
        }],
        "source": "publication_index",
        "asOf": "2026-09-23",
        "attribution": {
            "source": "Google Patents Public Datasets",
            "license": "CC BY 4.0",
            "text": ATTRIBUTION
        }
    })
}

// ----------------------------------------------------------------------------
// patent search --provider index
// ----------------------------------------------------------------------------

#[tokio::test]
async fn the_index_search_sends_the_provider_the_page_and_the_filters() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "search_patents",
        json!({
            "query": "\"solid state\" battery",
            "provider": "index",
            "range": "11-15",
            "cpc": ["H01M", "H01G"],
            "applicant": "toyota",
            "inventor": "sato",
            "office": ["EP", "US"],
            "kind": "B",
            "from": "2020-01-01",
            "to": "2025-12-31",
            "sort": "date"
        }),
        index_answer(),
    )
    .await;

    let output = run(
        &server,
        &[
            "--json",
            "patent",
            "search",
            "--provider",
            "index",
            "--query",
            "\"solid state\" battery",
            "--limit",
            "5",
            "--offset",
            "10",
            "--cpc",
            "h01m,H01G",
            "--applicant",
            "toyota",
            "--inventor",
            "sato",
            "--office",
            "ep,us",
            "--kind",
            "B",
            "--from",
            "2020-01-01",
            "--to",
            "2025-12-31",
            "--sort",
            "date",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    // JSON is the backend payload, unchanged.
    assert_eq!(stdout_json(&output), index_answer());
}

#[tokio::test]
async fn the_index_table_shows_the_rows_the_total_the_edition_and_the_credit() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "search_patents",
        json!({ "query": "battery", "provider": "index", "range": "1-10" }),
        index_answer(),
    )
    .await;

    let output = run(
        &server,
        &[
            "--output",
            "table",
            "patent",
            "search",
            "--provider",
            "index",
            "--query",
            "battery",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let table = stdout(&output);
    assert!(table.contains("EP4000001B1"), "{table}");
    assert!(table.contains("Total: 1234"), "{table}");
    assert!(table.contains("As of: 2026-09-23"), "{table}");
    assert!(table.contains(ATTRIBUTION), "{table}");
}

#[tokio::test]
async fn the_index_count_probe_asks_for_one_row_and_prints_the_total() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "search_patents",
        json!({ "query": "*", "provider": "index", "range": "1-1", "cpc": "H01M" }),
        index_answer(),
    )
    .await;

    let output = run(
        &server,
        &[
            "--output",
            "table",
            "patent",
            "search",
            "--provider",
            "index",
            "--query",
            "*",
            "--cpc",
            "H01M",
            "--count-only",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("Total: 1234"), "{text}");
    assert!(!text.contains("EP4000001B1"), "{text}");
}

/// The flag-off answer: the base schema has no provider `index`.
fn provider_refusal() -> ResponseTemplate {
    ResponseTemplate::new(422).set_body_json(json!({
        "success": false,
        "error": {
            "code": "INVALID_INPUT",
            "message": "Invalid input for tool search_patents",
            "issues": [{
                "path": "provider",
                "message": "Invalid option: expected one of \"epo_ops\"|\"uspto\""
            }]
        },
        "status": 422
    }))
}

#[tokio::test]
async fn a_server_without_the_index_gets_one_clear_line_and_no_dump() {
    let server = MockServer::start().await;
    mount_failure(&server, "search_patents", provider_refusal()).await;

    let output = run(
        &server,
        &[
            "--output",
            "table",
            "patent",
            "search",
            "--provider",
            "index",
            "--query",
            "battery",
        ],
    )
    .await;

    assert_ne!(output.status.code(), Some(0));
    let err = stderr(&output);
    assert!(
        err.contains("The publication index is not enabled on this server"),
        "{err}"
    );
    assert!(
        err.contains("flowleap patent search --provider epo --query"),
        "{err}"
    );
    // No raw envelope anywhere.
    assert_eq!(stdout(&output), "");
    assert!(!err.contains("INVALID_INPUT"), "{err}");
    assert!(!err.contains("Invalid option"), "{err}");
}

#[tokio::test]
async fn the_refusal_in_json_keeps_the_envelope_and_adds_the_hint() {
    let server = MockServer::start().await;
    mount_failure(&server, "search_patents", provider_refusal()).await;

    let output = run(
        &server,
        &[
            "--json",
            "patent",
            "search",
            "--provider",
            "index",
            "--query",
            "battery",
        ],
    )
    .await;

    assert_ne!(output.status.code(), Some(0));
    let envelope = stdout_json(&output);
    assert_eq!(envelope["status"], 422);
    assert_eq!(envelope["body"]["error"]["code"], "INVALID_INPUT");
    assert_eq!(
        envelope["indexUnavailableHint"]["code"],
        "publication_index_not_enabled"
    );
}

#[tokio::test]
async fn an_index_not_configured_answer_is_the_same_clear_line() {
    let server = MockServer::start().await;
    mount_failure(
        &server,
        "search_patents",
        ResponseTemplate::new(503).set_body_json(json!({
            "success": false,
            "error": { "code": "publication_index_not_configured", "message": "The publication index is not configured." },
            "status": 503
        })),
    )
    .await;

    let output = run(
        &server,
        &[
            "--output",
            "table",
            "patent",
            "search",
            "--provider",
            "index",
            "--query",
            "battery",
        ],
    )
    .await;

    assert_ne!(output.status.code(), Some(0));
    assert!(
        stderr(&output).contains("The publication index is not enabled on this server"),
        "{}",
        stderr(&output)
    );
}

// ----------------------------------------------------------------------------
// patent search without --provider: the index is the default
// ----------------------------------------------------------------------------

#[tokio::test]
async fn the_default_search_is_the_index_search() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "search_patents",
        json!({ "query": "\"solid state\" battery", "provider": "index", "range": "1-10" }),
        index_answer(),
    )
    .await;

    let output = run(
        &server,
        &[
            "--json",
            "patent",
            "search",
            "--query",
            "\"solid state\" battery",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout_json(&output), index_answer());
    assert_eq!(stderr(&output), "");
}

#[tokio::test]
async fn the_default_search_takes_the_index_filters_without_the_provider_flag() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "search_patents",
        json!({ "query": "*", "provider": "index", "range": "1-10", "cpc": "H01M", "kind": "B" }),
        index_answer(),
    )
    .await;

    let output = run(
        &server,
        &[
            "--output", "table", "patent", "search", "--query", "*", "--cpc", "H01M", "--kind", "B",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let out = stdout(&output);
    // The table says that the edition is dated and where newer publications are.
    assert!(out.contains("As of: 2026-09-23"), "{out}");
    assert!(out.contains("--provider epo"), "{out}");
}

/// The EPO answer of a fallback: `ta=` CQL made from the plain words.
async fn mount_epo_fallback(server: &MockServer, cql: &str, range: &str) {
    Mock::given(method("POST"))
        .and(path("/v1/tools/search_patents"))
        .and(wiremock::matchers::body_partial_json(json!({
            "query": cql, "provider": "epo_ops", "range": range
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(tool_ok(
            "search_patents",
            json!({ "total": 7, "docs": [{ "docId": "EP1000000A1", "title": "Battery" }] }),
        )))
        .expect(1)
        .mount(server)
        .await;
}

/// The index refusal, for a request that asks for provider `index` only.
async fn mount_index_refusal(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/v1/tools/search_patents"))
        .and(wiremock::matchers::body_partial_json(
            json!({ "provider": "index" }),
        ))
        .respond_with(response)
        // A 503 is sent again by the retry policy; a 422 is sent one time.
        .expect(1..)
        .mount(server)
        .await;
}

#[tokio::test]
async fn the_default_search_falls_back_to_epo_when_the_schema_refuses_the_index() {
    let server = MockServer::start().await;
    mount_index_refusal(&server, provider_refusal()).await;
    mount_epo_fallback(&server, "ta=\"solid state\" and ta=battery", "1-5").await;

    let output = run(
        &server,
        &[
            "--json",
            "patent",
            "search",
            "--query",
            "\"solid state\" battery",
            "--limit",
            "5",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    // stdout is the EPO payload; the refusal is not printed.
    assert_eq!(stdout_json(&output)["total"], 7);
    let err = stderr(&output);
    assert_eq!(err.lines().count(), 1, "one note: {err}");
    assert!(
        err.contains("the publication index is not enabled on this server"),
        "{err}"
    );
    assert!(
        err.contains("ta=\"solid state\" and ta=battery"),
        "the note names the CQL: {err}"
    );
}

#[tokio::test]
async fn the_default_search_falls_back_to_epo_when_the_index_is_not_configured() {
    let server = MockServer::start().await;
    mount_index_refusal(
        &server,
        ResponseTemplate::new(503).set_body_json(json!({
            "success": false,
            "error": { "code": "publication_index_not_configured", "message": "The publication index is not configured." },
            "status": 503
        })),
    )
    .await;
    mount_epo_fallback(&server, "ta=battery", "1-10").await;

    let output = run(
        &server,
        &[
            "--output", "table", "patent", "search", "--query", "battery",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("EP1000000A1"));
    assert!(
        stderr(&output).contains("ran on EPO OPS (--provider epo)"),
        "{}",
        stderr(&output)
    );
}

#[tokio::test]
async fn the_default_count_probe_falls_back_to_the_epo_count_probe() {
    let server = MockServer::start().await;
    mount_index_refusal(&server, provider_refusal()).await;
    mount_epo_fallback(&server, "ta=battery", "1-1").await;

    let output = run(
        &server,
        &[
            "--json",
            "patent",
            "search",
            "--query",
            "battery",
            "--count-only",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout_json(&output)["total"], 7);
}

#[tokio::test]
async fn a_default_search_with_an_index_filter_has_no_fallback() {
    let server = MockServer::start().await;
    mount_failure(&server, "search_patents", provider_refusal()).await;

    let output = run(
        &server,
        &[
            "--output", "table", "patent", "search", "--query", "battery", "--cpc", "H01M",
        ],
    )
    .await;

    assert_ne!(output.status.code(), Some(0));
    assert!(
        stderr(&output).contains("The publication index is not enabled on this server"),
        "{}",
        stderr(&output)
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn another_index_failure_is_not_a_reason_to_fall_back() {
    let server = MockServer::start().await;
    mount_failure(
        &server,
        "search_patents",
        ResponseTemplate::new(422).set_body_json(json!({
            "success": false,
            "error": { "code": "INVALID_INPUT", "message": "Invalid input",
                "issues": [{ "path": "query", "message": "too short" }] },
            "status": 422
        })),
    )
    .await;

    let output = run(&server, &["--json", "patent", "search", "--query", "b"]).await;

    assert_ne!(output.status.code(), Some(0));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_cql_query_with_an_index_filter_and_no_provider_is_refused_before_any_call() {
    let server = MockServer::start().await;

    let output = run(
        &server,
        &["patent", "search", "--query", "ti=battery", "--cpc", "H01M"],
    )
    .await;

    assert_ne!(output.status.code(), Some(0));
    let err = stderr(&output);
    assert!(err.contains("looks like CQL"), "{err}");
    assert!(err.contains("--provider index"), "{err}");
    assert!(err.contains("--provider epo"), "{err}");
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn an_index_filter_with_the_epo_provider_is_refused_before_any_call() {
    let server = MockServer::start().await;

    let output = run(
        &server,
        &[
            "patent",
            "search",
            "--provider",
            "epo",
            "--query",
            "ti=battery",
            "--cpc",
            "H01M",
        ],
    )
    .await;

    assert_ne!(output.status.code(), Some(0));
    assert!(
        stderr(&output).contains("--provider index only"),
        "{}",
        stderr(&output)
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_cql_query_without_the_provider_flag_runs_on_epo_with_one_note() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "search_patents",
        json!({ "query": "ti=battery", "provider": "epo_ops", "range": "1-10" }),
        json!({ "total": 1, "docs": [{ "docId": "EP1000000A1" }] }),
    )
    .await;

    let output = run(
        &server,
        &["--json", "patent", "search", "--query", "ti=battery"],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let err = stderr(&output);
    assert_eq!(err.lines().count(), 1, "{err}");
    assert!(err.contains("the query is CQL"), "{err}");
    assert!(err.contains("--provider epo"), "{err}");
}

#[tokio::test]
async fn the_countries_flag_without_the_provider_flag_runs_on_epo() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "search_patents",
        json!({ "query": "battery", "provider": "epo_ops", "range": "1-10", "countries": ["EP", "WO"] }),
        json!({ "total": 1, "docs": [{ "docId": "EP1000000A1" }] }),
    )
    .await;

    let output = run(
        &server,
        &[
            "--json",
            "patent",
            "search",
            "--query",
            "battery",
            "--countries",
            "EP,WO",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("--countries works with the EPO search only"),
        "{}",
        stderr(&output)
    );
}

#[tokio::test]
async fn an_explicit_epo_search_prints_no_note() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "search_patents",
        json!({ "query": "ti=battery", "provider": "epo_ops", "range": "1-10" }),
        json!({ "total": 1, "docs": [{ "docId": "EP1000000A1" }] }),
    )
    .await;

    let output = run(
        &server,
        &[
            "--json",
            "patent",
            "search",
            "--provider",
            "epo",
            "--query",
            "ti=battery",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
}

// ----------------------------------------------------------------------------
// Dated answers
// ----------------------------------------------------------------------------

/// A dated legal status: the PATSTAT copy, served when the live call cannot run.
fn dated_legal_status() -> Value {
    json!({
        "docId": "EP1000000",
        "events": [{ "code": "PGFP", "eventDate": "2024-05-01", "date": "2024-05-10" }],
        "eventsTotal": 1,
        "eventsReturned": 1,
        "offset": 0,
        "live": false,
        "source": "patstat",
        "asOf": "2026-10-01",
        "edition": "2026 Autumn",
        "liveUnavailable": { "reason": "live_budget_spent", "retryAfterSeconds": 18720 }
    })
}

#[tokio::test]
async fn a_dated_legal_status_says_its_source_its_date_and_the_wait() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "get_legal_status",
        json!({ "patent_number": "EP1000000" }),
        dated_legal_status(),
    )
    .await;

    let output = run(&server, &["--output", "table", "ops", "legal", "EP1000000"]).await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let err = stderr(&output);
    assert!(err.contains("DATED answer, not live"), "{err}");
    assert!(err.contains("PATSTAT"), "{err}");
    assert!(err.contains("As of: 2026-10-01"), "{err}");
    assert!(err.contains("today's live data budget is spent"), "{err}");
    assert!(err.contains("Retry in 5 h 12 min"), "{err}");
    assert!(err.contains("not proof that a right is in force"), "{err}");
}

#[tokio::test]
async fn a_dated_answer_in_json_is_the_backend_payload_and_no_note() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "get_legal_status",
        json!({ "patent_number": "EP1000000" }),
        dated_legal_status(),
    )
    .await;

    let output = run(&server, &["--json", "ops", "legal", "EP1000000"]).await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout_json(&output), dated_legal_status());
    assert!(!stderr(&output).contains("DATED"), "{}", stderr(&output));
}

#[tokio::test]
async fn a_dated_bibliography_from_the_index_carries_the_credit_and_the_quota_wait() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "get_bibliography",
        json!({ "patent_number": "EP1000000" }),
        json!({
            "docId": "EP1000000A1",
            "title": "Apparatus",
            "live": false,
            "source": "publication_index",
            "asOf": "2026-09-23",
            "attribution": { "text": ATTRIBUTION },
            "liveUnavailable": { "reason": "shared_quota_busy", "retryAfterSeconds": 60 }
        }),
    )
    .await;

    let output = run(
        &server,
        &["--output", "table", "ops", "biblio", "EP1000000"],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let err = stderr(&output);
    assert!(err.contains("the publication index"), "{err}");
    assert!(err.contains("As of: 2026-09-23"), "{err}");
    assert!(err.contains("shared office quota is busy"), "{err}");
    assert!(err.contains("Retry in 60 seconds"), "{err}");
    assert!(err.contains(ATTRIBUTION), "{err}");
    // Only a dated legal status carries the in-force warning.
    assert!(!err.contains("in force"), "{err}");
}

#[tokio::test]
async fn a_live_answer_prints_no_dated_note() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "get_legal_status",
        json!({ "patent_number": "EP1000000" }),
        json!({ "docId": "EP1000000", "events": [], "eventsTotal": 0, "eventsReturned": 0,
                "live": true, "source": "office" }),
    )
    .await;

    let output = run(&server, &["--output", "table", "ops", "legal", "EP1000000"]).await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(!stderr(&output).contains("DATED"), "{}", stderr(&output));
}

// ----------------------------------------------------------------------------
// "Not now": no hosted copy, the refusal carries liveUnavailable
// ----------------------------------------------------------------------------

/// A License holder's spent live data budget on a tool without a hosted copy.
fn live_budget_refusal() -> ResponseTemplate {
    ResponseTemplate::new(429)
        .insert_header("Retry-After", "18720")
        .set_body_json(json!({
            "success": false,
            "error": {
                "message": "Today's shared patent-data budget on your plan is used up.",
                "type": "rate_limit_error",
                "code": "live_data_budget_exhausted",
                "provider": "epo",
                "remaining": 0,
                "resets_at": "2026-11-23T00:00:00.000Z",
                "retry_after": 18720,
                "allowance": "calls",
                "keysPageUrl": "https://www.flowleap.co/en/dashboard/keys",
                "liveUnavailable": { "reason": "live_budget_spent", "retryAfterSeconds": 18720 }
            },
            "status": 429
        }))
}

#[tokio::test]
async fn a_spent_live_budget_is_a_wait_with_the_time_and_not_a_key_gate() {
    let server = MockServer::start().await;
    mount_failure(&server, "get_claims", live_budget_refusal()).await;

    let output = run(
        &server,
        &["--output", "table", "ops", "claims", "EP1000000"],
    )
    .await;

    // 429: back off, then retry. Not exit 9 (a human stop).
    assert_eq!(output.status.code(), Some(6), "{}", stderr(&output));
    let err = stderr(&output);
    assert!(err.contains("Live data: not now"), "{err}");
    assert!(err.contains("live data budget for EPO is spent"), "{err}");
    assert!(err.contains("5 h 12 min"), "{err}");
    assert!(err.contains("2026-11-23T00:00:00.000Z"), "{err}");
    assert!(err.contains("optional"), "{err}");
    assert!(
        err.contains("https://www.flowleap.co/en/dashboard/keys"),
        "{err}"
    );
    // Not the per-user request limit, and not a keys-required box.
    assert!(!err.contains("Rate limited"), "{err}");
    assert!(!err.contains("keys required"), "{err}");
}

#[tokio::test]
async fn the_not_now_hint_is_on_the_json_envelope() {
    let server = MockServer::start().await;
    mount_failure(&server, "get_claims", live_budget_refusal()).await;

    let output = run(&server, &["--json", "ops", "claims", "EP1000000"]).await;

    assert_eq!(output.status.code(), Some(6));
    let envelope = stdout_json(&output);
    let hint = &envelope["liveUnavailableHint"];
    assert_eq!(hint["reason"], "live_budget_spent");
    assert_eq!(hint["retryAfterSeconds"], 18720);
    assert_eq!(hint["provider"], "epo");
    assert_eq!(hint["resetsAt"], "2026-11-23T00:00:00.000Z");
    assert_eq!(hint["requiresHumanIntervention"], false);
    assert_eq!(envelope["retryAfterSeconds"], 18720);
    assert!(envelope.get("providerKeysHint").is_none());
    assert!(envelope.get("rateLimitHint").is_none());
    // The backend body is unchanged.
    assert_eq!(
        envelope["body"]["error"]["code"],
        "live_data_budget_exhausted"
    );
}

#[tokio::test]
async fn a_busy_shared_quota_with_live_unavailable_shows_the_wait_once() {
    let server = MockServer::start().await;
    mount_failure(
        &server,
        "get_claims",
        ResponseTemplate::new(503)
            .insert_header("Retry-After", "45")
            .set_body_json(json!({
                "success": false,
                "error": {
                    "code": "upstream_unavailable",
                    "message": "The shared EPO quota is busy.",
                    "details": {
                        "service": "epo_ops",
                        "retry_after": 45,
                        "shared_quota": { "bucket": "retrieval", "per_minute": 5 }
                    },
                    "liveUnavailable": { "reason": "shared_quota_busy", "retryAfterSeconds": 45 }
                },
                "status": 503
            })),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[
            ("FLOWLEAP_API_KEY", "fl_pat_test"),
            ("FLOWLEAP_MAX_RETRIES", "0"),
        ],
        &["--output", "table", "ops", "claims", "EP1000000"],
    )
    .await;

    assert_ne!(output.status.code(), Some(0));
    let err = stderr(&output);
    assert!(err.contains("Shared EPO quota busy"), "{err}");
    assert!(err.contains("45 seconds"), "{err}");
    // One box names the wait, not two.
    assert!(!err.contains("Live data: not now"), "{err}");
}

#[tokio::test]
async fn a_data_keys_required_answer_keeps_the_key_gate_of_today() {
    // An older server, or keyless access off: a License user without keys.
    let server = MockServer::start().await;
    mount_failure(
        &server,
        "get_claims",
        ResponseTemplate::new(400).set_body_json(json!({
            "success": false,
            "error": {
                "code": "data_keys_required",
                "message": "EPO keys are required.",
                "provider": "epo",
                "keysPageUrl": "https://www.flowleap.co/en/dashboard/keys"
            },
            "status": 400
        })),
    )
    .await;

    let output = run(&server, &["--json", "ops", "claims", "EP1000000"]).await;

    assert_eq!(output.status.code(), Some(9));
    let envelope = stdout_json(&output);
    assert_eq!(
        envelope["providerKeysHint"]["code"],
        "provider_keys_required"
    );
    assert!(envelope.get("liveUnavailableHint").is_none());
}
