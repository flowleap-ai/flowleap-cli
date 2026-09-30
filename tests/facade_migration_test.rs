//! Every data command runs on the `/v1/tools` facade (backend PRD 0013 Phase
//! 2). Dry-run mode surfaces the exact request a command would send, so these
//! assert the tool name (URL) and the tool-input JSON shape without a live
//! backend — the same seam `facade_test.rs` uses for the ergonomic verbs.
//!
//! `keys test`/`keys set` are the named non-facade exception and are
//! deliberately absent here. Since #96 no command calls a `/v1/patstat` route.

use std::process::Command;

use serde_json::{json, Value};

/// Run `flowleap --json <args> --dry-run` in an isolated HOME and parse the
/// dry-run description.
fn dry_run(args: &[&str]) -> Value {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let mut full_args = vec!["--json"];
    full_args.extend_from_slice(args);
    full_args.push("--dry-run");

    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env_remove("FLOWLEAP_API_KEY")
        .env_remove("FLOWLEAP_TOKEN")
        .args(&full_args)
        .output()
        .expect("run flowleap dry-run");

    assert!(
        output.status.success(),
        "dry-run failed for {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    serde_json::from_str(&stdout).expect("stdout is json")
}

/// Assert a command POSTs to `tool` with exactly the expected input fields.
fn assert_tool_call(args: &[&str], tool: &str, expected: &[(&str, Value)]) {
    let value = dry_run(args);
    assert_eq!(value["method"], "POST", "{args:?}");
    assert_eq!(
        value["url"],
        format!("https://api.flowleap.co/v1/tools/{tool}"),
        "{args:?}"
    );
    for (field, want) in expected {
        assert_eq!(&value["body"][field], want, "{args:?} field {field}");
    }
}

/// The EPO OPS read commands each map onto their single-document tool; the two
/// fulltext reads carry the language.
#[test]
fn ops_reads_map_onto_document_tools() {
    let doc = json!("EP1000000");
    for (subcommand, tool) in [
        ("biblio", "get_bibliography"),
        ("abstract", "get_abstract"),
        ("legal", "get_legal_status"),
    ] {
        assert_tool_call(
            &["ops", subcommand, "EP1000000"],
            tool,
            &[("patent_number", doc.clone())],
        );
    }

    // `ops family` is the INPADOC extended family (get_family), NOT the
    // simple-family equivalents tool, which keeps the get_patent_family name.
    assert_tool_call(
        &["ops", "family", "EP1000000"],
        "get_family",
        &[("patent_number", doc.clone())],
    );

    for (subcommand, tool) in [("claims", "get_claims"), ("description", "get_description")] {
        assert_tool_call(
            &["ops", subcommand, "EP1000000", "--lang", "de"],
            tool,
            &[("patent_number", doc.clone()), ("language", json!("de"))],
        );
    }
}

/// The EP designated contracting states are a filing's country coverage, and
/// they live in the INPADOC legal record rather than the bibliography document
/// — so `ops biblio` asks for them only when told to, and never by default.
#[test]
fn ops_biblio_requests_designated_states_only_on_demand() {
    let body = dry_run(&["ops", "biblio", "EP1000000"])["body"].clone();
    assert_eq!(body["patent_number"], json!("EP1000000"));
    assert!(
        body.get("include_designated_states").is_none(),
        "biblio asked for the extra legal read unprompted: {body}"
    );

    assert_tool_call(
        &["ops", "biblio", "EP1000000", "--designated-states"],
        "get_bibliography",
        &[
            ("patent_number", json!("EP1000000")),
            ("include_designated_states", json!(true)),
        ],
    );
}

#[test]
fn ops_search_uses_the_epo_leg_of_search_patents() {
    assert_tool_call(
        &[
            "ops",
            "search",
            "--cql",
            "ti=battery",
            "--start",
            "5",
            "--end",
            "20",
        ],
        "search_patents",
        &[
            ("provider", json!("epo_ops")),
            ("query", json!("ti=battery")),
            ("range", json!("5-20")),
        ],
    );
}

#[test]
fn uspto_lookups_map_onto_their_tools() {
    assert_tool_call(
        &["uspto", "grant", "11800000"],
        "get_us_grant",
        &[("patent_number", json!("11800000"))],
    );
    assert_tool_call(
        &["uspto", "application", "16123456"],
        "get_us_application",
        &[("application_number", json!("16123456"))],
    );
    assert_tool_call(
        &["uspto", "continuity", "16123456"],
        "get_continuity",
        &[("application_number", json!("16123456"))],
    );
}

#[test]
fn uspto_file_wrapper_projections_map_onto_their_tools() {
    let app = json!("14412875");
    for (subcommand, tool) in [
        ("transactions", "get_transactions"),
        ("assignments", "get_assignments"),
        ("foreign-priority", "get_foreign_priority"),
        ("adjustment", "get_patent_term_adjustment"),
        ("attorney", "get_attorney"),
    ] {
        assert_tool_call(
            &["uspto", subcommand, "14412875"],
            tool,
            &[("application_number", app.clone())],
        );
    }
}

/// Document filtering moved server-side: the flags become tool parameters,
/// normalized to the uppercase spellings the tool's enum takes.
#[test]
fn uspto_documents_filters_server_side() {
    assert_tool_call(
        &[
            "uspto",
            "documents",
            "14412875",
            "--code",
            "ctnf",
            "--direction",
            "outgoing",
        ],
        "get_application_documents",
        &[
            ("application_number", json!("14412875")),
            ("document_code", json!("CTNF")),
            ("direction", json!("OUTGOING")),
        ],
    );

    // Without filters neither parameter travels at all.
    let bare = dry_run(&["uspto", "documents", "14412875"]);
    assert!(bare["body"].get("document_code").is_none());
    assert!(bare["body"].get("direction").is_none());
}

#[test]
fn uspto_document_text_reads_through_the_facade() {
    assert_tool_call(
        &["uspto", "document-text", "14412875", "LAQYXZN3XBLUEX4"],
        "read_application_document",
        &[
            ("application_number", json!("14412875")),
            ("document_id", json!("LAQYXZN3XBLUEX4")),
        ],
    );
}

#[test]
fn citation_commands_map_onto_the_citation_tools() {
    assert_tool_call(
        &[
            "citation",
            "search",
            "16123456",
            "--category",
            "x",
            "--examiner-cited-only",
            "--from",
            "2020-01-01",
            "--to",
            "2024-12-31",
        ],
        "search_office_action_citations",
        &[
            ("application_number", json!("16123456")),
            ("category", json!("X")),
            ("examiner_cited_only", json!(true)),
            (
                "date_range",
                json!({ "from": "2020-01-01", "to": "2024-12-31" }),
            ),
        ],
    );

    // No date flags, no date_range key.
    let undated = dry_run(&["citation", "search", "16123456"]);
    assert!(undated["body"].get("date_range").is_none());

    assert_tool_call(
        &["citation", "forward", "US10123456"],
        "search_enriched_citations",
        &[("cited_document", json!("US10123456"))],
    );
    assert_tool_call(
        &["citation", "stats", "16123456"],
        "get_citation_stats",
        &[("application_number", json!("16123456"))],
    );
}

/// `citation novelty` is a recipe, not a capability: X-category plus
/// examiner-cited-only over the citation-search tool reproduces the retired
/// novelty route exactly.
#[test]
fn citation_novelty_is_a_recipe_over_citation_search() {
    assert_tool_call(
        &["citation", "novelty", "16123456", "--size", "25"],
        "search_office_action_citations",
        &[
            ("application_number", json!("16123456")),
            ("size", json!(25)),
            ("category", json!("X")),
            ("examiner_cited_only", json!(true)),
        ],
    );
}

#[test]
fn legal_commands_map_onto_the_reference_tools() {
    assert_tool_call(
        &[
            "legal",
            "search",
            "inventive step",
            "--jurisdiction",
            "epo",
            "--limit",
            "5",
        ],
        "reference_search",
        &[
            ("query", json!("inventive step")),
            ("jurisdiction", json!("EPO")),
            ("limit", json!(5)),
            ("search_mode", json!("hybrid")),
        ],
    );
    assert_tool_call(&["legal", "jurisdictions"], "get_legal_jurisdictions", &[]);
}

#[test]
fn literature_commands_map_onto_their_search_tools() {
    assert_tool_call(
        &[
            "academic",
            "search",
            "solid state battery",
            "--limit",
            "5",
            "--source",
            "scholar",
            "--from-year",
            "2020",
        ],
        "search_academic",
        &[
            ("query", json!("solid state battery")),
            ("max_results", json!(5)),
            ("sources", json!(["semantic-scholar"])),
            ("filter", json!({ "from_year": 2020 })),
        ],
    );

    assert_tool_call(
        &[
            "npl",
            "perovskite",
            "--limit",
            "5",
            "--open-access",
            "--type",
            "preprint",
        ],
        "search_npl",
        &[
            ("query", json!("perovskite")),
            ("limit", json!(5)),
            ("filter", json!({ "open_access": true, "type": "preprint" })),
        ],
    );
}

/// The retired subcommands are gone from the surface, so a stale invocation is
/// a local usage error instead of a request to an endpoint that answers 410.
/// `patstat portfolio` / `docs` / `query` are ergonomic verbs over the
/// PATSTAT tools (#95): snake_case input, `--retry-of` as `retry_of`, the
/// no-flag docs run asking for the full docs (`compact: false`), and the
/// portfolio asking for every office (`offices: "all"`, #107) unless
/// `--offices top` asks for the compact form MCP clients get.
#[test]
fn patstat_commands_map_onto_the_patstat_tools() {
    let sql = "SELECT office, COUNT(*) AS n FROM flowleap.applications GROUP BY office";
    let cases: [(&[&str], &str, Value); 7] = [
        (
            &[
                "patstat",
                "query",
                sql,
                "--question",
                "filings by office",
                "--retry-of",
                "patstat_sql_invalid",
            ],
            "patstat_query",
            json!({ "sql": sql, "question": "filings by office", "retry_of": "patstat_sql_invalid" }),
        ),
        (
            &[
                "patstat",
                "portfolio",
                "Siemens",
                "--from-year",
                "2020",
                "--to-year",
                "2024",
            ],
            "patstat_portfolio",
            json!({ "applicant": "Siemens", "from_year": 2020, "to_year": 2024, "offices": "all" }),
        ),
        (
            &["patstat", "portfolio", "Siemens", "--offices", "top"],
            "patstat_portfolio",
            json!({ "applicant": "Siemens", "offices": "top" }),
        ),
        (
            &["patstat", "docs", "--section", "examples"],
            "patstat_docs",
            json!({ "section": "examples" }),
        ),
        (
            &["patstat", "docs"],
            "patstat_docs",
            json!({ "compact": false }),
        ),
        (
            &[
                "patstat",
                "docs",
                "--section",
                "semantic-model",
                "--part",
                "index",
            ],
            "patstat_docs",
            json!({ "section": "semantic-model", "part": "index" }),
        ),
        (
            &[
                "patstat",
                "docs",
                "--section",
                "semantic-model",
                "--view",
                "applications",
            ],
            "patstat_docs",
            json!({ "section": "semantic-model", "view": "applications" }),
        ),
    ];

    for (args, tool, body) in cases {
        let value = dry_run(args);
        assert_eq!(value["method"], "POST", "{args:?}");
        assert_eq!(
            value["url"],
            format!("https://api.flowleap.co/v1/tools/{tool}"),
            "{args:?}"
        );
        assert_eq!(value["body"], body, "{args:?}");
    }
}

/// Every `patstat graph` verb — the six relays and the two native entry
/// ramps — is a POST to its `patstat_<verb>` tool (#96), with the flags
/// mapped to snake_case input. Free text and a number with a space or a slash
/// travel as one JSON string, and absent flags are left out so the backend
/// defaults apply.
#[test]
fn patstat_graph_verbs_map_onto_the_graph_tools() {
    let cases: [(&[&str], &str, Value); 11] = [
        (
            &["resolve", "EP3477840"],
            "patstat_resolve",
            json!({ "q": "EP3477840" }),
        ),
        (
            &["resolve", "Kia Motors & Co"],
            "patstat_resolve",
            json!({ "q": "Kia Motors & Co" }),
        ),
        (
            &["cpc", "solid state battery"],
            "patstat_cpc",
            json!({ "q": "solid state battery" }),
        ),
        (
            &["patent", "US5960411"],
            "patstat_patent",
            json!({ "number": "US5960411" }),
        ),
        (
            &["patent", "EP 3477840/A1"],
            "patstat_patent",
            json!({ "number": "EP 3477840/A1" }),
        ),
        (
            &["applicant", "30138991"],
            "patstat_applicant",
            json!({ "psn_id": 30138991 }),
        ),
        (
            &["technology", "H01M10/0562"],
            "patstat_technology",
            json!({ "cpc": "H01M10/0562" }),
        ),
        (
            &["neighborhood", "EP3477840"],
            "patstat_neighborhood",
            json!({ "node": "EP3477840" }),
        ),
        (
            &[
                "neighborhood",
                "EP3477840",
                "--depth",
                "2",
                "--edge-types",
                "cites, cited_by",
                "--token-budget",
                "500",
            ],
            "patstat_neighborhood",
            json!({
                "node": "EP3477840",
                "depth": 2,
                "edge_types": ["cites", "cited_by"],
                "token_budget": 500,
            }),
        ),
        (
            &[
                "path",
                "EP3477840",
                "US5960411",
                "--max-hops",
                "3",
                "--token-budget",
                "900",
            ],
            "patstat_path",
            json!({ "a": "EP3477840", "b": "US5960411", "max_hops": 3, "token_budget": 900 }),
        ),
        (
            &["explain", "pat:56123456", "--token-budget", "4000"],
            "patstat_explain",
            json!({ "node": "pat:56123456", "token_budget": 4000 }),
        ),
    ];

    for (args, tool, body) in cases {
        let mut argv = vec!["patstat", "graph"];
        argv.extend_from_slice(args);
        let value = dry_run(&argv);
        assert_eq!(value["method"], "POST", "{args:?}");
        assert_eq!(
            value["url"],
            format!("https://api.flowleap.co/v1/tools/{tool}"),
            "{args:?}"
        );
        assert_eq!(value["body"], body, "{args:?}");
    }
}

#[test]
fn retired_subcommands_are_no_longer_offered() {
    for args in [
        ["legal", "stats"].as_slice(),
        ["legal", "docs"].as_slice(),
        ["uspto", "associated-documents", "14412875"].as_slice(),
        ["health", "cache"].as_slice(),
        ["health", "redis"].as_slice(),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
            .args(args)
            .arg("--dry-run")
            .output()
            .expect("run retired subcommand");
        assert!(
            !output.status.success(),
            "{args:?} must no longer parse: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}
