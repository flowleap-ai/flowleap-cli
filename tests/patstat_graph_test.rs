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

/*
 * ── graph patent (issue #56) ─────────────────────────────────────────────
 * Shaped against flowleap-backend src/lib/patstat-graph/types.ts PatentView:
 * meta (caps/truncation/data_quality), anchor (AnchorView), header
 * (applicants/inventors/cpc), citations (backward_patent/backward_npl/
 * backward_unresolved/forward), family, priorities. Fixture IDs deliberately
 * avoid the substring "9999" (the unknown-year sentinel) so the
 * `!stdout.contains("9999")` assertions cannot false-positive on a fixture id.
 */

fn patent_view_body() -> serde_json::Value {
    json!({
        "success": true,
        "meta": {
            "composite": "patent_view",
            "data_edition": "PATSTAT 2026 Spring",
            "attribution": "This product contains data sourced from EPO databases, © European Patent Organisation",
            "caps": {
                "forward": 200, "backward_patent": 200, "backward_npl": 200,
                "backward_unresolved": 200, "family": 200, "persons": 200,
                "cpc": 200, "priorities": 200,
            },
            "truncation": {
                "forward": { "truncated": false, "total": 1, "shown": 1 },
                "backward_patent": { "truncated": false, "total": 1, "shown": 1 },
                "backward_npl": { "truncated": false, "total": 1, "shown": 1 },
                "backward_unresolved": { "truncated": false, "total": 1, "shown": 1 },
                "family": { "truncated": false, "total": 2, "shown": 2 },
                "persons": { "truncated": false, "total": 2, "shown": 2 },
                "cpc": { "truncated": false, "total": 1, "shown": 1 },
                "priorities": { "truncated": false, "total": 1, "shown": 1 },
            },
            "data_quality": [
                { "node": "pat:70112233", "issue": "unknown_filing_date",
                  "detail": "Family member 70112233 has a sentinel filing date; excluded from filing_year, flagged here instead." },
            ],
        },
        "anchor": {
            "node": "pat:56123456",
            "appln_id": 56123456,
            "application": "US08655468 (A)",
            "title": "Method and system for placing a purchase order",
            "title_lang": "en",
            "filing_date": "1996-06-06",
            "filing_year": 1996,
            "granted": true,
            "docdb_family_id": 65432100,
            "earliest_publn_date": "1999-09-28",
            "publications": [
                { "publn": "US5960411A", "kind": "A", "date": "1999-09-28",
                  "first_grant": true, "at": "tls211:1" },
            ],
            "at": "tls201:56123456",
        },
        "header": {
            "applicants": [
                { "node": "person:84210", "name": "AMAZON.COM INC", "country": "US",
                  "edge": "has_applicant", "confidence": { "tag": "EXTRACTED", "score": 1.0 },
                  "name_grouping": { "confidence": { "tag": "INFERRED", "score": 0.85 }, "note": "PSN harmonized" },
                  "at": "tls207:1/1" },
            ],
            "inventors": [
                { "node": "person:84211", "name": "JEFFREY P BEZOS", "country": "US",
                  "edge": "has_inventor", "confidence": { "tag": "EXTRACTED", "score": 1.0 },
                  "at": "tls207:2/1" },
            ],
            "cpc": [
                { "node": "cpc:G06Q30/06", "symbol": "G06Q30/06", "edge": "classified_as",
                  "confidence": { "tag": "EXTRACTED", "score": 1.0 }, "at": "tls224:56123456" },
            ],
        },
        "citations": {
            "backward_patent": [
                { "node": "pat:41112233", "cited": "US04949256A", "title": "Order entry system",
                  "date": "1988-01-01", "origin": "APP", "edge": "cites",
                  "confidence": { "tag": "EXTRACTED", "score": 1.0 }, "at": "tls212:1/1" },
            ],
            "backward_npl": [
                { "node": "doc:npl:88001", "biblio": "Smith, J. (1994) E-commerce systems.",
                  "origin": "SEA", "edge": "cites",
                  "confidence": { "tag": "EXTRACTED", "score": 1.0 }, "at": "tls212:1/2" },
            ],
            "backward_unresolved": [
                { "node": "doc:unresolved:1/3", "origin": "APP", "edge": "cites",
                  "confidence": { "tag": "AMBIGUOUS", "score": 0.2 },
                  "note": "Citation row has no resolvable cited document.", "at": "tls212:1/3" },
            ],
            "forward": [
                { "node": "pat:70112233", "citing": "US7013292B1", "title": "One-click checkout method",
                  "applicant": "EBAY INC", "date": "2006-03-14", "origin": "EXA",
                  "examiner_cited": true, "citing_family_size": 4, "edge": "cites",
                  "direction": "incoming", "confidence": { "tag": "EXTRACTED", "score": 1.0 },
                  "at": "tls212:2/1" },
            ],
        },
        "family": [
            { "node": "pat:56123456", "appln_id": 56123456, "application": "US08655468 (A)",
              "office": "US", "filing_date": "1996-06-06", "filing_year": 1996,
              "earliest_publn_date": "1999-09-28", "first_grant_date": "1999-09-28",
              "granted": true, "is_anchor": true, "edge": "in_family",
              "confidence": { "tag": "EXTRACTED", "score": 1.0 }, "at": "tls201:56123456" },
            { "node": "pat:70112233", "appln_id": 70112233, "application": "EP0807891 (A)",
              "office": "EP", "filing_date": null, "filing_year": null,
              "earliest_publn_date": "1997-11-19", "first_grant_date": null,
              "granted": false, "is_anchor": false, "edge": "in_family",
              "confidence": { "tag": "EXTRACTED", "score": 1.0 }, "at": "tls201:70112233" },
        ],
        "priorities": [
            { "node": "pat:56123456", "prior_application": "US08655468 (A)",
              "prior_filing_date": "1996-06-06", "edge": "claims_priority",
              "confidence": { "tag": "EXTRACTED", "score": 1.0 }, "at": "tls204:56123456/1" },
        ],
    })
}

/// A hub patent whose sections all hit their 200-cap — every truncated
/// section states the TRUE database total, never the shown count.
fn patent_view_truncated_hub_body() -> serde_json::Value {
    let mut body = patent_view_body();
    body["meta"]["truncation"]["forward"] = json!({ "truncated": true, "total": 8421, "shown": 2 });
    body["meta"]["truncation"]["backward_patent"] =
        json!({ "truncated": true, "total": 54321, "shown": 1 });
    body["meta"]["truncation"]["persons"] = json!({ "truncated": true, "total": 602, "shown": 2 });
    body["meta"]["truncation"]["cpc"] = json!({ "truncated": true, "total": 847, "shown": 1 });
    body["meta"]["truncation"]["family"] = json!({ "truncated": true, "total": 1204, "shown": 2 });
    body["citations"]["forward"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "node": "pat:70112234", "citing": "US7222333B2", "title": "Checkout confirmation flow",
            "applicant": "WALMART INC", "date": "2007-05-01", "origin": "APP",
            "examiner_cited": false, "citing_family_size": 2, "edge": "cites",
            "direction": "incoming", "confidence": { "tag": "EXTRACTED", "score": 1.0 },
            "at": "tls212:2/2",
        }));
    body
}

fn patent_error_body(code: &str, message: &str, status: u16) -> serde_json::Value {
    json!({
        "success": false,
        "error": { "code": code, "message": message },
        "status": status,
    })
}

async fn mount_patent_view(server: &MockServer, number: &str, template: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/patstat/graph/patent/{number}")))
        .respond_with(template)
        .mount(server)
        .await;
}

#[tokio::test]
async fn patent_view_renders_all_sections_in_human_mode() {
    let server = MockServer::start().await;
    mount_patent_view(
        &server,
        "US5960411",
        ResponseTemplate::new(200).set_body_json(patent_view_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "patent", "US5960411"],
    )
    .await;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");

    // Anchor.
    assert!(stdout.contains("Anchor: pat:56123456"));
    assert!(stdout.contains("Filed: 1996"));
    assert!(stdout.contains("Granted: yes"));
    assert!(stdout.contains("DOCDB family: 65432100"));
    // Header sections.
    assert!(stdout.contains("Applicants"));
    assert!(stdout.contains("AMAZON.COM INC"));
    assert!(stdout.contains("Inventors"));
    assert!(stdout.contains("JEFFREY P BEZOS"));
    assert!(stdout.contains("CPC Classifications"));
    assert!(stdout.contains("G06Q30/06"));
    // Citations, distinguished by section — examiner vs applicant origin.
    assert!(stdout.contains("Backward Citations — Patents"));
    assert!(stdout.contains("US04949256A"));
    assert!(stdout.contains("APP"));
    assert!(stdout.contains("Backward Citations — Non-Patent Literature"));
    assert!(stdout.contains("Smith, J. (1994) E-commerce systems."));
    assert!(stdout.contains("Backward Citations — Unresolved"));
    assert!(stdout.contains("AMBIGUOUS"));
    assert!(stdout.contains("Forward Citations"));
    assert!(stdout.contains("US7013292B1"));
    assert!(stdout.contains("EBAY INC"));
    assert!(stdout.contains("EXA"));
    // Family: filing→grant range, and the sentinel-date row renders "unknown"
    // rather than the 9999 sentinel.
    assert!(stdout.contains("DOCDB Family"));
    assert!(stdout.contains("1996 → 1999-09-28"));
    assert!(stdout.contains("unknown → not yet granted"));
    // Priorities.
    assert!(stdout.contains("Priority Claims"));
    // Data-quality flags surfaced, not dropped.
    assert!(stdout.contains("Data quality flags:"));
    assert!(stdout.contains("unknown_filing_date"));
    // Provenance footer: edition, attribution, snapshot caveat.
    assert!(stdout.contains("Source: PATSTAT data edition PATSTAT 2026 Spring."));
    assert!(stdout.contains("European Patent Organisation"));
    assert!(stdout.contains("current legal status"));
    assert!(stdout.contains("flowleap ops legal"));
    // No section hit its cap — no truncation notice should appear.
    assert!(!stdout.contains("Showing"));
    // The 9999 sentinel is never quoted anywhere in human output.
    assert!(!stdout.contains("9999"));
}

#[tokio::test]
async fn patent_view_json_mode_emits_the_backend_body_untouched() {
    let server = MockServer::start().await;
    mount_patent_view(
        &server,
        "US5960411",
        ResponseTemplate::new(200).set_body_json(patent_view_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "graph", "patent", "US5960411"],
    )
    .await;

    assert!(output.status.success());
    assert_eq!(stdout_json(&output), patent_view_body());
}

/// A hub patent past its 200-cap on several sections at once: every notice
/// states the TRUE database total, never the shown count read as a total.
#[tokio::test]
async fn patent_view_truncated_hub_patent_shows_true_totals() {
    let server = MockServer::start().await;
    mount_patent_view(
        &server,
        "US5960411",
        ResponseTemplate::new(200).set_body_json(patent_view_truncated_hub_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "patent", "US5960411"],
    )
    .await;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");

    assert!(stdout.contains("Showing 2 of 8421 forward citations."));
    assert!(stdout.contains("Showing 1 of 54321 backward patent citations."));
    assert!(stdout.contains("Showing 2 of 602 person rows (applicants + inventors combined)."));
    assert!(stdout.contains("Showing 1 of 847 CPC classifications."));
    assert!(stdout.contains("Showing 2 of 1204 family members."));
    // Sections that did NOT hit their cap get no notice.
    assert!(!stdout.contains("backward NPL citations."));
    assert!(!stdout.contains("priority claims."));
}

/// A parseable number with no row in the loaded edition — same typed 404
/// `render_graph_error` already renders for `resolve`, reached here through
/// the `graph patent` route.
#[tokio::test]
async fn patent_view_404_relays_the_not_found_message() {
    let server = MockServer::start().await;
    let body = patent_error_body(
        "patstat_patent_not_found",
        "No publication in the loaded PATSTAT edition matches \"EP9000001\". Check the number, \
         or include the country prefix.",
        404,
    );
    mount_patent_view(
        &server,
        "EP9000001",
        ResponseTemplate::new(404).set_body_json(body),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "patent", "EP9000001"],
    )
    .await;

    assert_eq!(output.status.code(), Some(5));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("No such publication in the loaded PATSTAT edition."));
}

/// `patstat_patent_ambiguous` is a real HTTP 422 on `graph patent` (unlike
/// `resolve`, which reaches the same interaction step on a 200): the
/// candidates are printed and nothing is auto-picked, via the shared
/// `render_graph_error` path `resolve`'s tests already lock.
#[tokio::test]
async fn patent_view_422_ambiguous_prints_candidates_and_exits_with_the_mapped_code() {
    let server = MockServer::start().await;
    let mut body = patent_error_body(
        "patstat_patent_ambiguous",
        "\"US5960411\" matches 2 distinct applications: US08655468 (A), US09123456 (A). Add the \
         kind code (e.g. A1 vs B1) or use a fuller number form to disambiguate.",
        422,
    );
    body["error"]["candidates"] = ambiguous_body()["candidates"].clone();
    mount_patent_view(
        &server,
        "US5960411",
        ResponseTemplate::new(422).set_body_json(body),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "patent", "US5960411"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("Ambiguous publication number"));
    assert!(stdout.contains("US08655468 (A)"));
    assert!(stdout.contains("US09123456 (A)"));
    assert!(stdout.contains("None is picked automatically."));
}

/// A publication number that needs percent-encoding to survive as one URL
/// path segment — asserted through `--dry-run` so the encoding is checked
/// without depending on a server's own path parsing.
#[tokio::test]
async fn patent_view_url_encodes_the_publication_path_segment() {
    let output = run_cli(
        "http://127.0.0.1:9",
        &[API_KEY_ENV],
        &[
            "--json",
            "--dry-run",
            "patstat",
            "graph",
            "patent",
            "EP 3477840/A1",
        ],
    )
    .await;

    assert!(output.status.success());
    let value = stdout_json(&output);
    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "GET");
    assert_eq!(
        value["url"],
        "http://127.0.0.1:9/v1/patstat/graph/patent/EP%203477840%2FA1"
    );
}

/*
 * ── graph applicant (issue #56) ──────────────────────────────────────────
 * Shaped against ApplicantView: meta (caps/truncation/sources/data_quality),
 * entity (ApplicantEntityCard), filings_by_year (no cap — full year
 * distribution), top_cpc, jurisdictions, co_applicants.
 */

fn applicant_view_body() -> serde_json::Value {
    json!({
        "success": true,
        "meta": {
            "composite": "applicant_view",
            "data_edition": "PATSTAT 2026 Spring",
            "attribution": "This product contains data sourced from EPO databases, © European Patent Organisation",
            "caps": { "top_cpc": 20, "jurisdictions": 50, "co_applicants": 20 },
            "truncation": {
                "top_cpc": { "truncated": false, "total": 2, "shown": 2 },
                "jurisdictions": { "truncated": false, "total": 2, "shown": 2 },
                "co_applicants": { "truncated": false, "total": 1, "shown": 1 },
            },
            "sources": {
                "filings_by_year": "tls201/tls207", "top_cpc": "tls224/tls207",
                "jurisdictions": "tls201/tls207", "co_applicants": "tls207",
            },
            "data_quality": [
                { "node": "person:84210", "issue": "unknown_filing_date",
                  "detail": "3 filings by this entity have a sentinel filing date; excluded from filings_by_year, flagged here instead." },
            ],
        },
        "entity": {
            "node": "person:84210",
            "psn_id": 84210,
            "name": "AMAZON.COM INC",
            "applications": 18234,
            "person_variants": 312,
            "confidence": { "tag": "INFERRED", "score": 0.85 },
            "name_grouping": { "confidence": { "tag": "INFERRED", "score": 0.85 },
                                "note": "Grouped under PSN harmonization; subsidiaries may be tracked separately." },
            "at": "tls206:psn/84210",
        },
        "filings_by_year": [
            { "year": 2018, "applications": 812 },
            { "year": 2019, "applications": 940 },
        ],
        "top_cpc": [
            { "node": "cpc:G06Q30/06", "symbol": "G06Q30/06", "applications": 4021 },
            { "node": "cpc:H04L67/00", "symbol": "H04L67/00", "applications": 2210 },
        ],
        "jurisdictions": [
            { "office": "US", "applications": 9812 },
            { "office": "EP", "applications": 3120 },
        ],
        "co_applicants": [
            { "node": "person:84299", "psn_id": 84299, "name": "WHOLE FOODS MARKET INC",
              "shared_applications": 42, "edge": "has_applicant",
              "confidence": { "tag": "INFERRED", "score": 0.85 }, "at": "tls206:psn/84299" },
        ],
    })
}

fn applicant_view_truncated_body() -> serde_json::Value {
    let mut body = applicant_view_body();
    body["meta"]["truncation"]["top_cpc"] = json!({ "truncated": true, "total": 134, "shown": 2 });
    body["meta"]["truncation"]["jurisdictions"] =
        json!({ "truncated": true, "total": 57, "shown": 2 });
    body["meta"]["truncation"]["co_applicants"] =
        json!({ "truncated": true, "total": 812, "shown": 1 });
    body
}

async fn mount_applicant_view(server: &MockServer, psn_id: u64, template: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/patstat/graph/applicant/{psn_id}")))
        .respond_with(template)
        .mount(server)
        .await;
}

#[tokio::test]
async fn applicant_view_renders_all_sections_in_human_mode() {
    let server = MockServer::start().await;
    mount_applicant_view(
        &server,
        84210,
        ResponseTemplate::new(200).set_body_json(applicant_view_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "applicant", "84210"],
    )
    .await;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");

    assert!(stdout.contains("Entity: AMAZON.COM INC (person:84210)"));
    assert!(stdout.contains("PSN ID: 84210"));
    assert!(stdout.contains("Applications (as applicant): 18234"));
    assert!(stdout.contains("Filings by Year"));
    assert!(stdout.contains("2018"));
    assert!(stdout.contains("812"));
    assert!(stdout.contains("Top CPC"));
    assert!(stdout.contains("G06Q30/06"));
    assert!(stdout.contains("Jurisdictions"));
    assert!(stdout.contains("9812"));
    assert!(stdout.contains("Co-Applicants"));
    assert!(stdout.contains("WHOLE FOODS MARKET INC"));
    assert!(stdout.contains("Data quality flags:"));
    assert!(stdout.contains("Source: PATSTAT data edition PATSTAT 2026 Spring."));
    assert!(stdout.contains("current legal status"));
    assert!(!stdout.contains("Showing"));
}

#[tokio::test]
async fn applicant_view_json_mode_emits_the_backend_body_untouched() {
    let server = MockServer::start().await;
    mount_applicant_view(
        &server,
        84210,
        ResponseTemplate::new(200).set_body_json(applicant_view_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "graph", "applicant", "84210"],
    )
    .await;

    assert!(output.status.success());
    assert_eq!(stdout_json(&output), applicant_view_body());
}

#[tokio::test]
async fn applicant_view_truncated_sections_show_true_totals() {
    let server = MockServer::start().await;
    mount_applicant_view(
        &server,
        84210,
        ResponseTemplate::new(200).set_body_json(applicant_view_truncated_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "applicant", "84210"],
    )
    .await;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");

    assert!(stdout.contains("Showing 2 of 134 top CPC classes."));
    assert!(stdout.contains("Showing 2 of 57 jurisdictions."));
    assert!(stdout.contains("Showing 1 of 812 co-applicants."));
}

/// No harmonized entity behind the given `psn_id` in the loaded edition —
/// same typed 404 family as `graph patent`, reached through the applicant
/// route.
#[tokio::test]
async fn applicant_view_404_relays_the_entity_not_found_message() {
    let server = MockServer::start().await;
    let body = patent_error_body(
        "patstat_entity_not_found",
        "No harmonized (PSN) applicant entity matches psn_id 1. Resolve a name first with \
         `graph resolve`.",
        404,
    );
    mount_applicant_view(&server, 1, ResponseTemplate::new(404).set_body_json(body)).await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "applicant", "1"],
    )
    .await;

    assert_eq!(output.status.code(), Some(5));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("No such applicant entity in the loaded PATSTAT edition."));
}

/// A non-integer `psn_id` is rejected by clap before any request is made —
/// the CLI-level type IS the validation, one step earlier than the backend's
/// own `patstat_invalid_request` guard on a malformed value that did parse.
#[tokio::test]
async fn applicant_rejects_a_non_numeric_psn_id_locally_without_a_network_call() {
    let output = run_cli(
        "http://127.0.0.1:9",
        &[API_KEY_ENV],
        &["patstat", "graph", "applicant", "not-a-number"],
    )
    .await;

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("psn_id") || stderr.contains("invalid"),
        "stderr: {stderr}"
    );
}
