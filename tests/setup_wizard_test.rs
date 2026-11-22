//! Setup wizard non-TTY guard (issue #42): the interactive wizard is
//! human-only, so `flowleap setup` with piped stdin (no TTY) must refuse with
//! the non-interactive alternatives and exit nonzero. The interactive flow
//! itself stays untested here — dialoguer needs a real TTY (accepted in the
//! PRD, issue #40).

mod support;

use support::run_cli;
use wiremock::MockServer;

/// Without a TTY, `flowleap setup` refuses before touching the backend,
/// names the non-interactive alternatives, and exits 1.
#[tokio::test]
async fn setup_without_tty_refuses_with_alternatives() {
    // No mocks mounted: the guard fires before any request is made.
    let server = MockServer::start().await;

    let output = run_cli(&server.uri(), &[], &["setup"]).await;

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf8");
    assert!(
        stderr.contains("Interactive setup needs a terminal"),
        "expected the TTY refusal on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("flowleap keys set epo") && stderr.contains("flowleap keys set uspto"),
        "expected the non-interactive `keys set` alternatives, got: {stderr}"
    );
    assert!(
        stderr.contains("Optional: add your own Patent-Data Keys"),
        "the key step is optional, got: {stderr}"
    );
    assert!(
        stderr.contains("FLOWLEAP_EPO_KEY")
            && stderr.contains("FLOWLEAP_EPO_SECRET")
            && stderr.contains("FLOWLEAP_USPTO_KEY"),
        "expected the env-var alternatives, got: {stderr}"
    );
}
