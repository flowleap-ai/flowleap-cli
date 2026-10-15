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
        workaround_tool: workaround["tool"]
            .as_str()
            .expect("workaround.tool")
            .to_string(),
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
    assert_eq!(
        count, 1,
        "flowleap-shared: expected the section once, found {count}"
    );
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

/// The share table: one row per office of `noTextSource`, the share equal to
/// the vendored `workaround.shares` value, and the data edition it was
/// measured on.
#[test]
fn shared_section_lists_every_office_without_text_with_its_vendored_share() {
    let section = shared_section();
    let coverage = office_coverage();
    assert_eq!(
        coverage.no_text_source.len(),
        coverage.shares.len(),
        "vendored file: every office without text has a share"
    );
    let mut wrong = Vec::new();
    for office in &coverage.no_text_source {
        let share = coverage.shares[office];
        let row = section
            .lines()
            .find(|line| line.trim_start().starts_with(&format!("| {office} ")));
        match row {
            Some(row) if row.contains(&format!("| {share}%")) => {}
            Some(row) => wrong.push(format!("{office}: expected {share}%, row {row:?}")),
            None => wrong.push(format!("{office}: no table row")),
        }
    }
    assert!(wrong.is_empty(), "flowleap-shared share table: {wrong:?}");
    assert!(
        section.contains(&coverage.data_edition),
        "flowleap-shared: the share table does not name the data edition {:?}",
        coverage.data_edition
    );
}

/// The two limits, a phrase from each: a member's claims are not the office's
/// claims, and the member's text must itself be served.
#[test]
fn shared_section_states_both_limits_of_the_workaround() {
    let section = shared_section();
    for phrase in ["not the office's claims", "must itself be served"] {
        assert!(
            section.contains(phrase),
            "flowleap-shared: the workaround section lacks the limit phrase {phrase:?}"
        );
    }
}

/// The served failure: `FULLTEXT_UNAVAILABLE` with reason `no_source` carries
/// the `workaround` field under `error.details`; reason `not_configured` is a
/// server configuration gap and carries no workaround.
#[test]
fn shared_section_explains_the_fulltext_unavailable_reasons() {
    let section = shared_section();
    for phrase in [
        "`FULLTEXT_UNAVAILABLE`",
        "`no_source`",
        "`error.details`",
        "`workaround`",
        "`not_configured`",
    ] {
        assert!(
            section.contains(phrase),
            "flowleap-shared: the workaround section lacks {phrase}"
        );
    }
}

/// CONTEXT.md defines the two terms the skills use, mirrored from the backend:
/// office coverage (per tool) and office depth (per office).
#[test]
fn context_md_defines_office_coverage_and_office_depth() {
    let text = read(&root().join("CONTEXT.md"));
    assert!(
        text.contains("**Office coverage** vs **office depth**"),
        "CONTEXT.md: no **Office coverage** vs **office depth** entry"
    );
    assert!(
        text.contains("Office coverage") && text.contains("office depth"),
        "CONTEXT.md: the two terms are not both defined"
    );
    assert!(
        text.contains("three senses"),
        "CONTEXT.md: the conventions do not say that \"coverage\" carries three senses"
    );
}

/// Every skill file, by skill name.
fn skills() -> Vec<(String, String)> {
    let dir = root().join("skills");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {}", dir.display(), e))
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().join("SKILL.md").is_file())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no skills under {}", dir.display());
    names
        .into_iter()
        .map(|name| {
            let text = skill(&name);
            (name, text)
        })
        .collect()
}

const TEXT_TOOLS: [&str; 5] = [
    "get_claims",
    "get_description",
    "get_fulltext",
    "ops claims",
    "ops description",
];

/// The lead sentence of a block: its first line up to the first colon or
/// full stop.
fn lead_sentence(block: &str) -> String {
    let first = block.lines().next().unwrap_or("");
    let end = first
        .find(':')
        .into_iter()
        .chain(first.find(". "))
        .min()
        .unwrap_or(first.len());
    first[..end].to_lowercase()
}

/// A skill must not list a text tool (`get_claims`, `get_description`,
/// `get_fulltext`, `ops claims`, `ops description`) in a paragraph or list
/// whose heading or lead sentence promises "worldwide" or "any publication
/// number": the text tools answer for EP, WO and US only (ADR 0024).
#[test]
fn no_skill_lists_a_text_tool_under_a_worldwide_lead() {
    const PROMISES: [&str; 2] = ["worldwide", "any publication number"];
    let mut offenders = Vec::new();
    for (name, text) in skills() {
        let mut heading = String::new();
        let mut block: Vec<&str> = Vec::new();
        let mut check = |heading: &str, block: &mut Vec<&str>| {
            if block.is_empty() {
                return;
            }
            let body = block.join("\n");
            let lead = lead_sentence(&body);
            let promised = PROMISES
                .iter()
                .any(|p| heading.contains(p) || lead.contains(p));
            if promised {
                for tool in TEXT_TOOLS {
                    if body.contains(tool) {
                        offenders.push(format!(
                            "{name}: `{tool}` under heading {heading:?}, lead {lead:?}"
                        ));
                    }
                }
            }
            block.clear();
        };
        for line in text.lines() {
            if line.starts_with('#') {
                check(&heading, &mut block);
                heading = line.to_lowercase();
            } else if line.trim().is_empty() {
                check(&heading, &mut block);
            } else {
                block.push(line);
            }
        }
        check(&heading, &mut block);
    }
    assert!(
        offenders.is_empty(),
        "text tools listed under a worldwide / any-publication-number lead: {offenders:#?}"
    );
}

/// The skills that read claims point at the shared block, never copy it: one
/// pointer phrase each, in the tools skill, the OPS skill and the four
/// recipes that read claims.
#[test]
fn text_reading_skills_point_at_the_family_member_workaround() {
    const POINTER: &str = "family-member workaround in `flowleap-shared`";
    let missing: Vec<&str> = [
        "flowleap-tools",
        "flowleap-ops",
        "recipe-freedom-to-operate",
        "recipe-prior-art-search",
        "recipe-claim-analysis",
        "recipe-invalidity-analysis",
    ]
    .into_iter()
    .filter(|name| !skill(name).contains(POINTER))
    .collect();
    assert!(
        missing.is_empty(),
        "skills without the pointer {POINTER:?}: {missing:?}"
    );
}
