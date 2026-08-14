//! Human/table rendering of USPTO ODP payloads.
//!
//! ODP puts a record's identity at the top level (`applicationNumberText`) and
//! everything else under `applicationMetaData`. The renderers used to look up
//! flat keys that shape never carries, so `uspto search` printed one useless
//! line per hit and `uspto grant` fell through to a raw JSON dump. `--json` was
//! always fine — it prints the backend payload verbatim — so these tests pin
//! the human view against the real shape and prove `--json` stays untouched.

mod support;

use serde_json::{json, Value};
use support::{run_cli, stdout_json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// One ODP file-wrapper record, in the shape USPTO actually returns.
fn odp_record() -> Value {
    json!({
        "applicationNumberText": "16123456",
        "applicationMetaData": {
            "inventionTitle": "Battery cooling assembly",
            "firstApplicantName": "Acme Robotics Inc.",
            "firstInventorName": "Ada Lovelace",
            "patentNumber": "11800000",
            "earliestPublicationNumber": "US20200123456A1",
            "earliestPublicationDate": "2020-04-23",
            "filingDate": "2019-10-01",
            "grantDate": "2023-10-31",
            "applicationStatusDescriptionText": "Patented Case",
        },
    })
}

/// Mount a facade tool answering with an ODP file-wrapper payload.
async fn mount_tool(server: &MockServer, tool: &str, data: Value) {
    Mock::given(method("POST"))
        .and(path(format!("/v1/tools/{tool}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "tool": tool,
            "data": data,
        })))
        .mount(server)
        .await;
}

async fn run(server: &MockServer, args: &[&str]) -> std::process::Output {
    run_cli(&server.uri(), &[("FLOWLEAP_API_KEY", "fl_pat_test")], args).await
}

/// Every column of a search hit resolves against the real nested shape.
#[tokio::test]
async fn search_renders_the_nested_odp_fields() {
    let server = MockServer::start().await;
    mount_tool(
        &server,
        "search_patents",
        json!({ "patentFileWrapperDataBag": [odp_record()] }),
    )
    .await;

    let output = run(&server, &["uspto", "search", "--query", "ti:battery"]).await;

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    for expected in [
        "16123456",
        "Battery cooling assembly",
        "Acme Robotics Inc.",
        "2019-10-01",
        "Patented Case",
    ] {
        assert!(stdout.contains(expected), "missing {expected}: {stdout}");
    }
}

/// A grant renders as labelled fields rather than falling through to a raw
/// JSON dump, reading the single record out of the file-wrapper bag.
#[tokio::test]
async fn grant_renders_labelled_fields_not_a_json_dump() {
    let server = MockServer::start().await;
    mount_tool(
        &server,
        "get_us_grant",
        json!({ "patentFileWrapperDataBag": [odp_record()] }),
    )
    .await;

    let output = run(&server, &["uspto", "grant", "11800000"]).await;

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(
        stdout.contains("Title: Battery cooling assembly"),
        "{stdout}"
    );
    assert!(stdout.contains("Patent #: 11800000"), "{stdout}");
    assert!(stdout.contains("Granted: 2023-10-31"), "{stdout}");
    assert!(
        !stdout.contains("patentFileWrapperDataBag"),
        "human output must not dump the envelope: {stdout}"
    );
}

/// `--json` keeps the backend payload verbatim — the rendering fix must not
/// reshape what agents parse.
#[tokio::test]
async fn json_output_stays_the_verbatim_backend_payload() {
    let server = MockServer::start().await;
    let data = json!({ "patentFileWrapperDataBag": [odp_record()] });
    mount_tool(&server, "get_us_grant", data.clone()).await;

    let output = run(&server, &["--json", "uspto", "grant", "11800000"]).await;

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout_json(&output), data);
}
