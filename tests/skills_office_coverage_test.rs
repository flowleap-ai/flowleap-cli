//! Office coverage of the text tools in the skills (#151, #152; backend ADR
//! 0024). The backend serves claims for EP and WO (EPO OPS) and US
//! (BigQuery), description for EP and WO, and no text for every other
//! office. The `flowleap-shared` skill carries that fact ONCE, with the
//! family-member workaround (`get_family`, then the member's text), its two
//! limits and the measured share per office. These tests read what an agent
//! reads, the skill files, and compare the facts against the backend's
//! `server_info` `officeCoverage` block, vendored verbatim in
//! `tests/support/office-coverage.json`. Refresh it when the backend changes:
//!
//!   flowleap --json tools run server_info | jq .officeCoverage > tests/support/office-coverage.json
//!
//! The US `get_claims` row carries a per-server `configured` flag (the
//! BigQuery full-text flag of the server that answered); the tests ignore
//! it. Fetched 2026-10-15 from backend main `officeCoverageBlock({
//! bigqueryConfigured: true })`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SECTION_HEADING: &str =
    "## Office coverage of the text tools and the family-member workaround";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e))
}

fn skill(name: &str) -> String {
    read(&root().join("skills").join(name).join("SKILL.md"))
}

/// The vendored `officeCoverage` block, the fields the skills restate.
struct OfficeCoverage {
    no_text_source: Vec<String>,
    workaround_tool: String,
    shares: BTreeMap<String, u64>,
    data_edition: String,
}

fn office_coverage() -> OfficeCoverage {
    let value: serde_json::Value =
        serde_json::from_str(&read(&root().join("tests/support/office-coverage.json")))
            .expect("tests/support/office-coverage.json is JSON");
    let workaround = &value["workaround"];
    OfficeCoverage {
        no_text_source: value["noTextSource"]
            .as_array()
            .expect("noTextSource is an array")
            .iter()
            .map(|o| o.as_str().expect("office").to_string())
            .collect(),
        workaround_tool: workaround["tool"].as_str().expect("workaround.tool").to_string(),
        shares: workaround["shares"]
            .as_object()
            .expect("workaround.shares")
            .iter()
            .map(|(office, share)| (office.clone(), share.as_u64().expect("share percent")))
            .collect(),
        data_edition: workaround["dataEdition"]
            .as_str()
            .expect("workaround.dataEdition")
            .to_string(),
    }
}

/// The shared section's text: from its heading to the next `## ` heading.
fn shared_section() -> String {
    let text = skill("flowleap-shared");
    let count = text.matches(SECTION_HEADING).count();
    assert_eq!(count, 1, "flowleap-shared: expected the section once, found {count}");
    let start = text.find(SECTION_HEADING).unwrap();
    let body = &text[start + SECTION_HEADING.len()..];
    let end = body.find("\n## ").unwrap_or(body.len());
    body[..end].to_string()
}

#[test]
fn shared_section_exists_once_and_names_the_workaround_tool() {
    let section = shared_section();
    let tool = office_coverage().workaround_tool;
    assert!(
        section.contains(&format!("`{tool}`")),
        "flowleap-shared: the workaround section does not name `{tool}`"
    );
}

// T2 (#153) adds here: the pointer sentences in the tools skill, the OPS
// skill and the four recipes; the sweep for a text tool under a "worldwide"
// or "any publication number" heading; the example-office check for `ops
// claims` / `ops description` numbers and `--lang`; the OPS "holding gap"
// wording; the freedom-to-operate phrases.
