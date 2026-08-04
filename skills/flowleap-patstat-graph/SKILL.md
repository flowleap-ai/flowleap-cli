---
name: flowleap-patstat-graph
description: Typed graph queries over the PATSTAT snapshot — citation networks, worldwide family/coverage, applicant landscapes, shortest citation/family paths, and node explanations, every edge carrying a confidence tag (EXTRACTED/INFERRED/AMBIGUOUS) and a PATSTAT row provenance ref. Trigger when an agent needs a specific patent's citation neighborhood or family coverage, "how are patent X and patent Y connected", why a patent matters (connection profile), or an applicant's landscape as typed graph data — anything entity-centric with provenance, as opposed to corpus aggregates (flowleap-patstat) or document retrieval (flowleap-patent/flowleap-ops).
---

# FlowLeap Patstat Graph (Graph Engine)

Auth and global flags: see `flowleap-shared`. All endpoints are plain
authenticated GETs — call them through the raw API passthrough (method is
lowercase):

```bash
flowleap --json api request get "/v1/patstat/graph/patent/EP3477840"
```

The passthrough wraps every response as `{ ok, status, body, contentType }` —
the endpoint's payload is under **`body`** (so a verb's agent text is
`body.text`, a composite's metadata is `body.meta`, and error envelopes are
`body.error`). Against a non-production backend (pre-merge testing), add
`--base-url http://localhost:8000`.

## Routing rule (vs. the other PATSTAT skills)

- **This skill** — entity-centric graph questions about *specific* patents or
  applicants: who cites EP3477840, where its family has coverage, the path
  between two patents, an applicant's landscape as structured data.
- **`flowleap-patstat`** — corpus *aggregates* by structured criteria
  (portfolio counts, guarded SQL for landscapes-by-numbers). If the answer is
  a table of counts, go there.
- **`flowleap-patent` / `flowleap-ops` / `flowleap-uspto`** — document
  retrieval (biblio, claims, full text, legal status) for a known publication.

## The six endpoints

| Endpoint | Query params | Purpose |
|---|---|---|
| `/v1/patstat/graph/resolve` | `q` (number or free text) | Number → the patent anchor; text → ranked applicant entities, largest portfolio first |
| `/v1/patstat/graph/patent/:number` | — | Full patent view: anchor, backward/forward citations, family, header (applicants/inventors/CPC) |
| `/v1/patstat/graph/applicant/:psnId` | — | Applicant landscape: filings/year, top CPC, jurisdictions, co-applicants |
| `/v1/patstat/graph/neighborhood` | `node`, `depth` (≤2), `edge_types` (csv), `token_budget` | Bounded expansion around any node |
| `/v1/patstat/graph/path` | `a`, `b`, `max_hops` (≤4), `token_budget` | Shortest citation/family path between two patents |
| `/v1/patstat/graph/explain` | `node`, `token_budget` | Node card + top connections, remainder grouped with true counts |

Node ids: `pat:<appln_id>`, `person:<psn_id>`, `family:<docdb_family_id>`,
`cpc:<symbol>`. `resolve` turns human inputs into these ids — start there
when you only have a publication number or a company name.

## Reading responses

The verb endpoints (`neighborhood`, `path`, `explain`) return
`{ success, text, data }`:

- **`text`** — line-per-fact plain text built for agent consumption:
  `EP3477840 --cites [EXTRACTED 1.0]--> DE4302443 at=tls212:530028653`.
  Quote these lines (with their confidence tag and `at=` ref) rather than
  re-deriving facts from `data`. Pass `token_budget` (default 2000) and
  respect `TRUNCATED` notices — they include narrowing hints specific to the
  verb; never present a truncated listing as complete.
- **`data`** — the typed graph (nodes/edges) when you need structure.

The composite endpoints (`patent/:number`, `applicant/:psnId`) return typed
JSON only. Same rules apply to their `meta`: `meta.truncation` lists every
capped listing with its TRUE total ("showing 200 of 2,244" — say so),
`meta.data_quality` flags sentinel/unknown dates (PATSTAT stores unknown
years as 9999 — never quote them as years).

## Confidence discipline

Every edge carries `confidence`:

- `EXTRACTED` (1.0) — a direct PATSTAT row. State as fact.
- `INFERRED` (0.75–0.85) — a derived join (harmonized-name grouping, extended
  family). Hedge it: "grouped under the harmonized entity…", never bare fact.
- `AMBIGUOUS` (≤0.3) — unresolved citations kept as ghost `doc:` nodes.
  Flag, don't omit — and never build conclusions on them.

Provenance `at=<table>:<key>` points at the PATSTAT row asserting the
relationship; carry it when the user needs to verify a claim.

## Ambiguity (422) and errors

Free-text `resolve` returning multiple entities, or an ambiguous publication
number, is an **interaction step, not a retryable error** — render the ranked
candidates (they include portfolio sizes and publications for deep-linking)
and let the user pick; never auto-pick. Other errors follow the FlowLeap
envelope (`error.code` in the `patstat_*` family); `503` means the graph
engine's database is unreachable — report it, do not retry-loop.

## Data Edition

Like all PATSTAT surfaces, answers are snapshots: carry
`meta.data_edition` alongside any number you quote, and only compare
numbers within the same edition.
