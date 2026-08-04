# PRD 0002 — Graph Analytics, CLI-first

**Status**: approved (grilling session 2026-08-04)
**Decision records**: flowleap-agent-v2 `CONTEXT.md` (three-engine glossary entry),
flowleap-agent-v2 `docs/adr/0007-graph-analytics-thin-verbs-no-analyst-tool.md`

## Context

flowleap-backend ships a PATSTAT graph engine (`src/lib/patstat-graph/`, routes
`GET /v1/patstat/graph/*`) that currently serves only the website analytics page. It answers a
question shape neither existing PATSTAT surface can: **a named node and the relationships around
it** — worldwide DOCDB citation networks (backward/forward, examiner vs applicant origin),
citation/family paths between two patents, family + priority networks, co-applicant networks.
Every edge carries a confidence tag (`EXTRACTED`/`INFERRED`/`AMBIGUOUS`) and row-level
provenance (`at=tls212:…`). Three of the six operations (`neighborhood`, `path`, `explain`)
return a token-budgeted line-per-fact `text` serialization built for LLM consumption, proven by
the backend's own analyst agent.

The domain glossary now names this the **Graph Analytics** engine, third alongside Topic and
Portfolio Analytics, routed by criteria shape: *free-text keywords → Topic; aggregate counts by
structured criteria → Portfolio; a named node and its connections → Graph.*

## Goals

1. Expose all six graph operations through the FlowLeap CLI as a thin 1:1 mirror:
   `flowleap patstat graph resolve|patent|applicant|neighborhood|path|explain`.
2. Ship a new multi-harness skill `flowleap-patstat-graph` with a traversal-shaped description,
   plus routing cross-references in `flowleap` (umbrella), `flowleap-patstat`, and
   `flowleap-citation`.
3. Fix the backend docs manifest: the graph routes join `/v1/patstat/docs`
   (`ENDPOINTS`/`WORKFLOWS`) so guide registries — and the future IDE api-guide seam — can serve
   them.
4. Keep the four skill copies in sync: canonical CLI → plugins → app-vendored, in the release
   motion.
5. (Follower, separate repo) One `patstat_graph` typed tool in the Patent Agent, six operations
   behind an `operation` parameter, relaying backend `text` verbatim, guide-fed.

## Non-goals

- **No `/v1/analyst` agent surface, ever** (ADR 0007). Agents use the verbs directly.
- No backend response caching (#203), no per-route rate limit, no `patstat_product` role cutover
  in this release — all deferred pending `patstat_graph_query_log` evidence / ops track.
- No new backend computation (community detection, centrality, etc.) — client work only, plus
  the manifest fix.

## Design constraints

- **Thin relay discipline** (same as guarded SQL): backend `text`, truncation notices, error
  envelopes, and `data_edition` are relayed verbatim, never rephrased. Human mode prints the
  verbs' `text` field raw; `--json` prints the typed `data`.
- **Typed error family**: `patstat_patent_not_found`, `patstat_entity_not_found` (404),
  `patstat_patent_ambiguous` (422, carries `candidates[]` — an interaction step, never
  auto-picked, mirroring `patstat_applicant_ambiguous`), `patstat_invalid_request` (400),
  `patstat_unavailable` (503). Dedicated rendering for ambiguity and unavailability; shared
  envelope + hint-box fallback otherwise; documented exit codes.
- **Snapshot honesty**: every rendered result names its Data Edition; current-legal-status
  questions defer to live document tools (OPS/USPTO), stated in skill + help text.
- **Citation-source split**: `flowleap-citation` = USPTO office-action enriched citations
  (US, X/Y/A relevance); Graph = worldwide DOCDB snapshot network. Both skills state it.
- **applicant vs portfolio caveat**: `portfolio` = filing counts + grant status via name-prefix
  alias grouping; `graph applicant` = one `psn_id` (from `resolve`) + co-applicants + top-CPC +
  jurisdictions. They may draw entity boundaries differently; skill prose owns the routing.

## Slices

1. Backend: graph routes join the `/v1/patstat/docs` manifest (flowleap-backend).
2. CLI: `graph resolve` end-to-end — command family scaffold, typed graph error rendering,
   ambiguity interaction, human/JSON output, exit codes, tests (tracer bullet).
3. CLI: agent verbs `neighborhood` / `path` / `explain` with `--token-budget`, verbatim `text`
   relay, tests.
4. CLI: composites `patent` / `applicant` — section tables, truncation notices, data-quality
   flags, edition line, tests.
5. Skill: `flowleap-patstat-graph` + cross-references + validator examples.
6. Sync: plugins re-sync + app re-vendor + skills-install refresh nudge.
7. (flowleap-agent-v2, parked follower) `patstat_graph` typed tool + api-guide sections +
   three-way prompt routing.

## Acceptance

- Live against the deployed backend: each of the six commands returns real data for a known
  anchor (e.g. `resolve EP3477840` → `pat:` anchor; `patent US5960411` shows 200-cap truncation
  with true totals; `path` between two related patents; `applicant` for a resolved `psn_id`).
- Ambiguous publication number prints candidates and exits with the documented code, no
  auto-pick.
- `--json` on every command emits the backend body unmodified.
- Skill passes the example validator; `check-drift.mjs` green after plugin sync.
- README command table + docs updated; `flowleap patstat docs --endpoint graph` (or per-verb
  names) serves the new manifest entries.
