use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

use crate::client::Context;
use crate::commands::tools;
use crate::output;

#[derive(Parser)]
pub struct LegalArgs {
    #[command(subcommand)]
    command: LegalCommand,
}

#[derive(Subcommand)]
enum LegalCommand {
    /// Search patent law documents
    Search {
        /// Search query
        query: String,

        /// Jurisdiction filter
        #[arg(long)]
        jurisdiction: Option<Jurisdiction>,

        /// Maximum results to return
        #[arg(long, default_value = "10")]
        limit: u32,

        /// Search mode
        #[arg(long, default_value = "hybrid")]
        search_mode: SearchMode,

        /// Include neighboring context chunks
        #[arg(long)]
        include_context: bool,

        /// Return grouped comprehensive results
        #[arg(long)]
        comprehensive: bool,
    },
    /// List available legal jurisdictions and sources
    Jurisdictions,
}

#[derive(Clone, ValueEnum)]
enum Jurisdiction {
    Epo,
    Uspto,
    Eu,
    Wipo,
    All,
}

#[derive(Clone, ValueEnum)]
enum SearchMode {
    Hybrid,
    Semantic,
    Keyword,
}

pub async fn run(ctx: &Context, args: LegalArgs) -> Result<()> {
    ctx.require_auth()?;

    match args.command {
        LegalCommand::Search {
            query,
            jurisdiction,
            limit,
            search_mode,
            include_context,
            comprehensive,
        } => {
            let mut input = json!({
                "query": query,
                "limit": limit,
                "search_mode": search_mode.as_backend_value(),
                "include_context": include_context,
                "comprehensive": comprehensive,
            });
            if let Some(jurisdiction) = jurisdiction {
                input["jurisdiction"] = json!(jurisdiction.as_backend_value());
            }
            call(ctx, "reference_search", &input).await
        }
        LegalCommand::Jurisdictions => call(ctx, "get_legal_jurisdictions", &json!({})).await,
    }
}

async fn call(ctx: &Context, tool: &str, input: &serde_json::Value) -> Result<()> {
    if let Some(result) = tools::call_tool_data(ctx, tool, input).await? {
        output::print_value(&ctx.output_format, &result, &[]);
    }
    Ok(())
}

impl Jurisdiction {
    fn as_backend_value(&self) -> &'static str {
        match self {
            Jurisdiction::Epo => "EPO",
            Jurisdiction::Uspto => "USPTO",
            Jurisdiction::Eu => "EU",
            Jurisdiction::Wipo => "WIPO",
            Jurisdiction::All => "all",
        }
    }
}

impl SearchMode {
    fn as_backend_value(&self) -> &'static str {
        match self {
            SearchMode::Hybrid => "hybrid",
            SearchMode::Semantic => "semantic",
            SearchMode::Keyword => "keyword",
        }
    }
}
