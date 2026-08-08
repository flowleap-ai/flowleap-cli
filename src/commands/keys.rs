use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password};
use serde_json::{json, Value};
use std::io::IsTerminal;

use crate::client::Context;
use crate::config::Credentials;
use crate::output;

pub const EPO_SIGNUP: &str = "https://developers.epo.org";
pub const USPTO_SIGNUP: &str = "https://data.uspto.gov/apis/getting-started";

#[derive(Parser)]
pub struct KeysArgs {
    #[command(subcommand)]
    command: KeysCommand,
}

#[derive(Clone, ValueEnum)]
pub enum Provider {
    Epo,
    Uspto,
}

#[derive(Subcommand)]
enum KeysCommand {
    /// Interactive wizard for all provider keys (human-only)
    Setup,
    /// Set keys for one provider (flags for non-interactive use)
    Set {
        provider: Provider,
        /// EPO consumer key or USPTO API key
        #[arg(long)]
        key: Option<String>,
        /// EPO consumer secret (EPO only)
        #[arg(long)]
        secret: Option<String>,
        /// Skip live validation against the backend
        #[arg(long)]
        no_verify: bool,
    },
    /// Show configured providers (secrets masked)
    List,
    /// Validate configured keys against the live providers
    Test,
    /// Remove stored keys for one provider
    Rm { provider: Provider },
}

pub async fn run(ctx: &Context, args: KeysArgs) -> Result<()> {
    match args.command {
        KeysCommand::Setup => setup_wizard(ctx).await,
        KeysCommand::Set {
            provider,
            key,
            secret,
            no_verify,
        } => set(ctx, provider, key, secret, no_verify).await,
        KeysCommand::List => list(ctx),
        KeysCommand::Test => test(ctx).await,
        KeysCommand::Rm { provider } => rm(ctx, provider),
    }
}

fn mask(value: &str) -> String {
    let visible: String = value.chars().take(4).collect();
    format!("{}…{}", visible, "•".repeat(4))
}

fn require_tty() -> Result<()> {
    if !std::io::stdin().is_terminal() {
        bail!(
            "Interactive setup needs a terminal (a human). Non-interactive alternatives:\n  \
             flowleap keys set epo --key <consumer-key> --secret <consumer-secret>\n  \
             flowleap keys set uspto --key <api-key>\n  \
             env: FLOWLEAP_EPO_KEY, FLOWLEAP_EPO_SECRET, FLOWLEAP_USPTO_KEY"
        );
    }
    Ok(())
}

/// Build a Context that authenticates like `ctx` but carries the candidate
/// provider keys, so validation exercises keys before anything is saved.
fn with_candidate_keys(ctx: &Context, creds: Credentials) -> Context {
    Context {
        config: ctx.config.clone(),
        credentials: creds,
        output_format: ctx.output_format.clone(),
        dry_run: false,
        dry_run_redacted: ctx.dry_run_redacted,
        verbose: ctx.verbose,
        token_overridden: ctx.token_overridden,
        assume_yes: ctx.assume_yes,
        http: ctx.http.clone(),
    }
}

/// Credentials for a single-provider validation probe. Starts from the
/// RUNTIME credentials (so --token/--api-key/env auth is kept — the backend's
/// /v1/keys/validate requires auth), swaps in the candidate keys, and strips
/// the OTHER provider's keys so e.g. a stale stored EPO pair can't 400 the
/// probe before the USPTO key under test is ever checked.
fn probe_credentials(
    ctx: &Context,
    provider: &Provider,
    key: &str,
    secret: Option<&str>,
) -> Credentials {
    let mut creds = ctx.credentials.clone();
    match provider {
        Provider::Epo => {
            creds.epo_key = Some(key.to_string());
            creds.epo_secret = secret.map(str::to_string);
            creds.uspto_key = None;
        }
        Provider::Uspto => {
            creds.uspto_key = Some(key.to_string());
            creds.epo_key = None;
            creds.epo_secret = None;
        }
    }
    creds
}

/// POST /v1/keys/validate with the given credentials. Returns the per-provider
/// verdicts object, mapping the middleware's eager EPO rejection (400
/// patent_provider_key_invalid) into an epo-invalid verdict.
///
/// Shared with `doctor`, which uses the same verdicts (source user|server|none,
/// valid true|false|null) to decide which provider next-steps actually block.
pub(crate) async fn validate(ctx: &Context, creds: Credentials) -> Result<Value> {
    let probe = with_candidate_keys(ctx, creds);
    let envelope = probe
        .execute_json_envelope(probe.post("/v1/keys/validate", &json!({})))
        .await?;

    if envelope["ok"].as_bool() == Some(true) {
        return Ok(envelope["body"]["providers"].clone());
    }
    let body_text = envelope["body"].to_string();
    if body_text.contains("patent_provider_key_invalid") {
        let message = envelope["body"]["error"]["message"]
            .as_str()
            .unwrap_or("Provider rejected the supplied keys.")
            .to_string();
        return Ok(json!({
            "epo": { "source": "user", "valid": false, "message": message },
            "uspto": { "source": "unknown", "valid": null, "message": "Not checked (EPO validation failed first)." },
        }));
    }
    bail!(
        "Key validation request failed (HTTP {}): {}",
        envelope["status"],
        body_text
    );
}

fn verdict_line(name: &str, verdict: &Value) -> String {
    let source = verdict["source"].as_str().unwrap_or("unknown");
    let message = verdict["message"].as_str().unwrap_or("");
    let symbol = match verdict["valid"] {
        Value::Bool(true) => "✓".green(),
        Value::Bool(false) => "✗".red(),
        _ => "•".yellow(),
    };
    format!("{} {:<6} [{}] {}", symbol, name, source, message)
}

// ---------------------------------------------------------------------------
// keys set
// ---------------------------------------------------------------------------

async fn set(
    ctx: &Context,
    provider: Provider,
    key: Option<String>,
    secret: Option<String>,
    no_verify: bool,
) -> Result<()> {
    let provider_name = match provider {
        Provider::Epo => "epo",
        Provider::Uspto => "uspto",
    };

    if ctx.dry_run {
        output::print_json(&json!({
            "dryRun": true,
            "action": "keys set",
            "provider": provider_name,
            "wouldValidate": !no_verify,
            "wouldSaveTo": Credentials::credentials_path()?,
        }));
        return Ok(());
    }

    // Resolve the candidate keys (flags, or prompts when interactive).
    let (candidate_key, candidate_secret): (String, Option<String>) = match provider {
        Provider::Epo => match (key, secret) {
            (Some(k), Some(s)) => (k, Some(s)),
            (None, None) => {
                require_tty()?;
                let (k, s) = prompt_epo_pair()?;
                (k, Some(s))
            }
            _ => bail!("EPO needs both --key and --secret (they only work as a pair)."),
        },
        Provider::Uspto => {
            if secret.is_some() {
                bail!("USPTO takes only --key (no secret).");
            }
            match key {
                Some(k) => (k, None),
                None => {
                    require_tty()?;
                    (prompt_uspto_key()?, None)
                }
            }
        }
    };

    // verified: Some(true) proven, Some(false) rejected, None unverifiable.
    let mut verified: Option<bool> = if no_verify { None } else { Some(false) };
    let mut verdict = Value::Null;
    if !no_verify {
        // Probe with runtime auth + the candidate keys in isolation.
        let probe = probe_credentials(ctx, &provider, &candidate_key, candidate_secret.as_deref());
        let verdicts = validate(ctx, probe).await?;
        verdict = verdicts[provider_name].clone();
        verified = match verdict["valid"] {
            Value::Bool(true) => Some(true),
            Value::Bool(false) => Some(false),
            _ => None, // provider unreachable — could not verify either way
        };
        if verified == Some(false) {
            if ctx.output_format == "json" {
                output::print_json(
                    &json!({ "ok": false, "provider": provider_name, "verdict": verdict }),
                );
            } else {
                eprintln!("{}", verdict_line(provider_name, &verdict));
                eprintln!("Keys NOT saved. Fix them and retry, or use --no-verify to save anyway.");
            }
            return Err(crate::client::PrintedError::new().into());
        }
    }

    // Persist to the on-disk credentials only (runtime --token/env auth is
    // never written to disk by this command).
    let mut creds = Credentials::load()?;
    match provider {
        Provider::Epo => {
            creds.epo_key = Some(candidate_key);
            creds.epo_secret = candidate_secret;
        }
        Provider::Uspto => {
            creds.uspto_key = Some(candidate_key);
        }
    }
    creds.save()?;

    if ctx.output_format == "json" {
        output::print_json(&json!({
            "ok": true,
            "saved": true,
            "verified": verified == Some(true),
            "verdict": verdict,
        }));
    } else {
        match verified {
            Some(true) => println!(
                "{} Keys verified and saved to {:?} (0600)",
                "✓".green(),
                Credentials::credentials_path()?
            ),
            _ => println!(
                "{} Keys saved to {:?} (0600) — {} verify later with {}",
                "!".yellow(),
                Credentials::credentials_path()?,
                if no_verify {
                    "NOT verified (--no-verify);"
                } else {
                    "could NOT be verified (provider unreachable);"
                },
                "flowleap keys test".cyan()
            ),
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// keys list / test / rm
// ---------------------------------------------------------------------------

fn list(ctx: &Context) -> Result<()> {
    let creds = &ctx.credentials;
    if ctx.output_format == "json" {
        // Presence + masked previews only — never the key material.
        output::print_json(&json!({
            "epo": {
                "configured": creds.epo_pair().is_some(),
                "keyPreview": creds.epo_key.as_deref().map(mask),
            },
            "uspto": {
                "configured": creds.uspto_key.is_some(),
                "keyPreview": creds.uspto_key.as_deref().map(mask),
            },
            "note": "Sources include env overrides (FLOWLEAP_EPO_KEY/FLOWLEAP_EPO_SECRET/FLOWLEAP_USPTO_KEY).",
        }));
        return Ok(());
    }
    println!("Provider keys (this machine):");
    match creds.epo_pair() {
        Some((key, _)) => println!("  epo    {} {} / secret set", "✓".green(), mask(key)),
        None => {
            if creds.epo_key.is_some() || creds.epo_secret.is_some() {
                println!(
                    "  epo    {} incomplete pair — both key and secret are required",
                    "!".yellow()
                );
            } else {
                println!("  epo    {} not set   ({})", "✗".red(), EPO_SIGNUP);
            }
        }
    }
    match &creds.uspto_key {
        Some(key) => println!("  uspto  {} {}", "✓".green(), mask(key)),
        None => println!("  uspto  {} not set   ({})", "✗".red(), USPTO_SIGNUP),
    }
    println!(
        "\nVerify against live providers: {}",
        "flowleap keys test".cyan()
    );
    Ok(())
}

async fn test(ctx: &Context) -> Result<()> {
    ctx.require_auth()?;
    if ctx.dry_run {
        output::print_json(&json!({
            "dryRun": true,
            "action": "keys test",
            "url": ctx.url("/v1/keys/validate"),
        }));
        return Ok(());
    }
    let verdicts = validate(ctx, ctx.credentials.clone()).await?;
    // Exit nonzero when any provider is invalid/missing so scripts and agents
    // can gate on `flowleap keys test`.
    let healthy = verdicts["epo"]["valid"] != Value::Bool(false)
        && verdicts["uspto"]["valid"] != Value::Bool(false);
    if ctx.output_format == "json" {
        output::print_json(&json!({ "ok": healthy, "providers": verdicts }));
    } else {
        println!("{}", verdict_line("epo", &verdicts["epo"]));
        println!("{}", verdict_line("uspto", &verdicts["uspto"]));
    }
    if !healthy {
        return Err(crate::client::PrintedError::new().into());
    }
    Ok(())
}

fn rm(ctx: &Context, provider: Provider) -> Result<()> {
    if ctx.dry_run {
        output::print_json(&json!({
            "dryRun": true,
            "action": "keys rm",
            "provider": match provider { Provider::Epo => "epo", Provider::Uspto => "uspto" },
            "wouldModify": Credentials::credentials_path()?,
        }));
        return Ok(());
    }
    let mut creds = Credentials::load()?;
    let name = match provider {
        Provider::Epo => {
            creds.epo_key = None;
            creds.epo_secret = None;
            "epo"
        }
        Provider::Uspto => {
            creds.uspto_key = None;
            "uspto"
        }
    };
    creds.save()?;
    if ctx.output_format == "json" {
        output::print_json(&json!({ "ok": true, "removed": name }));
    } else {
        println!("{} Removed {} keys.", "✓".green(), name);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Wizard (flowleap setup / flowleap keys setup)
// ---------------------------------------------------------------------------

fn prompt_epo_pair() -> Result<(String, String)> {
    let theme = ColorfulTheme::default();
    let key: String = Input::with_theme(&theme)
        .with_prompt("EPO consumer key")
        .validate_with(|v: &String| {
            if v.trim().is_empty() || v.contains(char::is_whitespace) {
                Err("must be non-empty, without spaces")
            } else {
                Ok(())
            }
        })
        .interact_text()?;
    let secret = Password::with_theme(&theme)
        .with_prompt("EPO consumer secret (hidden)")
        .interact()?;
    if secret.trim().is_empty() {
        bail!("EPO consumer secret cannot be empty.");
    }
    Ok((key.trim().to_string(), secret.trim().to_string()))
}

fn prompt_uspto_key() -> Result<String> {
    let key = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("USPTO ODP API key (hidden)")
        .interact()?;
    if key.trim().is_empty() {
        bail!("USPTO API key cannot be empty.");
    }
    Ok(key.trim().to_string())
}

fn skip_warning(provider: &str, server_has_keys: bool, commands: &str) {
    if server_has_keys {
        println!(
            "  {} Skipped — the server has its own {} keys, so commands still work.",
            "•".yellow(),
            provider.to_uppercase()
        );
    } else {
        println!();
        println!(
            "  {} Skipped {} keys — {} will fail until they are added:",
            "!".yellow().bold(),
            provider.to_uppercase(),
            commands
        );
        println!(
            "    add later with {} or {}",
            format!("flowleap keys set {}", provider).cyan(),
            "flowleap setup".cyan()
        );
        println!();
    }
}

/// One line of the fixed four-step frame: `[n/4] <label>`, label padded so
/// same-line verdicts (✓/✗) align into a column. Numbering is stable run to
/// run — steps already satisfied render as instant ✓ lines, never renumbered.
fn step(n: usize, label: &str) -> colored::ColoredString {
    format!("[{n}/4] {label:<11}").bold()
}

/// Per-step outcome, remembered for the closing recap card.
enum StepVerdict {
    /// Rendered as a green ✓ line.
    Done(String),
    /// Rendered as a yellow • line carrying the skip reason.
    Skipped(String),
}

/// Recap verdict for a skipped provider step, mirroring the server-aware
/// `skip_warning` semantics (covered by server > existing keys kept > blocking).
fn skip_verdict(
    provider_label: &str,
    covered_by_server: bool,
    kept_existing: bool,
    commands: &str,
) -> StepVerdict {
    let reason = if covered_by_server {
        "covered by server".to_string()
    } else if kept_existing {
        "kept existing keys".to_string()
    } else {
        format!("{commands} will fail until keys are added")
    };
    StepVerdict::Skipped(format!("{provider_label} skipped — {reason}"))
}

pub async fn setup_wizard(ctx: &Context) -> Result<()> {
    require_tty()?;
    let theme = ColorfulTheme::default();

    println!("{}", "FlowLeap CLI Setup".bold());
    println!("{}", "──────────────────".dimmed());

    // [1/4] Backend reachability
    let health = ctx
        .execute_json_envelope(ctx.request(reqwest::Method::GET, "/health", None))
        .await;
    match health {
        Ok(value) if value["ok"].as_bool() == Some(true) => {
            println!(
                "{} {} reachable ({})",
                step(1, "Backend"),
                "✓".green(),
                ctx.config.base_url
            );
        }
        _ => {
            println!(
                "{} {} not reachable at {} — check --base-url or start it, then re-run.",
                step(1, "Backend"),
                "✗".red(),
                ctx.config.base_url
            );
            bail!("Backend unreachable.");
        }
    }
    let backend_verdict = StepVerdict::Done("Backend reachable".to_string());

    // [2/4] Sign in — run it inline rather than bailing out of the wizard.
    // Work on a local Context so post-auth steps carry the new credential.
    let mut ctx = with_candidate_keys(ctx, ctx.credentials.clone());
    let auth_verdict;
    if ctx.credentials.auth_header().is_none() {
        println!("{}", step(2, "Sign in"));
        let choice = dialoguer::Select::with_theme(&theme)
            .with_prompt("Not signed in yet — how do you want to authenticate?")
            .items(&[
                "Sign in with browser (recommended)",
                "Paste a personal API token (fl_pat_…)",
                "Skip for now",
            ])
            .default(0)
            .interact()?;
        match choice {
            0 => {
                let access_token = crate::commands::auth::device_flow_login(&ctx).await?;
                crate::commands::auth::store_session_token(access_token.clone())?;
                ctx.credentials.token = Some(access_token);
                println!("{} Signed in", "✓".green());
                // Session tokens expire; a personal token makes this machine durable.
                if Confirm::with_theme(&theme)
                    .with_prompt("Create a long-lived API token for this machine? (recommended)")
                    .default(true)
                    .interact()?
                {
                    let prefix =
                        crate::commands::auth::mint_and_store_token(&ctx, "this-machine").await?;
                    ctx.credentials = Credentials::load()?;
                    println!(
                        "{} Personal token {} stored — sign-ins on this machine won't expire.",
                        "✓".green(),
                        prefix
                    );
                    auth_verdict = StepVerdict::Done("Authenticated (personal token)".to_string());
                } else {
                    auth_verdict = StepVerdict::Done("Authenticated (session token)".to_string());
                }
            }
            1 => {
                let token = Password::with_theme(&theme)
                    .with_prompt("Personal API token (hidden)")
                    .interact()?;
                let token = token.trim().to_string();
                if token.is_empty() {
                    bail!("Token cannot be empty.");
                }
                ctx.credentials.api_key = Some(token.clone());
                ctx.credentials.token = None;
                // Prove it works before saving.
                let probe = ctx.execute_json_envelope(ctx.get("/api/profile")).await?;
                if probe["ok"].as_bool() != Some(true) {
                    bail!(
                        "The backend rejected that token (HTTP {}). Check it and re-run setup.",
                        probe["status"]
                    );
                }
                let mut creds = Credentials::load()?;
                creds.api_key = Some(token);
                creds.token = None;
                creds.save()?;
                println!("{} Token verified and stored", "✓".green());
                auth_verdict = StepVerdict::Done("Authenticated (personal token)".to_string());
            }
            _ => {
                println!(
                    "  {} Skipped sign-in — authenticated commands will fail until you run {}",
                    "!".yellow(),
                    "flowleap auth login".cyan()
                );
                println!("  Provider keys can't be validated without auth, so setup stops here.");
                return Ok(());
            }
        }
    } else {
        println!(
            "{} {} already authenticated",
            step(2, "Sign in"),
            "✓".green()
        );
        auth_verdict = StepVerdict::Done(if ctx.credentials.token.is_some() {
            "Authenticated (session token)".to_string()
        } else {
            "Authenticated (personal token)".to_string()
        });
    }
    let ctx = &ctx;

    // What does the server already have? Drives the skip warnings.
    let baseline = validate(ctx, ctx.credentials.clone())
        .await
        .unwrap_or(json!({}));
    let epo_on_server = baseline["epo"]["source"].as_str() == Some("server");
    let uspto_on_server = baseline["uspto"]["source"].as_str() == Some("server");

    println!();
    println!("{}", "Patent data providers — bring your own keys".bold());
    println!("Keys are stored only on this machine (~/.config/flowleap/credentials.toml, 0600)");
    println!("and forwarded per-request. Each step is skippable.");
    println!();

    let mut creds = Credentials::load()?;

    // [3/4] EPO keys
    let epo_verdict;
    let epo_existing = creds.epo_pair().is_some();
    let epo_prompt = match creds.epo_pair() {
        Some((key, _)) => format!("EPO OPS keys (currently {}) — replace?", mask(key)),
        None => "Add EPO OPS keys? (worldwide patent data)".to_string(),
    };
    println!("{}", step(3, "EPO keys"));
    println!("  Get free EPO keys: {} → My apps", EPO_SIGNUP.underline());
    if Confirm::with_theme(&theme)
        .with_prompt(epo_prompt)
        .default(creds.epo_pair().is_none())
        .interact()?
    {
        loop {
            let (key, secret) = prompt_epo_pair()?;
            let spinner = indicatif::ProgressBar::new_spinner();
            spinner.set_message("Validating against EPO OPS…");
            spinner.enable_steady_tick(std::time::Duration::from_millis(90));
            // Probe carries runtime auth and ONLY the candidate EPO pair.
            let probe = probe_credentials(ctx, &Provider::Epo, &key, Some(&secret));
            let verdicts = validate(ctx, probe).await?;
            spinner.finish_and_clear();
            println!("  {}", verdict_line("epo", &verdicts["epo"]));
            match verdicts["epo"]["valid"] {
                Value::Bool(false) => {
                    if Confirm::with_theme(&theme)
                        .with_prompt("EPO rejected those keys — try again?")
                        .default(true)
                        .interact()?
                    {
                        continue;
                    }
                    skip_warning("epo", epo_on_server, "patent/ops commands");
                    epo_verdict =
                        skip_verdict("EPO", epo_on_server, epo_existing, "patent/ops commands");
                    break;
                }
                Value::Bool(true) => {
                    epo_verdict = StepVerdict::Done("EPO keys verified".to_string());
                }
                _ => {
                    if !Confirm::with_theme(&theme)
                        .with_prompt("EPO OPS is unreachable, so the keys could NOT be verified — save anyway?")
                        .default(true)
                        .interact()?
                    {
                        skip_warning("epo", epo_on_server, "patent/ops commands");
                        epo_verdict =
                            skip_verdict("EPO", epo_on_server, epo_existing, "patent/ops commands");
                        break;
                    }
                    epo_verdict =
                        StepVerdict::Done("EPO keys saved (could not verify)".to_string());
                }
            }
            creds.epo_key = Some(key);
            creds.epo_secret = Some(secret);
            break;
        }
    } else {
        skip_warning(
            "epo",
            epo_on_server || creds.epo_pair().is_some(),
            "patent/ops commands",
        );
        epo_verdict = skip_verdict("EPO", epo_on_server, epo_existing, "patent/ops commands");
    }

    // [4/4] USPTO key
    let uspto_verdict;
    let uspto_existing = creds.uspto_key.is_some();
    println!("{}", step(4, "USPTO key"));
    println!("  Get a free USPTO ODP key: {}", USPTO_SIGNUP.underline());
    let uspto_prompt = match &creds.uspto_key {
        Some(key) => format!("USPTO ODP key (currently {}) — replace?", mask(key)),
        None => "Add a USPTO ODP API key? (US prosecution data)".to_string(),
    };
    if Confirm::with_theme(&theme)
        .with_prompt(uspto_prompt)
        .default(creds.uspto_key.is_none())
        .interact()?
    {
        loop {
            let key = prompt_uspto_key()?;
            let spinner = indicatif::ProgressBar::new_spinner();
            spinner.set_message("Validating against USPTO ODP…");
            spinner.enable_steady_tick(std::time::Duration::from_millis(90));
            // Probe carries runtime auth and ONLY the candidate USPTO key, so
            // a stale stored EPO pair can't fail the probe first.
            let probe = probe_credentials(ctx, &Provider::Uspto, &key, None);
            let verdicts = validate(ctx, probe).await?;
            spinner.finish_and_clear();
            println!("  {}", verdict_line("uspto", &verdicts["uspto"]));
            match verdicts["uspto"]["valid"] {
                Value::Bool(false) => {
                    if Confirm::with_theme(&theme)
                        .with_prompt("USPTO rejected that key — try again?")
                        .default(true)
                        .interact()?
                    {
                        continue;
                    }
                    skip_warning("uspto", uspto_on_server, "uspto/citation commands");
                    uspto_verdict = skip_verdict(
                        "USPTO",
                        uspto_on_server,
                        uspto_existing,
                        "uspto/citation commands",
                    );
                    break;
                }
                Value::Bool(true) => {
                    uspto_verdict = StepVerdict::Done("USPTO key verified".to_string());
                }
                _ => {
                    if !Confirm::with_theme(&theme)
                        .with_prompt("USPTO ODP is unreachable, so the key could NOT be verified — save anyway?")
                        .default(true)
                        .interact()?
                    {
                        skip_warning("uspto", uspto_on_server, "uspto/citation commands");
                        uspto_verdict = skip_verdict(
                            "USPTO",
                            uspto_on_server,
                            uspto_existing,
                            "uspto/citation commands",
                        );
                        break;
                    }
                    uspto_verdict =
                        StepVerdict::Done("USPTO key saved (could not verify)".to_string());
                }
            }
            creds.uspto_key = Some(key);
            break;
        }
    } else {
        skip_warning(
            "uspto",
            uspto_on_server || creds.uspto_key.is_some(),
            "uspto/citation commands",
        );
        uspto_verdict = skip_verdict(
            "USPTO",
            uspto_on_server,
            uspto_existing,
            "uspto/citation commands",
        );
    }

    // Save + recap card: all four verdicts, then the existing outro.
    creds.save()?;
    println!();
    println!(
        "{} — saved to {}",
        "Setup complete".bold(),
        Credentials::credentials_path()?.display()
    );
    for verdict in [
        &backend_verdict,
        &auth_verdict,
        &epo_verdict,
        &uspto_verdict,
    ] {
        match verdict {
            StepVerdict::Done(text) => println!("  {} {}", "✓".green(), text),
            StepVerdict::Skipped(text) => println!("  {} {}", "•".yellow(), text),
        }
    }
    println!();
    println!("You're ready. Try:");
    println!(
        "  {}",
        r#"flowleap patent search --query 'ti="battery"' --limit 3"#.cyan()
    );
    println!("  {}", "flowleap keys test".cyan());
    Ok(())
}
