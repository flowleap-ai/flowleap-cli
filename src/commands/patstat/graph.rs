//! `flowleap patstat graph` — the Graph Analytics command family (PRD 0002).
//!
//! Graph Analytics is the third PATSTAT engine, routed by criteria shape:
//! free-text keywords are a Topic question, aggregate counts by structured
//! criteria are a Portfolio question (`patstat portfolio` / `patstat query`),
//! and *a named node and the relationships around it* is a Graph question.
//!
//! Every verb is a thin 1:1 relay of one `GET /v1/patstat/graph/*` route:
//! `--json` emits the backend body unmodified (including error bodies for the
//! typed graph family), human mode renders it, and ambiguity is always an
//! interaction step — candidates are printed, never auto-picked.

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::Value;

use crate::client::{encode_url_component, Context};
use crate::output;

#[derive(Parser)]
#[command(after_help = "Examples:
  flowleap patstat graph resolve EP3477840
  flowleap patstat graph resolve \"Siemens\"
  flowleap patstat graph neighborhood EP3477840
  flowleap patstat graph neighborhood pat:56123456 --depth 2 --edge-types cites,cited_by
  flowleap patstat graph path EP3477840 US5960411 --max-hops 3
  flowleap patstat graph explain EP3477840 --token-budget 4000
  flowleap --json patstat graph explain pat:56123456

Criteria shape picks the engine: a named node and its relationships (citations,
family, co-applicants) is a graph question; aggregate counts by structured
criteria stay with `patstat portfolio` / `patstat query`, and free-text keyword
discovery stays with `patent search`.

Node arguments take a `pat:<appln_id>` id or a publication number. If a number
is ambiguous the verbs refuse it rather than guess — run `graph resolve` first
and pass the `pat:` id of the one you meant.

Graph answers are PATSTAT snapshot data — for current legal status use the live
document tools (`flowleap ops legal`, `flowleap uspto`). A publication number
matching several distinct applications prints its candidates and exits 1;
nothing is ever auto-picked.")]
pub struct GraphArgs {
    #[command(subcommand)]
    command: GraphCommand,
}

#[derive(Subcommand)]
enum GraphCommand {
    /// Map an input onto a graph node: a publication number resolves to its
    /// `pat:<appln_id>` anchor, free text to ranked harmonized (PSN) entity
    /// candidates whose `psn_id` anchors the applicant landscape. Start here
    /// when you hold only a number or a company name.
    Resolve {
        /// Publication number (e.g. EP3477840, US5960411) or applicant name
        query: String,
    },

    /// Bounded expansion around one node: the edges reachable in 1–2 hops,
    /// examiner citations ranked first, each carrying its confidence tag and
    /// `at=` provenance ref. Per-hop cap 200 with loud TRUNCATED notices —
    /// narrow with --edge-types rather than reading a capped list as complete.
    Neighborhood {
        /// `pat:<appln_id>` node id, or a publication number to resolve
        node: String,

        /// Hops to expand: 1 or 2 (default 1; 2 is much wider)
        #[arg(long)]
        depth: Option<i32>,

        /// Comma-separated edge subset, e.g. cites,cited_by. Full set: cites,
        /// cited_by, in_family, has_applicant, has_inventor, classified_as,
        /// claims_priority
        #[arg(long)]
        edge_types: Option<String>,

        /// Token budget for the text serialization, clamped to 100–20000
        /// (default 2000). Trims `text` only — `--json` data stays complete.
        #[arg(long)]
        token_budget: Option<i32>,
    },

    /// Shortest citation/family path between two patents (bidirectional BFS).
    /// Absence of a path is not proof of unrelatedness — unrelated technology
    /// areas commonly have none, and the search itself reports its own limits.
    Path {
        /// First endpoint: `pat:<appln_id>` or a publication number
        a: String,

        /// Second endpoint: `pat:<appln_id>` or a publication number
        b: String,

        /// Maximum hops to search: 1–4 (default 4)
        #[arg(long)]
        max_hops: Option<i32>,

        /// Token budget for the text serialization, clamped to 100–20000
        /// (default 2000)
        #[arg(long)]
        token_budget: Option<i32>,
    },

    /// One node's card plus its top connections, with everything beyond the
    /// top grouped by relation carrying TRUE counts — the "why does this node
    /// matter" view.
    Explain {
        /// `pat:<appln_id>` node id, or a publication number to resolve
        node: String,

        /// Token budget for the text serialization, clamped to 100–20000
        /// (default 2000)
        #[arg(long)]
        token_budget: Option<i32>,
    },
}

pub async fn run(ctx: &Context, args: GraphArgs) -> Result<()> {
    match args.command {
        GraphCommand::Resolve { query } => resolve(ctx, &query).await,
        GraphCommand::Neighborhood {
            node,
            depth,
            edge_types,
            token_budget,
        } => {
            verb(
                ctx,
                "neighborhood",
                &[
                    ("node", Some(node)),
                    ("depth", depth.map(|depth| depth.to_string())),
                    ("edge_types", edge_types),
                    ("token_budget", token_budget.map(|n| n.to_string())),
                ],
            )
            .await
        }
        GraphCommand::Path {
            a,
            b,
            max_hops,
            token_budget,
        } => {
            verb(
                ctx,
                "path",
                &[
                    ("a", Some(a)),
                    ("b", Some(b)),
                    ("max_hops", max_hops.map(|hops| hops.to_string())),
                    ("token_budget", token_budget.map(|n| n.to_string())),
                ],
            )
            .await
        }
        GraphCommand::Explain { node, token_budget } => {
            verb(
                ctx,
                "explain",
                &[
                    ("node", Some(node)),
                    ("token_budget", token_budget.map(|n| n.to_string())),
                ],
            )
            .await
        }
    }
}

/// Run one agent verb: `GET /v1/patstat/graph/<verb>` with the caller's
/// parameters, then relay the result.
///
/// The three verbs differ only in which parameters they take — the response
/// contract (`{ success, text, data }`), the error family, and the relay
/// discipline are identical, so they share one execution path. Bounds
/// (`depth` 1–2, `max_hops` 1–4, `token_budget` 100–20000) are deliberately
/// NOT re-checked here: the backend owns them, and relaying its typed
/// `patstat_invalid_request` keeps one source of truth instead of two that
/// can drift.
async fn verb(ctx: &Context, verb: &str, params: &[(&str, Option<String>)]) -> Result<()> {
    let path = format!("/v1/patstat/graph/{verb}?{}", query_string(params));

    let envelope = ctx.execute_json_envelope(ctx.get(&path)).await?;
    if envelope.get("dryRun").and_then(Value::as_bool) == Some(true) {
        output::print_json(&envelope);
        return Ok(());
    }

    let http_ok = envelope.get("ok").and_then(Value::as_bool) == Some(true);
    let resp_body = envelope.get("body").cloned().unwrap_or(Value::Null);

    if !http_ok {
        return Err(render_graph_error(ctx, &envelope, &resp_body));
    }

    if ctx.output_format == "json" {
        output::print_json(&resp_body);
    } else {
        print_verb_text(&resp_body);
    }

    Ok(())
}

/// Query string from the parameters that are actually set. Absent flags are
/// omitted entirely rather than sent as defaults, so the backend's documented
/// defaults stay the single source of truth.
fn query_string(params: &[(&str, Option<String>)]) -> String {
    params
        .iter()
        .filter_map(|(key, value)| {
            value
                .as_ref()
                .map(|value| format!("{key}={}", encode_url_component(value)))
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// Human mode is the backend `text` field printed VERBATIM.
///
/// That string is already the product the verbs exist to produce: a
/// token-budgeted, line-per-fact serialization where every edge carries its
/// confidence tag and `at=` provenance ref, labels are quoted as inert data
/// (injection-guarded), the Data Edition rides in the header, and truncation
/// is announced in-band. Re-rendering it here could only lose those
/// guarantees, so nothing is reformatted, summarized, or reordered — a
/// `found: false` path prints its own NOT FOUND line and exits 0, because a
/// searched-and-absent answer is a successful answer.
fn print_verb_text(body: &Value) {
    match body.get("text").and_then(Value::as_str) {
        Some(text) => println!("{text}"),
        // No `text` field means this is not the response shape the verb
        // contract promises — show the caller what actually arrived rather
        // than printing nothing.
        None => output::print_json(body),
    }
}

async fn resolve(ctx: &Context, query: &str) -> Result<()> {
    let path = format!(
        "/v1/patstat/graph/resolve?q={}",
        encode_url_component(query)
    );

    let envelope = ctx.execute_json_envelope(ctx.get(&path)).await?;
    if envelope.get("dryRun").and_then(Value::as_bool) == Some(true) {
        output::print_json(&envelope);
        return Ok(());
    }

    let http_ok = envelope.get("ok").and_then(Value::as_bool) == Some(true);
    let resp_body = envelope.get("body").cloned().unwrap_or(Value::Null);

    if !http_ok {
        return Err(render_graph_error(ctx, &envelope, &resp_body));
    }

    if ctx.output_format == "json" {
        output::print_json(&resp_body);
    } else {
        print_resolve(&resp_body);
    }

    // One number behind several distinct applications is the same interaction
    // step the composites report as HTTP 422 — resolve just reaches it on a
    // 200. Reporting it through the 422 exit mapping keeps one contract for
    // the whole family: ambiguity never exits 0, so a script cannot mistake a
    // pick-one prompt for a resolved anchor.
    if resp_body.get("kind").and_then(Value::as_str) == Some("ambiguous") {
        return Err(crate::client::PrintedError::with_status(422).into());
    }

    Ok(())
}

/// Render a failed graph call and return the [`PrintedError`] the top-level
/// handler maps to the documented exit code.
///
/// The typed graph error family is relayed rather than rephrased: in `--json`
/// the backend error body is printed **unmodified** (same thin-relay rule as
/// the success bodies, so one parse path serves both), and human mode gets
/// dedicated per-code rendering. Everything else — auth failure, rate limit,
/// a non-envelope upstream error — falls back to the shared envelope +
/// hint-box rendering every other command uses, which is where the
/// `providerKeysHint` / `subscriptionHint` / `rateLimitHint` contracts live.
///
/// Shared across the family: `#55`'s verbs and `#56`'s composites call this
/// for every non-2xx, and `patstat_patent_ambiguous` is reachable only from
/// the composites — it is handled here so they inherit it.
///
/// [`PrintedError`]: crate::client::PrintedError
pub(super) fn render_graph_error(ctx: &Context, envelope: &Value, body: &Value) -> anyhow::Error {
    let code = body
        .pointer("/error/code")
        .and_then(Value::as_str)
        .unwrap_or("");

    let typed = matches!(
        code,
        "patstat_patent_not_found"
            | "patstat_entity_not_found"
            | "patstat_patent_ambiguous"
            | "patstat_invalid_request"
            | "patstat_unavailable"
    );

    if !typed {
        super::render_generic_error(ctx, envelope);
    } else if ctx.output_format == "json" {
        output::print_json(body);
    } else {
        print_typed_error(ctx, body, code);
    }

    match envelope.get("status").and_then(Value::as_u64) {
        Some(status) => crate::client::PrintedError::with_status(status as u16).into(),
        None => crate::client::PrintedError::new().into(),
    }
}

/// Human-mode rendering for the typed graph errors. Each backend message
/// already states the recovery action (a shorter name prefix, a fuller number
/// form, the country prefix), so it is printed verbatim under a headline that
/// names which lookup failed.
fn print_typed_error(ctx: &Context, body: &Value, code: &str) {
    let message = body
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("The graph engine rejected the request.");

    match code {
        "patstat_unavailable" => super::render_unavailable(ctx, body),
        "patstat_patent_ambiguous" => {
            print_ambiguous(message, candidates(body, "/error/candidates"))
        }
        "patstat_patent_not_found" => {
            println!("No such publication in the loaded PATSTAT edition.\n{message}")
        }
        "patstat_entity_not_found" => {
            println!("No such applicant entity in the loaded PATSTAT edition.\n{message}")
        }
        _ => println!("Invalid graph request.\n{message}"),
    }
}

/// Render a successful resolve result: one of three kinds, each answering a
/// different question the caller may not know they asked (a number they typed
/// may be one anchor or several; a name is always a pick-one list).
fn print_resolve(body: &Value) {
    match body.get("kind").and_then(Value::as_str) {
        Some("patent") => print_anchor(body.get("anchor").unwrap_or(&Value::Null)),
        Some("ambiguous") => print_ambiguous(
            &format!(
                "\"{}\" matches {} distinct applications.",
                text(body, "input"),
                candidates(body, "/candidates").len()
            ),
            candidates(body, "/candidates"),
        ),
        Some("entities") => print_entities(body),
        _ => output::print_json(body),
    }
}

/// The resolved anchor. The publications are listed because several
/// publications of ONE application collapse into a single anchor — seeing the
/// number that was typed among them is how the caller confirms the collapse
/// was the intended one.
fn print_anchor(anchor: &Value) {
    println!("Anchor: {}", text(anchor, "node"));
    println!("Application: {}", text(anchor, "application"));
    if let Some(title) = anchor.get("title").and_then(Value::as_str) {
        println!("Title: {title}");
    }
    println!(
        "Filed: {} · Granted: {} · DOCDB family: {}",
        filing_year(anchor),
        granted(anchor),
        text(anchor, "docdb_family_id"),
    );

    let publications = anchor
        .get("publications")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !publications.is_empty() {
        println!("\nPublications:");
        for publication in &publications {
            let first_grant =
                if publication.get("first_grant").and_then(Value::as_bool) == Some(true) {
                    "  first grant"
                } else {
                    ""
                };
            println!(
                "  - {}  {}{first_grant}  at={}",
                text(publication, "publn"),
                text(publication, "date"),
                text(publication, "at"),
            );
        }
    }

    println!(
        "\nConfidence: {} (at={}) — PATSTAT snapshot data; for current legal status use the live document tools.",
        text(anchor, "confidence"),
        text(anchor, "at"),
    );
}

/// One publication number behind several distinct applications. The backend
/// never picks between them and neither does this command: the candidates are
/// printed with the kind codes that discriminate them, and the caller re-runs
/// with a number form that names exactly one.
fn print_ambiguous(message: &str, candidates: Vec<Value>) {
    println!("Ambiguous publication number — {message}");
    println!();
    println!("Candidates:");
    for (index, candidate) in candidates.iter().enumerate() {
        println!(
            "  {}. {} — {}, filed {}, {}",
            index + 1,
            text(candidate, "node"),
            text(candidate, "application"),
            filing_year(candidate),
            if candidate.get("granted").and_then(Value::as_bool) == Some(true) {
                "granted"
            } else {
                "not granted"
            },
        );
        if let Some(title) = candidate.get("title").and_then(Value::as_str) {
            println!("       Title: {title}");
        }
        let publications: Vec<String> = candidate
            .get("publications")
            .and_then(Value::as_array)
            .map(|publications| {
                publications
                    .iter()
                    .map(|publication| {
                        format!(
                            "{} ({})",
                            text(publication, "publn"),
                            text(publication, "date")
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        if !publications.is_empty() {
            println!("       Publications: {}", publications.join(", "));
        }
    }
    println!();
    println!(
        "None is picked automatically. Re-run with the kind code (e.g. A1 vs B1) or a fuller \
         number form so exactly one application matches."
    );
}

/// Ranked harmonized-entity candidates, largest portfolio first. Truncation is
/// stated with the TRUE total — a shown count read as a total is the failure
/// mode this rendering exists to prevent.
fn print_entities(body: &Value) {
    let candidates = candidates(body, "/candidates");
    let total = body.get("total").and_then(Value::as_u64);
    let truncated = body.get("truncated").and_then(Value::as_bool) == Some(true);

    println!("Applicant entities matching \"{}\"", text(body, "input"));
    match (truncated, total) {
        (true, Some(total)) => {
            println!("Showing {} of {total} matching entities.", candidates.len())
        }
        _ => println!("{} matching entities.", candidates.len()),
    }
    println!();

    output::print_table(
        &candidates,
        &[
            ("psn_id", "PSN ID"),
            ("name", "Name"),
            ("applications", "Applications"),
            ("person_variants", "Name variants"),
        ],
    );

    println!(
        "\nHarmonized-name grouping is an inference, not a recorded fact — pick one psn_id \
         rather than merging entities. That psn_id is the anchor the `graph applicant` \
         landscape takes."
    );
}

fn candidates(body: &Value, pointer: &str) -> Vec<Value> {
    body.pointer(pointer)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// A field as display text, whether the backend typed it as a string or a
/// number. Absent and null both render as `?` rather than as `null`.
fn text(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(string)) => string.clone(),
        Some(Value::Null) | None => "?".to_string(),
        Some(other) => other.to_string(),
    }
}

/// PATSTAT stores an unknown filing year as the 9999 sentinel, which the
/// backend already maps to null — never quote either as a year.
fn filing_year(value: &Value) -> String {
    match value.get("filing_year").and_then(Value::as_i64) {
        Some(year) => year.to_string(),
        None => "unknown".to_string(),
    }
}

fn granted(value: &Value) -> &'static str {
    match value.get("granted").and_then(Value::as_bool) {
        Some(true) => "yes",
        _ => "no",
    }
}
