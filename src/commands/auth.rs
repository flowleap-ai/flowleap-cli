use crate::client::Context;
use crate::config::Credentials;
use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::io::{IsTerminal, Write};
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct DeviceAuthResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Parser)]
pub struct AuthArgs {
    #[command(subcommand)]
    command: AuthCommand,
}

#[derive(Subcommand)]
enum AuthCommand {
    /// Login via OAuth 2.0 (opens browser) or with API key/token
    Login {
        /// API key to store (skips OAuth flow)
        #[arg(long)]
        api_key: Option<String>,
        /// Bearer token to store (skips OAuth flow)
        #[arg(long)]
        token: Option<String>,
    },
    /// Clear stored credentials (everything, including provider keys)
    Logout {
        /// Clear only the OAuth session token; keep the API key and
        /// EPO/USPTO provider keys
        #[arg(long)]
        session_only: bool,
    },
    /// Show current authentication status
    Status,
    /// Create a personal API token (fl_pat_…) for headless/agent use
    CreateToken {
        /// Display name for the token (e.g. "ci-agent")
        #[arg(long)]
        name: String,
        /// Store the new token as this CLI's credential
        #[arg(long)]
        store: bool,
    },
    /// List personal API tokens
    Tokens,
    /// Revoke a personal API token by id
    RevokeToken {
        /// Token id (from 'flowleap auth tokens')
        id: String,
    },
}

pub async fn run(ctx: &Context, args: AuthArgs) -> Result<()> {
    match args.command {
        AuthCommand::Login { api_key, token } => login(ctx, api_key, token).await,
        AuthCommand::Logout { session_only } => logout(session_only).await,
        AuthCommand::Status => status(ctx).await,
        AuthCommand::CreateToken { name, store } => create_token(ctx, &name, store).await,
        AuthCommand::Tokens => list_tokens(ctx).await,
        AuthCommand::RevokeToken { id } => revoke_token(ctx, &id).await,
    }
}

async fn create_token(ctx: &Context, name: &str, store: bool) -> Result<()> {
    ctx.require_auth()?;

    let body = serde_json::json!({ "name": name });
    let result = ctx
        .execute_json_body_or_error(ctx.post("/api/tokens", &body))
        .await?;

    if store {
        if let Some(token) = result.get("token").and_then(|t| t.as_str()) {
            let mut creds = Credentials::load()?;
            creds.api_key = Some(token.to_string());
            // Clear the session token: auth prefers `token` over `api_key`, so
            // leaving the (short-lived) Clerk JWT in place would shadow the
            // durable personal token we just stored — commands would silently
            // keep using the JWT until it expires, then 401.
            creds.token = None;
            creds.save()?;
        }
    }

    if ctx.output_format == "json" || result.get("dryRun").is_some() {
        crate::output::print_json(&result);
    } else if let Some(token) = result.get("token").and_then(|t| t.as_str()) {
        println!("{} Token created. Shown once — store it now:", "✓".green());
        println!("\n  {}\n", token.bold());
        if store {
            println!("Stored as this CLI's credential (previous session token cleared).");
        } else {
            println!("Use: export FLOWLEAP_API_KEY={}", token);
        }
    } else {
        crate::output::print_json(&result);
    }
    Ok(())
}

async fn list_tokens(ctx: &Context) -> Result<()> {
    ctx.require_auth()?;
    let result = ctx
        .execute_json_body_or_error(ctx.get("/api/tokens"))
        .await?;
    let columns = &[
        ("id", "ID"),
        ("name", "Name"),
        ("tokenPrefix", "Prefix"),
        ("createdAt", "Created"),
        ("lastUsedAt", "Last used"),
        ("revokedAt", "Revoked"),
    ];
    if let Some(tokens) = result.get("tokens") {
        crate::output::print_value(&ctx.output_format, tokens, columns);
    } else {
        crate::output::print_value(&ctx.output_format, &result, columns);
    }
    Ok(())
}

async fn revoke_token(ctx: &Context, id: &str) -> Result<()> {
    ctx.require_auth()?;
    let path = format!("/api/tokens/{}", crate::client::encode_url_component(id));
    let result = ctx
        .execute_json_body_or_error(ctx.request(reqwest::Method::DELETE, &path, None))
        .await?;
    if ctx.output_format == "json" || result.get("dryRun").is_some() {
        crate::output::print_json(&result);
    } else {
        println!("{} Token revoked.", "✓".green());
    }
    Ok(())
}

/// Minimum remaining lifetime, in seconds, a session token must carry for
/// [`store_session_token`] to persist it. Ten minutes comfortably covers the
/// commands that follow a fresh login (`auth status`, a first real request)
/// without the credential dying mid-session.
const MIN_SESSION_TOKEN_LIFETIME_SECS: i64 = 600;

/// A JWT's `exp` (and, when present, `iat`) claims, decoded without any
/// signature verification — this guard only sanity-checks a lifetime the
/// server itself asserts, it never authenticates anything.
struct SessionTokenClaims {
    exp: i64,
    iat: Option<i64>,
}

/// Decode the middle segment of a `header.payload.signature` JWT and read
/// its `exp`/`iat` claims. Returns `None` for anything that isn't a
/// three-segment, base64url, JSON-payload token — most notably an opaque
/// `fl_pat_…` personal API token — so callers fail open rather than blocking
/// a credential this guard has no business judging.
fn decode_session_token_claims(token: &str) -> Option<SessionTokenClaims> {
    use base64::Engine as _;
    let segments: Vec<&str> = token.split('.').collect();
    if segments.len() != 3 {
        return None;
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(segments[1])
        .ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let exp = claims.get("exp")?.as_i64()?;
    let iat = claims.get("iat").and_then(|v| v.as_i64());
    Some(SessionTokenClaims { exp, iat })
}

/// Store a freshly obtained OAuth session token as this machine's
/// credential — the single choke point the human login flow, the
/// structured `--json` login flow, and the setup wizard all route through.
///
/// Refuses, loudly, to persist a token whose `exp` claim shows fewer than
/// [`MIN_SESSION_TOKEN_LIFETIME_SECS`] remaining: see
/// flowleap-backend#254 — device-flow approval has, on occasion, echoed
/// back a ~60s default Clerk session token instead of the long-lived
/// `flowleap`-template token the server is supposed to mint. Storing that
/// token would just shadow a still-good `api_key` (session tokens win in
/// [`Credentials::auth_header`]'s precedence) with a credential dead before
/// the next command runs.
///
/// A token that doesn't decode as a three-segment JWT with a readable `exp`
/// — an opaque personal API token, for instance — is stored without
/// complaint; the guard only fires when it can positively read a short
/// lifetime.
pub fn store_session_token(token: String) -> Result<()> {
    if let Some(claims) = decode_session_token_claims(&token) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let remaining = claims.exp - now;
        if remaining < MIN_SESSION_TOKEN_LIFETIME_SECS {
            let lifetime_desc = match claims.iat {
                Some(iat) => format!("{}s lifetime (iat to exp)", claims.exp - iat),
                None => format!("{remaining}s remaining"),
            };
            return Err(crate::client::SessionTokenRefusedError::new(format!(
                "Refusing to store this session token: it has only {lifetime_desc} — too \
                 short to survive to the next command. This is a known backend/website bug \
                 (flowleap-backend#254): device-flow approval can hand back a short-lived \
                 default Clerk session token instead of the long-lived flowleap-template \
                 token the server is supposed to mint. Run 'flowleap auth login' again; if \
                 this keeps happening, the server needs to fix the mint path."
            ))
            .into());
        }
    }
    let mut creds = Credentials::load()?;
    creds.token = Some(token);
    creds.save()?;
    Ok(())
}

async fn login(ctx: &Context, api_key: Option<String>, token: Option<String>) -> Result<()> {
    // If credentials passed directly, store them
    if api_key.is_some() || token.is_some() {
        let mut creds = Credentials::load()?;

        if let Some(key) = api_key {
            creds.api_key = Some(key);
            println!("{} API key stored.", "✓".green());
        }

        if let Some(tok) = token {
            creds.token = Some(tok);
            println!("{} Token stored.", "✓".green());
        }

        creds.save()?;
        println!(
            "Credentials saved to {:?}",
            Credentials::credentials_path()?
        );
        return Ok(());
    }

    // Structured mode (--json / --output json): blocking NDJSON event stream
    // for agents — no browser, clipboard, or spinner side effects.
    if ctx.output_format == "json" {
        return structured_device_login(ctx).await;
    }

    // OAuth 2.0 Device Authorization flow
    let access_token = device_flow_login(ctx).await?;
    store_session_token(access_token)?;
    println!("{} Successfully authenticated!", "✓".green());
    println!(
        "Credentials saved to {:?}",
        Credentials::credentials_path()?
    );
    Ok(())
}

/// Best-effort copy to the system clipboard (pbcopy/xclip/wl-copy). Never fails.
fn copy_to_clipboard(text: &str) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};
    for cmd in [
        &["pbcopy"][..],
        &["xclip", "-selection", "clipboard"][..],
        &["wl-copy"][..],
    ] {
        if let Ok(mut child) = Command::new(cmd[0])
            .args(&cmd[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                if stdin.write_all(text.as_bytes()).is_ok() {
                    let _ = child.wait();
                    return true;
                }
            }
            let _ = child.kill();
        }
    }
    false
}

/// Request device authorization from `POST /oauth/device`.
///
/// Reuses the shared, configured client (timeouts + versioned User-Agent)
/// rather than constructing an unconfigured one. The device endpoints are
/// unauthenticated, so no auth injection is needed here.
async fn request_device_authorization(ctx: &Context) -> Result<DeviceAuthResponse> {
    let base_url = ctx.config.base_url.trim_end_matches('/');
    let resp = ctx
        .http
        .post(format!("{}/oauth/device", base_url))
        .json(&serde_json::json!({"client_id": "flowleap-cli"}))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!("Device authorization request failed ({}): {}", status, body);
    }

    Ok(resp.json().await?)
}

/// Poll `POST /oauth/device/token` until the flow reaches a terminal state,
/// honoring the server's `interval`, `slow_down`, and `expires_in`. Returns
/// the access token on approval. `spinner` and `show_manual_hint` are the
/// human-flow UI; structured mode passes neither and prints nothing.
async fn poll_device_token(
    ctx: &Context,
    response: &DeviceAuthResponse,
    spinner: Option<&ProgressBar>,
    show_manual_hint: bool,
) -> Result<String> {
    let base_url = ctx.config.base_url.trim_end_matches('/');
    let clear_spinner = || {
        if let Some(spinner) = spinner {
            spinner.finish_and_clear();
        }
    };

    let mut interval = response.interval;
    let started = std::time::Instant::now();
    let deadline = started + Duration::from_secs(response.expires_in);
    let mut hinted = false;

    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;

        if std::time::Instant::now() > deadline {
            clear_spinner();
            bail!("Device authorization expired. Please try again.");
        }

        // Browser didn't open, or the tab got lost? Give the manual path once.
        if show_manual_hint && !hinted && started.elapsed() > Duration::from_secs(25) {
            hinted = true;
            let print_hint = || {
                println!(
                    "  Taking a while? Open {} manually and enter code {}",
                    response.verification_uri_complete.cyan(),
                    response.user_code.bold().yellow()
                );
            };
            match spinner {
                Some(spinner) => spinner.suspend(print_hint),
                None => print_hint(),
            }
        }

        let poll_resp = ctx
            .http
            .post(format!("{}/oauth/device/token", base_url))
            .json(&serde_json::json!({
                "device_code": response.device_code,
                "client_id": "flowleap-cli",
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
            }))
            .send()
            .await?;

        let body: serde_json::Value = poll_resp.json().await?;

        if let Some(access_token) = body.get("access_token").and_then(|v| v.as_str()) {
            clear_spinner();
            return Ok(access_token.to_string());
        }

        if let Some(error) = body.get("error").and_then(|v| v.as_str()) {
            match error {
                "authorization_pending" => continue,
                "slow_down" => {
                    interval += 5;
                    continue;
                }
                "expired_token" => {
                    clear_spinner();
                    bail!("Device authorization expired. Please try again.");
                }
                "access_denied" => {
                    clear_spinner();
                    bail!("Authorization was denied.");
                }
                other => {
                    clear_spinner();
                    let desc = body
                        .get("error_description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown error");
                    bail!("Authorization failed: {} — {}", other, desc);
                }
            }
        }
    }
}

/// Run the OAuth 2.0 Device Authorization flow and return the access token.
/// Prints the code/URL, polls with slow_down handling, and shows a
/// manual-fallback hint if approval takes a while. Browser auto-open, the
/// clipboard copy, and the spinner run only when stdout is a TTY — a headless
/// run must never pop UI or touch the user's clipboard. Does NOT persist
/// anything.
pub async fn device_flow_login(ctx: &Context) -> Result<String> {
    let interactive = std::io::stdout().is_terminal();

    println!("Starting device authorization flow...");

    let response = request_device_authorization(ctx).await?;

    // Only a human at a terminal can use the clipboard, and a headless run
    // would silently overwrite whatever the user had on it.
    let copied = interactive && copy_to_clipboard(&response.verification_uri_complete);
    println!(
        "\n  {} {}\n  {} {}{}\n",
        "▸ Visit:".bold(),
        response.verification_uri.cyan(),
        "▸ Enter code:".bold(),
        response.user_code.bold().yellow(),
        if copied {
            "   (sign-in link copied to clipboard)".dimmed().to_string()
        } else {
            String::new()
        },
    );

    let spinner = if interactive {
        let _ = open::that(&response.verification_uri_complete);
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        spinner.set_message("Waiting for authorization...");
        spinner.enable_steady_tick(Duration::from_millis(100));
        Some(spinner)
    } else {
        None
    };

    poll_device_token(ctx, &response, spinner.as_ref(), true).await
}

/// Write one NDJSON event: a compact JSON object plus newline, flushed
/// immediately so agents reading the pipe see it before polling completes.
fn emit_ndjson_event(value: &serde_json::Value) {
    let mut stdout = std::io::stdout().lock();
    let _ = serde_json::to_writer(&mut stdout, value);
    let _ = stdout.write_all(b"\n");
    let _ = stdout.flush();
}

/// Structured (--json) device-flow login for agents: a blocking NDJSON event
/// stream on stdout. Emits the `device_authorization` event immediately (so
/// the agent can relay the URL and user code to the human), polls until the
/// flow completes, and emits exactly one terminal event — `authorized`
/// (session token stored, exit 0) or `failed` (nonzero exit per the
/// documented exit-code table). No side effects: no browser auto-open, no
/// clipboard copy, no spinner — stdout carries nothing but NDJSON.
async fn structured_device_login(ctx: &Context) -> Result<()> {
    let outcome = async {
        let response = request_device_authorization(ctx).await?;
        emit_ndjson_event(&serde_json::json!({
            "event": "device_authorization",
            "verification_uri": response.verification_uri,
            "verification_uri_complete": response.verification_uri_complete,
            "user_code": response.user_code,
            "expires_in": response.expires_in,
            "interval": response.interval,
        }));
        let access_token = poll_device_token(ctx, &response, None, false).await?;
        // Store the session token exactly as the human flow does.
        store_session_token(access_token)?;
        anyhow::Ok(())
    }
    .await;

    match outcome {
        Ok(()) => {
            emit_ndjson_event(&serde_json::json!({"event": "authorized", "stored": true}));
            Ok(())
        }
        Err(err) => {
            let code = crate::client::error_exit_code(&err);
            emit_ndjson_event(&serde_json::json!({
                "event": "failed",
                "error": err.to_string(),
            }));
            // The failure is already on stdout as the terminal event; a
            // PrintedError keeps the top-level handler from printing a second
            // JSON envelope while preserving the documented exit code.
            Err(crate::client::PrintedError::with_exit_code(code).into())
        }
    }
}

/// Mint a personal API token via /api/tokens using `auth_ctx` (must carry a
/// Clerk credential) and store it as this machine's durable credential,
/// clearing the session token. Returns the masked prefix for display.
pub async fn mint_and_store_token(auth_ctx: &Context, name: &str) -> Result<String> {
    let body = serde_json::json!({ "name": name });
    let result = auth_ctx
        .execute_json_body_or_error(auth_ctx.post("/api/tokens", &body))
        .await?;
    let Some(token) = result.get("token").and_then(|t| t.as_str()) else {
        bail!("Token endpoint did not return a token.");
    };
    let mut creds = Credentials::load()?;
    creds.api_key = Some(token.to_string());
    creds.token = None;
    creds.save()?;
    let prefix: String = token.chars().take(11).collect();
    Ok(format!("{}…", prefix))
}

async fn logout(session_only: bool) -> Result<()> {
    let mut creds = Credentials::load()?;
    if session_only {
        if creds.token.is_none() {
            println!("No session token stored — nothing to clear.");
            return Ok(());
        }
        creds.clear_session();
        creds.save()?;
        println!(
            "{} Session token cleared. API key and provider keys kept.",
            "✓".green()
        );
        if creds.api_key.is_some() {
            println!("Requests will now authenticate with the stored API key.");
        }
    } else {
        creds.clear();
        creds.save()?;
        println!(
            "{} All credentials cleared (session, API key, provider keys).",
            "✓".green()
        );
    }
    Ok(())
}

/// What is actually known about the stored credential.
///
/// The distinction that matters is between *present* and *working*: reporting
/// presence as "Authenticated" is what let an expired token read as healthy
/// until the next data command 401'd. [`Unverified`] is the honest fourth
/// state — a credential that could not be checked because the backend was
/// unreachable is not thereby invalid, and sending someone to re-authenticate
/// over a dropped connection would not help them.
///
/// [`Unverified`]: CredentialState::Unverified
enum CredentialState {
    /// No credential in the environment or the config.
    Absent,
    /// The backend accepted it and identified the caller.
    Valid,
    /// The backend refused it (HTTP 401) — expired, revoked, or wrong.
    Rejected,
    /// Present, but validity is unknown: the check could not be completed.
    Unverified {
        reason: String,
        /// The HTTP status when the backend answered but unusably; `None`
        /// when it was never reached (network failure, or a dry run).
        http_status: Option<u16>,
    },
}

impl CredentialState {
    /// The machine-readable verdict for `--json`, and the switch the human
    /// rendering keys off.
    fn name(&self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Valid => "valid",
            Self::Rejected => "rejected",
            Self::Unverified { .. } => "unverified",
        }
    }
}

async fn status(ctx: &Context) -> Result<()> {
    let source = crate::client::credential_source(ctx);
    let (state, profile) = verify_credential(ctx).await;

    let mut report = serde_json::json!({
        "baseUrl": ctx.config.base_url,
        "credential": {
            "present": !matches!(state, CredentialState::Absent),
            "source": source,
        },
        "verification": {
            "state": state.name(),
            // Whether a live check actually reached a verdict, so a consumer
            // can tell "known good" from "assumed good".
            "checked": matches!(state, CredentialState::Valid | CredentialState::Rejected),
        },
    });
    if let CredentialState::Unverified {
        reason,
        http_status,
    } = &state
    {
        report["verification"]["reason"] = serde_json::json!(reason);
        if let Some(status) = http_status {
            report["verification"]["httpStatus"] = serde_json::json!(status);
        }
    }
    if let Some(profile) = &profile {
        let identity: serde_json::Map<String, serde_json::Value> = ["email", "name"]
            .iter()
            .filter_map(|key| profile.get(key).map(|v| (key.to_string(), v.clone())))
            .collect();
        if !identity.is_empty() {
            report["profile"] = serde_json::Value::Object(identity);
        }
    }
    if let Some(model) = &ctx.config.default_model {
        report["defaultModel"] = serde_json::json!(model);
    }

    if ctx.output_format == "json" {
        crate::output::print_json(&report);
    } else {
        let both_stored = ctx.credentials.token.is_some() && ctx.credentials.api_key.is_some();
        print_status_human(&state, source, &report, both_stored);
    }

    // Exit contract (mirrors `doctor`): the report is always fully emitted
    // first, then the state picks the documented code — 0 verified, 3 when a
    // credential is missing or refused (both mean "run auth login"), and the
    // status-derived code otherwise, which is 7 for an unreachable backend.
    // A dry run sends nothing and so can prove nothing; like doctor's, it
    // keeps the success exit.
    if ctx.dry_run {
        return Ok(());
    }
    match state {
        CredentialState::Valid => Ok(()),
        CredentialState::Absent | CredentialState::Rejected => Err(
            crate::client::PrintedError::with_exit_code(crate::client::EXIT_AUTH_REQUIRED).into(),
        ),
        CredentialState::Unverified { http_status, .. } => {
            let code = http_status
                .map(crate::client::exit_code_for_status)
                .unwrap_or(crate::client::EXIT_NETWORK);
            Err(crate::client::PrintedError::with_exit_code(code).into())
        }
    }
}

/// Probe the stored credential against `/api/profile` and report what came
/// back.
///
/// This request was already being made on every `auth status` run — its
/// result was simply discarded as "best-effort", which is precisely why an
/// expired token could still print "Authenticated". Validation therefore
/// costs no extra round trip and needs no opt-in flag: the verdict is just no
/// longer thrown away.
///
/// The probe goes through the normal client path, so a `Rejected` verdict
/// already accounts for the session-token-to-API-key fallback — it means the
/// effective credential was refused, not merely the first one tried.
async fn verify_credential(ctx: &Context) -> (CredentialState, Option<serde_json::Value>) {
    if ctx.credentials.auth_header().is_none() {
        return (CredentialState::Absent, None);
    }
    if ctx.dry_run {
        return (
            CredentialState::Unverified {
                reason: "Dry run — no request was sent, so the credential was not checked."
                    .to_string(),
                http_status: None,
            },
            None,
        );
    }

    match ctx.execute_json(ctx.get("/api/profile")).await {
        Ok(profile) => (CredentialState::Valid, Some(profile)),
        Err(err) => match err.downcast_ref::<crate::client::ApiError>() {
            Some(api) if api.status == 401 => (CredentialState::Rejected, None),
            // A paywall answers a different question than authentication: the
            // credential was accepted and the caller identified, so reporting
            // it as rejected would send them to `auth login` over an unpaid
            // subscription, which fixes nothing.
            Some(api) if api.status == 402 => (CredentialState::Valid, None),
            Some(api) => (
                CredentialState::Unverified {
                    reason: format!(
                        "The backend answered HTTP {} instead of a profile, so the credential \
                         could not be checked.",
                        api.status
                    ),
                    http_status: Some(api.status),
                },
                None,
            ),
            // Network failure, timeout, or an unparseable response: never
            // reached a verdict, so it is not evidence against the credential.
            None => (
                CredentialState::Unverified {
                    reason: err.to_string(),
                    http_status: None,
                },
                None,
            ),
        },
    }
}

/// Human rendering, reading the same report `--json` emits so the two views
/// cannot drift. Each state gets a distinct marker and, when something is
/// wrong, the specific command that fixes it.
fn print_status_human(
    state: &CredentialState,
    source: &str,
    report: &serde_json::Value,
    both_stored: bool,
) {
    println!("Base URL:  {}", report["baseUrl"].as_str().unwrap_or(""));

    match state {
        CredentialState::Valid => println!(
            "Auth:      {} ({source}) — verified against the backend",
            "Valid".green()
        ),
        CredentialState::Rejected => println!(
            "Auth:      {} ({source}) — the backend refused this credential (HTTP 401)",
            "Rejected".red()
        ),
        CredentialState::Unverified { reason, .. } => {
            println!(
                "Auth:      {} ({source}) — could not verify",
                "Present".yellow()
            );
            println!("           {reason}");
        }
        CredentialState::Absent => println!("Auth:      {}", "Not authenticated".red()),
    }

    for (label, key) in [("Email:    ", "email"), ("Name:     ", "name")] {
        if let Some(value) = report["profile"].get(key).and_then(|v| v.as_str()) {
            println!("{label} {value}");
        }
    }
    if let Some(model) = report["defaultModel"].as_str() {
        println!("Model:     {model}");
    }

    match state {
        CredentialState::Absent => {
            println!("\nRun 'flowleap auth login' to authenticate via OAuth,");
            println!("or 'flowleap auth login --api-key <key>' to store an API key.");
        }
        CredentialState::Rejected => {
            println!("\nThe stored credential is present but expired, revoked, or wrong.");
            println!("Run 'flowleap auth login' to re-authenticate, or");
            println!("'flowleap auth create-token --name <n> --store' for a long-lived token.");
            // The one case where a working credential may already be on disk:
            // an expired session token shadows the API key it is preferred
            // over, so dropping just the session recovers without a re-login.
            if both_stored {
                println!(
                    "\nBoth a session token and an API key are stored. If the session token is \
                     the expired one,\n'flowleap auth logout --session-only' drops it and keeps \
                     the API key."
                );
            }
        }
        CredentialState::Unverified { .. } => {
            println!("\nA stored credential is not proof of a working one — this says only that");
            println!("one is configured. Re-run when the backend is reachable to confirm it.");
        }
        CredentialState::Valid => {}
    }
}
