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
    Family {
        /// Patent document number
        doc: String,
    },
    /// Get legal status events
    Legal {
        /// Patent document number
        doc: String,
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
    /// EP Register search by register CQL: re=<representative>, op=<opponent> (EP only)
    ///
    /// The one search with a representative index. `re=` is the professional
    /// representative of record, `op=` an opponent, `ia=` inventor or
    /// applicant; `pa`, `in`, `ti`, `pn`, `ap`, `pr`, `pd`, `ic` work too.
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
        OpsCommand::Family { doc } => document(ctx, "get_family", &doc, None).await,
        OpsCommand::Legal { doc } => document(ctx, "get_legal_status", &doc, None).await,
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

/// Print an EP Register search result. JSON keeps the tool's `data` payload
/// verbatim (`total`, `returned`, `range`, `docs`). Table and human output
/// render the `docs` rows, then the whole-query `total` on its own line: OPS
/// counts the whole query (capped at 10000) while the page is only a sample,
/// and the count is the usual question ("how many filings does X handle").
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
}

async fn register_search(ctx: &Context, cql: &str, start: u32, end: u32) -> Result<()> {
    let input = register_search_input(cql, start, end);
    if let Some(result) = tools::call_tool_data(ctx, "search_register", &input).await? {
        print_register_result(ctx, &result);
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
    use super::{register_row, register_search_input};
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
}
