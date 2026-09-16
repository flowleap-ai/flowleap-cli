use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use crate::client::Context;
use crate::commands::tools;
use crate::output;

#[derive(Parser)]
pub struct PatentArgs {
    #[command(subcommand)]
    command: PatentCommand,
}

#[derive(Subcommand)]
enum PatentCommand {
    /// Search patents via EPO OPS (worldwide coverage)
    Search {
        /// EPO CQL query (e.g. 'ti="battery separator" and pa=lg'). Write the
        /// CQL yourself — the flowleap-patent skill carries the method.
        #[arg(long, short)]
        query: String,

        /// Maximum results to return (1-100)
        #[arg(long, default_value = "10")]
        limit: u32,

        /// Country filter, comma-separated (e.g. "EP,WO"); "all" disables
        #[arg(long)]
        countries: Option<String>,

        /// Only report the result total (cheap count probe: range 1-1,
        /// no per-document bibliography fan-out)
        #[arg(long)]
        count_only: bool,
    },
}

pub async fn run(ctx: &Context, args: PatentArgs) -> Result<()> {
    ctx.require_auth()?;

    match args.command {
        PatentCommand::Search {
            query,
            limit,
            countries,
            count_only,
        } => {
            if count_only {
                count_probe(ctx, &query, countries.as_deref()).await
            } else {
                search(ctx, &query, limit, countries.as_deref()).await
            }
        }
    }
}

/// Column set for a hydrated EPO result list — the shape `search_patents`
/// returns for provider=epo_ops.
pub(crate) const SEARCH_COLUMNS: &[(&str, &str)] = &[
    ("docId", "Patent ID"),
    ("title", "Title"),
    ("applicants", "Applicants"),
    ("publicationDate", "Date"),
];

/// Build the `search_patents` input for an EPO CQL search over `start`-`end`.
/// `countries` is the CLI's comma-separated filter; "all" means unfiltered, and
/// the tool takes a list of two-letter codes.
pub(crate) fn epo_search_input(query: &str, range: String, countries: Option<&str>) -> Value {
    let mut input = json!({
        "query": query,
        "provider": "epo_ops",
        "range": range,
    });
    let codes: Vec<String> = countries
        .filter(|value| !value.eq_ignore_ascii_case("all"))
        .map(|value| {
            value
                .split(',')
                .map(|code| code.trim().to_uppercase())
                .filter(|code| !code.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if !codes.is_empty() {
        input["countries"] = json!(codes);
    }
    input
}

/// Print an EPO search result. JSON keeps the tool's `data` payload verbatim
/// (`total` and `docs` included) so agents see the documented envelope;
/// table/human render the hydrated `docs` list when present, else whatever
/// the tool returned, so an unexpected shape is never swallowed.
/// The one-line warning for a page the backend could not fully hydrate.
///
/// `search_patents` returns `detailsUnavailable` — the count of results whose
/// bibliography could not be read — and OMITS it when the page is whole. Those
/// results still carry their `docId`; only the detail fields came back null.
///
/// The table prints `docs` alone, so without this line a half-read page looks
/// exactly like a page of patents that have no title and no applicant. That is
/// the wrong conclusion to hand an analyst, so say which it is. `None` when the
/// page is whole, so a complete search stays as quiet as it was before.
fn partial_page_note(result: &Value) -> Option<String> {
    let unavailable = result.get("detailsUnavailable")?.as_u64()?;
    if unavailable == 0 {
        return None;
    }
    Some(format!(
        "Note: {unavailable} result(s) could not be read — their title, applicants and date are \
         blank because the detail fetch failed, not because the patent has none. Re-read those by \
         number with `flowleap ops biblio <docId>`."
    ))
}

pub(crate) fn print_search_result(ctx: &Context, result: &Value) {
    if ctx.output_format == "json" {
        output::print_json(result);
        return;
    }
    match result.get("docs") {
        Some(docs) => output::print_value(&ctx.output_format, docs, SEARCH_COLUMNS),
        None => output::print_value(&ctx.output_format, result, SEARCH_COLUMNS),
    }
    // After the rows, so it is the last thing read and cannot be mistaken for a
    // column header. JSON output already carries the field verbatim.
    if let Some(note) = partial_page_note(result) {
        eprintln!("{note}");
    }
}

async fn search(ctx: &Context, query: &str, limit: u32, countries: Option<&str>) -> Result<()> {
    let input = epo_search_input(query, format!("1-{}", limit.clamp(1, 100)), countries);
    if let Some(result) = tools::call_tool_data(ctx, "search_patents", &input).await? {
        print_search_result(ctx, &result);
    }
    Ok(())
}

/// `--count-only`: probe the total for a CQL query without hydrating any
/// documents (range 1-1, `details: false`). The backend computes `total`
/// before the country post-filter, so it always describes the full CQL
/// result set — the probe reports it as such.
async fn count_probe(ctx: &Context, query: &str, countries: Option<&str>) -> Result<()> {
    let mut input = epo_search_input(query, "1-1".into(), countries);
    input["details"] = json!(false);
    let Some(result) = tools::call_tool_data(ctx, "search_patents", &input).await? else {
        return Ok(());
    };
    let total = result.get("total").cloned().unwrap_or(Value::Null);
    if ctx.output_format == "json" {
        let mut payload = json!({ "query": query, "total": total });
        if let Some(countries) = input.get("countries") {
            payload["countries"] = countries.clone();
            payload["note"] = json!("total counts the full CQL result set; the country filter applies to returned documents only");
        }
        output::print_json(&payload);
    } else {
        match &total {
            Value::Null => println!("Total: unknown (tool returned no `total` field)"),
            value => println!("Total: {}", value),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{epo_search_input, partial_page_note};
    use serde_json::json;

    #[test]
    fn a_partial_page_says_how_many_rows_were_not_read() {
        // Without this the table shows blank Title/Applicants cells and nothing
        // says whether the patent HAS no title or was simply never read.
        let note = partial_page_note(&json!({ "returned": 10, "detailsUnavailable": 3 }));
        let note = note.expect("a partial page must announce itself");
        assert!(note.contains('3'), "note must carry the count: {note}");
        assert!(
            note.contains("not read") || note.contains("could not be read"),
            "note must say the rows were unread, not empty: {note}"
        );
    }

    #[test]
    fn a_whole_page_says_nothing() {
        // The backend omits the field when every result hydrated, so silence is
        // the correct rendering — a "0 unavailable" line would be noise.
        assert_eq!(partial_page_note(&json!({ "returned": 10 })), None);
        assert_eq!(partial_page_note(&json!({ "detailsUnavailable": 0 })), None);
        assert_eq!(partial_page_note(&json!({ "docs": [] })), None);
    }

    #[test]
    fn a_non_numeric_count_is_ignored_rather_than_printed_raw() {
        assert_eq!(
            partial_page_note(&json!({ "detailsUnavailable": "three" })),
            None
        );
        assert_eq!(
            partial_page_note(&json!({ "detailsUnavailable": null })),
            None
        );
    }

    #[test]
    fn country_filter_becomes_a_code_list_and_all_means_unfiltered() {
        let filtered = epo_search_input("ti=battery", "1-10".into(), Some("ep, wo"));
        assert_eq!(filtered["provider"], "epo_ops");
        assert_eq!(filtered["range"], "1-10");
        assert_eq!(filtered["countries"], json!(["EP", "WO"]));

        // "all" and an absent filter both send no countries key at all.
        for countries in [Some("all"), Some("ALL"), None] {
            let unfiltered = epo_search_input("ti=battery", "1-10".into(), countries);
            assert!(
                unfiltered.get("countries").is_none(),
                "countries must be absent for {countries:?}"
            );
        }
    }
}
