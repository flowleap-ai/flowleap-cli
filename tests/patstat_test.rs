//! `flowleap patstat portfolio` (issue #32, PRD 0011; on the tool seam since
//! #95): success rendering in both output modes, the 422 ambiguous-applicant
//! interaction step, the typed `patstat_unavailable` unavailability, and an
//! auth failure; and `patstat docs --part index` / `--view <name>` (#108) — driven through the real binary against a wiremock backend
//! answering `POST /v1/tools/patstat_portfolio` with facade envelopes,
//! matching the exit-code contract's test harness (see
//! tests/exit_codes_test.rs).

mod support;

use serde_json::json;
use support::{run_cli, stdout_json};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const API_KEY_ENV: (&str, &str) = ("FLOWLEAP_API_KEY", "fl_pat_test_key");

/// The `patstat_portfolio` tool's `data` payload, shaped like the real
/// backend result (see flowleap-backend src/lib/patstat/portfolio.ts): the
/// route's data plus the EPO attribution.
fn portfolio_data() -> serde_json::Value {
    json!({
        "applicant": {
            "query": "Siemens",
            "matched_name": "SIEMENS AG",
            "matched_psn_names": ["SIEMENS AG", "SIEMENS AKTIENGESELLSCHAFT"],
            "other_matches": [],
        },
        "filters": { "from_year": 2015, "to_year": 2024 },
        "totals": { "applications": 120, "granted": 80 },
        "by_year": [
            { "year": 2015, "applications": 10, "granted": 8 },
            { "year": 2016, "applications": 12, "granted": 9 },
        ],
        "by_office": [
            { "office": "EP", "applications": 70, "granted": 50 },
            { "office": "WO", "applications": 50, "granted": null },
        ],
        "by_year_office": [
            { "year": 2015, "office": "EP", "applications": 6, "granted": 5 },
        ],
        "grant_status_caveats": [
            "WO: PCT applications never grant at WIPO — grant status is structurally meaningless",
            "Grant counts for the flagged authorities are reported as null and excluded from totals.",
        ],
        "notes": [],
        "summary": "SIEMENS AG: 120 patent applications filed 2015–2024 across 2 offices \
                     (top: EP 70, WO 50); 80 granted among offices with reliable grant status. \
                     Source: 2024 Autumn.",
        "data_edition": "2024 Autumn",
        "attribution": "This product contains data sourced from EPO databases, © European Patent Organisation",
    })
}

/// The facade success envelope around [`portfolio_data`].
fn success_body() -> serde_json::Value {
    json!({
        "success": true,
        "tool": "patstat_portfolio",
        "data": portfolio_data(),
        "executionTimeMs": 12,
    })
}

/// Canned 422 ambiguous-applicant facade error envelope: the facade nests
/// the candidates under `error.details` (the route flattened them into
/// `error`).
fn ambiguous_body() -> serde_json::Value {
    json!({
        "success": false,
        "error": {
            "code": "patstat_applicant_ambiguous",
            "message": "\"Kia\" matches 2 distinct applicant entities: KIA MOTORS (500 \
                         applications), KIA CORPORATION (10 applications). These may be \
                         separate companies, so they are not merged automatically — retry \
                         with a more specific name (one of the entities listed).",
            "details": {
                "candidates": [
                    { "name": "KIA MOTORS", "applications": 500 },
                    { "name": "KIA CORPORATION", "applications": 10 },
                ],
            },
        },
        "status": 422,
    })
}

/// Canned 503 `patstat_unavailable` error body.
fn unavailable_body() -> serde_json::Value {
    json!({
        "success": false,
        "error": {
            "code": "patstat_unavailable",
            "message": "The PATSTAT analytics layer is not configured on this deployment \
                         (PATSTAT_DATABASE_URL is unset). Aggregate portfolio analytics are \
                         unavailable — for individual documents use /v1/patent-search (EPO) or \
                         /v1/patent-search-uspto (USPTO) instead.",
        },
        "status": 503,
    })
}

async fn mount_portfolio(server: &MockServer, template: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/v1/tools/patstat_portfolio"))
        .respond_with(template)
        .mount(server)
        .await;
}

#[tokio::test]
async fn portfolio_sends_the_documented_request_shape() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/patstat_portfolio"))
        .and(body_json(json!({
            "applicant": "Siemens",
            "from_year": 2015,
            "to_year": 2024,
            "offices": "all",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body()))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "--json",
            "patstat",
            "portfolio",
            "Siemens",
            "--from-year",
            "2015",
            "--to-year",
            "2024",
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
async fn portfolio_omits_absent_year_bounds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/patstat_portfolio"))
        .and(body_json(
            json!({ "applicant": "Siemens", "offices": "all" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body()))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "portfolio", "Siemens"],
    )
    .await;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn success_human_mode_renders_summary_tables_and_provenance() {
    let server = MockServer::start().await;
    mount_portfolio(
        &server,
        ResponseTemplate::new(200).set_body_json(success_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "portfolio", "Siemens"],
    )
    .await;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");

    assert!(stdout.contains("SIEMENS AG: 120 patent applications"));
    assert!(stdout.contains("Filings by Year"));
    assert!(stdout.contains("Filings by Office"));
    assert!(stdout.contains("Grant status caveats:"));
    assert!(stdout.contains("PCT applications never grant at WIPO"));
    assert!(stdout.contains("Source: PATSTAT data edition 2024 Autumn"));
    // Real table cells, not a raw JSON dump.
    assert!(stdout.contains("2015"));
    assert!(!stdout.contains("\"by_year\""));
}

#[tokio::test]
async fn success_json_mode_emits_the_tool_data_verbatim() {
    let server = MockServer::start().await;
    mount_portfolio(
        &server,
        ResponseTemplate::new(200).set_body_json(success_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "portfolio", "Siemens"],
    )
    .await;

    assert!(output.status.success());
    let value = stdout_json(&output);

    // The verbatim-JSON convention: `--json` prints the tool's `data`
    // payload exactly, never the facade envelope around it.
    assert_eq!(value, portfolio_data());
    assert_eq!(value["applicant"]["matched_name"], "SIEMENS AG");
    assert_eq!(value["totals"]["applications"], 120);
    assert_eq!(value["by_year"][0]["year"], 2015);
    assert_eq!(value["by_office"][1]["office"], "WO");
    assert!(value["by_office"][1]["granted"].is_null());
    assert_eq!(value["data_edition"], "2024 Autumn");
    assert!(value.get("ok").is_none());
    assert!(value.get("success").is_none());
}

#[tokio::test]
async fn ambiguous_422_renders_candidates_in_human_mode() {
    let server = MockServer::start().await;
    mount_portfolio(
        &server,
        ResponseTemplate::new(422).set_body_json(ambiguous_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "portfolio", "Kia"],
    )
    .await;

    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");

    assert!(stdout.contains("Ambiguous applicant"));
    assert!(stdout.contains("Candidates:"));
    assert!(stdout.contains("KIA MOTORS (500 applications)"));
    assert!(stdout.contains("KIA CORPORATION (10 applications)"));
    assert!(stdout.contains("Re-run with one exact candidate name"));
    // Never a silent pick between the two entities.
    assert!(!stdout.contains("\"error\""));
}

#[tokio::test]
async fn ambiguous_422_renders_candidates_in_json_mode() {
    let server = MockServer::start().await;
    mount_portfolio(
        &server,
        ResponseTemplate::new(422).set_body_json(ambiguous_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "portfolio", "Kia"],
    )
    .await;

    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(0));
    let value = stdout_json(&output);

    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "patstat_applicant_ambiguous");
    let candidates = &value["error"]["details"]["candidates"];
    assert_eq!(candidates[0]["name"], "KIA MOTORS");
    assert_eq!(candidates[0]["applications"], 500);
    assert_eq!(candidates[1]["name"], "KIA CORPORATION");
    // The backend details ride along verbatim.
    assert_eq!(
        value["error"]["details"],
        ambiguous_body()["error"]["details"]
    );
    assert_eq!(output.status.code(), Some(1));
}

#[tokio::test]
async fn patstat_unavailable_renders_plainly_in_human_mode() {
    let server = MockServer::start().await;
    mount_portfolio(
        &server,
        ResponseTemplate::new(503).set_body_json(unavailable_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "portfolio", "Siemens"],
    )
    .await;

    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout
        .contains("PATSTAT analytics unavailable: backend has no PATSTAT dataset configured."));
}

#[tokio::test]
async fn patstat_unavailable_renders_plainly_in_json_mode() {
    let server = MockServer::start().await;
    mount_portfolio(
        &server,
        ResponseTemplate::new(503).set_body_json(unavailable_body()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "portfolio", "Siemens"],
    )
    .await;

    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(0));
    let value = stdout_json(&output);
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "patstat_unavailable");
    assert!(value["error"]["message"]
        .as_str()
        .is_some_and(|m| m.contains("not configured")));
}

/// An HTTP 401 (rejected/expired credentials) is not retried and exits with
/// the documented auth-required code, same as every other authenticated
/// command.
#[tokio::test]
async fn auth_failure_401_exits_with_auth_required_code() {
    let server = MockServer::start().await;
    mount_portfolio(
        &server,
        ResponseTemplate::new(401).set_body_json(json!({ "error": "unauthorized" })),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "portfolio", "Siemens"],
    )
    .await;

    assert_eq!(output.status.code(), Some(3));
    let value = stdout_json(&output);
    assert_eq!(value["ok"], false);
    assert_eq!(value["status"], 401);
}

/// With no credentials configured at all, the command fails fast locally
/// (never reaches the network) — same `require_auth` guard every other
/// authenticated command uses.
#[tokio::test]
async fn missing_credentials_fails_locally_without_a_network_call() {
    let output = run_cli(
        "http://127.0.0.1:9",
        &[],
        &["--json", "patstat", "portfolio", "Siemens"],
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

/// A compact reply (`offices: "top"`, backend #484) tells the reader the
/// year-by-office matrix is cut and how to get every office, and a capped
/// sibling list tells how many other entities matched (#107).
#[tokio::test]
async fn truncated_scope_and_capped_siblings_print_their_footers() {
    let mut data = portfolio_data();
    data["by_year_office_scope"] = json!({
        "offices_shown": 8,
        "offices_total": 49,
        "truncated": true,
    });
    data["applicant"]["other_matches"] = json!([
        { "name": "SIEMENS-SCHUCKERTWERKE", "applications": 51309 },
        { "name": "SIEMENS HEALTHCARE", "applications": 8975 },
    ]);
    data["applicant"]["other_matches_total"] = json!(188);
    let server = MockServer::start().await;
    mount_portfolio(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "tool": "patstat_portfolio",
            "data": data,
        })),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "portfolio", "Siemens", "--offices", "top"],
    )
    .await;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains(
            "Showing 8 of 49 offices in the year-by-office matrix (--offices all for every office)"
        ),
        "{stdout}"
    );
    assert!(stdout.contains("SIEMENS-SCHUCKERTWERKE"), "{stdout}");
    assert!(
        stdout.contains("… and 186 more (the tool caps the list at 10)"),
        "{stdout}"
    );
}

/// A complete reply prints neither footer.
#[tokio::test]
async fn complete_scope_prints_no_footer() {
    let mut data = portfolio_data();
    data["by_year_office_scope"] = json!({
        "offices_shown": 2,
        "offices_total": 2,
        "truncated": false,
    });
    data["applicant"]["other_matches_total"] = json!(0);
    let server = MockServer::start().await;
    mount_portfolio(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "tool": "patstat_portfolio",
            "data": data,
        })),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "portfolio", "Siemens"],
    )
    .await;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(!stdout.contains("Showing"), "{stdout}");
    assert!(!stdout.contains("more (the tool caps"), "{stdout}");
    assert!(!stdout.contains("Other matching applicants"), "{stdout}");
}

// ---------------------------------------------------------------------------
// `patstat docs --part index` / `--view <name>` (#108): the semantic model in
// parts, on the `patstat_docs` tool.
// ---------------------------------------------------------------------------

/// The `part: "index"` payload, shaped like the live backend answer.
fn index_data() -> serde_json::Value {
    json!({
        "part": "index",
        "data_edition": "PATSTAT 2026 Spring",
        "note": "The catalog of the semantic model. Read it first.",
        "model": { "name": "flowleap-patstat" },
        "glossary": { "family": "A DOCDB simple family." },
        "global_caveats": { "publication_lag": "Recent counts are incomplete." },
        "interpretation_conventions": {
            "defaults": { "counting_unit": "FAMILIES for invention-level counting." },
            "when_to_ask": "Ask only when interpretations diverge materially."
        },
        "view_conventions": { "families": ["applications"] },
        "metrics": { "grant_rate": "granted / applications" },
        "logical_tables": [
            { "name": "applications",
              "description": "One row per patent application filed anywhere in the world.",
              "columns": ["application_id", "office", "filing_year", "family_id"] }
        ]
    })
}

/// The `view: "applications"` payload: the table in full, the conventions it
/// names, the caveats that bear on it (top-level `global_caveats`, filtered
/// by the backend) and the join paths that name it.
fn view_data() -> serde_json::Value {
    json!({
        "data_edition": "PATSTAT 2026 Spring",
        "view": {
            "name": "applications",
            "description": "One row per patent application. THE anchor table.",
            "physical": "tls201_appln",
            "columns": {
                "application_id": { "type": "bigint", "description": "Primary key; joins to every other table." },
                "family_id": { "type": "bigint", "description": "DOCDB family id." }
            },
            "conventions": ["families"]
        },
        "interpretation_conventions": { "families": "TWO family notions, and picking the wrong one changes the number." },
        "global_caveats": { "wo_never_grants": "WO applications never grant by definition." },
        "join_paths": ["applications.application_id = applicants.application_id  (portfolio questions)"]
    })
}

async fn mount_docs(server: &MockServer, input: serde_json::Value, template: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/v1/tools/patstat_docs"))
        .and(body_json(input))
        .respond_with(template)
        .mount(server)
        .await;
}

fn ok_docs(data: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "success": true, "tool": "patstat_docs", "data": data, "executionTimeMs": 12,
    }))
}

#[tokio::test]
async fn docs_index_human_mode_renders_conventions_then_the_view_table() {
    let server = MockServer::start().await;
    mount_docs(
        &server,
        json!({ "section": "semantic-model", "part": "index" }),
        ok_docs(index_data()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "patstat",
            "docs",
            "--section",
            "semantic-model",
            "--part",
            "index",
        ],
    )
    .await;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("# Loaded edition: PATSTAT 2026 Spring"),
        "{stdout}"
    );
    let conventions = stdout.find("\nConventions\n").expect("conventions heading");
    assert!(stdout.contains("    counting_unit: FAMILIES for invention-level counting."));
    assert!(stdout.contains("  when_to_ask: Ask only when interpretations diverge materially."));
    let views = stdout.find("\nViews\n").expect("views heading");
    assert!(
        conventions < views,
        "conventions come before the table: {stdout}"
    );
    for header in ["View", "Description", "Columns"] {
        assert!(stdout.contains(header), "header {header}: {stdout}");
    }
    // One row, printed whole: no cell is cut at 50 characters.
    assert!(stdout.contains("applications"));
    assert!(stdout.contains("One row per patent application filed anywhere in the world."));
    assert!(stdout.contains("application_id, office, filing_year, family_id"));
}

#[tokio::test]
async fn docs_view_human_mode_renders_columns_conventions_caveats_and_join_paths() {
    let server = MockServer::start().await;
    mount_docs(
        &server,
        json!({ "section": "semantic-model", "view": "applications" }),
        ok_docs(view_data()),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "patstat",
            "docs",
            "--section",
            "semantic-model",
            "--view",
            "applications",
        ],
    )
    .await;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("applications: One row per patent application. THE anchor table."));
    assert!(stdout.contains("Physical: tls201_appln"));
    let sections: Vec<usize> = [
        "\nColumns\n",
        "\nConventions\n",
        "\nCaveats\n",
        "\nJoin paths\n",
    ]
    .iter()
    .map(|heading| {
        stdout
            .find(heading)
            .unwrap_or_else(|| panic!("{heading}: {stdout}"))
    })
    .collect();
    assert!(
        sections.windows(2).all(|w| w[0] < w[1]),
        "section order: {stdout}"
    );
    assert!(stdout.contains("Primary key; joins to every other table."));
    assert!(stdout.contains("bigint"));
    assert!(stdout.contains("  families: TWO family notions"));
    assert!(stdout.contains("  wo_never_grants: WO applications never grant by definition."));
    assert!(stdout.contains(
        "  - applications.application_id = applicants.application_id  (portfolio questions)"
    ));
}

#[tokio::test]
async fn docs_part_and_view_json_mode_emit_the_tool_data_verbatim() {
    let server = MockServer::start().await;
    mount_docs(
        &server,
        json!({ "section": "semantic-model", "part": "index" }),
        ok_docs(index_data()),
    )
    .await;
    mount_docs(
        &server,
        json!({ "section": "semantic-model", "view": "applications" }),
        ok_docs(view_data()),
    )
    .await;

    for (selector, value, want) in [
        ("--part", "index", index_data()),
        ("--view", "applications", view_data()),
    ] {
        let output = run_cli(
            &server.uri(),
            &[API_KEY_ENV],
            &[
                "--json",
                "patstat",
                "docs",
                "--section",
                "semantic-model",
                selector,
                value,
            ],
        )
        .await;
        assert!(output.status.success(), "{selector} {value}");
        assert_eq!(stdout_json(&output), want, "{selector} {value}");
    }
}

#[tokio::test]
async fn docs_unknown_view_human_mode_lists_the_available_views() {
    let server = MockServer::start().await;
    mount_docs(
        &server,
        json!({ "section": "semantic-model", "view": "nope" }),
        ResponseTemplate::new(404).set_body_json(json!({
            "success": false,
            "error": {
                "code": "NOT_FOUND",
                "message": "View 'nope' not found",
                "details": { "availableViews": ["applications", "applicants"] },
            },
        })),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "patstat",
            "docs",
            "--section",
            "semantic-model",
            "--view",
            "nope",
        ],
    )
    .await;

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("  Error: View 'nope' not found"),
        "{stdout}"
    );
    assert!(
        stdout.contains("  Available views: applications, applicants"),
        "{stdout}"
    );
}

/// `--part` and `--view` exclude each other and only go with `--section
/// semantic-model`: every broken combination is a usage error (exit 2) that
/// names the rule, in both output modes, and no request is sent.
#[tokio::test]
async fn docs_part_and_view_need_the_semantic_model_section() {
    let server = MockServer::start().await;
    for (args, names) in [
        (
            [
                "--section",
                "semantic-model",
                "--part",
                "index",
                "--view",
                "applications",
            ]
            .as_slice(),
            "--view",
        ),
        (["--part", "index"].as_slice(), "--section semantic-model"),
        (
            ["--view", "applications"].as_slice(),
            "--section semantic-model",
        ),
        (
            ["--section", "examples", "--part", "index"].as_slice(),
            "--section semantic-model",
        ),
        (
            ["--workflow", "graph", "--view", "applications"].as_slice(),
            "--view",
        ),
    ] {
        for json_mode in [false, true] {
            let mut full = Vec::new();
            if json_mode {
                full.push("--json");
            }
            full.extend(["patstat", "docs"]);
            full.extend_from_slice(args);
            let output = run_cli(&server.uri(), &[API_KEY_ENV], &full).await;
            assert_eq!(output.status.code(), Some(2), "{full:?} is a usage error");
            let shown = if json_mode {
                let value = stdout_json(&output);
                assert_eq!(value["ok"], false, "{full:?}");
                value["error"]["message"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string()
            } else {
                String::from_utf8_lossy(&output.stderr).to_string()
            };
            assert!(shown.contains(names), "{full:?} names {names}: {shown}");
        }
    }
    assert!(
        server
            .received_requests()
            .await
            .unwrap_or_default()
            .is_empty(),
        "no request is sent"
    );
}
