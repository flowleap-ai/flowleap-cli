---
name: flowleap
description: Start here — the umbrella skill for the FlowLeap Patent AI CLI. Maps every command family (patent/USPTO/OPS/academic/NPL/legal/citation reads, analytics, OCR, claim analysis, one-call patent verbs, the tools facade, and the raw API escape hatch) and routes to the specialist skills. Trigger when a user asks an agent to use FlowLeap, query the FlowLeap backend, verify local or deployed FlowLeap API health, run patent research commands, or debug FlowLeap CLI/API behavior.
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

Server-covered provider keys never appear in `nextSteps` — the list is only
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
- **Provider keys (EPO OPS / USPTO ODP BYOK)** → `flowleap-keys`. A
  `provider_keys_required` / `provider_keys_invalid` hint means a human must sign
  up in a browser — stop and ask.
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
- **Agent-first tool facade** (`flowleap tools list|describe|run …`) and the
  one-call verbs `summary`, `timeline`, `compare` → `flowleap-tools`.
- **Document utilities** — `flowleap figures <doc>`, `flowleap convert-number
  <doc> --to docdb`, `flowleap analytics --keyword …`, `flowleap ocr <file>`,
  `flowleap analyze-claim --file claim1.txt --focus full`.
- **Raw API escape hatch** — `flowleap --json api request get /v1/health`. Use
  high-level commands first; never run a live `post`/`put`/`patch`/`delete`
  unless the user asked for that specific write, and prefer `--dry-run`.

## Install Skills

```bash
flowleap skills install              # → ~/.claude/skills
flowleap skills install --project    # → .claude/skills
flowleap skills install --dir <path> # any other agent
```

## Keep FlowLeap Updated

One command upgrades the CLI on any install channel (npm, Homebrew, install.sh
binary, cargo) — no need to know which one you're on:

```bash
flowleap upgrade --check   # channel + versions, no changes (add --json to branch on it)
flowleap upgrade           # upgrade in place; skill content refreshes separately
flowleap skills update     # refresh installed skill files after upgrading
```

`upgrade --check --json` returns `{ channel, currentVersion, latestVersion,
updateAvailable, command }` so an agent can decide whether to act.

## Skill Map

- Shared reference: `flowleap-shared` (auth, flags, config), `flowleap-auth`, `flowleap-keys`
- Data sources: `flowleap-patent` (EPO CQL), `flowleap-uspto` (ODP Lucene), `flowleap-ops` (EPO documents), `flowleap-academic`, `flowleap-npl`, `flowleap-legal`, `flowleap-citation`, `flowleap-patstat` (Portfolio Analytics), `flowleap-patstat-graph` (Graph Analytics), `flowleap-tools` (facade)
- Personas: `persona-patent-attorney`, `persona-ip-analyst`, `persona-researcher`, `persona-startup-founder`
- Recipes (search/analysis): `recipe-prior-art-search`, `recipe-patent-landscape`, `recipe-freedom-to-operate`, `recipe-claim-analysis`, `recipe-patent-to-report`, `recipe-academic-literature-review`
- Recipes (prosecution/litigation, full pack only): `recipe-office-action-response`, `recipe-invalidity-analysis`, `recipe-infringement-charting`, `recipe-claim-drafting`, `recipe-invention-disclosure`, `recipe-audit-report`
