---
name: flowleap
description: Start here — the umbrella skill for the FlowLeap Patent AI CLI. Maps every command family (patent/USPTO/OPS/academic/NPL/legal/citation reads, analytics, OCR, one-call patent verbs, the tools facade, and the raw API escape hatch) and routes to the specialist skills. Trigger when a user asks an agent to use FlowLeap, query the FlowLeap backend, verify local or deployed FlowLeap API health, run patent research commands, or debug FlowLeap CLI/API behavior.
---

# FlowLeap CLI

`flowleap` is the command layer for the FlowLeap Patent AI backend. This is the
entry-point skill: it verifies the setup and routes to the specialist skills.
Always pass `--json` for agent parsing; use `--dry-run` before protected calls.

## Start Here

```bash
command -v flowleap || true
flowleap --json doctor
```

Just checking the backend is up? `flowleap --json health api` is the public
readiness probe and reports the backend's `apiVersion` — no subscription, no
patent-data key, no provider call. **Never probe reachability with a search
command:** a search costs a provider call and fails for reasons unrelated to
reachability (no subscription, a key gate, a bad query), so it answers a
different question than the one you asked.

Doctor exits **0 iff the machine is ready to work** (backend reachable,
authenticated, nothing blocking); otherwise it exits 1 and its JSON lists the
pending blocking steps in `nextSteps`, each tagged with an `actor`. Drive
onboarding agent-mediated from that list:

1. Execute every `actor: "agent"` step yourself via its `run` command (e.g.
   `mint-personal-token`, `store-epo-keys`, `verify-keys`).
2. Relay every `actor: "human"` step to the user — its `title` plus `url`
   (provider signups) or the verification link from `flowleap --json auth
   login` (see `flowleap-auth`).
3. Re-run `flowleap --json doctor` until `ready` is `true` (empty
   `nextSteps`).

Server-covered patent-data keys never appear in `nextSteps` — the list is only
what actually blocks work. Full contract: `flowleap-shared`.

**CLI not installed?** Install it first — npm when Node is present, the
install script otherwise:

```
npm install -g flowleap
curl -fsSL https://raw.githubusercontent.com/flowleap-ai/flowleap-cli/main/install.sh | sh
```

Then authenticate: `flowleap auth login` opens a device-code sign-in (a free
FlowLeap account is created at flowleap.co if you don't have one). Headless
agents use a `fl_pat_` API token instead — see `flowleap-auth`.

`doctor` targets the production backend (https://api.flowleap.co) by default —
no `--base-url` needed. Developing the FlowLeap backend itself? Add
`--base-url http://localhost:8000` to point at a local server.

## Where Things Live

- **Auth, global flags, config, output formats** → `flowleap-shared`; login,
  token minting, and 401 self-heal → `flowleap-auth`.
- **Patent-data keys (EPO OPS / USPTO ODP BYOK)** → `flowleap-keys`. A
  `provider_keys_required` / `provider_keys_invalid` hint — raised from the
  backend codes `data_keys_required`, `patent_provider_key_invalid` and
  `odp_api_key_missing`, never from message text — means a human must sign up in
  a browser (free at both offices). That is a **user-action stop for that
  office, never an exhausted route**: do not substitute web-scraped data for it,
  for searches or single-document reads. Finish the live office, name the gap as
  a missing-key gap, ask at the end; PATSTAT, legal, and academic/NPL stay live
  keyless as *different* data. Full doctrine: `flowleap-keys`.
- **Patent search & CQL** → `flowleap-patent`; **USPTO ODP** → `flowleap-uspto`.
- **EPO document data** (biblio, claims, description, family, legal) → `flowleap-ops`.
- **Academic / non-patent literature** → `flowleap-academic`, `flowleap-npl`.
- **Patent-law RAG** → `flowleap-legal`; **enriched citations** → `flowleap-citation`.
- **Portfolio Analytics** (structured criteria — named applicant, CPC/IPC,
  office, year, family, grant status) → `flowleap-patstat`; free-text
  keyword analytics (`flowleap analytics`, Topic Analytics) stay below.
- **Graph Analytics** (`flowleap patstat graph …` — a named node and the
  relationships around it: who cites a patent, the path between two patents,
  family coverage, an applicant's co-applicant network) →
  `flowleap-patstat-graph`. Counts belong to Portfolio Analytics;
  *connections* belong here.
- **Tools facade** (`flowleap tools list|describe|run …`) and the one-call verbs
  `summary`, `timeline`, `compare` → `flowleap-tools`.
- **Document utilities** — `flowleap figures <doc>`, `flowleap convert-number
  <doc> --to docdb`, `flowleap analytics --keyword …`, `flowleap ocr <file>`.
- **Search queries and claim decomposition are yours to write** — there is no
  server-side query builder or claim analyzer. `flowleap-patent` carries the
  CQL method (term extraction, discriminating term, count probe);
  `recipe-claim-analysis` carries the claim-decomposition method.
- **Raw API escape hatch** — `flowleap --json api request get <path>`. It calls
  whatever path you give it, with no schema and no error contract of its own, so
  reach for it only when no command and no tool covers what you need. It is not
  a way back onto a retired endpoint: those answer `410 Gone` here too. Never
  run a live `post`/`put`/`patch`/`delete` unless the user asked for that
  specific write, and prefer `--dry-run`.

## One surface for patent data

Every data command — `patent`, `ops`, `uspto`, `citation`, `legal`, `npl`,
`academic`, `analytics`, `ocr`, and the one-call verbs — runs on the **Tools
facade**: named tools invoked through `/v1/tools`, one success envelope, one
error contract, a self-describing registry. The per-source **provider routes**
they used to call are **retired endpoints** — permanently removed, answering
`410 Gone` with a machine-readable successor, and never reused.

Named non-facade exceptions: PATSTAT (`flowleap patstat …`), auth/OAuth, key
validation, and the raw `api request` escape hatch.

Practical consequence: a command failing with exit **8** (`endpoint_gone`) means
your CLI build is stale, not that the capability is gone. Read
`endpointGoneHint.successor`, run `flowleap upgrade`, then
`flowleap skills update` — upgrading the binary without refreshing skill files
walks straight back into the same 410. Never retry the call itself; retirement
is permanent. Exit codes and hints in full: `flowleap-shared`.

## Install Skills

```bash
flowleap skills install              # → ~/.claude/skills
flowleap skills install --project    # → .claude/skills
flowleap skills install --dir <path> # any other agent
```

## Keep FlowLeap Updated

One command upgrades the CLI on any install channel (npm, install.sh binary,
cargo) — no need to know which one you're on:

```bash
flowleap upgrade --check   # channel + versions, no changes (add --json to branch on it)
flowleap upgrade           # upgrade in place
flowleap skills update     # refresh installed skill files (automatic on a raw-binary upgrade)
```

`upgrade --check --json` returns `{ channel, currentVersion, latestVersion,
updateAvailable, command }` so an agent can decide whether to act.

Skill files are copies, so an upgrade leaves them behind on every channel
except the raw binary, which refreshes them with the new build. Stale skill
files teach retired commands, so `flowleap doctor` reports them as a ✗ carrying
a `refresh-skills` next step.

## Skill Map

- Shared reference: `flowleap-shared` (auth, flags, config), `flowleap-auth`, `flowleap-keys`
- Data sources: `flowleap-patent` (EPO CQL), `flowleap-uspto` (ODP Lucene), `flowleap-ops` (EPO documents), `flowleap-academic`, `flowleap-npl`, `flowleap-legal`, `flowleap-citation`, `flowleap-patstat` (Portfolio Analytics), `flowleap-patstat-graph` (Graph Analytics), `flowleap-tools` (facade)
- Personas: `persona-patent-attorney`, `persona-ip-analyst`, `persona-researcher`, `persona-startup-founder`
- Recipes (search/analysis): `recipe-prior-art-search`, `recipe-patent-landscape`, `recipe-freedom-to-operate`, `recipe-claim-analysis`, `recipe-patent-to-report`, `recipe-academic-literature-review`
- Recipes (prosecution/litigation, full pack only): `recipe-office-action-response`, `recipe-invalidity-analysis`, `recipe-infringement-charting`, `recipe-claim-drafting`, `recipe-invention-disclosure`, `recipe-audit-report`
