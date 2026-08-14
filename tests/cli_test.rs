use std::process::Command;

#[test]
fn exposes_uspto_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .args(["uspto", "--help"])
        .output()
        .expect("run flowleap uspto --help");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("USPTO Open Data Portal commands"));
    assert!(stdout.contains("search"));
    assert!(stdout.contains("continuity"));
}

#[test]
fn dry_run_succeeds_without_credentials() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env_remove("FLOWLEAP_API_KEY")
        .env_remove("FLOWLEAP_TOKEN")
        .args([
            "patent",
            "search",
            "--query",
            "ti=\"wireless charging\"",
            "--limit",
            "5",
            "--countries",
            "EP,WO",
            "--dry-run",
            "--output",
            "json",
        ])
        .output()
        .expect("run dry-run search");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "POST");
    assert_eq!(value["authenticated"], false);
    assert_eq!(
        value["url"],
        "https://api.flowleap.co/v1/tools/search_patents"
    );
    // The search_patents contract for EPO: { query (CQL), provider, range:
    // "start-end", countries?: [CC] }.
    assert_eq!(value["body"]["provider"], "epo_ops");
    assert_eq!(value["body"]["query"], "ti=\"wireless charging\"");
    assert_eq!(value["body"]["range"], "1-5");
    assert_eq!(value["body"]["countries"], serde_json::json!(["EP", "WO"]));
    assert!(value["body"].get("source").is_none());
    assert!(value["body"].get("limit").is_none());
}

#[test]
fn redacted_dry_run_preserves_shape_without_sensitive_values() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let query = "ta=\"confidential UV-C earbud charging case\"";
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_API_KEY")
        .env_remove("FLOWLEAP_TOKEN")
        .args([
            "--json",
            "--dry-run",
            "--dry-run-redacted",
            "patent",
            "search",
            "--query",
            query,
        ])
        .output()
        .expect("run redacted patent-search dry-run");

    assert!(output.status.success(), "redacted dry-run failed");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(!stdout.contains("earbud"));
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");
    assert_eq!(value["dryRun"], true);
    assert_eq!(value["dryRunRedacted"], true);
    assert_eq!(value["body"]["query"], "[REDACTED]");
}

#[test]
fn uspto_search_dry_run_uses_odp_request_shape() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env("FLOWLEAP_API_KEY", "fl_org_test_secret")
        .env_remove("FLOWLEAP_TOKEN")
        .args([
            "--json",
            "uspto",
            "search",
            "--query",
            "wireless charging",
            "--limit",
            "1",
            "--dry-run",
        ])
        .output()
        .expect("run uspto search dry-run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "POST");
    assert_eq!(value["authenticated"], true);
    assert_eq!(
        value["url"],
        "https://api.flowleap.co/v1/tools/search_patents"
    );
    assert_eq!(value["body"]["provider"], "uspto");
    assert_eq!(value["body"]["query"], "wireless charging");
    assert_eq!(value["body"]["limit"], 1);
    assert_eq!(value["body"]["offset"], 0);
    // The ODP wire spellings never leave the CLI — the tool takes flat params.
    assert!(value["body"].get("q").is_none());
    assert!(value["body"].get("pagination").is_none());
}

#[test]
fn exposes_agent_first_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .arg("--help")
        .output()
        .expect("run flowleap --help");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    for command in [
        "doctor",
        "api",
        "health",
        "uspto",
        "npl",
        "legal",
        "citation",
        "analytics",
        "ocr",
    ] {
        assert!(stdout.contains(command), "missing command {command}");
    }
}

#[test]
fn raw_request_dry_run_succeeds_without_credentials() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env_remove("FLOWLEAP_API_KEY")
        .env_remove("FLOWLEAP_TOKEN")
        .args([
            "--json",
            "api",
            "request",
            "post",
            "/v1/patent-search",
            "--body",
            r#"{"query":"solar","limit":1}"#,
            "--dry-run",
        ])
        .output()
        .expect("run raw request dry-run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "POST");
    assert_eq!(value["authenticated"], false);
    assert_eq!(value["body"]["query"], "solar");
}

#[test]
fn org_api_key_dry_run_is_authenticated_without_leaking_key() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env("FLOWLEAP_API_KEY", "fl_org_test_secret")
        .env_remove("FLOWLEAP_TOKEN")
        .args([
            "--json",
            "--verbose",
            "api",
            "request",
            "get",
            "/v1/health",
            "--dry-run",
        ])
        .output()
        .expect("run org-key dry-run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["dryRun"], true);
    assert_eq!(value["authenticated"], true);
    assert!(!stderr.contains("fl_org_test_secret"));
    assert!(!stdout.contains("fl_org_test_secret"));
}

#[test]
fn parse_errors_honor_json_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .args(["--json", "not-a-command"])
        .output()
        .expect("run invalid command");

    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["ok"], false);
    assert!(value["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unrecognized subcommand"));
}

#[test]
fn exposes_analytics_command_with_runnable_example() {
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .args(["analytics", "--help"])
        .output()
        .expect("run flowleap analytics --help");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("--keyword"));
    assert!(stdout.contains("--phrase"));
    assert!(stdout.contains("--assignee"));
    assert!(stdout.contains("--cpc"));
    assert!(stdout.contains("--ipc"));
    assert!(stdout.contains("--country"));
    assert!(stdout.contains("--date-from"));
    assert!(stdout.contains("--date-to"));
    assert!(stdout.contains("flowleap analytics --keyword battery --date-from 2015-01-01"));
    assert!(stdout.contains("deprecated free-form query"));
}

#[test]
fn analytics_dry_run_uses_structured_request_shape() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env_remove("FLOWLEAP_API_KEY")
        .env_remove("FLOWLEAP_TOKEN")
        .args([
            "--json",
            "analytics",
            "--keyword",
            "AI",
            "--keyword",
            "battery",
            "--phrase",
            "machine learning",
            "--assignee",
            "Siemens",
            "--cpc",
            "G06N",
            "--cpc",
            "H01M",
            "--ipc",
            "H04L",
            "--country",
            "US",
            "--date-from",
            "2020-01-01",
            "--date-to",
            "2025-12-31",
            "--dry-run",
        ])
        .output()
        .expect("run analytics dry-run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "POST");
    assert_eq!(
        value["url"],
        "https://api.flowleap.co/v1/tools/patent_analytics"
    );
    assert_eq!(
        value["body"]["keywords"],
        serde_json::json!(["AI", "battery"])
    );
    assert_eq!(
        value["body"]["phrases"],
        serde_json::json!(["machine learning"])
    );
    assert_eq!(value["body"]["assignee"], "Siemens");
    assert_eq!(value["body"]["cpc"], serde_json::json!(["G06N", "H01M"]));
    assert_eq!(value["body"]["ipc"], serde_json::json!(["H04L"]));
    assert_eq!(value["body"]["countryCode"], "US");
    assert_eq!(value["body"]["dateFrom"], "2020-01-01");
    assert_eq!(value["body"]["dateTo"], "2025-12-31");
    // The deprecated free-form parameter must never travel.
    assert!(value["body"].get("query").is_none());
}

#[test]
fn analytics_dry_run_omits_absent_criteria() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env_remove("FLOWLEAP_API_KEY")
        .env_remove("FLOWLEAP_TOKEN")
        .args(["--json", "analytics", "--keyword", "battery", "--dry-run"])
        .output()
        .expect("run analytics dry-run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(
        value["body"],
        serde_json::json!({ "keywords": ["battery"] })
    );
}

#[test]
fn analytics_rejects_missing_criteria_locally_in_json_mode() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env_remove("FLOWLEAP_API_KEY")
        .env_remove("FLOWLEAP_TOKEN")
        // An unroutable base URL proves the rejection happens before any
        // network call: reaching the backend would fail differently.
        .args(["--base-url", "http://127.0.0.1:1", "--json", "analytics"])
        .output()
        .expect("run analytics with no criteria");

    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["ok"], false);
    let message = value["error"]["message"].as_str().unwrap();
    assert!(message.contains("--keyword"));
    assert!(message.contains("--date-to"));
    assert!(message.contains("flowleap analytics --keyword battery"));
}

#[test]
fn analytics_rejects_missing_criteria_locally_in_human_mode() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env("FLOWLEAP_NO_UPDATE_CHECK", "1")
        .env_remove("FLOWLEAP_API_KEY")
        .env_remove("FLOWLEAP_TOKEN")
        .args(["--base-url", "http://127.0.0.1:1", "analytics"])
        .output()
        .expect("run analytics with no criteria");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr is utf8");
    assert!(stderr.contains("No analytics criteria given"));
    assert!(stderr.contains("--keyword"));
}

/// Canned /v1/patent-analytics success body served by the one-shot stub.
const ANALYTICS_STUB_RESPONSE: &str = r#"{"success":true,"searchDescription":"keywords: battery","request":{"keywords":["battery"]},"analytics":{"byYear":[{"year":2015,"count":1200},{"year":2016,"count":1500}],"byCountry":[{"country":"US","count":900},{"country":"CN","count":600}],"topAssignees":[{"assignee":"Acme Corp","count":42}],"topCPC":[{"cpc":"H01M","count":333}]}}"#;

/// Minimal one-shot HTTP stub: accepts a single request, replies 200 with the
/// given JSON body, and returns the raw request text for on-the-wire asserts.
fn spawn_one_shot_stub(response_body: &'static str) -> (String, std::thread::JoinHandle<String>) {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind stub");
    let base_url = format!("http://{}", listener.local_addr().expect("stub addr"));

    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept connection");
        let mut request = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = stream.read(&mut chunk).expect("read request");
            request.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&request).to_string();
            if let Some(header_end) = text.find("\r\n\r\n") {
                let content_length = text
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        if name.eq_ignore_ascii_case("content-length") {
                            value.trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                if request.len() >= header_end + 4 + content_length {
                    break;
                }
            }
            if n == 0 {
                break;
            }
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
        String::from_utf8_lossy(&request).to_string()
    });

    (base_url, handle)
}

#[test]
fn analytics_human_mode_renders_four_labeled_tables() {
    let (base_url, stub) = spawn_one_shot_stub(ANALYTICS_STUB_RESPONSE);

    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env("FLOWLEAP_API_KEY", "fl_pat_test_key")
        .env("FLOWLEAP_NO_UPDATE_CHECK", "1")
        .env_remove("FLOWLEAP_TOKEN")
        .args(["--base-url", &base_url, "analytics", "--keyword", "battery"])
        .output()
        .expect("run analytics against stub");

    let request = stub.join().expect("stub thread");
    assert!(request.starts_with("POST /v1/tools/patent_analytics HTTP/1.1"));
    assert!(request.contains(r#"{"keywords":["battery"]}"#));

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    for label in [
        "Filings by Year",
        "Filings by Country",
        "Top Assignees",
        "Top CPC Classes",
    ] {
        assert!(stdout.contains(label), "missing table label {label}");
    }
    assert!(stdout.contains("Search: keywords: battery"));
    // Real table cells, not a raw JSON dump.
    assert!(stdout.contains("2015"));
    assert!(stdout.contains("Acme Corp"));
    assert!(stdout.contains("H01M"));
    assert!(!stdout.contains("\"byYear\""));
}

#[test]
fn analytics_json_mode_emits_endpoint_envelope_untouched() {
    let (base_url, stub) = spawn_one_shot_stub(ANALYTICS_STUB_RESPONSE);

    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env("FLOWLEAP_API_KEY", "fl_pat_test_key")
        .env_remove("FLOWLEAP_TOKEN")
        .args([
            "--json",
            "--base-url",
            &base_url,
            "analytics",
            "--keyword",
            "battery",
        ])
        .output()
        .expect("run analytics against stub");

    stub.join().expect("stub thread");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["success"], true);
    assert_eq!(value["searchDescription"], "keywords: battery");
    assert_eq!(value["request"]["keywords"], serde_json::json!(["battery"]));
    assert_eq!(value["analytics"]["byYear"][0]["year"], 2015);
    assert_eq!(value["analytics"]["byCountry"][0]["country"], "US");
    assert_eq!(
        value["analytics"]["topAssignees"][0]["assignee"],
        "Acme Corp"
    );
    assert_eq!(value["analytics"]["topCPC"][0]["cpc"], "H01M");
}

#[test]
fn init_honors_json_flag() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .args(["--json", "init", "--base-url", "http://localhost:8000"])
        .output()
        .expect("run json init");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["ok"], true);
    assert_eq!(value["baseUrl"], "http://localhost:8000");
}

#[test]
fn ocr_url_dry_run_sends_url_field() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env_remove("FLOWLEAP_API_KEY")
        .env_remove("FLOWLEAP_TOKEN")
        .args(["--json", "ocr", "https://example.com/spec.pdf", "--dry-run"])
        .output()
        .expect("run ocr url dry-run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "POST");
    assert_eq!(value["url"], "https://api.flowleap.co/v1/tools/ocr");
    assert_eq!(value["body"]["url"], "https://example.com/spec.pdf");
    assert!(value["body"].get("file").is_none());
    assert!(value["body"].get("filename").is_none());
}

#[test]
fn ocr_local_file_dry_run_sends_base64_and_filename() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let file_path = temp_home.path().join("scan.png");
    std::fs::write(&file_path, b"fake png bytes").expect("write sample file");

    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .env_remove("FLOWLEAP_API_KEY")
        .env_remove("FLOWLEAP_TOKEN")
        .args([
            "--json",
            "ocr",
            file_path.to_str().expect("path is utf8"),
            "--dry-run",
        ])
        .output()
        .expect("run ocr file dry-run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["dryRun"], true);
    assert_eq!(value["method"], "POST");
    assert_eq!(value["url"], "https://api.flowleap.co/v1/tools/ocr");
    // base64("fake png bytes")
    assert_eq!(value["body"]["file"], "ZmFrZSBwbmcgYnl0ZXM=");
    assert_eq!(value["body"]["filename"], "scan.png");
    assert!(value["body"].get("url").is_none());
}

#[test]
fn ocr_rejects_unsupported_file_type_locally() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let file_path = temp_home.path().join("notes.txt");
    std::fs::write(&file_path, b"plain text").expect("write sample file");

    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .args([
            "--json",
            "ocr",
            file_path.to_str().expect("path is utf8"),
            "--dry-run",
        ])
        .output()
        .expect("run ocr unsupported-type");

    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["ok"], false);
    let message = value["error"]["message"].as_str().unwrap();
    assert!(message.contains("Unsupported file type 'txt'"));
    assert!(message.contains("pdf"));
}

#[test]
fn ocr_rejects_missing_file_locally() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .args(["--json", "ocr", "/nonexistent/never.pdf", "--dry-run"])
        .output()
        .expect("run ocr missing-file");

    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["ok"], false);
    assert!(value["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Cannot read /nonexistent/never.pdf"));
}

#[test]
fn ocr_rejects_oversized_file_locally() {
    let temp_home = tempfile::tempdir().expect("create temp home");
    let file_path = temp_home.path().join("huge.pdf");
    // Sparse file: over the 36 MB limit without materializing the bytes.
    let file = std::fs::File::create(&file_path).expect("create sparse file");
    file.set_len(37 * 1024 * 1024).expect("set sparse length");

    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .env("HOME", temp_home.path())
        .env("XDG_CONFIG_HOME", temp_home.path().join(".config"))
        .env_remove("FLOWLEAP_BASE_URL")
        .args([
            "--json",
            "ocr",
            file_path.to_str().expect("path is utf8"),
            "--dry-run",
        ])
        .output()
        .expect("run ocr oversized");

    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is json");

    assert_eq!(value["ok"], false);
    assert!(value["error"]["message"]
        .as_str()
        .unwrap()
        .contains("exceeds the 36 MB OCR upload limit"));
}

#[test]
fn ocr_help_includes_examples() {
    let output = Command::new(env!("CARGO_BIN_EXE_flowleap"))
        .args(["ocr", "--help"])
        .output()
        .expect("run --help");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("Examples:"),
        "ocr --help is missing examples"
    );
    assert!(
        stdout.contains("flowleap ocr"),
        "ocr --help examples don't show an invocation"
    );
}
