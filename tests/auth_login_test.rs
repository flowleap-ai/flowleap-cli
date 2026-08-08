//! Structured device-flow login (issue #41): `flowleap --json auth login`
//! must emit a blocking NDJSON event stream on stdout — the
//! `device_authorization` event first (URL + user code for the agent to
//! relay), then exactly one terminal event (`authorized` exit 0 / `failed`
//! nonzero) — with nothing but NDJSON on stdout and the session token stored
//! exactly as the human flow stores it. Driven through the real binary
//! against a wiremock backend mocking the two unauthenticated device
//! endpoints.

mod support;

use std::process::Output;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use serde_json::json;
use support::run_cli;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Current Unix time in seconds, for building JWTs relative to "now".
fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs() as i64
}

/// Build an unsigned, three-segment `header.payload.signature` JWT-shaped
/// token carrying `exp` (and, when given, `iat`). The store-time TTL guard
/// (flowleap-backend#254) never checks a signature — it only reads these
/// claims — so a fake signature segment is enough to exercise it.
fn make_jwt(exp: i64, iat: Option<i64>) -> String {
    let encode = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let header = encode.encode(r#"{"alg":"none","typ":"JWT"}"#);
    let mut claims = json!({ "exp": exp });
    if let Some(iat) = iat {
        claims["iat"] = json!(iat);
    }
    let payload = encode.encode(claims.to_string());
    format!("{header}.{payload}.fake-signature")
}

/// Mount `POST /oauth/device` answering with a device authorization. The
/// `interval` drives the CLI's poll sleep — 0 keeps tests fast.
async fn mount_device_authorization(server: &MockServer, interval: u64, expires_in: u64) {
    Mock::given(method("POST"))
        .and(path("/oauth/device"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "device_code": "dev-code-123",
            "user_code": "ABCD-1234",
            "verification_uri": "https://flowleap.co/device",
            "verification_uri_complete": "https://flowleap.co/device?code=ABCD-1234",
            "expires_in": expires_in,
            "interval": interval,
        })))
        .mount(server)
        .await;
}

/// Mount `POST /oauth/device/token` answering `authorization_pending` exactly
/// once, so the next mounted token mock serves the terminal response. Proves
/// the process blocks through polling rather than stopping at the first poll.
async fn mount_pending_once(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/oauth/device/token"))
        .respond_with(
            ResponseTemplate::new(400).set_body_json(json!({ "error": "authorization_pending" })),
        )
        .up_to_n_times(1)
        .mount(server)
        .await;
}

/// Mount a terminal `POST /oauth/device/token` response.
async fn mount_token_response(server: &MockServer, template: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/oauth/device/token"))
        .respond_with(template)
        .mount(server)
        .await;
}

/// Parse stdout as NDJSON: every line must be a JSON object, so any stray
/// human-formatted output fails the test.
fn ndjson_events(output: &Output) -> Vec<serde_json::Value> {
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is utf8");
    stdout
        .lines()
        .map(|line| {
            serde_json::from_str(line).unwrap_or_else(|_| {
                panic!("stdout line was not JSON: {line:?}\nfull stdout: {stdout}")
            })
        })
        .collect()
}

/// pending → authorized: the first NDJSON line carries the URL and user code,
/// the terminal event is `authorized` with exit 0, and the session token is
/// stored in credentials.toml exactly as the human flow stores it.
#[tokio::test]
async fn json_login_pending_then_authorized_streams_events_and_stores_token() {
    let server = MockServer::start().await;
    mount_device_authorization(&server, 0, 300).await;
    mount_pending_once(&server).await;
    mount_token_response(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({ "access_token": "jwt-session-token" })),
    )
    .await;

    // Own the HOME so the credentials file survives the run for inspection.
    let home = tempfile::tempdir().expect("create temp home");
    let home_str = home.path().to_str().expect("utf8 home").to_string();
    let xdg = home.path().join(".config");
    let xdg_str = xdg.to_str().expect("utf8 xdg").to_string();

    let output = run_cli(
        &server.uri(),
        &[("HOME", &home_str), ("XDG_CONFIG_HOME", &xdg_str)],
        &["--json", "auth", "login"],
    )
    .await;

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let events = ndjson_events(&output);
    assert_eq!(events.len(), 2, "exactly two NDJSON events: {events:?}");
    assert_eq!(events[0]["event"], "device_authorization");
    assert_eq!(events[0]["verification_uri"], "https://flowleap.co/device");
    assert_eq!(
        events[0]["verification_uri_complete"],
        "https://flowleap.co/device?code=ABCD-1234"
    );
    assert_eq!(events[0]["user_code"], "ABCD-1234");
    assert_eq!(events[0]["expires_in"], 300);
    assert_eq!(events[0]["interval"], 0);
    assert_eq!(events[1], json!({ "event": "authorized", "stored": true }));

    // Session token stored where the human flow stores it.
    let credentials_path = if cfg!(target_os = "macos") {
        home.path()
            .join("Library/Application Support/flowleap/credentials.toml")
    } else {
        xdg.join("flowleap/credentials.toml")
    };
    let credentials =
        std::fs::read_to_string(&credentials_path).expect("credentials.toml was written");
    assert!(
        credentials.contains("jwt-session-token"),
        "stored credentials must carry the session token: {credentials}"
    );
}

/// pending → access_denied: terminal event is `failed` with a denial error
/// and the documented generic-failure exit code (1).
#[tokio::test]
async fn json_login_denied_emits_failed_event_and_exits_nonzero() {
    let server = MockServer::start().await;
    mount_device_authorization(&server, 0, 300).await;
    mount_pending_once(&server).await;
    mount_token_response(
        &server,
        ResponseTemplate::new(400).set_body_json(json!({ "error": "access_denied" })),
    )
    .await;

    let output = run_cli(&server.uri(), &[], &["--json", "auth", "login"]).await;

    assert_eq!(output.status.code(), Some(1));
    let events = ndjson_events(&output);
    assert_eq!(events.len(), 2, "exactly two NDJSON events: {events:?}");
    assert_eq!(events[0]["event"], "device_authorization");
    assert_eq!(events[0]["user_code"], "ABCD-1234");
    assert_eq!(events[1]["event"], "failed");
    assert!(
        events[1]["error"]
            .as_str()
            .is_some_and(|e| e.contains("denied")),
        "failed event must describe the denial: {}",
        events[1]
    );
}

/// pending → expired_token: terminal event is `failed` with an expiry error
/// and the documented generic-failure exit code (1).
#[tokio::test]
async fn json_login_expired_emits_failed_event_and_exits_nonzero() {
    let server = MockServer::start().await;
    mount_device_authorization(&server, 0, 300).await;
    mount_pending_once(&server).await;
    mount_token_response(
        &server,
        ResponseTemplate::new(400).set_body_json(json!({ "error": "expired_token" })),
    )
    .await;

    let output = run_cli(&server.uri(), &[], &["--json", "auth", "login"]).await;

    assert_eq!(output.status.code(), Some(1));
    let events = ndjson_events(&output);
    assert_eq!(events.len(), 2, "exactly two NDJSON events: {events:?}");
    assert_eq!(events[0]["event"], "device_authorization");
    assert_eq!(events[1]["event"], "failed");
    assert!(
        events[1]["error"]
            .as_str()
            .is_some_and(|e| e.contains("expired")),
        "failed event must describe the expiry: {}",
        events[1]
    );
}

/// Store-time TTL guard (flowleap-backend#254): the device-approval endpoint
/// has, on occasion, echoed back a ~60s default Clerk session token instead
/// of the long-lived flowleap-template token. A JWT whose `exp` claim shows
/// that short a lifetime must be refused loudly — exit code 3 (auth
/// required, since the run ends unauthenticated), a `failed` event naming
/// the actual lifetime and the known bug, and nothing written to
/// credentials.toml.
#[tokio::test]
async fn json_login_short_lived_jwt_is_refused_and_nothing_is_stored() {
    let server = MockServer::start().await;
    mount_device_authorization(&server, 0, 300).await;
    mount_pending_once(&server).await;
    let now = now_secs();
    let short_lived = make_jwt(now + 60, Some(now));
    mount_token_response(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({ "access_token": short_lived })),
    )
    .await;

    let home = tempfile::tempdir().expect("create temp home");
    let home_str = home.path().to_str().expect("utf8 home").to_string();
    let xdg = home.path().join(".config");
    let xdg_str = xdg.to_str().expect("utf8 xdg").to_string();

    let output = run_cli(
        &server.uri(),
        &[("HOME", &home_str), ("XDG_CONFIG_HOME", &xdg_str)],
        &["--json", "auth", "login"],
    )
    .await;

    assert_eq!(
        output.status.code(),
        Some(3),
        "auth-required exit code — the run ends unauthenticated; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let events = ndjson_events(&output);
    assert_eq!(events.len(), 2, "exactly two NDJSON events: {events:?}");
    assert_eq!(events[0]["event"], "device_authorization");
    assert_eq!(events[1]["event"], "failed");
    let error = events[1]["error"].as_str().expect("failed event has error");
    assert!(
        error.contains("flowleap-backend#254"),
        "refusal must name the known bug: {error}"
    );
    assert!(
        error.contains("60s"),
        "refusal must state the actual lifetime found: {error}"
    );
    assert!(
        error.to_lowercase().contains("flowleap-template"),
        "refusal must say the server must deliver the long-lived flowleap-template token: {error}"
    );

    let credentials_path = if cfg!(target_os = "macos") {
        home.path()
            .join("Library/Application Support/flowleap/credentials.toml")
    } else {
        xdg.join("flowleap/credentials.toml")
    };
    assert!(
        !credentials_path.exists(),
        "a refused token must never reach disk"
    );
}

/// The counterpart to the short-lived case: a JWT with a comfortably long
/// `exp` is stored exactly as before — the guard only blocks tokens it can
/// positively prove are about to die.
#[tokio::test]
async fn json_login_long_lived_jwt_is_stored_as_today() {
    let server = MockServer::start().await;
    mount_device_authorization(&server, 0, 300).await;
    mount_pending_once(&server).await;
    let now = now_secs();
    let long_lived = make_jwt(now + 86_400, Some(now));
    mount_token_response(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({ "access_token": long_lived.clone() })),
    )
    .await;

    let home = tempfile::tempdir().expect("create temp home");
    let home_str = home.path().to_str().expect("utf8 home").to_string();
    let xdg = home.path().join(".config");
    let xdg_str = xdg.to_str().expect("utf8 xdg").to_string();

    let output = run_cli(
        &server.uri(),
        &[("HOME", &home_str), ("XDG_CONFIG_HOME", &xdg_str)],
        &["--json", "auth", "login"],
    )
    .await;

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let events = ndjson_events(&output);
    assert_eq!(events[1], json!({ "event": "authorized", "stored": true }));

    let credentials_path = if cfg!(target_os = "macos") {
        home.path()
            .join("Library/Application Support/flowleap/credentials.toml")
    } else {
        xdg.join("flowleap/credentials.toml")
    };
    let credentials =
        std::fs::read_to_string(&credentials_path).expect("credentials.toml was written");
    assert!(
        credentials.contains(&long_lived),
        "stored credentials must carry the long-lived token: {credentials}"
    );
}

/// An opaque, non-JWT `access_token` (the shape a personal API token or any
/// non-Clerk credential would take) must be stored without complaint — the
/// guard only fires when it can positively decode a short `exp`, and fails
/// open on anything it can't parse as a three-segment JWT.
#[tokio::test]
async fn json_login_opaque_non_jwt_token_is_stored_without_complaint() {
    let server = MockServer::start().await;
    mount_device_authorization(&server, 0, 300).await;
    mount_pending_once(&server).await;
    let opaque = "opaque-non-jwt-access-token-no-dots-here";
    mount_token_response(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({ "access_token": opaque })),
    )
    .await;

    let home = tempfile::tempdir().expect("create temp home");
    let home_str = home.path().to_str().expect("utf8 home").to_string();
    let xdg = home.path().join(".config");
    let xdg_str = xdg.to_str().expect("utf8 xdg").to_string();

    let output = run_cli(
        &server.uri(),
        &[("HOME", &home_str), ("XDG_CONFIG_HOME", &xdg_str)],
        &["--json", "auth", "login"],
    )
    .await;

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let events = ndjson_events(&output);
    assert_eq!(events[1], json!({ "event": "authorized", "stored": true }));

    let credentials_path = if cfg!(target_os = "macos") {
        home.path()
            .join("Library/Application Support/flowleap/credentials.toml")
    } else {
        xdg.join("flowleap/credentials.toml")
    };
    let credentials =
        std::fs::read_to_string(&credentials_path).expect("credentials.toml was written");
    assert!(
        credentials.contains(opaque),
        "stored credentials must carry the opaque token: {credentials}"
    );
}
