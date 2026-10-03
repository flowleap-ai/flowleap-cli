//! Connector-first guidance in the skills (#122). In a chat client with the
//! FlowLeap connector there is no shell, so the skills route the agent to
//! the tool of the same name. These tests keep that routing true:
//!
//! - every tool name in the `flowleap-shared` connector table and in the
//!   umbrella skill's one-call verb table exists in the registry, read from
//!   the vendored list in `tests/support/registry-tools.txt` (refresh it with
//!   the command in its header when the registry changes);
//! - every skill that invokes `flowleap …` commands, inline or in a fenced
//!   block, carries the one-line routing sentence.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const ROUTING_SENTENCE: &str = "In a chat client with the FlowLeap connector, call the tools named in the `flowleap-shared` connector table instead of these commands.";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e))
}

fn registry_tools() -> BTreeSet<String> {
    read(&root().join("tests/support/registry-tools.txt"))
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// The backticked identifiers in column `col` (0-based) of the first markdown
/// table after `heading` in `skill`'s SKILL.md.
fn table_tool_names(skill: &str, heading: &str, col: usize) -> Vec<String> {
    let text = read(&root().join("skills").join(skill).join("SKILL.md"));
    let start = text
        .find(heading)
        .unwrap_or_else(|| panic!("{skill}: heading {heading:?} not found"));
    let rows: Vec<&str> = text[start..]
        .lines()
        .skip_while(|line| !line.trim_start().starts_with('|'))
        .take_while(|line| line.trim_start().starts_with('|'))
        .skip(2) // header and separator rows
        .collect();
    assert!(!rows.is_empty(), "{skill}: no table rows after {heading:?}");

    let mut names = Vec::new();
    for row in rows {
        let cell = row.split('|').nth(col + 1).unwrap_or_default();
        for (i, part) in cell.split('`').enumerate() {
            // Odd parts sit between backticks. Every one goes to the registry
            // check, so a misspelling with a digit or uppercase is reported.
            if i % 2 == 1 {
                names.push(part.to_string());
            }
        }
    }
    names
}

fn assert_in_registry(skill: &str, names: &[String]) {
    let registry = registry_tools();
    let missing: Vec<&String> = names.iter().filter(|n| !registry.contains(*n)).collect();
    assert!(
        missing.is_empty(),
        "{skill}: tool names not in the registry: {missing:?}"
    );
}

/// True for a command line such as `flowleap --json ops claims EP1`: the
/// line matches `^\s*flowleap( --\S+)* [a-z]`.
fn invokes_flowleap(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix("flowleap ") else {
        return false;
    };
    let mut rest = rest;
    while let Some(flag) = rest.strip_prefix("--") {
        match flag.split_once(' ') {
            Some((name, tail)) if !name.is_empty() && !name.contains(char::is_whitespace) => {
                rest = tail
            }
            _ => return false,
        }
    }
    rest.starts_with(|c: char| c.is_ascii_lowercase())
}

#[test]
fn invokes_flowleap_matches_command_lines_only() {
    assert!(invokes_flowleap("flowleap ops claims EP1"));
    assert!(invokes_flowleap("  flowleap --json --dry-run summary EP1"));
    assert!(!invokes_flowleap("flowleap --version"));
    assert!(!invokes_flowleap("the flowleap ops command"));
    assert!(!invokes_flowleap("flowleap <command>"));
}

#[test]
fn shared_connector_table_names_only_registry_tools() {
    let names = table_tool_names(
        "flowleap-shared",
        "## Chat clients with the FlowLeap connector",
        1,
    );
    // The ticket's mapping lists 38 distinct tools; guard against a parser
    // that silently reads nothing.
    let distinct: BTreeSet<&String> = names.iter().collect();
    assert!(
        distinct.len() >= 38,
        "expected the full command-to-tool table, read {} tools",
        distinct.len()
    );
    assert_in_registry("flowleap-shared", &names);
}

#[test]
fn umbrella_verb_table_names_only_registry_tools() {
    let names = table_tool_names(
        "flowleap",
        "**In a chat client with the FlowLeap connector**",
        1,
    );
    assert!(names.len() >= 4, "expected the one-call verb table");
    assert_in_registry("flowleap", &names);
}

#[test]
fn every_command_invoking_skill_routes_chat_clients() {
    // flowleap-shared holds the table and the umbrella skill holds its own
    // routing paragraph; every other skill that invokes `flowleap …` carries
    // the routing sentence.
    let exempt = ["flowleap", "flowleap-shared"];
    let mut missing = Vec::new();
    let mut checked = 0;
    for entry in std::fs::read_dir(root().join("skills")).expect("read skills/") {
        let dir = entry.expect("dir entry").path();
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let skill_md = dir.join("SKILL.md");
        if exempt.contains(&name.as_str()) || !skill_md.exists() {
            continue;
        }
        let text = read(&skill_md);
        if !text.contains("`flowleap ") && !text.lines().any(invokes_flowleap) {
            continue;
        }
        checked += 1;
        if !text.contains(ROUTING_SENTENCE) {
            missing.push(name);
        }
    }
    assert!(checked > 0, "found no command-invoking skills");
    assert!(
        missing.is_empty(),
        "skills without the chat-client routing sentence: {missing:?}"
    );
}
