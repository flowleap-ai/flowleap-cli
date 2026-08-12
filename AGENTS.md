# FlowLeap CLI — Agent & Contributor Guide

## Overview

`flowleap` is a Rust CLI for the FlowLeap Patent AI backend API. It provides patent search, CQL query building, academic search, and direct EPO OPS access — designed for both human users and AI agents.

## Build & Test

```bash
cargo build              # Build the binary
cargo test               # Run all tests
cargo clippy             # Lint (must pass with zero warnings)
cargo fmt --check        # Format check
```

All four must pass before submitting changes.

Gotcha: `skills/` content is embedded via `include_dir!`, which does NOT
trigger recompilation when only SKILL.md files change — a skills-only edit
tests against a stale embed. `touch src/commands/skills.rs` (then rebuild)
before trusting `skills install` output or regenerating goldens.

## Architecture

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entry point, clap argument parsing, command routing |
| `src/config.rs` | TOML config (`config.toml`) and credentials (`credentials.toml`) management |
| `src/client.rs` | HTTP client context — auth injection, request building, dry-run/verbose |
| `src/output.rs` | Output module (re-exports formatter) |
| `src/output/formatter.rs` | JSON, table, and human-readable output formatting |
| `src/commands/auth.rs` | OAuth device flow, personal API tokens (create/list/revoke), status |
| `src/commands/tools.rs` | Agent-first tool facade: list/describe/run `/v1/tools/*` |
| `src/commands/skills.rs` | Embedded agent-skill installer (`skills/` baked into binary): multi-harness targets (claude/claude-project/codex/cursor/gemini/--dir), version stamps, `skills update` |
| `src/commands/patent.rs` | EPO patent search (CQL) and CQL query builder |
| `src/commands/uspto.rs` | USPTO ODP search, grants, applications, continuity, file wrapper (transactions/assignments/foreign-priority/adjustment/attorney/documents + OCR document text), query builder |
| `src/commands/ops.rs` | Direct EPO OPS API (biblio, claims, family, legal, abstract) |
| `src/commands/academic.rs` | Academic literature search |
| `src/commands/npl.rs` | Non-patent literature search (OpenAlex) |
| `src/commands/legal.rs` | Patent-law document search (legal RAG) |
| `src/commands/citation.rs` | USPTO enriched citation search |
| `src/commands/api.rs` | Profile/usage + raw API escape hatch |
| `src/commands/health.rs` / `doctor.rs` | Health probes and environment diagnosis |
| `src/commands/config_cmd.rs` | CLI configuration management |
| `src/commands/upgrade.rs` | Channel-aware self-update (`upgrade`/`update`): detects npm/Homebrew/raw-binary/cargo from the running binary's canonical path; raw binaries self-update with sha256-verified atomic swap; `--check` reports `{channel, currentVersion, latestVersion, updateAvailable, command}` with no side effects |
| `src/update.rs` | Once-a-day update notice (recommends `flowleap upgrade`) + `cached_latest()` seam consumed by `doctor` |

## Command Structure

```
flowleap <command> <subcommand> [flags]
```

All commands support `--output json|table|human`, `--dry-run`, and `--verbose`.
Use `--dry-run-redacted` with `--dry-run` when request bodies may contain
unpublished inventions, claims, document text, URLs, or search queries.

## Config Precedence

CLI flags > environment variables > `~/.config/flowleap/config.toml`

## Authentication

Every authenticated request sends `Authorization: Bearer <credential>` — the
backend has **no** `X-API-Key` path. The credential is either a Clerk JWT (from
the OAuth device flow) or a long-lived personal API token (`fl_pat_…`).

Credential sources (checked in order):
1. `--token` flag or `FLOWLEAP_TOKEN` env var
2. `--api-key` flag or `FLOWLEAP_API_KEY` env var (use an `fl_pat_…` token here)
3. Stored credentials in `~/.config/flowleap/credentials.toml` (written 0600)

Token lifecycle: `flowleap auth login` (OAuth) → `flowleap auth create-token
--name <n> [--store]` → `flowleap auth tokens` / `flowleap auth revoke-token <id>`.
API tokens cannot mint further tokens (backend-enforced).

`flowleap auth status` **verifies** the credential (it probes `/api/profile`),
so it reports whether the credential works, not merely that one is stored:
`verification.state` is `valid` (exit 0), `rejected` — present but refused with
a 401 (exit 3), `absent` (exit 3), or `unverified` when the check could not be
completed (exit 7; `--dry-run` keeps exit 0). Read `verification.checked` before
trusting a state: an unreachable backend yields `unverified`, never `rejected`,
because the absence of a verdict is not evidence against the credential.

All `/v1/*` patent-data routes additionally require an active subscription
(402 `subscription_required` with an `upgradeUrl`) and share a fixed
60 requests/minute/user rate limit (429 + `Retry-After`, surfaced as
`retryAfterSeconds` in JSON error envelopes).

**Store-time TTL guard** — every path that persists an OAuth session token
(`auth login`, `--json auth login`, `flowleap setup`) decodes its `exp` claim
before writing to `credentials.toml` and refuses, loudly, to store one with
under 10 minutes left. This guards against flowleap-backend#254: device-flow
approval has, on occasion, echoed back a short-lived default Clerk session
token instead of the long-lived `flowleap`-template token the server is
supposed to mint — storing it would silently shadow a still-good `fl_pat_…`
token with a credential dead before the next command runs. The refusal exits
3 (auth required) and names the actual lifetime found; re-run `flowleap auth
login`. Tokens that aren't a decodable JWT (an `fl_pat_…` token, for example)
are stored as before — the guard only fires on a positively short `exp`.

## Patent-Data Keys (BYOK)

Patent data may require the user's own provider credentials — EPO OPS
(consumer key + secret, a pair) and USPTO ODP (API key). The domain term is
**patent-data keys**; `provider_keys_required` / `provider_keys_invalid` are the
wire codes. The CLI stores them in
`credentials.toml` (0600) and forwards them per-request as
`x-epo-ops-key`/`x-epo-ops-secret`/`x-uspto-odp-key` headers; they are never
logged (verbose/dry-run output redacts them).

- `flowleap setup` / `flowleap keys setup` — interactive wizard (**human-only**:
  keys come from browser signups; refuses to run without a TTY)
- `flowleap keys set epo --key <k> --secret <s>` / `keys set uspto --key <k>` —
  non-interactive; validates live before saving (`--no-verify` to skip)
- `flowleap keys list` (masked) / `keys test` (live verdicts via
  `POST /v1/keys/validate`) / `keys rm <provider>`
- Env overrides: `FLOWLEAP_EPO_KEY`, `FLOWLEAP_EPO_SECRET`, `FLOWLEAP_USPTO_KEY`

**Agent protocol:** when a command fails because keys are missing or rejected,
the JSON error envelope carries a `providerKeysHint` object with
`code` (`provider_keys_required` | `provider_keys_invalid`), `provider`, and
`requiresHumanIntervention: true`. Do NOT retry or invent keys — surface the
hint and ask the user to run `flowleap setup` (or provide keys via env/flags).
Human/table output renders the same hint as an info box on stderr.

**Key-gate doctrine** — authored in the `flowleap-keys` skill, mirrored from the
app's shipped prompt so both harnesses have one personality. A
`provider_keys_required` gate is a user-action stop for that office, never an
exhausted route: no web-scraped substitute for it (searches or single-document
reads), and the free key is never framed as a paywall. A gate is read, never
inferred — only that explicit code means gated, so empty results, truncated
payloads, and 5xx keep the normal fallbacks. With the other office live, deliver
its results in full, name the gap as a missing-key gap, ask once at the end, and
never silently narrow a prior-art or FTO scope. Keyless commands (`patstat`,
`legal`, `academic`, `npl`) may be offered as *different* data, never as a
substitute. After the key lands, re-run only the gated office and merge. Keep
this wording aligned with the app prompt when either side changes
(abdullahatrash/flowleap-agent-v2#173).

## Exit Codes & Structured Hints (agent integration)

Every run exits with a documented code, so agents can branch on `$?` without
parsing JSON:

| Code | Meaning | Trigger |
|------|---------|---------|
| 0 | Success | |
| 1 | Generic failure | Any error without a dedicated code (config, response parsing, other 4xx/5xx) |
| 2 | Usage error | clap argument/flag parse failure |
| 3 | Auth required | HTTP 401 — run `flowleap auth login` or set `FLOWLEAP_API_KEY` / `FLOWLEAP_TOKEN` |
| 4 | Subscription required | HTTP 402 — a human must subscribe; see `subscriptionHint` |
| 5 | Not found | HTTP 404 |
| 6 | Rate limited | HTTP 429 — back off, then retry; see `rateLimitHint` |
| 7 | Network failure | Connection failure or request timeout reaching the backend |

On failure the JSON error envelope may carry structured hints — **additive**
fields only, so existing envelope consumers are unaffected. Human/table output
renders each hint as an info box on stderr:

- `providerKeysHint` — missing/rejected EPO or USPTO keys (see Provider Keys
  above). Needs a human; do not retry.
- `subscriptionHint` (402) — `{ requiresHumanIntervention: true, plan:
  "Basic", upgradeUrl, message }`. The upgrade URL comes from the response
  body when present, else `https://flowleap.co/pricing`. Subscribing happens
  in a browser — surface the URL to the user; do not retry.
- `rateLimitHint` (429) — `{ retryAfterSeconds?, message }`. When
  `retryAfterSeconds` is present (from the `Retry-After` header, also surfaced
  top-level on the envelope), wait exactly that long before retrying.

## API Endpoints

The `/v1/tools/*` facade is the preferred agent surface: `flowleap tools list`
discovers every tool with its JSON input schema, `flowleap tools run <name>`
executes one. Provider-specific routes remain for humans and compatibility.

| Endpoint | Method | Auth Required |
|----------|--------|---------------|
| `/oauth/device` | POST | No |
| `/oauth/device/token` | POST | No |
| `/v1/tools` | GET | Yes |
| `/v1/tools/openapi.json` | GET | Yes |
| `/v1/tools/{tool_name}` | POST | Yes |
| `/v1/patent-search` | POST | Yes |
| `/v1/academic-search` | POST | Yes |
| `/v1/npl-search` | POST | Yes |
| `/v1/legal-search` (+ `/stats`, `/jurisdictions`, `/docs`) | POST/GET | Yes |
| `/v1/citation-search` (+ `/forward`, `/stats/{n}`, `/novelty/{n}`) | POST/GET | Yes |
| `/v1/patent-search-uspto/search` | POST | Yes |
| `/v1/patent-search-uspto/grants/{patentNumber}` | GET | Yes |
| `/v1/patent-search-uspto/applications/{appNumber}` (+ `/continuity`, `/transactions`, `/assignment`, `/foreign-priority`, `/adjustment`, `/attorney`, `/associated-documents`) | GET | Yes |
| `/v1/patent-search-uspto/applications/{appNumber}/documents` (+ `/{documentId}/text` — server-side OCR) | GET | Yes |
| `/v1/ops/biblio?doc={id}` | GET | Yes |
| `/v1/ops/abstract?doc={id}` | GET | Yes |
| `/v1/ops/family?doc={id}` | GET | Yes |
| `/v1/ops/legal?doc={id}` | GET | Yes |
| `/v1/ops/fulltext/claims?doc={id}&lang={lang}` | GET | Yes |
| `/v1/ops/fulltext/description?doc={id}&lang={lang}` | GET | Yes |
| `/api/profile` | GET | Yes |
| `/api/usage` | GET | Yes |
| `/api/tokens` (create/list) | POST/GET | Yes (create requires Clerk auth, not an API token) |
| `/api/tokens/{id}` | DELETE | Yes |
| `/v1/keys/validate` | POST | Yes (no subscription required) |

OPS endpoints wrap payloads in a response envelope:

```json
{ "success": true, "data": { /* endpoint-specific */ }, "cached": false, "executionTimeMs": 432 }
```

On failure:

```json
{ "success": false, "error": "...", "code": "NOT_FOUND", "status": 404 }
```

Error `code` values: `MISSING_PARAM` (400), `NOT_FOUND` (404), `RATE_LIMITED` (429), `INTERNAL_ERROR` (500). Canonical endpoint list: the backend's `GET /v1/ops/health` response. Live OpenAPI spec: `<base-url>/docs/json` (requires `ENABLE_SWAGGER=true` in production).

## Security

- Never output stored credentials (API keys, tokens) in logs or verbose mode
- Use `--dry-run` for safety when testing mutating operations
- Use `--dry-run-redacted` when dry-run output itself may enter terminal, CI,
  or agent logs. It preserves request shape while replacing sensitive values.
- Search queries are written by the caller, locally — there is no server-side
  query builder, so an unpublished invention description never has to leave
  the machine to become a query (see the `flowleap-patent` skill).
- Authorization header is stripped from verbose output
- Base-URL credential guard: when the effective base URL's host is not
  `flowleap.co`/`*.flowleap.co`/`localhost`/`127.0.0.1`/`::1`, the CLI prints
  one stderr warning per invocation naming the host and the credential kinds
  that will be sent (presence only, never values). In an interactive terminal
  it requires y/N confirmation before the first request; `--yes` (or
  `FLOWLEAP_ASSUME_YES=1`) skips the prompt. Non-TTY, `--json`, and `--dry-run`
  runs warn and proceed, so agents are never blocked and stdout stays clean.

## Skills

The `skills/` directory contains SKILL.md files for AI agent consumption. Each skill describes one CLI capability with usage examples, flags, and expected output. Skills are organized into:

- **Service skills** (`flowleap-*`): One per CLI command
- **Persona skills** (`persona-*`): Role-based bundles for specific workflows
- **Recipe skills** (`recipe-*`): Multi-step workflow templates

### Skills vs. tools (when to write which)

**Skills instruct, tools reach.** A skill can only orchestrate data that some
existing backend tool/command already touches. Default every new capability to
a skill; add a backend tool only when the data is unreachable — and then add
the *thin* passthrough first and put the workflow in a skill on top of it.

- **Author CLI-canonical first.** New capability-skills are written here (the
  CLI dialect), then ported to the VS Code app's skill dialect
  (`flowleap-agent-v2 …/assets/skills/`, extension tool names,
  `user-invocable`) in the same sitting. The two ecosystems do not sync;
  see `CONTEXT.md` for the vocabulary.
- **Never route around a missing tool.** A skill must not instruct agents to
  call provider APIs (EPO/USPTO) directly with the user's BYOK keys — that
  forfeits caching, key handling, rate-limit protection, and uniform error
  envelopes. Missing data is the signal to add the thin tool, not to
  hand-roll HTTP in a skill.
- **Distribution is release-gated.** CLI skills ship inside the binary (next
  tag) and reach the website marketplace via the `flowleap-plugins` re-sync
  (bump `sync.json` ref, add the entry, copy byte-for-byte). Budget the
  re-sync into every release that touches `skills/`.

## Environment Variables

| Variable | Description |
|----------|-------------|
| `FLOWLEAP_API_KEY` | API key for authentication |
| `FLOWLEAP_TOKEN` | Bearer token for authentication |
| `FLOWLEAP_BASE_URL` | API base URL override |
| `FLOWLEAP_ASSUME_YES` | Skip confirmation prompts (same as `--yes`) |


## Testing

- Unit tests: config parsing, credential storage, output formatting
- Integration tests: in `tests/` directory
- Test with `cargo test`
