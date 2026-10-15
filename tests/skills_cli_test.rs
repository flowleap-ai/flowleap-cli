//! End-to-end tests for the multi-harness skills installer, driving the real
//! binary with HOME/XDG_CONFIG_HOME pointed at a temp directory so config
//! records never touch the developer's machine.

mod support;

use std::path::PathBuf;
use std::process::{Command, Output};

use support::skills::read;
use tempfile::TempDir;

const VERSION: &str = env!("CARGO_PKG_VERSION");

struct Sandbox {
    home: TempDir,
    cwd: TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            home: tempfile::tempdir().expect("temp home"),
            cwd: tempfile::tempdir().expect("temp cwd"),
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_flowleap"))
            .current_dir(self.cwd.path())
            .env("HOME", self.home.path())
            .env("XDG_CONFIG_HOME", self.home.path().join(".config"))
            .env_remove("FLOWLEAP_BASE_URL")
            .env("XDG_CONFIG_HOME", self.home.path().join("xdg"))
            .env("FLOWLEAP_NO_UPDATE_CHECK", "1")
            .env_remove("FLOWLEAP_API_KEY")
            .env_remove("FLOWLEAP_TOKEN")
            .args(args)
            .output()
            .expect("run flowleap")
    }

    fn run_ok(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "flowleap {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("stdout is utf8")
    }

    /// The config file the sandboxed binary writes (location differs by OS).
    fn config_path(&self) -> PathBuf {
        let candidates = [
            self.home
                .path()
                .join("Library/Application Support/flowleap/config.toml"),
            self.home.path().join("xdg/flowleap/config.toml"),
            self.home.path().join(".config/flowleap/config.toml"),
        ];
        candidates
            .iter()
            .find(|path| path.exists())
            .cloned()
            .unwrap_or_else(|| panic!("no config.toml under {:?}", self.home.path()))
    }

    fn cwd_file(&self, name: &str) -> PathBuf {
        self.cwd.path().join(name)
    }
}

#[test]
fn codex_target_writes_marked_agents_block_and_records_install() {
    let sandbox = Sandbox::new();
    // Pre-existing hand-written AGENTS.md content must survive the install.
    std::fs::write(sandbox.cwd_file("AGENTS.md"), "# My Rules\n\nKeep me.\n").unwrap();

    let stdout = sandbox.run_ok(&["skills", "install", "--target", "codex", "--json"]);
    assert!(stdout.contains("\"ok\": true"), "stdout: {}", stdout);

    let agents = read(&sandbox.cwd_file("AGENTS.md"));
    assert!(agents.contains("# My Rules"));
    assert!(agents.contains("Keep me."));
    assert!(agents.contains("<!-- BEGIN FLOWLEAP AGENT RULES"));
    assert!(agents.contains(&format!("flowleap-cli v{}", VERSION)));
    assert!(agents.contains("<!-- END FLOWLEAP AGENT RULES -->"));

    let config = read(&sandbox.config_path());
    assert!(config.contains("[[skill_installs]]"), "config: {}", config);
    assert!(config.contains("target = \"codex\""));
    assert!(config.contains(&format!("version = \"{}\"", VERSION)));

    // Re-installing replaces the block instead of appending a second one.
    sandbox.run_ok(&["skills", "install", "--target", "codex", "--json"]);
    let agents = read(&sandbox.cwd_file("AGENTS.md"));
    assert_eq!(agents.matches("<!-- BEGIN FLOWLEAP AGENT RULES").count(), 1);
}

#[test]
fn cursor_and_gemini_targets_emit_their_autoloaded_files() {
    let sandbox = Sandbox::new();

    sandbox.run_ok(&["skills", "install", "--target", "cursor", "--json"]);
    let mdc = read(&sandbox.cwd_file(".cursor/rules/flowleap.mdc"));
    assert!(mdc.starts_with("---\n"));
    assert!(mdc.contains("alwaysApply: true"));
    assert!(mdc.contains(&format!("flowleap-cli v{}", VERSION)));

    sandbox.run_ok(&["skills", "install", "--target", "gemini", "--json"]);
    let gemini = read(&sandbox.cwd_file("GEMINI.md"));
    assert!(gemini.contains("<!-- BEGIN FLOWLEAP AGENT RULES"));
    assert!(gemini.contains(&format!("flowleap-cli v{}", VERSION)));

    let config = read(&sandbox.config_path());
    assert!(config.contains("target = \"cursor\""));
    assert!(config.contains("target = \"gemini\""));
}

#[test]
fn dir_target_copies_skills_and_stamps_version() {
    let sandbox = Sandbox::new();
    let dir = sandbox.cwd_file("agent-skills");

    sandbox.run_ok(&[
        "skills",
        "install",
        "--dir",
        dir.to_str().unwrap(),
        "--json",
        "flowleap",
    ]);

    assert!(dir.join("flowleap/SKILL.md").is_file());
    let manifest = read(&dir.join(".flowleap-skills-manifest.json"));
    assert!(
        manifest.contains(&format!("\"cliVersion\": \"{}\"", VERSION)),
        "manifest: {}",
        manifest
    );
    assert!(manifest.contains("\"flowleap\""), "manifest: {}", manifest);
    let config = read(&sandbox.config_path());
    assert!(config.contains("target = \"dir\""));
    assert!(config.contains("skills = [\"flowleap\"]"));
}

#[test]
fn reinstall_protects_user_edited_skills() {
    let sandbox = Sandbox::new();
    let dir = sandbox.cwd_file("agent-skills");
    let dir_arg = dir.to_str().unwrap();
    sandbox.run_ok(&["skills", "install", "--dir", dir_arg, "--json", "flowleap"]);

    let skill_md = dir.join("flowleap/SKILL.md");
    std::fs::write(&skill_md, "user edits\n").unwrap();

    // Re-running install must not clobber the hand edit.
    let stdout = sandbox.run_ok(&["skills", "install", "--dir", dir_arg, "--json", "flowleap"]);
    assert!(stdout.contains("\"skipped\""), "stdout: {}", stdout);
    let skipped_flowleap = stdout.contains("flowleap") && stdout.contains("--force");
    assert!(skipped_flowleap, "expected a --force hint, got: {}", stdout);
    assert_eq!(read(&skill_md), "user edits\n");

    // --force overwrites the edit with the bundled content.
    sandbox.run_ok(&[
        "skills", "install", "--dir", dir_arg, "--force", "--json", "flowleap",
    ]);
    assert_ne!(read(&skill_md), "user edits\n");
    assert!(read(&skill_md).contains("FlowLeap"));
}

#[test]
fn doctor_reports_recorded_skill_installs() {
    let sandbox = Sandbox::new();
    let dir = sandbox.cwd_file("agent-skills");
    sandbox.run_ok(&[
        "skills",
        "install",
        "--dir",
        dir.to_str().unwrap(),
        "--json",
        "flowleap",
    ]);

    // doctor pings the backend; point it at a closed port so it fails fast and
    // we only exercise the offline-computed skills section.
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .current_dir(sandbox.cwd.path())
        .env("HOME", sandbox.home.path())
        .env("XDG_CONFIG_HOME", sandbox.home.path().join("xdg"))
        .env("FLOWLEAP_BASE_URL", "http://127.0.0.1:9")
        .env("FLOWLEAP_NO_UPDATE_CHECK", "1")
        .args(["--json", "doctor"])
        .output()
        .expect("run doctor");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"skills\""), "doctor json: {}", stdout);
    assert!(
        stdout.contains("\"recorded\": 1"),
        "doctor json: {}",
        stdout
    );
}

#[test]
fn update_rerenders_recorded_targets_and_bumps_stamp() {
    let sandbox = Sandbox::new();
    sandbox.run_ok(&["skills", "install", "--target", "codex", "--json"]);

    // Simulate an older install: age the recorded stamp and delete the file.
    let config_path = sandbox.config_path();
    let aged =
        read(&config_path).replace(&format!("version = \"{}\"", VERSION), "version = \"0.0.1\"");
    std::fs::write(&config_path, aged).unwrap();
    std::fs::remove_file(sandbox.cwd_file("AGENTS.md")).unwrap();

    let stdout = sandbox.run_ok(&["skills", "update", "--json"]);
    assert!(stdout.contains("\"ok\": true"), "stdout: {}", stdout);

    let agents = read(&sandbox.cwd_file("AGENTS.md"));
    assert!(agents.contains(&format!("flowleap-cli v{}", VERSION)));
    let config = read(&config_path);
    assert!(config.contains(&format!("version = \"{}\"", VERSION)));
    assert!(!config.contains("version = \"0.0.1\""));
}

#[test]
fn update_without_records_is_a_friendly_noop() {
    let sandbox = Sandbox::new();
    let stdout = sandbox.run_ok(&["skills", "update", "--json"]);
    assert!(stdout.contains("\"updated\": []"), "stdout: {}", stdout);
}

#[test]
fn claude_default_behavior_unchanged() {
    let sandbox = Sandbox::new();
    // `--project` keeps working as the claude-project shorthand.
    let stdout = sandbox.run_ok(&["skills", "install", "--project", "--json", "flowleap"]);
    assert!(stdout.contains("\"ok\": true"), "stdout: {}", stdout);
    assert!(sandbox
        .cwd_file(".claude/skills/flowleap/SKILL.md")
        .is_file());
    let config = read(&sandbox.config_path());
    assert!(config.contains("target = \"claude-project\""));
}
