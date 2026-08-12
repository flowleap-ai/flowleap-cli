use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::json;

use crate::client::Context;
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
    },
}

pub async fn run(ctx: &Context, args: PatentArgs) -> Result<()> {
    ctx.require_auth()?;

    match args.command {
        PatentCommand::Search {
            query,
            limit,
            countries,
        } => search(ctx, &query, limit, countries.as_deref()).await,
    }
}

async fn search(ctx: &Context, query: &str, limit: u32, countries: Option<&str>) -> Result<()> {
    let mut body = json!({
        "query": query,
        "range": format!("1-{}", limit.clamp(1, 100)),
    });
    if let Some(countries) = countries {
        body["countries"] = json!(countries);
    }

    let req = ctx.post("/v1/patent-search", &body);
    let result = ctx.execute_json_body_or_error(req).await?;

    let columns = &[
        ("docId", "Patent ID"),
        ("title", "Title"),
        ("applicants", "Applicants"),
        ("publicationDate", "Date"),
    ];

    if let Some(docs) = result.get("docs") {
        output::print_value(&ctx.output_format, docs, columns);
    } else {
        output::print_value(&ctx.output_format, &result, columns);
    }

    Ok(())
}
