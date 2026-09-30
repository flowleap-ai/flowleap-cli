use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};

use crate::client::Context;
use crate::commands::tools;
use crate::output;

mod graph;

#[derive(Parser)]
#[command(after_help = "Examples:
  flowleap patstat portfolio Siemens
  flowleap patstat portfolio \"Kia Motors\" --from-year 2015 --to-year 2024
  flowleap patstat docs --section semantic-model --part index
  flowleap patstat docs --section semantic-model --view applications
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

/// How much of the year-by-office matrix `patstat_portfolio` returns.
#[derive(Clone, Copy, ValueEnum)]
enum Offices {
    /// The 8 largest offices plus one OTHER row per year (the tool default)
    Top,
    /// Every office
    All,
}

impl Offices {
    fn as_str(self) -> &'static str {
        match self {
            Offices::Top => "top",
            Offices::All => "all",
        }
    }
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

        /// Offices in the year-by-office matrix: `all` keeps every office;
        /// `top` keeps the 8 largest plus one OTHER row per year, the compact
        /// form MCP clients get by default
        #[arg(long, value_enum, default_value_t = Offices::All)]
        offices: Offices,
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
    /// is the full schema + interpretation-conventions YAML; read it in parts
    /// BEFORE writing query SQL: --part index first, then --view <name> for
    /// each view you will query. --section examples is verified
    /// question→SQL pairs.
    Docs {
        /// Served data section: semantic-model | examples
        #[arg(long, conflicts_with_all = ["workflow", "endpoint", "compact"])]
        section: Option<String>,

        /// One part of the semantic model: index (the catalog of views,
        /// conventions and metrics). Only with --section semantic-model.
        #[arg(long, conflicts_with_all = ["view", "workflow", "endpoint", "compact"])]
        part: Option<String>,

        /// One logical table (view) of the semantic model in full, with its
        /// conventions and join paths; names come from --part index. Only
        /// with --section semantic-model.
        #[arg(long, conflicts_with_all = ["workflow", "endpoint", "compact"])]
        view: Option<String>,

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
    if let PatstatCommand::Docs {
        section,
        part,
        view,
        ..
    } = &args.command
    {
        check_semantic_model_part(ctx, section.as_deref(), part.is_some() || view.is_some())?;
    }
    ctx.require_auth()?;

    match args.command {
        PatstatCommand::Portfolio {
            applicant,
            from_year,
            to_year,
            offices,
        } => portfolio(ctx, &applicant, from_year, to_year, offices).await,
        PatstatCommand::Query {
            sql,
            question,
            retry_of,
        } => query(ctx, &sql, question, retry_of).await,
        PatstatCommand::Docs {
            section,
            part,
            view,
            workflow,
            endpoint,
            compact,
        } => {
            let selector = DocsSelector {
                section,
                part,
                view,
                workflow,
                endpoint,
                compact,
            };
            docs(ctx, selector).await
        }
        PatstatCommand::Graph(args) => graph::run(ctx, args).await,
    }
}

async fn query(
    ctx: &Context,
    sql: &str,
    question: Option<String>,
    retry_of: Option<String>,
) -> Result<()> {
    let mut input = json!({ "sql": sql });
    if let Some(question) = question {
        input["question"] = json!(question);
    }
    if let Some(retry_of) = retry_of {
        input["retry_of"] = json!(retry_of);
    }

    // `patstat_query` is sent exactly once — the tool seam switches the
    // client's generic resend off for guarded SQL (see
    // `tools::retry_policy_for`).
    let data = match call(ctx, "patstat_query", &input).await? {
        Outcome::DryRun => return Ok(()),
        Outcome::Failed(envelope) => return Err(render_query_error(ctx, &envelope)),
        Outcome::Data(data) => data,
    };

    if ctx.output_format == "json" {
        output::print_json(&data);
        return Ok(());
    }

    print_query_result(&data);
    Ok(())
}

/// What one PATSTAT tool call came back with.
enum Outcome {
    /// `--dry-run`: the request description is already printed.
    DryRun,
    /// The tool's `data` payload, unwrapped from the facade envelope.
    Data(Value),
    /// The raw response envelope of a failed call (nothing printed yet), so
    /// the typed PATSTAT errors keep their dedicated rendering.
    Failed(Value),
}

/// Run one PATSTAT tool on the shared tool seam (`POST /v1/tools/<name>`).
/// Unlike `tools::call_tool_data`, a failure is handed back unprinted: the
/// ambiguity, guarded-SQL and unavailability errors render their own way.
async fn call(ctx: &Context, tool: &str, input: &Value) -> Result<Outcome> {
    let envelope = tools::call_tool_envelope(ctx, tool, input).await?;
    let dry_run = envelope.get("dryRun").and_then(Value::as_bool) == Some(true);
    if !dry_run && envelope.get("ok").and_then(Value::as_bool) != Some(true) {
        return Ok(Outcome::Failed(envelope));
    }
    // A dry run is the envelope itself; a success is its body.
    let result = if dry_run {
        envelope
    } else {
        envelope.get("body").cloned().unwrap_or(Value::Null)
    };
    Ok(match tools::unwrap_tool_result(ctx, result) {
        Some(data) => Outcome::Data(data),
        None => Outcome::DryRun,
    })
}

/// The backend error envelope of a failed call (`{ success: false, error }`).
fn error_body(envelope: &Value) -> Value {
    envelope.get("body").cloned().unwrap_or(Value::Null)
}

/// One extra field of a typed PATSTAT error: the facade nests the route's
/// extra fields under `error.details`.
fn error_field<'a>(body: &'a Value, key: &str) -> Option<&'a Value> {
    body.pointer(&format!("/error/details/{key}"))
}

/// The documented exit code for a failed call's HTTP status.
fn printed_error(envelope: &Value) -> anyhow::Error {
    match envelope.get("status").and_then(Value::as_u64) {
        Some(status) => crate::client::PrintedError::with_status(status as u16).into(),
        None => crate::client::PrintedError::new().into(),
    }
}

/// Typed rendering for the guarded-SQL error family. Every `patstat_sql_*`
/// message already carries the exact parser/Postgres detail plus a recovery
/// line (the retry prompt) — it is relayed VERBATIM, never rephrased, with
/// the one-retry contract appended. `patstat_busy` is a back-off signal
/// (retry the SAME SQL after Retry-After), never a rewrite signal.
fn render_query_error(ctx: &Context, envelope: &Value) -> anyhow::Error {
    let body = &error_body(envelope);
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
                if let Some(value) = error_field(body, key) {
                    println!("  {key}: {value}");
                }
            }
            println!();
            if code == "patstat_busy" {
                println!(
                    "Back off and retry the SAME SQL — this is load, not a problem with the query."
                );
            } else if code == "patstat_sql_timeout" {
                // A cold cache fails like a heavy query (backend #402): the
                // one informed retry is the same SQL, not a rewrite.
                println!(
                    "Cold timeout: re-run the SAME SQL once with --retry-of patstat_sql_timeout; \
                     only if it times out again, narrow it."
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

    printed_error(envelope)
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

/// The docs selector flags, as parsed (clap keeps the families exclusive).
struct DocsSelector {
    section: Option<String>,
    part: Option<String>,
    view: Option<String>,
    workflow: Option<String>,
    endpoint: Option<String>,
    compact: bool,
}

/// The one flag rule clap cannot state: `--part` and `--view` select inside
/// the semantic model, so they need `--section semantic-model` exactly. A
/// broken combination is a usage error, rendered the way `main` renders a
/// clap parse error (JSON envelope on stdout in JSON mode, stderr otherwise)
/// and exiting 2, before any credential check or request.
fn check_semantic_model_part(
    ctx: &Context,
    section: Option<&str>,
    selects_part: bool,
) -> Result<()> {
    if !selects_part || section == Some("semantic-model") {
        return Ok(());
    }
    let (kind, message) = match section {
        None => (
            clap::error::ErrorKind::MissingRequiredArgument,
            "--part and --view need --section semantic-model",
        ),
        Some(_) => (
            clap::error::ErrorKind::ArgumentConflict,
            "--part and --view need --section semantic-model (not another section)",
        ),
    };
    let err = clap::Error::raw(kind, format!("{message}\n"));
    if ctx.output_format == "json" {
        output::print_usage_error_json(&err);
    } else {
        eprint!("{err}");
    }
    Err(crate::client::PrintedError::with_exit_code(err.exit_code()).into())
}

async fn docs(ctx: &Context, selector: DocsSelector) -> Result<()> {
    let data = match call(ctx, "patstat_docs", &docs_input(&selector)).await? {
        Outcome::DryRun => return Ok(()),
        Outcome::Failed(envelope) => {
            let body = error_body(&envelope);
            let available = error_field(&body, "availableViews").and_then(Value::as_array);
            if body.pointer("/error/code").and_then(Value::as_str) == Some("patstat_unavailable") {
                render_unavailable(ctx, &body);
            } else if let (Some(views), false) = (available, ctx.output_format == "json") {
                print_unknown_view(ctx, &body, views);
            } else {
                render_generic_error(ctx, &envelope);
            }
            return Err(printed_error(&envelope));
        }
        Outcome::Data(data) => data,
    };

    if ctx.output_format != "json" {
        // The semantic model ships as verbatim YAML — print it raw in human
        // mode so nothing is lost between the backend's single source and
        // the agent.
        if let Some(yaml) = data.get("yaml").and_then(Value::as_str) {
            print_edition(&data);
            println!("{yaml}");
            return Ok(());
        }
        if selector.part.is_some() && data.get("logical_tables").is_some() {
            print_model_index(&data);
            return Ok(());
        }
        if selector.view.is_some() && data.get("view").is_some() {
            print_model_view(&data);
            return Ok(());
        }
    }

    output::print_json(&data);
    Ok(())
}

/// The `patstat_docs` input for the docs selectors (clap keeps them mutually
/// exclusive). The tool answers an empty input with the compact manifest, so
/// the no-flag run asks for `compact: false` — the full docs, as before.
fn docs_input(selector: &DocsSelector) -> Value {
    if let Some(section) = &selector.section {
        let mut input = json!({ "section": section });
        if let Some(part) = &selector.part {
            input["part"] = json!(part);
        }
        if let Some(view) = &selector.view {
            input["view"] = json!(view);
        }
        input
    } else if let Some(workflow) = &selector.workflow {
        json!({ "workflow": workflow })
    } else if let Some(endpoint) = &selector.endpoint {
        json!({ "endpoint": endpoint })
    } else {
        json!({ "compact": selector.compact })
    }
}

fn print_edition(data: &Value) {
    if let Some(edition) = data.get("data_edition").and_then(Value::as_str) {
        println!("# Loaded edition: {edition}");
    }
}

/// The index for human output: the served note, the general interpretation
/// conventions as short sections, then one table row per logical table.
/// Glossary, metrics and global caveats stay in the JSON output.
fn print_model_index(data: &Value) {
    print_edition(data);
    if let Some(note) = data.get("note").and_then(Value::as_str) {
        println!("{note}");
    }
    print_topics("Conventions", data.get("interpretation_conventions"));

    let rows: Vec<Value> = data
        .get("logical_tables")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|table| {
            json!({
                "view": table.get("name"),
                "description": table.get("description"),
                "columns": table.get("columns"),
            })
        })
        .collect();
    println!("\nViews");
    output::print_table_whole(
        &rows,
        &[
            ("view", "View"),
            ("description", "Description"),
            ("columns", "Columns"),
        ],
    );
    println!(
        "\nRead one view in full: flowleap patstat docs --section semantic-model --view <name>"
    );
    println!("Glossary, metrics and global caveats: add --output json.");
}

/// One view for human output: its columns table, then the conventions topics
/// it names, then the join paths that name it.
fn print_model_view(data: &Value) {
    print_edition(data);
    let view = &data["view"];
    let name = view.get("name").and_then(Value::as_str).unwrap_or("view");
    match view.get("description").and_then(Value::as_str) {
        Some(description) => println!("{name}: {description}"),
        None => println!("{name}"),
    }
    if let Some(physical) = view.get("physical").and_then(Value::as_str) {
        println!("Physical: {physical}");
    }

    let rows: Vec<Value> = view
        .get("columns")
        .and_then(Value::as_object)
        .map(|columns| {
            columns
                .iter()
                .map(|(column, spec)| {
                    json!({
                        "column": column,
                        "type": spec.get("type"),
                        "description": spec.get("description"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    println!("\nColumns");
    output::print_table_whole(
        &rows,
        &[
            ("column", "Column"),
            ("type", "Type"),
            ("description", "Description"),
        ],
    );

    print_topics("Conventions", data.get("interpretation_conventions"));
    // The view answer's top-level global_caveats are already the ones that
    // bear on this view (backend #485); the view object carries none.
    print_topics("Caveats", data.get("global_caveats"));

    let paths = data
        .get("join_paths")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if !paths.is_empty() {
        println!("\nJoin paths");
        for path in paths {
            match path.as_str() {
                Some(text) => println!("  - {text}"),
                None => println!("  - {path}"),
            }
        }
    }
}

/// A `{ topic: text | { key: text } }` map as short sections. Values that
/// are neither a string nor a map print as compact JSON.
fn print_topics(label: &str, topics: Option<&Value>) {
    let Some(topics) = topics.and_then(Value::as_object).filter(|t| !t.is_empty()) else {
        return;
    };
    println!("\n{label}");
    for (topic, value) in topics {
        match value {
            Value::String(text) => println!("  {topic}: {text}"),
            Value::Object(entries) => {
                println!("  {topic}:");
                for (key, entry) in entries {
                    match entry.as_str() {
                        Some(text) => println!("    {key}: {text}"),
                        None => println!("    {key}: {entry}"),
                    }
                }
            }
            other => println!("  {topic}: {other}"),
        }
    }
}

/// An unknown `--view`, in human mode: the backend's message and the views
/// it offers, through the shared value printer like other human errors.
fn print_unknown_view(ctx: &Context, body: &Value, views: &[Value]) {
    let message = body
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("Unknown view");
    output::print_value(
        &ctx.output_format,
        &json!({ "error": message, "availableViews": views }),
        &[("error", "Error"), ("availableViews", "Available views")],
    );
}

async fn portfolio(
    ctx: &Context,
    applicant: &str,
    from_year: Option<i32>,
    to_year: Option<i32>,
    offices: Offices,
) -> Result<()> {
    // The tool defaults to `top` so MCP replies stay small; the CLI is not
    // size-limited, so it asks for every office unless told otherwise (#107).
    let mut input = json!({ "applicant": applicant, "offices": offices.as_str() });
    if let Some(from_year) = from_year {
        input["from_year"] = json!(from_year);
    }
    if let Some(to_year) = to_year {
        input["to_year"] = json!(to_year);
    }

    let data = match call(ctx, "patstat_portfolio", &input).await? {
        Outcome::DryRun => return Ok(()),
        Outcome::Failed(envelope) => return Err(render_error(ctx, &envelope)),
        Outcome::Data(data) => data,
    };

    if ctx.output_format == "json" {
        output::print_json(&data);
        return Ok(());
    }

    print_portfolio(&data);
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
fn render_error(ctx: &Context, envelope: &Value) -> anyhow::Error {
    let body = &error_body(envelope);
    let code = body
        .pointer("/error/code")
        .and_then(Value::as_str)
        .unwrap_or("");

    match code {
        "patstat_applicant_ambiguous" => render_ambiguous(ctx, body),
        "patstat_unavailable" => render_unavailable(ctx, body),
        _ => render_generic_error(ctx, envelope),
    }

    printed_error(envelope)
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
    let candidates = error_field(body, "candidates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if ctx.output_format == "json" {
        output::print_json(&json!({
            "ok": false,
            "error": {
                "code": "patstat_applicant_ambiguous",
                "message": message,
                "details": body.pointer("/error/details").cloned().unwrap_or_else(|| json!({})),
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
    print_office_scope(result);
    print_other_matches(result);

    print_notes(result, "grant_status_caveats", "Grant status caveats");
    print_notes(result, "notes", "Notes");

    if let Some(edition) = result.get("data_edition").and_then(Value::as_str) {
        println!("\nSource: PATSTAT data edition {edition}");
    }
}

/// One line when the tool cut the year-by-office matrix (`offices: "top"`).
fn print_office_scope(result: &Value) {
    let Some(scope) = result.get("by_year_office_scope") else {
        return;
    };
    if scope.get("truncated").and_then(Value::as_bool) != Some(true) {
        return;
    }
    let shown = scope.get("offices_shown").and_then(Value::as_u64);
    let total = scope.get("offices_total").and_then(Value::as_u64);
    if let (Some(shown), Some(total)) = (shown, total) {
        println!(
            "\nShowing {shown} of {total} offices in the year-by-office matrix \
             (--offices all for every office)"
        );
    }
}

/// The other applicant entities that matched the name but were not merged,
/// with the count the tool's cap of 10 leaves out.
fn print_other_matches(result: &Value) {
    let applicant = result.get("applicant");
    let matches = applicant
        .and_then(|a| a.get("other_matches"))
        .and_then(Value::as_array);
    let Some(matches) = matches.filter(|m| !m.is_empty()) else {
        return;
    };
    println!("\nOther matching applicants (not merged)");
    output::print_table(
        matches,
        &[("name", "Name"), ("applications", "Applications")],
    );
    let total = applicant
        .and_then(|a| a.get("other_matches_total"))
        .and_then(Value::as_u64);
    if let Some(total) = total {
        let shown = matches.len() as u64;
        if total > shown {
            println!(
                "  … and {} more (the tool caps the list at 10)",
                total - shown
            );
        }
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
