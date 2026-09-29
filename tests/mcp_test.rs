//! Protocol-level tests for `flowleap mcp`: spawn the built binary with piped
//! stdin/stdout against a wiremock backend (same env isolation as
//! `tests/support/mod.rs`) and drive JSON-RPC frames end-to-end.
//!
//! Every stdout line is parsed as a JSON frame — any stray output fails the
//! test, which is exactly the "nothing but protocol frames on stdout" rule.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Run `flowleap mcp` against `base_url` in an isolated environment (temp
/// `HOME`, no ambient credentials, update check disabled), feed it `lines` on
/// stdin, and return the parsed JSON-RPC response frames from stdout.
async fn run_mcp(base_url: &str, envs: &[(&str, &str)], lines: &[String]) -> Vec<Value> {
    run_mcp_with_stderr(base_url, envs, &[], lines).await.0
}

/// [`run_mcp`] with extra global flags placed before `mcp` (e.g.
/// `--dry-run`), also returning everything the server wrote to stderr.
async fn run_mcp_with_stderr(
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

fn frame(value: Value) -> String {
    value.to_string()
}

fn initialize_frame(id: u64, protocol_version: &str) -> String {
    frame(json!({
        "jsonrpc": "2.0", "id": id, "method": "initialize",
        "params": {
            "protocolVersion": protocol_version,
            "capabilities": {},
            "clientInfo": { "name": "test-harness", "version": "0.0.0" },
        },
    }))
}

const AUTH_ENV: &[(&str, &str)] = &[("FLOWLEAP_API_KEY", "fl_pat_test_key")];

/// A registry entry with a deep, non-trivial schema: verbatim passthrough
/// means every nested keyword must survive untouched.
fn mock_tools() -> Value {
    json!([
        {
            "name": "patent_analytics",
            "description": "Full-corpus patent analytics.",
            "inputSchema": {
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dimension": { "type": "string", "enum": ["filings", "countries", "assignees"] },
                    "years": {
                        "type": "array",
                        "items": { "type": "integer", "minimum": 1900 },
                        "minItems": 1
                    }
                },
                "required": ["dimension"]
            }
        },
        {
            "name": "convert_patent_number",
            "description": "Convert a patent number between formats.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "patent_number": { "type": "string" },
                    "format": { "type": "string", "default": "docdb" }
                },
                "required": ["patent_number"]
            }
        }
    ])
}

#[tokio::test]
async fn initialize_mirrors_supported_version_and_falls_back() {
    let server = MockServer::start().await;
    let responses = run_mcp(
        &server.uri(),
        AUTH_ENV,
        &[
            initialize_frame(1, "2025-03-26"),
            frame(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })),
            initialize_frame(2, "1999-01-01"),
            initialize_frame(3, "2026-07-28"),
        ],
    )
    .await;

    let server_info = json!({ "name": "flowleap", "version": env!("CARGO_PKG_VERSION") });
    let capabilities = json!({ "tools": {}, "resources": {}, "prompts": {} });
    assert_eq!(
        responses,
        vec![
            json!({
                "jsonrpc": "2.0", "id": 1,
                "result": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": capabilities,
                    "serverInfo": server_info,
                },
            }),
            json!({
                "jsonrpc": "2.0", "id": 2,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": capabilities,
                    "serverInfo": server_info,
                },
            }),
            json!({
                "jsonrpc": "2.0", "id": 3,
                "result": {
                    "protocolVersion": "2026-07-28",
                    "capabilities": capabilities,
                    "serverInfo": server_info,
                },
            }),
        ]
    );
}

#[tokio::test]
async fn tools_list_mirrors_backend_schemas_verbatim() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/tools"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "tools": mock_tools() })))
        .mount(&server)
        .await;

    let responses = run_mcp(
        &server.uri(),
        AUTH_ENV,
        &[
            initialize_frame(1, "2024-11-05"),
            frame(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })),
        ],
    )
    .await;

    assert_eq!(responses[1]["result"], json!({ "tools": mock_tools() }));
}

#[tokio::test]
async fn tools_call_round_trips_the_tool_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/convert_patent_number"))
        .and(body_json(
            json!({ "patent_number": "EP1000000", "format": "docdb" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "tool": "convert_patent_number",
            "data": { "converted": "EP 1000000", "format": "docdb" },
            "executionTimeMs": 12,
        })))
        .mount(&server)
        .await;

    let responses = run_mcp(
        &server.uri(),
        AUTH_ENV,
        &[
            initialize_frame(1, "2024-11-05"),
            frame(json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": {
                    "name": "convert_patent_number",
                    "arguments": { "patent_number": "EP1000000", "format": "docdb" },
                },
            })),
        ],
    )
    .await;

    let result = &responses[1]["result"];
    assert_eq!(result.get("isError"), None, "success must not set isError");
    assert_eq!(result["content"][0]["type"], "text");
    let payload: Value =
        serde_json::from_str(result["content"][0]["text"].as_str().expect("text block"))
            .expect("text block is JSON");
    assert_eq!(
        payload,
        json!({ "converted": "EP 1000000", "format": "docdb" })
    );
}

#[tokio::test]
async fn tools_call_error_is_a_tool_result_carrying_the_hint() {
    let server = MockServer::start().await;
    // Missing EPO keys: the backend answers 400 data_keys_required naming the
    // provider, and the client turns that code into a providerKeysHint. The
    // message here is deliberately uninformative — nothing may depend on its
    // wording, which backend policy makes freely editable.
    Mock::given(method("POST"))
        .and(path("/v1/tools/search_patents"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": {
                "message": "wording the backend is free to change at any time",
                "type": "invalid_request_error",
                "code": "data_keys_required",
                "provider": "epo",
            },
        })))
        .mount(&server)
        .await;
    // Rate limit with a long Retry-After passes through as a hint too.
    Mock::given(method("POST"))
        .and(path("/v1/tools/get_bibliography"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "30")
                .set_body_json(json!({ "error": "rate limit exceeded" })),
        )
        .mount(&server)
        .await;

    let responses = run_mcp(
        &server.uri(),
        AUTH_ENV,
        &[
            initialize_frame(1, "2024-11-05"),
            frame(json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": "search_patents", "arguments": { "query": "ti=battery" } },
            })),
            frame(json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": "get_bibliography", "arguments": { "patent_number": "EP1000000" } },
            })),
        ],
    )
    .await;

    // Tool-level failures are isError results, never JSON-RPC errors.
    let keys_result = &responses[1]["result"];
    assert_eq!(keys_result["isError"], true);
    let keys_envelope: Value =
        serde_json::from_str(keys_result["content"][0]["text"].as_str().expect("text"))
            .expect("error text is JSON");
    assert_eq!(keys_envelope["status"], 400);
    assert_eq!(
        keys_envelope["providerKeysHint"]["code"],
        "provider_keys_required"
    );
    assert_eq!(keys_envelope["providerKeysHint"]["provider"], "epo");

    let rate_result = &responses[2]["result"];
    assert_eq!(rate_result["isError"], true);
    let rate_envelope: Value =
        serde_json::from_str(rate_result["content"][0]["text"].as_str().expect("text"))
            .expect("error text is JSON");
    assert_eq!(rate_envelope["status"], 429);
    assert_eq!(rate_envelope["retryAfterSeconds"], 30);
}

#[tokio::test]
async fn malformed_json_yields_parse_error_and_server_keeps_serving() {
    let server = MockServer::start().await;
    let responses = run_mcp(
        &server.uri(),
        AUTH_ENV,
        &[
            "{this is not json".to_string(),
            initialize_frame(1, "2024-11-05"),
        ],
    )
    .await;

    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["id"], Value::Null);
    assert_eq!(responses[0]["error"]["code"], -32700);
    assert_eq!(responses[1]["id"], 1, "server keeps serving after -32700");
}

#[tokio::test]
async fn unknown_method_yields_method_not_found_and_notifications_are_silent() {
    let server = MockServer::start().await;
    let responses = run_mcp(
        &server.uri(),
        AUTH_ENV,
        &[
            frame(json!({ "jsonrpc": "2.0", "id": 7, "method": "completion/complete" })),
            // Unknown notification (no id): must produce no frame at all.
            frame(json!({ "jsonrpc": "2.0", "method": "notifications/cancelled" })),
        ],
    )
    .await;

    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["id"], 7);
    assert_eq!(responses[0]["error"]["code"], -32601);
}

#[tokio::test]
async fn unauthenticated_server_starts_and_gates_tools_with_login_help() {
    let server = MockServer::start().await;
    let responses = run_mcp(
        &server.uri(),
        &[], // no credentials anywhere
        &[
            initialize_frame(1, "2024-11-05"),
            frame(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })),
            frame(json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": "search_patents", "arguments": {} },
            })),
        ],
    )
    .await;

    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "flowleap");
    let list_message = responses[1]["error"]["message"].as_str().expect("message");
    assert!(
        list_message.contains("flowleap auth login"),
        "list error must point at auth login: {list_message}"
    );
    assert_eq!(responses[2]["result"]["isError"], true);
    let call_text = responses[2]["result"]["content"][0]["text"]
        .as_str()
        .expect("text");
    assert!(
        call_text.contains("flowleap auth login"),
        "call error must point at auth login: {call_text}"
    );
}

#[tokio::test]
async fn mcp_check_reports_ready_with_auth_and_backend() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "status": "ok" })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/tools"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "tools": mock_tools() })))
        .mount(&server)
        .await;

    mount_doctrine(&server).await;

    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env("FLOWLEAP_BASE_URL", server.uri())
        .env("FLOWLEAP_API_KEY", "fl_pat_test_key")
        .env("FLOWLEAP_NO_UPDATE_CHECK", "1")
        .env_remove("FLOWLEAP_TOKEN")
        .args(["--json", "mcp", "--check"])
        .output()
        .expect("run mcp --check");

    assert!(output.status.success(), "expected ready exit 0");
    let value: Value = serde_json::from_slice(&output.stdout).expect("check output is json");
    assert_eq!(value["ready"], true);
    assert_eq!(value["backend"]["reachable"], true);
    assert_eq!(value["auth"]["configured"], true);
    assert_eq!(
        value["toolCount"],
        mock_tools().as_array().map(|t| t.len()).unwrap_or(0)
    );
    assert_eq!(value["resourceCount"], 5);
    assert_eq!(value["promptCount"], 3);
    assert_eq!(value["unloadedDocuments"], json!([]));
}

#[tokio::test]
async fn mcp_check_fails_without_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "status": "ok" })))
        .mount(&server)
        .await;

    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env("FLOWLEAP_BASE_URL", server.uri())
        .env("FLOWLEAP_NO_UPDATE_CHECK", "1")
        .env_remove("FLOWLEAP_API_KEY")
        .env_remove("FLOWLEAP_TOKEN")
        .args(["mcp", "--check"])
        .output()
        .expect("run mcp --check unauthenticated");

    assert!(!output.status.success(), "expected not-ready nonzero exit");
    let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
    assert!(
        stderr.contains("flowleap auth login"),
        "stderr guides to login: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// PATSTAT doctrine: resources and prompts served verbatim from the backend.
// ---------------------------------------------------------------------------

const SEMANTIC_MODEL_YAML: &str = "# PATSTAT semantic model\nmodel:\n  name: flowleap-patstat\n  note: \"ünïcode — kept byte for byte\"\n";

fn workflow_doc(name: &str, description: &str) -> Value {
    json!({
        "success": true,
        "data": {
            "baseUrl": "https://api.flowleap.co",
            "workflow": {
                "name": name,
                "description": description,
                "steps": [
                    { "step": 1, "action": "Resolve the applicant", "endpoint": "portfolio",
                      "note": "Pick one candidate.", "curlExample": "curl ..." },
                    { "step": 2, "action": "Present the numbers", "tools": ["patstat_portfolio", "patstat_query"],
                      "note": "Cite the data edition." }
                ]
            }
        }
    })
}

/// Mount the five doctrine documents at `GET /v1/patstat/docs?…`.
async fn mount_doctrine(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/v1/patstat/docs"))
        .and(query_param("section", "semantic-model"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "data": { "data_edition": "PATSTAT 2026 Spring", "yaml": SEMANTIC_MODEL_YAML },
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/patstat/docs"))
        .and(query_param("section", "examples"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "data": { "examples": [{ "question": "Q?", "logical_sql": "SELECT 1" }] },
        })))
        .mount(server)
        .await;
    for (workflow, name) in [
        ("portfolio-analysis", "Portfolio Analysis"),
        ("guarded-sql", "Guarded SQL (Layer 2)"),
        ("graph", "Graph Analytics"),
    ] {
        Mock::given(method("GET"))
            .and(path("/v1/patstat/docs"))
            .and(query_param("workflow", workflow))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(workflow_doc(name, &format!("The {workflow} workflow."))),
            )
            .mount(server)
            .await;
    }
}

#[tokio::test]
async fn resources_list_serves_the_five_doctrine_documents() {
    let server = MockServer::start().await;
    mount_doctrine(&server).await;

    let responses = run_mcp(
        &server.uri(),
        AUTH_ENV,
        &[
            initialize_frame(1, "2026-07-28"),
            frame(json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" })),
        ],
    )
    .await;

    let resources = responses[1]["result"]["resources"]
        .as_array()
        .expect("resources array");
    let uris: Vec<&str> = resources
        .iter()
        .map(|r| r["uri"].as_str().expect("uri"))
        .collect();
    assert_eq!(
        uris,
        vec![
            "flowleap://patstat/semantic-model",
            "flowleap://patstat/examples",
            "flowleap://patstat/workflow/portfolio-analysis",
            "flowleap://patstat/workflow/guarded-sql",
            "flowleap://patstat/workflow/graph",
        ]
    );
    for resource in resources {
        for field in ["name", "title", "description", "mimeType"] {
            assert!(
                resource[field].as_str().is_some_and(|v| !v.is_empty()),
                "resource {} lacks {field}",
                resource["uri"]
            );
        }
    }
    assert_eq!(resources[0]["mimeType"], "application/yaml");
    assert_eq!(resources[1]["mimeType"], "application/json");
    // Titles and descriptions come from the backend payload where one exists.
    assert_eq!(resources[2]["title"], "Portfolio Analysis");
    assert_eq!(
        resources[2]["description"],
        "The portfolio-analysis workflow."
    );
}

#[tokio::test]
async fn resources_read_returns_documents_verbatim() {
    let server = MockServer::start().await;
    mount_doctrine(&server).await;

    let responses = run_mcp(
        &server.uri(),
        AUTH_ENV,
        &[
            frame(json!({
                "jsonrpc": "2.0", "id": 1, "method": "resources/read",
                "params": { "uri": "flowleap://patstat/semantic-model" },
            })),
            frame(json!({
                "jsonrpc": "2.0", "id": 2, "method": "resources/read",
                "params": { "uri": "flowleap://patstat/workflow/graph" },
            })),
        ],
    )
    .await;

    let yaml = &responses[0]["result"]["contents"][0];
    assert_eq!(yaml["uri"], "flowleap://patstat/semantic-model");
    assert_eq!(yaml["mimeType"], "application/yaml");
    assert_eq!(yaml["text"], SEMANTIC_MODEL_YAML, "YAML is byte-identical");

    let graph = &responses[1]["result"]["contents"][0];
    assert_eq!(graph["mimeType"], "application/json");
    let served: Value =
        serde_json::from_str(graph["text"].as_str().expect("text")).expect("JSON text");
    assert_eq!(
        served,
        workflow_doc("Graph Analytics", "The graph workflow.")["data"]
    );
}

#[tokio::test]
async fn resources_read_unknown_uri_is_invalid_params_naming_the_uri() {
    let server = MockServer::start().await;
    mount_doctrine(&server).await;

    let responses = run_mcp(
        &server.uri(),
        AUTH_ENV,
        &[frame(json!({
            "jsonrpc": "2.0", "id": 1, "method": "resources/read",
            "params": { "uri": "flowleap://patstat/nope" },
        }))],
    )
    .await;

    assert_eq!(responses[0]["error"]["code"], -32602);
    let message = responses[0]["error"]["message"].as_str().expect("message");
    assert!(
        message.contains("flowleap://patstat/nope"),
        "names the URI: {message}"
    );
}

#[tokio::test]
async fn prompts_list_and_get_render_the_served_workflows() {
    let server = MockServer::start().await;
    mount_doctrine(&server).await;

    let responses = run_mcp(
        &server.uri(),
        AUTH_ENV,
        &[
            frame(json!({ "jsonrpc": "2.0", "id": 1, "method": "prompts/list" })),
            frame(json!({
                "jsonrpc": "2.0", "id": 2, "method": "prompts/get",
                "params": { "name": "patstat-portfolio-analysis" },
            })),
            frame(json!({
                "jsonrpc": "2.0", "id": 3, "method": "prompts/get",
                "params": { "name": "patstat-nope" },
            })),
        ],
    )
    .await;

    let prompts = responses[0]["result"]["prompts"]
        .as_array()
        .expect("prompts array");
    let names: Vec<&str> = prompts
        .iter()
        .map(|p| p["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        vec![
            "patstat-portfolio-analysis",
            "patstat-guarded-sql",
            "patstat-graph"
        ]
    );
    assert_eq!(prompts[1]["title"], "Guarded SQL (Layer 2)");
    assert_eq!(prompts[1]["description"], "The guarded-sql workflow.");
    assert_eq!(prompts[1]["arguments"], json!([]));

    let result = &responses[1]["result"];
    assert_eq!(result["description"], "The portfolio-analysis workflow.");
    let messages = result["messages"].as_array().expect("messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"]["type"], "text");
    assert_eq!(
        messages[0]["content"]["text"],
        "Portfolio Analysis\n\
         \n\
         The portfolio-analysis workflow.\n\
         \n\
         Steps:\n\
         1. Resolve the applicant\n   \
            Endpoint: portfolio\n   \
            Note: Pick one candidate.\n\
         2. Present the numbers\n   \
            Tools: patstat_portfolio, patstat_query\n   \
            Note: Cite the data edition.\n"
    );

    assert_eq!(responses[2]["error"]["code"], -32602);
    let message = responses[2]["error"]["message"].as_str().expect("message");
    assert!(
        message.contains("patstat-nope"),
        "names the prompt: {message}"
    );
}

#[tokio::test]
async fn a_document_that_fails_to_load_is_skipped_and_logged() {
    let server = MockServer::start().await;
    // The docs route is plan-gated: a plan-less user gets 402 for one doc.
    Mock::given(method("GET"))
        .and(path("/v1/patstat/docs"))
        .and(query_param("workflow", "graph"))
        .respond_with(ResponseTemplate::new(402).set_body_json(json!({
            "error": { "code": "subscription_required", "message": "subscribe" },
        })))
        .with_priority(1)
        .mount(&server)
        .await;
    mount_doctrine(&server).await;

    let (responses, stderr) = run_mcp_with_stderr(
        &server.uri(),
        AUTH_ENV,
        &[],
        &[
            frame(json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/list" })),
            frame(json!({ "jsonrpc": "2.0", "id": 2, "method": "prompts/list" })),
        ],
    )
    .await;

    assert_eq!(
        responses[0]["result"]["resources"].as_array().map(Vec::len),
        Some(4)
    );
    assert_eq!(
        responses[1]["result"]["prompts"].as_array().map(Vec::len),
        Some(2)
    );
    assert!(
        stderr.contains("flowleap mcp: could not load workflow/graph: HTTP 402"),
        "stderr names the failed doc: {stderr}"
    );
}

#[tokio::test]
async fn backend_unreachable_at_startup_still_serves_with_zero_doctrine() {
    // Port 9 (discard) on loopback: nothing listens, connections are refused.
    let (responses, stderr) = run_mcp_with_stderr(
        "http://127.0.0.1:9",
        AUTH_ENV,
        &[],
        &[
            initialize_frame(1, "2026-07-28"),
            frame(json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" })),
            frame(json!({ "jsonrpc": "2.0", "id": 3, "method": "prompts/list" })),
        ],
    )
    .await;

    assert_eq!(responses[0]["result"]["protocolVersion"], "2026-07-28");
    assert_eq!(responses[1]["result"], json!({ "resources": [] }));
    assert_eq!(responses[2]["result"], json!({ "prompts": [] }));
    assert!(
        stderr.contains("flowleap mcp: could not load semantic-model"),
        "stderr logs the failures: {stderr}"
    );
}

#[tokio::test]
async fn dry_run_prints_the_five_requests_and_serves_nothing() {
    let server = MockServer::start().await;
    mount_doctrine(&server).await;

    let (responses, stderr) = run_mcp_with_stderr(
        &server.uri(),
        AUTH_ENV,
        &["--dry-run"],
        &[frame(
            json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/list" }),
        )],
    )
    .await;

    assert_eq!(responses[0]["result"], json!({ "resources": [] }));
    for query in [
        "section=semantic-model",
        "section=examples",
        "workflow=portfolio-analysis",
        "workflow=guarded-sql",
        "workflow=graph",
    ] {
        assert!(
            stderr.contains(&format!("/v1/patstat/docs?{query}")),
            "dry-run prints the {query} request: {stderr}"
        );
    }
    assert!(
        server
            .received_requests()
            .await
            .unwrap_or_default()
            .is_empty(),
        "dry-run sends nothing"
    );
}
