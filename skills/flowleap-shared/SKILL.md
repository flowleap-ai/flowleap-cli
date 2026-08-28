---
name: flowleap-shared
description: Shared reference for every FlowLeap skill — authentication (OAuth device flow, fl_pat_ personal API tokens), credential storage, config precedence, global flags, and output formats. Trigger when a FlowLeap command needs credentials set up, a global flag explained, config file locations, or output-format guidance; for the overall command map start from the `flowleap` skill.
---

# FlowLeap CLI — Shared Reference

Shared authentication, configuration, and global-flag reference used by every
other FlowLeap skill. For the overall map of commands, skills, and workflows,
start from the `flowleap` skill.

## Authentication

Every authenticated request sends `Authorization: Bearer <credential>` — either
a session JWT from the OAuth device flow or a long-lived personal API token
(`fl_pat_…`).

Environment variable overrides (highest priority):
- `FLOWLEAP_API_KEY` — personal API token (`fl_pat_…`)
- `FLOWLEAP_TOKEN` — Bearer token
- `FLOWLEAP_BASE_URL` — API base URL

The login, token minting/listing/revocation, and 401 self-heal commands live in
`flowleap-auth`. Patent-data keys (EPO OPS / USPTO ODP BYOK) live in
`flowleap-keys`.

## Global Flags

| Flag | Description | Default |
|------|-------------|---------|
| `--json` | Shorthand for `--output json` | `false` |
| `--output <format>` | Output format: `json`, `table`, `human` | `human` |
| `--base-url <url>` | API base URL | `https://api.flowleap.co` |
| `--api-key <key>` | Override stored API key (`fl_pat_…`) | — |
| `--token <token>` | Override stored token | — |
| `--dry-run` | Show request without executing | `false` |
| `--dry-run-redacted` | Redact sensitive values from dry-run output; requires `--dry-run` | `false` |
| `--verbose`, `-v` | Show request/response details | `false` |

## Configuration

Config is stored in `~/.config/flowleap/config.toml`. Credentials live
separately in `~/.config/flowleap/credentials.toml` (written mode 0600).

```bash
flowleap config set base-url https://api.flowleap.co
flowleap config get base-url
```

## Config Precedence

CLI flags > environment variables > config file

## Output Formats

- `--json` (or `--output json`) — Machine-readable JSON (best for agents)
- `--output table` — Formatted table
- `--output human` — Human-readable text (default)

When using FlowLeap as an AI agent, always pass `--json` for reliable parsing.

## Subscription, Rate Limits & Exit Codes

Patent data runs on the **Tools facade** — the single agent surface, reached as
`/v1/tools`. Every data command POSTs to a named tool there; the per-source
**provider routes** they used to call are **retired endpoints** (see the
`endpoint_gone` row below). All of it requires an active subscription and shares
a limit of 60 requests/minute/user. `doctor`, `health`, `auth`, and `keys test`
work without a subscription, so setup can always be diagnosed.

Error envelopes carry additive hints — `subscriptionHint` (402, has
`upgradeUrl`, needs a human), `providerKeysHint` (missing/rejected EPO/USPTO
patent-data keys, or the trial's shared data budget spent for today — needs a
human), `rateLimitHint` (429, has `retryAfterSeconds`), and `endpointGoneHint`
(410, `{ requiresUpgrade: true, successor?, reason?, message }`).

**Branch on codes, never on message text.** Backend codes come from a closed
registry and never change once shipped; wording is freely editable by policy, so
matching text can invent a verdict that is not there. A `providerKeysHint` with
code `provider_keys_required` is a **user-action stop for that office, never an
exhausted route**: do not retry, do not invent keys, and do not substitute
web-scraped patent data for the gated office — searches and single-document
reads alike. The keys are free from each office. Read the gate, never infer it:
only the explicit gate codes mean gated, so an empty result, a truncated
payload, or a 5xx stays an ordinary dead route with the normal fallbacks. Full
doctrine (proceed-then-ask, keyless pivot, resume): `flowleap-keys`.

| Exit code | Meaning |
|-----------|---------|
| 0 | Success |
| 1 | Generic failure |
| 2 | Usage error (bad flags/arguments) |
| 3 | Auth required (HTTP 401, or no credential configured at all) — log in or set `FLOWLEAP_API_KEY` |
| 4 | Subscription required (HTTP 402) — surface `subscriptionHint.upgradeUrl` to a human |
| 5 | Not found (HTTP 404) |
| 6 | Rate limited (HTTP 429) — back off per `rateLimitHint.retryAfterSeconds` |
| 7 | Network failure reaching the backend |
| 8 | Endpoint gone (HTTP 410 `endpoint_gone`) — this build calls a retired endpoint |
| 9 | Patent-data keys required/rejected, or the trial data budget exhausted — see `providerKeysHint`; a human must add keys, never retry |

**Exit 9 is the key gate, and only the key gate.** It is the code behind
`provider_keys_required` / `provider_keys_invalid` /
`trial_budget_exhausted`, on data commands and on `keys test` / `keys set`
alike, so a first-run key problem is distinguishable from a bad query without
parsing the envelope. Treat it exactly as the `providerKeysHint` doctrine below
says: a user-action stop for that office. The `trial_budget_exhausted` variant
(backend ADR 0017) is the soft one — its hint carries `resetsAt` (next UTC
day), so with the user's blessing an agent may also wait it out; the user's own
free keys lift it permanently. Success envelopes warn before the wall: a
`trial_data_budget_low` entry in `body.warnings` means finish the current work,
then surface the key ask.

**Exit 3 also covers the local auth guard.** A command that needs a credential
and finds none fails before anything is sent; its `--json` envelope is
`{ "ok": false, "error": { "message": …, "code": "unauthenticated" } }`. That
`code` is the only machine-readable signal for the failure an agent hits before
it ever reaches the backend — branch on it, not on the message.

**Exit 8 is not a retry.** A retired endpoint is permanently removed and a
retired path is never reused, so the same call will never succeed again. Read
`endpointGoneHint.successor` for where the capability moved, run
`flowleap upgrade`, then `flowleap skills update` — an upgraded CLI with stale
skill files walks straight back into the same 410.

Every request also sends the **Client version header**
(`X-FlowLeap-Client: cli/<version>`). It is observational only — logged as
stale-client and route-usage evidence, never used to reject a request — so it is
nothing to configure or work around.

## Reachability — `flowleap health`

```bash
flowleap --json health        # liveness
flowleap --json health api    # readiness; carries the backend's apiVersion
```

Both are public: no subscription, no patent-data key, no provider call. Use one
of these (or `flowleap --json doctor` for the full checklist) to test whether the
backend is up. **Never probe reachability with a search command** — a search
costs a provider call and can fail for reasons that have nothing to do with
reachability (no subscription, a key gate, a bad query), so it answers a
different question than the one you asked.

## Readiness — `flowleap --json doctor`

Doctor is the machine-readable onboarding contract. Its JSON always carries:

- `ready: bool` — backend reachable AND authenticated AND no **blocking** next
  step pending. Stricter than `ok`, which keeps its reachability-only meaning.
- `nextSteps` — the pending onboarding steps in dependency order (empty array
  when complete). Steps already covered — e.g. a provider the server has its
  own keys for — are omitted. Each step:

```json
{ "id": "store-epo-keys", "actor": "agent",
  "title": "Store the EPO consumer key and secret",
  "run": "flowleap keys set epo --key <k> --secret <s>" }
```

Stable step ids (public contract): `auth-login` (human), `mint-personal-token`
(agent — pending while auth is only a session token with no `fl_pat_` personal
token), `obtain-epo-keys` (human), `store-epo-keys` (agent),
`obtain-uspto-key` (human), `store-uspto-key` (agent), `verify-keys` (agent),
`refresh-skills` (agent — installed skill files were written by an older CLI
and still teach retired commands).

A step carrying `advisory: true` is worth doing but blocks nothing right now,
so it does not count against `ready`. Only `mint-personal-token` is advisory: a
session token works today and merely expires later, so an otherwise-green
machine exits 0 with it still listed. Act on advisory steps; do not treat one
as a reason the machine is not ready.

**Exit contract: doctor exits 0 iff `ready`, else 1** — with the checklist
JSON always fully emitted first, so `flowleap doctor && <work>` gates
pipelines without parsing. An unreachable backend still emits the checklist
from local state (offline diagnosis works); `keyValidation.source` says
whether provider verdicts came from the server (`"server"`) or fell back to
local key presence (`"local"`, with a `note`).

**Agent-mediated sequence**: run `flowleap --json doctor`; for each step in
`nextSteps`, execute `actor: "agent"` steps yourself via their `run` command,
and relay `actor: "human"` steps (title + `url`) to the user; re-run doctor
until `ready` is true.

## Updating the CLI

`flowleap upgrade` (alias `flowleap update`) updates the CLI itself, detecting
the install channel from the running binary and acting accordingly: npm runs
`npm i -g flowleap@latest`, an install.sh/raw binary self-updates in place
(downloads the platform release asset, verifies its sha256 against
`checksums.txt`, atomically swaps), and a cargo install prints the
`cargo install --git … --force` command. `--check` (and `--json`/`--dry-run`)
report `{ channel, currentVersion, latestVersion, updateAvailable, command }`
with no side effects, so agents branch on the result. The daily update notice
and `flowleap doctor` both point here.

Installed skill files are copies, so an upgrade leaves them behind. A
raw-binary self-update refreshes them with the new build; on every other
channel run `flowleap skills update` after upgrading. Until you do, the skill
files being read document retired commands — `flowleap doctor` reports a stale
install as a ✗ carrying the `refresh-skills` next step.

```bash
flowleap upgrade --check --json
flowleap upgrade
```

## Safety

- Use `--dry-run` before executing mutating operations
- Add `--dry-run-redacted` when the request contains an unpublished invention,
  claim, document text, URL, or search query that should not enter logs
- Search queries are written by you, locally — an unpublished invention
  description never has to leave the machine to become a query (see
  `flowleap-patent`)
- Use `--verbose` to inspect request details (credentials are redacted)
- Never include credentials in commit messages or logs
