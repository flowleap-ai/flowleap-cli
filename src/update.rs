//! Once-a-day update notifier.
//!
//! Agent-safe by construction: never runs for --json / --dry-run / non-TTY
//! stderr / FLOWLEAP_NO_UPDATE_CHECK, the notice goes to stderr only, and the
//! registry fetch overlaps the command's own work (plus a short grace period)
//! so it adds no meaningful latency. All failures are silent — an update
//! notice is never worth breaking a command for.

use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use crate::config::Config;

const CHECK_INTERVAL_SECS: u64 = 60 * 60 * 24;
const REGISTRY_URL: &str = "https://registry.npmjs.org/flowleap/latest";
const FETCH_TIMEOUT: Duration = Duration::from_millis(1500);
/// Post-command wait ceiling for the spawned check; kept just above
/// FETCH_TIMEOUT so the task resolves rather than being cancelled mid-write.
pub const CHECK_GRACE: Duration = Duration::from_millis(1700);

#[derive(Debug, Default, Serialize, Deserialize)]
struct CheckState {
    #[serde(default)]
    last_checked_unix: u64,
    #[serde(default)]
    latest: String,
}

fn state_path() -> Option<PathBuf> {
    Config::config_dir()
        .ok()
        .map(|dir| dir.join("update-check.toml"))
}

fn load_state() -> CheckState {
    state_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_state(state: &CheckState) {
    if let (Some(path), Ok(text)) = (state_path(), toml::to_string(state)) {
        let _ = std::fs::write(path, text);
    }
}

/// Strictly-newer x.y.z compare, tolerant of a leading `v` and trailing
/// pre-release/build suffixes on the patch component.
pub fn is_newer(latest: &str, current: &str) -> bool {
    fn parse(v: &str) -> Option<(u64, u64, u64)> {
        let mut parts = v.trim().trim_start_matches('v').splitn(3, '.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch_digits: String = parts
            .next()?
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        Some((major, minor, patch_digits.parse().ok()?))
    }
    match (parse(latest), parse(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// Latest version seen by the most recent daily check, if any. Read from the
/// cached state only (never the network), so callers like `flowleap doctor`
/// stay offline-safe. Empty/unknown yields None.
pub fn cached_latest() -> Option<String> {
    let latest = load_state().latest;
    (!latest.is_empty()).then_some(latest)
}

/// The daily notice now points at the channel-aware `flowleap upgrade`
/// command instead of a channel-specific instruction — one command upgrades
/// every install channel (npm/brew/binary/cargo).
fn notice(latest: &str, current: &str) -> Option<String> {
    if latest.is_empty() || !is_newer(latest, current) {
        return None;
    }
    Some(format!(
        "flowleap {latest} is available (you have {current}). Update: flowleap upgrade"
    ))
}

/// Spawn the update check, or None when gated off. The handle resolves to
/// the "a newer flowleap is available" line, if any. It does NOT hold the
/// stale-skills line: the task starts before the command, and a command can
/// change the recorded skill installs (`flowleap skills update`). The caller
/// adds that line after the command, with `closing_notice`.
pub fn spawn_check(
    http: &reqwest::Client,
    json: bool,
    dry_run: bool,
) -> Option<JoinHandle<Option<String>>> {
    if json
        || dry_run
        || std::env::var_os("FLOWLEAP_NO_UPDATE_CHECK").is_some()
        || !std::io::stderr().is_terminal()
    {
        return None;
    }
    let http = http.clone();
    Some(tokio::spawn(check(http)))
}

async fn check(http: reqwest::Client) -> Option<String> {
    let current = env!("CARGO_PKG_VERSION");
    let mut state = load_state();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();

    // Inside the daily window: answer from the cached state, no network.
    if now.saturating_sub(state.last_checked_unix) < CHECK_INTERVAL_SECS {
        return notice(&state.latest, current);
    }

    let resp = http
        .get(REGISTRY_URL)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .ok()?;
    let body: serde_json::Value = resp.json().await.ok()?;
    state.latest = body.get("version")?.as_str()?.to_string();
    state.last_checked_unix = now;
    save_state(&state);
    notice(&state.latest, current)
}

/// The notice to print after the command: the update line from the spawned
/// check (None when there is none, or the check did not finish in time), then
/// the stale-skills line.
///
/// CALL THIS AFTER THE COMMAND HAS RUN. The stale-skills line is read from
/// the config at the time of the call. When it was read in the spawned check
/// (before the command), `flowleap skills update` refreshed every target and
/// then printed "N installed skill target(s) were rendered by an older
/// flowleap", which was no longer true.
pub fn closing_notice(update_notice: Option<String>) -> Option<String> {
    let stale = crate::commands::skills::stale_skills_notice(env!("CARGO_PKG_VERSION"));
    join_notices(update_notice, stale)
}

/// The update line and the stale-skills line as one notice, one line each.
fn join_notices(update_notice: Option<String>, stale_skills: Option<String>) -> Option<String> {
    if update_notice.is_none() && stale_skills.is_none() {
        return None;
    }
    Some(
        update_notice
            .into_iter()
            .chain(stale_skills)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_detection() {
        assert!(is_newer("0.2.5", "0.2.4"));
        assert!(is_newer("0.3.0", "0.2.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("v0.2.5", "0.2.4"));
        assert!(is_newer("0.2.5-rc.1", "0.2.4"));
        assert!(!is_newer("0.2.4", "0.2.4"));
        assert!(!is_newer("0.2.3", "0.2.4"));
        assert!(!is_newer("garbage", "0.2.4"));
        assert!(!is_newer("", "0.2.4"));
    }

    #[test]
    fn notice_only_when_newer() {
        assert!(notice("0.2.5", "0.2.4").is_some());
        assert!(notice("0.2.4", "0.2.4").is_none());
        assert!(notice("", "0.2.4").is_none());
    }

    #[test]
    fn the_update_line_does_not_hold_the_stale_skills_line() {
        // The spawned check runs before the command. Its result must not say
        // anything about the skill installs: the command can change them.
        let text = notice("0.2.5", "0.2.4").unwrap();
        assert!(!text.contains("skill"), "{text}");
    }

    #[test]
    fn closing_notice_joins_the_two_lines() {
        let update = || Some("flowleap 0.2.5 is available".to_string());
        let stale = || {
            Some("2 installed skill target(s) were rendered by an older flowleap. Refresh: flowleap skills update".to_string())
        };
        assert_eq!(join_notices(None, None), None);
        assert_eq!(join_notices(update(), None), update());
        assert_eq!(join_notices(None, stale()), stale());
        assert_eq!(
            join_notices(update(), stale()),
            Some(format!("{}\n{}", update().unwrap(), stale().unwrap()))
        );
    }

    #[test]
    fn notice_points_at_upgrade_command() {
        // The daily notice recommends the channel-aware `flowleap upgrade`
        // command; per-channel behavior lives in commands::upgrade.
        let text = notice("0.2.5", "0.2.4").unwrap();
        assert!(text.contains("flowleap upgrade"));
    }
}
