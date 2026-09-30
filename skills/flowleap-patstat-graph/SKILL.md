---
name: flowleap-patstat-graph
description: Graph Analytics over the PATSTAT snapshot — a named node and the relationships around it. Worldwide DOCDB citation networks (who cites a patent, examiner vs applicant origin), citation/family paths between two patents, family coverage, and an applicant's co-applicant network with top CPC and jurisdictions, every edge carrying a confidence tag (EXTRACTED/INFERRED/AMBIGUOUS) and a PATSTAT row provenance ref. Trigger when an agent needs a traversal answer about a specific patent or applicant — "who cites EP3477840", "how are patent X and patent Y connected", "why does this patent matter", "who does this company file with", where a family has coverage — as opposed to corpus aggregate counts (flowleap-patstat), free-text keyword analytics (flowleap analytics), or document retrieval (flowleap-patent/flowleap-ops).
---

# FlowLeap Patstat Graph (Graph Analytics)

Auth and global flags: see `flowleap-shared`.

Eight native commands under `flowleap patstat graph`. Each one runs one
**PATSTAT tool** on the Tools facade, the same tool `flowleap mcp` serves under
the same name. The graph tools need sign-in only: no plan and no patent-data
key. They share a 30 requests/minute limit.

## Routing: which engine answers this?

| The question's essential criterion | Engine | Skill |
|---|---|---|
| Free-text keywords over title/abstract | Topic Analytics | `flowleap analytics` |
| Structured criteria giving a table of counts | Portfolio Analytics | `flowleap-patstat` |
| **A named node and its relationships** | **Graph Analytics** | **this skill** |

A count goes to `flowleap-patstat`. A *connection* (who cites what, what links
two patents, who co-files with whom) is here. One known document's text, claims
or legal status is neither: use `flowleap-patent`, `flowleap-ops` or
`flowleap-uspto`. The served procedure, routing first, then resolve, is
`flowleap patstat docs --workflow graph`.

## The eight commands

```bash
flowleap patstat graph resolve EP3477840
flowleap patstat graph cpc "solid electrolyte"
flowleap patstat graph patent EP3477840
flowleap patstat graph applicant 98765
flowleap patstat graph technology H01M10/0562
flowleap patstat graph neighborhood pat:56123456 --depth 2 --edge-types cites,cited_by
flowleap patstat graph path EP3477840 US5960411 --max-hops 3
flowleap patstat graph explain EP3477840 --token-budget 4000
```

| Command | Tool | Answers |
|---|---|---|
| `resolve <query>` | `patstat_resolve` | Number → its `pat:<appln_id>` anchor; free text → ranked applicant entities with `psn_id`, largest portfolio first |
| `cpc <keyword>` | `patstat_cpc` | Technology keyword → ranked CPC symbols with scheme title and application count |
| `patent <number>` | `patstat_patent` | The whole patent picture in one call: anchor, backward/forward citations, family, applicants/inventors/CPC, priorities |
| `applicant <psn_id>` | `patstat_applicant` | One harmonized entity: filings by year, top CPC, jurisdictions, co-applicants |
| `technology <cpc>` | `patstat_technology` | One CPC area: top applicants, filing trend, grant rate by office, new entrants, seminal families, top inventors, geography |
| `neighborhood <node>` | `patstat_neighborhood` | Bounded 1–2 hop expansion, examiner citations ranked first |
| `path <a> <b>` | `patstat_path` | Shortest citation/family path between two patents |
| `explain <node>` | `patstat_explain` | Node card + top connections, the remainder grouped with TRUE counts |

Node ids are `pat:<appln_id>`, `person:<psn_id>`, `family:<docdb_family_id>`,
`cpc:<symbol>`. Through `tools run` the inputs are snake_case: `q`, `number`,
`psn_id`, `cpc`, `depth`, `edge_types` (an array), `max_hops`, `token_budget`.

## Start with resolve, or with cpc

A human input becomes a node id through an entry ramp, and the graph verbs
**refuse** rather than guess:

- A number or a company name goes through `resolve`. `applicant` takes a strict
  numeric `psn_id`, which only `resolve <name>` produces.
- A technology word goes through `cpc`, and one symbol from its list goes to
  `technology`. Take CPC codes from `cpc`, never from memory. When nothing
  matches, `cpc` prints one line and exits 0: try a shorter or different
  keyword.

`resolve` on a company name is a **pick-one list, not an answer**: present the
ranked candidates and let the user choose. It exits 0.

An ambiguous publication number has one answer per verb family, and none exits
0, so a script can never mistake a pick-one prompt for a resolved anchor:

- `resolve` answers 200 with `kind: "ambiguous"` and its candidates, and exits 1.
- `patent` answers 422 `patstat_patent_ambiguous`, with the candidates at
  `error.details.candidates`.
- `neighborhood`, `path` and `explain` refuse with 400
  `patstat_invalid_request`, whose message names the candidates in prose.

In each case, pass the `pat:` id the user meant.

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

**Human mode, `cpc` / `technology`** — `cpc` prints one line per candidate
(symbol, scheme title, application count). `technology` prints the area card,
then Top Applicants → Filing Trend → Grant Rate by Office → New Entrants →
Seminal Families → Top Inventors → the three geography rankings → notes and
data-quality flags → footer. Its notes say the most recent filing years are
incomplete: never read that tail as a decline.

**`--json`, every command** — the tool data verbatim, with no CLI envelope and
no `success` flag. `neighborhood`, `path` and `explain` answer
`{ text, data, data_edition, attribution }`; the other verbs answer their own
fields with `data_edition` and `attribution` beside them. A typed error prints
as `{ error: { code, message, details } }`.

**Which number is which.** Every node the backend answers with carries a
citable `publication` — the first grant where one exists, else the earliest
publication (may be `null` for an application with no publication at all).
`application` / `prior_application` are **deprecated**: they carry DOCDB's own
application-number format, which for a US application is a 6-digit serial
plus the 2-digit filing year (`US10374408 (A)` decodes to USPTO application
12/103,744 — it is not itself a lookupable number). EP happens to be the one
office where DOCDB's format equals the real application number, which is why
this stayed hidden until a US-bearing family was reported (flowleap-backend
#419). Prefer `publication` / `prior_publication`; read `docdb_application` /
`prior_docdb_application` only when you need the raw DOCDB string and know to
label it as such. In `--json` all four keys ride on every node — this is
additive, not a breaking change. Human-mode text already applies this rule:
it prints the citable publication, falling back to a `DOCDB appln …`-labeled
string only when no publication exists.

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
| `patstat_patent_ambiguous` (422) | `graph patent` only: a number matching several applications, with the candidates at `error.details.candidates` | Render the candidates and let the user pick. |
| `patstat_patent_not_found` / `patstat_entity_not_found` (404) | Nothing matches in the loaded edition | Check the number, or the input may postdate the snapshot. |
| `patstat_unavailable` (503) | No PATSTAT dataset configured on this deployment | Report plainly; do not retry-loop. |

Exit codes follow the CLI-wide table. A `3` means the credential is missing or
rejected — a human must run `flowleap auth login`; see `flowleap-auth` for the
verification states.

## Snapshot honesty

PATSTAT is a named snapshot, not live data. Every result names its Data
Edition: carry it with any number you quote, compare numbers only within the
same edition, and keep the `attribution` line with any table you hand on. For **current legal status** (in force, lapsed,
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
