use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

use crate::client::Context;
use crate::commands::tools;
use crate::output;

#[derive(Parser)]
pub struct CitationArgs {
    #[command(subcommand)]
    command: CitationCommand,
}

#[derive(Subcommand)]
enum CitationCommand {
    /// Search citations by USPTO application number
    Search {
        /// USPTO application number
        application_number: String,

        /// Number of results to return
        #[arg(long, default_value = "100")]
        size: u32,

        /// Pagination offset
        #[arg(long, default_value = "0")]
        offset: u32,

        /// Citation category filter
        #[arg(long)]
        category: Option<CitationCategory>,

        /// Only return examiner-cited references
        #[arg(long)]
        examiner_cited_only: bool,

        /// Earliest office-action date (YYYY-MM-DD)
        #[arg(long = "from", value_name = "YYYY-MM-DD")]
        from: Option<String>,

        /// Latest office-action date (YYYY-MM-DD)
        #[arg(long = "to", value_name = "YYYY-MM-DD")]
        to: Option<String>,
    },
    /// Find patents that cite a document
    Forward {
        /// Cited patent or publication document
        cited_document: String,

        /// Number of results to return
        #[arg(long, default_value = "100")]
        size: u32,

        /// Pagination offset
        #[arg(long, default_value = "0")]
        offset: u32,

        /// Citation category filter
        #[arg(long)]
        category: Option<CitationCategory>,

        /// Only return examiner-cited references
        #[arg(long)]
        examiner_cited_only: bool,
    },
    /// Get citation statistics for an application
    Stats {
        /// USPTO application number
        application_number: String,
    },
    /// Get X-rated novelty-destroying citations
    Novelty {
        /// USPTO application number
        application_number: String,

        /// Number of results to return
        #[arg(long, default_value = "100")]
        size: u32,
    },
}

#[derive(Clone, ValueEnum)]
enum CitationCategory {
    X,
    Y,
    A,
    All,
}

pub async fn run(ctx: &Context, args: CitationArgs) -> Result<()> {
    ctx.require_auth()?;

    match args.command {
        CitationCommand::Search {
            application_number,
            size,
            offset,
            category,
            examiner_cited_only,
            from,
            to,
        } => {
            let mut input = json!({
                "application_number": application_number,
                "size": size,
                "offset": offset,
                "examiner_cited_only": examiner_cited_only,
            });
            if let Some(category) = category {
                input["category"] = json!(category.as_backend_value());
            }
            if let Some(range) = date_range(from.as_deref(), to.as_deref()) {
                input["date_range"] = range;
            }
            call(ctx, "search_office_action_citations", &input).await
        }
        CitationCommand::Forward {
            cited_document,
            size,
            offset,
            category,
            examiner_cited_only,
        } => {
            let mut input = json!({
                "cited_document": cited_document,
                "size": size,
                "offset": offset,
                "examiner_cited_only": examiner_cited_only,
            });
            if let Some(category) = category {
                input["category"] = json!(category.as_backend_value());
            }
            call(ctx, "search_enriched_citations", &input).await
        }
        CitationCommand::Stats { application_number } => {
            let input = json!({ "application_number": application_number });
            call(ctx, "get_citation_stats", &input).await
        }
        // The novelty recipe over the citation tool: X-category references the
        // examiner held to destroy novelty under 35 USC 102 on their own. It is
        // a documented parameter combination, not a capability of its own.
        CitationCommand::Novelty {
            application_number,
            size,
        } => {
            let input = json!({
                "application_number": application_number,
                "size": size,
                "category": "X",
                "examiner_cited_only": true,
            });
            call(ctx, "search_office_action_citations", &input).await
        }
    }
}

/// The tool's inclusive `date_range` window, or None when neither bound is set.
fn date_range(from: Option<&str>, to: Option<&str>) -> Option<serde_json::Value> {
    if from.is_none() && to.is_none() {
        return None;
    }
    let mut range = json!({});
    if let Some(from) = from {
        range["from"] = json!(from);
    }
    if let Some(to) = to {
        range["to"] = json!(to);
    }
    Some(range)
}

async fn call(ctx: &Context, tool: &str, input: &serde_json::Value) -> Result<()> {
    if let Some(result) = tools::call_tool_data(ctx, tool, input).await? {
        output::print_value(&ctx.output_format, &result, &[]);
    }
    Ok(())
}

impl CitationCategory {
    fn as_backend_value(&self) -> &'static str {
        match self {
            CitationCategory::X => "X",
            CitationCategory::Y => "Y",
            CitationCategory::A => "A",
            CitationCategory::All => "all",
        }
    }
}
