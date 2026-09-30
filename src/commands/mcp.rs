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
//! through the `patstat_docs` tool (the semantic model, the verified examples
//! and three workflows) and serves them verbatim as MCP resources, plus one
//! MCP prompt per workflow. The tool, not the plan-gated docs route: it is
//! free at sign-in (backend ADR 0021), so every signed-in user gets them. The bridge authors no doctrine: a prompt is a plain
//! text rendering of the served workflow JSON. A document that fails to load
//! is logged to stderr and skipped; the rest keep serving.

use anyhow::Result;
use clap::Parser;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use tokio::sync::{Notify, OnceCell};

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

    // The doctrine loads once, at startup, concurrently with serving. No
    // frame waits for it: initialize, ping and tools answer at once, and a
    // resources/prompts request that arrives before the load completes is
    // queued and answered when it does. The loaded set is kept for the life
    // of the process — a document that failed (or an unauthenticated start)
    // stays missing until the server is restarted.
    let doctrine = OnceCell::new();
    let loaded = Notify::new();
    let preload = async {
        let doctrine_loaded = load_doctrine(ctx).await;
        eprintln!(
            "flowleap mcp: serving {} resources and {} prompts",
            doctrine_loaded.resources.len(),
            doctrine_loaded.prompts.len()
        );
        let _ = doctrine.set(doctrine_loaded);
        // A stored permit: the serve loop sees it even if not yet waiting.
        loaded.notify_one();
    };
    let serving = serve(ctx, &doctrine, &loaded);
    tokio::pin!(preload, serving);
    // Stdin EOF before the load completes ends the server without waiting.
    tokio::select! {
        served = &mut serving => return served,
        () = &mut preload => {}
    }
    serving.await
}

/// A doctrine request queued while the load is in flight.
struct Deferred {
    id: Value,
    params: Value,
    handler: DoctrineHandler,
}

/// What one inbound line produces.
enum Handled {
    Respond(Value),
    Defer(Deferred),
    /// A notification: no frame goes back.
    Silent,
}

async fn serve(ctx: &Context, doctrine: &OnceCell<Doctrine>, loaded: &Notify) -> Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    let mut pending: Vec<Deferred> = Vec::new();
    let mut awaiting_load = true;

    loop {
        tokio::select! {
            () = loaded.notified(), if awaiting_load => {
                awaiting_load = false;
                if let Some(doctrine) = doctrine.get() {
                    for deferred in pending.drain(..) {
                        let response = (deferred.handler)(doctrine, deferred.id, &deferred.params);
                        write_frame(&mut stdout, &response).await?;
                    }
                }
            }
            line = lines.next_line() => {
                let Some(line) = line? else { break };
                if line.trim().is_empty() {
                    continue;
                }
                match handle_line(ctx, doctrine.get(), &line).await {
                    Handled::Respond(response) => write_frame(&mut stdout, &response).await?,
                    Handled::Defer(deferred) => pending.push(deferred),
                    Handled::Silent => {}
                }
            }
        }
    }
    // Stdin closed with requests still queued: answer them once loaded.
    if !pending.is_empty() {
        if awaiting_load {
            loaded.notified().await;
        }
        if let Some(doctrine) = doctrine.get() {
            for deferred in pending.drain(..) {
                let response = (deferred.handler)(doctrine, deferred.id, &deferred.params);
                write_frame(&mut stdout, &response).await?;
            }
        }
    }
    Ok(())
}

/// One JSON-RPC frame per line on stdout — the only writer of stdout.
async fn write_frame(stdout: &mut tokio::io::Stdout, response: &Value) -> Result<()> {
    let mut frame = serde_json::to_string(response)?;
    frame.push('\n');
    stdout.write_all(frame.as_bytes()).await?;
    stdout.flush().await?;
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
                "unavailablePrompts": doctrine.as_ref().map(|d| d.unavailable_prompts.clone()),
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
                let resources = doctrine.resources.len();
                if doctrine.unloaded.is_empty() {
                    println!("  resources ok   {resources} served");
                } else {
                    let unloaded = doctrine.unloaded.join(", ");
                    println!("  resources warn {resources} served (could not load: {unloaded})");
                }
                let prompts = doctrine.prompts.len();
                if doctrine.unavailable_prompts.is_empty() {
                    println!("  prompts   ok   {prompts} served");
                } else {
                    let unavailable = doctrine.unavailable_prompts.join(", ");
                    println!("  prompts   warn {prompts} served (unavailable: {unavailable})");
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
async fn handle_line(ctx: &Context, doctrine: Option<&Doctrine>, line: &str) -> Handled {
    let message: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(err) => {
            return Handled::Respond(error_response(
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
    let Some(id) = message.get("id").cloned() else {
        return Handled::Silent;
    };
    let params = message.get("params").cloned().unwrap_or(Value::Null);

    let doctrine_frame = |handler: DoctrineHandler| {
        if ctx.credentials.auth_header().is_none() {
            return Handled::Respond(error_response(
                id.clone(),
                -32002,
                AUTH_REQUIRED_MESSAGE,
                None,
            ));
        }
        match doctrine {
            Some(doctrine) => Handled::Respond(handler(doctrine, id.clone(), &params)),
            None => Handled::Defer(Deferred {
                id: id.clone(),
                params: params.clone(),
                handler,
            }),
        }
    };

    match method.as_str() {
        "initialize" => Handled::Respond(result_response(id, initialize_result(&params))),
        "ping" => Handled::Respond(result_response(id, json!({}))),
        "tools/list" => Handled::Respond(tools_list(ctx, id).await),
        "tools/call" => Handled::Respond(tools_call(ctx, id, &params).await),
        "resources/list" => doctrine_frame(resources_list),
        "resources/templates/list" => doctrine_frame(resource_templates_list),
        "resources/read" => doctrine_frame(resources_read),
        "prompts/list" => doctrine_frame(prompts_list),
        "prompts/get" => doctrine_frame(prompts_get),
        _ => Handled::Respond(error_response(
            id,
            -32601,
            &format!("Method not found: {method}"),
            None,
        )),
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
    let Some(body) = ok_body(&envelope) else {
        let status = envelope_status(&envelope);
        return error_response(
            id,
            -32002,
            &format!("FlowLeap backend rejected tools/list (HTTP {status})"),
            Some(envelope),
        );
    };
    let tools = body
        .get("tools")
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

    if let Some(body) = ok_body(&envelope) {
        // Backend tool envelope: { success, tool, data, executionTimeMs } —
        // surface `data` (like `flowleap tools run`), whole body otherwise.
        return text_result(id, false, &data_payload(body));
    }
    // The envelope already carries status, body, and any structured hints
    // (providerKeysHint, retryAfterSeconds) — pass it through whole.
    tool_error(id, envelope)
}

/// The response body of a 2xx client envelope; `None` for any other status.
fn ok_body(envelope: &Value) -> Option<Value> {
    (envelope.get("ok").and_then(Value::as_bool) == Some(true))
        .then(|| envelope.get("body").cloned().unwrap_or(Value::Null))
}

fn envelope_status(envelope: &Value) -> u64 {
    envelope.get("status").and_then(Value::as_u64).unwrap_or(0)
}

/// A backend `{ success, data, … }` body's `data`, else the whole body.
fn data_payload(body: Value) -> Value {
    match body.get("data") {
        Some(data) => data.clone(),
        None => body,
    }
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

/// A successful (or `isError`) MCP tool result with one compact-JSON text
/// block. Compact, not pretty: indentation added about a third to large
/// replies and pushed them past the client's tool-output limit (#104).
fn text_result(id: Value, is_error: bool, payload: &Value) -> Value {
    let text = payload.to_string();
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
    /// The `patstat_docs` selector: one `(field, value)` input pair.
    selector: (&'static str, &'static str),
    /// Workflow documents also become a prompt of this name.
    prompt: Option<&'static str>,
    /// Neutral labels used when the payload carries no name or description.
    fallback_title: &'static str,
    fallback_description: &'static str,
}

const DOC_SPECS: [DocSpec; 5] = [
    DocSpec {
        key: "semantic-model",
        selector: ("section", "semantic-model"),
        prompt: None,
        fallback_title: "PATSTAT semantic model",
        fallback_description: "The PATSTAT semantic model, as served by the FlowLeap backend.",
    },
    DocSpec {
        key: "examples",
        selector: ("section", "examples"),
        prompt: None,
        fallback_title: "PATSTAT examples",
        fallback_description: "The PATSTAT verified examples, as served by the FlowLeap backend.",
    },
    DocSpec {
        key: "workflow/portfolio-analysis",
        selector: ("workflow", "portfolio-analysis"),
        prompt: Some("patstat-portfolio-analysis"),
        fallback_title: "PATSTAT workflow: portfolio-analysis",
        fallback_description:
            "The PATSTAT portfolio-analysis workflow, as served by the FlowLeap backend.",
    },
    DocSpec {
        key: "workflow/guarded-sql",
        selector: ("workflow", "guarded-sql"),
        prompt: Some("patstat-guarded-sql"),
        fallback_title: "PATSTAT workflow: guarded-sql",
        fallback_description:
            "The PATSTAT guarded-sql workflow, as served by the FlowLeap backend.",
    },
    DocSpec {
        key: "workflow/graph",
        selector: ("workflow", "graph"),
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
    /// Keys of the documents whose resource could not be loaded, in spec order.
    unloaded: Vec<String>,
    /// Names of the prompts that could not be built, in spec order: the
    /// document failed, or it loaded but carries no workflow object.
    unavailable_prompts: Vec<String>,
}

/// Answers one resources/* or prompts/* request from the loaded doctrine.
type DoctrineHandler = fn(&Doctrine, Value, &Value) -> Value;

/// Fetch the five documents concurrently (a fixed set of five small tool
/// calls is its own bound) through the shared tool seam, so credentials, base
/// URL, dry-run, redaction and the generic retry all apply. Failures are logged and skipped.
async fn load_doctrine(ctx: &Context) -> Doctrine {
    let mut doctrine = Doctrine::default();
    if ctx.credentials.auth_header().is_none() && !ctx.dry_run {
        doctrine.unloaded = DOC_SPECS.iter().map(|s| s.key.to_string()).collect();
        doctrine.unavailable_prompts = DOC_SPECS
            .iter()
            .filter_map(|s| s.prompt.map(str::to_string))
            .collect();
        eprintln!(
            "flowleap mcp: no credentials — serving no PATSTAT resources or prompts \
             (run 'flowleap auth login', then restart the server)"
        );
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
                log_unloaded(spec.key, &reason);
                doctrine.unloaded.push(spec.key.to_string());
                if let Some(name) = spec.prompt {
                    doctrine.unavailable_prompts.push(name.to_string());
                }
            }
            Fetched::Loaded(data) => {
                let (resource, prompt) = build_doc(spec, &data);
                doctrine.resources.push(resource);
                match prompt {
                    Some(Ok(prompt)) => doctrine.prompts.push(prompt),
                    Some(Err((name, reason))) => {
                        log_unloaded(&format!("prompt {name}"), &reason);
                        doctrine.unavailable_prompts.push(name.to_string());
                    }
                    None => {}
                }
            }
        }
    }
    doctrine
}

fn log_unloaded(what: &str, reason: &str) {
    eprintln!("flowleap mcp: could not load {what}: {reason} (restart the server to retry)");
}

enum Fetched {
    Loaded(Value),
    Failed(String),
    DryRun(String),
}

async fn fetch_doc(ctx: &Context, spec: &DocSpec) -> Fetched {
    let (field, value) = spec.selector;
    let input = json!({ field: value });
    let envelope = match tools::call_tool_envelope(ctx, "patstat_docs", &input).await {
        Ok(envelope) => envelope,
        Err(err) => return Fetched::Failed(err.to_string()),
    };
    if envelope.get("dryRun").and_then(Value::as_bool) == Some(true) {
        return Fetched::DryRun(envelope.to_string());
    }
    let Some(body) = ok_body(&envelope) else {
        let status = envelope_status(&envelope);
        let code = envelope
            .pointer("/body/error/code")
            .and_then(Value::as_str)
            .map(|code| format!(" {code}"))
            .unwrap_or_default();
        return Fetched::Failed(format!("HTTP {status}{code}"));
    };
    Fetched::Loaded(data_payload(body))
}

/// Build the resource (and, for a workflow, the prompt) from one payload.
/// The resource text is the payload verbatim: the YAML string itself when
/// the document is YAML, else the `data` object as JSON.
/// A prompt that could not be built: its name and the reason.
type PromptError = (&'static str, String);

fn build_doc(spec: &DocSpec, data: &Value) -> (Resource, Option<Result<Prompt, PromptError>>) {
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
        None => Err((name, "payload carries no workflow object".to_string())),
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
///
/// It renders only the workflow's `name`, `description` and `steps`, and of
/// each step only `step`, `action`, `endpoint`, `tools` and `note`; every
/// other field is omitted — `curlExample` by decision, since an MCP client
/// calls tools, not raw HTTP routes. The full payload stays available,
/// verbatim, as the workflow resource. A `tools` value is a string or an
/// array of strings; non-string array items are dropped.
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

fn resources_list(doctrine: &Doctrine, id: Value, _params: &Value) -> Value {
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

fn resource_templates_list(_doctrine: &Doctrine, id: Value, _params: &Value) -> Value {
    result_response(id, json!({ "resourceTemplates": [] }))
}

fn resources_read(doctrine: &Doctrine, id: Value, params: &Value) -> Value {
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

fn prompts_list(doctrine: &Doctrine, id: Value, _params: &Value) -> Value {
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

fn prompts_get(doctrine: &Doctrine, id: Value, params: &Value) -> Value {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn result_text(response: &Value) -> &str {
        response["result"]["content"][0]["text"]
            .as_str()
            .expect("text block")
    }

    /// Fifty nested records, shaped like a tool reply (a portfolio row list).
    fn nested_fixture() -> Value {
        let rows: Vec<Value> = (0..50)
            .map(|i| {
                json!({
                    "applicant": { "name": format!("Applicant {i}"), "country": "DE" },
                    "counts": { "families": i, "applications": i * 2, "granted": i / 2 },
                    "cpc": [{ "code": "H04L", "share": 0.5 }, { "code": "G06F", "share": 0.25 }],
                })
            })
            .collect();
        json!({ "rows": rows, "meta": { "returned": 50, "edition": "2026 Spring" } })
    }

    #[test]
    fn tool_result_text_is_compact_json_with_no_newline_or_double_space() {
        let payload = nested_fixture();
        for is_error in [false, true] {
            let response = text_result(json!(1), is_error, &payload);
            let text = result_text(&response);
            assert!(!text.contains('\n'), "no newline in tool text");
            assert!(!text.contains("  "), "no double space in tool text");
            assert_eq!(serde_json::from_str::<Value>(text).unwrap(), payload);
        }
    }

    #[test]
    fn compact_text_is_at_least_a_quarter_shorter_than_pretty_for_nested_replies() {
        // Documents why #104 dropped pretty-printing: indentation alone
        // pushed large replies past the client's MCP tool-output limit.
        let payload = nested_fixture();
        let compact = result_text(&text_result(json!(1), false, &payload)).len();
        let pretty = serde_json::to_string_pretty(&payload).unwrap().len();
        eprintln!("nested fixture: pretty {pretty} chars, compact {compact} chars");
        assert!(
            compact * 4 <= pretty * 3,
            "compact {compact} must be <= 75% of pretty {pretty}"
        );
    }

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
