use anyhow::{bail, Result};
use clap::Parser;
use serde_json::{json, Value};

use crate::client::Context;
use crate::commands::tools;
use crate::output;

/// Full-corpus patent analytics: filing trends by year, country and CPC
/// breakdowns, and top assignees (the `patent_analytics` tool).
///
/// At least one criterion is required. The backend's old free-form `query`
/// parameter is deprecated and rejected server-side, so this command only
/// accepts structured criteria.
#[derive(Parser)]
#[command(after_help = "Examples:
  flowleap analytics --keyword battery --date-from 2015-01-01
  flowleap analytics --keyword AI --phrase \"machine learning\" --assignee Siemens \\
      --cpc G06N --ipc H04L --country US --date-from 2020-01-01 --date-to 2025-12-31

Note: the deprecated free-form query parameter is not supported by the backend;
use the structured flags above instead.")]
pub struct AnalyticsArgs {
    /// Keyword to match (repeatable; OR logic between keywords)
    #[arg(long = "keyword", value_name = "WORD")]
    keywords: Vec<String>,

    /// Exact phrase to match (repeatable)
    #[arg(long = "phrase", value_name = "PHRASE")]
    phrases: Vec<String>,

    /// Assignee (applicant/company) name filter
    #[arg(long)]
    assignee: Option<String>,

    /// CPC classification prefix, e.g. G06N (repeatable)
    #[arg(long = "cpc", value_name = "CODE")]
    cpc: Vec<String>,

    /// IPC classification prefix, e.g. H04L (repeatable)
    #[arg(long = "ipc", value_name = "CODE")]
    ipc: Vec<String>,

    /// Two-letter country code filter, e.g. US
    #[arg(long = "country", value_name = "CC")]
    country: Option<String>,

    /// Earliest publication date (YYYY-MM-DD)
    #[arg(long = "date-from", value_name = "YYYY-MM-DD")]
    date_from: Option<String>,

    /// Latest publication date (YYYY-MM-DD)
    #[arg(long = "date-to", value_name = "YYYY-MM-DD")]
    date_to: Option<String>,
}

impl AnalyticsArgs {
    /// The backend requires at least one search or filter criterion.
    fn has_criterion(&self) -> bool {
        !self.keywords.is_empty()
            || !self.phrases.is_empty()
            || self.assignee.is_some()
            || !self.cpc.is_empty()
            || !self.ipc.is_empty()
            || self.country.is_some()
            || self.date_from.is_some()
            || self.date_to.is_some()
    }

    /// Structured request body; absent criteria are omitted entirely.
    fn request_body(&self) -> Value {
        let mut body = json!({});
        if !self.keywords.is_empty() {
            body["keywords"] = json!(self.keywords);
        }
        if !self.phrases.is_empty() {
            body["phrases"] = json!(self.phrases);
        }
        if let Some(ref assignee) = self.assignee {
            body["assignee"] = json!(assignee);
        }
        if !self.cpc.is_empty() {
            body["cpc"] = json!(self.cpc);
        }
        if !self.ipc.is_empty() {
            body["ipc"] = json!(self.ipc);
        }
        if let Some(ref country) = self.country {
            body["countryCode"] = json!(country);
        }
        if let Some(ref date_from) = self.date_from {
            body["dateFrom"] = json!(date_from);
        }
        if let Some(ref date_to) = self.date_to {
            body["dateTo"] = json!(date_to);
        }
        body
    }
}

pub async fn run(ctx: &Context, args: AnalyticsArgs) -> Result<()> {
    // Validate locally before any network call: the backend would reject an
    // empty request with a 400 anyway, so fail fast with actionable guidance.
    if !args.has_criterion() {
        bail!(
            "No analytics criteria given. Provide at least one of: --keyword, --phrase, \
             --assignee, --cpc, --ipc, --country, --date-from, or --date-to.\n\
             Example: flowleap analytics --keyword battery --date-from 2015-01-01"
        );
    }
    ctx.require_auth()?;

    let input = args.request_body();
    let Some(result) = tools::call_tool_data(ctx, "patent_analytics", &input).await? else {
        return Ok(());
    };

    // JSON mode emits the tool payload untouched.
    if ctx.output_format == "json" {
        output::print_value(&ctx.output_format, &result, &[]);
        return Ok(());
    }

    print_analytics(&result);
    Ok(())
}

/// Render the four aggregates as labeled tables for human/table output.
fn print_analytics(result: &Value) {
    if let Some(description) = result.get("searchDescription").and_then(Value::as_str) {
        if !description.is_empty() {
            println!("Search: {description}");
        }
    }

    let analytics = result.get("analytics").cloned().unwrap_or(Value::Null);
    print_aggregate(
        &analytics,
        "byYear",
        "Filings by Year",
        &[("year", "Year"), ("count", "Filings")],
    );
    print_aggregate(
        &analytics,
        "byCountry",
        "Filings by Country",
        &[("country", "Country"), ("count", "Filings")],
    );
    print_aggregate(
        &analytics,
        "topAssignees",
        "Top Assignees",
        &[("assignee", "Assignee"), ("count", "Filings")],
    );
    print_aggregate(
        &analytics,
        "topCPC",
        "Top CPC Classes",
        &[("cpc", "CPC"), ("count", "Filings")],
    );
}

fn print_aggregate(analytics: &Value, key: &str, label: &str, columns: &[(&str, &str)]) {
    println!("\n{label}");
    match analytics.get(key).and_then(Value::as_array) {
        Some(rows) if !rows.is_empty() => output::print_table(rows, columns),
        _ => println!("  (no data)"),
    }
}
