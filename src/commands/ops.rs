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
    }
}

async fn search(ctx: &Context, cql: &str, start: u32, end: u32) -> Result<()> {
    let input = patent::epo_search_input(cql, format!("{}-{}", start, end), None);
    if let Some(result) = tools::call_tool_data(ctx, "search_patents", &input).await? {
        patent::print_search_result(ctx, &result);
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
