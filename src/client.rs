use std::time::Duration;

use anyhow::{bail, Result};
use reqwest::{Client, Method, Request, RequestBuilder, Response, StatusCode};
use serde_json::{json, Value};

use crate::config::{Config, Credentials};

/// Versioned User-Agent sent on every request so the backend can version-gate
/// and debug client issues.
const USER_AGENT: &str = concat!("flowleap-cli/", env!("CARGO_PKG_VERSION"));

/// The client-identification header every request carries (backend ADR 0014
/// rule 6). `product/semver` — the backend logs it so retirement decisions rest
/// on usage evidence. Observational only: no request is ever rejected on it.
const CLIENT_HEADER: &str = "X-FlowLeap-Client";
const CLIENT_HEADER_VALUE: &str = concat!("cli/", env!("CARGO_PKG_VERSION"));

const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 5;
/// Retries beyond the first attempt for transient failures on read-only calls.
const DEFAULT_MAX_RETRIES: u32 = 2;
/// A 429 is only retried in-band when the server asks us to wait at most this
/// long; longer waits pass through to the envelope so the caller decides.
const RETRY_AFTER_MAX_SECS: u64 = 5;

/// Build the one shared HTTP client every command uses. Applies a bounded
/// request/connect timeout (so a hung backend can never block a caller forever)
/// and the versioned User-Agent. Both timeouts are overridable via env for
/// slow-network or CI use; a non-positive or unparseable value falls back to the
/// default rather than disabling the timeout.
pub fn build_http_client() -> Client {
    let timeout = env_secs("FLOWLEAP_TIMEOUT_SECS", DEFAULT_TIMEOUT_SECS);
    let connect = env_secs(
        "FLOWLEAP_CONNECT_TIMEOUT_SECS",
        DEFAULT_CONNECT_TIMEOUT_SECS,
    );
    let mut builder = Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(timeout))
        .connect_timeout(Duration::from_secs(connect));
    // Test-only DNS pin: FLOWLEAP_TEST_RESOLVE="host=ip:port[,host=ip:port]"
    // points hostnames at a local mock server so integration tests can exercise
    // host-based behavior (e.g. the base-URL credential guard) end-to-end. It
    // only overrides address resolution — the original hostname is still what
    // gets classified and sent in the Host header.
    if let Ok(pins) = std::env::var("FLOWLEAP_TEST_RESOLVE") {
        for pin in pins.split(',') {
            if let Some((host, addr)) = pin.split_once('=') {
                if let Ok(addr) = addr.trim().parse::<std::net::SocketAddr>() {
                    builder = builder.resolve(host.trim(), addr);
                }
            }
        }
    }
    builder.build().unwrap_or_else(|_| Client::new())
}

/// Read a positive whole-seconds value from `name`, falling back to `default`
/// for anything missing, unparseable, or non-positive (never disables a timeout).
fn env_secs(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|&secs| secs > 0)
        .unwrap_or(default)
}

fn max_retries() -> u32 {
    std::env::var("FLOWLEAP_MAX_RETRIES")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(DEFAULT_MAX_RETRIES)
}

/// Only connection-level failures (refused/reset/unreachable/DNS) are retried.
/// A request timeout means a stalled server and surfaces immediately as a clear
/// error rather than being multiplied into a longer hang.
fn is_retryable_transport(err: &reqwest::Error) -> bool {
    err.is_connect()
}

/// Exponential backoff (250ms, 500ms, …) plus a little jitter, derived from the
/// clock so we avoid pulling in an RNG dependency.
fn backoff(attempt: u32) -> Duration {
    let base = 250u64.saturating_mul(1u64 << attempt.min(6));
    Duration::from_millis(base + jitter_ms())
}

fn jitter_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| (elapsed.subsec_nanos() % 128) as u64)
        .unwrap_or(0)
}

/// Turn a low-level transport failure into a clear, actionable message.
/// reqwest's own Display for a timeout is just "error sending request for url
/// (…)" — the "operation timed out" lives in the source chain, which the
/// top-level error renderer never shows. Spell it out so a caller (human or
/// agent) sees a timeout as a timeout, not a vague send failure. Timeouts and
/// connection failures come back as [`NetworkError`] so the top-level handler
/// maps them to the network exit code.
fn clarify_transport_error(err: reqwest::Error) -> anyhow::Error {
    let url = err
        .url()
        .map(|url| url.as_str().to_string())
        .unwrap_or_default();
    if err.is_timeout() {
        NetworkError::new(format!(
            "Request timed out contacting the FlowLeap backend ({url}). The server did not \
             respond in time; check connectivity or raise FLOWLEAP_TIMEOUT_SECS. ({err})"
        ))
        .into()
    } else if err.is_connect() {
        NetworkError::new(format!(
            "Could not connect to the FlowLeap backend ({url}). Check the base URL and your \
             network. ({err})"
        ))
        .into()
    } else {
        anyhow::Error::new(err)
    }
}

/// The short, bounded delay a 429 asks us to wait, if any. `None` when there is
/// no Retry-After, it is unparseable, or it exceeds `RETRY_AFTER_MAX_SECS`.
fn retry_after_short(resp: &Response) -> Option<Duration> {
    let secs = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    (secs <= RETRY_AFTER_MAX_SECS).then(|| Duration::from_secs(secs))
}

/// Exit codes for the documented contract (see AGENTS.md "Exit Codes").
/// 0 = success, 1 = generic failure, 2 = usage/parse error (clap); the rest
/// map HTTP status classes so agents can branch on `$?` without parsing JSON.
pub const EXIT_AUTH_REQUIRED: i32 = 3;
pub const EXIT_SUBSCRIPTION_REQUIRED: i32 = 4;
pub const EXIT_NOT_FOUND: i32 = 5;
pub const EXIT_RATE_LIMITED: i32 = 6;
pub const EXIT_NETWORK: i32 = 7;
/// HTTP 410 — the endpoint this build calls was retired. Distinct from a
/// generic failure so a stale CLI fails loudly with a migration path instead of
/// looking like any other error (backend ADR 0014 rule 3).
pub const EXIT_ENDPOINT_GONE: i32 = 8;

/// Map an HTTP status to its documented exit code; anything without a
/// dedicated code is a generic failure (1).
pub fn exit_code_for_status(status: u16) -> i32 {
    match status {
        401 => EXIT_AUTH_REQUIRED,
        402 => EXIT_SUBSCRIPTION_REQUIRED,
        404 => EXIT_NOT_FOUND,
        410 => EXIT_ENDPOINT_GONE,
        429 => EXIT_RATE_LIMITED,
        _ => 1,
    }
}

/// Map a failed run's error to its documented exit code by downcasting the
/// typed errors this module produces. Anything untyped is generic (1).
pub fn error_exit_code(err: &anyhow::Error) -> i32 {
    if let Some(printed) = err.downcast_ref::<PrintedError>() {
        printed.exit_code()
    } else if let Some(api) = err.downcast_ref::<ApiError>() {
        exit_code_for_status(api.status)
    } else if err.downcast_ref::<NetworkError>().is_some() {
        EXIT_NETWORK
    } else if err.downcast_ref::<SessionTokenRefusedError>().is_some() {
        EXIT_AUTH_REQUIRED
    } else {
        1
    }
}

/// An error whose details were already rendered (envelope on stdout, hint
/// boxes on stderr) — the top-level handler must exit without printing more.
/// Carries the HTTP status of the failed request, when there was one, so the
/// handler can map it to the documented exit code.
#[derive(Debug, Default)]
pub struct PrintedError {
    status: Option<u16>,
    code: Option<i32>,
}

impl PrintedError {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_status(status: u16) -> Self {
        Self {
            status: Some(status),
            code: None,
        }
    }

    /// An already-rendered error carrying a precomputed exit code — for paths
    /// (like structured device-flow login) that map their error via
    /// [`error_exit_code`] before rendering, so no code is invented.
    pub fn with_exit_code(code: i32) -> Self {
        Self {
            status: None,
            code: Some(code),
        }
    }

    pub fn exit_code(&self) -> i32 {
        if let Some(code) = self.code {
            return code;
        }
        self.status.map(exit_code_for_status).unwrap_or(1)
    }
}

impl std::fmt::Display for PrintedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "request failed")
    }
}

impl std::error::Error for PrintedError {}

/// A non-2xx API response on the paths that surface errors as messages rather
/// than envelopes. Typed so the top-level handler can map the status to the
/// documented exit code; Display keeps the historical "API error (…)" shape.
#[derive(Debug)]
pub struct ApiError {
    pub status: u16,
    message: String,
}

impl ApiError {
    fn from_response(status: StatusCode, body: &str) -> Self {
        Self {
            status: status.as_u16(),
            message: format!("API error ({}): {}", status, body),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ApiError {}

/// A transport-level failure (request timeout or connection failure) reaching
/// the backend. Typed so the top-level handler exits with the network code.
#[derive(Debug)]
pub struct NetworkError {
    message: String,
}

impl NetworkError {
    fn new(message: String) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for NetworkError {}

/// A session token whose own `exp` claim shows it's already dead, or nearly
/// so, at the moment the CLI would otherwise persist it — the store-time TTL
/// guard's refusal (flowleap-backend#254). Typed so the top-level handler
/// exits with the documented auth-required code: the run genuinely ends
/// unauthenticated, the same as if no token had come back at all.
#[derive(Debug)]
pub struct SessionTokenRefusedError {
    message: String,
}

impl SessionTokenRefusedError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for SessionTokenRefusedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SessionTokenRefusedError {}

/// Headers that must never appear in verbose or debug output.
const SECRET_HEADERS: &[&str] = &[
    "authorization",
    "x-api-key",
    "x-epo-ops-key",
    "x-epo-ops-secret",
    "x-uspto-odp-key",
];

fn is_secret_header(name: &reqwest::header::HeaderName) -> bool {
    SECRET_HEADERS.contains(&name.as_str())
}

/// The backend error code carried by an error body, wherever the unified
/// envelope puts it: `{ error: { code } }` (every `/v1` surface) or a
/// top-level `code` (the legacy OPS envelope, still emitted by the non-facade
/// exceptions — PATSTAT, key validation).
fn error_code(body: &Value) -> Option<&str> {
    body.pointer("/error/code")
        .or_else(|| body.get("code"))
        .and_then(Value::as_str)
}

/// Detect patent-data-key problems in an error response and produce a
/// structured, agent-parseable hint. Returns None when the error is unrelated
/// to keys.
///
/// Codes only (backend ADR 0014 rule 4): the closed error-code registry and the
/// structured `provider` field are the whole protocol. Message wording is
/// freely editable backend-side by policy, so matching on it — as this function
/// once did — made an innocent reword a silent breaking change.
///
/// - `patent_provider_key_invalid` (+ `provider`) → the user's keys were rejected
/// - `data_keys_required`          (+ `provider`) → no key for that office anywhere
/// - `odp_api_key_missing`                        → no USPTO ODP key, server-side
pub fn provider_keys_hint(status: u16, body: &Value) -> Option<Value> {
    if status < 400 {
        return None;
    }
    // The provider the backend named. Structured field only — never inferred
    // from the message, and never guessed from an office name appearing in it.
    let named_provider = body
        .pointer("/error/provider")
        .or_else(|| body.get("provider"))
        .and_then(Value::as_str);

    let (code, provider) = match error_code(body)? {
        "patent_provider_key_invalid" => ("provider_keys_invalid", named_provider?),
        "data_keys_required" => ("provider_keys_required", named_provider?),
        // The ODP taxonomy's own "no USPTO key configured" verdict: the code
        // itself names the office, so it needs no provider field.
        "odp_api_key_missing" => ("provider_keys_required", "uspto"),
        _ => return None,
    };

    Some(json!({
        "code": code,
        "provider": provider,
        "requiresHumanIntervention": true,
        "humanAction": "Run 'flowleap setup' (or 'flowleap keys set') in a terminal. Getting keys involves a browser signup, so an agent cannot complete this alone — ask the user.",
        "nonInteractive": {
            "command": if provider == "epo" {
                "flowleap keys set epo --key <consumer-key> --secret <consumer-secret>"
            } else {
                "flowleap keys set uspto --key <api-key>"
            },
            "env": if provider == "epo" {
                json!(["FLOWLEAP_EPO_KEY", "FLOWLEAP_EPO_SECRET"])
            } else {
                json!(["FLOWLEAP_USPTO_KEY"])
            },
        },
        "signup": if provider == "epo" {
            "https://developers.epo.org (free, 'My apps' → create app)"
        } else {
            "https://data.uspto.gov/apis/getting-started (free API key)"
        },
        "verify": "flowleap keys test",
    }))
}

/// Human-readable info box for a provider-keys hint, printed to stderr so it
/// never corrupts parseable stdout.
pub fn print_keys_hint_box(hint: &Value) {
    use colored::Colorize;
    let provider = hint["provider"].as_str().unwrap_or("provider");
    let invalid = hint["code"].as_str() == Some("provider_keys_invalid");
    let title = if invalid {
        format!("{} keys were rejected", provider.to_uppercase())
    } else {
        format!("{} keys required", provider.to_uppercase())
    };
    let signup = hint["signup"].as_str().unwrap_or("");
    let command = hint["nonInteractive"]["command"].as_str().unwrap_or("");

    eprintln!();
    eprintln!(
        "┌─ {} {}",
        title.yellow().bold(),
        "─".repeat(50_usize.saturating_sub(title.len()))
    );
    if invalid {
        eprintln!(
            "│ The backend rejected the configured {} credentials.",
            provider.to_uppercase()
        );
    } else {
        eprintln!(
            "│ This command needs {} credentials and none are configured",
            provider.to_uppercase()
        );
        eprintln!("│ (neither on this machine nor on the server).");
    }
    eprintln!("│");
    eprintln!("│ Fix it (requires a human — keys come from a browser signup):");
    eprintln!("│   {}   guided setup", "flowleap setup".cyan().bold());
    eprintln!("│   {}", command.cyan());
    eprintln!("│");
    eprintln!("│ Get keys: {}", signup);
    eprintln!("│ Verify:   {}", "flowleap keys test".cyan());
    eprintln!("└{}", "─".repeat(64));
}

/// The [`PrintedError`] for an already-rendered error envelope, carrying its
/// HTTP status (when present) for the exit-code mapping.
fn printed_error_for(envelope: &Value) -> PrintedError {
    match envelope.get("status").and_then(|value| value.as_u64()) {
        Some(status) => PrintedError::with_status(status as u16),
        None => PrintedError::new(),
    }
}

/// Fallback upgrade URL when a 402 response body carries none.
const DEFAULT_UPGRADE_URL: &str = "https://flowleap.co/pricing";

/// Structured, agent-parseable hint for a 402 subscription paywall. The
/// upgrade URL is taken from the response body when the backend sent one
/// (top-level or nested under `error`/`detail`), else the pricing page.
pub fn subscription_hint(body: &Value) -> Value {
    let upgrade_url = body
        .get("upgradeUrl")
        .or_else(|| body.pointer("/error/upgradeUrl"))
        .or_else(|| body.pointer("/detail/upgradeUrl"))
        .and_then(|value| value.as_str())
        .unwrap_or(DEFAULT_UPGRADE_URL);
    json!({
        "requiresHumanIntervention": true,
        "plan": "Basic",
        "upgradeUrl": upgrade_url,
        "message": format!(
            "This command requires an active FlowLeap subscription (Basic plan or higher). \
             Subscribing happens in a browser, so an agent cannot complete this alone — ask \
             the user to subscribe at {upgrade_url}, then retry."
        ),
    })
}

/// Structured, agent-parseable hint for a 429 rate limit. Carries the
/// Retry-After the envelope already captured, when the server sent one.
pub fn rate_limit_hint(retry_after_seconds: Option<&Value>) -> Value {
    let mut hint = json!({
        "message": match retry_after_seconds.and_then(|value| value.as_u64()) {
            Some(secs) => format!(
                "Rate limited: the per-user request limit was hit. Wait {secs} seconds, then retry."
            ),
            None => "Rate limited: the per-user request limit was hit. Back off before retrying."
                .to_string(),
        },
    });
    if let Some(secs) = retry_after_seconds {
        hint["retryAfterSeconds"] = secs.clone();
    }
    hint
}

/// Structured hint for a `410 endpoint_gone` body: this build called an
/// endpoint the backend has retired for good. Driven by the code alone; the
/// backend's own `successor` and `reason` fields are relayed verbatim so the
/// caller learns exactly what to call instead (backend ADR 0014 rule 3).
pub fn endpoint_gone_hint(body: &Value) -> Option<Value> {
    if error_code(body)? != "endpoint_gone" {
        return None;
    }
    let mut hint = json!({
        "code": "endpoint_gone",
        "requiresUpgrade": true,
        "message": "This FlowLeap build called a backend endpoint that has been retired. \
                    Upgrade the CLI ('flowleap upgrade'); if the failure persists, the \
                    command needs migrating to the successor named below.",
    });
    for field in ["successor", "reason"] {
        if let Some(value) = body.pointer(&format!("/error/{field}")) {
            hint[field] = value.clone();
        }
    }
    if let Some(message) = body.pointer("/error/message") {
        hint["serverMessage"] = message.clone();
    }
    Some(hint)
}

/// Human-readable info box for a retired endpoint, printed to stderr so it
/// never corrupts parseable stdout.
pub fn print_endpoint_gone_box(hint: &Value) {
    use colored::Colorize;
    let title = "Endpoint retired";

    eprintln!();
    eprintln!(
        "┌─ {} {}",
        title.yellow().bold(),
        "─".repeat(50_usize.saturating_sub(title.len()))
    );
    eprintln!("│ The backend answered 410: this endpoint is gone for good.");
    if let Some(reason) = hint["reason"].as_str() {
        eprintln!("│ Why: {reason}");
    }
    match hint["successor"].as_str() {
        Some(successor) => eprintln!("│ Use instead: {}", successor.cyan().bold()),
        None => eprintln!("│ No successor was named — the capability was withdrawn."),
    }
    eprintln!("│");
    eprintln!("│ Update this CLI: {}", "flowleap upgrade".cyan().bold());
    eprintln!("└{}", "─".repeat(64));
}

/// Human-readable info box for the 402 subscription hint, printed to stderr
/// so it never corrupts parseable stdout.
pub fn print_subscription_hint_box(hint: &Value) {
    use colored::Colorize;
    let title = "FlowLeap subscription required";
    let plan = hint["plan"].as_str().unwrap_or("Basic");
    let upgrade_url = hint["upgradeUrl"].as_str().unwrap_or(DEFAULT_UPGRADE_URL);

    eprintln!();
    eprintln!(
        "┌─ {} {}",
        title.yellow().bold(),
        "─".repeat(50_usize.saturating_sub(title.len()))
    );
    eprintln!(
        "│ This command needs an active FlowLeap subscription ({} plan",
        plan
    );
    eprintln!("│ or higher); the backend answered 402.");
    eprintln!("│");
    eprintln!("│ Subscribe: {}", upgrade_url.cyan().bold());
    eprintln!("│ Then re-run this command.");
    eprintln!("└{}", "─".repeat(64));
}

/// Human-readable info box for the 429 rate-limit hint, printed to stderr so
/// it never corrupts parseable stdout.
pub fn print_rate_limit_hint_box(hint: &Value) {
    use colored::Colorize;
    let title = "Rate limited";

    eprintln!();
    eprintln!(
        "┌─ {} {}",
        title.yellow().bold(),
        "─".repeat(50_usize.saturating_sub(title.len()))
    );
    eprintln!("│ The backend answered 429: the per-user request limit was hit.");
    match hint["retryAfterSeconds"].as_u64() {
        Some(secs) => eprintln!(
            "│ Retry in {} — the server asked for that wait.",
            format!("{} seconds", secs).cyan().bold()
        ),
        None => eprintln!("│ Back off briefly, then retry."),
    }
    eprintln!("└{}", "─".repeat(64));
}

/// Where the effective credential comes from, in the same precedence order
/// [`Credentials::auth_header`] resolves it: environment first, then stored
/// config. Shared by `auth status` and `doctor` so the two can never name the
/// same credential differently.
pub fn credential_source(ctx: &Context) -> &'static str {
    if std::env::var("FLOWLEAP_TOKEN").is_ok() {
        "env-token"
    } else if std::env::var("FLOWLEAP_API_KEY").is_ok() {
        "env-api-key"
    } else if ctx.credentials.token.is_some() {
        "config-token"
    } else if ctx.credentials.api_key.is_some() {
        "config-api-key"
    } else {
        "missing"
    }
}

pub fn encode_url_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    encoded
}

pub struct Context {
    pub config: Config,
    pub credentials: Credentials,
    pub output_format: String,
    pub dry_run: bool,
    pub dry_run_redacted: bool,
    pub verbose: bool,
    /// True when the session token came from --token / FLOWLEAP_TOKEN rather
    /// than the credential store. An explicit token expresses intent, so the
    /// 401 → api_key fallback is disabled.
    pub token_overridden: bool,
    /// Skip interactive confirmation prompts (--yes; FLOWLEAP_ASSUME_YES also
    /// works). Warnings still print.
    pub assume_yes: bool,
    pub http: Client,
}

impl Context {
    fn client(&self) -> &Client {
        &self.http
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url.trim_end_matches('/'), path)
    }

    /// Auth, forwarded patent-data keys, and the client-identification header —
    /// applied to every request the CLI builds.
    fn apply_headers(&self, req: RequestBuilder) -> RequestBuilder {
        let req = req.header(CLIENT_HEADER, CLIENT_HEADER_VALUE);

        // The backend accepts exactly one credential shape: `Authorization:
        // Bearer <token>` — a Clerk JWT or a personal API token (fl_pat_…).
        // There is no X-API-Key path server-side.
        let req = if let Some(ref token) = self.credentials.token {
            req.header("Authorization", format!("Bearer {}", token))
        } else if let Some(ref key) = self.credentials.api_key {
            req.header("Authorization", format!("Bearer {}", key))
        } else {
            req
        };

        // BYOK provider keys, forwarded per-request. The EPO pair only travels
        // complete — the backend 400s on half a pair.
        let req = if let Some((key, secret)) = self.credentials.epo_pair() {
            req.header("x-epo-ops-key", key)
                .header("x-epo-ops-secret", secret)
        } else {
            req
        };
        if let Some(ref uspto) = self.credentials.uspto_key {
            req.header("x-uspto-odp-key", uspto)
        } else {
            req
        }
    }

    /// Build a GET request with auth
    pub fn get(&self, path: &str) -> RequestBuilder {
        let req = self.client().get(self.url(path));
        self.apply_headers(req)
    }

    /// Build a POST request with auth and JSON body
    pub fn post(&self, path: &str, body: &Value) -> RequestBuilder {
        let req = self.client().post(self.url(path)).json(body);
        self.apply_headers(req)
    }

    /// Build an arbitrary request with optional JSON body.
    pub fn request(&self, method: Method, path: &str, body: Option<&Value>) -> RequestBuilder {
        let req = self.client().request(method, self.url(path));
        let req = if let Some(body) = body {
            req.json(body)
        } else {
            req
        };
        self.apply_headers(req)
    }

    /// Base-URL credential guard: before anything leaves the process, warn
    /// (and, in an interactive terminal, confirm) when the destination host is
    /// not a FlowLeap or local-development host. Runs at most once per
    /// invocation — see `crate::url_guard`.
    fn guard_credentials(&self, req: &Request) -> Result<()> {
        let Some(host) = req.url().host_str() else {
            return Ok(());
        };
        crate::url_guard::enforce(
            host,
            &self.credentials,
            self.output_format == "json",
            self.dry_run,
            self.assume_yes,
        )
    }

    /// The API key to retry with when the stored session token is rejected
    /// with a 401. None when there is nothing to fall back to: no session
    /// token in play, no stored api_key, or the token was passed explicitly
    /// (--token / FLOWLEAP_TOKEN).
    pub fn auth_fallback_key(&self) -> Option<&str> {
        if self.token_overridden || self.credentials.token.is_none() {
            return None;
        }
        self.credentials.api_key.as_deref()
    }

    /// Execute a request with bounded, jittered retry on transient failures.
    /// Every FlowLeap endpoint the CLI calls is read-only or idempotent, so a
    /// resend is safe. Retries: 5xx (server transient), connection-level errors
    /// (refused/reset/unreachable), and a 429 whose Retry-After is short. A
    /// request timeout and any 4xx are never retried. A non-cloneable body
    /// (streaming) is attempted exactly once.
    async fn send_retrying(&self, req: Request) -> Result<Response> {
        let max = max_retries();
        let mut attempt: u32 = 0;
        loop {
            let Some(this_attempt) = req.try_clone() else {
                return self
                    .client()
                    .execute(req)
                    .await
                    .map_err(clarify_transport_error);
            };
            let outcome = self.client().execute(this_attempt).await;

            let wait = match &outcome {
                Ok(resp) if resp.status() == StatusCode::TOO_MANY_REQUESTS => {
                    retry_after_short(resp)
                }
                Ok(resp) if resp.status().is_server_error() => Some(backoff(attempt)),
                Ok(_) => None,
                Err(err) if is_retryable_transport(err) => Some(backoff(attempt)),
                Err(_) => None,
            };

            match wait {
                Some(delay) if attempt < max => {
                    if self.verbose {
                        eprintln!(
                            "  transient failure — retry {}/{} in {}ms",
                            attempt + 1,
                            max,
                            delay.as_millis()
                        );
                    }
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
                _ => return outcome.map_err(clarify_transport_error),
            }
        }
    }

    /// Send a request; on 401 with a stored session token, retry once with
    /// the stored API key. A Clerk session token expires quickly and would
    /// otherwise shadow a still-valid fl_pat_ key (see auth_header precedence).
    async fn send_with_auth_fallback(&self, req: Request) -> Result<Response> {
        let retry = match self.auth_fallback_key() {
            Some(key) if req.headers().contains_key(reqwest::header::AUTHORIZATION) => {
                req.try_clone().and_then(|mut r| {
                    reqwest::header::HeaderValue::from_str(&format!("Bearer {}", key))
                        .ok()
                        .map(|v| {
                            r.headers_mut().insert(reqwest::header::AUTHORIZATION, v);
                            r
                        })
                })
            }
            _ => None,
        };

        let resp = self.send_retrying(req).await?;
        if resp.status() != reqwest::StatusCode::UNAUTHORIZED {
            return Ok(resp);
        }
        let Some(retry) = retry else { return Ok(resp) };

        if self.verbose {
            eprintln!("  401 with session token — retrying with stored API key");
        }
        let retry_resp = self.send_retrying(retry).await?;
        if retry_resp.status().is_success() {
            eprintln!(
                "warning: the stored session token was rejected (401); the request succeeded with the stored API key. \
                 Clear the stale session with 'flowleap auth logout --session-only'."
            );
        }
        Ok(retry_resp)
    }

    /// Execute a request, handling dry-run and verbose modes
    pub async fn execute(&self, req: RequestBuilder) -> Result<Response> {
        let req = req.build()?;
        self.guard_credentials(&req)?;

        if self.verbose {
            eprintln!("{} {}", req.method(), req.url());
            for (key, val) in req.headers() {
                if !is_secret_header(key) {
                    eprintln!("  {}: {}", key, val.to_str().unwrap_or("(binary)"));
                }
            }
        }

        if self.dry_run {
            eprintln!("[dry-run] {} {}", req.method(), req.url());
            if let Some(body) = req.body() {
                if let Some(bytes) = body.as_bytes() {
                    if let Ok(mut json) = serde_json::from_slice::<Value>(bytes) {
                        if self.dry_run_redacted {
                            redact_sensitive_json(&mut json);
                        }
                        eprintln!("[dry-run] Body: {}", serde_json::to_string_pretty(&json)?);
                    }
                }
            }
            bail!("Dry run — no request was sent");
        }

        let resp = self.send_with_auth_fallback(req).await?;

        if self.verbose {
            eprintln!("  Status: {}", resp.status());
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::from_response(status, &body).into());
        }

        Ok(resp)
    }

    /// Execute and parse JSON response
    pub async fn execute_json(&self, req: RequestBuilder) -> Result<Value> {
        let req = req.build()?;
        self.guard_credentials(&req)?;

        if self.verbose {
            self.print_request(&req);
        }

        if self.dry_run {
            return self.dry_run_response(&req);
        }

        let resp = self.send_with_auth_fallback(req).await?;

        if self.verbose {
            eprintln!("  Status: {}", resp.status());
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::from_response(status, &body).into());
        }

        let json: Value = resp.json().await?;
        Ok(json)
    }

    /// Execute and parse JSON body WITHOUT bailing on non-2xx status.
    /// Lets the caller inspect application-level error envelopes (e.g. ops
    /// endpoints return `{ success: false, error, code }` alongside a 4xx status).
    /// Still respects dry-run and verbose modes via execute_raw().
    pub async fn execute_json_allow_error(&self, req: RequestBuilder) -> Result<Value> {
        let req = req.build()?;
        self.guard_credentials(&req)?;

        if self.verbose {
            self.print_request(&req);
        }

        if self.dry_run {
            return self.dry_run_response(&req);
        }

        let resp = self.send_with_auth_fallback(req).await?;

        if self.verbose {
            eprintln!("  Status: {}", resp.status());
        }

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();

        // Try to parse as JSON; if not JSON and non-2xx, surface as generic error.
        match serde_json::from_str::<Value>(&body) {
            Ok(json) => Ok(json),
            Err(_) if !status.is_success() => Err(ApiError::from_response(status, &body).into()),
            Err(e) => bail!("Failed to parse response: {}", e),
        }
    }

    /// Execute and return a stable response envelope, preserving non-2xx API bodies.
    pub async fn execute_json_envelope(&self, req: RequestBuilder) -> Result<Value> {
        let req = req.build()?;
        self.guard_credentials(&req)?;

        if self.verbose {
            self.print_request(&req);
        }

        if self.dry_run {
            return self.dry_run_response(&req);
        }

        let resp = self.send_with_auth_fallback(req).await?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let text = resp.text().await.unwrap_or_default();
        let body = serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!(text));

        if self.verbose {
            eprintln!("  Status: {}", status);
        }

        let mut envelope = json!({
            "ok": status.is_success(),
            "status": status.as_u16(),
            "contentType": headers
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or(""),
            "body": body,
        });
        // Surface Retry-After so agents know how long to back off on 429.
        if let Some(retry_after) = headers.get("retry-after").and_then(|v| v.to_str().ok()) {
            envelope["retryAfterSeconds"] = retry_after
                .parse::<u64>()
                .map(|secs| json!(secs))
                .unwrap_or_else(|_| json!(retry_after));
        }
        // Structured hints (additive fields only). Provider-key problems and
        // the subscription paywall need human intervention rather than a
        // retry; a rate limit needs a precise back-off.
        if !status.is_success() {
            if let Some(hint) = provider_keys_hint(status.as_u16(), &envelope["body"]) {
                envelope["providerKeysHint"] = hint;
            }
            if let Some(hint) = endpoint_gone_hint(&envelope["body"]) {
                envelope["endpointGoneHint"] = hint;
            }
            match status.as_u16() {
                402 => {
                    let hint = subscription_hint(&envelope["body"]);
                    envelope["subscriptionHint"] = hint;
                }
                429 => {
                    let hint = rate_limit_hint(envelope.get("retryAfterSeconds"));
                    envelope["rateLimitHint"] = hint;
                }
                _ => {}
            }
        }
        Ok(envelope)
    }

    pub async fn execute_json_body_or_error(&self, req: RequestBuilder) -> Result<Value> {
        let envelope = self.execute_json_envelope(req).await?;
        if envelope.get("dryRun").and_then(|v| v.as_bool()) == Some(true) {
            return Ok(envelope);
        }
        if envelope.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            return Ok(envelope.get("body").cloned().unwrap_or(Value::Null));
        }
        self.print_error_envelope(&envelope);
        Err(printed_error_for(&envelope).into())
    }

    pub async fn execute_json_envelope_or_error(&self, req: RequestBuilder) -> Result<Value> {
        let envelope = self.execute_json_envelope(req).await?;
        if envelope.get("dryRun").and_then(|v| v.as_bool()) == Some(true)
            || envelope.get("ok").and_then(|v| v.as_bool()) == Some(true)
        {
            return Ok(envelope);
        }
        self.print_error_envelope(&envelope);
        Err(printed_error_for(&envelope).into())
    }

    /// Print an error envelope; in human/table formats also render the
    /// structured hints (provider keys, subscription, rate limit) as info
    /// boxes on stderr.
    fn print_error_envelope(&self, envelope: &Value) {
        crate::output::print_value(&self.output_format, envelope, &[]);
        if self.output_format != "json" {
            if let Some(hint) = envelope.get("providerKeysHint") {
                print_keys_hint_box(hint);
            }
            if let Some(hint) = envelope.get("subscriptionHint") {
                print_subscription_hint_box(hint);
            }
            if let Some(hint) = envelope.get("rateLimitHint") {
                print_rate_limit_hint_box(hint);
            }
            if let Some(hint) = envelope.get("endpointGoneHint") {
                print_endpoint_gone_box(hint);
            }
        }
    }

    /// Require authentication, returning an error if not configured
    pub fn require_auth(&self) -> Result<()> {
        if self.dry_run {
            return Ok(());
        }
        if self.credentials.auth_header().is_none() {
            bail!(
                "Not authenticated. Run 'flowleap auth login' or set FLOWLEAP_API_KEY / FLOWLEAP_TOKEN."
            );
        }
        Ok(())
    }

    fn print_request(&self, req: &Request) {
        eprintln!("{} {}", req.method(), req.url());
        for (key, val) in req.headers() {
            if !is_secret_header(key) {
                eprintln!("  {}: {}", key, val.to_str().unwrap_or("(binary)"));
            }
        }
    }

    fn dry_run_response(&self, req: &Request) -> Result<Value> {
        let mut body = req
            .body()
            .and_then(|body| body.as_bytes())
            .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok());

        if self.dry_run_redacted {
            if let Some(body) = body.as_mut() {
                redact_sensitive_json(body);
            }
        }

        let dry_run = json!({
            "dryRun": true,
            "dryRunRedacted": self.dry_run_redacted,
            "method": req.method().as_str(),
            "url": req.url().as_str(),
            "authenticated": req.headers().contains_key("authorization") || req.headers().contains_key("x-api-key"),
            // Presence only — the key material itself never appears in output.
            "providerKeys": {
                "epo": req.headers().contains_key("x-epo-ops-key"),
                "uspto": req.headers().contains_key("x-uspto-odp-key"),
            },
            "body": body,
        });

        Ok(dry_run)
    }
}

const SENSITIVE_BODY_FIELDS: &[&str] = &[
    "claim",
    "content",
    "description",
    "filebase64",
    "q",
    "query",
    "text",
    "url",
];

fn redact_sensitive_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if SENSITIVE_BODY_FIELDS.contains(&key.to_ascii_lowercase().as_str()) {
                    *value = Value::String("[REDACTED]".to_string());
                } else {
                    redact_sensitive_json(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_sensitive_json(value);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The full exit-code contract for typed errors (see AGENTS.md).
    #[test]
    fn error_exit_codes_map_typed_errors_to_the_contract() {
        assert_eq!(
            [401, 402, 404, 429, 400, 403, 500].map(exit_code_for_status),
            [3, 4, 5, 6, 1, 1, 1]
        );

        let cases: Vec<(anyhow::Error, i32)> = vec![
            (PrintedError::with_status(401).into(), 3),
            (PrintedError::with_status(402).into(), 4),
            (PrintedError::new().into(), 1),
            (
                ApiError::from_response(StatusCode::NOT_FOUND, "nope").into(),
                5,
            ),
            (
                NetworkError::new("connection refused".to_string()).into(),
                7,
            ),
            (anyhow::anyhow!("anything untyped"), 1),
        ];
        for (err, expected) in cases {
            assert_eq!(error_exit_code(&err), expected, "error: {err}");
        }
    }
}
