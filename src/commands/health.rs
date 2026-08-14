use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::Value;

use crate::client::Context;
use crate::output;

/// Backend health probes. The backend exposes exactly two (both public):
/// liveness at `/health` and readiness at `/v1/health`, the latter carrying the
/// server's `apiVersion`. The per-route and cache/Redis probes were retired
/// with the provider routes (backend ADR 0014).
#[derive(Parser)]
pub struct HealthArgs {
    #[command(subcommand)]
    command: Option<HealthCommand>,
}

#[derive(Subcommand)]
enum HealthCommand {
    /// Readiness probe — reports the backend's apiVersion
    Api,
}

pub async fn run(ctx: &Context, args: HealthArgs) -> Result<()> {
    let path = match args.command {
        None => "/health",
        Some(HealthCommand::Api) => "/v1/health",
    };

    let result = ctx.execute_json_envelope_or_error(ctx.get(path)).await?;
    output::print_value(&ctx.output_format, &result, &[]);

    // The server build, spelled out for humans; JSON callers read it from the
    // body they already have. Liveness carries no apiVersion at all, so say
    // where it lives instead of leaving the reader to guess it is missing.
    if ctx.output_format != "json" {
        match result.pointer("/body/apiVersion").and_then(Value::as_str) {
            Some(version) => println!("apiVersion: {version}"),
            None if path == "/health" => {
                println!(
                    "apiVersion: not reported by the liveness probe — run `flowleap health api`"
                )
            }
            None => {}
        }
    }
    Ok(())
}
