//! `flowleap mcp` — a stdio MCP server bridging the `/v1/tools` facade.
//!
//! A thin bridge with no per-tool code and no name mapping: the backend's
//! `/v1/tools` vocabulary is canonical. `tools/list` passes each registry
//! entry through verbatim (the backend already publishes MCP-shaped
//! `{ name, description, inputSchema }` objects), so new backend tools appear
//! in every MCP harness without a CLI release. `tools/call` runs a tool via
//! the same facade path as `flowleap tools run`.
//!
//! Protocol discipline: nothing but JSON-RPC frames on stdout (one per line);
//! all logging goes to stderr. Tool-level failures — including the backend's
//! structured `providerKeysHint` / subscription / rate-limit envelopes — are
//! returned as MCP tool results with `isError: true`, never as JSON-RPC
//! transport errors, so the calling agent can read the hint and act.
//!
//! Doctrine: at startup the bridge fetches the backend's PATSTAT documents
//! (`GET /v1/patstat/docs`: the semantic model, the verified examples and
//! three workflows) and serves them verbatim as MCP resources, plus one MCP
//! prompt per workflow. The bridge authors no doctrine: a prompt is a plain
//! text rendering of the served workflow JSON. A document that fails to load
//! is logged to stderr and skipped; the rest keep serving.

use anyhow::Result;
use clap::Parser;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use tokio::sync::OnceCell;

use crate::client::Context;
use crate::commands::tools;

/// Protocol version offered when the client requests one we don't know.
const FALLBACK_PROTOCOL_VERSION: &str = "2024-11-05";
/// Versions we can mirror back — the tools/resources/prompts surface is
/// identical in all. 2026-07-28 removed the session id and the initialize
/// handshake for the HTTP transport only; stdio is unchanged.
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] =
    &["2024-11-05", "2025-03-26", "2025-06-18", "2026-07-28"];

const AUTH_REQUIRED_MESSAGE: &str = "Not authenticated with FlowLeap. Run 'flowleap auth login' \
     (or set FLOWLEAP_API_KEY / FLOWLEAP_TOKEN), then restart the MCP server.";

const HARNESS_WIRING_HELP: &str = "\
Wire into an MCP-capable harness:

  Claude Code:
    claude mcp add flowleap -- flowleap mcp

  Cursor (~/.cursor/mcp.json or .cursor/mcp.json):
    {
      \"mcpServers\": {
        \"flowleap\": { \"command\": \"flowleap\", \"args\": [\"mcp\"] }
      }
    }

  Codex (~/.codex/config.toml):
    [mcp_servers.flowleap]
    command = \"flowleap\"
    args = [\"mcp\"]

Authentication reuses stored credentials: run 'flowleap auth login' once
before wiring the server (unauthenticated servers still start, but every
tool call returns an error explaining how to log in).";

/// Serve FlowLeap backend tools over the Model Context Protocol (stdio).
///
/// Bridges the /v1/tools facade: tools/list mirrors every backend tool with
/// its JSON input schema verbatim, tools/call runs one. Also serves the
/// backend's PATSTAT documents as MCP resources and prompts. Speaks JSON-RPC
/// 2.0, one frame per line, on stdin/stdout.
#[derive(Parser)]
#[command(after_long_help = HARNESS_WIRING_HELP)]
pub struct McpArgs {
    /// Print readiness diagnostics (backend, auth, provider keys, tool,
    /// resource and prompt counts) and exit instead of serving — the
    /// one-command onboarding check.
    #[arg(long)]
    pub check: bool,
}

pub async fn run(ctx: &Context, args: McpArgs) -> Result<()> {
    if args.check {
        return run_check(ctx).await;
    }
    // stdout carries protocol frames only; everything human goes to stderr.
    eprintln!(
        "flowleap mcp v{}: serving MCP over stdio (backend: {})",
        env!("CARGO_PKG_VERSION"),
        ctx.config.base_url
    );
    if ctx.credentials.auth_header().is_none() {
        eprintln!(
            "flowleap mcp: no stored credentials — tool calls will ask for 'flowleap auth login'"
        );
    }

    // The doctrine loads at startup, concurrently with serving: initialize and
    // tools never wait on it, and a resources/prompts request waits for the
    // one in-flight load instead of starting another.
    let doctrine = OnceCell::new();
    let preload = async {
        let loaded = doctrine.get_or_init(|| load_doctrine(ctx)).await;
        eprintln!(
            "flowleap mcp: serving {} resources and {} prompts",
            loaded.resources.len(),
            loaded.prompts.len()
        );
    };
    let (_, served) = tokio::join!(preload, serve(ctx, &doctrine));
    served
}

async fn serve(ctx: &Context, doctrine: &OnceCell<Doctrine>) -> Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let Some(response) = handle_line(ctx, doctrine, &line).await else {
            continue; // notification — no frame goes back
        };
        let mut frame = serde_json::to_string(&response)?;
        frame.push('\n');
        stdout.write_all(frame.as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}

/// `flowleap mcp --check` — readiness diagnostics for harness onboarding.
/// This is NOT protocol mode: results go to stdout (JSON when requested),
/// remediation hints to stderr, and the exit code reflects readiness.
async fn run_check(ctx: &Context) -> Result<()> {
    let wants_json = ctx.output_format == "json";

    // 1. Backend reachability (no auth required).
    let backend_ok = ctx
        .http
        .get(format!("{}/health", ctx.config.base_url))
        .send()
        .await
        .map(|resp| resp.status().is_success())
        .unwrap_or(false);

    // 2. Stored/ambient FlowLeap auth.
    let auth_kind = if ctx.credentials.token.is_some() {
        Some("session token")
    } else if ctx.credentials.api_key.is_some() {
        Some("personal token / API key")
    } else {
        None
    };

    // 3. Provider keys (presence only — never values).
    let epo_ok = ctx.credentials.epo_key.is_some() && ctx.credentials.epo_secret.is_some();
    let uspto_ok = ctx.credentials.uspto_key.is_some();

    // 4. Live tool list — also proves the auth actually works.
    let tool_count = if auth_kind.is_some() && backend_ok {
        match tools::fetch_tools_envelope(ctx).await {
            Ok(envelope) => envelope
                .get("body")
                .and_then(|body| body.get("tools"))
                .and_then(Value::as_array)
                .map(|tools| tools.len()),
            Err(_) => None,
        }
    } else {
        None
    };

    // 5. Doctrine documents — informative only: a document that fails to
    // load (a plan-gated 402, say) never makes the bridge unready.
    let doctrine = if tool_count.is_some() {
        Some(load_doctrine(ctx).await)
    } else {
        None
    };

    let ready = backend_ok && auth_kind.is_some() && tool_count.is_some();

    if wants_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ready": ready,
                "backend": { "url": ctx.config.base_url, "reachable": backend_ok },
                "auth": { "configured": auth_kind.is_some(), "kind": auth_kind, "verified": tool_count.is_some() },
                "providerKeys": { "epoOps": epo_ok, "usptoOdp": uspto_ok },
                "toolCount": tool_count,
                "resourceCount": doctrine.as_ref().map(|d| d.resources.len()),
                "promptCount": doctrine.as_ref().map(|d| d.prompts.len()),
                "unloadedDocuments": doctrine.as_ref().map(|d| d.unloaded.clone()),
            }))?
        );
    } else {
        let mark = |ok: bool| if ok { "ok " } else { "MISSING" };
        println!("flowleap mcp readiness check");
        println!("  backend   {}  {}", mark(backend_ok), ctx.config.base_url);
        println!(
            "  auth      {}  {}",
            mark(auth_kind.is_some() && tool_count.is_some()),
            auth_kind.unwrap_or("no credentials stored")
        );
        println!(
            "  epo ops   {}  (optional: unlocks EPO-gated data)",
            mark(epo_ok)
        );
        println!(
            "  uspto odp {}  (optional: unlocks USPTO-gated data)",
            mark(uspto_ok)
        );
        match tool_count {
            Some(count) => println!("  tools     ok   {count} tools served over MCP"),
            None => println!("  tools     n/a  (needs backend + working auth)"),
        }
        match &doctrine {
            Some(doctrine) => {
                let unloaded = doctrine.unloaded.join(", ");
                let (resources, prompts) = (doctrine.resources.len(), doctrine.prompts.len());
                if doctrine.unloaded.is_empty() {
                    println!("  resources ok   {resources} served");
                    println!("  prompts   ok   {prompts} served");
                } else {
                    println!("  resources warn {resources} served (could not load: {unloaded})");
                    println!("  prompts   warn {prompts} served (could not load: {unloaded})");
                }
            }
            None => {
                println!("  resources n/a  (needs backend + working auth)");
                println!("  prompts   n/a  (needs backend + working auth)");
            }
        }
    }

    if !ready {
        if auth_kind.is_none() {
            eprintln!(
                "fix: run 'flowleap auth login' (or set FLOWLEAP_TOKEN in the MCP config env)"
            );
        } else if backend_ok && tool_count.is_none() {
            eprintln!("fix: credentials present but rejected — re-run 'flowleap auth login'");
        }
        if !backend_ok {
            eprintln!("fix: backend unreachable — check network or base URL ('flowleap doctor')");
        }
        anyhow::bail!("mcp is not ready to serve");
    }
    if !epo_ok || !uspto_ok {
        eprintln!("note: provider keys are optional; add via 'flowleap keys set' to unlock provider-gated data");
    }
    Ok(())
}

/// Parse one inbound line and produce the response frame, if any.
async fn handle_line(ctx: &Context, doctrine: &OnceCell<Doctrine>, line: &str) -> Option<Value> {
    let message: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(err) => {
            return Some(error_response(
                Value::Null,
                -32700,
                &format!("Parse error: {err}"),
                None,
            ))
        }
    };

    let method = message
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or_default()
        .to_string();
    // Absent id = notification (no response). A present id — even null — is
    // echoed back so the client can correlate.
    let id = message.get("id").cloned();
    let params = message.get("params").cloned().unwrap_or(Value::Null);

    match method.as_str() {
        "initialize" => id.map(|id| result_response(id, initialize_result(&params))),
        "notifications/initialized" => None,
        "ping" => id.map(|id| result_response(id, json!({}))),
        "tools/list" => match id {
            Some(id) => Some(tools_list(ctx, id).await),
            None => None,
        },
        "tools/call" => match id {
            Some(id) => Some(tools_call(ctx, id, &params).await),
            None => None,
        },
        "resources/list"
        | "resources/read"
        | "resources/templates/list"
        | "prompts/list"
        | "prompts/get" => match id {
            Some(id) => {
                let doctrine = doctrine.get_or_init(|| load_doctrine(ctx)).await;
                Some(doctrine_request(doctrine, &method, id, &params))
            }
            None => None,
        },
        _ => id.map(|id| error_response(id, -32601, &format!("Method not found: {method}"), None)),
    }
}

/// Mirror the client's protocol version when we support it; otherwise offer
/// the oldest version we speak, per the MCP version-negotiation rules.
fn initialize_result(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(|v| v.as_str());
    let version = match requested {
        Some(v) if SUPPORTED_PROTOCOL_VERSIONS.contains(&v) => v,
        _ => FALLBACK_PROTOCOL_VERSION,
    };
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {}, "resources": {}, "prompts": {} },
        "serverInfo": {
            "name": "flowleap",
            "version": env!("CARGO_PKG_VERSION"),
        },
    })
}

/// tools/list ← GET /v1/tools. Registry entries pass through verbatim — the
/// backend already publishes MCP-shaped `{ name, description, inputSchema }`
/// objects, and extra fields are legal for MCP clients to ignore.
async fn tools_list(ctx: &Context, id: Value) -> Value {
    if ctx.credentials.auth_header().is_none() {
        return error_response(id, -32002, AUTH_REQUIRED_MESSAGE, None);
    }
    let envelope = match tools::fetch_tools_envelope(ctx).await {
        Ok(envelope) => envelope,
        Err(err) => return error_response(id, -32603, &err.to_string(), None),
    };
    if envelope.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let status = envelope.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
        return error_response(
            id,
            -32002,
            &format!("FlowLeap backend rejected tools/list (HTTP {status})"),
            Some(envelope),
        );
    }
    let tools = envelope
        .get("body")
        .and_then(|body| body.get("tools"))
        .and_then(|tools| tools.as_array())
        .cloned()
        .unwrap_or_default();
    result_response(id, json!({ "tools": tools }))
}

/// tools/call ← POST /v1/tools/{name}. Success returns the tool payload as
/// pretty JSON text; backend error envelopes (providerKeysHint, subscription,
/// rate-limit) come back as `isError` tool results carrying the full
/// structured envelope — never as JSON-RPC transport errors.
async fn tools_call(ctx: &Context, id: Value, params: &Value) -> Value {
    if ctx.credentials.auth_header().is_none() {
        return tool_error(id, json!({ "error": AUTH_REQUIRED_MESSAGE }));
    }
    let Some(name) = params.get("name").and_then(|n| n.as_str()) else {
        return error_response(id, -32602, "Invalid params: missing tool 'name'", None);
    };
    let arguments = match params.get("arguments") {
        None | Some(Value::Null) => json!({}),
        Some(value @ Value::Object(_)) => value.clone(),
        Some(_) => {
            return error_response(
                id,
                -32602,
                "Invalid params: 'arguments' must be an object",
                None,
            )
        }
    };

    let envelope = match tools::call_tool_envelope(ctx, name, &arguments).await {
        Ok(envelope) => envelope,
        // Transport failure (timeout, connection refused): still a tool-level
        // error the agent should read, not a protocol error.
        Err(err) => return tool_error(id, json!({ "error": err.to_string() })),
    };

    if envelope.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        // Backend tool envelope: { success, tool, data, executionTimeMs } —
        // surface `data` (like `flowleap tools run`), whole body otherwise.
        let body = envelope.get("body").cloned().unwrap_or(Value::Null);
        let payload = body.get("data").cloned().unwrap_or(body);
        return text_result(id, false, &payload);
    }
    // The envelope already carries status, body, and any structured hints
    // (providerKeysHint, retryAfterSeconds) — pass it through whole.
    tool_error(id, envelope)
}

fn result_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut error = json!({ "code": code, "message": message });
    if let Some(data) = data {
        error["data"] = data;
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": error })
}

/// A successful (or `isError`) MCP tool result with one pretty-JSON text block.
fn text_result(id: Value, is_error: bool, payload: &Value) -> Value {
    let text = serde_json::to_string_pretty(payload).unwrap_or_else(|_| payload.to_string());
    let mut result = json!({ "content": [{ "type": "text", "text": text }] });
    if is_error {
        result["isError"] = json!(true);
    }
    result_response(id, result)
}

fn tool_error(id: Value, payload: Value) -> Value {
    text_result(id, true, &payload)
}

// ---------------------------------------------------------------------------
// Doctrine: PATSTAT documents served as MCP resources and prompts.
// ---------------------------------------------------------------------------

/// One backend document the bridge serves.
struct DocSpec {
    /// Short key, used in logs and in the resource URI after `patstat/`.
    key: &'static str,
    /// Query string for `GET /v1/patstat/docs`.
    query: &'static str,
    /// Workflow documents also become a prompt of this name.
    prompt: Option<&'static str>,
    /// Neutral labels used when the payload carries no name or description.
    fallback_title: &'static str,
    fallback_description: &'static str,
}

const DOC_SPECS: [DocSpec; 5] = [
    DocSpec {
        key: "semantic-model",
        query: "section=semantic-model",
        prompt: None,
        fallback_title: "PATSTAT semantic model",
        fallback_description: "The PATSTAT semantic model, as served by the FlowLeap backend.",
    },
    DocSpec {
        key: "examples",
        query: "section=examples",
        prompt: None,
        fallback_title: "PATSTAT examples",
        fallback_description: "The PATSTAT verified examples, as served by the FlowLeap backend.",
    },
    DocSpec {
        key: "workflow/portfolio-analysis",
        query: "workflow=portfolio-analysis",
        prompt: Some("patstat-portfolio-analysis"),
        fallback_title: "PATSTAT workflow: portfolio-analysis",
        fallback_description:
            "The PATSTAT portfolio-analysis workflow, as served by the FlowLeap backend.",
    },
    DocSpec {
        key: "workflow/guarded-sql",
        query: "workflow=guarded-sql",
        prompt: Some("patstat-guarded-sql"),
        fallback_title: "PATSTAT workflow: guarded-sql",
        fallback_description:
            "The PATSTAT guarded-sql workflow, as served by the FlowLeap backend.",
    },
    DocSpec {
        key: "workflow/graph",
        query: "workflow=graph",
        prompt: Some("patstat-graph"),
        fallback_title: "PATSTAT workflow: graph",
        fallback_description: "The PATSTAT graph workflow, as served by the FlowLeap backend.",
    },
];

struct Resource {
    uri: String,
    name: String,
    title: String,
    description: String,
    mime_type: &'static str,
    text: String,
}

struct Prompt {
    name: &'static str,
    title: String,
    description: String,
    text: String,
}

/// Everything loaded at startup. Empty when the backend is unreachable, the
/// server is unauthenticated, or `--dry-run` is set.
#[derive(Default)]
struct Doctrine {
    resources: Vec<Resource>,
    prompts: Vec<Prompt>,
    /// Keys of the documents that could not be loaded, in spec order.
    unloaded: Vec<String>,
}

/// Fetch the five documents concurrently (a fixed set of five small GETs is
/// its own bound) through the shared client, so credentials, base URL,
/// dry-run and redaction all apply. Failures are logged and skipped.
async fn load_doctrine(ctx: &Context) -> Doctrine {
    let mut doctrine = Doctrine::default();
    if ctx.credentials.auth_header().is_none() && !ctx.dry_run {
        doctrine.unloaded = DOC_SPECS.iter().map(|s| s.key.to_string()).collect();
        eprintln!("flowleap mcp: no credentials — serving no PATSTAT resources or prompts");
        return doctrine;
    }

    let [a, b, c, d, e] = &DOC_SPECS;
    let fetched = tokio::join!(
        fetch_doc(ctx, a),
        fetch_doc(ctx, b),
        fetch_doc(ctx, c),
        fetch_doc(ctx, d),
        fetch_doc(ctx, e)
    );
    let fetched = [fetched.0, fetched.1, fetched.2, fetched.3, fetched.4];

    for (spec, outcome) in DOC_SPECS.iter().zip(fetched) {
        match outcome {
            Fetched::DryRun(request) => {
                // stdout carries frames only: the dry-run request goes to stderr.
                eprintln!("{request}");
            }
            Fetched::Failed(reason) => {
                eprintln!("flowleap mcp: could not load {}: {reason}", spec.key);
                doctrine.unloaded.push(spec.key.to_string());
            }
            Fetched::Loaded(data) => {
                let (resource, prompt) = build_doc(spec, &data);
                doctrine.resources.push(resource);
                match prompt {
                    Some(Ok(prompt)) => doctrine.prompts.push(prompt),
                    Some(Err(reason)) => {
                        eprintln!("flowleap mcp: could not load {}: {reason}", spec.key);
                        doctrine.unloaded.push(spec.key.to_string());
                    }
                    None => {}
                }
            }
        }
    }
    doctrine
}

enum Fetched {
    Loaded(Value),
    Failed(String),
    DryRun(String),
}

async fn fetch_doc(ctx: &Context, spec: &DocSpec) -> Fetched {
    let path = format!("/v1/patstat/docs?{}", spec.query);
    let envelope = match ctx.execute_json_envelope(ctx.get(&path)).await {
        Ok(envelope) => envelope,
        Err(err) => return Fetched::Failed(err.to_string()),
    };
    if envelope.get("dryRun").and_then(Value::as_bool) == Some(true) {
        return Fetched::DryRun(envelope.to_string());
    }
    if envelope.get("ok").and_then(Value::as_bool) != Some(true) {
        let status = envelope.get("status").and_then(Value::as_u64).unwrap_or(0);
        let code = envelope
            .pointer("/body/error/code")
            .and_then(Value::as_str)
            .map(|code| format!(" {code}"))
            .unwrap_or_default();
        return Fetched::Failed(format!("HTTP {status}{code}"));
    }
    let body = envelope.get("body").cloned().unwrap_or(Value::Null);
    Fetched::Loaded(body.get("data").cloned().unwrap_or(body))
}

/// Build the resource (and, for a workflow, the prompt) from one payload.
/// The resource text is the payload verbatim: the YAML string itself when
/// the document is YAML, else the `data` object as JSON.
fn build_doc(spec: &DocSpec, data: &Value) -> (Resource, Option<Result<Prompt, String>>) {
    let (text, mime_type) = match data.get("yaml").and_then(Value::as_str) {
        Some(yaml) => (yaml.to_string(), "application/yaml"),
        None => (
            serde_json::to_string_pretty(data).unwrap_or_else(|_| data.to_string()),
            "application/json",
        ),
    };
    let workflow = data.get("workflow").filter(|w| w.is_object());
    let payload_title = workflow.and_then(|w| non_empty_str(w, "name"));
    let payload_description = workflow.and_then(|w| non_empty_str(w, "description"));
    let title = payload_title.unwrap_or(spec.fallback_title).to_string();
    let description = match (payload_description, data.get("data_edition")) {
        (Some(description), _) => description.to_string(),
        (None, Some(Value::String(edition))) => {
            format!("{} Data edition: {edition}.", spec.fallback_description)
        }
        _ => spec.fallback_description.to_string(),
    };

    let prompt = spec.prompt.map(|name| match workflow {
        Some(workflow) => Ok(Prompt {
            name,
            title: title.clone(),
            description: description.clone(),
            text: render_workflow(workflow),
        }),
        None => Err("payload carries no workflow object".to_string()),
    });

    let resource = Resource {
        uri: format!("flowleap://patstat/{}", spec.key),
        name: format!("patstat-{}", spec.key.replace('/', "-")),
        title,
        description,
        mime_type,
        text,
    };
    (resource, prompt)
}

fn non_empty_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// A plain text rendering of a served workflow: name, description, then each
/// step's number, action, endpoint or tools, and note. Adds no words of its
/// own beyond the field labels.
fn render_workflow(workflow: &Value) -> String {
    let mut text = String::new();
    if let Some(name) = non_empty_str(workflow, "name") {
        text.push_str(name);
        text.push_str("\n\n");
    }
    if let Some(description) = non_empty_str(workflow, "description") {
        text.push_str(description);
        text.push_str("\n\n");
    }
    let steps = workflow
        .get("steps")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if !steps.is_empty() {
        text.push_str("Steps:\n");
    }
    for (index, step) in steps.iter().enumerate() {
        let number = step
            .get("step")
            .and_then(Value::as_u64)
            .unwrap_or(index as u64 + 1);
        let action = non_empty_str(step, "action").unwrap_or("");
        text.push_str(&format!("{number}. {action}\n"));
        if let Some(endpoint) = non_empty_str(step, "endpoint") {
            text.push_str(&format!("   Endpoint: {endpoint}\n"));
        }
        let tools: Vec<&str> = match step.get("tools") {
            Some(Value::Array(tools)) => tools.iter().filter_map(Value::as_str).collect(),
            Some(Value::String(tool)) => vec![tool.as_str()],
            _ => Vec::new(),
        };
        if !tools.is_empty() {
            text.push_str(&format!("   Tools: {}\n", tools.join(", ")));
        }
        if let Some(note) = non_empty_str(step, "note") {
            text.push_str(&format!("   Note: {note}\n"));
        }
    }
    text
}

/// resources/* and prompts/* against the loaded doctrine.
fn doctrine_request(doctrine: &Doctrine, method: &str, id: Value, params: &Value) -> Value {
    match method {
        "resources/list" => {
            let resources: Vec<Value> = doctrine
                .resources
                .iter()
                .map(|r| {
                    json!({
                        "uri": r.uri,
                        "name": r.name,
                        "title": r.title,
                        "description": r.description,
                        "mimeType": r.mime_type,
                    })
                })
                .collect();
            result_response(id, json!({ "resources": resources }))
        }
        "resources/templates/list" => result_response(id, json!({ "resourceTemplates": [] })),
        "resources/read" => {
            let Some(uri) = params.get("uri").and_then(Value::as_str) else {
                return error_response(id, -32602, "Invalid params: missing resource 'uri'", None);
            };
            match doctrine.resources.iter().find(|r| r.uri == uri) {
                Some(r) => result_response(
                    id,
                    json!({
                        "contents": [{ "uri": r.uri, "mimeType": r.mime_type, "text": r.text }],
                    }),
                ),
                None => error_response(
                    id,
                    -32602,
                    &format!("Unknown resource: {uri}"),
                    Some(json!({ "uri": uri })),
                ),
            }
        }
        "prompts/list" => {
            let prompts: Vec<Value> = doctrine
                .prompts
                .iter()
                .map(|p| {
                    json!({
                        "name": p.name,
                        "title": p.title,
                        "description": p.description,
                        "arguments": [],
                    })
                })
                .collect();
            result_response(id, json!({ "prompts": prompts }))
        }
        "prompts/get" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return error_response(id, -32602, "Invalid params: missing prompt 'name'", None);
            };
            match doctrine.prompts.iter().find(|p| p.name == name) {
                Some(p) => result_response(
                    id,
                    json!({
                        "description": p.description,
                        "messages": [{
                            "role": "user",
                            "content": { "type": "text", "text": p.text },
                        }],
                    }),
                ),
                None => error_response(id, -32602, &format!("Unknown prompt: {name}"), None),
            }
        }
        _ => error_response(id, -32601, &format!("Method not found: {method}"), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_workflow_numbers_steps_from_the_payload_or_position() {
        let workflow = json!({
            "name": "W",
            "steps": [
                { "action": "first", "note": "n1" },
                { "step": 7, "action": "seventh", "tools": "only_tool" },
            ],
        });
        assert_eq!(
            render_workflow(&workflow),
            "W\n\nSteps:\n1. first\n   Note: n1\n7. seventh\n   Tools: only_tool\n"
        );
    }

    #[test]
    fn yaml_payload_is_served_raw_and_edition_reaches_the_description() {
        let data = json!({ "data_edition": "E1", "yaml": "a: 1\n" });
        let (resource, prompt) = build_doc(&DOC_SPECS[0], &data);
        assert_eq!(resource.text, "a: 1\n");
        assert_eq!(resource.mime_type, "application/yaml");
        assert_eq!(resource.title, "PATSTAT semantic model");
        assert!(resource.description.ends_with("Data edition: E1."));
        assert!(prompt.is_none());
    }

    #[test]
    fn workflow_without_workflow_object_serves_the_resource_but_no_prompt() {
        let (resource, prompt) = build_doc(&DOC_SPECS[4], &json!({ "other": true }));
        assert_eq!(resource.uri, "flowleap://patstat/workflow/graph");
        assert_eq!(resource.name, "patstat-workflow-graph");
        assert_eq!(resource.mime_type, "application/json");
        assert!(matches!(prompt, Some(Err(_))));
    }
}
