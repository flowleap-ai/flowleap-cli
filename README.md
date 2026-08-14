# FlowLeap CLI

One CLI + MCP server for [FlowLeap Patent AI](https://flowleap.co) — built for humans and AI agents.

`flowleap` puts the FlowLeap Patent AI backend in your terminal: patent search across EPO and USPTO, natural-language query building, academic and non-patent literature, patent-law reference search, enriched citation data, OCR, full-corpus analytics, and one-call patent snapshots. The same binary serves every backend tool to AI agents — as a stdio [MCP](https://modelcontextprotocol.io) server (`flowleap mcp`) and as bundled Agent Skills (`flowleap skills install`) for Claude Code, Codex, Cursor, and Gemini CLI.

## Quick Start

```bash
# 1. Install (more channels under "Installation" below)
npm i -g flowleap

# 2. Authenticate — OAuth 2.0 device flow, opens your browser
flowleap auth login

# 3. First query
flowleap patent search --query "solar panel efficiency" --limit 5
```

Verify your setup any time:

```bash
flowleap doctor
```

`doctor` checks config, credentials, and backend reachability, and tells you what to fix.

## Access & Pricing

The CLI itself is open source (MIT) and free to install. The patent-data commands call the FlowLeap backend, which requires:

- **A FlowLeap account with an active Basic plan.** A trial is available — see [flowleap.co/pricing](https://flowleap.co/pricing).
- Without an active plan, data commands return HTTP 402 and exit with code 4. In `--json` mode the error envelope carries a structured `subscriptionHint` — `{ requiresHumanIntervention: true, plan: "Basic", upgradeUrl, message }` — with the upgrade URL. **This is expected behavior, not a bug**: agents should surface the URL to a human rather than retry.
- All `/v1` data routes share a rate limit of 60 requests/minute/user. Exceeding it returns HTTP 429 (exit code 6) with a `rateLimitHint` carrying `retryAfterSeconds`.

Some data sources additionally need your own **patent-data keys** — free signups at each office: EPO OPS (key + secret) and USPTO ODP (API key). Run `flowleap setup` for the guided wizard, or see `flowleap keys --help`. `provider_keys_required` / `provider_keys_invalid` are the wire codes that name them in error envelopes.

**Agent behavior when a key is missing** (the key-gate doctrine, shipped in full in the `flowleap-keys` skill): a `provider_keys_required` error is a **user-action stop for that office, never an exhausted route**. An agent must not substitute web-scraped patent data for the gated office — searches and single-document reads alike — and must not frame the free key as a paywall. With the other office live it delivers those results in full, names the gap as a *missing-key gap* rather than a coverage finding, and asks once at the end of the turn; it never silently narrows a prior-art or FTO scope to the office whose key happens to be set. Keyless commands (`patstat`, `legal`, `academic`, `npl`) stay available meanwhile, offered as *different* data rather than a replacement for live patent search. A gate is read, never inferred: only that explicit code means gated, so an empty result, a truncated payload, or a 5xx keeps the normal retry paths. Once the key is added, only the previously gated office is re-run — no restart, no new session.

`doctor`, `health`, `auth`, and `keys test` work without a subscription, so you can always diagnose your setup.

## Use with AI Agents

Two integration paths, both embedded in the binary — no network needed to install:

- **MCP server**: `flowleap mcp` serves every backend tool over stdio MCP. Tools are mirrored live from the backend's `/v1/tools` registry, so new backend tools appear without a CLI update.
- **Agent Skills**: `flowleap skills install` writes SKILL.md documentation (or a condensed rules file, depending on the harness) into the location your agent auto-loads.

Authenticate once before wiring either path (`flowleap auth login`, or set `FLOWLEAP_API_KEY` for headless use). An unauthenticated MCP server still starts, but every tool call returns an error explaining how to log in.

Verify readiness any time with the one-command diagnostic — it checks backend reachability, auth, patent-data keys, and the live tool count, and exits nonzero with fix instructions when something is missing:

```bash
flowleap mcp --check
```

### Claude Code

```bash
# MCP server
claude mcp add flowleap -- flowleap mcp

# and/or skills (full SKILL.md directories)
flowleap skills install                            # user-level: ~/.claude/skills
flowleap skills install --target claude-project    # this project: ./.claude/skills
```

### Codex

```bash
flowleap skills install --target codex    # marked block in ./AGENTS.md
```

MCP server — add to `~/.codex/config.toml`:

```toml
[mcp_servers.flowleap]
command = "flowleap"
args = ["mcp"]
```

### Cursor

```bash
flowleap skills install --target cursor   # ./.cursor/rules/flowleap.mdc
```

MCP server — add to `~/.cursor/mcp.json` (or `.cursor/mcp.json` in a project):

```json
{
  "mcpServers": {
    "flowleap": { "command": "flowleap", "args": ["mcp"] }
  }
}
```

### Gemini CLI

```bash
flowleap skills install --target gemini   # marked block in ./GEMINI.md
```

Any other MCP-capable harness works the same way: run `flowleap` with the single argument `mcp` as a stdio server.

After upgrading the CLI, refresh every recorded skill install in one go:

```bash
flowleap skills update
```

### Keeping skills fresh (and which channel to use)

**User-scope skills win.** In Claude-family harnesses, a skill in `~/.claude/skills`
(or a project's `.claude/skills`) *shadows* a same-named skill provided by a plugin
or marketplace pack — the user copy is loaded and the packaged one is ignored, even
when the packaged one is newer. So a one-time `flowleap skills install` can silently
pin you to a stale copy of a skill while the packaged version moves on.

To stay current:

- **Pick one channel per machine.** Inside the FlowLeap IDE, prefer the marketplace
  packs — they auto-update. Use `flowleap skills install` for other harnesses (Codex,
  Cursor, Gemini CLI, or plain Claude Code) — don't run both for the same skills.
- **Re-running install refreshes its own copies.** `flowleap skills install` compares
  each installed skill against the bundled one via a content digest recorded in
  `.flowleap-skills-manifest.json`: copies it wrote that you haven't touched are
  refreshed in place; skills you've edited locally are left alone and reported (pass
  `--force` to overwrite those too). Skills it never installed are never touched.
- **Refresh everything after upgrading** with `flowleap skills update`, and check
  status any time with `flowleap doctor` — its `skills` section flags installs made by
  an older CLI.

## Commands

| Command | Description |
|---------|-------------|
| `doctor` | Check CLI config, auth, and backend reachability |
| `setup` | Interactive onboarding: backend check, auth, patent-data keys (human-only) |
| `keys` | Manage patent-data keys (EPO OPS, USPTO ODP): set, list, test, rm |
| `init` | Store initial CLI configuration (base URL) |
| `auth` | Login (OAuth device flow), logout, status, personal API tokens |
| `api` | Profile, usage, and the raw authenticated API escape hatch |
| `health` | Public backend health probes (api, cache, redis, …) |
| `patent` | Search patents (EPO) with CQL you write yourself |
| `ops` | Direct EPO OPS data: search, biblio, claims, description, family, legal, abstract |
| `uspto` | USPTO Open Data Portal: search, grants, applications, continuity |
| `academic` | Search academic literature (Semantic Scholar, arXiv) |
| `npl` | Search non-patent literature (OpenAlex) |
| `legal` | Search patent-law reference documents (EPC, EPO Guidelines, MPEP, …) |
| `citation` | USPTO enriched citation data: search, forward, stats, novelty |
| `analytics` | Full-corpus patent analytics (filing trends, countries, assignees, CPC) |
| `patstat` | PATSTAT analytics: portfolio aggregates for one applicant, guarded SQL, and `graph` — a named node and the relationships around it |
| `ocr` | Extract text from a PDF, image, or document via OCR (file or URL) |
| `compare` | Compare 2–10 patents side by side (bibliography) |
| `figures` | List a patent's drawings/figures; save image data with `--out` |
| `summary` | One-call patent snapshot: bibliography, legal status, family, term |
| `timeline` | Chronological prosecution timeline for a patent |
| `convert-number` | Convert a patent number between formats (epodoc, docdb, original) |
| `tools` | Discover and run backend tools (agent-first `/v1/tools` facade) |
| `mcp` | Serve backend tools over the Model Context Protocol (stdio) |
| `skills` | List, install, and update bundled agent skills (multi-harness) |
| `config` | Manage CLI configuration |
| `upgrade` | Upgrade the CLI itself (channel-aware: npm/binary/cargo); alias `update` |

A sampler:

```bash
# Patents
flowleap patent search --query "lithium battery recycling" --limit 5
flowleap uspto search --query "wireless charging" --limit 3
flowleap uspto grant 11800000
flowleap ops biblio EP1234567
flowleap ops claims EP1234567 --lang en

# One-call verbs
flowleap summary EP1000000
flowleap timeline EP1000000
flowleap compare EP1000000 US5443036
flowleap figures EP1000000 --out fig1.png
flowleap convert-number US5443036.A --to epodoc

# Literature, law, citations, analytics
flowleap academic search "machine learning patent classification"
flowleap npl "battery thermal management" --limit 10
flowleap legal search "doctrine of equivalents" --limit 10
flowleap citation search 16000001 --size 20
flowleap analytics --keyword battery --country US --date-from 2020-01-01
flowleap patstat portfolio Siemens --from-year 2015 --to-year 2024
flowleap patstat graph resolve EP3477840
flowleap patstat graph patent EP3477840
flowleap patstat graph applicant 98765
flowleap patstat graph neighborhood EP3477840 --depth 2 --edge-types cites,cited_by
flowleap patstat graph path EP3477840 US5960411
flowleap patstat graph explain pat:56123456

# Documents
flowleap ocr ./office-action.pdf > office-action.md

# Agent-first tool facade (same registry the MCP server mirrors)
flowleap tools list
flowleap tools describe get_patent_summary
flowleap tools run search_patents --input '{"query":"ti=solar","limit":3}'

# Raw API escape hatch — only when a high-level command is missing
flowleap --json api request get /v1/health
flowleap --json api request post /v1/patent-search --body-file request.json --dry-run
```

## Installation

**npm / pnpm / yarn:**

```bash
npm i -g flowleap
```

The npm package has no install scripts. The native binary is downloaded on first run from the matching GitHub release, verified against the release's sha256 `checksums.txt` before it executes, and the package is published with [npm provenance](https://docs.npmjs.com/generating-provenance-statements) attesting the exact repo, commit, and CI run that built it.

**Shell installer (macOS / Linux):**

```bash
curl -fsSL https://raw.githubusercontent.com/flowleap-ai/flowleap-cli/main/install.sh | sh
```

The script detects your platform, downloads the latest release binary, and verifies its sha256 against the release's `checksums.txt` before installing to `/usr/local/bin`.

**From source (requires Rust):**

```bash
cargo install --git https://github.com/flowleap-ai/flowleap-cli.git
```

Prebuilt release binaries cover macOS (x86_64, arm64), Linux (x86_64 and arm64 glibc, plus a static `flowleap-linux-x86_64-musl` build for Alpine/containers — download it directly from the [releases page](https://github.com/flowleap-ai/flowleap-cli/releases)), and Windows (x86_64).

**Downloading a binary from the releases page on macOS:** the release binaries are not notarized, so Gatekeeper quarantines anything the browser downloaded and refuses to run it ("cannot be opened because the developer cannot be verified"). Clear the quarantine attribute after downloading:

```bash
xattr -d com.apple.quarantine ./flowleap-darwin-arm64
chmod +x ./flowleap-darwin-arm64
```

The shell installer and npm paths above are unaffected — neither marks the binary as browser-downloaded.

**Updating:** `flowleap upgrade` updates the CLI on whichever channel installed it — no need to remember which. It runs `npm i -g flowleap@latest` for npm installs, self-updates the raw binary in place (downloading the platform release asset and verifying its sha256 against `checksums.txt` before an atomic swap, exactly like first-run), and prints the `cargo install … --force` command for source installs. Use `flowleap upgrade --check` (add `--json` for agents) to see the channel and available version without changing anything.

Installed skill files are copies, so an upgrade leaves them behind. A raw-binary self-update refreshes them with the new build automatically; on the other channels run `flowleap skills update` yourself once the upgrade finishes — until you do, agents read skill files that document retired commands.

## Authentication

Every authenticated request sends `Authorization: Bearer <credential>`. Three ways to supply one:

1. **OAuth 2.0 device flow** (recommended for humans): `flowleap auth login` prints a device code and opens your browser to approve it.
2. **Personal API token** (recommended for agents/CI): `flowleap auth create-token --name my-agent --store` mints a revocable `fl_pat_…` token. List with `flowleap auth tokens`, revoke with `flowleap auth revoke-token <id>`.
3. **Environment variables**: `FLOWLEAP_API_KEY` (an `fl_pat_…` token) or `FLOWLEAP_TOKEN`.

Credentials are stored in `~/.config/flowleap/credentials.toml` (mode 0600).

```bash
flowleap auth status                  # is the credential present AND working?
flowleap auth logout                  # clear everything, including patent-data keys
flowleap auth logout --session-only   # keep API key + patent-data keys
```

`auth status` verifies the credential against the backend rather than only
reporting that one is stored, and exits with the verdict so scripts can branch
on `$?`:

| State | Meaning | Exit |
|-------|---------|------|
| `valid` | The backend accepted the credential | 0 |
| `rejected` | Present but refused (HTTP 401) — expired, revoked, or wrong | 3 |
| `absent` | No credential configured | 3 |
| `unverified` | Could not check (backend unreachable, or `--dry-run`) | 7 (0 for `--dry-run`) |

An unreachable backend is reported as "could not verify", never as invalid —
the absence of a verdict is not evidence against the credential. `--json` emits
the same verdict as `{ verification: { state, checked, reason? } }`, where
`checked` says whether a live check actually reached a conclusion.

Every device-flow login also runs a store-time TTL guard: it decodes the
session token's `exp` claim and refuses to write it to `credentials.toml` if
under 10 minutes remain, printing the actual lifetime and exiting 3 rather
than silently persisting a credential that's already effectively dead
(flowleap-backend#254). Non-JWT tokens (e.g. `fl_pat_…`) are unaffected.

### CI / Headless Use

Mint a token once on your machine, then export it wherever the CLI runs:

```bash
# once, interactively (token creation requires an OAuth session, not an API token)
flowleap auth login
flowleap auth create-token --name ci-agent

# in CI: export the fl_pat_… token
export FLOWLEAP_API_KEY="fl_pat_..."
flowleap --json patent search --query "battery cooling" --limit 3
```

Headless notes:

- `--yes` (or `FLOWLEAP_ASSUME_YES=1`) skips confirmation prompts, e.g. the credential guard that fires when `--base-url` points at a non-FlowLeap host. Non-TTY, `--json`, and `--dry-run` runs are never blocked on a prompt.
- Provider keys can come from env too: `FLOWLEAP_EPO_KEY`, `FLOWLEAP_EPO_SECRET`, `FLOWLEAP_USPTO_KEY`.
- API tokens cannot mint further tokens (backend-enforced).

## JSON Output & Exit Codes

Add `--json` (or `--output json`) for stable machine-readable output — recommended for all agent and script use. Success responses carry the command's data; failures print a JSON error envelope on stdout:

```json
{ "ok": false, "error": { "message": "…" } }
```

Envelopes may carry additive structured hints: `subscriptionHint` (402 — upgrade URL, needs a human), `providerKeysHint` (missing/rejected patent-data keys — needs a human, do not retry, and do not substitute web-scraped data for the gated office; see [Access & Pricing](#access--pricing)), `rateLimitHint` (429 — wait `retryAfterSeconds`, then retry), and `endpointGoneHint` (410 — the build is stale; upgrade rather than retry, and the hint names the successor endpoint). Human/table output renders the same hints as info boxes on stderr.

Every run exits with a documented code, so scripts can branch on `$?` without parsing JSON:

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Generic failure |
| 2 | Usage error (bad flags/arguments) |
| 3 | Auth required (HTTP 401, or no credential configured at all) — `flowleap auth login` or set `FLOWLEAP_API_KEY` |
| 4 | Subscription required (HTTP 402) — see `subscriptionHint` |
| 5 | Not found (HTTP 404) |
| 6 | Rate limited (HTTP 429) — back off, see `rateLimitHint` |
| 7 | Network failure reaching the backend |
| 8 | Endpoint gone (HTTP 410) — this build calls a retired endpoint; run `flowleap upgrade`, see `endpointGoneHint` |
| 9 | Patent-data keys required or rejected — a human must add EPO/USPTO keys; see `providerKeysHint`, do not retry |

Two details worth knowing:

- **The local auth guard.** A command that needs a credential and finds none
  fails before anything is sent. It exits 3 like a rejected 401, and its
  `--json` envelope carries `error.code: "unauthenticated"` — the one failure
  that never reaches the backend still names itself in the closed code registry.
- **Exit 9 covers `keys` too.** `flowleap keys test` and `flowleap keys set`
  exit 9 on a rejected or missing provider key, the same code a key-gated data
  command returns.

Use `--dry-run` to see the exact method, URL, auth status, and JSON body the CLI would send — the safest way to debug request shape:

```bash
flowleap --json patent search --query "battery cooling system" --limit 1 --dry-run
```

Add `--dry-run-redacted` when the body contains an unpublished invention,
claim, search query, document text, or URL. The output retains the JSON shape
but replaces sensitive values with `[REDACTED]`.

Search queries are written by the caller (you or your agent), locally — there
is no server-side query builder, so an unpublished invention description never
has to leave the machine to become a query. The bundled `flowleap-patent` and
`flowleap-uspto` skills carry the query-writing method (term extraction,
discriminating terms, and the mandatory count probe).

The full contract (hint schemas, endpoint list, agent protocol) lives in [AGENTS.md](AGENTS.md).

## Cookbook: Recipe Skills

The CLI bundles 12 multi-step workflow recipes (`recipe-*` skills). Install them with `flowleap skills install` and your agent can run each end to end:

| Recipe | What it does |
|--------|--------------|
| `recipe-prior-art-search` | Comprehensive prior art search: query generation, dual EPO/USPTO search, academic sweep, deep dives on closest hits |
| `recipe-patent-landscape` | Map a technology area: scoped searches, key players, recent activity, full-corpus filing analytics |
| `recipe-freedom-to-operate` | FTO/clearance search: per-feature queries, blocking-patent search, legal-status and claims checks |
| `recipe-claim-analysis` | Extract and analyze a patent's claims with full context and element decomposition |
| `recipe-patent-to-report` | Extract everything about one patent into a structured report (dossier) |
| `recipe-academic-literature-review` | Technology review combining scholarly literature and a matching patent sweep |
| `recipe-invention-disclosure` | Turn an inventor conversation into a complete invention disclosure form with novelty pre-check |
| `recipe-claim-drafting` | Draft claims grounded in the closest prior art and checked against MPEP/EPO formal rules |
| `recipe-office-action-response` | OCR an office action, pull cited references, map rejections, ground arguments in guidelines |
| `recipe-invalidity-analysis` | Build a prior-art invalidity case: priority date, element hunt, X/Y/A invalidity chart |
| `recipe-infringement-charting` | Element-by-element infringement claim chart against an accused product |
| `recipe-audit-report` | Auditable record of AI-assisted research: command log, provenance, AI-usage disclosure |

## Skills Reference

28 skills ship embedded in the binary — `flowleap skills list` shows them, `flowleap skills install` works offline anywhere. Three categories:

| Category | Description |
|----------|-------------|
| **Service skills** (`flowleap-*`) | One per CLI capability (patent, ops, uspto, …) |
| **Persona skills** (`persona-*`) | Role-based bundles (patent attorney, IP analyst, researcher, startup founder) |
| **Recipe skills** (`recipe-*`) | Multi-step workflows (see Cookbook above) |

```
skills/                            # 28 skills, embedded in the binary
  flowleap/SKILL.md                # Start here: umbrella + skill map
  flowleap-shared/SKILL.md         # Auth, global flags, config reference
  flowleap-auth/SKILL.md           # OAuth device flow + fl_pat_ tokens
  flowleap-keys/SKILL.md           # BYOK provider keys (EPO OPS, USPTO ODP)
  flowleap-patent/SKILL.md         # EPO patent search + CQL query builder
  flowleap-uspto/SKILL.md          # USPTO ODP search, grants, continuity
  flowleap-ops/SKILL.md            # Direct EPO OPS document data
  flowleap-academic/SKILL.md       # Academic literature search
  flowleap-npl/SKILL.md            # Non-patent literature (OpenAlex)
  flowleap-legal/SKILL.md          # Patent-law reference search (RAG)
  flowleap-citation/SKILL.md       # USPTO enriched citation data
  flowleap-tools/SKILL.md          # Agent-first /v1/tools facade
  persona-patent-attorney/SKILL.md
  persona-ip-analyst/SKILL.md
  persona-researcher/SKILL.md
  persona-startup-founder/SKILL.md
  recipe-prior-art-search/SKILL.md
  recipe-patent-landscape/SKILL.md
  recipe-freedom-to-operate/SKILL.md
  recipe-claim-analysis/SKILL.md
  recipe-patent-to-report/SKILL.md
  recipe-academic-literature-review/SKILL.md
  recipe-office-action-response/SKILL.md    # prosecution
  recipe-claim-drafting/SKILL.md            # prosecution
  recipe-invention-disclosure/SKILL.md      # prosecution
  recipe-invalidity-analysis/SKILL.md       # litigation
  recipe-infringement-charting/SKILL.md     # litigation
  recipe-audit-report/SKILL.md              # governance
```

Claude targets copy the full SKILL.md directories; the codex/cursor/gemini targets render a condensed agent-rules document (command reference + workflow triggers) generated from the same embedded content — the file each harness actually auto-loads. Every install is stamped with the CLI version and recorded in the config file, and copy targets also get a `.flowleap-skills-manifest.json` recording a content digest per skill. That digest lets a later install or `flowleap skills update` refresh the copies it wrote while leaving skills you've edited locally untouched (see "Keeping skills fresh" above). The daily update notice and `flowleap doctor` both flag installs made by an older CLI.

Every `flowleap` example inside the embedded skills — and this README — is parse-checked against the real CLI in the test suite, so documentation cannot drift from the commands it documents.

## Configuration

Configuration is stored in `~/.config/flowleap/config.toml`.

```bash
flowleap config set base-url https://api.flowleap.co
flowleap config get base-url
```

Precedence: CLI flags > environment variables > config file.

### Environment Variables

| Variable | Description |
|----------|-------------|
| `FLOWLEAP_API_KEY` | API key for authentication (an `fl_pat_…` token) |
| `FLOWLEAP_TOKEN` | Bearer token for authentication |
| `FLOWLEAP_BASE_URL` | API base URL override |
| `FLOWLEAP_EPO_KEY` / `FLOWLEAP_EPO_SECRET` | EPO OPS patent-data key pair |
| `FLOWLEAP_USPTO_KEY` | USPTO ODP patent-data key |
| `FLOWLEAP_ASSUME_YES` | Skip confirmation prompts (same as `--yes`) |
| `FLOWLEAP_NO_UPDATE_CHECK` | Disable the once-a-day update notice (already skipped for `--json`, `--dry-run`, and non-TTY runs) |
| `FLOWLEAP_RELEASES_API_URL` | Override the GitHub releases API URL `flowleap upgrade` checks (enterprise mirrors) |
| `FLOWLEAP_RELEASES_DOWNLOAD_BASE` | Override the base URL `flowleap upgrade` downloads release assets from (enterprise mirrors) |
| `FLOWLEAP_NPM_REGISTRY_URL` | Override the npm registry URL used for the version check |

### Global Flags

```
--json              Emit stable machine-readable JSON
--output <format>   Output format: json, table, human (default: human)
--base-url <url>    Override API base URL
--api-key <key>     Override stored API key
--token <token>     Override stored token
--dry-run           Show request details without executing
--dry-run-redacted  Redact sensitive values from dry-run output (requires --dry-run)
--yes               Assume "yes" for confirmation prompts
--verbose, -v       Show verbose request/response details
```

## Development

```bash
cargo build     # Build
cargo test      # Run tests (includes skill + README example validation)
cargo clippy    # Lint
cargo fmt       # Format
```

Architecture, endpoint list, exit-code contract, and agent protocol: [AGENTS.md](AGENTS.md). Entry point for Claude Code: [CLAUDE.md](CLAUDE.md).

## License

[MIT](LICENSE) — Copyright (c) 2026 FlowLeap.
