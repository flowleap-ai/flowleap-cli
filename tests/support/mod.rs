//! Shared mock-HTTP test harness.
//!
//! Spins up a `wiremock` server and drives the built `flowleap` binary against
//! it in an isolated `HOME`, so tests exercise the real client (timeouts,
//! User-Agent, retry) end-to-end without touching a live backend or the user's
//! credentials. This is the foundation the exit-code (#20) and hint (#21)
//! slices build on — keep the entry points stable.
//!
//! Entry points:
//! - [`run_cli`] — run `flowleap <args>` against a base URL with extra env.
//! - [`stdout_json`] — parse a run's stdout as JSON.
//! - [`run_mcp`] / [`run_mcp_with_stderr`] — drive `flowleap mcp` over stdio
//!   with JSON-RPC lines built by [`frame`] / [`initialize_frame`].
//! - Callers create the server themselves via `wiremock::MockServer::start()`
//!   and pass `server.uri()` as the base URL.
#![allow(dead_code)]

use std::io::Write;
use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};

/// Run the built `flowleap` binary against `base_url` (typically a
/// `wiremock::MockServer::uri()`), with `envs` layered on top of a clean
/// environment: an isolated temp `HOME`, no ambient credentials, and the update
/// check disabled. The blocking subprocess runs off the async runtime so the
/// mock server keeps serving while it executes.
pub async fn run_cli(base_url: &str, envs: &[(&str, &str)], args: &[&str]) -> Output {
    let base_url = base_url.to_string();
    let envs: Vec<(String, String)> = envs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();

    tokio::task::spawn_blocking(move || {
        let home = tempfile::tempdir().expect("create temp home");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_flowleap"));
        cmd.env("HOME", home.path())
            .env("XDG_CONFIG_HOME", home.path().join(".config"))
            .env("FLOWLEAP_BASE_URL", &base_url)
            .env("FLOWLEAP_NO_UPDATE_CHECK", "1")
            .env_remove("FLOWLEAP_TOKEN")
            .env_remove("FLOWLEAP_API_KEY")
            .env_remove("FLOWLEAP_EPO_KEY")
            .env_remove("FLOWLEAP_EPO_SECRET")
            .env_remove("FLOWLEAP_USPTO_KEY")
            .env_remove("FLOWLEAP_ASSUME_YES");
        for (key, value) in &envs {
            cmd.env(key, value);
        }
        cmd.args(&args);
        cmd.output().expect("run flowleap binary")
    })
    .await
    .expect("join flowleap subprocess")
}

/// The facade success envelope (`{ success, tool, data, executionTimeMs }`)
/// around one tool's `data` — what a mocked `POST /v1/tools/<tool>` answers.
pub fn tool_ok(tool: &str, data: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "success": true, "tool": tool, "data": data, "executionTimeMs": 12 })
}

/// Parse a run's stdout as JSON. Panics with the raw stdout on failure.
pub fn stdout_json(output: &Output) -> serde_json::Value {
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is utf8");
    serde_json::from_str(&stdout).unwrap_or_else(|_| panic!("stdout was not json: {stdout}"))
}

/// Run `flowleap mcp` against `base_url` in an isolated environment (temp
/// `HOME`, no ambient credentials, update check disabled), feed it `lines` on
/// stdin, and return the parsed JSON-RPC response frames from stdout.
pub async fn run_mcp(base_url: &str, envs: &[(&str, &str)], lines: &[String]) -> Vec<Value> {
    run_mcp_with_stderr(base_url, envs, &[], lines).await.0
}

/// [`run_mcp`] with extra global flags placed before `mcp` (e.g.
/// `--dry-run`), also returning everything the server wrote to stderr.
pub async fn run_mcp_with_stderr(
    base_url: &str,
    envs: &[(&str, &str)],
    flags: &[&str],
    lines: &[String],
) -> (Vec<Value>, String) {
    let flags: Vec<String> = flags.iter().map(|flag| flag.to_string()).collect();
    let base_url = base_url.to_string();
    let envs: Vec<(String, String)> = envs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    let input = format!("{}\n", lines.join("\n"));

    tokio::task::spawn_blocking(move || {
        let home = tempfile::tempdir().expect("create temp home");
        let mut child = Command::new(env!("CARGO_BIN_EXE_flowleap"))
            .env("HOME", home.path())
            .env("XDG_CONFIG_HOME", home.path().join(".config"))
            .env_remove("FLOWLEAP_BASE_URL")
            .env("FLOWLEAP_BASE_URL", &base_url)
            .env("FLOWLEAP_NO_UPDATE_CHECK", "1")
            .env_remove("FLOWLEAP_TOKEN")
            .env_remove("FLOWLEAP_API_KEY")
            .envs(envs)
            .args(&flags)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn flowleap mcp");

        let mut stdin = child.stdin.take().expect("child stdin");
        stdin.write_all(input.as_bytes()).expect("write stdin");
        drop(stdin); // EOF ends the server loop

        let output = child.wait_with_output().expect("wait for flowleap mcp");
        assert!(
            output.status.success(),
            "mcp exited nonzero: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let frames = String::from_utf8(output.stdout)
            .expect("stdout is utf8")
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str(line)
                    .unwrap_or_else(|_| panic!("stdout line was not a JSON frame: {line}"))
            })
            .collect();
        let stderr = String::from_utf8(output.stderr).expect("stderr is utf8");
        (frames, stderr)
    })
    .await
    .expect("join flowleap mcp subprocess")
}

pub fn frame(value: Value) -> String {
    value.to_string()
}

pub fn initialize_frame(id: u64, protocol_version: &str) -> String {
    frame(json!({
        "jsonrpc": "2.0", "id": id, "method": "initialize",
        "params": {
            "protocolVersion": protocol_version,
            "capabilities": {},
            "clientInfo": { "name": "test-harness", "version": "0.0.0" },
        },
    }))
}
