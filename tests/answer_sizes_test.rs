//! The answer-size flags (backend #635): the summary views and the pages of
//! `ops family`, `ops legal`, `uspto application`, `uspto grant` and
//! `uspto documents`.
//!
//! The backend default answers fit a chat context: a family is one page of
//! members, a file wrapper record has its long lists cut, a document listing
//! and a legal status answer are one page. These tests pin the flags that ask
//! for the remaining data, and the stderr notes that say an answer is one part
//! of the whole. `--json` stdout stays the backend payload verbatim.

mod support;

use serde_json::{json, Value};
use support::{run_cli, stdout_json, tool_ok};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Mount `tool` so that it answers `data` only for exactly the input
/// `expected_input`. A different input gets wiremock's 404, and the run fails.
async fn mount_tool_for(server: &MockServer, tool: &str, expected_input: Value, data: Value) {
    Mock::given(method("POST"))
        .and(path(format!("/v1/tools/{tool}")))
        .and(body_json(expected_input))
        .respond_with(ResponseTemplate::new(200).set_body_json(tool_ok(tool, data)))
        .expect(1)
        .mount(server)
        .await;
}

async fn run(server: &MockServer, args: &[&str]) -> std::process::Output {
    run_cli(&server.uri(), &[("FLOWLEAP_API_KEY", "fl_pat_test")], args).await
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is utf8")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf8")
}

/// The summary-view family answer for a page of 2 of 5 members.
fn family_summary() -> Value {
    json!({
        "docId": "EP1000000",
        "view": "summary",
        "countries": { "AT": 1, "DE": 1, "EP": 1, "NL": 1, "US": 1 },
        "representatives": { "members": [{ "country": "EP" }, { "country": "US" }] },
        "representativesTotal": 5,
        "representativesReturned": 2,
        "recordsTotal": 12,
        "recordsReturned": 0,
        "offset": 0,
        "next": "This answer holds 2 of 5 family members (applications). Call again with offset=2 for the next page."
    })
}

#[tokio::test]
async fn ops_family_without_flags_sends_the_number_only_and_prints_next_on_stderr() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "get_family",
        json!({ "patent_number": "EP1000000" }),
        family_summary(),
    )
    .await;

    let output = run(&server, &["ops", "family", "EP1000000"]).await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout_json(&output), family_summary());
    assert!(
        stderr(&output).contains("note: This answer holds 2 of 5 family members"),
        "{}",
        stderr(&output)
    );
}

#[tokio::test]
async fn ops_family_sends_view_country_limit_and_offset() {
    let server = MockServer::start().await;
    let data = json!({ "docId": "EP1000000", "view": "full", "members": [] });
    mount_tool_for(
        &server,
        "get_family",
        json!({
            "patent_number": "EP1000000",
            "view": "full",
            "country": "US",
            "limit": 20,
            "offset": 40
        }),
        data.clone(),
    )
    .await;

    let output = run(
        &server,
        &[
            "ops",
            "family",
            "EP1000000",
            "--view",
            "full",
            "--country",
            "us",
            "--limit",
            "20",
            "--offset",
            "40",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout_json(&output), data);
    // A complete answer has no `next`: no note.
    assert!(!stderr(&output).contains("note:"), "{}", stderr(&output));
}

#[tokio::test]
async fn ops_family_json_keeps_stderr_clean() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "get_family",
        json!({ "patent_number": "EP1000000" }),
        family_summary(),
    )
    .await;

    let output = run(&server, &["--json", "ops", "family", "EP1000000"]).await;

    assert_eq!(output.status.code(), Some(0));
    // The `next` string is in the payload; `--json` adds no stderr line.
    assert_eq!(stdout_json(&output)["next"], family_summary()["next"]);
    assert!(!stderr(&output).contains("note:"), "{}", stderr(&output));
}

#[test]
fn ops_family_refuses_an_unknown_view() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .args(["ops", "family", "EP1000000", "--view", "rows", "--dry-run"])
        .output()
        .expect("run flowleap");
    assert_eq!(output.status.code(), Some(2));
}

/// A legal status page: 2 of 50 events, each with the event date and the
/// later record date.
fn legal_page() -> Value {
    json!({
        "docId": "EP1000000B1",
        "designatedStates": ["DE", "FR"],
        "events": [
            {
                "code": "PLBE", "country": "EP", "date": "2022-02-02",
                "eventDate": "2003-12-19", "text": "NO OPPOSITION FILED WITHIN TIME LIMIT"
            },
            {
                "code": "TPAB", "country": "EP", "date": "2021-09-29",
                "eventDate": "2014-06-07", "text": "OBSERVATIONS BY THIRD PARTIES DELETED"
            }
        ],
        "eventsReturned": 2,
        "eventsTotal": 50,
        "offset": 10
    })
}

#[tokio::test]
async fn ops_legal_sends_the_page_and_names_a_partial_answer_on_stderr() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "get_legal_status",
        json!({ "patent_number": "EP1000000B1", "limit": 2, "offset": 10 }),
        legal_page(),
    )
    .await;

    let output = run(
        &server,
        &[
            "ops",
            "legal",
            "EP1000000B1",
            "--limit",
            "2",
            "--offset",
            "10",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    // Human output keeps the payload verbatim, as every `ops` read does.
    assert_eq!(stdout_json(&output), legal_page());
    let stderr = stderr(&output);
    assert!(stderr.contains("2 of 50 legal status events"), "{stderr}");
    assert!(stderr.contains("--offset 12"), "{stderr}");
}

#[tokio::test]
async fn ops_legal_table_has_an_event_date_column_and_a_record_date_column() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "get_legal_status",
        json!({ "patent_number": "EP1000000B1" }),
        legal_page(),
    )
    .await;

    let output = run(
        &server,
        &["--output", "table", "ops", "legal", "EP1000000B1"],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let stdout = stdout(&output);
    for expected in [
        "Event date",
        "Record date",
        "2003-12-19",
        "2022-02-02",
        "PLBE",
    ] {
        assert!(stdout.contains(expected), "missing {expected}: {stdout}");
    }
}

#[tokio::test]
async fn ops_legal_with_every_event_prints_no_note() {
    let server = MockServer::start().await;
    let mut data = legal_page();
    data["eventsTotal"] = json!(2);
    data["offset"] = json!(0);
    mount_tool_for(
        &server,
        "get_legal_status",
        json!({ "patent_number": "EP1000000B1" }),
        data,
    )
    .await;

    let output = run(&server, &["ops", "legal", "EP1000000B1"]).await;

    assert_eq!(output.status.code(), Some(0));
    assert!(!stderr(&output).contains("note:"), "{}", stderr(&output));
}

/// A summary-view file wrapper answer with two cut lists.
fn record_summary() -> Value {
    json!({
        "view": "summary",
        "patentFileWrapperDataBag": [{
            "applicationNumberText": "16123456",
            "applicationMetaData": { "inventionTitle": "Battery cooling assembly" }
        }],
        "truncated": [
            { "path": "eventDataBag", "total": 58, "returned": 10, "tool": "get_transactions" },
            { "path": "recordAttorney.attorneyBag", "total": 40, "returned": 3, "tool": "get_attorney" }
        ]
    })
}

#[tokio::test]
async fn uspto_application_prints_each_truncated_list_on_stderr() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "get_us_application",
        json!({ "application_number": "16123456" }),
        record_summary(),
    )
    .await;

    let output = run(&server, &["uspto", "application", "16123456"]).await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("Title: Battery cooling assembly"));
    let stderr = stderr(&output);
    assert!(
        stderr.contains("eventDataBag holds 10 of 58 row(s)"),
        "{stderr}"
    );
    assert!(stderr.contains("get_transactions"), "{stderr}");
    assert!(
        stderr.contains("recordAttorney.attorneyBag holds 3 of 40 row(s)"),
        "{stderr}"
    );
}

#[tokio::test]
async fn uspto_application_full_sends_view_full() {
    let server = MockServer::start().await;
    let data = json!({ "view": "full", "patentFileWrapperDataBag": [] });
    mount_tool_for(
        &server,
        "get_us_application",
        json!({ "application_number": "16123456", "view": "full" }),
        data.clone(),
    )
    .await;

    let output = run(
        &server,
        &["--json", "uspto", "application", "16123456", "--full"],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout_json(&output), data);
}

#[tokio::test]
async fn uspto_grant_full_sends_view_full_and_the_default_sends_no_view() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "get_us_grant",
        json!({ "patent_number": "11800000", "view": "full" }),
        json!({ "view": "full", "patentFileWrapperDataBag": [] }),
    )
    .await;
    mount_tool_for(
        &server,
        "get_us_grant",
        json!({ "patent_number": "11800000" }),
        record_summary(),
    )
    .await;

    let full = run(&server, &["--json", "uspto", "grant", "11800000", "--full"]).await;
    assert_eq!(full.status.code(), Some(0), "{}", stderr(&full));
    assert_eq!(stdout_json(&full)["view"], "full");

    let summary = run(&server, &["--json", "uspto", "grant", "11800000"]).await;
    assert_eq!(summary.status.code(), Some(0), "{}", stderr(&summary));
    // `--json` keeps `truncated` in the payload and adds no stderr line.
    assert_eq!(stdout_json(&summary), record_summary());
    assert!(!stderr(&summary).contains("note:"), "{}", stderr(&summary));
}

fn document_codes() -> Value {
    json!({ "CTNF": 1, "NOA": 2, "REM": 1 })
}

#[tokio::test]
async fn uspto_documents_sends_the_page_and_the_download_options() {
    let server = MockServer::start().await;
    let data = json!({
        "applicationNumber": "16123456",
        "total": 52, "matched": 52, "returned": 1, "offset": 40,
        "documentCodes": document_codes(),
        "documents": [{
            "documentIdentifier": "L7AH385RGREENX5", "documentCode": "SRNT",
            "downloadOptionBag": [{ "mimeTypeIdentifier": "PDF" }]
        }]
    });
    mount_tool_for(
        &server,
        "get_application_documents",
        json!({
            "application_number": "16123456",
            "limit": 1,
            "offset": 40,
            "include_download_options": true
        }),
        data.clone(),
    )
    .await;

    let output = run(
        &server,
        &[
            "--json",
            "uspto",
            "documents",
            "16123456",
            "--limit",
            "1",
            "--offset",
            "40",
            "--download-options",
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout_json(&output), data);
}

#[tokio::test]
async fn uspto_documents_names_the_next_page_of_a_partial_listing() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "get_application_documents",
        json!({ "application_number": "16123456", "limit": 1 }),
        json!({
            "total": 52, "matched": 52, "returned": 1, "offset": 0,
            "documentCodes": document_codes(),
            "documents": [{ "documentIdentifier": "L7AH385RGREENX5", "documentCode": "SRNT" }]
        }),
    )
    .await;

    let output = run(&server, &["uspto", "documents", "16123456", "--limit", "1"]).await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("L7AH385RGREENX5"));
    let stderr = stderr(&output);
    assert!(stderr.contains("1 of 52 matching document(s)"), "{stderr}");
    assert!(stderr.contains("--offset 1"), "{stderr}");
}

/// A filter that no document passes: the note says so and lists the codes the
/// file wrapper holds, from `documentCodes`.
#[tokio::test]
async fn uspto_documents_with_no_match_lists_the_codes_that_exist() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "get_application_documents",
        json!({ "application_number": "16123456", "document_code": "CTFR" }),
        json!({
            "total": 52, "matched": 0, "returned": 0, "offset": 0,
            "documentCodes": document_codes(),
            "documents": []
        }),
    )
    .await;

    let output = run(
        &server,
        &["uspto", "documents", "16123456", "--code", "ctfr"],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let stderr = stderr(&output);
    assert!(stderr.contains("none match the filter"), "{stderr}");
    assert!(stderr.contains("CTNF (1), NOA (2), REM (1)"), "{stderr}");
}

/// A page after the last document is empty, but documents match: the note
/// must not say that none match the filter.
#[tokio::test]
async fn uspto_documents_page_after_the_end_is_not_a_filter_miss() {
    let server = MockServer::start().await;
    mount_tool_for(
        &server,
        "get_application_documents",
        json!({ "application_number": "16123456", "offset": 5000 }),
        json!({
            "total": 52, "matched": 52, "returned": 0, "offset": 5000,
            "documentCodes": document_codes(),
            "documents": []
        }),
    )
    .await;

    let output = run(
        &server,
        &["uspto", "documents", "16123456", "--offset", "5000"],
    )
    .await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let stderr = stderr(&output);
    assert!(!stderr.contains("none match the filter"), "{stderr}");
    assert!(
        stderr.contains("--offset 5000 is after the last document: 52 document(s) match"),
        "{stderr}"
    );
}
