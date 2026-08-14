use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

use crate::client::Context;
use crate::commands::tools;
use crate::output;

#[derive(Parser)]
pub struct AcademicArgs {
    #[command(subcommand)]
    command: AcademicCommand,
}

#[derive(Subcommand)]
enum AcademicCommand {
    /// Search academic literature (Semantic Scholar + arXiv)
    Search {
        /// Search query
        query: String,

        /// Maximum results
        #[arg(long, default_value = "10")]
        limit: u32,

        /// Sources to search (repeatable)
        #[arg(long, value_enum)]
        source: Vec<AcademicSource>,

        /// Only include papers published in or after this year
        #[arg(long)]
        from_year: Option<u32>,

        /// Only include papers published in or before this year
        #[arg(long)]
        to_year: Option<u32>,
    },
}

#[derive(Clone, ValueEnum)]
enum AcademicSource {
    Scholar,
    Arxiv,
}

impl AcademicSource {
    /// The `search_academic` tool names Semantic Scholar `semantic-scholar`;
    /// papers still come back tagged with the lib's historical `scholar`.
    fn as_backend_value(&self) -> &'static str {
        match self {
            AcademicSource::Scholar => "semantic-scholar",
            AcademicSource::Arxiv => "arxiv",
        }
    }
}

pub async fn run(ctx: &Context, args: AcademicArgs) -> Result<()> {
    ctx.require_auth()?;

    match args.command {
        AcademicCommand::Search {
            query,
            limit,
            source,
            from_year,
            to_year,
        } => search(ctx, &query, limit, &source, from_year, to_year).await,
    }
}

async fn search(
    ctx: &Context,
    query: &str,
    limit: u32,
    sources: &[AcademicSource],
    from_year: Option<u32>,
    to_year: Option<u32>,
) -> Result<()> {
    let mut input = json!({
        "query": query,
        "max_results": limit,
    });
    if !sources.is_empty() {
        input["sources"] = json!(sources
            .iter()
            .map(AcademicSource::as_backend_value)
            .collect::<Vec<_>>());
    }
    let mut filter = json!({});
    if let Some(year) = from_year {
        filter["from_year"] = json!(year);
    }
    if let Some(year) = to_year {
        filter["to_year"] = json!(year);
    }
    if filter.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
        input["filter"] = filter;
    }

    let Some(result) = tools::call_tool_data(ctx, "search_academic", &input).await? else {
        return Ok(());
    };

    let columns = &[
        ("title", "Title"),
        ("authors", "Authors"),
        ("year", "Year"),
        ("source", "Source"),
        ("citations", "Citations"),
    ];

    match result.get("papers") {
        Some(papers) => output::print_value(&ctx.output_format, papers, columns),
        None => output::print_value(&ctx.output_format, &result, columns),
    }

    Ok(())
}
