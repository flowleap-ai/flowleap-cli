use anyhow::{bail, Context as AnyhowContext, Result};
use clap::{Parser, Subcommand};
use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;

use crate::client::{encode_url_component, Context};
use crate::output;

#[derive(Parser)]
pub struct ToolsArgs {
    #[command(subcommand)]
    command: ToolsCommand,
}

#[derive(Subcommand)]
enum ToolsCommand {
    /// List available backend tools
    List,
    /// Show a tool's description and JSON input schema
    Describe {
        /// Tool name, e.g. get_bibliography
        name: String,
    },
    /// Run a tool with a JSON input
    Run {
        /// Tool name, e.g. search_patents
        name: String,

        /// Inline JSON input, e.g. '{"query":"ti=battery"}'
        #[arg(long, conflicts_with = "input_file")]
        input: Option<String>,

        /// File containing the JSON input
        #[arg(long)]
        input_file: Option<PathBuf>,

        /// key=value input pairs (values parsed as JSON when possible),
        /// e.g. patent_number=EP1000000 limit=5
        #[arg(value_name = "KEY=VALUE")]
        params: Vec<String>,
    },
    /// Print the tool registry's OpenAPI document
    Openapi,
}

pub async fn run(ctx: &Context, args: ToolsArgs) -> Result<()> {
    ctx.require_auth()?;

    match args.command {
        ToolsCommand::List => list(ctx).await,
        ToolsCommand::Describe { name } => describe(ctx, &name).await,
        ToolsCommand::Run {
            name,
            input,
            input_file,
            params,
        } => run_tool(ctx, &name, input.as_deref(), input_file, &params).await,
        ToolsCommand::Openapi => openapi(ctx).await,
    }
}

async fn fetch_tools(ctx: &Context) -> Result<Value> {
    ctx.execute_json_body_or_error(ctx.get("/v1/tools")).await
}

async fn list(ctx: &Context) -> Result<()> {
    let result = fetch_tools(ctx).await?;
    let columns = &[("name", "Tool"), ("description", "Description")];
    if let Some(tools) = result.get("tools") {
        output::print_value(&ctx.output_format, tools, columns);
    } else {
        output::print_value(&ctx.output_format, &result, columns);
    }
    Ok(())
}

async fn describe(ctx: &Context, name: &str) -> Result<()> {
    let result = fetch_tools(ctx).await?;
    if result.get("dryRun").and_then(|v| v.as_bool()) == Some(true) {
        output::print_json(&result);
        return Ok(());
    }
    let tool = result
        .get("tools")
        .and_then(|t| t.as_array())
        .and_then(|tools| {
            tools
                .iter()
                .find(|t| t.get("name").and_then(|n| n.as_str()) == Some(name))
        });

    match tool {
        Some(tool) => {
            output::print_json(tool);
            Ok(())
        }
        None => bail!(
            "Unknown tool: {}. Run 'flowleap tools list' to see available tools.",
            name
        ),
    }
}

fn tool_path(name: &str) -> String {
    format!("/v1/tools/{}", encode_url_component(name))
}

/// Execute a backend tool through the `/v1/tools/{name}` facade.
///
/// Returns the backend's tool envelope (`{ success, tool, data,
/// executionTimeMs }`), or the dry-run description when `--dry-run` is
/// active. Shared by `tools run` and the ergonomic verbs (`compare`,
/// `figures`, `summary`, `timeline`, `convert-number`).
pub async fn call_tool(ctx: &Context, name: &str, input: &Value) -> Result<Value> {
    ctx.execute_json_body_or_error(ctx.post(&tool_path(name), input))
        .await
}

/// Run a backend tool and hand back its `data` payload — the seam every
/// data command sits on since the facade migration (backend PRD 0013).
///
/// Unwraps the facade envelope (`{ success, tool, data, executionTimeMs,
/// cached? }`) so callers see the same payload shape the retired provider
/// routes returned at the top level. Returns `None` when the run is already
/// fully handled and the caller must print nothing more: a `--dry-run`
/// description. Failures print their error envelope (with the structured
/// hints) and return the typed error, exactly as [`call_tool`] does.
pub async fn call_tool_data(ctx: &Context, name: &str, input: &Value) -> Result<Option<Value>> {
    let result = call_tool(ctx, name, input).await?;
    if result.get("dryRun").and_then(|v| v.as_bool()) == Some(true) {
        output::print_json(&result);
        return Ok(None);
    }
    if ctx.verbose {
        if let Some(cached) = result.get("cached").and_then(|v| v.as_bool()) {
            eprintln!("  cached: {}", cached);
        }
        if let Some(ms) = result.get("executionTimeMs").and_then(|v| v.as_u64()) {
            eprintln!("  executionTimeMs: {}", ms);
        }
    }
    // A tool that returns a bare value (rather than an object) still lands
    // under `data`; falling back to the whole envelope keeps a hypothetical
    // envelope-less response printable instead of empty.
    Ok(Some(result.get("data").cloned().unwrap_or(result)))
}

/// Execute a backend tool and return the raw response envelope
/// (`{ ok, status, body, providerKeysHint?, retryAfterSeconds? }`) without
/// printing anything. The seam the MCP bridge (`flowleap mcp`) runs on: it
/// must keep stdout protocol-clean and needs the structured error hints
/// intact instead of `call_tool`'s print-and-fail behavior.
pub async fn call_tool_envelope(ctx: &Context, name: &str, input: &Value) -> Result<Value> {
    ctx.execute_json_envelope(ctx.post(&tool_path(name), input))
        .await
}

/// Fetch the `/v1/tools` registry as a raw response envelope (never prints).
/// Same seam as [`call_tool_envelope`], for MCP `tools/list`.
pub async fn fetch_tools_envelope(ctx: &Context) -> Result<Value> {
    ctx.execute_json_envelope(ctx.get("/v1/tools")).await
}

async fn run_tool(
    ctx: &Context,
    name: &str,
    input: Option<&str>,
    input_file: Option<PathBuf>,
    params: &[String],
) -> Result<()> {
    let body = build_input(input, input_file, params)?;
    let result = call_tool(ctx, name, &body).await?;

    if result.get("dryRun").and_then(|v| v.as_bool()) == Some(true) {
        output::print_json(&result);
        return Ok(());
    }

    // Envelope: { success, tool, data, executionTimeMs }
    if ctx.verbose {
        if let Some(ms) = result.get("executionTimeMs").and_then(|v| v.as_u64()) {
            eprintln!("  executionTimeMs: {}", ms);
        }
    }
    let data = result.get("data").unwrap_or(&result);
    output::print_json(data);
    Ok(())
}

async fn openapi(ctx: &Context) -> Result<()> {
    let result = ctx
        .execute_json_body_or_error(ctx.get("/v1/tools/openapi.json"))
        .await?;
    output::print_json(&result);
    Ok(())
}

/// Build the tool input object from --input / --input-file / key=value pairs.
/// key=value pairs overlay the base JSON, so both styles compose.
fn build_input(
    input: Option<&str>,
    input_file: Option<PathBuf>,
    params: &[String],
) -> Result<Value> {
    let mut object = match (input, input_file) {
        (Some(raw), _) => parse_object(raw)?,
        (None, Some(path)) => {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("read input file {}", path.display()))?;
            parse_object(&raw)?
        }
        (None, None) => Map::new(),
    };

    for pair in params {
        let Some((key, value)) = pair.split_once('=') else {
            bail!("Invalid parameter '{}': expected key=value", pair);
        };
        // Values that parse as JSON keep their type (numbers, bools, arrays);
        // everything else is a plain string.
        let parsed = serde_json::from_str::<Value>(value)
            .unwrap_or_else(|_| Value::String(value.to_string()));
        object.insert(key.to_string(), parsed);
    }

    Ok(Value::Object(object))
}

fn parse_object(raw: &str) -> Result<Map<String, Value>> {
    let value: Value =
        serde_json::from_str(raw).context("tool input must be a valid JSON object")?;
    match value {
        Value::Object(map) => Ok(map),
        _ => bail!("tool input must be a JSON object"),
    }
}

#[cfg(test)]
mod tests {
    use super::build_input;

    #[test]
    fn key_value_pairs_parse_json_types() {
        let input = build_input(None, None, &["limit=5".into(), "query=ti=battery".into()])
            .expect("build_input");
        assert_eq!(input["limit"], 5);
        // Only the FIRST '=' splits key from value.
        assert_eq!(input["query"], "ti=battery");
    }

    #[test]
    fn inline_input_with_overlay() {
        let input = build_input(
            Some(r#"{"query":"x","limit":1}"#),
            None,
            &["limit=9".into()],
        )
        .expect("build_input");
        assert_eq!(input["query"], "x");
        assert_eq!(input["limit"], 9);
    }

    #[test]
    fn rejects_non_object_input() {
        assert!(build_input(Some("[1,2]"), None, &[]).is_err());
        assert!(build_input(None, None, &["no-equals".into()]).is_err());
    }
}
