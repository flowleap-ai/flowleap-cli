---
name: flowleap-patent
description: Search patents — the publication index with plain words and filters (discovery, no key, no office quota), and EPO OPS with CQL you write yourself — field reference, the discriminating-term rule, and the mandatory count probe. Trigger when an agent needs European or worldwide patent search results, needs to turn an invention description into a CQL query, or wants to tune a query for recall versus precision. For US-specific ODP searches see flowleap-uspto.
---

# FlowLeap Patent

Auth and global flags: see `flowleap-shared`.

In a chat client with the FlowLeap connector or in the FlowLeap Patent AI app, call the tools named in the `flowleap-shared` tool table instead of these commands, and never run a `flowleap` command there.

No user needs a Patent-Data Key for a search. The index search uses no office
call. The EPO search uses FlowLeap's House keys inside the daily live data
budget; an own free EPO key is optional and lifts that limit. A
`liveUnavailableHint` or a `sharedQuotaHint` is a wait: do not send the search
again before its `retryAfterSeconds`. A `provider_keys_required` error still
comes from an older server: it is a **user-action stop for that office, never an
exhausted route**: do not fall back to web-scraped patent data, deliver whatever
the live office (US via `flowleap uspto search`) returns in full, name the gap as
a missing-key gap, and ask for the free key at the end of the turn. A zero-result
search is not a gate — reformulate and persist as normal. See `flowleap-keys`.

## Discovery first: the publication index

```bash
flowleap --json patent search --query '"solid state" battery -sulfide' --cpc H01M --applicant toyota --kind B --sort date
flowleap --json patent search --query 'lidar point cloud segmentation' --office EP,US --to 2018-02-28
flowleap --json patent search --query '*' --cpc H01M10/0525 --count-only
```

Start a search here. `flowleap patent search` without `--provider` is the index
search (`--provider index` is the same search, written in full). The
publication index is FlowLeap's hosted edition of
every publication of every office: number, dates, kind, English title and
abstract (CN through its English machine-translated abstract), CPC and IPC,
applicants, inventors and simple family. It runs the `search_patents` tool with
`provider: index`. It uses no office quota, no Patent-Data Key and no live data
budget.

| Flag | Description |
|------|-------------|
| `--query`, `-q` | Plain words: `"quoted phrase"`, `OR`, `-exclusion`. No CQL and no field codes. `*` with `--cpc`, `--applicant` or `--inventor` searches by filters only |
| `--cpc` | CPC subclass (`H01M`), main group (`H01M10`) or full symbol (`H01M10/0525`); comma-separated for a list. A partial subgroup (`H01M10/05`) matches only that exact symbol: use the main group |
| `--applicant` / `--inventor` | Words of the name, any case; all words must be in the name |
| `--office` | Publication office (country code), comma-separated: `EP,US,CN` |
| `--kind` | Kind-code prefix, comma-separated: `B` grants, `A` applications, or an exact kind such as `A1` |
| `--from` / `--to` | Publication date, inclusive, `YYYY-MM-DD`. For prior art, set `--to` to the day before the priority date |
| `--sort` | `relevance` (default) or `date` (newest first) |
| `--limit` / `--offset` | Page size (1-100, default 10) and rows to skip |
| `--count-only` | Print the total only |

The answer has `docs` (the item shape of the EPO search, plus `inventors`,
`cpc`, `familyId`, `kind`, `applicationNumber`, `filingDate`, `priorityDate`),
`total` (`totalIsLowerBound: true` when the count stopped at its cap),
`source: "publication_index"`, `asOf` and `attribution`. Table and human output
print the rows, then `Total:`, `As of:` and the `Source:` credit line.

Rules:

1. **Read `asOf`.** It is the edition date. A publication newer than `asOf` is
   not in the index. Add an EPO search for those:
   `flowleap --json patent search --provider epo --query '<CQL> and pd>=<asOf as YYYYMMDD>'`.
2. **Give the credit.** When you show index results, give `attribution.text`
   (CC BY 4.0).
3. **The index does not give legal status, claims or citations.** Read a
   candidate with `flowleap ops …` or `flowleap summary`.
4. A document with `abstractUnavailable: "no_english_abstract"` has no English
   abstract in the index: read it with `flowleap ops abstract`.
5. The count probe and the discriminating-term rule below apply to the EPO CQL
   search. In the index, use `--count-only` and the filters to control the size
   of a result.
6. **A server without the index.** A search without `--provider` and without
   an index filter then runs on EPO OPS: the CLI makes title/abstract CQL from
   the plain words and prints one note on stderr with that CQL. Tell the user
   that the index is not available on this server. With an index filter, or
   with an explicit `--provider index`, the CLI prints "The publication index
   is not enabled on this server" (in `--json`, an `indexUnavailableHint`): use
   the EPO search below.
7. The index filters and `--countries` do not mix: `--countries` is for the
   EPO search, `--office` is for the index.

### When to add `--provider epo`

- **Publications newer than `asOf`.** The index is one dated edition (rule 1).
- **CQL field precision.** A term in the title only, an exact applicant or
  inventor name, a classification with a date range in one statement, the
  count probe on a search statement.
- **An exact check.** One applicant's exact filing list or one family: the live
  EPO data is the authority.

Family, legal status, claims and citations are not a search: read them with
`flowleap ops …` (see `flowleap-ops`).

Write `--provider epo` on every CQL search. Without `--provider`, the CLI reads
a CQL query (a relation such as `ti=battery`) or `--countries` as an EPO search
and prints one note on stderr; a CQL query together with an index filter is
refused, because the two searches read the query differently.

## Search Patents (EPO OPS, CQL)

```bash
flowleap patent search --provider epo --query <CQL> [flags]
```

Runs the `search_patents` tool (`provider: epo_ops`) on the Tools facade — the
single agent surface for patent data. Returns results with publication number,
title, applicant, and date; the per-source provider route this used to call is
a retired endpoint. You never name the surface yourself: the command carries it.

| Flag | Description | Default |
|------|-------------|---------|
| `--query`, `-q` | EPO CQL query (required) — e.g. `ti="battery separator"` | — |
| `--limit` | Maximum results (1-100) | `10` |
| `--countries` | Country filter, comma-separated (e.g. `EP,WO`) | none |
| `--count-only` | Only report the result total (cheap probe, no documents) | `false` |

Jurisdiction is set with `--countries` in the EPO search (`--office` in the
index search) — `patent search` has no `--source` flag
(that flag belongs to `academic search`, for `scholar` vs `arxiv`).

For US-specific searches use `flowleap uspto search` (ODP Lucene syntax).

### Response Format (JSON)

Returns an array of results, each carrying a document identifier, title,
applicant(s), publication date, and abstract, for example:

```json
[
  {
    "docId": "EP1234567.A1",
    "title": "Solar Panel with Improved Efficiency",
    "applicants": ["SolarCorp Inc."],
    "publicationDate": "20240115",
    "abstract": "..."
  }
]
```

Field names are illustrative — inspect a live `--json` response for the exact
keys. Strip the kind suffix (`EP1234567.A1` → `EP1234567`) to use the document
identifier as the `<patent-number>` argument to `ops` and the tools facade.

## Writing the Query — You Write the CQL

There is no server-side query builder: you turn the description into CQL
yourself, with the three steps below. None of them is optional.

### Step 1 — extract the terms (mandatory, before any CQL)

List every specific noun phrase in the description: materials, mechanisms,
subject matter ("sulfide glass ceramic", "foreign object detection", "prior
art"). These are your candidate discriminating terms. For each one you leave out
of the query, state why. Uncertainty about phrasing ("glass ceramic" vs
"glass-ceramic") is a reason to OR both forms, never to drop the term. The most
common failure is dropping the phrase that *is* the invention.

### Step 2 — write the query

Every query needs at least one **discriminating** term — one that separates this
invention from the millions of generic patents in its technology area. A CPC
code is never discriminating: it names a neighbourhood, not a house. The one
exception is a prior-art classification-only track: a main group AND two or
three specific indexing codes (2000-series, e.g. `cpc=F15B7/006 AND
cpc=F15B2211/7053 AND cpc=F15B2211/785`) is a set small enough to read in
full, and it finds documents whose wording no keyword query matches (see
`recipe-prior-art-search`, Step 2). For "AI for
patent analysis", the discriminating term is `ta="patent analysis"` — never just
`ta="artificial intelligence"`, and `ic=G06N` alone returns every
machine-learning patent ever filed.

```bash
flowleap patent search --provider epo --query 'pa=GOOGLE* AND ta="machine learning" AND ic=G06N' --limit 20
```

**Fields** — do not guess field names; every field in your query must be here:

| Field | Meaning | Wildcards |
|-------|---------|-----------|
| `ta` | title + abstract combined — **preferred**; costs one term instead of `ti` + `ab` | yes |
| `ti` / `ab` | title only / abstract only | yes |
| `pa` | applicant (`pa=GOOGLE*`, `pa="TESLA INC"`) | yes |
| `in` | inventor (`in=SMITH*`) | yes |
| `ic` / `cpc` / `cl` | IPC / CPC / all classifications (`ic=G06N`, `cpc=G06N3/08`) | **no** |
| `pn` / `ap` / `pr` | publication / application / priority number | **no** |
| `ct` | citation — documents citing this one (`ct=EP1234567`) | no |
| `pd` | publication date — `pd=2023`, `pd>=2020`, `pd within "2020 2023"` | no |

**Hard constraints — these produce API errors, not bad results:**

- Maximum ~10 terms per query (`MaximumTotalTerms` beyond that).
- No wildcards on `ic`/`cpc`/`cl`/`pn` (`TruncationForbidden`).
- Terms must be at least 3 characters — OPS rejects shorter prefixes with
  `400 CLIENT.PrefixTooShort`. Spell the word out (`ti="ultraviolet"`, not
  `ti="uv"`).
- Operators `AND`/`OR`/`NOT` uppercase; phrases in `"double quotes"`.
- **Grouping: parentheses group complete `field=value` clauses. Repeat the
  field inside the parentheses; never put the parentheses inside the value.**

```text
(ic=H02J OR ic=B60L)                        valid — covers two classes
(ta="glass ceramic" OR ta="glass-ceramic")  valid — covers both word forms
ta=(turbine OR blade)                       NOT CQL — hard OPS 404
```

**Choosing terms:** `ta` carries the discrimination (the specific subject
matter, not the technology area). A classification is never discriminating on
its own — pair it with a `ta` term, and when the invention spans classes, OR
the classes: `(ic=G06N OR ic=G06F)`. Aim for two or three discriminating terms:
one extra term routinely cuts a count by two orders of magnitude. For
combination inventions ("drone inspecting turbine blades"), the *pair* of terms
is the discrimination — `ta=drone AND ta="turbine blade"` — so keep both;
dropping the category word loses the invention.

### Step 3 — probe the count (mandatory, before trusting any results)

Use `--count-only`: it asks for range 1-1 with `details=false` (no
per-document bibliography fan-out), so a probe is cheap — a count is all you
want here:

```bash
flowleap --json patent search --provider epo --query 'ta="foreign object" AND ta=charging' --count-only
```

The JSON payload is `{ query, total }`. (Equivalent raw-tool probe:
`flowleap --json tools run search_patents query='…' range=1-1 details=false`.)
Note: `total` counts the full CQL result set — a `--countries` filter narrows
the returned documents, never the total.

- **Over ~1,000 hits:** too broad — add the next discriminating term from your
  Step 1 list and probe again.
- **Under 10:** too narrow — drop the classification, OR in synonyms, use the
  parent CPC class, widen dates.

A query you never probed is a guess; in a prior-art search a query returning
thousands of hits instead of tens means the closest art is never seen. A
prior-art search starts broad and narrows — you cannot notice what a too-narrow
query never returned.

**Reading the set.** Results come back newest first, not by relevance, and
`--limit` stops at 100: older art is at the end. A set you rely on is read in
full by paging `flowleap --json ops search --cql '<query>' --start 101 --end
200` (and so on), or narrowed until it can be. Never judge a set from its first
page. OPS returns one publication per family, so a document you expect may
appear as another member (a CA or WO publication for a US one): compare by
family.

### Recall vs precision

A judgement, not a setting:

- **Broad** (recall — novelty, FTO): drop the classification, widen dates, OR
  in synonyms, use the parent CPC class.
- **Precise** (precision — closest few documents to read in full): add a second
  `ta` term, narrow the classification to a subgroup, tighten dates.
- **Balanced** (default): one discriminating `ta` term, one classification, one
  date bound.

## Verify CPC Codes — Never Guess

CPC reclassifies (photovoltaics moved from `H01L31` to `H10F` in 2023 and the
backfile was rewritten), so a remembered code may silently return the wrong
corpus. The official, version-stamped scheme is queryable:

```bash
flowleap patstat query "SELECT symbol, title FROM flowleap.cpc_scheme WHERE title ILIKE '%photovoltaic%' ORDER BY symbol LIMIT 15" --question "candidate CPC codes for photovoltaics"
```

Read the results at the right level: a 4-char class carries only the headline
(`H10F` = "inorganic semiconductor devices sensitive to radiation"); the
specific technology titles live in its **groups** (`H10F10/00`, `H10F71/00` …).
Match keywords against group titles, then search with the 4-char class
(`ic=H10F`) or the exact group (`cpc=H10F10/00`).

## Workflow: Description to Patent Results

1. Extract the candidate terms from the description (Step 1).
2. Write the CQL (Step 2) and probe the count (Step 3); refine until workable.
   Show the statement to the user as a Search Statement block — see
   [references/search-statement.md](references/search-statement.md).
3. Run the search: `flowleap --json patent search --provider epo --query "<CQL>" --limit 20`.
   The JSON payload is `{ total, docs }`. Results arrive in EPO OPS default
   order (no relevance ranking — recent publications tend to come first), so
   read the whole page, not just the top hits; the CQL itself is the only
   relevance control.
