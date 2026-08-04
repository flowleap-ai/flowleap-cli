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
use serde_json::{json, Value};

use crate::client::{encode_url_component, Context};
use crate::output;

#[derive(Parser)]
#[command(after_help = "Examples:
  flowleap patstat graph resolve EP3477840
  flowleap patstat graph resolve \"Siemens\"
  flowleap --json patstat graph resolve EP3477840
  flowleap patstat graph neighborhood EP3477840
  flowleap patstat graph neighborhood pat:56123456 --depth 2 --edge-types cites,cited_by
  flowleap patstat graph path EP3477840 US5960411 --max-hops 3
  flowleap patstat graph explain EP3477840 --token-budget 4000
  flowleap --json patstat graph explain pat:56123456
  flowleap patstat graph patent US5960411
  flowleap patstat graph applicant 98765

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
nothing is ever auto-picked.

`graph applicant` vs `patstat portfolio`: portfolio groups filings by
name-prefix alias with grant status; `graph applicant` takes one strict
`psn_id` (from `graph resolve <name>`) and answers with its co-applicant
network, top CPC classes, and jurisdictions. The two may draw the boundary
between \"one company\" and \"several\" differently — they are not
interchangeable views of the same entity.")]
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

    /// The full citation picture of one patent in one call: header
    /// (applicants, inventors, CPC), backward patent/NPL/unresolved
    /// citations (examiner vs applicant origin), forward citations, DOCDB
    /// family with filing→grant ranges, and priority claims. Every section
    /// states its cap and TRUE total when truncated. A publication number
    /// matching several distinct applications reports the same pick-one
    /// candidates as `resolve`, reached here via HTTP 422 — never
    /// auto-picked. PATSTAT snapshot data — current legal status belongs to
    /// the live document tools.
    Patent {
        /// Publication number (e.g. EP3477840, US5960411)
        publication: String,
    },

    /// One harmonized applicant's network: entity card, filings by year, top
    /// CPC classes, jurisdictions, and the co-applicant network. Takes a
    /// strict `psn_id` (from `graph resolve <name>`) — a different entity
    /// boundary than `patstat portfolio`'s name-prefix alias grouping; the
    /// two may disagree on where one company ends and another begins.
    Applicant {
        /// Harmonized entity ID from `graph resolve` (the `psn_id` in a
        /// `person:<psn_id>` node)
        psn_id: u64,
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
        GraphCommand::Patent { publication } => patent(ctx, &publication).await,
        GraphCommand::Applicant { psn_id } => applicant(ctx, psn_id).await,
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

/// The full citation picture of one patent: `GET /v1/patstat/graph/patent/{number}`.
/// A publication number matching several distinct applications is reported by
/// the backend as a real HTTP 422 `patstat_patent_ambiguous` — `render_graph_error`
/// already renders it (candidates, never auto-picked) via `print_typed_error`.
async fn patent(ctx: &Context, publication: &str) -> Result<()> {
    let path = format!(
        "/v1/patstat/graph/patent/{}",
        encode_url_component(publication)
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
        print_patent_view(&resp_body);
    }

    Ok(())
}

/// One applicant's network: `GET /v1/patstat/graph/applicant/{psnId}`. `psn_id`
/// is a strict integer anchor from `graph resolve <name>` — a non-integer
/// value cannot be typed here (clap rejects it before any request is made);
/// an integer with no matching entity still round-trips to the backend's
/// typed `patstat_entity_not_found` (404).
async fn applicant(ctx: &Context, psn_id: u64) -> Result<()> {
    let path = format!("/v1/patstat/graph/applicant/{psn_id}");

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
        print_applicant_view(&resp_body);
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

/// The full patent screen: anchor, header (applicants/inventors/CPC),
/// citations (backward patent/NPL/unresolved, forward), family, priorities —
/// one section table at a time, each with its own truncation notice when the
/// backend capped it. Every list is printed even when empty, so an empty
/// section reads as "checked, none recorded" rather than being silently
/// skipped.
fn print_patent_view(body: &Value) {
    let meta = body.get("meta").cloned().unwrap_or(Value::Null);

    print_patent_anchor(body.get("anchor").unwrap_or(&Value::Null));

    println!("\nApplicants");
    print_section_table(
        &flatten_confidence(&candidates(body, "/header/applicants")),
        &[
            ("name", "Name"),
            ("country", "Country"),
            ("confidence", "Confidence"),
            ("at", "Provenance"),
        ],
    );

    println!("\nInventors");
    print_section_table(
        &flatten_confidence(&candidates(body, "/header/inventors")),
        &[
            ("name", "Name"),
            ("country", "Country"),
            ("confidence", "Confidence"),
            ("at", "Provenance"),
        ],
    );
    // applicants + inventors share ONE capped tls207 query (persons cap) —
    // one combined notice, not two, so the cap is not implied twice.
    print_truncation_notice(
        &meta,
        "persons",
        "person rows (applicants + inventors combined)",
    );

    println!("\nCPC Classifications");
    print_section_table(
        &flatten_confidence(&candidates(body, "/header/cpc")),
        &[
            ("symbol", "CPC Symbol"),
            ("confidence", "Confidence"),
            ("at", "Provenance"),
        ],
    );
    print_truncation_notice(&meta, "cpc", "CPC classifications");

    println!("\nBackward Citations — Patents");
    print_section_table(
        &flatten_confidence(&candidates(body, "/citations/backward_patent")),
        &[
            ("cited", "Cited"),
            ("title", "Title"),
            ("date", "Date"),
            ("origin", "Origin"),
            ("confidence", "Confidence"),
            ("at", "Provenance"),
        ],
    );
    print_truncation_notice(&meta, "backward_patent", "backward patent citations");

    // NPL and unresolved citation rows are never merged into the patent
    // citation list or dropped — they get their own labeled sections, and
    // each row's confidence tag (AMBIGUOUS for unresolved rows) is relayed
    // exactly as the backend returned it, never rewritten here.
    println!("\nBackward Citations — Non-Patent Literature");
    print_section_table(
        &flatten_confidence(&candidates(body, "/citations/backward_npl")),
        &[
            ("biblio", "Reference"),
            ("origin", "Origin"),
            ("confidence", "Confidence"),
            ("at", "Provenance"),
        ],
    );
    print_truncation_notice(&meta, "backward_npl", "backward NPL citations");

    println!("\nBackward Citations — Unresolved (flagged, not dropped)");
    print_section_table(
        &flatten_confidence(&candidates(body, "/citations/backward_unresolved")),
        &[
            ("node", "Node"),
            ("origin", "Origin"),
            ("note", "Note"),
            ("confidence", "Confidence"),
            ("at", "Provenance"),
        ],
    );
    print_truncation_notice(
        &meta,
        "backward_unresolved",
        "unresolved backward citations",
    );

    println!("\nForward Citations");
    print_section_table(
        &flatten_confidence(&candidates(body, "/citations/forward")),
        &[
            ("citing", "Citing"),
            ("title", "Title"),
            ("applicant", "Applicant"),
            ("date", "Date"),
            ("origin", "Origin"),
            ("examiner_cited", "Examiner-cited"),
            ("citing_family_size", "Citing family size"),
            ("confidence", "Confidence"),
            ("at", "Provenance"),
        ],
    );
    print_truncation_notice(&meta, "forward", "forward citations");

    println!("\nDOCDB Family");
    print_section_table(
        &flatten_family(&candidates(body, "/family")),
        &[
            ("application", "Application"),
            ("office", "Office"),
            ("range", "Filed → Granted"),
            ("is_anchor", "Anchor?"),
            ("confidence", "Confidence"),
            ("at", "Provenance"),
        ],
    );
    print_truncation_notice(&meta, "family", "family members");

    println!("\nPriority Claims");
    print_section_table(
        &flatten_confidence(&candidates(body, "/priorities")),
        &[
            ("prior_application", "Prior Application"),
            ("prior_filing_date", "Prior Filing Date"),
            ("confidence", "Confidence"),
            ("at", "Provenance"),
        ],
    );
    print_truncation_notice(&meta, "priorities", "priority claims");

    print_data_quality(&meta);
    print_provenance_footer(&meta);
}

/// The anchor header for `graph patent` — an `AnchorView`, not a
/// `ResolvedPatent` (it additionally carries `earliest_publn_date` and
/// `title_lang`, and its `filing_year`/`granted`/`docdb_family_id` fields
/// share `resolve`'s anchor shape, so the same `text`/`filing_year`/`granted`
/// helpers apply unchanged).
fn print_patent_anchor(anchor: &Value) {
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
    if let Some(earliest) = anchor.get("earliest_publn_date").and_then(Value::as_str) {
        println!("Earliest publication: {earliest}");
    }

    let publications = candidates(anchor, "/publications");
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
}

/// The applicant screen: entity card, filings by year, top CPC, jurisdictions,
/// co-applicant network — one section table at a time. `filings_by_year`
/// carries no cap (it is the full year distribution, already stripped of
/// 9999-sentinel rows, which show up in `meta.data_quality` instead), so it
/// gets no truncation notice.
fn print_applicant_view(body: &Value) {
    let meta = body.get("meta").cloned().unwrap_or(Value::Null);

    print_applicant_entity(body.get("entity").unwrap_or(&Value::Null));

    println!("\nFilings by Year");
    print_section_table(
        &candidates(body, "/filings_by_year"),
        &[("year", "Year"), ("applications", "Applications")],
    );

    println!("\nTop CPC");
    print_section_table(
        &candidates(body, "/top_cpc"),
        &[("symbol", "CPC Symbol"), ("applications", "Applications")],
    );
    print_truncation_notice(&meta, "top_cpc", "top CPC classes");

    println!("\nJurisdictions");
    print_section_table(
        &candidates(body, "/jurisdictions"),
        &[("office", "Office"), ("applications", "Applications")],
    );
    print_truncation_notice(&meta, "jurisdictions", "jurisdictions");

    println!("\nCo-Applicants");
    print_section_table(
        &flatten_confidence(&candidates(body, "/co_applicants")),
        &[
            ("name", "Name"),
            ("shared_applications", "Shared Applications"),
            ("confidence", "Confidence"),
            ("at", "Provenance"),
        ],
    );
    print_truncation_notice(&meta, "co_applicants", "co-applicants");

    print_data_quality(&meta);
    print_provenance_footer(&meta);
}

/// The entity card: an `ApplicantEntityCard` (an `EntityCandidate` plus
/// `name_grouping`) — the same PSN harmonization trust `resolve`'s entity
/// search candidates carry, plus the note explaining the grouping.
fn print_applicant_entity(entity: &Value) {
    println!(
        "Entity: {} ({})",
        text(entity, "name"),
        text(entity, "node")
    );
    println!("PSN ID: {}", text(entity, "psn_id"));
    println!(
        "Applications (as applicant): {}",
        text(entity, "applications")
    );
    println!(
        "Name variants (all recorded tls206 rows): {}",
        text(entity, "person_variants")
    );
    println!(
        "Confidence: {} (at={})",
        entity
            .pointer("/confidence/tag")
            .and_then(Value::as_str)
            .unwrap_or("?"),
        text(entity, "at"),
    );
    if let Some(note) = entity
        .pointer("/name_grouping/note")
        .and_then(Value::as_str)
    {
        let tag = entity
            .pointer("/name_grouping/confidence/tag")
            .and_then(Value::as_str)
            .unwrap_or("?");
        println!("Name grouping: {tag} — {note}");
    }
}

/// One section's table, or an explicit "none recorded" line — an empty
/// section is a checked, empty result, never a silently skipped one.
fn print_section_table(rows: &[Value], columns: &[(&str, &str)]) {
    if rows.is_empty() {
        println!("  (none recorded)");
    } else {
        output::print_table(rows, columns);
    }
}

/// Flatten each row's nested `confidence: { tag, score }` down to the tag
/// string so `output::print_table`'s generic column lookup (which only
/// renders scalars) can show it — the score still rides in the untouched
/// `--json` body.
fn flatten_confidence(rows: &[Value]) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            let mut row = row.clone();
            if let Some(tag) = row.pointer("/confidence/tag").and_then(Value::as_str) {
                row["confidence"] = json!(tag);
            }
            row
        })
        .collect()
}

/// Family rows additionally get a computed `Filed → Granted` range column
/// (filing year via the shared 9999-sentinel-safe helper, first-grant date
/// verbatim) and a yes/no `is_anchor` flag in the same style as `granted()`.
fn flatten_family(rows: &[Value]) -> Vec<Value> {
    flatten_confidence(rows)
        .into_iter()
        .map(|mut row| {
            let range = format!(
                "{} → {}",
                filing_year(&row),
                row.get("first_grant_date")
                    .and_then(Value::as_str)
                    .unwrap_or("not yet granted"),
            );
            row["range"] = json!(range);
            row["is_anchor"] = json!(
                if row.get("is_anchor").and_then(Value::as_bool) == Some(true) {
                    "yes"
                } else {
                    "no"
                }
            );
            row
        })
        .collect()
}

/// A loud per-section truncation notice, stated with the TRUE database total
/// — never the shown count presented as a total. Silent when the section did
/// not hit its cap (`meta.truncation` still carries the entry either way).
fn print_truncation_notice(meta: &Value, section: &str, label: &str) {
    let Some(info) = meta.pointer(&format!("/truncation/{section}")) else {
        return;
    };
    if info.get("truncated").and_then(Value::as_bool) != Some(true) {
        return;
    }
    let shown = info.get("shown").and_then(Value::as_u64).unwrap_or(0);
    let total = info.get("total").and_then(Value::as_u64).unwrap_or(0);
    match info.get("ranking").and_then(Value::as_str) {
        Some(ranking) => println!("  Showing {shown} of {total} {label} (ranked by {ranking})."),
        None => println!("  Showing {shown} of {total} {label}."),
    }
}

/// `meta.data_quality` flags (e.g. a 9999-sentinel filing date) render as
/// flags, never silently dropped — a data gap stays visible as a gap.
fn print_data_quality(meta: &Value) {
    let flags = candidates(meta, "/data_quality");
    if flags.is_empty() {
        return;
    }
    println!("\nData quality flags:");
    for flag in &flags {
        println!(
            "  - {}: {} — {}",
            text(flag, "node"),
            text(flag, "issue"),
            text(flag, "detail"),
        );
    }
}

/// Every composite result closes with its Data Edition, the EPO attribution
/// line, and the snapshot-honesty caveat — current legal status is a live
/// document tools question, never a PATSTAT one.
fn print_provenance_footer(meta: &Value) {
    if let Some(edition) = meta.get("data_edition").and_then(Value::as_str) {
        println!("\nSource: PATSTAT data edition {edition}.");
    }
    if let Some(attribution) = meta.get("attribution").and_then(Value::as_str) {
        println!("{attribution}");
    }
    println!(
        "This is PATSTAT snapshot data — for current legal status, use the live document \
         tools (flowleap ops legal, flowleap uspto)."
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
