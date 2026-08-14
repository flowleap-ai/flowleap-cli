use anyhow::Result;
use serde_json::{json, Value};

use crate::client::Context;
use crate::config::{Config, Credentials};
use crate::output;

pub async fn run(ctx: &Context) -> Result<()> {
    let config_path = Config::config_path()?;
    let credentials_path = Credentials::credentials_path()?;
    let auth_source = auth_source(ctx);

    // Readiness (/v1/health), not liveness: it is equally public and carries
    // the server's apiVersion, so one probe answers "is it up" and "which
    // build" together (backend ADR 0014 rule 6).
    let health = ctx
        .execute_json_envelope(ctx.request(reqwest::Method::GET, "/v1/health", None))
        .await;

    let (reachable, status, api_version, error, error_kind, hint) = match health {
        Ok(value) => {
            let reachable = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            let status = value.get("status").and_then(|v| v.as_u64());
            let api_version = value
                .pointer("/body/apiVersion")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let hint = if reachable {
                None
            } else {
                Some("Backend responded but not with a successful status.")
            };
            (reachable, status, api_version, None, None, hint)
        }
        Err(err) => {
            let message = err.to_string();
            let (kind, hint) = classify_error(&message);
            (false, None, None, Some(message), Some(kind), Some(hint))
        }
    };

    let authenticated = ctx.credentials.auth_header().is_some();

    // Best-effort server verdicts (POST /v1/keys/validate) so "key missing
    // locally but covered by the server" produces no next step. Any failure —
    // unauthenticated, unreachable, HTTP error — falls back to local key
    // presence; doctor never errors because of this call.
    let verdicts: Option<Value> = if reachable && authenticated && !ctx.dry_run {
        crate::commands::keys::validate(ctx, ctx.credentials.clone())
            .await
            .ok()
    } else {
        None
    };

    let next_steps = next_steps(ctx, authenticated, verdicts.as_ref());
    // Ready means nothing blocks work — stricter than `ok` (reachability).
    let ready = reachable && authenticated && next_steps.is_empty();

    let key_validation = match &verdicts {
        Some(_) => json!({ "source": "server", "note": Value::Null }),
        None => json!({
            "source": "local",
            "note": "Server key validation was unavailable (unauthenticated, unreachable, or the call failed) — provider verdicts reflect local key presence only. Missing keys may still be covered by the server; check with 'flowleap keys test' once authenticated.",
        }),
    };

    let report = json!({
        "ok": reachable,
        "ready": ready,
        "command": "flowleap",
        "baseUrl": ctx.config.base_url,
        "auth": {
            "available": ctx.credentials.auth_header().is_some(),
            "source": auth_source,
            "setup": if ctx.credentials.auth_header().is_some() {
                serde_json::Value::Null
            } else {
                json!("Run `flowleap auth login --api-key <key>` or set FLOWLEAP_API_KEY/FLOWLEAP_TOKEN.")
            },
        },
        "config": {
            "path": config_path,
            "credentialsPath": credentials_path,
            "defaultModel": ctx.config.default_model,
            "outputFormat": ctx.config.output_format,
        },
        "providerKeys": {
            "epo": ctx.credentials.epo_pair().is_some(),
            "epoIncompletePair": ctx.credentials.epo_pair().is_none()
                && (ctx.credentials.epo_key.is_some() || ctx.credentials.epo_secret.is_some()),
            "uspto": ctx.credentials.uspto_key.is_some(),
            "setup": if ctx.credentials.epo_pair().is_some() && ctx.credentials.uspto_key.is_some() {
                serde_json::Value::Null
            } else {
                json!("Missing keys may be fine if the server has its own — check with 'flowleap keys test'. Add local keys via 'flowleap setup' (interactive) or 'flowleap keys set <provider>'.")
            },
        },
        "keyValidation": key_validation,
        "backend": {
            "reachable": reachable,
            "healthStatus": status,
            "apiVersion": api_version,
            "error": error,
            "errorKind": error_kind,
            "hint": hint,
        },
        "cli": cli_status(),
        "skills": crate::commands::skills::doctor_skills_status(env!("CARGO_PKG_VERSION")),
        "nextSteps": next_steps,
    });

    if ctx.output_format == "json" {
        output::print_value(&ctx.output_format, &report, &[]);
    } else {
        render_human(&report);
    }

    // Exit contract: 0 iff ready, else the generic-failure code (1). The
    // checklist above is always fully emitted first; PrintedError tells the
    // top-level handler not to print anything more. Dry-run sends nothing and
    // so can never prove readiness — it keeps its historical success exit.
    if ready || ctx.dry_run {
        Ok(())
    } else {
        Err(crate::client::PrintedError::new().into())
    }
}

/// Human rendering of the doctor report: a ✓/✗/• checklist mirroring the JSON
/// sections (backend, auth, provider keys, CLI, skills), then — only when
/// something is pending — a numbered "Next steps:" list rendering the same
/// `nextSteps` data, each step tagged with its actor. Reads exclusively from
/// the report so the human view can never drift from `--json`; the exit
/// contract is shared with JSON mode and unchanged here.
fn render_human(report: &Value) {
    use colored::Colorize;

    println!("{}", "FlowLeap doctor".bold());
    println!();

    // Backend
    let base_url = report["baseUrl"].as_str().unwrap_or_default();
    if report["backend"]["reachable"] == Value::Bool(true) {
        match report["backend"]["apiVersion"].as_str() {
            Some(version) => println!(
                "  {} Backend reachable ({base_url}, API {version})",
                "✓".green()
            ),
            None => println!("  {} Backend reachable ({base_url})", "✓".green()),
        }
    } else {
        println!("  {} Backend unreachable ({base_url})", "✗".red());
        if let Some(hint) = report["backend"]["hint"].as_str() {
            println!("    {}", hint.dimmed());
        }
    }

    // Auth — the credential kind uses the domain vocabulary: an api-key
    // source is a personal token (durable), a token source is a session
    // token (expires on its own; doctor pends mint-personal-token).
    if report["auth"]["available"] == Value::Bool(true) {
        let kind = match report["auth"]["source"].as_str() {
            Some("env-api-key") | Some("config-api-key") => "personal token",
            _ => "session token",
        };
        println!("  {} Authenticated ({kind})", "✓".green());
    } else {
        println!("  {} Not signed in", "✗".red());
    }

    // Provider keys, one line each.
    provider_line(report, "epo", "EPO keys", "store-epo-keys");
    provider_line(report, "uspto", "USPTO key", "store-uspto-key");

    // CLI version — an available upgrade is informational (•), never blocking.
    let version = report["cli"]["currentVersion"].as_str().unwrap_or_default();
    match &report["cli"]["updateAvailable"] {
        Value::Bool(true) => println!(
            "  {} CLI {version} — {} available: {}",
            "•".yellow(),
            report["cli"]["latestVersion"]
                .as_str()
                .unwrap_or("a newer version"),
            "flowleap upgrade".cyan(),
        ),
        Value::Bool(false) => println!("  {} CLI {version} (latest)", "✓".green()),
        _ => println!("  {} CLI {version}", "✓".green()),
    }

    // Skills — stale installs are informational (•), never blocking.
    let stale = report["skills"]["stale"].as_array().map_or(0, Vec::len);
    if stale == 0 {
        println!("  {} Skills up to date", "✓".green());
    } else {
        println!(
            "  {} {stale} stale skill install(s) — refresh: {}",
            "•".yellow(),
            "flowleap skills update".cyan(),
        );
    }

    // Next steps — same pending-only, actor-tagged data as the JSON contract.
    // A ready machine has none and the section is omitted entirely.
    let steps = report["nextSteps"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    if !steps.is_empty() {
        println!();
        println!("{}", "Next steps:".bold());
        for (index, step) in steps.iter().enumerate() {
            let tag = format!("[{}]", step["actor"].as_str().unwrap_or("agent"));
            let title = step["title"].as_str().unwrap_or_default();
            match step["run"].as_str() {
                Some(run) => println!("  {}. {} {title}: {}", index + 1, tag.bold(), run.cyan()),
                None => println!("  {}. {} {title}", index + 1, tag.bold()),
            }
            if let Some(url) = step["url"].as_str() {
                println!("     {}", url.cyan());
            }
        }
    }
}

/// One provider checklist line, derived from the report alone. Blocking is
/// "this provider has a pending store step in nextSteps": ✗ when blocking,
/// ✓ when keys are set locally, and • when neither — the only way to be
/// non-blocking without local keys is server coverage (informational, not a
/// gap to chase).
fn provider_line(report: &Value, provider: &str, label: &str, store_step_id: &str) {
    use colored::Colorize;

    let local = report["providerKeys"][provider] == Value::Bool(true);
    let pending = report["nextSteps"]
        .as_array()
        .is_some_and(|steps| steps.iter().any(|s| s["id"] == store_step_id));
    let server_checked = report["keyValidation"]["source"] == "server";

    match (pending, local) {
        (false, true) => println!("  {} {label}: set locally", "✓".green()),
        (false, false) => println!(
            "  {} {label}: none locally — covered by server",
            "•".yellow()
        ),
        (true, true) => println!("  {} {label}: set locally, rejected by server", "✗".red()),
        // With a server verdict we know coverage is absent; on the
        // local-presence fallback we only know the keys are not set.
        (true, false) if server_checked => {
            println!("  {} {label}: not set, not covered", "✗".red())
        }
        (true, false) => println!("  {} {label}: not set", "✗".red()),
    }
}

/// The pending, blocking onboarding steps in dependency order. Step ids are a
/// public contract (see docs/adr/0001): `auth-login`, `mint-personal-token`,
/// `obtain-epo-keys`, `store-epo-keys`, `obtain-uspto-key`, `store-uspto-key`,
/// `verify-keys`. Steps whose need is already covered (e.g. a provider the
/// server has its own keys for) are omitted — the list means "what blocks
/// you", not "what could be configured".
fn next_steps(ctx: &Context, authenticated: bool, verdicts: Option<&Value>) -> Vec<Value> {
    let mut steps = Vec::new();

    if !authenticated {
        steps.push(step(
            "auth-login",
            "human",
            "Sign in to FlowLeap (browser device flow — run the command, relay the verification URL and code to the human, and wait for approval)",
            Some("flowleap --json auth login"),
            None,
        ));
    } else if session_only(&ctx.credentials) {
        steps.push(step(
            "mint-personal-token",
            "agent",
            "Mint and store a long-lived personal token — the session token expires on its own",
            Some("flowleap --json auth create-token --name <n> --store"),
            None,
        ));
    }

    let epo_pending = provider_pending(verdicts, "epo", ctx.credentials.epo_pair().is_some());
    let uspto_pending = provider_pending(verdicts, "uspto", ctx.credentials.uspto_key.is_some());

    if epo_pending {
        steps.push(step(
            "obtain-epo-keys",
            "human",
            "Sign up for free EPO OPS credentials (browser: 'My apps' → create app)",
            None,
            Some(crate::commands::keys::EPO_SIGNUP),
        ));
        steps.push(step(
            "store-epo-keys",
            "agent",
            "Store the EPO consumer key and secret",
            Some("flowleap keys set epo --key <k> --secret <s>"),
            None,
        ));
    }
    if uspto_pending {
        steps.push(step(
            "obtain-uspto-key",
            "human",
            "Sign up for a free USPTO ODP API key (browser)",
            None,
            Some(crate::commands::keys::USPTO_SIGNUP),
        ));
        steps.push(step(
            "store-uspto-key",
            "agent",
            "Store the USPTO ODP API key",
            Some("flowleap keys set uspto --key <k>"),
            None,
        ));
    }
    if epo_pending || uspto_pending {
        steps.push(step(
            "verify-keys",
            "agent",
            "Verify the stored provider keys against the live providers",
            Some("flowleap --json keys test"),
            None,
        ));
    }

    steps
}

/// Session-only auth: signed in with a short-lived Clerk session token and no
/// durable fl_pat_ personal token to fall back on. Durability is a pending
/// step (`mint-personal-token`) until one exists.
fn session_only(creds: &Credentials) -> bool {
    creds.token.is_some()
        && !creds
            .api_key
            .as_deref()
            .is_some_and(|key| key.starts_with("fl_pat_"))
}

/// Whether a provider blocks work. With server verdicts (source
/// user|server|none, valid true|false|null): server-covered providers never
/// block, and — mirroring `keys test` — a provider blocks only when provably
/// absent everywhere (`source: "none"`) or provably invalid (`valid: false`).
/// Without verdicts (unauthenticated / call failed), fall back to local key
/// presence.
fn provider_pending(verdicts: Option<&Value>, provider: &str, local_present: bool) -> bool {
    match verdicts {
        Some(verdicts) => {
            let verdict = &verdicts[provider];
            let source = verdict["source"].as_str().unwrap_or("unknown");
            source != "server" && (verdict["valid"] == Value::Bool(false) || source == "none")
        }
        None => !local_present,
    }
}

/// One next step: stable kebab-case id, exactly one actor ("human" |
/// "agent"), a title, and optionally a runnable command and/or a URL.
fn step(id: &str, actor: &str, title: &str, run: Option<&str>, url: Option<&str>) -> Value {
    let mut step = json!({
        "id": id,
        "actor": actor,
        "title": title,
    });
    if let Some(run) = run {
        step["run"] = json!(run);
    }
    if let Some(url) = url {
        step["url"] = json!(url);
    }
    step
}

/// CLI self-status: install channel and any known-available upgrade, read
/// from the daily update cache (no extra network call) so doctor stays
/// offline-safe. Always recommends the channel-aware `flowleap upgrade`.
fn cli_status() -> serde_json::Value {
    let current = env!("CARGO_PKG_VERSION");
    let channel = crate::commands::upgrade::current_channel();
    let latest = crate::update::cached_latest();
    let update_available = latest
        .as_deref()
        .map(|l| crate::update::is_newer(l, current));
    let hint = if update_available == Some(true) {
        Some(format!(
            "flowleap {} is available (you have {current}) — run 'flowleap upgrade'",
            latest.as_deref().unwrap_or_default()
        ))
    } else {
        None
    };
    json!({
        "channel": channel.as_str(),
        "currentVersion": current,
        "latestVersion": latest,
        "updateAvailable": update_available,
        "upgradeCommand": "flowleap upgrade",
        "hint": hint,
    })
}

fn classify_error(message: &str) -> (&'static str, &'static str) {
    let lower = message.to_ascii_lowercase();
    if lower.contains("operation not permitted") {
        (
            "network-permission",
            "The runtime blocked network access. Retry with network permission or run from a normal shell.",
        )
    } else if lower.contains("connection refused") || lower.contains("couldn't connect") {
        (
            "connection-refused",
            "No backend accepted the connection. Start the backend or check --base-url.",
        )
    } else if lower.contains("dns") || lower.contains("failed to lookup") {
        (
            "dns",
            "The host could not be resolved. Check --base-url and network connectivity.",
        )
    } else if lower.contains("timed out") || lower.contains("timeout") {
        (
            "timeout",
            "The backend did not respond before the request timed out.",
        )
    } else {
        (
            "request-error",
            "The health request failed. Check --base-url, backend status, and network access.",
        )
    }
}

fn auth_source(ctx: &Context) -> &'static str {
    crate::client::credential_source(ctx)
}
