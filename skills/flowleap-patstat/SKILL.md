---
name: flowleap-patstat
description: Portfolio Analytics AND guarded SQL over the PATSTAT snapshot — structured-criteria aggregation by named applicant, CPC/IPC class, office, year, family, and grant status, with harmonized entity resolution and Data Edition provenance; plus agent-written SELECTs against the flowleap.* semantic views for any aggregate the typed commands don't cover (landscapes, grant rates, citation impact, inventor analytics, priority/continuation chains and divisional share, decoded legal events such as EP oppositions and per-state lapses, concept-to-CPC text discovery that derives codes from the corpus instead of a hardcoded table, and INPADOC extended-family coverage). Trigger when an agent needs a named applicant's filing portfolio, structured-criteria corpus counts (not free-text search), what descended from a filing, which codes a legal concept spells out to at an office, which CPC codes a concept actually uses, where an extended family reaches, any other PATSTAT aggregate, or any number that must carry a PATSTAT edition citation.
---

# FlowLeap Patstat (Portfolio Analytics)

Auth and global flags: see `flowleap-shared`.

PATSTAT is a **named non-facade exception**: it keeps its own surface instead of
running on the Tools facade, and needs no patent-data key. Nothing here was
touched by the provider-route retirement.

## Which engine? — the three-way routing rule

FlowLeap runs three analytics engines, split by *criteria shape*, not by
metric:

- **Topic Analytics** (`flowleap analytics`, the Google-Patents corpus
  engine) — the question's essential criterion is **free-text keywords** over
  title/abstract ("quantum computing filings over time"). Publication-level
  counts, substring name matching, per-query cost.
- **Portfolio Analytics** (`flowleap patstat`, this skill, the PATSTAT
  engine) — the question is expressible in **structured criteria**: named
  applicant (entity-resolved, harmonized names), CPC/IPC class, office, year,
  family, grant status. Family-level counting, zero marginal cost.
- **Graph Analytics** (`flowleap patstat graph …` → `flowleap-patstat-graph`)
  — the question is about **a named node and the relationships around it**:
  who cites EP3477840, the citation/family path between two patents, where a
  family has coverage, an applicant's co-applicant network. Typed nodes and
  edges, each with a confidence tag and row-level provenance.

Routing rule: free text → `flowleap analytics`; structured criteria,
especially a named company → `flowleap patstat`; a *connection* rather than a
count → `flowleap-patstat-graph`. If the answer is a table of counts it is
here; if it is who-links-to-what, it is traversal — go across. Individual
documents (one known publication or application) are none of the three — use
the search/retrieval skills (`flowleap-patent`, `flowleap-uspto`,
`flowleap-ops`).

**Keyless, but not a stand-in.** PATSTAT needs no patent-data key, so it stays
live when EPO OPS or USPTO ODP answers `provider_keys_required`. You may offer it
to keep work moving — framed for what it is: aggregate counts from a
twice-yearly snapshot, not documents and not current. It never answers "what
prior art exists for this claim", and a PATSTAT table never closes a missing-key
gap in a prior-art, FTO, or invalidity deliverable. See `flowleap-keys`.

Note that `patstat portfolio` and `graph applicant` draw entity boundaries
differently: `portfolio` groups by name-prefix aliases, `graph applicant`
takes one harmonized `psn_id`. They may disagree about where one company ends
and another begins — always say which produced a number.

## Portfolio

```bash
flowleap --json patstat portfolio "Siemens AG" --from-year 2015 --to-year 2023
```

Response shape: a quotable `summary` line first — relay it verbatim before
adding any narrative — then filings-by-year/office/grant-status aggregate
tables, then a `data_edition` provenance line.

## Ambiguous applicant (422)

An unresolved applicant name returns HTTP 422 with a candidate list. This is
an **interaction step, not a retryable error**: render every candidate to the
user in both `--json` and human output, and **never auto-pick one**. Once the
user picks, re-run with the exact candidate name and pin that exact string —
a caller that needs to repeat the query (e.g. a `recipe-custom-dashboard`
script) hard-codes the resolved name as a constant so the choice is made once,
not re-asked on every run.

## Data Edition

PATSTAT is published in discrete snapshot editions (~twice a year). Every
Portfolio Analytics answer carries its `data_edition` — treat Portfolio
Analytics as a snapshot with a name, not live data. Two answers are only
comparable within the **same** `data_edition`; always surface the edition
alongside any number quoted from this skill.

## Guarded SQL (Layer 2) — aggregates beyond the typed commands

For aggregate questions no typed command answers — technology landscapes by
CPC ("who dominates solid-state electrolytes"), grant rates, citation-impact
rankings, inventor analytics, family/jurisdiction coverage — write **one SQL
SELECT** against the `flowleap.*` semantic views and run it through the
deterministic backend gate (single-SELECT parse check, flowleap-only
allowlist, EXPLAIN cost ceiling, 5,000-row/5 MB hard caps, 20 s timeout;
budget 10 queries/min).

The mandatory workflow, in order:

1. **Examples first — don't write SQL you don't need:**

   ```bash
   flowleap patstat docs --section examples
   ```

   Verified question→SQL pairs. If one matches, reuse its SQL; if it carries
   `promoted_to`, use that typed command/endpoint instead.

2. **Fetch the schema and conventions — never work from memory:**

   ```bash
   flowleap patstat docs --section semantic-model
   ```

   The served YAML is the single authoritative source: logical views and
   columns, metric formulas, join paths, caveats, and the
   `interpretation_conventions` block (default counting units and year
   bases, the ask-when-material rule). Apply it as served — this skill
   deliberately does not restate it, so it can never drift.

3. **Run, always sending the user's question verbatim** (it feeds the
   query-review pipeline that turns good queries into verified examples):

   ```bash
   flowleap patstat query "SELECT office, COUNT(DISTINCT family_id) AS inventions FROM flowleap.applications a JOIN flowleap.applicants ap ON ap.application_id = a.application_id WHERE UPPER(ap.name) LIKE 'SIEMENS%' GROUP BY office ORDER BY inventions DESC" --question "where does Siemens hold the most inventions?"
   ```

   Schema-qualify every table as `flowleap.<view>`. No LIMIT needed — the
   backend caps rows and errors (never truncates) past the cap.

4. **On a `patstat_sql_*` error, fix ONCE, then stop.** The error message
   carries the exact parser/Postgres detail plus the recovery instruction —
   follow it, re-run with `--retry-of <code>`, and after a second failure
   report the error instead of looping. `patstat_busy` is different: back
   off a few seconds and retry the SAME SQL — it is load, not a SQL problem.

5. **Present with the interpretation stated** ("counted as DOCDB families by
   earliest filing year") and the `data_edition` named. Surface any
   `patstat_sql_expensive` warning as a heaviness note. Full step-by-step:
   `flowleap patstat docs --workflow guarded-sql`.

Entity disambiguation in guarded SQL: no 422 here — probe candidates with a
cheap `SELECT name … LIKE 'X%' GROUP BY name` query first, and apply the same
never-auto-pick rule as the portfolio flow when candidates diverge.

## Widening recipes — chains, legal events, text discovery, INPADOC coverage

Four view families the typed commands do not reach, each with a worked recipe
in `references/widening-recipes.md`. The SQL there is byte-identical to the
backend's verified queries, so the file and `patstat docs --section examples`
agree; the served copy also carries the live status and the measured
fingerprint. Read the reference before writing chain, legal-event, text or
extended-family SQL of your own. The rules below are the ones that decide
whether the number is right, so they live here rather than one hop away.

**Chains** (`flowleap.priorities`, `flowleap.continuations`) —
*count over the DOCDB family, walk the chain for descent.* A family is a set
of applications sharing priorities; a chain is a path through the edges. Both
views point child → parent: filtering `application_id` walks **up** and rides
the primary key, filtering `priority_application_id` / `parent_application_id`
walks **down** and costs one scan of a 54M-row table **per hop** — descent has
no index, so bound the hops (2 is the proven bound, 3 will not run) and never
write an unbounded recursive walk. Dedupe on `application_id`: the hops
overlap and raw edge rows inflate the count by ~47%. Match on the decoded
`link_kind` (`divisional`, `continuation`, `continuation-in-part`,
`internal-priority`, …, `unknown` for a blank code), never on the raw
`link_type`. Recipes: descendants of an application within 2 hops; divisional
share per applicant in a CPC prefix.

**Legal events** (`flowleap.legal_events`, `flowleap.legal_event_codes`) —
*opposition is not one category.* Enumerate the codes from
`flowleap.legal_event_codes` for the office and the concept **first**, then
filter on that explicit list; never aggregate on `event_category` as if it
were the concept (69 EP opposition codes sit across categories L, W and Y, and
EP category H mixes lapse with revocation, expiry, annulment and surrender).
Curate the shortlist by hand — sibling codes mean the opposite. Count DISTINCT
applications, not event rows: EP `26` and `PLBI` both mean OPPOSITION FILED
and co-fire, so rows inflate by 215%. PATSTAT's no-value sentinels are already
scrubbed to NULL, so `IS NULL` is the right test. Anchor on a bounded
application set before touching the 519M-row event table — a ~4k-application
portfolio returns in ~2 s, a 25k one exceeds the 20 s timeout. And it is
as-of-edition: every legal-event answer must send the user to the
`get_legal_status` tool for live status. Recipes: EP oppositions against an
applicant's grants by year; lapses per year and state (PG25/VS25 only).

**Text discovery** (`flowleap.application_texts`) — *discovery returns
identifiers, never document text as the answer.* Turn the hits into an
aggregate or a shortlist; send every document read to `flowleap-ops` or
`flowleap-uspto`. The match idiom is exact —
`to_tsvector('english', title) @@ plainto_tsquery('english', $q) AND title_lang = 'en'`,
both halves, or the partial expression index is not used and the gate rejects
the seq scan. The abstract variant is the same idiom on `abstract` /
`abstract_lang`; its index exists and it is the more expensive, higher-recall
path. **EXPLAIN does not bound a GIN seed** — Postgres estimates ~1 row per
match however broad, so the cost gate bounds nothing here; the 20 s timeout
and the 5,000-row cap do, and you must narrow with `ipr_type = 'PI'`, a year
floor and/or an office before joining out. Three traps: circularity (the seed
picks the corpus — cross-check a second phrasing), generic co-occurring codes
(rank 1 is usually a tag such as `Y02E60/10`, the discriminating code is rank
2 — join `cpc_scheme` and read the top few), and reclassification mix (legacy
and current codes coexist). Recipes: concept → CPC distribution; top
applicants for a concept. This lookup **outranks** the hardcoded CPC tables in
the VS Code app skills (`patent-search/references/cql-reference.md`,
`prior-art/references/cpc-classification.md`) — those are now a last-resort
fallback for when PATSTAT is unavailable.

**INPADOC coverage** (`flowleap.inpadoc_family_members`) — *count over DOCDB,
cover over INPADOC.* The extended family holds applications sharing a priority
directly **or indirectly**, so it answers "which offices is this right alive
in" and nothing about how many inventions there are. Counting inventions over
it inflates the answer 6× on the proven anchor. Keep
`COUNT(DISTINCT family_id)` over `flowleap.family_members` as the invention
count, name which family you used, and never persist or cross-edition-compare
an `inpadoc_family_id` — PATSTAT recomputes it every edition. Recipe: where
the extended family of an application is in force, plus the
count-versus-cover check that measures the inflation factor before you quote a
number.

## patstat_unavailable

If the backend has no PATSTAT database configured, it returns a
`patstat_unavailable` error. Say so plainly ("backend has no PATSTAT dataset
configured") and stop — this is a deployment gap, not a transient failure; do
not retry.

Also available as `flowleap tools run patstat_portfolio …` once the backend
tool-registry entry lands — see `flowleap-tools`.

```bash
flowleap --json tools run patstat_portfolio applicant="<applicant name>"
```
