//! The FlowLeap Patent AI app ships only the methods: the `recipe-*` skills.
//! The app has no `flowleap` command and none of the manual or persona
//! skills, so a recipe names tools, never a command, the CLI, or a skill the
//! app does not ship. Two paragraphs, pinned below, tell the agent how tool
//! names and missing keys work in every host.

mod support;

use std::path::Path;

use support::skills::{read, root, skills};

const TOOLS_PARAGRAPH: &str = "**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.";

const KEYS_PARAGRAPH: &str = "**Missing keys.** When a tool answers that an office's key is not configured or that the office rejected the stored key, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as \"no results\"), never fill it with data from the web, and at the end tell the user which key is missing or rejected. On the desktop app the user adds it in Settings > Patent data connectors; on the firm OS the firm administrator sets the keys, so tell the attorney to ask the administrator; in another client, relay the page that the tool's answer names. Never ask for a key value in the chat.";

/// Every Markdown file of one skill folder (SKILL.md and references/), as
/// `(relative path, text)`.
fn markdown_files(dir: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).expect("read skill folder") {
            let path = entry.expect("skill folder entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "md") {
                let rel = path.strip_prefix(root()).unwrap_or(&path);
                files.push((rel.display().to_string(), read(&path)));
            }
        }
    }
    files
}

fn methods() -> Vec<String> {
    let names: Vec<String> = skills()
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| name.starts_with("recipe-"))
        .collect();
    assert_eq!(names.len(), 15, "expected the 15 methods, found {names:?}");
    names
}

/// The skills that the app does not ship: every skill that is not a method.
fn non_methods() -> Vec<String> {
    skills()
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| !name.starts_with("recipe-"))
        .collect()
}

#[test]
fn methods_name_no_command_cli_or_unshipped_skill() {
    let unshipped = non_methods();
    let mut offenders = Vec::new();
    for name in methods() {
        for (path, text) in markdown_files(&root().join("skills").join(&name)) {
            for (i, line) in text.lines().enumerate() {
                let lower = line.to_lowercase();
                let names_command = line.contains("`flowleap ")
                    || line.trim_start().starts_with("flowleap ")
                    || line.contains("$ flowleap ");
                let names_cli = line
                    .split(|c: char| !c.is_ascii_alphanumeric())
                    .any(|word| word == "CLI");
                let names_skill = unshipped.iter().any(|skill| {
                    lower.contains(&format!("`{skill}`")) || lower.contains(&format!("\"{skill}\""))
                });
                if names_command || names_cli || names_skill {
                    offenders.push(format!("{path}:{}: {line}", i + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "methods must name tools, not commands, the CLI or unshipped skills:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn methods_carry_the_tools_and_missing_keys_paragraphs() {
    let mut missing = Vec::new();
    for name in methods() {
        let text = read(&root().join("skills").join(&name).join("SKILL.md"));
        for (label, paragraph) in [("Tools", TOOLS_PARAGRAPH), ("Missing keys", KEYS_PARAGRAPH)] {
            if !text.contains(paragraph) {
                missing.push(format!("{name}: {label}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "methods without a pinned paragraph: {missing:?}"
    );
}
