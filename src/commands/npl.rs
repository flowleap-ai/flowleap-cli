use anyhow::Result;
use clap::{Parser, ValueEnum};
use serde_json::json;

use crate::client::Context;
use crate::commands::tools;
use crate::output;

#[derive(Parser)]
pub struct NplArgs {
    /// Search query for scholarly works
    pub query: String,

    /// Maximum results to return
    #[arg(long, default_value = "10")]
    pub limit: u32,

    /// Page number
    #[arg(long, default_value = "1")]
    pub page: u32,

    /// Filter by publication year from
    #[arg(long)]
    pub from_year: Option<u32>,

    /// Filter by publication year to
    #[arg(long)]
    pub to_year: Option<u32>,

    /// Only return open-access works
    #[arg(long)]
    pub open_access: bool,

    /// Filter by publication type
    #[arg(long)]
    pub r#type: Option<NplType>,
}

#[derive(Clone, ValueEnum)]
pub enum NplType {
    JournalArticle,
    BookChapter,
    ProceedingsArticle,
    Preprint,
}

pub async fn run(ctx: &Context, args: NplArgs) -> Result<()> {
    ctx.require_auth()?;

    let mut filter = json!({});
    if let Some(year) = args.from_year {
        filter["from_year"] = json!(year);
    }
    if let Some(year) = args.to_year {
        filter["to_year"] = json!(year);
    }
    if args.open_access {
        filter["open_access"] = json!(true);
    }
    if let Some(kind) = args.r#type {
        filter["type"] = json!(kind.as_backend_value());
    }

    let mut input = json!({
        "query": args.query,
        "limit": args.limit,
        "page": args.page,
    });
    if filter
        .as_object()
        .map(|obj| !obj.is_empty())
        .unwrap_or(false)
    {
        input["filter"] = filter;
    }

    if let Some(result) = tools::call_tool_data(ctx, "search_npl", &input).await? {
        output::print_value(&ctx.output_format, &result, &[]);
    }
    Ok(())
}

impl NplType {
    fn as_backend_value(&self) -> &'static str {
        match self {
            NplType::JournalArticle => "journal-article",
            NplType::BookChapter => "book-chapter",
            NplType::ProceedingsArticle => "proceedings-article",
            NplType::Preprint => "preprint",
        }
    }
}
