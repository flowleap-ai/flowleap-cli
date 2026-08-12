use anyhow::Result;
use clap::{error::ErrorKind, Parser, Subcommand};
use flowleap_cli::commands::{
    academic, analytics, api, auth, citation, config_cmd, doctor, facade, health, keys, legal, mcp,
    npl, ocr, ops, patent, patstat, skills, tools, upgrade, uspto,
};
use flowleap_cli::{client, config, update};
use serde_json::json;

/// One CLI for FlowLeap Patent AI — built for humans and AI agents.
#[derive(Parser)]
#[command(name = "flowleap", version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// API base URL
    #[arg(long, env = "FLOWLEAP_BASE_URL", global = true)]
    base_url: Option<String>,

    /// API key (overrides stored credentials)
    #[arg(long, env = "FLOWLEAP_API_KEY", global = true)]
    api_key: Option<String>,

    /// Bearer token (overrides stored credentials)
    #[arg(long, env = "FLOWLEAP_TOKEN", global = true)]
    token: Option<String>,

    /// Emit stable JSON output
    #[arg(long, global = true)]
    json: bool,

    /// Output format
    #[arg(long, value_parser = ["json", "table", "human"], global = true)]
    output: Option<String>,

    /// Show request details without executing
    #[arg(long, global = true)]
    dry_run: bool,

    /// Redact sensitive request values from dry-run output
    #[arg(long, global = true, requires = "dry_run")]
    dry_run_redacted: bool,

    /// Show verbose request/response details
    #[arg(long, short, global = true)]
    verbose: bool,

    /// Assume "yes" for confirmation prompts, e.g. sending credentials to a
    /// non-FlowLeap base URL (FLOWLEAP_ASSUME_YES=1 works too)
    #[arg(long, global = true)]
    yes: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check CLI config, auth, and backend reachability
    Doctor,
    /// Interactive onboarding: backend check, auth, provider keys (human-only)
    Setup,
    /// Manage patent-provider keys (EPO OPS, USPTO ODP)
    Keys(keys::KeysArgs),
    /// Store initial CLI configuration
    Init {
        /// API base URL to store
        #[arg(long, default_value = "https://api.flowleap.co")]
        base_url: String,
    },
    /// Authenticate with FlowLeap API
    Auth(auth::AuthArgs),
    /// Raw and user API helpers
    Api(api::ApiArgs),
    /// Public backend health probes
    Health(health::HealthArgs),
    /// Search and analyze patents
    Patent(patent::PatentArgs),
    /// Direct EPO OPS API commands
    Ops(ops::OpsArgs),
    /// USPTO Open Data Portal commands
    Uspto(uspto::UsptoArgs),
    /// Search academic literature
    Academic(academic::AcademicArgs),
    /// Search non-patent literature
    Npl(npl::NplArgs),
    /// Search patent-law documents
    Legal(legal::LegalArgs),
    /// Search USPTO citation/prior-art data
    Citation(citation::CitationArgs),
    /// Full-corpus patent analytics (filing trends, countries, assignees, CPC)
    Analytics(analytics::AnalyticsArgs),
    /// Aggregate PATSTAT portfolio analytics for one applicant
    Patstat(patstat::PatstatArgs),
    /// Extract text from a PDF, image, or document via OCR (file or URL)
    Ocr(ocr::OcrArgs),
    /// Compare 2-10 patents side by side (bibliography)
    Compare(facade::CompareArgs),
    /// List a patent's drawings/figures; save image data with --out
    Figures(facade::FiguresArgs),
    /// One-call patent snapshot: bibliography, legal status, family, term
    Summary(facade::SummaryArgs),
    /// Chronological prosecution timeline for a patent
    Timeline(facade::TimelineArgs),
    /// Convert a patent number between formats (epodoc, docdb, original)
    ConvertNumber(facade::ConvertNumberArgs),
    /// Discover and run backend tools (agent-first /v1/tools facade)
    Tools(tools::ToolsArgs),
    /// Serve backend tools over the Model Context Protocol (stdio)
    Mcp(mcp::McpArgs),
    /// Install FlowLeap agent skills into an agent's skills directory
    Skills(skills::SkillsArgs),
    /// Manage CLI configuration
    Config(config_cmd::ConfigArgs),
    /// Upgrade the CLI itself (channel-aware: npm/brew/binary/cargo)
    #[command(alias = "update")]
    Upgrade(upgrade::UpgradeArgs),
}

#[tokio::main]
async fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            if args_want_json()
                && !matches!(
                    err.kind(),
                    ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
                )
            {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": false,
                        "error": {
                            "message": err.to_string(),
                            "kind": format!("{:?}", err.kind()),
                        }
                    }))
                    .unwrap_or_default()
                );
                std::process::exit(err.exit_code());
            }
            err.exit();
        }
    };
    let wants_json = cli.json || cli.output.as_deref() == Some("json");

    if let Err(err) = run(cli).await {
        // Documented exit-code contract (see AGENTS.md "Exit Codes"): typed
        // errors carry the failed request's status class; anything else is 1.
        let code = client::error_exit_code(&err);
        if err.downcast_ref::<client::PrintedError>().is_some() {
            // Already rendered (envelope + hint boxes) — just exit.
            std::process::exit(code);
        }
        if wants_json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": false,
                    "error": {
                        "message": err.to_string(),
                    }
                }))
                .unwrap_or_default()
            );
        } else {
            eprintln!("Error: {err}");
        }
        std::process::exit(code);
    }
}

fn args_want_json() -> bool {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--json" {
            return true;
        }
        if arg == "--output=json" {
            return true;
        }
        if arg == "--output" && args.next().as_deref() == Some("json") {
            return true;
        }
    }
    false
}

async fn run(cli: Cli) -> Result<()> {
    // Load config and credentials
    let mut cfg = config::Config::load()?;
    let mut creds = config::Credentials::load()?;

    // CLI flags > env vars > config file
    if let Some(ref url) = cli.base_url {
        cfg.base_url = url.clone();
    }
    if let Some(ref key) = cli.api_key {
        creds.api_key = Some(key.clone());
    }
    if let Some(ref tok) = cli.token {
        creds.token = Some(tok.clone());
    }

    // fl_org_ keys were a v0.1.x concept with no working backend path; they now
    // travel as Bearer and always 401. Warn instead of failing mysteriously.
    if creds
        .api_key
        .as_deref()
        .is_some_and(|k| k.starts_with("fl_org_"))
    {
        eprintln!(
            "warning: fl_org_ organization keys are not supported (the backend never accepted them). \
             Mint a personal token instead: flowleap auth login && flowleap auth create-token --name <name> --store"
        );
    }

    // Provider-key env overrides (headless/agent path; humans use `flowleap setup`).
    if let Ok(value) = std::env::var("FLOWLEAP_EPO_KEY") {
        creds.epo_key = Some(value);
    }
    if let Ok(value) = std::env::var("FLOWLEAP_EPO_SECRET") {
        creds.epo_secret = Some(value);
    }
    if let Ok(value) = std::env::var("FLOWLEAP_USPTO_KEY") {
        creds.uspto_key = Some(value);
    }

    let output_format = if cli.json {
        "json".to_string()
    } else {
        cli.output
            .clone()
            .or_else(|| cfg.output_format.clone())
            .unwrap_or_else(|| "human".to_string())
    };

    let ctx = client::Context {
        config: cfg,
        credentials: creds,
        output_format,
        dry_run: cli.dry_run,
        dry_run_redacted: cli.dry_run_redacted,
        verbose: cli.verbose,
        token_overridden: cli.token.is_some(),
        assume_yes: cli.yes,
        http: client::build_http_client(),
    };

    // First-run auto-onboarding: an authenticated command with no credentials,
    // in an interactive terminal, offers setup instead of just erroring. Skips
    // for --json/--dry-run and non-TTY (agents get the structured error), and
    // for the commands that manage their own auth or don't need it.
    let wants_first_run = command_needs_auth(&cli.command)
        && ctx.credentials.auth_header().is_none()
        && !cli.dry_run
        && !cli.json
        && std::io::IsTerminal::is_terminal(&std::io::stdin())
        && std::io::IsTerminal::is_terminal(&std::io::stderr());
    // Once-a-day update notice. Spawned before the command so the registry
    // fetch overlaps its work; printed to stderr after it finishes. Never
    // runs in MCP server mode: the process is a long-lived protocol server.
    let update_check = if matches!(cli.command, Commands::Mcp(_) | Commands::Upgrade(_)) {
        None
    } else {
        update::spawn_check(&ctx.http, cli.json, cli.dry_run)
    };

    let result = if wants_first_run && offer_first_run_setup(&ctx).await? {
        // Reload credentials the wizard just wrote and continue the command.
        let creds = config::Credentials::load()?;
        let ctx = client::Context {
            credentials: creds,
            ..ctx
        };
        dispatch(cli.command, &ctx).await
    } else {
        dispatch(cli.command, &ctx).await
    };

    if let Some(handle) = update_check {
        // Grace period sized just above the check's own fetch timeout so the
        // task always resolves (answer or timeout) instead of being cancelled
        // mid-flight. Only the one fetch run per day can wait here at all;
        // cached runs resolve instantly.
        if let Ok(Ok(Some(notice))) = tokio::time::timeout(update::CHECK_GRACE, handle).await {
            eprintln!("{}", notice);
        }
    }

    result
}

/// Commands that hit an authenticated endpoint (everything except local/auth-
/// managing ones). Used to decide whether to offer first-run setup. `mcp` is
/// excluded even though it calls authenticated endpoints: stdin is its
/// protocol channel, and an unauthenticated server must still start and
/// report auth errors in-band.
fn command_needs_auth(command: &Commands) -> bool {
    !matches!(
        command,
        Commands::Doctor
            | Commands::Setup
            | Commands::Init { .. }
            | Commands::Auth(_)
            | Commands::Health(_)
            | Commands::Skills(_)
            | Commands::Config(_)
            | Commands::Mcp(_)
            | Commands::Upgrade(_)
    )
}

/// Prompt to run the setup wizard on first use. Returns true if setup ran and
/// the caller should retry the original command.
async fn offer_first_run_setup(ctx: &client::Context) -> Result<bool> {
    use dialoguer::{theme::ColorfulTheme, Confirm};
    eprintln!("You're not set up yet — FlowLeap needs a quick one-time sign-in.");
    let run = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Run setup now?")
        .default(true)
        .interact()
        .unwrap_or(false);
    if !run {
        return Ok(false);
    }
    keys::setup_wizard(ctx).await?;
    eprintln!();
    Ok(true)
}

async fn dispatch(command: Commands, ctx: &client::Context) -> Result<()> {
    match command {
        Commands::Doctor => doctor::run(ctx).await,
        Commands::Setup => keys::setup_wizard(ctx).await,
        Commands::Keys(args) => keys::run(ctx, args).await,
        Commands::Init { base_url } => init(ctx, &base_url).await,
        Commands::Auth(args) => auth::run(ctx, args).await,
        Commands::Api(args) => api::run(ctx, args).await,
        Commands::Health(args) => health::run(ctx, args).await,
        Commands::Patent(args) => patent::run(ctx, args).await,
        Commands::Ops(args) => ops::run(ctx, args).await,
        Commands::Uspto(args) => uspto::run(ctx, args).await,
        Commands::Academic(args) => academic::run(ctx, args).await,
        Commands::Npl(args) => npl::run(ctx, args).await,
        Commands::Legal(args) => legal::run(ctx, args).await,
        Commands::Citation(args) => citation::run(ctx, args).await,
        Commands::Analytics(args) => analytics::run(ctx, args).await,
        Commands::Patstat(args) => patstat::run(ctx, args).await,
        Commands::Ocr(args) => ocr::run(ctx, args).await,
        Commands::Compare(args) => facade::compare(ctx, args).await,
        Commands::Figures(args) => facade::figures(ctx, args).await,
        Commands::Summary(args) => facade::summary(ctx, args).await,
        Commands::Timeline(args) => facade::timeline(ctx, args).await,
        Commands::ConvertNumber(args) => facade::convert_number(ctx, args).await,
        Commands::Tools(args) => tools::run(ctx, args).await,
        Commands::Mcp(args) => mcp::run(ctx, args).await,
        Commands::Skills(args) => skills::run(ctx, args),
        Commands::Config(args) => config_cmd::run(ctx, args).await,
        Commands::Upgrade(args) => upgrade::run(ctx, args).await,
    }
}

async fn init(ctx: &client::Context, base_url: &str) -> Result<()> {
    let parsed =
        reqwest::Url::parse(base_url).map_err(|e| anyhow::anyhow!("Invalid URL: {}", e))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        anyhow::bail!("base-url must use http or https");
    }
    if parsed.host_str().is_none() {
        anyhow::bail!("base-url must include a host");
    }

    let mut cfg = config::Config::load()?;
    cfg.base_url = base_url.to_string();
    cfg.save()?;
    let value = json!({
        "ok": true,
        "baseUrl": base_url,
        "configPath": config::Config::config_path()?,
    });
    if ctx.output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("Configured FlowLeap base URL: {}", base_url);
    }
    Ok(())
}

/// Skill↔CLI validation: every `flowleap …` example inside a ```bash fence
/// of every embedded skill must parse against the real clap command tree, so
/// skill content cannot drift from the CLI it documents.
///
/// Normalization applied to each documented command line before parsing:
/// - `\` line continuations are joined into one line
/// - tokens are split shell-words style (quoted strings stay one token)
/// - the command is cut at the first shell operator token (`|`, `||`, `&&`,
///   `;`, `>`, `>>`, `<`) or comment token (`#…`) — only the flowleap
///   invocation itself is validated
/// - `<angle-bracket>` placeholders and `$(command substitutions)` stand for
///   user-supplied values and are replaced with the literal `X`
/// - `[optional]` usage-notation tokens are dropped
///
/// A parse outcome counts as valid when the command and its flags exist:
/// `Ok`, help/version, missing required arguments, and invalid *values* are
/// all fine. Unknown arguments, unknown subcommands, and conflicting flags
/// fail the test.
#[cfg(test)]
mod skill_validation_tests {
    use super::Cli;
    use clap::error::ErrorKind;
    use clap::Parser as _;
    use flowleap_cli::commands::skills::embedded_skill_docs;

    /// Extract every `flowleap …` command line from the ```bash fences of a
    /// SKILL.md document, joining `\` line continuations.
    fn extract_flowleap_commands(contents: &str) -> Vec<String> {
        let mut commands = Vec::new();
        // Some(true) = inside a bash fence, Some(false) = inside another
        // fence (json, …), None = outside any fence.
        let mut fence: Option<bool> = None;
        let mut pending = String::new();
        for line in contents.lines() {
            let trimmed = line.trim();
            if let Some(info) = trimmed.strip_prefix("```") {
                fence = match fence {
                    Some(_) => None,
                    None => Some(matches!(info.trim(), "bash" | "sh")),
                };
                pending.clear();
                continue;
            }
            if fence != Some(true) {
                continue;
            }
            let continued = !pending.is_empty();
            if !continued && !trimmed.starts_with("flowleap ") {
                continue;
            }
            match trimmed.strip_suffix('\\') {
                Some(head) => {
                    pending.push_str(head.trim_end());
                    pending.push(' ');
                }
                None => {
                    pending.push_str(trimmed);
                    commands.push(std::mem::take(&mut pending));
                }
            }
        }
        commands
    }

    /// Shell operator tokens that end the flowleap invocation.
    fn is_shell_operator(token: &str) -> bool {
        matches!(token, "|" | "||" | "&&" | ";" | ">" | ">>" | "<")
    }

    /// Usage-notation opener like `[flags]` or `[--lang` — but not a JSON
    /// value such as `["EP1000000"]`, which stays a real argument.
    fn starts_optional_notation(token: &str) -> bool {
        token.starts_with('[') && !token.contains('{') && !token.contains('"')
    }

    /// Tokenize one documented command line into argv for clap (see the
    /// module doc for the normalization rules). `[optional]` usage notation
    /// may span several tokens (`[--lang en]`); the whole span is dropped.
    fn normalize_command(line: &str) -> Result<Vec<String>, String> {
        let tokens = shell_words::split(line).map_err(|err| format!("cannot tokenize: {err}"))?;
        let mut argv = Vec::new();
        let mut in_optional_notation = false;
        for token in tokens {
            if in_optional_notation {
                in_optional_notation = !token.ends_with(']');
                continue;
            }
            if is_shell_operator(&token) || token.starts_with('#') {
                break;
            }
            if starts_optional_notation(&token) {
                in_optional_notation = !token.ends_with(']');
                continue;
            }
            if (token.starts_with('<') && token.ends_with('>') && token.len() > 2)
                || token.contains("$(")
            {
                argv.push("X".to_string());
            } else {
                argv.push(token);
            }
        }
        Ok(argv)
    }

    /// Validate every extracted command of one document; returns one message
    /// per invalid example.
    fn validate_snippet(source: &str, contents: &str) -> Vec<String> {
        let mut errors = Vec::new();
        for line in extract_flowleap_commands(contents) {
            let argv = match normalize_command(&line) {
                Ok(argv) => argv,
                Err(message) => {
                    errors.push(format!("{source}: `{line}` — {message}"));
                    continue;
                }
            };
            if let Err(err) = Cli::try_parse_from(&argv) {
                if matches!(
                    err.kind(),
                    ErrorKind::UnknownArgument
                        | ErrorKind::InvalidSubcommand
                        | ErrorKind::ArgumentConflict
                ) {
                    let reason = err.to_string();
                    let reason = reason.lines().next().unwrap_or_default();
                    errors.push(format!("{source}: `{line}` — {reason}"));
                }
            }
        }
        errors
    }

    /// Full `description:` frontmatter value of one SKILL.md, unquoted.
    fn frontmatter_description(contents: &str) -> Option<String> {
        contents
            .lines()
            .find(|line| line.starts_with("description:"))
            .map(|line| {
                line.trim_start_matches("description:")
                    .trim()
                    .trim_matches('"')
                    .to_string()
            })
    }

    #[test]
    fn every_documented_flowleap_example_parses() {
        let mut errors = Vec::new();
        let mut checked = 0;
        for (name, contents) in embedded_skill_docs() {
            checked += extract_flowleap_commands(contents).len();
            errors.extend(validate_snippet(name, contents));
        }
        assert!(
            checked > 0,
            "no flowleap examples found in any embedded skill"
        );
        assert!(
            errors.is_empty(),
            "documented commands that do not parse against the CLI:\n{}",
            errors.join("\n")
        );
    }

    /// README↔CLI validation: every `flowleap …` example inside a ```bash
    /// fence of README.md must parse against the real clap command tree,
    /// under the same normalization rules as the embedded skills.
    #[test]
    fn every_documented_readme_example_parses() {
        let readme = include_str!("../README.md");
        let checked = extract_flowleap_commands(readme).len();
        assert!(checked > 0, "no flowleap examples found in README.md");
        let errors = validate_snippet("README.md", readme);
        assert!(
            errors.is_empty(),
            "README examples that do not parse against the CLI:\n{}",
            errors.join("\n")
        );
    }

    #[test]
    fn every_skill_description_is_a_routing_signal() {
        for (name, contents) in embedded_skill_docs() {
            let description = frontmatter_description(contents)
                .unwrap_or_else(|| panic!("{name}: missing description frontmatter"));
            assert!(
                description.len() >= 60,
                "{name}: description too short ({} chars) to route on: {description}",
                description.len()
            );
            assert!(
                description.contains("Trigger when"),
                "{name}: description lacks a \"Trigger when\" routing clause: {description}"
            );
        }
    }

    /// A deliberately broken snippet must fail: proves the validator rejects
    /// unknown subcommands and unknown flags rather than passing everything.
    #[test]
    fn broken_examples_are_rejected() {
        let bad = "```bash\n\
                   flowleap patnet search --query battery\n\
                   flowleap patent search --query battery --nonexistent-flag 5\n\
                   flowleap patent search --query battery --limit 5\n\
                   ```\n";
        let errors = validate_snippet("inline", bad);
        assert_eq!(
            errors.len(),
            2,
            "expected exactly the two broken examples to fail: {errors:?}"
        );
        assert!(errors[0].contains("patnet"));
        assert!(errors[1].contains("nonexistent-flag"));
    }

    /// Normalization keeps quoted values together, replaces placeholders,
    /// and cuts at pipes/redirects/comments.
    #[test]
    fn normalization_rules() {
        let argv = normalize_command(
            "flowleap ops search --cql \"ti=solar AND pa=Tesla\" --end 50 # comment",
        )
        .expect("tokenize");
        assert_eq!(
            argv,
            [
                "flowleap",
                "ops",
                "search",
                "--cql",
                "ti=solar AND pa=Tesla",
                "--end",
                "50"
            ]
        );

        let argv = normalize_command("flowleap ocr <office-action.pdf> [flags] > out.md")
            .expect("tokenize");
        assert_eq!(argv, ["flowleap", "ocr", "X"]);

        // Multi-token optional notation is dropped; JSON arrays are kept.
        let argv = normalize_command("flowleap ops claims <doc> [--lang en]").expect("tokenize");
        assert_eq!(argv, ["flowleap", "ops", "claims", "X"]);
        let argv = normalize_command(
            "flowleap tools run compare_patents --input '{\"patent_numbers\":[\"EP1000000\"]}'",
        )
        .expect("tokenize");
        assert_eq!(argv.len(), 6);

        let argv =
            normalize_command("flowleap ocr \"$(cat scan-path.txt)\" | head").expect("tokenize");
        assert_eq!(argv, ["flowleap", "ocr", "X"]);
    }
}
