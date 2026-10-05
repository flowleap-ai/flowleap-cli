//! `flowleap patent examiner-baseline` (PRD 0019 F1). The verb calls the
//! backend `examiner_baseline` tool when the registry lists it, and walks the
//! family over the tools facade itself when it does not. The matrix logic has
//! unit tests in the module; this file guards the I/O: which path runs, which
//! tools are called with which input, that a per-document failure becomes a
//! gap row, and that a failure a human must act on still stops the verb with
//! its documented exit code.

mod support;

use serde_json::json;
use support::{run_cli, stdout_json, tool_ok};
use wiremock::matchers::{body_json, body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const API_KEY_ENV: (&str, &str) = ("FLOWLEAP_API_KEY", "fl_pat_test_key");

async fn mount_tool(
    server: &MockServer,
    tool: &str,
    input: serde_json::Value,
    response: ResponseTemplate,
) {
    Mock::given(method("POST"))
        .and(path(format!("/v1/tools/{tool}")))
        .and(body_partial_json(input))
        .respond_with(response)
        .mount(server)
        .await;
}

fn ok(tool: &str, data: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(tool_ok(tool, data))
}

/// The `/v1/tools` registry, listing the given tool names.
async fn mount_registry(server: &MockServer, names: &[&str]) {
    let tools: Vec<serde_json::Value> = names.iter().map(|n| json!({ "name": n })).collect();
    Mock::given(method("GET"))
        .and(path("/v1/tools"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "tools": tools })))
        .mount(server)
        .await;
}

/// An older backend: the registry lists the four walk tools, not the
/// one-call tool.
async fn mount_registry_without_tool(server: &MockServer) {
    mount_registry(
        server,
        &[
            "get_family",
            "get_bibliography",
            "get_us_grant",
            "search_office_action_citations",
        ],
    )
    .await;
}

async fn mount_family(server: &MockServer) {
    mount_tool(
        server,
        "get_family",
        json!({ "patent_number": "EP2110298B1" }),
        ok(
            "get_family",
            json!({ "representatives": { "members": [
                { "application": "EP09250131 (A)", "publications": ["EP2110298A3", "EP2110298B1"],
                  "representativePublication": "EP2110298B1" },
                { "application": "US10374408 (A)", "publications": ["US7722129B2"],
                  "representativePublication": "US7722129B2" },
                { "application": "CN1 (A)", "publications": ["CN1A"],
                  "representativePublication": "CN1A" },
            ]}}),
        ),
    )
    .await;
}

#[tokio::test]
async fn the_walk_reads_every_publication_and_turns_a_failed_read_into_a_gap() {
    let server = MockServer::start().await;
    mount_registry_without_tool(&server).await;
    mount_family(&server).await;
    mount_tool(
        &server,
        "get_bibliography",
        json!({ "patent_number": "EP2110298A3" }),
        ok("get_bibliography", json!({ "citedReferences": [
            { "docId": "US4763957", "kind": "A", "citedBy": "examiner", "category": "X", "relevantClaims": "13" },
        ]})),
    )
    .await;
    mount_tool(
        &server,
        "get_bibliography",
        json!({ "patent_number": "EP2110298B1" }),
        ok("get_bibliography", json!({ "title": "t" })),
    )
    .await;
    mount_tool(
        &server,
        "get_bibliography",
        json!({ "patent_number": "US7722129B2" }),
        ok(
            "get_bibliography",
            json!({ "citedReferences": [
                { "docId": "US4763957", "kind": "A", "citedBy": "applicant" },
            ]}),
        ),
    )
    .await;
    mount_tool(
        &server,
        "get_bibliography",
        json!({ "patent_number": "CN1A" }),
        ResponseTemplate::new(404).set_body_json(json!({
            "success": false, "error": { "code": "NOT_FOUND", "message": "no such document" }, "status": 404
        })),
    )
    .await;
    mount_tool(
        &server,
        "get_us_grant",
        json!({ "patent_number": "7722129" }),
        ok(
            "get_us_grant",
            json!({ "applicationNumberText": "12103744" }),
        ),
    )
    .await;
    mount_tool(
        &server,
        "search_office_action_citations",
        json!({ "application_number": "12103744", "size": 1000 }),
        ok("search_office_action_citations", json!({ "total": 1, "citations": [
            { "citedDocument": "US 5,135,330", "documentReference": { "publicationNumber": "US5135330" },
              "category": "X", "examinerCited": true, "applicantCited": false, "rejectedClaims": "1",
              "officeActionDate": "2009-08-18T00:00:00", "officeActionType": "CTNF", "isNPL": false },
        ]})),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patent", "examiner-baseline", "EP2110298B1"],
    )
    .await;
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body = stdout_json(&output);

    let cells: Vec<(String, serde_json::Value)> = body["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            let texts: serde_json::Map<String, serde_json::Value> = d["cells"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(office, cell)| (office.clone(), cell["text"].clone()))
                .collect();
            (
                d["document"].as_str().unwrap().to_string(),
                serde_json::Value::Object(texts),
            )
        })
        .collect();
    let gaps: Vec<&str> = body["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["message"].as_str().unwrap())
        .collect();
    assert_eq!(
        (body["offices"].clone(), cells, gaps),
        (
            json!(["EP", "US", "CN"]),
            vec![
                (
                    "US4763957".to_string(),
                    json!({ "EP": "X cl. 13", "US": "applicant" })
                ),
                (
                    "US5135330".to_string(),
                    json!({ "US": "X cl. 1 (US OA 2009-08-18)" })
                ),
            ],
            vec![
                "could not read citations from CN (CN1A): 404 NOT_FOUND",
                "no citation record from CN (CN1A)",
            ],
        )
    );
}

#[tokio::test]
async fn a_failure_a_human_must_act_on_stops_the_walk() {
    let server = MockServer::start().await;
    mount_registry_without_tool(&server).await;
    mount_family(&server).await;
    mount_tool(
        &server,
        "get_bibliography",
        json!({}),
        ResponseTemplate::new(402).set_body_json(json!({
            "success": false, "error": { "code": "subscription_required", "message": "subscribe" }, "status": 402
        })),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["--json", "patent", "examiner-baseline", "EP2110298B1"],
    )
    .await;
    assert_eq!(
        output.status.code(),
        Some(4),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// One cell of the backend answer, as the live tool served EP2110298B1 on
/// 2026-10-05 (trimmed).
fn tool_answer() -> serde_json::Value {
    json!({
        "publication": "EP2110298B1",
        "offices": ["EP", "US"],
        "dedupe": "docdb-number",
        "membersWalked": [
            { "office": "EP", "representativePublication": "EP2110298B1", "docdbApplication": "EP09250131 (A)",
              "publications": [{ "publication": "EP2110298A3", "status": "read", "citedCount": 5,
                                 "examinerCount": 4, "applicantCount": 1 }] },
        ],
        "documents": [
            { "document": "US4763957", "kinds": ["A"], "familyId": null, "cells": {
                "EP": { "text": "X cl. 13, A cl. 1,5,9", "citations": [
                    { "source": "ops_biblio", "citing": "EP2110298A3", "citedBy": "examiner",
                      "category": "X,A", "relevantClaims": "13", "phase": "national-search-report",
                      "categoryClaims": [{ "category": "X", "claims": "13" }, { "category": "A", "claims": "1,5,9" }] },
                ]},
                "US": { "text": "applicant", "citations": [
                    { "source": "ops_biblio", "citing": "US7722129B2", "citedBy": "applicant" },
                ]},
            }},
        ],
        "gaps": [
            { "office": "US", "member": "US8056987B2", "source": "uspto_enriched", "reason": "no_citation_record",
              "message": "no USPTO enriched-citation record for US8056987B2 (application 12756531)" },
        ],
    })
}

#[tokio::test]
async fn the_verb_prints_the_backend_tool_answer_when_the_registry_lists_it() {
    let server = MockServer::start().await;
    mount_registry(&server, &["get_family", "examiner_baseline"]).await;
    Mock::given(method("POST"))
        .and(path("/v1/tools/examiner_baseline"))
        .and(body_json(json!({ "publication": "EP2110298B1" })))
        .respond_with(ok("examiner_baseline", tool_answer()))
        .expect(2)
        .mount(&server)
        .await;
    // The local walk must not run.
    Mock::given(method("POST"))
        .and(path("/v1/tools/get_family"))
        .respond_with(ok("get_family", json!({})))
        .expect(0)
        .mount(&server)
        .await;

    let json_run = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "--json",
            "--verbose",
            "patent",
            "examiner-baseline",
            "EP2110298B1",
        ],
    )
    .await;
    let human_run = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &["patent", "examiner-baseline", "EP2110298B1"],
    )
    .await;
    let human = String::from_utf8_lossy(&human_run.stdout).to_string();
    assert_eq!(
        (
            json_run.status.success(),
            stdout_json(&json_run),
            String::from_utf8_lossy(&json_run.stderr).contains("backend tool examiner_baseline"),
            human_run.status.success(),
            human.contains("X cl. 13, A cl. 1,5,9"),
            human.contains("no USPTO enriched-citation record for US8056987B2"),
        ),
        (true, tool_answer(), true, true, true, true),
        "human output:\n{human}"
    );
}

#[tokio::test]
async fn the_verb_names_the_local_walk_in_verbose_output() {
    let server = MockServer::start().await;
    mount_registry_without_tool(&server).await;
    mount_tool(
        &server,
        "get_family",
        json!({ "patent_number": "EP1A1" }),
        ok(
            "get_family",
            json!({ "members": [{ "publication": "EP1A1" }] }),
        ),
    )
    .await;
    mount_tool(
        &server,
        "get_bibliography",
        json!({ "patent_number": "EP1A1" }),
        ok("get_bibliography", json!({ "citedReferences": [
            { "docId": "US1", "kind": "A", "citedBy": "examiner", "category": "X,A", "relevantClaims": "1",
              "categoryClaims": [{ "category": "X", "claims": "1" }, { "category": "A", "claims": "2" }] },
        ]})),
    )
    .await;

    let output = run_cli(
        &server.uri(),
        &[API_KEY_ENV],
        &[
            "--json",
            "--verbose",
            "patent",
            "examiner-baseline",
            "EP1A1",
        ],
    )
    .await;
    let body = stdout_json(&output);
    assert_eq!(
        (
            String::from_utf8_lossy(&output.stderr).contains("local family walk"),
            body["documents"][0]["cells"]["EP"]["text"].clone(),
        ),
        (true, json!("X cl. 1, A cl. 2")),
    );
}
