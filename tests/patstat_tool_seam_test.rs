//! `patstat query` / `patstat docs` / `patstat portfolio` on the shared tool
//! seam (issue #95): each command POSTs `/v1/tools/patstat_<name>` with
//! snake_case input, prints the tool's `data` verbatim in JSON mode, relays
//! the typed PATSTAT errors unchanged, and — for guarded SQL only — never
//! resends on its own (ADR 0010: the agent owns the one retry). Driven
//! through the real binary against a wiremock backend, and through
//! `flowleap mcp` for the `tools/call` path.

mod support;

use serde_json::{json, Value};
use support::{frame, initialize_frame, run_cli, run_mcp, stdout_json};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const API_KEY_ENV: (&str, &str) = ("FLOWLEAP_API_KEY", "fl_pat_test_key");
const ATTRIBUTION: &str =
    "This product contains data sourced from EPO databases, © European Patent Organisation";
const SQL: &str = "SELECT office, COUNT(DISTINCT family_id) AS inventions FROM flowleap.applications GROUP BY office";

fn envelope(tool: &str, data: Value) -> Value {
    json!({ "success": true, "tool": tool, "data": data, "executionTimeMs": 40 })
}

fn query_data() -> Value {
    json!({
        "rows": [
            { "office": "EP", "inventions": 70 },
            { "office": "US", "inventions": 50 },
        ],
        "rowCount": 2,
        "data_edition": "PATSTAT 2026 Spring",
        "attribution": ATTRIBUTION,
    })
}

/// A typed facade error envelope: the route's code, message and status,
/// the route's extra fields nested under `error.details`.
fn tool_error(code: &str, message: &str, details: Value, status: u16) -> Value {
    json!({
        "success": false,
        "error": { "code": code, "message": message, "details": details },
        "status": status,
    })
}

fn timeout_error() -> Value {
    tool_error(
        "patstat_sql_timeout",
        "The query exceeded the 20000 ms statement timeout. Narrow the filters or aggregate earlier.",
        json!({ "timeout_ms": 20000 }),
        504,
    )
}

async fn requests_to(server: &MockServer, route: &str) -> usize {
    server
        .received_requests()
        .await
        .expect("request recording is on")
        .iter()
        .filter(|request| request.url.path() == route)
        .count()
}

// The dry-run request shape of the three commands is asserted with every
// other data command in tests/facade_migration_test.rs.

// ------------------------------------------------------------------ query

#[tokio::test]
async fn query_json_mode_prints_the_tool_data_verbatim() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/patstat_query"))
        .and(body_json(
            json!({ "sql": SQL, "question": "filings by office" }),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(envelope("patstat_query", query_data())),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "--json",
            "patstat",
            "query",
            SQL,
            "--question",
            "filings by office",
        ],
    )
    .await;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(stdout_json(&output), query_data());
}

#[tokio::test]
async fn query_human_mode_renders_rows_count_and_edition() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/patstat_query"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(envelope("patstat_query", query_data())),
        )
        .mount(&server)
        .await;

    let output = run_cli(&server.uri(), &[API_KEY_ENV], &["patstat", "query", SQL]).await;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("inventions"), "{stdout}");
    assert!(stdout.contains("Rows: 2"), "{stdout}");
    assert!(
        stdout.contains("Source: PATSTAT data edition PATSTAT 2026 Spring"),
        "{stdout}"
    );
    assert!(!stdout.contains("\"rows\""), "{stdout}");
}

/// A cold timeout is the agent's signal: it must arrive after exactly one
/// request, with the backend envelope verbatim and the generic exit code.
#[tokio::test]
async fn sql_timeout_reaches_the_caller_after_exactly_one_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/patstat_query"))
        .respond_with(ResponseTemplate::new(504).set_body_json(timeout_error()))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "query", SQL, "--question", "q"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout_json(&output), timeout_error());
    assert_eq!(
        requests_to(&server, "/v1/tools/patstat_query").await,
        1,
        "guarded SQL must never be resent by the client"
    );
}

#[tokio::test]
async fn typed_sql_error_keeps_code_message_and_details_in_both_modes() {
    let invalid = tool_error(
        "patstat_sql_invalid",
        "syntax error at or near \"SELEC\" Send exactly one SELECT statement.",
        json!({ "position": 0, "reason": "parse_error", "hint": "start with SELECT" }),
        400,
    );
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/patstat_query"))
        .respond_with(ResponseTemplate::new(400).set_body_json(invalid.clone()))
        .mount(&server)
        .await;

    let json_run = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "query", "SELEC 1"],
    )
    .await;
    assert_eq!(json_run.status.code(), Some(1));
    let value = stdout_json(&json_run);
    assert_eq!(value, invalid);

    let human_run = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "query", "SELEC 1"],
    )
    .await;
    assert_eq!(human_run.status.code(), Some(1));
    let stdout = String::from_utf8(human_run.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains(
            "Guarded SQL rejected (patstat_sql_invalid): syntax error at or near \"SELEC\" \
             Send exactly one SELECT statement."
        ),
        "{stdout}"
    );
    assert!(stdout.contains("  position: 0"), "{stdout}");
    assert!(stdout.contains("  hint: \"start with SELECT\""), "{stdout}");
    assert!(
        stdout.contains("--retry-of patstat_sql_invalid"),
        "{stdout}"
    );
}

/// `patstat_busy` is a back-off signal with its own exit code; even its
/// short Retry-After must not trigger a hidden client resend.
#[tokio::test]
async fn busy_is_rate_limited_and_not_resent() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/patstat_query"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "1")
                .set_body_json(tool_error(
                    "patstat_busy",
                    "The analytics database is busy. Retry the same SQL shortly.",
                    json!({}),
                    429,
                )),
        )
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patstat", "query", SQL],
    )
    .await;

    assert_eq!(output.status.code(), Some(6));
    assert_eq!(stdout_json(&output)["error"]["code"], "patstat_busy");
    assert_eq!(requests_to(&server, "/v1/tools/patstat_query").await, 1);
}

// ------------------------------------------------------------ retry seam

/// The per-request opt-out is scoped to `patstat_query`: the same 503 is a
/// single send for it and the usual bounded resends for any other tool.
#[tokio::test]
async fn only_patstat_query_opts_out_of_the_generic_5xx_resend() {
    let server = MockServer::start().await;
    for tool in ["patstat_query", "get_bibliography"] {
        Mock::given(method("POST"))
            .and(path(format!("/v1/tools/{tool}")))
            .respond_with(ResponseTemplate::new(503).set_body_json(json!({
                "success": false,
                "error": { "code": "INTERNAL_ERROR", "message": "upstream down" },
            })))
            .mount(&server)
            .await;
    }

    let guarded = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "tools", "run", "patstat_query", "sql=SELECT 1"],
    )
    .await;
    assert!(!guarded.status.success());

    let other = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "--json",
            "tools",
            "run",
            "get_bibliography",
            "patent_number=EP1000000",
        ],
    )
    .await;
    assert!(!other.status.success());

    assert_eq!(requests_to(&server, "/v1/tools/patstat_query").await, 1);
    // One send plus the default two retries.
    assert_eq!(requests_to(&server, "/v1/tools/get_bibliography").await, 3);
}

// ------------------------------------------------------------------- docs

#[tokio::test]
async fn docs_selectors_map_to_the_tool_input() {
    let cases: [(&[&str], Value); 5] = [
        (&[], json!({ "compact": false })),
        (&["--compact"], json!({ "compact": true })),
        (&["--section", "examples"], json!({ "section": "examples" })),
        (
            &["--workflow", "guarded-sql"],
            json!({ "workflow": "guarded-sql" }),
        ),
        (&["--endpoint", "query"], json!({ "endpoint": "query" })),
    ];

    for (flags, input) in cases {
        let server = MockServer::start().await;
        let data = json!({ "selected": input.clone(), "attribution": ATTRIBUTION });
        Mock::given(method("POST"))
            .and(path("/v1/tools/patstat_docs"))
            .and(body_json(input.clone()))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(envelope("patstat_docs", data.clone())),
            )
            .expect(1)
            .mount(&server)
            .await;

        let mut argv = vec!["--json", "patstat", "docs"];
        argv.extend_from_slice(flags);
        let output = run_cli(&server.uri(), &[API_KEY_ENV], &argv).await;

        assert!(
            output.status.success(),
            "{flags:?} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(stdout_json(&output), data, "{flags:?}");
    }
}

#[tokio::test]
async fn docs_semantic_model_prints_the_yaml_raw_in_human_mode() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/patstat_docs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope(
            "patstat_docs",
            json!({
                "yaml": "views:\n  applications: {}\n",
                "data_edition": "PATSTAT 2026 Spring",
                "attribution": ATTRIBUTION,
            }),
        )))
        .mount(&server)
        .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patstat", "docs", "--section", "semantic-model"],
    )
    .await;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert_eq!(
        stdout,
        "# Loaded edition: PATSTAT 2026 Spring\nviews:\n  applications: {}\n\n"
    );
}

// -------------------------------------------------------------------- mcp

fn call_frame(id: u64, sql: &str) -> String {
    frame(json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": { "name": "patstat_query", "arguments": { "sql": sql } },
    }))
}

#[tokio::test]
async fn mcp_invalid_sql_is_an_error_result_with_the_backend_code_and_message() {
    let message = "syntax error at or near \"SELEC\" Send exactly one SELECT statement.";
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/patstat_query"))
        .respond_with(ResponseTemplate::new(400).set_body_json(tool_error(
            "patstat_sql_invalid",
            message,
            json!({ "position": 0, "reason": "parse_error" }),
            400,
        )))
        .mount(&server)
        .await;

    let responses = run_mcp(
        &server.uri(),
        &[API_KEY_ENV],
        &[initialize_frame(1, "2024-11-05"), call_frame(2, "SELEC 1")],
    )
    .await;

    let result = &responses[1]["result"];
    assert_eq!(result["isError"], true);
    let text: Value = serde_json::from_str(result["content"][0]["text"].as_str().expect("text"))
        .expect("error text is JSON");
    assert_eq!(text["status"], 400);
    assert_eq!(text["body"]["error"]["code"], "patstat_sql_invalid");
    assert_eq!(text["body"]["error"]["message"], message);
    assert_eq!(text["body"]["error"]["details"]["reason"], "parse_error");
}

#[tokio::test]
async fn mcp_patstat_query_timeout_is_a_single_send() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/patstat_query"))
        .respond_with(ResponseTemplate::new(504).set_body_json(timeout_error()))
        .mount(&server)
        .await;

    let responses = run_mcp(
        &server.uri(),
        &[API_KEY_ENV],
        &[initialize_frame(1, "2024-11-05"), call_frame(2, SQL)],
    )
    .await;

    let result = &responses[1]["result"];
    assert_eq!(result["isError"], true);
    let text: Value = serde_json::from_str(result["content"][0]["text"].as_str().expect("text"))
        .expect("error text is JSON");
    assert_eq!(text["body"], timeout_error());
    assert_eq!(requests_to(&server, "/v1/tools/patstat_query").await, 1);
}
