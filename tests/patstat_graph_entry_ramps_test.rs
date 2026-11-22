//! `flowleap patstat graph cpc` and `flowleap patstat graph technology`
//! (issue #96): the two native entry ramps onto the technology landscape.
//! `cpc` turns a keyword into ranked CPC symbols (`patstat_cpc`);
//! `technology` takes one CPC prefix and answers the area card
//! (`patstat_technology`); with more prefixes or with `--keywords` it answers
//! a corpus landscape (flowleap-backend#642). Both run on the shared tool seam: `--json` is the
//! tool data verbatim, human mode renders it. The human output of each is
//! locked by a golden in tests/golden — run with `UPDATE_GOLDEN=1` to
//! regenerate after a deliberate rendering change.

mod support;

use std::path::Path;

use serde_json::{json, Value};
use support::{run_cli, stdout_json, tool_ok};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const API_KEY_ENV: (&str, &str) = ("FLOWLEAP_API_KEY", "fl_pat_test_key");
const ATTRIBUTION: &str =
    "This product contains data sourced from EPO databases, © European Patent Organisation";

fn assert_golden(got: &str, golden: &str) {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(golden);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&file, got).expect("write golden");
        return;
    }
    let want = std::fs::read_to_string(&file)
        .unwrap_or_else(|_| panic!("missing golden {golden}; run UPDATE_GOLDEN=1 cargo test"));
    assert_eq!(got, want, "golden mismatch: {golden}");
}

/// Shaped like the live `patstat_cpc` data (flowleap-backend
/// src/lib/patstat-graph/types.ts → CpcSearchResult, plus the facade's
/// top-level `attribution`).
fn cpc_data() -> Value {
    json!({
        "verb": "cpc_search",
        "query": "solid electrolyte",
        "candidates": [
            { "symbol": "H01M10/0562", "level": 10, "title": "Solid materials", "applications": 35939 },
            { "symbol": "H01M10/056", "level": 9,
              "title": "Accumulators characterised by the electrolytes, e.g. solid electrolytes",
              "applications": 61220 },
            { "symbol": "H01B1/06", "level": null,
              "title": "Conductors mainly consisting of other non-metallic substances; solid electrolytes",
              "applications": 8120 },
        ],
        "total": 14,
        "truncated": true,
        "data_edition": "PATSTAT 2026 Spring",
        "attribution": ATTRIBUTION,
    })
}

fn cpc_empty_data() -> Value {
    json!({
        "verb": "cpc_search",
        "query": "solid state battery",
        "candidates": [],
        "total": 0,
        "truncated": false,
        "data_edition": "PATSTAT 2026 Spring",
        "attribution": ATTRIBUTION,
    })
}

/// Shaped like the live `patstat_technology` data for H01M10/0562 (trimmed
/// lists; TechnologyView in flowleap-backend src/lib/patstat-graph/types.ts).
fn technology_data() -> Value {
    json!({
        "meta": {
            "composite": "technology_view",
            "data_edition": "PATSTAT 2026 Spring",
            "attribution": ATTRIBUTION,
            "cpc_prefix": "H01M10/0562",
            "caps": { "top_applicants": 15, "new_entrants": 10, "seminal_families": 10,
                      "top_inventors": 10, "geography": 10 },
            "truncation": {
                "top_applicants": { "shown": 2, "total": 3473, "truncated": true },
                "new_entrants": { "shown": 1, "total": 1, "truncated": false },
                "seminal_families": { "shown": 1, "total": 12909, "truncated": true },
                "top_inventors": { "shown": 1, "total": 37529, "truncated": true },
            },
            "notes": [
                "Filing trend covers earliest filing years 2001–2026; the most recent ~24 months are incomplete (publication lag) — never read the tail as decline.",
            ],
            "sources": { "area": "tls224 × tls201" },
            "data_quality": [
                { "node": "cpc:H01M10/0562", "issue": "unknown_filing_date",
                  "detail": "1 famil(ies) carry PATSTAT's 9999 unknown-date sentinel — excluded from filing_trend, the gap is real." },
            ],
        },
        "area": { "cpc_prefix": "H01M10/0562", "title": "Solid materials",
                  "families": 12909, "applications": 35939 },
        "top_applicants": [
            { "psn_id": 33067109, "name": "TOYOTA MOTOR CORPORATION", "families": 610, "active_since": 2016 },
            { "psn_id": 24799563, "name": "PANASONIC INTELLECTUAL PROPERTY", "families": 507, "active_since": 2016 },
        ],
        "filing_trend": [
            { "year": 2001, "families": 27 },
            { "year": 2002, "families": 45 },
        ],
        "grant_rate": [
            { "office": "JP", "applications": 2933, "granted": 2382, "grant_rate_pct": 81.2 },
            { "office": "US", "applications": 3882, "granted": 2622, "grant_rate_pct": 67.5 },
        ],
        "new_entrants": [
            { "psn_id": 183880853, "name": "KIA MOTORS CORP.", "families": 123, "active_since": 2021 },
        ],
        "seminal_families": [
            { "family_id": 27392946, "citing_families": 620, "filing_year": 2000,
              "publication": "WO0173957",
              "title": "BATTERY-OPERATED WIRELESS-COMMUNICATION APPARATUS AND METHOD" },
        ],
        "top_inventors": [
            { "psn_id": 28166147, "name": "SAKAI AKIHIRO", "families": 97 },
        ],
        "geography": {
            "inventor_countries": [ { "country": "JP", "families": 3993 } ],
            "applicant_countries": [ { "country": "JP", "families": 3918 } ],
            "offices": [ { "country": "CN", "families": 8167 } ],
            "truncation": {
                "inventor_countries": { "shown": 1, "total": 60, "truncated": true },
                "applicant_countries": { "shown": 1, "total": 45, "truncated": true },
                "offices": { "shown": 1, "total": 56, "truncated": true },
            },
        },
    })
}

/// Shaped like the live `patstat_technology` data for a corpus (trimmed
/// lists; TechnologyCorpusView in flowleap-backend
/// src/lib/patstat-graph/types.ts, built in technology-corpus-view.ts).
fn technology_corpus_data() -> Value {
    json!({
        "meta": {
            "composite": "technology_view",
            "mode": "corpus",
            "data_edition": "PATSTAT 2026 Spring",
            "attribution": ATTRIBUTION,
            "caps": { "top_applicants": 15, "new_entrants": 10, "offices": 10, "top_cpc": 10,
                      "seminal_families": 10, "corpus_families": 20000 },
            "truncation": {
                "top_applicants": { "shown": 2, "total": 412, "truncated": true },
                "new_entrants": { "shown": 1, "total": 1, "truncated": false },
                "offices": { "shown": 2, "total": 31, "truncated": true },
                "top_cpc": { "shown": 2, "total": 955, "truncated": true },
                "seminal_families": { "shown": 1, "total": 1840, "truncated": true },
            },
            "notes": [
                "This is a corpus landscape (`scope`), not the card of one CPC prefix: the numbers are for the families that match the scope.",
                "Filing trend covers earliest filing years 2001–2026; the most recent ~24 months are incomplete (publication lag) — never read the tail as decline.",
            ],
            "applicant_merge": null,
            "sources": { "corpus": "tls202 / tls203 (English full text) × tls224 × tls201" },
            "data_quality": [
                { "node": "corpus", "issue": "unknown_filing_date",
                  "detail": "3 famil(ies) carry PATSTAT's 9999 unknown-date sentinel — excluded from filing_trend, the gap is real." },
            ],
        },
        "scope": {
            "keywords": ["\"solid electrolyte\"", "sulfide or sulphide or argyrodite"],
            "text_query": "('solid' <-> 'electrolyt') & ('sulfid' | 'sulphid' | 'argyrodit')",
            "text": "English title or English abstract",
            "cpc": ["H01M10/0562", "H01M10/052"],
            "exclude_cpc": ["H01M8"],
            "counting": "DOCDB simple families. A family is in the corpus when one of its applications matches the scope; each section counts all applications of the corpus families.",
            "lower_bound": true,
        },
        "area": { "families": 1840, "applications": 6211, "matched_applications": 2977 },
        "top_applicants": [
            { "psn_id": 33067109, "name": "TOYOTA MOTOR CORPORATION", "families": 214, "active_since": 2016 },
            { "psn_id": 12345678, "name": "IDEMITSU KOSAN", "families": 131, "active_since": 2016 },
        ],
        "filing_trend": [
            { "year": 2019, "families": 148 },
            { "year": 2020, "families": 173 },
        ],
        "grant_rate": [
            { "office": "JP", "applications": 512, "granted": 401, "grant_rate_pct": 78.3 },
        ],
        "new_entrants": [
            { "psn_id": 183880853, "name": "KIA MOTORS CORP.", "families": 22, "active_since": 2021 },
        ],
        "offices": [
            { "office": "JP", "families": 1102 },
            { "office": "US", "families": 987 },
        ],
        "top_cpc": [
            { "symbol": "H01M10/0562", "title": "Solid materials", "families": 1711 },
            { "symbol": "H01M10/0525", "title": null, "families": 1203 },
        ],
        "seminal_families": [
            { "family_id": 44115555, "citing_families": 310, "filing_year": 2010,
              "publication": "US8697292B2",
              "title": "SULFIDE SOLID ELECTROLYTE MATERIAL AND LITHIUM SOLID STATE BATTERY" },
        ],
    })
}

async fn mount(server: &MockServer, tool: &str, input: Value, template: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path(format!("/v1/tools/{tool}")))
        .and(body_json(input))
        .respond_with(template)
        .mount(server)
        .await;
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf8")
}

/*
 * ── graph cpc ───────────────────────────────────────────────────────────────
 */

#[tokio::test]
async fn cpc_renders_symbol_title_and_applications_per_candidate() {
    let server = MockServer::start().await;
    mount(
        &server,
        "patstat_cpc",
        json!({ "q": "solid electrolyte" }),
        ResponseTemplate::new(200).set_body_json(tool_ok("patstat_cpc", cpc_data())),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "cpc", "solid electrolyte"],
    )
    .await;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = stdout(&output);
    assert!(stdout.contains("H01M10/0562"));
    assert!(stdout.contains("Solid materials"));
    assert!(stdout.contains("35939"));
    // The shown count is never presented as the total.
    assert!(stdout.contains("Showing 3 of 14 matching CPC symbols."));
    assert!(stdout.contains("graph technology"));
    assert!(stdout.contains("Source: PATSTAT data edition PATSTAT 2026 Spring."));
    assert_golden(&stdout, "patstat-graph-cpc.txt");
}

#[tokio::test]
async fn cpc_json_mode_prints_the_tool_data_verbatim() {
    let server = MockServer::start().await;
    mount(
        &server,
        "patstat_cpc",
        json!({ "q": "solid electrolyte" }),
        ResponseTemplate::new(200).set_body_json(tool_ok("patstat_cpc", cpc_data())),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "graph", "cpc", "solid electrolyte"],
    )
    .await;

    assert!(output.status.success());
    assert_eq!(stdout_json(&output), cpc_data());
}

/// The issue's own example, `graph cpc "solid state battery"`, answers
/// `total: 0` on the live edition today (backend #477). A searched-and-empty
/// answer is not a failure: one clean line, exit 0, no empty list.
#[tokio::test]
async fn cpc_solid_state_battery_with_no_candidates_says_so_cleanly_and_exits_zero() {
    let server = MockServer::start().await;
    mount(
        &server,
        "patstat_cpc",
        json!({ "q": "solid state battery" }),
        ResponseTemplate::new(200).set_body_json(tool_ok("patstat_cpc", cpc_empty_data())),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "cpc", "solid state battery"],
    )
    .await;

    assert_eq!(output.status.code(), Some(0));
    let stdout = stdout(&output);
    assert!(stdout.contains("No CPC candidates match \"solid state battery\""));
    assert!(!stdout.contains("Symbol"));
}

/*
 * ── graph technology ────────────────────────────────────────────────────────
 */

#[tokio::test]
async fn technology_renders_the_area_card_applicants_trend_and_grant_rate() {
    let server = MockServer::start().await;
    mount(
        &server,
        "patstat_technology",
        json!({ "cpc": "H01M10/0562" }),
        ResponseTemplate::new(200).set_body_json(tool_ok("patstat_technology", technology_data())),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "technology", "H01M10/0562"],
    )
    .await;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = stdout(&output);

    // Area card first, then top applicants, then trend and grant rate.
    let card = stdout.find("Technology area: H01M10/0562").expect("card");
    let applicants = stdout.find("Top Applicants").expect("applicants");
    let trend = stdout.find("Filing Trend").expect("trend");
    let grant = stdout.find("Grant Rate by Office").expect("grant rate");
    assert!(card < applicants && applicants < trend && trend < grant);

    assert!(stdout.contains("Solid materials"));
    assert!(stdout.contains("TOYOTA MOTOR CORPORATION"));
    assert!(stdout.contains("33067109"));
    assert!(stdout.contains("Showing 2 of 3473 top applicants."));
    assert!(stdout.contains("81.2"));
    assert!(stdout.contains("Data quality flags:"));
    assert!(stdout.contains("Source: PATSTAT data edition PATSTAT 2026 Spring."));
    assert!(stdout.contains(ATTRIBUTION));
    assert_golden(&stdout, "patstat-graph-technology.txt");
}

#[tokio::test]
async fn technology_json_mode_prints_the_tool_data_verbatim() {
    let server = MockServer::start().await;
    mount(
        &server,
        "patstat_technology",
        json!({ "cpc": "H01M10/0562" }),
        ResponseTemplate::new(200).set_body_json(tool_ok("patstat_technology", technology_data())),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "graph", "technology", "H01M10/0562"],
    )
    .await;

    assert!(output.status.success());
    assert_eq!(stdout_json(&output), technology_data());
}

/// A malformed CPC prefix is the engine's 400 `patstat_invalid_request`,
/// relayed with its message and the 400 exit mapping.
#[tokio::test]
async fn technology_invalid_prefix_relays_the_backend_message() {
    let server = MockServer::start().await;
    let message = "`cpc` must be a CPC symbol prefix such as H01M10/0562, G06F or Y02E.";
    mount(
        &server,
        "patstat_technology",
        json!({ "cpc": "battery" }),
        ResponseTemplate::new(400).set_body_json(json!({
            "success": false,
            "error": { "code": "patstat_invalid_request", "message": message },
            "status": 400,
        })),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "graph", "technology", "battery"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout(&output);
    assert!(stdout.contains("Invalid graph request."));
    assert!(stdout.contains(message));
}

/*
 * ── graph technology, corpus ────────────────────────────────────────────────
 */

/// The request of the corpus tests: two prefixes, two keyword entries and
/// one excluded prefix.
const CORPUS_ARGS: [&str; 10] = [
    "technology",
    "H01M10/0562",
    "--cpc",
    "H01M10/052",
    "--keywords",
    "\"solid electrolyte\"",
    "--keywords",
    "sulfide or sulphide or argyrodite",
    "--exclude-cpc",
    "H01M8",
];

fn corpus_input() -> Value {
    json!({
        "cpc": ["H01M10/0562", "H01M10/052"],
        "keywords": ["\"solid electrolyte\"", "sulfide or sulphide or argyrodite"],
        "exclude_cpc": "H01M8",
    })
}

#[tokio::test]
async fn technology_corpus_renders_scope_size_and_sections() {
    let server = MockServer::start().await;
    mount(
        &server,
        "patstat_technology",
        corpus_input(),
        ResponseTemplate::new(200)
            .set_body_json(tool_ok("patstat_technology", technology_corpus_data())),
    )
    .await;

    let mut args = vec!["patstat", "graph"];
    args.extend_from_slice(&CORPUS_ARGS);
    let output = run_cli(&server.uri(), &[API_KEY_ENV], &args).await;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = stdout(&output);

    // The scope first, then the size, then the sections.
    let scope = stdout.find("Technology corpus").expect("scope");
    let size = stdout.find("Families: 1840").expect("size");
    let applicants = stdout.find("Top Applicants").expect("applicants");
    let trend = stdout.find("Filing Trend").expect("trend");
    let top_cpc = stdout.find("Top CPC Symbols").expect("top cpc");
    let offices = stdout.find("Filing Offices").expect("offices");
    assert!(scope < size && size < applicants && applicants < trend);
    assert!(trend < top_cpc && top_cpc < offices);

    assert!(stdout.contains("CPC (one of): H01M10/0562, H01M10/052"));
    assert!(stdout.contains(
        "Keywords (each entry must match): \"solid electrolyte\" · sulfide or sulphide or argyrodite"
    ));
    assert!(stdout.contains("Excluded CPC: H01M8"));
    assert!(stdout.contains("Applications that match the scope: 2977"));
    assert!(stdout.contains("LOWER BOUND"));
    assert!(stdout.contains("Showing 2 of 412 top applicants."));
    assert!(stdout.contains("Showing 2 of 955 CPC symbols."));
    assert!(stdout.contains("Showing 2 of 31 filing offices."));
    // The families without a filing date are shown, not dropped.
    assert!(stdout.contains("3 famil(ies) carry PATSTAT's 9999 unknown-date sentinel"));
    // A corpus has no inventor ranking and no CPC-card headline.
    assert!(!stdout.contains("Top Inventors"));
    assert!(!stdout.contains("Technology area:"));
    assert!(stdout.contains("Source: PATSTAT data edition PATSTAT 2026 Spring."));
    assert!(stdout.contains(ATTRIBUTION));
    assert_golden(&stdout, "patstat-graph-technology-corpus.txt");
}

#[tokio::test]
async fn technology_corpus_json_mode_prints_the_tool_data_verbatim() {
    let server = MockServer::start().await;
    mount(
        &server,
        "patstat_technology",
        corpus_input(),
        ResponseTemplate::new(200)
            .set_body_json(tool_ok("patstat_technology", technology_corpus_data())),
    )
    .await;

    let mut args = vec!["--json", "patstat", "graph"];
    args.extend_from_slice(&CORPUS_ARGS);
    let output = run_cli(&server.uri(), &[API_KEY_ENV], &args).await;

    assert!(output.status.success());
    assert_eq!(stdout_json(&output), technology_corpus_data());
}

/// A corpus with no family is the engine's 404 `patstat_entity_not_found`
/// with `details.reason: "empty_corpus"`. The headline names the scope, not
/// an applicant entity, and the backend hint is relayed.
#[tokio::test]
async fn technology_empty_corpus_names_the_scope_and_relays_the_hint() {
    let server = MockServer::start().await;
    let message = "No family in the loaded PATSTAT edition matches this scope. Use fewer or more general entries.";
    mount(
        &server,
        "patstat_technology",
        json!({ "keywords": "argyrodite zebra" }),
        ResponseTemplate::new(404).set_body_json(json!({
            "success": false,
            "error": {
                "code": "patstat_entity_not_found",
                "message": message,
                "details": { "reason": "empty_corpus", "hint": "Use fewer or more general entries." },
            },
            "status": 404,
        })),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "patstat",
            "graph",
            "technology",
            "--keywords",
            "argyrodite zebra",
        ],
    )
    .await;

    assert!(!output.status.success());
    let stdout = stdout(&output);
    assert!(stdout.contains(message));
    assert!(!stdout.contains("applicant entity"));
}
