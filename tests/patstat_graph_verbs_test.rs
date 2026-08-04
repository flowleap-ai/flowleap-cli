//! `flowleap patstat graph neighborhood|path|explain` (issue #55, PRD 0002):
//! the three LLM-tier agent verbs — parameter passthrough, the verbatim
//! `text` relay, `path`'s not-found result, and the typed error family.
//!
//! The invariant this file guards is the relay discipline: human mode is the
//! backend `text` byte-for-byte (it already carries the confidence tags,
//! `at=` provenance refs, Data Edition, and truncation notices), and `--json`
//! is the whole body including the typed `data` twin.

mod support;

use serde_json::json;
use support::{run_cli, stdout_json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const API_KEY_ENV: (&str, &str) = ("FLOWLEAP_API_KEY", "fl_pat_test_key");

/// A verb response, shaped like the real backend
/// (flowleap-backend src/routes/patstat.ts → `{ success, text, data }`).
/// The `text` here mirrors the real serialization closely enough to prove it
/// survives the relay unaltered: header with Data Edition, fact lines with
/// confidence tags and provenance, then a truncation notice.
fn neighborhood_body() -> serde_json::Value {
    json!({
        "success": true,
        "text": "# neighborhood pat:56123456 (EP3477840B1) depth=1 — 2024 Autumn\n\
                 # \"Method for operating a wind turbine\"\n\
                 EP3477840 --cites [EXTRACTED 1.0]--> DE4302443 at=tls212:530028653\n\
                 EP3477840 --cited_by [EXTRACTED 1.0]--> US2021123456 at=tls212:530028999\n\
                 TRUNCATED: showing 200 of 2244 cited_by edges. Narrow with edge_types=['cites'].",
        "data": {
            "anchor": { "node": "pat:56123456", "label": "EP3477840B1" },
            "depth": 1,
            "edges": [
                { "from": "pat:56123456", "edge": "cites", "to": "doc:DE4302443",
                  "confidence": "EXTRACTED", "score": 1.0, "at": "tls212:530028653" },
            ],
            "notices": ["TRUNCATED: showing 200 of 2244 cited_by edges."],
            "data_quality": [],
            "data_edition": "2024 Autumn",
        },
    })
}

fn path_found_body() -> serde_json::Value {
    json!({
        "success": true,
        "text": "# path EP3477840B1 → US5960411A (max_hops=4) — 2024 Autumn\n\
                 FOUND: 2 hops: EP3477840B1 → DE4302443 → US5960411A\n\
                 EP3477840 --cites [EXTRACTED 1.0]--> DE4302443 at=tls212:1\n\
                 DE4302443 --cites [EXTRACTED 1.0]--> US5960411 at=tls212:2",
        "data": {
            "from": { "node": "pat:56123456", "label": "EP3477840B1" },
            "to": { "node": "pat:11111111", "label": "US5960411A" },
            "found": true,
            "hops": 2,
            "max_hops": 4,
            "nodes": [], "edges": [], "notices": [],
            "data_edition": "2024 Autumn",
        },
    })
}

/// `found: false` arrives on a 200 — the search ran and the answer is "no
/// path within the limit". A successful answer, not a failure.
fn path_not_found_body() -> serde_json::Value {
    json!({
        "success": true,
        "text": "# path EP3477840B1 → US5960411A (max_hops=4) — 2024 Autumn\n\
                 NOT FOUND within the hop limit.\n\
                 TRUNCATED: frontier capped at 500 nodes — a found path is still valid, \
                 but absence is not proof. Prefer closer endpoints.",
        "data": {
            "from": { "node": "pat:56123456", "label": "EP3477840B1" },
            "to": { "node": "pat:11111111", "label": "US5960411A" },
            "found": false,
            "max_hops": 4,
            "nodes": [], "edges": [],
            "notices": ["TRUNCATED: frontier capped at 500 nodes"],
            "data_edition": "2024 Autumn",
        },
    })
}

fn explain_body() -> serde_json::Value {
    json!({
        "success": true,
        "text": "# explain pat:56123456 — 2024 Autumn\n\
                 EP18000829 (A) \"Method for operating a wind turbine\"\n\
                 filed 2018-01-15 granted=true family:65432100 publications: EP3477840A1, \
                 EP3477840B1 at=tls201:56123456\n\
                 EP3477840 --cited_by [EXTRACTED 1.0]--> US2021123456 at=tls212:1\n\
                 … 2044 more cited_by connections (grouped — not shown)",
        "data": {
            "card": { "node": "pat:56123456", "application": "EP18000829 (A)" },
            "top_connections": [],
            "remainder": [{ "edge": "cited_by", "count": 2044 }],
            "notices": [], "data_quality": [],
            "data_edition": "2024 Autumn",
        },
    })
}

fn error_body(code: &str, message: &str, status: u16) -> serde_json::Value {
    json!({
        "success": false,
        "error": { "code": code, "message": message },
        "status": status,
    })
}

async fn mount(server: &MockServer, verb: &str, template: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/patstat/graph/{verb}")))
        .respond_with(template)
        .mount(server)
        .await;
}

/*
 * ── parameter passthrough ───────────────────────────────────────────────────
 */

#[tokio::test]
async fn neighborhood_sends_every_parameter_it_was_given() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/patstat/graph/neighborhood"))
        .and(query_param("node", "pat:56123456"))
        .and(query_param("depth", "2"))
        .and(query_param("edge_types", "cites,cited_by"))
        .and(query_param("token_budget", "4000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(neighborhood_body()))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "--json",
            "patstat",
            "graph",
            "neighborhood",
            "pat:56123456",
            "--depth",
            "2",
            "--edge-types",
            "cites,cited_by",
            "--token-budget",
            "4000",
        ],
    )
    .await;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Absent flags are omitted from the query entirely, so the backend applies
/// its own documented defaults rather than the CLI pinning them.
#[tokio::test]
async fn absent_flags_are_omitted_from_the_query_string() {
    let output = run_cli(
        "http://127.0.0.1:9",
        &[API_KEY_ENV],
        &[
            "--json",
            "--dry-run",
            "patstat",
            "graph",
            "neighborhood",
            "EP3477840",
        ],
    )
    .await;

    assert!(output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        value["url"],
        "http://127.0.0.1:9/v1/patstat/graph/neighborhood?node=EP3477840"
    );
}

#[tokio::test]
async fn path_sends_both_endpoints_and_max_hops() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/patstat/graph/path"))
        .and(query_param("a", "EP3477840"))
        .and(query_param("b", "US5960411"))
        .and(query_param("max_hops", "3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(path_found_body()))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "--json",
            "patstat",
            "graph",
            "path",
            "EP3477840",
            "US5960411",
            "--max-hops",
            "3",
        ],
    )
    .await;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn explain_sends_node_and_token_budget() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/patstat/graph/explain"))
        .and(query_param("node", "pat:56123456"))
        .and(query_param("token_budget", "8000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(explain_body()))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "--json",
            "patstat",
            "graph",
            "explain",
            "pat:56123456",
            "--token-budget",
            "8000",
        ],
    )
    .await;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/*
 * ── relay discipline ────────────────────────────────────────────────────────
 */

/// Human mode is the `text` field byte-for-byte: fact lines keep their
/// confidence tags and `at=` refs, the Data Edition header survives, and the
/// truncation notice is not swallowed.
#[tokio::test]
async fn human_mode_prints_the_backend_text_verbatim() {
    let server = MockServer::start().await;
    mount(
        &server,
        "neighborhood",
        ResponseTemplate::new(200).set_body_json(neighborhood_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "neighborhood", "EP3477840"],
    )
    .await;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let text = neighborhood_body()["text"].as_str().unwrap().to_string();

    // Byte-for-byte, modulo the trailing newline `println!` adds.
    assert_eq!(stdout, format!("{text}\n"));
    // Nothing of the typed twin leaks into human output.
    assert!(!stdout.contains("\"data\""));
}

#[tokio::test]
async fn json_mode_emits_the_whole_body_including_the_typed_data_twin() {
    let server = MockServer::start().await;
    mount(
        &server,
        "explain",
        ResponseTemplate::new(200).set_body_json(explain_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "graph", "explain", "pat:56123456"],
    )
    .await;

    assert!(output.status.success());
    let value = stdout_json(&output);

    assert_eq!(value, explain_body());
    // No CLI envelope wrapper.
    assert!(value.get("ok").is_none());
}

#[tokio::test]
async fn path_found_prints_the_path_line_verbatim() {
    let server = MockServer::start().await;
    mount(
        &server,
        "path",
        ResponseTemplate::new(200).set_body_json(path_found_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "path", "EP3477840", "US5960411"],
    )
    .await;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("FOUND: 2 hops: EP3477840B1 → DE4302443 → US5960411A"));
}

/// "No path within the limit" is an answer, not a failure: the backend says
/// so on a 200 and the CLI exits 0, carrying the caveat that absence is not
/// proof.
#[tokio::test]
async fn path_not_found_renders_cleanly_and_exits_zero() {
    let server = MockServer::start().await;
    mount(
        &server,
        "path",
        ResponseTemplate::new(200).set_body_json(path_not_found_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "path", "EP3477840", "US5960411"],
    )
    .await;

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("NOT FOUND within the hop limit."));
    assert!(stdout.contains("absence is not proof"));
}

#[tokio::test]
async fn path_not_found_json_mode_keeps_found_false_in_the_data_twin() {
    let server = MockServer::start().await;
    mount(
        &server,
        "path",
        ResponseTemplate::new(200).set_body_json(path_not_found_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "--json",
            "patstat",
            "graph",
            "path",
            "EP3477840",
            "US5960411",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0));
    let value = stdout_json(&output);
    assert_eq!(value["data"]["found"], false);
    assert_eq!(value, path_not_found_body());
}

/*
 * ── bounds and the typed error family ───────────────────────────────────────
 */

/// The CLI does not police `depth` — the backend owns the bound, and its
/// typed message (which states the valid values) is relayed verbatim, so
/// there is one source of truth instead of two that can drift.
#[tokio::test]
async fn out_of_range_depth_relays_the_backend_message_verbatim() {
    let server = MockServer::start().await;
    let message = "`depth` must be 1 or 2 (per-hop cap 200 — engine spec #200).";
    mount(
        &server,
        "neighborhood",
        ResponseTemplate::new(400).set_body_json(error_body(
            "patstat_invalid_request",
            message,
            400,
        )),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "patstat",
            "graph",
            "neighborhood",
            "EP3477840",
            "--depth",
            "7",
        ],
    )
    .await;

    // Reached the backend rather than being rejected locally as a usage error.
    assert_ne!(output.status.code(), Some(2));
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("Invalid graph request."));
    assert!(stdout.contains(message));
}

#[tokio::test]
async fn out_of_range_max_hops_relays_the_backend_message_verbatim() {
    let server = MockServer::start().await;
    let message = "`max_hops` must be an integer between 1 and 4.";
    mount(
        &server,
        "path",
        ResponseTemplate::new(400).set_body_json(error_body(
            "patstat_invalid_request",
            message,
            400,
        )),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "patstat",
            "graph",
            "path",
            "EP3477840",
            "US5960411",
            "--max-hops",
            "9",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains(message));
}

/// Unlike depth/max_hops, an out-of-range token budget is CLAMPED by the
/// backend into [100, 20000] rather than refused — so the call succeeds and
/// the caller gets text rendered at the clamped budget.
#[tokio::test]
async fn out_of_range_token_budget_is_clamped_not_refused() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/patstat/graph/explain"))
        .and(query_param("token_budget", "999999"))
        .respond_with(ResponseTemplate::new(200).set_body_json(explain_body()))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "patstat",
            "graph",
            "explain",
            "pat:56123456",
            "--token-budget",
            "999999",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("# explain pat:56123456"));
}

/// An ambiguous node reaches these verbs as a 400 `patstat_invalid_request`,
/// NOT the composites' 422 `patstat_patent_ambiguous`: the backend's
/// resolveNodeArg guard refuses rather than raising the candidate error, and
/// names the candidates inside the message prose. The shared graph error
/// rendering relays that message verbatim, so the recovery instruction
/// ("re-run with an explicit node id") reaches the caller intact.
#[tokio::test]
async fn ambiguous_node_is_refused_as_invalid_request_with_the_candidates_named() {
    let server = MockServer::start().await;
    let message = "\"US5960411\" is ambiguous — it matches 2 distinct applications \
                   (US08655468 (A), US09123456 (A)). Re-run with an explicit node id \
                   (pat:<appln_id>) or a kind code.";
    mount(
        &server,
        "neighborhood",
        ResponseTemplate::new(400).set_body_json(error_body(
            "patstat_invalid_request",
            message,
            400,
        )),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "neighborhood", "US5960411"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("is ambiguous"));
    assert!(stdout.contains("US08655468 (A)"));
    assert!(stdout.contains("Re-run with an explicit node id"));
    // Nothing is auto-picked and no phantom 422 candidate list is invented.
    assert!(!stdout.contains("Candidates:"));
}

/// An applicant-entity name is likewise refused: graph verbs take patent
/// nodes, and the message routes the caller to the right surface.
#[tokio::test]
async fn entity_input_is_refused_with_the_routing_instruction() {
    let server = MockServer::start().await;
    let message = "\"Siemens\" is not a patent node — it matched applicant entities. \
                   Graph verbs take pat:<appln_id> or a publication number; use \
                   applicant_view for entities.";
    mount(
        &server,
        "explain",
        ResponseTemplate::new(400).set_body_json(error_body(
            "patstat_invalid_request",
            message,
            400,
        )),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "explain", "Siemens"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("is not a patent node"));
}

#[tokio::test]
async fn node_not_found_404_exits_with_the_not_found_code() {
    let server = MockServer::start().await;
    let body = error_body(
        "patstat_patent_not_found",
        "No publication in the loaded PATSTAT edition matches \"EP9999999\".",
        404,
    );
    mount(
        &server,
        "explain",
        ResponseTemplate::new(404).set_body_json(body.clone()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "graph", "explain", "EP9999999"],
    )
    .await;

    assert_eq!(output.status.code(), Some(5));
    // Typed graph errors relay the backend body unmodified in json mode.
    assert_eq!(stdout_json(&output), body);
}

#[tokio::test]
async fn patstat_unavailable_503_renders_plainly_for_every_verb() {
    let server = MockServer::start().await;
    mount(
        &server,
        "path",
        ResponseTemplate::new(503).set_body_json(error_body(
            "patstat_unavailable",
            "The PATSTAT analytics layer is not configured on this deployment.",
            503,
        )),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "path", "EP3477840", "US5960411"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout
        .contains("PATSTAT analytics unavailable: backend has no PATSTAT dataset configured."));
}

/// Untyped failures keep the CLI-wide envelope + hint-box rendering and its
/// documented exit code — the verbs inherit this from the shared graph error
/// path rather than handling auth themselves.
#[tokio::test]
async fn untyped_failure_falls_back_to_the_shared_envelope() {
    let server = MockServer::start().await;
    mount(
        &server,
        "neighborhood",
        ResponseTemplate::new(401).set_body_json(json!({ "error": "unauthorized" })),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "graph", "neighborhood", "EP3477840"],
    )
    .await;

    assert_eq!(output.status.code(), Some(3));
    let value = stdout_json(&output);
    assert_eq!(value["ok"], false);
    assert_eq!(value["status"], 401);
}
