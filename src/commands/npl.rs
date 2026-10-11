use anyhow::Result;
use clap::{Parser, ValueEnum};
use serde_json::{json, Value};

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
    Review,
    Book,
    Dissertation,
    Report,
    Standard,
    Dataset,
}

/// Resolve one cited non-patent reference to its OpenAlex record.
#[derive(Parser)]
#[command(after_help = "Examples:
  flowleap npl-work 10.1038/nature14539
  flowleap --json npl-work \"SMITH J: 'Hydraulic actuator control', IEEE TRANS., 2015\"

REFERENCE is a DOI, an OpenAlex work id (W…) or a cited-NPL string exactly as
an office printed it (a citedReferences[].npl entry of `ops biblio`, an NPL row
of `patent examiner-baseline`). Only status `matched` is the cited paper;
`candidates` are similar works, none certain.")]
pub struct NplWorkArgs {
    /// DOI, OpenAlex work id, or a cited-NPL string as printed
    pub reference: String,
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

pub async fn run_work(ctx: &Context, args: NplWorkArgs) -> Result<()> {
    ctx.require_auth()?;

    let input = json!({ "reference": args.reference });
    let Some(result) = tools::call_tool_data(ctx, "get_npl_work", &input).await? else {
        return Ok(());
    };
    if ctx.output_format == "json" {
        output::print_json(&result);
    } else {
        print!("{}", render_npl_work(&result));
    }
    Ok(())
}

/// Human rendering of a `get_npl_work` result. A `candidates` answer is shown
/// as possible matches only: none of them is the cited paper.
pub fn render_npl_work(result: &Value) -> String {
    let text = |v: &Value| v.as_str().unwrap_or("").to_string();
    let status = result["status"].as_str().unwrap_or("unknown");
    let mut out = format!(
        "Reference: {}\nStatus:    {}",
        text(&result["reference"]),
        status
    );
    if let Some(method) = result["method"].as_str() {
        out.push_str(&format!(" (by {method})"));
    }
    out.push('\n');

    match status {
        "matched" => {
            let work = &result["work"];
            let authors = work["authors"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            for (label, value) in [
                ("Title", text(&work["title"])),
                ("Authors", authors),
                ("Venue", text(&work["source"])),
                ("Date", text(&work["publicationDate"])),
                ("DOI", text(&work["doi"])),
                ("OpenAlex", text(&work["id"])),
                ("Open access", text(&work["openAccessUrl"])),
            ] {
                if !value.is_empty() {
                    out.push_str(&format!("{label:<12} {value}\n"));
                }
            }
            if let Some(abstract_text) = work["abstract"].as_str() {
                out.push_str(&format!("\n{abstract_text}\n"));
            }
        }
        "candidates" => {
            out.push_str("Possible matches, none certain (do not treat any as the cited paper):\n");
            for candidate in result["candidates"].as_array().into_iter().flatten() {
                let work = &candidate["work"];
                let score = candidate["score"].as_f64().unwrap_or(0.0);
                out.push_str(&format!(
                    "  - {} ({}) {} [similarity {:.2}]\n",
                    text(&work["title"]),
                    text(&work["publicationDate"]),
                    text(&work["doi"]),
                    score
                ));
            }
        }
        _ => out.push_str(
            "No OpenAlex record (standards, product manuals and web pages are often absent).\n",
        ),
    }
    out
}

impl NplType {
    fn as_backend_value(&self) -> &'static str {
        match self {
            NplType::JournalArticle => "journal-article",
            NplType::BookChapter => "book-chapter",
            NplType::ProceedingsArticle => "proceedings-article",
            NplType::Preprint => "preprint",
            NplType::Review => "review",
            NplType::Book => "book",
            NplType::Dissertation => "dissertation",
            NplType::Report => "report",
            NplType::Standard => "standard",
            NplType::Dataset => "dataset",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matched_work_renders_its_record() {
        let out = render_npl_work(&json!({
            "reference": "10.1038/nature14539",
            "status": "matched",
            "method": "doi",
            "work": {
                "id": "W2919115771",
                "doi": "10.1038/nature14539",
                "title": "Deep learning",
                "publicationDate": "2015-05-26",
                "authors": ["Yann LeCun", "Yoshua Bengio"],
                "source": "Nature",
                "abstract": null
            },
            "candidates": []
        }));
        assert!(out.contains("Status:    matched (by doi)"), "{out}");
        assert!(out.contains("Deep learning"), "{out}");
        assert!(out.contains("Yann LeCun, Yoshua Bengio"), "{out}");
        assert!(out.contains("Nature"), "{out}");
    }

    #[test]
    fn candidates_are_never_presented_as_the_cited_paper() {
        let out = render_npl_work(&json!({
            "reference": "SMITH: 'A study', 2015",
            "status": "candidates",
            "method": null,
            "work": null,
            "candidates": [
                { "work": { "title": "A study of things", "publicationDate": "2015-01-01", "doi": "10.1/x" }, "score": 0.71 }
            ]
        }));
        assert!(out.contains("none certain"), "{out}");
        assert!(out.contains("A study of things"), "{out}");
        assert!(!out.contains("Title "), "{out}");
    }
}
