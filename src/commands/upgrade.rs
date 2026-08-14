//! `flowleap upgrade` (alias `update`): channel-aware self-update.
//!
//! Any-harness users and agents shouldn't need to know which channel installed
//! the binary. This command detects the install channel from the *running*
//! binary's canonical path and acts accordingly:
//!
//! - **raw binary** (install.sh → /usr/local/bin or custom): self-updates
//!   exactly like the first-run downloader — fetch the platform release asset,
//!   verify its sha256 against `checksums.txt`, atomically swap over self,
//!   re-exec `--version`.
//! - **npm**: runs `npm i -g flowleap@<latest> --prefer-online`, pinned to the
//!   version it just resolved — the `latest` dist-tag can be stale in npm's
//!   packument cache for minutes after a release and would silently reinstall
//!   the previous version (#38). This is its final act (the running wrapper
//!   may be replaced mid-update, so nothing after it depends on self).
//! - **cargo**: prints `cargo install --git … --force` (never silently kicks
//!   off a full from-source recompile).
//! - **Homebrew**: detected for completeness, but **no tap is published**, so
//!   nothing installs through this channel today. Do not advertise it.
//!
//! Installed skill files are separate content and must be refreshed by the
//! *new* binary — refreshing them from the running (old) process would write
//! the old content back. Only the raw-binary channel can do that in-band,
//! because it is the one channel where the upgraded binary is at a path this
//! process already knows; the others print the follow-up command.
//!
//! `--check` (and `--json`/`--dry-run`) report `{ channel, currentVersion,
//! latestVersion, updateAvailable, command }` with no side effects, so agents
//! can branch on the result.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context as _, Result};
use serde_json::json;

use crate::client::Context;
use crate::output;
use crate::update::is_newer;

const REPO: &str = "flowleap-ai/flowleap-cli";
const NPM_REGISTRY_URL: &str = "https://registry.npmjs.org/flowleap/latest";
const GITHUB_LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/flowleap-ai/flowleap-cli/releases/latest";
const INSTALL_SH_URL: &str =
    "https://raw.githubusercontent.com/flowleap-ai/flowleap-cli/main/install.sh";
const CARGO_INSTALL_CMD: &str =
    "cargo install --git https://github.com/flowleap-ai/flowleap-cli --force";
/// Binary downloads can be several MB on a slow link; give them a much longer
/// ceiling than the 30s default the shared client uses for JSON API calls.
const DOWNLOAD_TIMEOUT_SECS: u64 = 300;

#[derive(clap::Args)]
pub struct UpgradeArgs {
    /// Report the install channel and available version without upgrading
    #[arg(long)]
    pub check: bool,
}

/// How this binary was installed, which decides how `upgrade` acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// npm global wrapper (`npm i -g flowleap`).
    Npm,
    /// Homebrew cellar.
    Homebrew,
    /// `cargo install` into ~/.cargo/bin.
    Cargo,
    /// install.sh / manual download to /usr/local/bin or elsewhere.
    RawBinary,
}

impl Channel {
    /// Stable identifier for JSON output and matching in tests.
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Npm => "npm",
            Channel::Homebrew => "homebrew",
            Channel::Cargo => "cargo",
            Channel::RawBinary => "raw-binary",
        }
    }

    /// The shell command that upgrades this channel — surfaced by `--check`
    /// so a caller can act without invoking `flowleap upgrade` itself.
    /// When the resolved target version is known, the npm command pins it
    /// instead of trusting the `latest` dist-tag: npm resolves dist-tags from
    /// its packument cache (fresh for ~5 minutes), which right after a release
    /// can still point at the previous version and silently reinstall it
    /// (#38). `--prefer-online` forces a staleness check so the pinned
    /// version resolves even against a warm cache.
    pub fn command_for(self, latest: Option<&str>) -> String {
        match self {
            Channel::Npm => match latest {
                Some(v) => format!("npm i -g flowleap@{v} --prefer-online"),
                None => "npm i -g flowleap@latest".to_string(),
            },
            Channel::Homebrew => "brew upgrade flowleap".to_string(),
            Channel::Cargo => CARGO_INSTALL_CMD.to_string(),
            Channel::RawBinary => format!("curl -fsSL {INSTALL_SH_URL} | sh"),
        }
    }

    /// `command_for` without a resolved version — the generic fallback shown
    /// when the latest version is unknown.
    pub fn command(self) -> String {
        self.command_for(None)
    }
}

/// Detect the install channel from the running binary's path. Order matters:
/// an npm wrapper binary can live under a cache dir that would otherwise look
/// standalone, so its markers are checked first; cargo is checked before the
/// raw-binary fallback so a `~/.cargo/bin` binary is not self-updated with a
/// release asset that would desync from `cargo install`.
pub fn detect_channel(exe: &Path) -> Channel {
    let has_component = |name: &str| exe.components().any(|c| c.as_os_str() == name);

    // npm: the wrapper downloads the native binary as `flowleap-native[.exe]`,
    // either inside the package's node_modules bin dir or the per-user cache.
    let npm_wrapper_name = exe
        .file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s == "flowleap-native");
    if npm_wrapper_name || has_component("node_modules") {
        return Channel::Npm;
    }
    // Homebrew installs land under a `Cellar` directory (canonicalize resolves
    // the /opt/homebrew/bin symlink to its cellar target).
    if has_component("Cellar") {
        return Channel::Homebrew;
    }
    // cargo install writes to ~/.cargo/bin.
    if has_component(".cargo") {
        return Channel::Cargo;
    }
    Channel::RawBinary
}

/// Channel of the currently running binary, resolved through its canonical
/// path so symlinks (Homebrew's bin shim, a hand-linked /usr/local/bin entry)
/// point at the real install location. Falls back to the raw-binary channel
/// if the path can't be resolved.
pub fn current_channel() -> Channel {
    current_exe()
        .map(|exe| detect_channel(&exe))
        .unwrap_or(Channel::RawBinary)
}

/// Canonical path of the running executable (real path, symlinks resolved).
fn current_exe() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("cannot locate the running executable")?;
    Ok(std::fs::canonicalize(&exe).unwrap_or(exe))
}

pub async fn run(ctx: &Context, args: UpgradeArgs) -> Result<()> {
    let channel = current_channel();
    let current = env!("CARGO_PKG_VERSION");

    // Report-only mode: --check, machine-readable output, and --dry-run must
    // never touch the filesystem or spawn a package manager.
    let report_only = args.check || ctx.output_format == "json" || ctx.dry_run;
    if report_only {
        return report(ctx, channel, current).await;
    }

    let latest = fetch_latest(ctx, channel).await?;
    if !is_newer(&latest, current) {
        println!("flowleap is already up to date (v{current}).");
        return Ok(());
    }

    println!(
        "Upgrading flowleap {current} -> {latest} ({} install)…",
        channel.as_str()
    );
    match channel {
        Channel::RawBinary => self_update_raw(ctx, current, &latest).await?,
        Channel::Npm => run_npm_upgrade(current, &latest)?,
        Channel::Homebrew => run_brew_upgrade(current, &latest)?,
        Channel::Cargo => print_cargo_upgrade(current, &latest),
    }
    Ok(())
}

/// Print the no-side-effect channel + version report.
async fn report(ctx: &Context, channel: Channel, current: &str) -> Result<()> {
    let latest = fetch_latest(ctx, channel).await.ok();
    let update_available = latest.as_deref().map(|l| is_newer(l, current));
    let command = channel.command_for(latest.as_deref());
    let value = json!({
        "channel": channel.as_str(),
        "currentVersion": current,
        "latestVersion": latest,
        "updateAvailable": update_available,
        "command": command,
    });
    if ctx.output_format == "json" {
        output::print_json(&value);
    } else {
        match (&latest, update_available) {
            (Some(l), Some(true)) => {
                println!("flowleap {l} is available (you have {current}).");
                println!("Channel: {}", channel.as_str());
                println!("Upgrade: flowleap upgrade  (or: {command})");
            }
            (Some(l), _) => {
                println!(
                    "flowleap is up to date (v{current}, latest v{l}, {} install).",
                    channel.as_str()
                );
            }
            (None, _) => {
                println!("flowleap v{current} ({} install).", channel.as_str());
                println!("Could not determine the latest version. Upgrade with: {command}");
            }
        }
    }
    Ok(())
}

/// Latest published version for the channel's source of truth: npm's registry
/// for the npm wrapper, the GitHub release tag for everything else. Both track
/// the same tag at steady state, so `--check` never disagrees with the daily
/// update notice.
async fn fetch_latest(ctx: &Context, channel: Channel) -> Result<String> {
    match channel {
        Channel::Npm => fetch_npm_latest(ctx).await,
        _ => fetch_github_latest(ctx).await,
    }
}

/// npm registry URL for the version check. Overridable via
/// `FLOWLEAP_NPM_REGISTRY_URL` for enterprise mirrors and integration tests.
fn npm_registry_url() -> String {
    std::env::var("FLOWLEAP_NPM_REGISTRY_URL").unwrap_or_else(|_| NPM_REGISTRY_URL.to_string())
}

/// GitHub releases/latest API URL. Overridable via `FLOWLEAP_RELEASES_API_URL`
/// for enterprise mirrors and integration tests.
fn releases_api_url() -> String {
    std::env::var("FLOWLEAP_RELEASES_API_URL")
        .unwrap_or_else(|_| GITHUB_LATEST_RELEASE_URL.to_string())
}

/// Base URL under which `<tag>/<asset>` and `<tag>/checksums.txt` are fetched
/// for the raw-binary self-update. Overridable via
/// `FLOWLEAP_RELEASES_DOWNLOAD_BASE` for enterprise mirrors and integration
/// tests.
fn releases_download_base() -> String {
    std::env::var("FLOWLEAP_RELEASES_DOWNLOAD_BASE")
        .unwrap_or_else(|_| format!("https://github.com/{REPO}/releases/download"))
}

async fn fetch_npm_latest(ctx: &Context) -> Result<String> {
    let body: serde_json::Value = ctx
        .http
        .get(npm_registry_url())
        .send()
        .await
        .context("cannot reach the npm registry")?
        .error_for_status()
        .context("npm registry returned an error")?
        .json()
        .await
        .context("npm registry response was not JSON")?;
    body.get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .context("npm registry response had no version")
}

async fn fetch_github_latest(ctx: &Context) -> Result<String> {
    let body: serde_json::Value = ctx
        .http
        .get(releases_api_url())
        .send()
        .await
        .context("cannot reach the GitHub releases API")?
        .error_for_status()
        .context("GitHub releases API returned an error")?
        .json()
        .await
        .context("GitHub releases response was not JSON")?;
    body.get("tag_name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim_start_matches('v').to_string())
        .context("GitHub releases response had no tag_name")
}

/// Release asset name for the running platform, mirroring the npm downloader's
/// PLATFORM_MAP and install.sh's `flowleap-<os>-<arch>` layout.
fn platform_asset() -> Result<String> {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        "windows" => "windows",
        other => bail!("unsupported OS for self-update: {other}"),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => bail!("unsupported architecture for self-update: {other}"),
    };
    let suffix = if os == "windows" { ".exe" } else { "" };
    Ok(format!("flowleap-{os}-{arch}{suffix}"))
}

/// Self-update a raw-binary install: download the platform asset, verify its
/// sha256 against the release checksums, and atomically swap it over self.
async fn self_update_raw(ctx: &Context, current: &str, latest: &str) -> Result<()> {
    let exe = current_exe()?;
    let dir = exe
        .parent()
        .context("running executable has no parent directory")?;
    let asset = platform_asset()?;
    let tag = format!("v{latest}");
    let base = format!("{}/{tag}", releases_download_base());

    let bytes = download_bytes(ctx, &format!("{base}/{asset}")).await?;
    let checksums = download_text(ctx, &format!("{base}/checksums.txt")).await?;
    let expected = expected_checksum(&checksums, &asset)?;
    let actual = sha256_hex(&bytes);
    if actual != expected {
        bail!(
            "sha256 mismatch for {asset}: expected {expected}, got {actual} — refusing to install"
        );
    }
    println!("sha256 verified.");

    // Write into the same directory (same filesystem) so the swap is a real
    // atomic rename, not a cross-device copy. Preserve the current binary's
    // permission bits so the replacement stays executable.
    let tmp = dir.join(format!(".flowleap-upgrade-{}.tmp", std::process::id()));
    write_atomic_target(&exe, &tmp, &bytes).with_context(|| {
        format!(
            "cannot write the new binary next to {} — is the install directory writable? \
             If it needs root, re-run install.sh: curl -fsSL {INSTALL_SH_URL} | sh",
            exe.display()
        )
    })?;
    std::fs::rename(&tmp, &exe).with_context(|| {
        let _ = std::fs::remove_file(&tmp);
        format!("cannot replace {} with the new binary", exe.display())
    })?;

    println!("Upgraded flowleap {current} -> {latest}.");
    verify_reexec(&exe);
    refresh_skills_with(&exe);
    Ok(())
}

/// Refresh recorded skill installs by running `skills update` on the binary at
/// `exe` — which, after the swap, is the NEW build. Doing this in-process would
/// write the old bundled content back over the files it just upgraded past.
/// Best-effort: a failure here never fails the upgrade, it only falls back to
/// telling the user to run the command.
fn refresh_skills_with(exe: &Path) {
    match Command::new(exe).args(["skills", "update"]).output() {
        Ok(out) if out.status.success() => {
            println!("Refreshed installed agent skills to the new version.")
        }
        _ => {
            eprintln!(
                "note: could not refresh installed skills automatically — run \
                 `flowleap skills update`. Stale skill files teach retired commands."
            );
        }
    }
}

/// Write `bytes` to `tmp`, copying the executable bits from `exe` (or 0o755
/// when its mode can't be read). Kept separate so the permission handling is
/// unix-gated in one place.
fn write_atomic_target(exe: &Path, tmp: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(exe)
            .map(|m| m.permissions().mode())
            .unwrap_or(0o755);
        std::fs::set_permissions(tmp, std::fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = exe;
    Ok(())
}

/// Run the freshly-installed binary with `--version` as a sanity check; a
/// failure here is informational only — the swap already succeeded.
fn verify_reexec(exe: &Path) {
    match Command::new(exe).arg("--version").output() {
        Ok(out) => {
            let version = String::from_utf8_lossy(&out.stdout);
            print!("Now running: {}", version.trim_end());
            println!();
        }
        Err(err) => eprintln!("note: could not re-exec the new binary to confirm: {err}"),
    }
}

/// Extract the sha256 for `asset` from a `checksums.txt` body. Lines look like
/// `<sha256>  <asset-dir>/<asset>` (or just `<asset>`), matching install.sh
/// and the npm downloader.
fn expected_checksum(text: &str, asset: &str) -> Result<String> {
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(file)) = (parts.next(), parts.next()) else {
            continue;
        };
        if file == asset || file.ends_with(&format!("/{asset}")) {
            return Ok(hash.to_string());
        }
    }
    bail!("no checksum entry for {asset} in checksums.txt")
}

/// Hex-encoded sha256 of `bytes`, via the `ring` digest already pulled in by
/// the TLS stack (no new dependency).
fn sha256_hex(bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    let mut hex = String::with_capacity(digest.as_ref().len() * 2);
    for byte in digest.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

async fn download_bytes(ctx: &Context, url: &str) -> Result<Vec<u8>> {
    let resp = ctx
        .http
        .get(url)
        .timeout(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .send()
        .await
        .with_context(|| format!("cannot download {url}"))?
        .error_for_status()
        .with_context(|| format!("download failed for {url}"))?;
    Ok(resp
        .bytes()
        .await
        .context("reading download body")?
        .to_vec())
}

async fn download_text(ctx: &Context, url: &str) -> Result<String> {
    let resp = ctx
        .http
        .get(url)
        .timeout(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .send()
        .await
        .with_context(|| format!("cannot download {url}"))?
        .error_for_status()
        .with_context(|| format!("download failed for {url}"))?;
    resp.text().await.context("reading download body")
}

/// npm channel: run `npm i -g flowleap@<latest> --prefer-online`, streaming
/// its output. The version is pinned to what `fetch_latest` just resolved —
/// installing the `latest` dist-tag instead would trust npm's packument
/// cache, which can lag a release by minutes and silently reinstall the
/// previous version right after the banner promised an upgrade (#38). This
/// is the final act — the running wrapper may be replaced mid-update, so
/// nothing after the spawn depends on this process's own binary staying
/// valid.
fn run_npm_upgrade(current: &str, latest: &str) -> Result<()> {
    let cmd = Channel::Npm.command_for(Some(latest));
    if which("npm").is_none() {
        print_skills_reminder_with_delta(current, latest);
        bail!("npm is not on PATH. Upgrade manually: {cmd}");
    }
    print_skills_reminder_with_delta(current, latest);
    println!("Running: {cmd}");
    let status = npm_command()
        .args(["i", "-g", &format!("flowleap@{latest}"), "--prefer-online"])
        .status()
        .context("failed to run npm")?;
    if !status.success() {
        bail!("{cmd} failed. Re-run it manually to see the error.");
    }
    Ok(())
}

/// Homebrew channel: run `brew upgrade flowleap` when brew is present, else
/// print the command.
fn run_brew_upgrade(current: &str, latest: &str) -> Result<()> {
    print_skills_reminder_with_delta(current, latest);
    if which("brew").is_none() {
        println!("Homebrew is not on PATH. Upgrade with: brew upgrade flowleap");
        return Ok(());
    }
    println!("Running: brew upgrade flowleap");
    let status = Command::new("brew")
        .args(["upgrade", "flowleap"])
        .status()
        .context("failed to run brew")?;
    if !status.success() {
        bail!("brew upgrade flowleap failed. Re-run it manually to see the error.");
    }
    Ok(())
}

/// cargo channel: never trigger a silent from-source recompile — print the
/// command for the user to run.
fn print_cargo_upgrade(current: &str, latest: &str) {
    println!("This is a cargo install. Upgrade from source with:");
    println!("  {CARGO_INSTALL_CMD}");
    print_skills_reminder_with_delta(current, latest);
}

/// `npm` invocation, using a shell on Windows where `npm` is a `.cmd` shim
/// that `Command::new("npm")` can't spawn directly.
fn npm_command() -> Command {
    if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "npm"]);
        cmd
    } else {
        Command::new("npm")
    }
}

/// Locate an executable on PATH (like `which`), respecting PATHEXT on Windows.
fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".to_string())
            .split(';')
            .map(|e| e.to_string())
            .collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path) {
        for ext in &exts {
            let candidate = dir.join(format!("{program}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Skill files ship inside the binary but are installed as copies, so an
/// upgrade leaves them behind. On these channels the new binary is not at a
/// path this process can name, so the refresh has to be the user's next step —
/// stated with its consequence, not as a housekeeping note.
fn print_skills_reminder() {
    println!(
        "Next: run `flowleap skills update` — installed skill files still carry the old \
         version and teach retired commands until they are refreshed."
    );
}

fn print_skills_reminder_with_delta(current: &str, latest: &str) {
    println!("flowleap {current} -> {latest} once the command below finishes.");
    print_skills_reminder();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_detection() {
        let cases: Vec<(&str, Channel)> = vec![
            // npm: per-user wrapper cache (binary named flowleap-native)
            (
                "/home/u/.cache/flowleap/v0.3.0/flowleap-native",
                Channel::Npm,
            ),
            // npm: inside the global package's bin dir
            (
                "/usr/local/lib/node_modules/flowleap/bin/flowleap-native",
                Channel::Npm,
            ),
            // npm: node_modules component wins even with an unexpected name
            ("/opt/node/node_modules/flowleap/bin/flowleap", Channel::Npm),
            // npm installed under Homebrew's node (founder's real setup): the
            // /opt/homebrew prefix must NOT read as Homebrew — node_modules is
            // checked first and the Homebrew marker is `Cellar`, not the prefix.
            (
                "/opt/homebrew/lib/node_modules/flowleap/bin/flowleap-native",
                Channel::Npm,
            ),
            // homebrew: cellar (canonicalized from /opt/homebrew/bin/flowleap)
            (
                "/opt/homebrew/Cellar/flowleap/0.3.3/bin/flowleap",
                Channel::Homebrew,
            ),
            (
                "/usr/local/Cellar/flowleap/0.3.3/bin/flowleap",
                Channel::Homebrew,
            ),
            // cargo: ~/.cargo/bin
            ("/home/u/.cargo/bin/flowleap", Channel::Cargo),
            // raw binary: install.sh default and arbitrary custom locations
            ("/usr/local/bin/flowleap", Channel::RawBinary),
            ("/home/u/bin/flowleap", Channel::RawBinary),
        ];
        let detected: Vec<(&str, Channel)> = cases
            .iter()
            .map(|(path, _)| (*path, detect_channel(Path::new(path))))
            .collect();
        assert_eq!(detected, cases);
    }

    #[test]
    fn channel_commands() {
        assert_eq!(Channel::Npm.command(), "npm i -g flowleap@latest");
        assert_eq!(Channel::Homebrew.command(), "brew upgrade flowleap");
        assert_eq!(Channel::Cargo.command(), CARGO_INSTALL_CMD);
        assert!(Channel::RawBinary.command().contains("install.sh"));
    }

    #[test]
    fn npm_command_pins_resolved_version() {
        // #38: right after a release npm's cached packument can still map the
        // `latest` dist-tag to the previous version. With the target version
        // resolved, the command must pin it and force cache revalidation.
        assert_eq!(
            Channel::Npm.command_for(Some("0.9.9")),
            "npm i -g flowleap@0.9.9 --prefer-online"
        );
        // Non-npm channels ignore the resolved version.
        assert_eq!(
            Channel::Homebrew.command_for(Some("0.9.9")),
            "brew upgrade flowleap"
        );
    }

    #[test]
    fn platform_asset_matches_npm_map() {
        // Whatever host runs the test, the asset name must be one of the
        // published release assets (mirrors npm/download.mjs PLATFORM_MAP).
        let asset = platform_asset().expect("supported test platform");
        let known = [
            "flowleap-darwin-aarch64",
            "flowleap-darwin-x86_64",
            "flowleap-linux-aarch64",
            "flowleap-linux-x86_64",
            "flowleap-windows-x86_64.exe",
        ];
        assert!(known.contains(&asset.as_str()), "unexpected asset: {asset}");
    }

    #[test]
    fn checksum_lookup_handles_both_line_layouts() {
        let text = "\
deadbeef  flowleap-linux-x86_64\n\
cafef00d  flowleap-darwin-aarch64/flowleap-darwin-aarch64\n";
        assert_eq!(
            expected_checksum(text, "flowleap-linux-x86_64").unwrap(),
            "deadbeef"
        );
        assert_eq!(
            expected_checksum(text, "flowleap-darwin-aarch64").unwrap(),
            "cafef00d"
        );
        assert!(expected_checksum(text, "flowleap-windows-x86_64.exe").is_err());
    }

    #[test]
    fn sha256_is_correct() {
        // Known vector: sha256("") = e3b0c442...
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
