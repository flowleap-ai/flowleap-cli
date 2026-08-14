//! Every data command runs on the `/v1/tools` facade (backend PRD 0013 Phase
//! 2). Dry-run mode surfaces the exact request a command would send, so these
//! assert the tool name (URL) and the tool-input JSON shape without a live
//! backend — the same seam `facade_test.rs` uses for the ergonomic verbs.
//!
//! PATSTAT commands and `keys test`/`keys set` are the named non-facade
//! exceptions and are deliberately absent here.

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
