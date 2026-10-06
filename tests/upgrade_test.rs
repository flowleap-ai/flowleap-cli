//! `flowleap upgrade` integration tests (issue #156).
//!
//! The self-update path is exercised end-to-end against a *fixture* GitHub
//! release layout served by wiremock: a `releases/latest` API response, the
//! platform asset bytes, and a matching `checksums.txt`. To avoid clobbering
//! the test binary itself, the real binary is copied into a temp dir and that
//! copy is what self-updates — so the assertion is that the copy's bytes were
//! atomically replaced by the verified fixture bytes.
//!
//! Endpoint redirection uses the same env-override seam production exposes for
//! enterprise mirrors (`FLOWLEAP_RELEASES_API_URL` /
//! `FLOWLEAP_RELEASES_DOWNLOAD_BASE`), mirroring how the HTTP tests use
//! `FLOWLEAP_TEST_RESOLVE`.

use std::fs;
use std::process::{Command, Stdio};

use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Release asset name for the host platform (mirrors the production mapping).
fn platform_asset() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        "windows" => "windows",
        other => panic!("unsupported test OS: {other}"),
    };
    let suffix = if os == "windows" { ".exe" } else { "" };
    format!("flowleap-{os}-{}{suffix}", std::env::consts::ARCH)
}

/// sha256 of `bytes` via the system tooling install.sh itself relies on, so
/// the fixture `checksums.txt` matches what the binary computes internally.
fn sha256_hex(bytes: &[u8]) -> String {
    use std::io::Write;
    for (bin, args) in [("sha256sum", &[][..]), ("shasum", &["-a", "256"][..])] {
        let Ok(mut child) = Command::new(bin)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        else {
            continue;
        };
        {
            let mut stdin = child.stdin.take().expect("child stdin");
            stdin.write_all(bytes).expect("write to sha tool");
        }
        let out = child.wait_with_output().expect("sha tool output");
        if out.status.success() {
            let text = String::from_utf8(out.stdout).expect("sha tool utf8");
            return text
                .split_whitespace()
                .next()
                .expect("sha tool hash field")
                .to_string();
        }
    }
    panic!("no sha256 tool (sha256sum / shasum) available on this host");
}

fn exe_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    }
}

/// Mount a fixture release: `releases/latest` → `tag`, plus the asset bytes
/// and a matching `checksums.txt` under `/dl/<tag>/…`.
async fn mount_release(server: &MockServer, tag: &str, asset: &str, bytes: &[u8]) {
    Mock::given(method("GET"))
        .and(path("/releases/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "tag_name": tag })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/dl/{tag}/{asset}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.to_vec()))
        .mount(server)
        .await;
    let checksums = format!("{}  {asset}\n", sha256_hex(bytes));
    Mock::given(method("GET"))
        .and(path(format!("/dl/{tag}/checksums.txt")))
        .respond_with(ResponseTemplate::new(200).set_body_string(checksums))
        .mount(server)
        .await;
}

#[tokio::test]
async fn raw_binary_self_updates_with_checksum_verification() {
    let server = MockServer::start().await;
    let asset = platform_asset();
    let new_bytes = b"NEW-FLOWLEAP-BINARY-v99.0.0\n".to_vec();
    mount_release(&server, "v99.0.0", &asset, &new_bytes).await;

    // Copy the built binary into a temp dir; the copy is a raw-binary install
    // (path has no node_modules/Cellar/.cargo marker) and is what self-updates.
    let dir = tempfile::tempdir().expect("temp dir");
    let exe = dir.path().join(exe_name("flowleap"));
    fs::copy(env!("CARGO_BIN_EXE_flowleap"), &exe).expect("copy binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    let home = tempfile::tempdir().expect("temp home");
    let output = tokio::task::spawn_blocking({
        let exe = exe.clone();
        let uri = server.uri();
        let home = home.path().to_path_buf();
        move || {
            Command::new(&exe)
                .env("HOME", &home)
                .env("XDG_CONFIG_HOME", home.join(".config"))
                .env("FLOWLEAP_NO_UPDATE_CHECK", "1")
                .env(
                    "FLOWLEAP_RELEASES_API_URL",
                    format!("{uri}/releases/latest"),
                )
                .env("FLOWLEAP_RELEASES_DOWNLOAD_BASE", format!("{uri}/dl"))
                .env_remove("FLOWLEAP_TOKEN")
                .env_remove("FLOWLEAP_API_KEY")
                .arg("upgrade")
                .output()
                .expect("run flowleap upgrade")
        }
    })
    .await
    .expect("join upgrade subprocess");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "upgrade did not succeed\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("sha256 verified"), "stdout: {stdout}");
    assert!(stdout.contains("Upgraded flowleap"), "stdout: {stdout}");
    // The atomic swap replaced the on-disk binary with exactly the fixture.
    assert_eq!(fs::read(&exe).expect("read swapped binary"), new_bytes);
}

#[tokio::test]
async fn check_json_reports_channel_and_versions_without_acting() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/releases/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "tag_name": "v99.0.0" })))
        .mount(&server)
        .await;

    // Run the built binary directly (target/debug → raw-binary channel); the
    // download base is left unmounted, proving --check never fetches an asset.
    let home = tempfile::tempdir().expect("temp home");
    let output = tokio::task::spawn_blocking({
        let uri = server.uri();
        let home = home.path().to_path_buf();
        move || {
            Command::new(env!("CARGO_BIN_EXE_flowleap"))
                .env("HOME", &home)
                .env("XDG_CONFIG_HOME", home.join(".config"))
                .env("FLOWLEAP_NO_UPDATE_CHECK", "1")
                .env(
                    "FLOWLEAP_RELEASES_API_URL",
                    format!("{uri}/releases/latest"),
                )
                .env_remove("FLOWLEAP_TOKEN")
                .env_remove("FLOWLEAP_API_KEY")
                .args(["upgrade", "--check", "--json"])
                .output()
                .expect("run flowleap upgrade --check")
        }
    })
    .await
    .expect("join check subprocess");

    assert!(output.status.success());
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("check output is JSON");
    assert_eq!(value["channel"], "raw-binary");
    assert_eq!(value["latestVersion"], "99.0.0");
    assert_eq!(value["updateAvailable"], true);
    assert!(value["command"]
        .as_str()
        .expect("command string")
        .contains("install.sh"));
}
