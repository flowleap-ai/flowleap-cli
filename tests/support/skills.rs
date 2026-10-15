//! Shared readers for the tests that read what an agent reads: the `skills/`
//! tree, `CONTEXT.md` and the vendored lists under `tests/support/`.
//!
//! Entry points:
//! - [`root`] — the crate root (`CARGO_MANIFEST_DIR`).
//! - [`read`] — a file as a string, panicking with its path.
//! - [`skill`] — one skill's `SKILL.md` by skill name.
//! - [`skills`] — every skill, `(name, text)`, sorted by name.

use std::path::{Path, PathBuf};

/// The crate root.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Read `path` as a string; panic with the path when it does not read.
pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e))
}

/// The text of `skills/<name>/SKILL.md`.
pub fn skill(name: &str) -> String {
    read(&root().join("skills").join(name).join("SKILL.md"))
}

/// Every directory under `skills/` that holds a `SKILL.md`: `(name, text)`,
/// sorted by name.
pub fn skills() -> Vec<(String, String)> {
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
