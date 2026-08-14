---
name: flowleap-patstat-graph
description: Graph Analytics over the PATSTAT snapshot — a named node and the relationships around it. Worldwide DOCDB citation networks (who cites a patent, examiner vs applicant origin), citation/family paths between two patents, family coverage, and an applicant's co-applicant network with top CPC and jurisdictions, every edge carrying a confidence tag (EXTRACTED/INFERRED/AMBIGUOUS) and a PATSTAT row provenance ref. Trigger when an agent needs a traversal answer about a specific patent or applicant — "who cites EP3477840", "how are patent X and patent Y connected", "why does this patent matter", "who does this company file with", where a family has coverage — as opposed to corpus aggregate counts (flowleap-patstat), free-text keyword analytics (flowleap analytics), or document retrieval (flowleap-patent/flowleap-ops).
---

# FlowLeap Patstat Graph (Graph Analytics)

Auth and global flags: see `flowleap-shared`. Six native commands under
`flowleap patstat graph` — no raw `api request` escape hatch needed.

Like the rest of PATSTAT, this is a **named non-facade exception**: its own
surface, no patent-data key, untouched by the provider-route retirement.

## Routing: which engine answers this?

FlowLeap runs three analytics engines, split by **criteria shape**:

| The question's essential criterion | Engine | Skill |
|---|---|---|
| Free-text keywords over title/abstract | Topic Analytics | `flowleap analytics` |
| Structured criteria → a table of counts | Portfolio Analytics | `flowleap-patstat` |
| **A named node and its relationships** | **Graph Analytics** | **this skill** |

If the answer is a count, go to `flowleap-patstat`. If it is a *connection* —
who cites what, what links these two patents, who co-files with whom — it is
here. One known document's text, claims, or legal status is neither: use
`flowleap-patent` / `flowleap-ops` / `flowleap-uspto`.

## The six commands

```bash
flowleap patstat graph resolve EP3477840
flowleap patstat graph patent EP3477840
flowleap patstat graph applicant 98765
flowleap patstat graph neighborhood pat:56123456 --depth 2 --edge-types cites,cited_by
flowleap patstat graph path EP3477840 US5960411 --max-hops 3
flowleap patstat graph explain EP3477840 --token-budget 4000
```

| Command | Answers |
|---|---|
| `resolve <query>` | Number → its `pat:<appln_id>` anchor; free text → ranked applicant entities with `psn_id`, largest portfolio first |
| `patent <number>` | The whole patent picture in one call: anchor, backward/forward citations, family, applicants/inventors/CPC, priorities |
| `applicant <psn_id>` | One harmonized entity: filings by year, top CPC, jurisdictions, co-applicants |
| `neighborhood <node>` | Bounded 1–2 hop expansion, examiner citations ranked first |
| `path <a> <b>` | Shortest citation/family path between two patents |
| `explain <node>` | Node card + top connections, the remainder grouped with TRUE counts |

Node ids are `pat:<appln_id>`, `person:<psn_id>`, `family:<docdb_family_id>`,
`cpc:<symbol>`.

## Start with resolve

`resolve` is how a human input becomes a node id, and the graph verbs
**refuse** rather than guess:

- An ambiguous publication number is rejected with HTTP 400
  `patstat_invalid_request` whose message names the candidates in prose. The
  verbs never prompt — run `resolve` and pass the `pat:` id you meant.
- `applicant` takes a strict numeric `psn_id`, which only `resolve <name>`
  produces. A company name will not work there.
- Passing a company name to a verb is refused the same way ("not a patent
  node").

`resolve` on a company name is a **pick-one list, not an answer** — present
the ranked candidates and let the user choose. It exits 0. An ambiguous
*number* exits 1, so a script can never mistake a pick-one prompt for a
resolved anchor.

## Reading output

**Human mode, `neighborhood` / `path` / `explain`** — the command prints the
backend's line-per-fact `text` verbatim:

```
EP3477840 --cites [EXTRACTED 1.0]--> DE4302443 at=tls212:530028653
```

Quote those lines with their confidence tag and `at=` ref rather than
re-deriving facts. The Data Edition is in the header, and `TRUNCATED` notices
carry verb-specific narrowing hints — never present a truncated listing as
complete.

**Human mode, `patent` / `applicant`** — section tables, in a fixed order.
`graph patent`: Anchor → Applicants → Inventors → CPC Classifications →
Backward Citations (Patents → Non-Patent Literature → Unresolved) → Forward
Citations → DOCDB Family → Priority Claims → data-quality flags → edition and
attribution footer. `graph applicant`: entity card → Filings by Year → Top CPC
→ Jurisdictions → Co-Applicants → footer.

Every section header prints even when empty, showing `(none recorded)` — an
absent section means no data, never a parsing slip. Truncation reads literally
`Showing {shown} of {total} {label}.` — repeat the TRUE total when you quote
the list. `Filings by Year` is uncapped; do not imply a cap there.

**`--json`, every command** — the backend body exactly as it arrived, with no
CLI envelope around it. There is no `body` wrapper: read `text`, `data`,
`meta`, or `error` at the top level.

## Budgets and bounds

- `--token-budget` (default 2000) trims the `text` serialization only; `--json`
  data stays complete. Out-of-range values are **clamped silently** into
  100–20000 — never an error, so a huge budget simply gives you the maximum.
- `--depth` (1 or 2) and `--max-hops` (1–4) are **refused** with a relayed
  `patstat_invalid_request` message stating the valid range. Read the message
  rather than guessing.
- `--edge-types` takes a comma-separated subset of `cites`, `cited_by`,
  `in_family`, `has_applicant`, `has_inventor`, `classified_as`,
  `claims_priority`. Narrow with it instead of accepting a capped listing.

`path` reporting `found: false` is a **200 and exit 0** — the search ran and
there is no path within the hop limit. That is an answer, not a failure, and
it carries its own caveat: absence is not proof of unrelatedness. Unrelated
technology areas commonly have no path; raise `--max-hops` only if the
connection actually matters.

## Confidence discipline

Every edge carries `confidence`:

- `EXTRACTED` (1.0) — a direct PATSTAT row. State as fact.
- `INFERRED` (0.75–0.85) — a derived join (harmonized-name grouping, extended
  family). Hedge it: "grouped under the harmonized entity…", never bare fact.
- `AMBIGUOUS` (≤0.3) — unresolved citations kept as ghost `doc:` nodes.
  Flag, don't omit — and never build conclusions on them.

Provenance `at=<table>:<key>` points at the PATSTAT row asserting the
relationship; carry it when the user needs to verify a claim.

## Errors

| Code | Meaning | What to do |
|---|---|---|
| `patstat_invalid_request` (400) | Bad node, ambiguous input, or out-of-range bound | Read the relayed message — it states the fix. Resolve first if ambiguous. |
| `patstat_patent_ambiguous` (422) | **Composites only**: a number matching several applications, with structured `error.candidates` | Render the candidates; never auto-pick. |
| `patstat_patent_not_found` / `patstat_entity_not_found` (404) | Nothing matches in the loaded edition | Check the number, or the input may postdate the snapshot. |
| `patstat_unavailable` (503) | No PATSTAT dataset configured on this deployment | Report plainly; do not retry-loop. |

Exit codes follow the CLI-wide table. A `3` means the credential is missing or
rejected — a human must run `flowleap auth login`; see `flowleap-auth` for the
verification states.

## Snapshot honesty

PATSTAT is a named snapshot, not live data. Every result names its Data
Edition — carry it alongside any number you quote, and only compare numbers
within the same edition. For **current legal status** (in force, lapsed,
opposed) the snapshot is the wrong source: use the live document tools
(`flowleap ops legal`, `flowleap-uspto`).

Two boundaries worth stating to users:

- **`graph applicant` vs `patstat portfolio`** draw entity boundaries
  differently. `applicant` is one harmonized `psn_id`; `portfolio` groups by
  name-prefix aliases. They may disagree about where one company ends and
  another begins — say which one a number came from.
- **This engine vs `flowleap-citation`** are different citation universes.
  Here: the worldwide DOCDB citation network from the PATSTAT snapshot, with
  examiner-vs-applicant origin. There: USPTO office-action enriched citations
  — US only, with X/Y/A relevance categories. Neither is a superset; pick by
  whether the question is about worldwide structure or US examiner reasoning.
