---
name: flowleap-shared
description: "Common rules for every FlowLeap skill: the tool to call for each method in the FlowLeap Patent AI app and in chat clients, what to do when a patent office needs a data key, which offices each text tool covers, and, for the FlowLeap command-line tool, sign-in, settings and output formats. Use it when another FlowLeap skill refers to it, when a patent office answers that its data key is missing, or when the office coverage of a text tool matters."
user-invocable: false
---

# FlowLeap — Shared Reference

Shared authentication, configuration, and global-flag reference used by every
other FlowLeap skill. For the overall map of commands, skills, and workflows,
start from the `flowleap` skill.

## Chat clients and the FlowLeap Patent AI app

Two hosts run these skills without the CLI. In both, never run a `flowleap …`
command and never tell the user to run one: call the tool that the table below
names for the command.

`auth`, `doctor`, `setup`, `init`, `keys`, `config`, `skills`, `upgrade`,
`api`, `health`, and `mcp` have no tool in either host. Global flags
(`--json`, `--dry-run`, `--output`) do not apply: a tool call always returns
the JSON envelope. **Never ask for a key value in the chat.** "The key gate"
below (a gated office is a user-action stop) applies in both hosts; only where
the user adds a key differs.

### Chat clients with the FlowLeap connector

In claude.ai, Claude Desktop and mobile, Cowork, and ChatGPT, FlowLeap is a
connector to the **hosted MCP server**. These clients have no shell.

- **Call the tool of the same name through the connector.** The connector lists
  the same registry as `flowleap tools list`: the same tool names, input schemas
  (`snake_case` parameters), success envelope, and error codes.
- **Sign-in is the connector's OAuth.** The user connects FlowLeap in the
  client's connector settings. In place of `health`, call `server_info`: it
  reports the backend's configured providers and available tools.
- **Patent-data keys are stored keys.** The CLI forwards the keys kept on the
  user's machine; a chat client has no keys to forward, so the backend uses the
  stored key the user added on the FlowLeap Patent-data keys page
  (https://www.flowleap.co/en/dashboard/keys). When a tool answers a key gate,
  relay that page (the error's `keysPageUrl` and `nextStep`, when present).

### The FlowLeap Patent AI app (firm OS and desktop)

The app runs the office tools in its own process on the data keys of the firm
or of the desktop user. It has no `flowleap` command for you to run. The office
tools need no FlowLeap sign-in; the paid tools need FlowLeap Data, which the
firm administrator or the desktop user sets up outside the chat.

- **Office tools are native, under the table names with no prefix**
  (`search_patents`, `get_bibliography`, `get_us_grant`, `ocr`, …). The app also
  has `compare_claims` and `read_pdf` (OCR of a workspace file).
- **Paid tools need FlowLeap Data.** `patent_analytics`, the `patstat_*` tools,
  `get_fulltext`, and `server_info` exist only as `mcp__flowleap__<name>`, and
  only when the firm or the user has FlowLeap Data. US claims come from
  `mcp__flowleap__get_claims`: the native `get_claims` has no US source, and its
  answer names that tool. When a paid tool is not in your tool list, tell the
  user that the step needs FlowLeap Data, and continue with the native tools.
- **Data keys are the firm's or the desktop user's.** The app's data keys are
  the Patent-Data Keys (EPO OPS, USPTO ODP) and the Semantic Scholar and
  Mistral OCR keys. A tool without a usable key answers `<Connector> key not
  configured. …` or `<Connector> rejected the stored key. …`, naming the
  Settings card. On the desktop, relay that message: the user adds or replaces
  the key in Settings > Patent data connectors. On the firm OS the firm
  administrator sets the keys for the whole firm and an attorney cannot change
  them, so tell the attorney to ask the administrator. The flowleap.co keys
  page does not apply in the app.
- **No network outside the tools.** Shell commands have no network, so use the
  shell only for local files, such as a skill's `references/`. The app has no
  web search or web fetch tool.

| Command family | Tool(s) | Notes |
|---|---|---|
| `ops biblio` / `claims` / `description` / `family` / `legal` / `abstract` | `get_bibliography`, `get_claims`, `get_description`, `get_family`, `get_legal_status`, `get_abstract` | `get_family` is the INPADOC extended family; `--resolve-npl` is `resolve_npl: true` |
| `ops register-biblio` | `get_register_biblio` | EP only: representatives of record, opponents |
| `ops register-search` | `search_register` | EP Register CQL (`query`, `range`); the one search with a representative index (`re=`), opponent (`op=`), name in any role (`nm=`, surname-first — use it when a role index returns 0); EP applications only |
| `patent search`, `ops search` | `search_patents` | `provider: "epo_ops"`, CQL |
| `uspto search` | `search_patents` | `provider: "uspto"`, Lucene |
| `uspto application` / `grant` / `continuity` | `get_us_application`, `get_us_grant`, `get_continuity` | |
| `uspto transactions` / `assignments` / `foreign-priority` / `adjustment` / `attorney` | `get_transactions`, `get_assignments`, `get_foreign_priority`, `get_patent_term_adjustment`, `get_attorney` | |
| `uspto documents` / `document-text` | `get_application_documents`, `read_application_document` | |
| `citation search` / `forward` / `stats` | `search_office_action_citations`, `search_enriched_citations`, `get_citation_stats` | `citation novelty` is `search_office_action_citations` with `category: "X"`, `examiner_cited_only: true` |
| EPO forward citations (no command) | `get_citations` | |
| `patent examiner-baseline` | `examiner_baseline` | The tool returns the same JSON. |
| `academic search`, `npl` | `search_academic`, `search_npl` | |
| `npl-work` | `get_npl_work` | one cited paper: DOI, OpenAlex id or citation string |
| `analytics` | `patent_analytics` | App: `mcp__flowleap__patent_analytics`, FlowLeap Data only |
| `ocr` | `ocr` | |
| `patstat portfolio` / `query` / `docs` | `patstat_portfolio`, `patstat_query`, `patstat_docs` | App: `mcp__flowleap__` prefix, FlowLeap Data only |
| `patstat graph resolve` / `cpc` / `patent` / `applicant` / `technology` / `neighborhood` / `path` / `explain` | `patstat_resolve`, `patstat_cpc`, `patstat_patent`, `patstat_applicant`, `patstat_technology`, `patstat_neighborhood`, `patstat_path`, `patstat_explain` | App: `mcp__flowleap__` prefix, FlowLeap Data only |
| `tools run <name>` | the same names | |

The one-call verbs (`summary`, `compare`, `timeline`, `figures`,
`convert-number`) map the same way; their table is in the `flowleap` skill.

## The key gate

**A key gate is a USER-ACTION STOP, never an exhausted route.** A gate means
that office needs a key only the user can obtain — it is not a transient error,
not a zero-result, and not a route you have exhausted. So for the gated office:

- **Never substitute web-scraped data for it** — not for searches, and not for
  single-document reads. "Give me the claims of EP…" with no EPO OPS key is
  declined *for that office* with the key named as the one-step fix; it is never
  quietly served from Google Patents, Espacenet, freepatentsonline, or a web
  search instead. Only the user adding the key opens that office.
- **The keys are FREE** from each office (EPO OPS and USPTO ODP both issue them
  at no cost, browser signup). Never frame the ask as a paywall, an upsell, or a
  FlowLeap limitation.
- **A gate is READ, never INFERRED.** An office is gated only when a call you
  actually made returned an explicit gate: with the CLI or in a chat client, one
  of the gate **codes** in `flowleap-keys` ("Which codes mean gated"); in the
  FlowLeap Patent AI app, one of the two fixed answers `<Connector> key not
  configured. …` and `<Connector> rejected the stored key. …`. Never conclude a
  gate from an empty result set, a truncated or partial payload, a 5xx, a
  timeout, from any other error message that happens to name a credential, or
  from `keys list` showing nothing. Anything short of an
  explicit gate is an ordinary dead or empty route, and normal
  persistence and fallbacks apply to it in full — reformulate, try the alternate
  office or route, then the usual web fallback where a web tool exists.
- The forbid rule covers **only** an office gated on a missing patent-data key.
  Offices with no backend route at all (CN/JP/KR) and routes that are genuinely
  dead or empty *with a working key* keep their existing fallbacks unchanged.

**Never ask for the key value in the chat.** A key typed into a chat reaches
the transcript and every log that keeps it. Tell the human where the key is
added instead: with the CLI and in a chat client, see `flowleap-keys`; in the
FlowLeap Patent AI app, see "The FlowLeap Patent AI app" above.

**Proceed, then ask.** When one provider is live and the other is gated:

1. Complete the LIVE office fully — every search, read, and analysis the task
   asks of it. A missing key for one office is not a reason to do less work in
   the other.
2. Deliver those results as the normal deliverable.
3. Name the gap explicitly as a **missing-key gap**, never as a data or coverage
   finding: "EP coverage is missing because your EPO OPS key is not set" — not
   "no EP results were found", not "EP coverage is limited", not silence.
4. Ask for the missing key once, at the END of the turn, after the results.
5. **Never silently narrow scope.** A prior-art, novelty, patentability,
   freedom-to-operate, invalidity, or landscape task keeps the scope the work
   requires; the unsearched office is stated as an open gap in the deliverable,
   so no one mistakes a configuration detail for a clearance result.

**Keyless pivot — offer it as DIFFERENT data, never as a substitute.** These
need no patent-data key and stay live while an office is gated; label each for
what it actually is:

- PATSTAT (`flowleap-patstat`, `flowleap-patstat-graph`; in the app only with
  FlowLeap Data) — aggregates from a twice-yearly SNAPSHOT, not documents and
  not current. It does not answer "what prior art exists for this claim".
- Patent LAW (MPEP, EPC, guidelines) comes from the office's own page, read
  with your web tool (`flowleap-legal`; the app has no web tool), not from a
  FlowLeap key. It gives the legal standard, never what has been published or
  filed.
- `flowleap-academic` / `flowleap-npl` — scholarly LITERATURE. Papers are prior
  art in their own right, but they are a different corpus from patents.

Say plainly that this is different data, not a stand-in for the gated office's
live search.

**Resume — merge, do not restart.** When the user says they added the missing
key, re-run ONLY the previously gated office and merge its results into the
deliverable you already produced. Do not redo the live office's work. With the
CLI, a new key reaches the next command; in a chat client, a key stored on the
Patent-data keys page reaches the next tool call; on the desktop app, a key
saved in Settings reaches the next tool call; on the firm OS, the administrator
sets the key and tells the attorney. None needs a new session.

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
`endpoint_gone` row below). All of it requires an active plan — every new
account starts a free 7-day trial period at sign-up, no card — and shares
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
doctrine (proceed-then-ask, keyless pivot, resume): "The key gate" above.

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
  when complete). Steps already covered — e.g. a provider with a stored key
  (validate `source: "stored"`), or one the server has its own keys for — are
  omitted. Each step:

```json
{ "id": "store-epo-keys", "actor": "agent",
  "title": "Store the EPO consumer key and secret",
  "run": "flowleap keys set epo --key <k> --secret <s>" }
```

Stable step ids (public contract): `auth-login` (human), `mint-personal-token`
(agent — pending while auth is only a session token with no `fl_pat_` personal
token), `obtain-epo-keys` (human), `store-epo-keys` (agent),
`obtain-uspto-key` (human), `store-uspto-key` (agent; human, with the keys page
`url`, when the office rejected the stored key), `verify-keys` (agent),
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
local key presence (`"local"`, with a `note`). `keyValidation.providers`
(`{ epo, uspto }`) gives, per office, the validate `source` verbatim — `user`
(forwarded key), `stored`, `server` or `none`; null when that office was not
checked, and null for both on the local fallback.

**Agent-mediated sequence**: run `flowleap --json doctor`; for each step in
`nextSteps`, execute `actor: "agent"` steps yourself via their `run` command,
and relay `actor: "human"` steps (title + `url`) to the user; re-run doctor
until `ready` is true. One exception: run a `store-epo-keys` /
`store-uspto-key` agent step only with keys the user gave you without being
asked. Otherwise relay the FlowLeap Patent-data keys page
(https://www.flowleap.co/en/dashboard/keys; CLI and chat client only) — never
ask for the key value in the chat (see `flowleap-keys`).

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

## Office coverage of the text tools and the family-member workaround

**Office coverage** is a property of a tool: the offices it answers for, and
from which source. The three text tools serve two offices from EPO OPS and US
claims from BigQuery. The other document reads (`ops biblio`, `family`,
`legal`, `abstract`) are worldwide bibliographic; `get_citations` is worldwide
DOCDB citation data; `get_patent_image` serves EPO OPS images for the offices
EPO OPS holds.

- `ops claims` / `get_claims`: EP and WO from EPO OPS; US from BigQuery, in
  English only.
- `ops description` / `get_description`: EP and WO from EPO OPS. US description
  is not served.
- `get_fulltext` (no command): probes the EPO OPS holdings only, so it answers
  for EP and WO. A negative probe means EPO OPS holds no text, not that no text
  exists.

Eight offices have bibliographic, family and legal-event depth and **no text
source**: CN, JP, KR, CA, AU, FR, DE and GB. Backend ADR 0024 adds no source
until a customer names an office, so never call one of them "pending".

**Family-member workaround** for a publication at one of those offices, two
steps:

1. `flowleap --json ops family <number>` / `get_family` (the extended INPADOC
   family) to find the EP, WO or US member.
2. `flowleap --json ops claims <member>` or `ops description <member>` /
   `get_claims` or `get_description` on that member.

Two limits apply, always. The member's claims are not the office's claims: use
them for prior art and reading only, never for a freedom-to-operate,
infringement or validity verdict on that office's right, and label the text as
the member's. The member's text must itself be served: only EP, WO or US text
is served, and US description is not.

How likely the workaround is: the share of A filings 2015 to 2024 whose
extended family has an EP, WO or US member, measured on PATSTAT 2026 Spring.
Tell the human the share before you spend a family read.

| Office | Share with such a member |
|---|---|
| CA | 95% |
| AU | 88% |
| FR | 60% |
| DE | 55% |
| GB | 54% |
| JP | 48% |
| KR | 38% |
| CN | 13% |

The share for CN is low. For a CN prior-art question go to the abstract, the
bibliography and the citations first, and try the family after.

**The served failure.** A text read for an office with no text source answers
`FULLTEXT_UNAVAILABLE` (HTTP 404, exit 5; a data verdict, never retry). Under
`error.details`, `reason` is `no_source` and the `workaround` field beside it
is `{ tool: "get_family", note }`: act on that field, not on the message text. A
US publication the BigQuery table does not hold answers `no_source` with no
workaround (a holding gap). Reason `not_configured` means the server has no
BigQuery configured: a server configuration gap, no workaround, and
`server_info` shows it on the US `get_claims` row (`officeCoverage`).

## Safety

- Use `--dry-run` before executing mutating operations
- Add `--dry-run-redacted` when the request contains an unpublished invention,
  claim, document text, URL, or search query that should not enter logs
- Search queries are written by you, locally — an unpublished invention
  description never has to leave the machine to become a query (see
  `flowleap-patent`)
- Use `--verbose` to inspect request details (credentials are redacted)
- Never include credentials in commit messages or logs
