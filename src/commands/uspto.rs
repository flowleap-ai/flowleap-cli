use std::io::Read;
use std::path::PathBuf;

use anyhow::{bail, Context as AnyhowContext, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use crate::client::Context;
use crate::commands::tools;
use crate::output;

#[derive(Parser)]
pub struct UsptoArgs {
    #[command(subcommand)]
    command: UsptoCommand,
}

#[derive(Subcommand)]
enum UsptoCommand {
    /// Search USPTO Open Data Portal records with an ODP Lucene query
    ///
    /// Provide either a `--query` Lucene string (wrapped as `{"q": ...}`) or a
    /// full ODP request body via `--body` / `--body-file`. Write the Lucene
    /// query yourself — the flowleap-uspto skill carries the method.
    Search {
        /// USPTO ODP Lucene query string (wrapped as `{"q": ...}`)
        #[arg(long, short, conflicts_with_all = ["body", "body_file"])]
        query: Option<String>,

        /// Full ODP request body as inline JSON ({"q": "<lucene>", ...}).
        /// Pass `-` to read the body from stdin.
        #[arg(long, conflicts_with = "body_file")]
        body: Option<String>,

        /// File containing a full ODP request body as JSON
        #[arg(long)]
        body_file: Option<PathBuf>,

        /// Maximum results to return (ignored when the body already sets pagination)
        #[arg(long, default_value = "10")]
        limit: u32,

        /// Only report the result count (cheap probe: limit 1)
        #[arg(long)]
        count_only: bool,
    },
    /// Get a granted patent by patent number
    Grant {
        /// Patent number (for example, 11800000)
        patent_number: String,
    },
    /// Get a patent application by application number
    Application {
        /// Application number
        app_number: String,
    },
    /// Get application continuity data
    Continuity {
        /// Application number
        app_number: String,
    },
    /// Prosecution transaction/event history (filings, office actions, fees)
    Transactions {
        /// Application number
        app_number: String,
    },
    /// Recorded assignments — reel/frame, assignors, assignees (chain of title)
    Assignments {
        /// Application number
        app_number: String,
    },
    /// Foreign priority claims (filing date, application number, IP office)
    ForeignPriority {
        /// Application number
        app_number: String,
    },
    /// Official patent term adjustment (PTA) day counts and history
    Adjustment {
        /// Application number
        app_number: String,
    },
    /// Attorney/agent of record — power of attorney, customer number
    Attorney {
        /// Application number
        app_number: String,
    },
    /// List Image File Wrapper (IFW) documents — office actions, responses, notices
    Documents {
        /// Application number
        app_number: String,

        /// Filter by USPTO document code (e.g. CTNF, CTFR, NOA, CLM, REM)
        #[arg(long)]
        code: Option<String>,

        /// Filter by direction: INCOMING (applicant→office) or OUTGOING (office→applicant)
        #[arg(long, value_parser = ["incoming", "outgoing"])]
        direction: Option<String>,
    },
    /// Fetch one IFW document as OCR-extracted markdown text
    ///
    /// Downloads the document PDF from USPTO ODP and OCRs it server-side (most
    /// IFW documents are scanned images with no text layer). Get the document
    /// id from `uspto documents`. First read of a long document can take tens
    /// of seconds; results are cached server-side for 7 days.
    DocumentText {
        /// Application number
        app_number: String,

        /// IFW documentIdentifier from `uspto documents` (e.g. LAQYXZN3XBLUEX4)
        document_id: String,
    },
}

pub async fn run(ctx: &Context, args: UsptoArgs) -> Result<()> {
    ctx.require_auth()?;

    match args.command {
        UsptoCommand::Search {
            query,
            body,
            body_file,
            limit,
            count_only,
        } => {
            if count_only {
                count_probe(ctx, query.as_deref(), body.as_deref(), body_file).await
            } else {
                search(ctx, query.as_deref(), body.as_deref(), body_file, limit).await
            }
        }
        UsptoCommand::Grant { patent_number } => grant(ctx, &patent_number).await,
        UsptoCommand::Application { app_number } => application(ctx, &app_number).await,
        UsptoCommand::Continuity { app_number } => continuity(ctx, &app_number).await,
        UsptoCommand::Transactions { app_number } => {
            wrapper_bag(
                ctx,
                &app_number,
                "get_transactions",
                "eventDataBag",
                event_columns(),
            )
            .await
        }
        UsptoCommand::Assignments { app_number } => {
            wrapper_bag(
                ctx,
                &app_number,
                "get_assignments",
                "assignmentBag",
                assignment_columns(),
            )
            .await
        }
        UsptoCommand::ForeignPriority { app_number } => {
            wrapper_bag(
                ctx,
                &app_number,
                "get_foreign_priority",
                "foreignPriorityBag",
                foreign_priority_columns(),
            )
            .await
        }
        UsptoCommand::Adjustment { app_number } => {
            wrapper_bag(
                ctx,
                &app_number,
                "get_patent_term_adjustment",
                "patentTermAdjustmentData",
                adjustment_columns(),
            )
            .await
        }
        UsptoCommand::Attorney { app_number } => {
            wrapper_bag(ctx, &app_number, "get_attorney", "recordAttorney", &[]).await
        }
        UsptoCommand::Documents {
            app_number,
            code,
            direction,
        } => documents(ctx, &app_number, code.as_deref(), direction.as_deref()).await,
        UsptoCommand::DocumentText {
            app_number,
            document_id,
        } => document_text(ctx, &app_number, &document_id).await,
    }
}

/// The ODP field a CPC-class constraint travels in. USPTO ODP search only
/// indexes `inventionTitle` plus a handful of metadata fields — there is no
/// abstract/claims full-text — so a mis-guessed CPC class (H01M batteries for
/// a UV-C sterilization case, say) drops recall to zero. The zero-recall
/// fallback strips this constraint and retries.
const CPC_FIELD: &str = "applicationMetaData.cpcClassificationBag:";

async fn search(
    ctx: &Context,
    query: Option<&str>,
    body: Option<&str>,
    body_file: Option<PathBuf>,
    limit: u32,
) -> Result<()> {
    let request = build_search_request(query, body, body_file, limit)?;

    let Some(mut result) = tools::call_tool_data(ctx, "search_patents", &request).await? else {
        return Ok(());
    };

    // Zero-recall fallback. The backend query generator guesses a CPC class and
    // ANDs it into a title-only search; when that guess is wrong the search
    // returns nothing. Rather than silently handing back an empty set, drop the
    // CPC constraint and retry once so an over-narrow classification can never
    // blind the USPTO leg on its own.
    if count_results(&result) == 0 {
        if let Some(retried) = cpc_fallback(ctx, &request).await? {
            result = retried;
        }
    }

    // Whatever the query shape, an empty result set is never returned silently:
    // ODP has no abstract/claims full-text, so a feature that lives only in the
    // abstract cannot be matched — the recall pass has to key on the title.
    if count_results(&result) == 0 {
        eprintln!(
            "note: USPTO ODP search returned 0 results. ODP indexes the invention title and \
             metadata only (no abstract/claims full-text), so a distinguishing feature that lives \
             in the abstract cannot be matched here. Broaden to a title search on the core device \
             noun (e.g. --query 'applicationMetaData.inventionTitle:\"charging case\"') and triage \
             abstracts with 'flowleap ops abstract <number>'."
        );
    }

    print_uspto_collection(ctx, &result);
    Ok(())
}

/// `--count-only`: probe the ODP match count without pulling records
/// (limit 1). Reads `count` from the tool payload — the total ODP matched,
/// not the page size.
async fn count_probe(
    ctx: &Context,
    query: Option<&str>,
    body: Option<&str>,
    body_file: Option<PathBuf>,
) -> Result<()> {
    let request = build_search_request(query, body, body_file, 1)?;
    let Some(result) = tools::call_tool_data(ctx, "search_patents", &request).await? else {
        return Ok(());
    };
    let count = result.get("count").cloned().unwrap_or(Value::Null);
    if ctx.output_format == "json" {
        let mut payload = json!({ "count": count });
        if let Some(query) = request.get("query") {
            payload["query"] = query.clone();
        }
        output::print_json(&payload);
    } else {
        match &count {
            Value::Null => println!("Count: unknown (tool returned no `count` field)"),
            value => println!("Count: {}", value),
        }
    }
    Ok(())
}

/// Build the `search_patents` input for a USPTO search, from `--query` or
/// `--body`/`--body-file`. A `--query` string becomes `{query, limit, offset}`;
/// a full ODP request body is translated field-by-field onto the tool's
/// snake_case parameters, with `limit` supplying the pagination default when
/// the body does not already carry one.
fn build_search_request(
    query: Option<&str>,
    body: Option<&str>,
    body_file: Option<PathBuf>,
    limit: u32,
) -> Result<Value> {
    match (query, body, body_file) {
        (Some(query), None, None) => Ok(json!({
            "provider": "uspto",
            "query": query,
            "limit": limit,
            "offset": 0,
        })),
        (None, Some(body), None) => normalize_body(&read_body_arg(body)?, limit),
        (None, None, Some(path)) => {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("read body file {}", path.display()))?;
            normalize_body(&raw, limit)
        }
        (None, None, None) => bail!(
            "provide a query: --query \"<lucene>\", or a full ODP request body via --body / --body-file"
        ),
        _ => bail!("--query, --body and --body-file are mutually exclusive"),
    }
}

/// Read a `--body` argument, treating a lone `-` as "read the body from stdin"
/// so a pipeline can stream a prepared request body in.
fn read_body_arg(body: &str) -> Result<String> {
    if body == "-" {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .context("read request body from stdin")?;
        Ok(buffer)
    } else {
        Ok(body.to_string())
    }
}

/// ODP request-body fields the tool takes under a different name. Everything
/// else a caller writes is forwarded untouched — the tool's schema is the
/// single validator, so no client-side allowlist can silently drop a parameter
/// a newer backend understands (backend ADR 0013).
/// (`sort`, `fields`, `facets` and `enrich` are spelled the same on both sides
/// and need no entry.)
const ODP_FIELD_ALIASES: &[(&str, &str)] = &[("q", "query"), ("rangeFilters", "range_filters")];

/// Translate a full ODP request body into `search_patents` input: rename the
/// fields the tool spells differently, flatten `pagination` onto limit/offset,
/// and default the page size from `--limit` when the body sets none.
fn normalize_body(raw: &str, limit: u32) -> Result<Value> {
    let value: Value = serde_json::from_str(raw).context("request body must be valid JSON")?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("request body must be a JSON object"))?;
    if !object.contains_key("q") && !object.contains_key("query") {
        bail!("request body must contain a \"q\" field (an ODP Lucene query)");
    }

    let mut input = json!({ "provider": "uspto" });
    for (key, value) in object {
        // pagination is the one nested field: ODP nests it, the tool takes
        // limit/offset flat.
        if key == "pagination" {
            if let Some(page_limit) = value.get("limit") {
                input["limit"] = page_limit.clone();
            }
            if let Some(offset) = value.get("offset") {
                input["offset"] = offset.clone();
            }
            continue;
        }
        let name = ODP_FIELD_ALIASES
            .iter()
            .find(|(from, _)| *from == key)
            .map(|(_, to)| *to)
            .unwrap_or(key.as_str());
        input[name] = value.clone();
    }

    if input.get("limit").is_none() {
        input["limit"] = json!(limit);
        input["offset"] = json!(0);
    }
    Ok(input)
}

/// Retry a zero-recall search with the CPC-class constraint stripped. Returns
/// the retried result when a CPC clause was present and removable (even if the
/// retry itself is empty — the caller then falls through to the guidance note),
/// or None when there was no CPC constraint to strip.
async fn cpc_fallback(ctx: &Context, request: &Value) -> Result<Option<Value>> {
    let Some(q) = request.get("query").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(stripped) = strip_cpc_constraint(q) else {
        return Ok(None);
    };

    eprintln!(
        "note: the CPC-constrained USPTO query returned 0 results; retrying without the \
         CPC filter ({CPC_FIELD}…)."
    );

    let mut retry = request.clone();
    retry["query"] = Value::String(stripped);
    tools::call_tool_data(ctx, "search_patents", &retry).await
}

/// Remove the `cpcClassificationBag:` constraint from an ODP Lucene `q`,
/// together with the boolean operator that joins it, so a zero-recall query can
/// be retried without the (often mis-guessed) CPC filter. Splits on top-level
/// ` AND `, respecting parentheses and quotes. Returns None when there is no CPC
/// clause or removing it would leave nothing to search.
fn strip_cpc_constraint(q: &str) -> Option<String> {
    if !q.contains(CPC_FIELD) {
        return None;
    }
    let clauses = split_top_level_and(q);
    let kept: Vec<&str> = clauses
        .iter()
        .copied()
        .filter(|clause| !clause.contains(CPC_FIELD))
        .collect();
    if kept.len() == clauses.len() || kept.is_empty() {
        return None;
    }
    let rebuilt = kept.join(" AND ");
    (rebuilt != q).then_some(rebuilt)
}

/// Split a Lucene query on top-level ` AND ` separators (case-insensitive),
/// ignoring any ` AND ` that sits inside parentheses or a quoted phrase.
fn split_top_level_and(q: &str) -> Vec<&str> {
    let bytes = q.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth: i32 = 0;
    let mut in_quote = false;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => in_quote = !in_quote,
            b'(' if !in_quote => depth += 1,
            b')' if !in_quote => depth -= 1,
            _ => {}
        }
        if depth == 0 && !in_quote && is_and_separator(bytes, i) {
            parts.push(q[start..i].trim());
            i += 5; // len(" AND ")
            start = i;
            continue;
        }
        i += 1;
    }
    parts.push(q[start..].trim());
    parts
}

/// True when a case-insensitive ` AND ` separator begins at `i`.
fn is_and_separator(bytes: &[u8], i: usize) -> bool {
    let window = b" AND ";
    bytes.len() >= i + window.len()
        && bytes[i..i + window.len()]
            .iter()
            .zip(window)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// Count the records in an ODP search response across the shapes the backend
/// returns them under.
fn count_results(result: &Value) -> usize {
    for key in ["patentFileWrapperDataBag", "results", "docs", "data"] {
        if let Some(array) = result.get(key).and_then(Value::as_array) {
            return array.len();
        }
    }
    // Some shapes return the collection at the top level.
    result.as_array().map(|array| array.len()).unwrap_or(0)
}

async fn grant(ctx: &Context, patent_number: &str) -> Result<()> {
    let input = json!({ "patent_number": patent_number });
    if let Some(result) = tools::call_tool_data(ctx, "get_us_grant", &input).await? {
        print_uspto_record(ctx, &result);
    }
    Ok(())
}

async fn application(ctx: &Context, app_number: &str) -> Result<()> {
    let input = json!({ "application_number": app_number });
    if let Some(result) = tools::call_tool_data(ctx, "get_us_application", &input).await? {
        print_uspto_record(ctx, &result);
    }
    Ok(())
}

/// Render one ODP file-wrapper record. JSON output keeps the verbatim backend
/// payload; human/table output reads the single record out of
/// `patentFileWrapperDataBag` so the detail columns have a record to resolve
/// against instead of the envelope wrapping it.
fn print_uspto_record(ctx: &Context, result: &Value) {
    if ctx.output_format == "json" {
        output::print_json(result);
        return;
    }
    let record = result
        .get("patentFileWrapperDataBag")
        .and_then(|bag| bag.get(0))
        .unwrap_or(result);
    output::print_value(&ctx.output_format, record, detail_columns());
}

async fn continuity(ctx: &Context, app_number: &str) -> Result<()> {
    let input = json!({ "application_number": app_number });
    if let Some(result) = tools::call_tool_data(ctx, "get_continuity", &input).await? {
        output::print_value(&ctx.output_format, &result, continuity_columns());
    }
    Ok(())
}

/// Run a file-wrapper projection tool and print the named inner value from the
/// first file wrapper record (arrays get `columns`; objects fall back to JSON
/// via the formatter). JSON output always prints the tool payload untouched so
/// agents keep the verbatim backend shape.
async fn wrapper_bag(
    ctx: &Context,
    app_number: &str,
    tool: &str,
    inner_key: &str,
    columns: &[(&str, &str)],
) -> Result<()> {
    let input = json!({ "application_number": app_number });
    let Some(result) = tools::call_tool_data(ctx, tool, &input).await? else {
        return Ok(());
    };
    if ctx.output_format == "json" {
        output::print_json(&result);
        return Ok(());
    }
    let inner = result
        .get("patentFileWrapperDataBag")
        .and_then(|bag| bag.get(0))
        .and_then(|record| record.get(inner_key));
    match inner {
        Some(value) => output::print_value(&ctx.output_format, value, columns),
        None => output::print_value(&ctx.output_format, &result, columns),
    }
    Ok(())
}

/// List IFW documents. Filtering happens server-side — the tool takes the
/// document code and direction and returns the compacted listing
/// (`{ applicationNumber, total, returned, documents }`), each record keeping
/// its `downloadOptionBag` alongside the derived `pageCount`.
async fn documents(
    ctx: &Context,
    app_number: &str,
    code: Option<&str>,
    direction: Option<&str>,
) -> Result<()> {
    let mut input = json!({ "application_number": app_number });
    if let Some(code) = code {
        input["document_code"] = json!(code.to_uppercase());
    }
    if let Some(direction) = direction {
        input["direction"] = json!(direction.to_uppercase());
    }

    let Some(result) = tools::call_tool_data(ctx, "get_application_documents", &input).await?
    else {
        return Ok(());
    };
    if ctx.output_format == "json" {
        output::print_json(&result);
        return Ok(());
    }

    let total = result.get("total").and_then(Value::as_u64).unwrap_or(0);
    let documents = result
        .get("documents")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let returned = documents.as_array().map(Vec::len).unwrap_or(0);
    if returned == 0 && total > 0 {
        eprintln!(
            "note: {total} document(s) exist but none match the filter. \
             Drop --code/--direction to list them all."
        );
    }
    output::print_value(&ctx.output_format, &documents, document_columns());
    Ok(())
}

async fn document_text(ctx: &Context, app_number: &str, document_id: &str) -> Result<()> {
    let input = json!({
        "application_number": app_number,
        "document_id": document_id,
    });
    let Some(result) = tools::call_tool_data(ctx, "read_application_document", &input).await?
    else {
        return Ok(());
    };
    if ctx.output_format == "json" {
        output::print_json(&result);
        return Ok(());
    }
    // Human/table: the markdown itself is the payload — keep stdout clean text
    // (pipeable into a file or pager) and put the metadata line on stderr.
    if let (Some(pages), Some(model)) = (
        result.get("pageCount").and_then(Value::as_u64),
        result.get("model").and_then(Value::as_str),
    ) {
        let cached = result
            .get("cached")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        eprintln!(
            "{document_id}: {pages} page(s), OCR model {model}{}",
            if cached { " (cached)" } else { "" }
        );
    }
    match result.get("markdown").and_then(Value::as_str) {
        Some(markdown) => println!("{markdown}"),
        None => output::print_json(&result),
    }
    Ok(())
}

fn event_columns() -> &'static [(&'static str, &'static str)] {
    &[
        ("eventDate", "Date"),
        ("eventCode", "Code"),
        ("eventDescriptionText", "Event"),
    ]
}

fn assignment_columns() -> &'static [(&'static str, &'static str)] {
    &[
        ("assignmentRecordedDate", "Recorded"),
        ("reelAndFrameNumber", "Reel/Frame"),
        ("conveyanceText", "Conveyance"),
    ]
}

fn foreign_priority_columns() -> &'static [(&'static str, &'static str)] {
    &[
        ("filingDate", "Filed"),
        ("applicationNumberText", "Application #"),
        ("ipOfficeName", "IP Office"),
    ]
}

fn adjustment_columns() -> &'static [(&'static str, &'static str)] {
    &[
        ("adjustmentTotalQuantity", "Total PTA (days)"),
        ("aDelayQuantity", "A delay"),
        ("bDelayQuantity", "B delay"),
        ("cDelayQuantity", "C delay"),
        ("applicantDayDelayQuantity", "Applicant delay"),
        ("overlappingDayQuantity", "Overlap"),
    ]
}

fn document_columns() -> &'static [(&'static str, &'static str)] {
    &[
        ("documentIdentifier", "Document ID"),
        ("officialDate", "Date"),
        ("documentCode", "Code"),
        ("description", "Description"),
        ("direction", "Direction"),
        ("pageCount", "Pages"),
    ]
}

fn print_uspto_collection(ctx: &Context, result: &serde_json::Value) {
    // JSON keeps the tool's `data` payload verbatim (`count`,
    // `patentFileWrapperDataBag`, …); table/human unwrap the record bag.
    if ctx.output_format == "json" {
        output::print_json(result);
        return;
    }
    let columns = search_columns();
    if let Some(results) = result.get("patentFileWrapperDataBag") {
        output::print_value(&ctx.output_format, results, columns);
    } else if let Some(results) = result.get("results") {
        output::print_value(&ctx.output_format, results, columns);
    } else if let Some(docs) = result.get("docs") {
        output::print_value(&ctx.output_format, docs, columns);
    } else if let Some(data) = result.get("data") {
        output::print_value(&ctx.output_format, data, columns);
    } else {
        output::print_value(&ctx.output_format, result, columns);
    }
}

/// USPTO ODP records carry their identity at the top level
/// (`applicationNumberText`) and everything else under `applicationMetaData`.
/// The columns spell those paths out; the formatter falls back to the last
/// segment as a flat key, so a flattened payload still renders.
fn search_columns() -> &'static [(&'static str, &'static str)] {
    &[
        ("applicationNumberText", "Application #"),
        ("applicationMetaData.patentNumber", "Patent #"),
        (
            "applicationMetaData.earliestPublicationNumber",
            "Publication #",
        ),
        ("applicationMetaData.inventionTitle", "Title"),
        ("applicationMetaData.firstApplicantName", "Applicant"),
        ("applicationMetaData.filingDate", "Filed"),
        (
            "applicationMetaData.applicationStatusDescriptionText",
            "Status",
        ),
    ]
}

fn detail_columns() -> &'static [(&'static str, &'static str)] {
    &[
        ("applicationNumberText", "Application #"),
        ("applicationMetaData.patentNumber", "Patent #"),
        ("applicationMetaData.inventionTitle", "Title"),
        ("applicationMetaData.firstApplicantName", "Applicant"),
        ("applicationMetaData.firstInventorName", "Inventor"),
        ("applicationMetaData.filingDate", "Filed"),
        ("applicationMetaData.grantDate", "Granted"),
        (
            "applicationMetaData.earliestPublicationDate",
            "First published",
        ),
        (
            "applicationMetaData.applicationStatusDescriptionText",
            "Status",
        ),
    ]
}

fn continuity_columns() -> &'static [(&'static str, &'static str)] {
    &[
        ("applicationNumber", "Application #"),
        ("parentApplicationNumber", "Parent Application #"),
        ("childApplicationNumber", "Child Application #"),
        ("continuityType", "Type"),
        ("filingDate", "Filed"),
        ("status", "Status"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression for issue #152: the evaluator's Phase-2 dead-end. For the UV-C
    /// earbud sterilizing case, the (since retired) server query builder
    /// emitted this exact body — a CPC guess of H01M (batteries) ANDed onto
    /// title-only ODP terms — and `uspto search` returned 0, dead-ending the
    /// USPTO leg. Any full body must be (a) accepted directly via --body, and
    /// (b) recoverable: the zero-recall fallback strips the wrong CPC class so
    /// the search is retried on the recall terms instead of silently empty.
    #[test]
    fn issue_152_uvc_earbud_full_body_is_accepted_and_recoverable() {
        let recommended_query = r#"{
            "q": "applicationMetaData.cpcClassificationBag:H01M* AND (\"UV-C\" OR \"ultraviolet\" OR \"steriliz*\" OR \"disinfect*\") AND \"earbud\"",
            "fields": ["applicationMetaData.inventionTitle"],
            "pagination": { "limit": 25, "offset": 0 }
        }"#;

        // (a) the full body submits directly through --body.
        let request = build_search_request(None, Some(recommended_query), None, 10).unwrap();
        let q = request["query"].as_str().unwrap();
        assert_eq!(request["limit"], 25); // body pagination preserved

        // (b) the H01M guess is stripped so the search can be retried.
        let recovered = strip_cpc_constraint(q).expect("CPC clause must be strippable");
        assert!(!recovered.contains(CPC_FIELD));
        assert!(recovered.contains("earbud"));
    }

    #[test]
    fn strip_cpc_drops_the_class_clause_and_one_operator() {
        // The evaluator's exact Phase-2 dead-end: a guessed H01M (batteries)
        // class for a UV-C sterilization case, so the CPC-constrained
        // query returned 0. Stripping the CPC clause leaves the recall terms.
        let cases = [
            (
                "applicationMetaData.cpcClassificationBag:H01M* AND (\"UV-C\" OR \"steriliz*\") AND \"earbud\"",
                Some("(\"UV-C\" OR \"steriliz*\") AND \"earbud\""),
            ),
            // Leading CPC clause.
            (
                "applicationMetaData.cpcClassificationBag:H04* AND (\"UV-C\" OR \"ultraviolet\")",
                Some("(\"UV-C\" OR \"ultraviolet\")"),
            ),
            // Trailing CPC clause.
            (
                "(\"earbud\") AND applicationMetaData.cpcClassificationBag:A61L*",
                Some("(\"earbud\")"),
            ),
            // No CPC clause — nothing to strip.
            ("applicationMetaData.inventionTitle:\"charging case\"", None),
            // CPC is the only clause — stripping would leave nothing.
            ("applicationMetaData.cpcClassificationBag:H01M*", None),
        ];
        for (input, expected) in cases {
            assert_eq!(
                strip_cpc_constraint(input).as_deref(),
                expected,
                "input: {input}"
            );
        }
    }

    #[test]
    fn split_top_level_and_ignores_nested_and() {
        // A parenthesized " AND " must not be treated as a top-level separator.
        let q = "applicationMetaData.cpcClassificationBag:H04* AND (\"a\" AND \"b\") AND \"c\"";
        assert_eq!(
            split_top_level_and(q),
            vec![
                "applicationMetaData.cpcClassificationBag:H04*",
                "(\"a\" AND \"b\")",
                "\"c\"",
            ]
        );
    }

    #[test]
    fn build_request_wraps_query_and_translates_odp_body() {
        // --query becomes the tool's flat {query, limit, offset}.
        let wrapped = build_search_request(Some("ti:battery"), None, None, 7).unwrap();
        assert_eq!(wrapped["provider"], "uspto");
        assert_eq!(wrapped["query"], "ti:battery");
        assert_eq!(wrapped["limit"], 7);
        assert_eq!(wrapped["offset"], 0);

        // --body: `q` becomes `query`; the page size defaults to --limit.
        let body = build_search_request(None, Some(r#"{"q":"ti:x"}"#), None, 25).unwrap();
        assert_eq!(body["query"], "ti:x");
        assert_eq!(body["limit"], 25);

        // A body that already paginates keeps its own window, flattened.
        let paged = build_search_request(
            None,
            Some(r#"{"q":"ti:x","pagination":{"limit":3,"offset":9}}"#),
            None,
            25,
        )
        .unwrap();
        assert_eq!(paged["limit"], 3);
        assert_eq!(paged["offset"], 9);
    }

    /// No client-side allowlist: every field a caller writes reaches the tool,
    /// under the tool's own name where ODP spells it differently. An older CLI
    /// must never be what stops a newer backend parameter from arriving.
    #[test]
    fn odp_body_fields_are_forwarded_not_filtered() {
        let request = build_search_request(
            None,
            Some(
                r#"{"q":"ti:x","fields":["a"],"enrich":["abstract"],
                    "rangeFilters":"filingDate 2020-01-01->2024-12-31",
                    "sort":"applicationMetaData.filingDate desc",
                    "facets":"applicationMetaData.applicationTypeLabelName",
                    "someFutureField":"typeCode UTL"}"#,
            ),
            None,
            10,
        )
        .unwrap();

        assert_eq!(request["fields"], json!(["a"]));
        assert_eq!(request["enrich"], json!(["abstract"]));
        assert_eq!(
            request["range_filters"],
            "filingDate 2020-01-01->2024-12-31"
        );
        assert_eq!(request["sort"], "applicationMetaData.filingDate desc");
        assert_eq!(
            request["facets"],
            "applicationMetaData.applicationTypeLabelName"
        );
        // A field this CLI has never heard of still travels — the tool schema
        // is the only validator.
        assert_eq!(request["someFutureField"], "typeCode UTL");
    }

    #[test]
    fn build_request_rejects_unusable_input() {
        // A body without a query field is rejected.
        assert!(build_search_request(None, Some(r#"{"fields":[]}"#), None, 10).is_err());
        // Nothing provided is a usage error.
        assert!(build_search_request(None, None, None, 10).is_err());
    }

    #[test]
    fn count_results_reads_every_collection_shape() {
        let bag = json!({ "patentFileWrapperDataBag": [1, 2, 3] });
        assert_eq!(count_results(&bag), 3);
        assert_eq!(count_results(&json!({ "results": [] })), 0);
        assert_eq!(count_results(&json!([1, 2])), 2);
        assert_eq!(count_results(&json!({ "other": true })), 0);
    }
}
