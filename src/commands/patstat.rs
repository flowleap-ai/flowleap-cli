use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use crate::client::Context;
use crate::output;

mod graph;

#[derive(Parser)]
#[command(after_help = "Examples:
  flowleap patstat portfolio Siemens
  flowleap patstat portfolio \"Kia Motors\" --from-year 2015 --to-year 2024
  flowleap patstat docs --section semantic-model
  flowleap patstat docs --section examples
  flowleap patstat query \"SELECT office, COUNT(DISTINCT family_id) AS inventions FROM flowleap.applications GROUP BY office\" --question \"filings by office\"
  flowleap patstat graph resolve EP3477840

Note: an ambiguous applicant name (matching several distinct corporate
entities) is never merged or auto-picked — re-run with one exact candidate
name from the list the command prints. For query, fetch
`docs --section semantic-model` first and follow the guarded-sql workflow
(`docs --workflow guarded-sql`): one informed retry per error, then stop.
Criteria shape picks the surface: aggregate counts by structured criteria are
portfolio/query, a named node and its relationships is `graph`.")]
pub struct PatstatArgs {
    #[command(subcommand)]
    command: PatstatCommand,
}

#[derive(Subcommand)]
enum PatstatCommand {
    /// Aggregate patent portfolio for one applicant: filings by year, office,
    /// and grant status. An ambiguous applicant name prints its candidates
    /// instead of guessing — re-run with one exact name from that list.
    Portfolio {
        /// Applicant name or harmonized PSN name prefix (e.g. "Siemens")
        applicant: String,

        /// Earliest filing year, inclusive (default: to-year - 9)
        #[arg(long)]
        from_year: Option<i32>,

        /// Latest filing year, inclusive (default: current year)
        #[arg(long)]
        to_year: Option<i32>,
    },

    /// Run ONE guarded SQL SELECT against the flowleap.* semantic views
    /// (Layer 2, backend ADR 0010). The backend gates deterministically:
    /// single-SELECT parse check, flowleap-only allowlist, EXPLAIN cost
    /// ceiling, 5000-row/5MB hard caps, 20s timeout. A typed patstat_sql_*
    /// error message carries the exact fix instruction — fix the SQL once,
    /// re-run with --retry-of, then stop.
    Query {
        /// One SQL SELECT; every table schema-qualified as flowleap.<view>
        sql: String,

        /// The user's question, verbatim (audit-only; feeds the query-review
        /// pipeline — always send it)
        #[arg(long)]
        question: Option<String>,

        /// On a retry only: marker tying this attempt to the error it fixes
        #[arg(long)]
        retry_of: Option<String>,
    },

    /// PATSTAT analytics docs and served sections. --section semantic-model
    /// is the full schema + interpretation-conventions YAML (read BEFORE
    /// writing query SQL); --section examples is verified question→SQL pairs.
    Docs {
        /// Served data section: semantic-model | examples
        #[arg(long, conflicts_with_all = ["workflow", "endpoint", "compact"])]
        section: Option<String>,

        /// Workflow guide by name (e.g. guarded-sql)
        #[arg(long, conflicts_with_all = ["endpoint", "compact"])]
        workflow: Option<String>,

        /// One endpoint's docs by name (e.g. query, portfolio)
        #[arg(long, conflicts_with = "compact")]
        endpoint: Option<String>,

        /// Compact endpoint/workflow/section listing
        #[arg(long)]
        compact: bool,
    },

    /// Graph Analytics over the PATSTAT snapshot: a named node and the
    /// relationships around it — citation networks, family coverage,
    /// applicant landscapes — every edge carrying a confidence tag and
    /// row-level provenance. Aggregate counts stay with portfolio/query.
    Graph(graph::GraphArgs),
}

pub async fn run(ctx: &Context, args: PatstatArgs) -> Result<()> {
    ctx.require_auth()?;

    match args.command {
        PatstatCommand::Portfolio {
            applicant,
            from_year,
            to_year,
        } => portfolio(ctx, &applicant, from_year, to_year).await,
        PatstatCommand::Query {
            sql,
            question,
            retry_of,
        } => query(ctx, &sql, question, retry_of).await,
        PatstatCommand::Docs {
            section,
            workflow,
            endpoint,
            compact,
        } => docs(ctx, section, workflow, endpoint, compact).await,
        PatstatCommand::Graph(args) => graph::run(ctx, args).await,
    }
}

async fn query(
    ctx: &Context,
    sql: &str,
    question: Option<String>,
    retry_of: Option<String>,
) -> Result<()> {
    let mut body = json!({ "sql": sql });
    if let Some(question) = question {
        body["question"] = json!(question);
    }
    if let Some(retry_of) = retry_of {
        body["retryOf"] = json!(retry_of);
    }

    let envelope = ctx
        .execute_json_envelope(ctx.post("/v1/patstat/query", &body))
        .await?;
    if envelope.get("dryRun").and_then(Value::as_bool) == Some(true) {
        output::print_json(&envelope);
        return Ok(());
    }

    let http_ok = envelope.get("ok").and_then(Value::as_bool) == Some(true);
    let resp_body = envelope.get("body").cloned().unwrap_or(Value::Null);

    if !http_ok {
        return Err(render_query_error(ctx, &envelope, &resp_body));
    }

    if ctx.output_format == "json" {
        output::print_json(&resp_body);
        return Ok(());
    }

    print_query_result(&resp_body);
    Ok(())
}

/// Typed rendering for the guarded-SQL error family. Every `patstat_sql_*`
/// message already carries the exact parser/Postgres detail plus a recovery
/// line (the retry prompt) — it is relayed VERBATIM, never rephrased, with
/// the one-retry contract appended. `patstat_busy` is a back-off signal
/// (retry the SAME SQL after Retry-After), never a rewrite signal.
fn render_query_error(ctx: &Context, envelope: &Value, body: &Value) -> anyhow::Error {
    let code = body
        .pointer("/error/code")
        .and_then(Value::as_str)
        .unwrap_or("");

    if code.starts_with("patstat_sql_") || code == "patstat_busy" {
        if ctx.output_format == "json" {
            output::print_json(body);
        } else {
            let message = body
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Guarded SQL rejected.");
            println!("Guarded SQL rejected ({code}): {message}");
            for key in [
                "sqlstate",
                "position",
                "hint",
                "detail",
                "relations",
                "estimated_cost",
                "cost_ceiling",
                "row_cap",
                "timeout_ms",
            ] {
                if let Some(value) = body.pointer(&format!("/error/{key}")) {
                    println!("  {key}: {value}");
                }
            }
            println!();
            if code == "patstat_busy" {
                println!(
                    "Back off and retry the SAME SQL — this is load, not a problem with the query."
                );
            } else {
                println!(
                    "Fix the SQL once per the instruction above and re-run with --retry-of {code}; \
                     after a second failure, stop and report the error."
                );
            }
        }
    } else if code == "patstat_unavailable" {
        render_unavailable(ctx, body);
    } else {
        render_generic_error(ctx, envelope);
    }

    match envelope.get("status").and_then(Value::as_u64) {
        Some(status) => crate::client::PrintedError::with_status(status as u16).into(),
        None => crate::client::PrintedError::new().into(),
    }
}

/// Render guarded-SQL rows as a table with generic columns (taken from the
/// first row), then rowCount, warn-band warnings, and the data-edition
/// provenance line. The interpretation contract rides along: state the
/// counting unit/year basis chosen, and always name the edition.
fn print_query_result(result: &Value) {
    let rows = result
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if rows.is_empty() {
        println!("0 rows — the filters may be too narrow, or the entity/CPC prefix may not match.");
        println!("Probe candidates before assuming absence (see: flowleap patstat docs --workflow guarded-sql).");
    } else {
        let keys: Vec<String> = rows[0]
            .as_object()
            .map(|obj| obj.keys().cloned().collect())
            .unwrap_or_default();
        let columns: Vec<(&str, &str)> = keys.iter().map(|k| (k.as_str(), k.as_str())).collect();
        output::print_table(&rows, &columns);
    }

    if let Some(row_count) = result.get("rowCount").and_then(Value::as_u64) {
        println!(
            "
Rows: {row_count}"
        );
    }
    if let Some(warnings) = result.get("warnings").and_then(Value::as_array) {
        for warning in warnings {
            let code = warning
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("warning");
            let message = warning.get("message").and_then(Value::as_str).unwrap_or("");
            println!("Warning ({code}): {message}");
        }
    }
    if let Some(edition) = result.get("data_edition").and_then(Value::as_str) {
        println!("Source: PATSTAT data edition {edition} — state the interpretation used (counting unit, year basis) and name this edition when quoting the numbers.");
    }
}

async fn docs(
    ctx: &Context,
    section: Option<String>,
    workflow: Option<String>,
    endpoint: Option<String>,
    compact: bool,
) -> Result<()> {
    let path = if let Some(section) = &section {
        format!("/v1/patstat/docs?section={section}")
    } else if let Some(workflow) = &workflow {
        format!("/v1/patstat/docs?workflow={workflow}")
    } else if let Some(endpoint) = &endpoint {
        format!("/v1/patstat/docs?endpoint={endpoint}")
    } else if compact {
        "/v1/patstat/docs?format=compact".to_string()
    } else {
        "/v1/patstat/docs".to_string()
    };

    let envelope = ctx.execute_json_envelope(ctx.get(&path)).await?;
    if envelope.get("dryRun").and_then(Value::as_bool) == Some(true) {
        output::print_json(&envelope);
        return Ok(());
    }

    let http_ok = envelope.get("ok").and_then(Value::as_bool) == Some(true);
    let resp_body = envelope.get("body").cloned().unwrap_or(Value::Null);

    if !http_ok {
        let code = resp_body
            .pointer("/error/code")
            .and_then(Value::as_str)
            .unwrap_or("");
        if code == "patstat_unavailable" {
            render_unavailable(ctx, &resp_body);
        } else {
            render_generic_error(ctx, &envelope);
        }
        return Err(match envelope.get("status").and_then(Value::as_u64) {
            Some(status) => crate::client::PrintedError::with_status(status as u16).into(),
            None => crate::client::PrintedError::new().into(),
        });
    }

    // The semantic model ships as verbatim YAML — print it raw in human mode
    // so nothing is lost between the backend's single source and the agent.
    if ctx.output_format != "json" {
        if let Some(yaml) = resp_body.pointer("/data/yaml").and_then(Value::as_str) {
            if let Some(edition) = resp_body
                .pointer("/data/data_edition")
                .and_then(Value::as_str)
            {
                println!("# Loaded edition: {edition}");
            }
            println!("{yaml}");
            return Ok(());
        }
    }

    output::print_json(&resp_body);
    Ok(())
}

async fn portfolio(
    ctx: &Context,
    applicant: &str,
    from_year: Option<i32>,
    to_year: Option<i32>,
) -> Result<()> {
    let mut body = json!({ "applicant": applicant });
    if let Some(from_year) = from_year {
        body["fromYear"] = json!(from_year);
    }
    if let Some(to_year) = to_year {
        body["toYear"] = json!(to_year);
    }

    let envelope = ctx
        .execute_json_envelope(ctx.post("/v1/patstat/portfolio", &body))
        .await?;
    if envelope.get("dryRun").and_then(Value::as_bool) == Some(true) {
        output::print_json(&envelope);
        return Ok(());
    }

    let http_ok = envelope.get("ok").and_then(Value::as_bool) == Some(true);
    let resp_body = envelope.get("body").cloned().unwrap_or(Value::Null);

    if !http_ok {
        return Err(render_error(ctx, &envelope, &resp_body));
    }

    if ctx.output_format == "json" {
        output::print_json(&resp_body);
        return Ok(());
    }

    print_portfolio(&resp_body);
    Ok(())
}

/// Render a failed portfolio call and return the [`PrintedError`] the
/// top-level handler maps to the documented exit code.
///
/// Two typed PATSTAT error codes get dedicated rendering in both output
/// modes (never a raw envelope dump): an ambiguous applicant prints its
/// candidate list as an interaction step (never auto-picked), and an
/// unconfigured deployment states plainly that PATSTAT is unavailable.
/// Anything else (auth, rate limit, generic upstream failure, …) falls back
/// to the shared envelope + hint-box rendering every other command uses.
///
/// [`PrintedError`]: crate::client::PrintedError
fn render_error(ctx: &Context, envelope: &Value, body: &Value) -> anyhow::Error {
    let code = body
        .pointer("/error/code")
        .and_then(Value::as_str)
        .unwrap_or("");

    match code {
        "patstat_applicant_ambiguous" => render_ambiguous(ctx, body),
        "patstat_unavailable" => render_unavailable(ctx, body),
        _ => render_generic_error(ctx, envelope),
    }

    match envelope.get("status").and_then(Value::as_u64) {
        Some(status) => crate::client::PrintedError::with_status(status as u16).into(),
        None => crate::client::PrintedError::new().into(),
    }
}

/// The candidate-list rendering the ambiguity flow needs in both output
/// modes: the backend never merges distinct applicant entities, so this
/// command never auto-picks one either — it prints the candidates and tells
/// the caller to re-run with one exact name.
fn render_ambiguous(ctx: &Context, body: &Value) {
    let message = body
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("The applicant name matches several distinct entities.");
    let candidates = body
        .pointer("/error/candidates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if ctx.output_format == "json" {
        output::print_json(&json!({
            "ok": false,
            "error": {
                "code": "patstat_applicant_ambiguous",
                "message": message,
                "candidates": candidates,
            },
        }));
        return;
    }

    println!("Ambiguous applicant — {message}");
    println!();
    println!("Candidates:");
    for candidate in &candidates {
        let name = candidate.get("name").and_then(Value::as_str).unwrap_or("?");
        let applications = candidate
            .get("applications")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        println!("  - {name} ({applications} applications)");
    }
    println!();
    println!(
        "These may be separate companies, so none is picked automatically. Re-run with one \
         exact candidate name from the list above."
    );
}

/// The PATSTAT analytics layer is not configured on this deployment — a
/// typed unavailability, not a retryable error, so it is stated plainly
/// rather than rendered as a generic upstream failure.
fn render_unavailable(ctx: &Context, body: &Value) {
    let message = body
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or(
            "The PATSTAT analytics layer is not configured on this deployment. Aggregate \
             portfolio analytics are unavailable.",
        );

    if ctx.output_format == "json" {
        output::print_json(&json!({
            "ok": false,
            "error": {
                "code": "patstat_unavailable",
                "message": message,
            },
        }));
        return;
    }

    println!("PATSTAT analytics unavailable: backend has no PATSTAT dataset configured.");
    println!("{message}");
}

/// The shared envelope + hint-box rendering every other command uses
/// (mirrors `Context::print_error_envelope`, which is private to the client
/// module) — kept for error shapes this command has no dedicated rendering
/// for (auth failure, rate limit, generic upstream failure, …).
fn render_generic_error(ctx: &Context, envelope: &Value) {
    output::print_value(&ctx.output_format, envelope, &[]);
    if ctx.output_format != "json" {
        if let Some(hint) = envelope.get("providerKeysHint") {
            crate::client::print_keys_hint_box(hint);
        }
        if let Some(hint) = envelope.get("subscriptionHint") {
            crate::client::print_subscription_hint_box(hint);
        }
        if let Some(hint) = envelope.get("rateLimitHint") {
            crate::client::print_rate_limit_hint_box(hint);
        }
    }
}

/// Render a successful portfolio result for human/table output: the
/// backend's quotable summary first, then the year and office aggregates as
/// tables, any grant-status/data caveats, and a provenance line naming the
/// loaded PATSTAT edition.
fn print_portfolio(result: &Value) {
    if let Some(summary) = result.get("summary").and_then(Value::as_str) {
        println!("{summary}");
    }

    print_aggregate(
        result,
        "by_year",
        "Filings by Year",
        &[
            ("year", "Year"),
            ("applications", "Applications"),
            ("granted", "Granted"),
        ],
    );
    print_aggregate(
        result,
        "by_office",
        "Filings by Office",
        &[
            ("office", "Office"),
            ("applications", "Applications"),
            ("granted", "Granted"),
        ],
    );

    print_notes(result, "grant_status_caveats", "Grant status caveats");
    print_notes(result, "notes", "Notes");

    if let Some(edition) = result.get("data_edition").and_then(Value::as_str) {
        println!("\nSource: PATSTAT data edition {edition}");
    }
}

fn print_aggregate(result: &Value, key: &str, label: &str, columns: &[(&str, &str)]) {
    println!("\n{label}");
    match result.get(key).and_then(Value::as_array) {
        Some(rows) if !rows.is_empty() => output::print_table(rows, columns),
        _ => println!("  (no data)"),
    }
}

fn print_notes(result: &Value, key: &str, label: &str) {
    let Some(notes) = result.get(key).and_then(Value::as_array) else {
        return;
    };
    if notes.is_empty() {
        return;
    }
    println!("\n{label}:");
    for note in notes {
        if let Some(text) = note.as_str() {
            println!("  - {text}");
        }
    }
}
