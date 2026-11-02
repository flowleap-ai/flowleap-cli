//! Every skill's frontmatter is valid YAML. A YAML reader, such as the
//! harness skill reader, skips a skill whose frontmatter does not parse, so
//! no agent of that host gets the skill. The CLI's own readers take the
//! `description:` line as text and cannot catch this.

mod support;

use support::skills::skills;
use yaml_rust2::{Yaml, YamlLoader};

#[test]
fn every_skill_frontmatter_parses_as_yaml() {
    let mut errors = Vec::new();
    for (name, text) in skills() {
        let Some(block) = text
            .strip_prefix("---\n")
            .and_then(|rest| rest.split_once("\n---\n"))
            .map(|(block, _)| block)
        else {
            errors.push(format!("{name}: no frontmatter block"));
            continue;
        };
        let doc = match YamlLoader::load_from_str(block) {
            Ok(mut docs) if docs.len() == 1 => docs.remove(0),
            Ok(docs) => {
                errors.push(format!("{name}: {} YAML documents", docs.len()));
                continue;
            }
            Err(e) => {
                errors.push(format!("{name}: {e}"));
                continue;
            }
        };
        if doc["name"].as_str() != Some(name.as_str()) {
            errors.push(format!("{name}: `name` is not the folder name"));
        }
        if doc["description"]
            .as_str()
            .is_none_or(|d| d.trim().is_empty())
        {
            errors.push(format!("{name}: `description` is not a non-empty string"));
        }
        for key in ["user-invocable", "disable-model-invocation"] {
            if !matches!(doc[key], Yaml::BadValue | Yaml::Boolean(_)) {
                errors.push(format!("{name}: `{key}` is not a boolean"));
            }
        }
    }
    assert!(errors.is_empty(), "invalid skill frontmatter: {errors:#?}");
}
