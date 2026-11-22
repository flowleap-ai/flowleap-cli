use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use crate::client::Context;
use crate::commands::{patent, tools};
use crate::output;

#[derive(Parser)]
pub struct OpsArgs {
    #[command(subcommand)]
    command: OpsCommand,
}

#[derive(Subcommand)]
enum OpsCommand {
    /// Search patents using CQL query
    Search {
        /// CQL query string
        #[arg(long)]
        cql: String,

        /// Start position
        #[arg(long, default_value = "1")]
        start: u32,

        /// End position
        #[arg(long, default_value = "25")]
        end: u32,
    },
    /// Get bibliographic data for a patent
    Biblio {
        /// Patent document number (e.g., EP1234567)
        doc: String,

        /// Also return the EP designated contracting states and extension
        /// states (one extra EPO read — they are not in the biblio document)
        #[arg(long)]
        designated_states: bool,

        /// Also look up each cited non-patent reference (citedReferences[].npl)
        /// in OpenAlex and attach its record as `nplWork` (one OpenAlex read
        /// per reference)
        #[arg(long)]
        resolve_npl: bool,
    },
    /// Get claims text for a patent
    Claims {
        /// Patent document number
        doc: String,
        /// Language code (e.g., en, de, fr)
        #[arg(long, default_value = "en")]
        lang: String,
    },
    /// Get full description text for a patent
    Description {
        /// Patent document number
        doc: String,
        /// Language code (e.g., en, de, fr)
        #[arg(long, default_value = "en")]
        lang: String,
    },
    /// Get patent family members (INPADOC extended family)
    ///
    /// The default answer is the summary view: one page of the member list
    /// (`representatives.members`), the members per office (`countries`) and
    /// the counts. `--view full` adds every member and the `members` row list.
    Family {
        /// Patent document number
        doc: String,

        /// summary (the backend default): one page of members and the counts.
        /// full: every member and the `members` row list
        #[arg(long, value_parser = ["summary", "full"])]
        view: Option<String>,

        /// Only the members of one office (two-letter code, e.g. US, EP, JP)
        #[arg(long)]
        country: Option<String>,

        /// Members per answer in the summary view (backend default 100, max 1000)
        #[arg(long)]
        limit: Option<u32>,

        /// Members to skip in the summary view, for the next page
        #[arg(long)]
        offset: Option<u32>,
    },
    /// Get legal status events
    ///
    /// One page of events. Each event has `eventDate` (the date of the event)
    /// and `date` (the date the office recorded it). `--output table` prints
    /// the events as a table.
    Legal {
        /// Patent document number
        doc: String,

        /// Events per answer (backend default 50, max 1000)
        #[arg(long)]
        limit: Option<u32>,

        /// Events to skip, for the next page
        #[arg(long)]
        offset: Option<u32>,
    },
    /// Get abstract text
    Abstract {
        /// Patent document number
        doc: String,
    },
    /// EP Register bibliography: representatives of record, opponents, status
    RegisterBiblio {
        /// EP publication number (e.g., EP1000000)
        doc: String,
    },
    /// EP Register search by register CQL: re=<representative>, op=<opponent>, nm=<name in any role> (EP only)
    ///
    /// The one search with a representative index. `re=` is the professional
    /// representative of record, `op=` an opponent, `ia=` inventor or
    /// applicant; `pa`, `in`, `ti`, `pn`, `ap`, `pr`, `pd`, `ic` work too.
    /// `nm=` is a name in any role (applicant, inventor, representative,
    /// opponent): use it when a role index (re/op/ia/pa/in) returns 0 to find
    /// which role the person holds. The register stores names surname-first,
    /// so use the surname alone or nm="Surname, First".
    /// `total` counts the whole query (capped at 10000); one row per EP
    /// application.
    RegisterSearch {
        /// Register CQL query (e.g., re=Kreuels; quote a multi-word value)
        #[arg(long)]
        cql: String,

        /// Start position
        #[arg(long, default_value = "1")]
        start: u32,

        /// End position (max 100 per page)
        #[arg(long, default_value = "25")]
        end: u32,
    },
}

pub async fn run(ctx: &Context, args: OpsArgs) -> Result<()> {
    ctx.require_auth()?;

    match args.command {
        OpsCommand::Search { cql, start, end } => search(ctx, &cql, start, end).await,
        // The designated states live in the INPADOC legal record, not in the
        // bibliography document, so the backend charges an extra OPS read for
        // them — opt-in, never on by default.
        // Cited-NPL resolution is likewise opt-in: one OpenAlex read per
        // reference.
        OpsCommand::Biblio {
            doc,
            designated_states,
            resolve_npl,
        } => {
            let extras = [
                designated_states.then_some(("include_designated_states", json!(true))),
                resolve_npl.then_some(("resolve_npl", json!(true))),
            ];
            let extras: Vec<_> = extras.into_iter().flatten().collect();
            document_with(ctx, "get_bibliography", &doc, None, &extras).await
        }
        OpsCommand::Claims { doc, lang } => document(ctx, "get_claims", &doc, Some(&lang)).await,
        OpsCommand::Description { doc, lang } => {
            document(ctx, "get_description", &doc, Some(&lang)).await
        }
        // The INPADOC family — every application and publication linked through
        // common priorities. get_patent_family is the narrower simple-family
        // equivalents tool and deliberately keeps that meaning.
        OpsCommand::Family {
            doc,
            view,
            country,
            limit,
            offset,
        } => {
            let input = family_input(&doc, view.as_deref(), country.as_deref(), limit, offset);
            family(ctx, &input).await
        }
        OpsCommand::Legal { doc, limit, offset } => {
            legal(ctx, &legal_input(&doc, limit, offset)).await
        }
        OpsCommand::Abstract { doc } => document(ctx, "get_abstract", &doc, None).await,
        OpsCommand::RegisterBiblio { doc } => {
            document(ctx, "get_register_biblio", &doc, None).await
        }
        OpsCommand::RegisterSearch { cql, start, end } => {
            register_search(ctx, &cql, start, end).await
        }
    }
}

async fn search(ctx: &Context, cql: &str, start: u32, end: u32) -> Result<()> {
    let input = patent::epo_search_input(cql, format!("{}-{}", start, end), None);
    if let Some(result) = tools::call_tool_data(ctx, "search_patents", &input).await? {
        patent::print_search_result(ctx, &result);
    }
    Ok(())
}

/// Column set for one EP Register search row: one row per EP application,
/// the shape `search_register` returns. `docId` is the earliest EP
/// publication and is null while none is published, so the application
/// number is the stable identifier and leads.
const REGISTER_COLUMNS: &[(&str, &str)] = &[
    ("applicationNumber", "Application"),
    ("docId", "Publication"),
    ("title", "Title"),
    ("applicants", "Applicants"),
    ("representativeNames", "Representatives"),
    ("status", "Status"),
];

/// The `search_register` input over `start`-`end`: register CQL plus the
/// same `"start-end"` range form `ops search` sends.
fn register_search_input(cql: &str, start: u32, end: u32) -> Value {
    json!({ "query": cql, "range": format!("{}-{}", start, end) })
}

/// Flatten one register row for the table and human printers: the
/// representatives arrive as objects (`name`, `country`, `address`), and the
/// column printers render strings and string lists only, so their names are
/// joined into `representativeNames`. JSON output never passes through here.
fn register_row(doc: &Value) -> Value {
    let mut row = doc.clone();
    let names: Vec<Value> = doc
        .get("representatives")
        .and_then(Value::as_array)
        .map(|reps| {
            reps.iter()
                .filter_map(|rep| rep.get("name").and_then(Value::as_str))
                .map(|name| json!(name))
                .collect()
        })
        .unwrap_or_default();
    row["representativeNames"] = Value::Array(names);
    row
}

/// The `Hint: <text>` line for the table and human printers, from the
/// optional top-level `hint` string the backend adds on a zero-result
/// role-index query (for example "No results for re=Boniberger. The name may
/// appear in another role; retry with nm=Boniberger."). None when the field
/// is absent or not a string, so a result without a hint prints as before.
fn register_hint_line(result: &Value) -> Option<String> {
    result
        .get("hint")
        .and_then(Value::as_str)
        .map(|hint| format!("Hint: {hint}"))
}

/// Print an EP Register search result. JSON keeps the tool's `data` payload
/// verbatim (`total`, `returned`, `range`, `docs`, `hint`). Table and human
/// output render the `docs` rows, then the whole-query `total` on its own
/// line: OPS counts the whole query (capped at 10000) while the page is only
/// a sample, and the count is the usual question ("how many filings does X
/// handle"). The backend `hint`, when present, follows the total on its own
/// line.
fn print_register_result(ctx: &Context, result: &Value) {
    if ctx.output_format == "json" {
        output::print_json(result);
        return;
    }
    match result.get("docs").and_then(Value::as_array) {
        Some(docs) => {
            let rows: Vec<Value> = docs.iter().map(register_row).collect();
            output::print_value(&ctx.output_format, &Value::Array(rows), REGISTER_COLUMNS);
        }
        None => output::print_value(&ctx.output_format, result, REGISTER_COLUMNS),
    }
    if let Some(total) = result.get("total").and_then(Value::as_u64) {
        println!(
            "Total: {total} EP application(s) for the whole query (OPS caps the count at 10000)"
        );
    }
    if let Some(line) = register_hint_line(result) {
        println!("{line}");
    }
}

async fn register_search(ctx: &Context, cql: &str, start: u32, end: u32) -> Result<()> {
    let input = register_search_input(cql, start, end);
    if let Some(result) = tools::call_tool_data(ctx, "search_register", &input).await? {
        print_register_result(ctx, &result);
    }
    Ok(())
}

/// The `get_family` input. Only the flags the caller gave are sent, so the
/// backend defaults (summary view, 100 members) stay the backend's.
fn family_input(
    doc: &str,
    view: Option<&str>,
    country: Option<&str>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Value {
    let mut input = json!({ "patent_number": doc });
    if let Some(view) = view {
        input["view"] = json!(view);
    }
    if let Some(country) = country {
        input["country"] = json!(country.to_uppercase());
    }
    if let Some(limit) = limit {
        input["limit"] = json!(limit);
    }
    if let Some(offset) = offset {
        input["offset"] = json!(offset);
    }
    input
}

/// The `get_legal_status` input: the document, plus the page when given.
fn legal_input(doc: &str, limit: Option<u32>, offset: Option<u32>) -> Value {
    let mut input = json!({ "patent_number": doc });
    if let Some(limit) = limit {
        input["limit"] = json!(limit);
    }
    if let Some(offset) = offset {
        input["offset"] = json!(offset);
    }
    input
}

/// The stderr note for a family answer that holds only a part of the family:
/// the backend `next` string says how to get the remaining members. None when
/// the answer is complete (no `next`).
fn family_next_note(result: &Value) -> Option<String> {
    result
        .get("next")
        .and_then(Value::as_str)
        .filter(|next| !next.is_empty())
        .map(|next| format!("note: {next}"))
}

/// Print the family answer verbatim. Outside `--json` the backend `next`
/// string goes to stderr, so a partial member list is never read as the whole
/// family and stdout stays the payload.
async fn family(ctx: &Context, input: &Value) -> Result<()> {
    let Some(result) = tools::call_tool_data(ctx, "get_family", input).await? else {
        return Ok(());
    };
    output::print_json(&result);
    if ctx.output_format != "json" {
        if let Some(note) = family_next_note(&result) {
            eprintln!("{note}");
        }
    }
    Ok(())
}

/// Columns of one legal status event for `--output table`. `eventDate` is the
/// date of the event; `date` is the date the office recorded it, which can be
/// years later.
const LEGAL_EVENT_COLUMNS: &[(&str, &str)] = &[
    ("eventDate", "Event date"),
    ("date", "Record date"),
    ("country", "Office"),
    ("code", "Code"),
    ("text", "Event"),
];

/// The stderr note for a legal status answer that holds only a part of the
/// events. None when the counts are absent (older backend) or the answer
/// holds every event.
fn legal_page_note(result: &Value) -> Option<String> {
    let total = result.get("eventsTotal").and_then(Value::as_u64)?;
    let returned = result.get("eventsReturned").and_then(Value::as_u64)?;
    if returned >= total {
        return None;
    }
    let offset = result.get("offset").and_then(Value::as_u64).unwrap_or(0);
    let next = offset + returned;
    Some(if next < total {
        format!(
            "note: this answer holds {returned} of {total} legal status events (offset {offset}). \
             Next page: --offset {next}. All events in one answer: --limit {total} (max 1000)."
        )
    } else {
        format!(
            "note: this answer holds {returned} of {total} legal status events (offset {offset}). \
             This is the last page."
        )
    })
}

/// Print the legal status answer. JSON and human output keep the payload
/// verbatim (`designatedStates`, `events`, the counts); `--output table`
/// prints the events as rows. Outside `--json` a partial event list is named
/// on stderr.
async fn legal(ctx: &Context, input: &Value) -> Result<()> {
    let Some(result) = tools::call_tool_data(ctx, "get_legal_status", input).await? else {
        return Ok(());
    };
    match result.get("events").filter(|events| events.is_array()) {
        Some(events) if ctx.output_format == "table" => {
            output::print_value("table", events, LEGAL_EVENT_COLUMNS);
        }
        _ => output::print_json(&result),
    }
    if ctx.output_format != "json" {
        if let Some(note) = legal_page_note(&result) {
            eprintln!("{note}");
        }
    }
    Ok(())
}

/// Read one document projection through the facade. Every ops read is a
/// single-document tool taking `patent_number`, optionally with a language.
async fn document(ctx: &Context, tool: &str, doc: &str, lang: Option<&str>) -> Result<()> {
    document_with(ctx, tool, doc, lang, &[]).await
}

/// `document`, plus optional extra input fields. Every ops read goes through
/// here so the tool call is built in one place; only `biblio` passes extras,
/// for the opt-in `include_designated_states` join and `resolve_npl` lookup.
async fn document_with(
    ctx: &Context,
    tool: &str,
    doc: &str,
    lang: Option<&str>,
    extras: &[(&str, Value)],
) -> Result<()> {
    let mut input = json!({ "patent_number": doc });
    if let Some(lang) = lang {
        input["language"] = json!(lang);
    }
    for (key, value) in extras {
        input[*key] = value.clone();
    }
    if let Some(data) = tools::call_tool_data(ctx, tool, &input).await? {
        output::print_json(&data);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        family_input, family_next_note, legal_input, legal_page_note, register_hint_line,
        register_row, register_search_input,
    };
    use serde_json::json;

    #[test]
    fn register_search_input_sends_register_cql_and_the_range_form_ops_search_uses() {
        assert_eq!(
            register_search_input("re=Kreuels", 1, 3),
            json!({ "query": "re=Kreuels", "range": "1-3" })
        );
    }

    #[test]
    fn a_register_row_joins_representative_names_and_keeps_a_null_publication() {
        let doc = json!({
            "applicationNumber": "EP25183540",
            "docId": null,
            "representatives": [
                { "name": "Kreuels, Justus", "country": "DE", "address": ["Düsseldorf"] },
                { "name": "Pannen, Kai", "country": "DE" }
            ],
            "status": "Examination is in progress"
        });
        let row = register_row(&doc);
        assert_eq!(
            (row["representativeNames"].clone(), row["docId"].clone()),
            (json!(["Kreuels, Justus", "Pannen, Kai"]), json!(null))
        );
    }

    #[test]
    fn a_register_row_without_representatives_renders_an_empty_list() {
        let row = register_row(&json!({ "applicationNumber": "EP1" }));
        assert_eq!(row["representativeNames"], json!([]));
    }

    #[test]
    fn the_backend_hint_prints_as_its_own_hint_line() {
        let result = json!({
            "total": 0,
            "docs": [],
            "hint": "No results for re=Boniberger. The name may appear in another role; retry with nm=Boniberger."
        });
        assert_eq!(
            register_hint_line(&result).as_deref(),
            Some("Hint: No results for re=Boniberger. The name may appear in another role; retry with nm=Boniberger.")
        );
    }

    #[test]
    fn a_result_without_a_hint_prints_no_hint_line() {
        let result = json!({ "total": 3, "docs": [], "hint": null });
        assert_eq!(register_hint_line(&result), None);
    }

    #[test]
    fn family_input_sends_only_the_flags_the_caller_gave() {
        assert_eq!(
            family_input("EP1000000", None, None, None, None),
            json!({ "patent_number": "EP1000000" })
        );
        assert_eq!(
            family_input("EP1000000", Some("full"), Some("us"), Some(20), Some(40)),
            json!({
                "patent_number": "EP1000000",
                "view": "full",
                "country": "US",
                "limit": 20,
                "offset": 40
            })
        );
    }

    #[test]
    fn a_family_answer_without_next_has_no_note() {
        assert_eq!(
            family_next_note(&json!({ "representativesTotal": 5 })),
            None
        );
        assert_eq!(
            family_next_note(&json!({ "next": "Call again with offset=2." })).as_deref(),
            Some("note: Call again with offset=2.")
        );
    }

    #[test]
    fn legal_input_sends_the_page_only_when_given() {
        assert_eq!(
            legal_input("EP1000000B1", None, None),
            json!({ "patent_number": "EP1000000B1" })
        );
        assert_eq!(
            legal_input("EP1000000B1", Some(10), Some(50)),
            json!({ "patent_number": "EP1000000B1", "limit": 10, "offset": 50 })
        );
    }

    #[test]
    fn a_partial_legal_answer_names_the_next_page_and_a_full_one_is_silent() {
        let partial = json!({ "eventsTotal": 120, "eventsReturned": 50, "offset": 0 });
        let note = legal_page_note(&partial).expect("a partial answer has a note");
        assert!(note.contains("50 of 120"), "{note}");
        assert!(note.contains("--offset 50"), "{note}");

        let last = json!({ "eventsTotal": 120, "eventsReturned": 20, "offset": 100 });
        let note = legal_page_note(&last).expect("the last page has a note");
        assert!(note.contains("last page"), "{note}");

        let whole = json!({ "eventsTotal": 12, "eventsReturned": 12, "offset": 0 });
        assert_eq!(legal_page_note(&whole), None);
        // An older backend sends no counts: no note.
        assert_eq!(legal_page_note(&json!({ "events": [] })), None);
    }
}
