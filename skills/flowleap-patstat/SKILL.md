---
name: flowleap-patstat
description: Portfolio Analytics AND guarded SQL over the PATSTAT snapshot — structured-criteria aggregation by named applicant, CPC/IPC class, office, year, family, and grant status, with harmonized entity resolution and Data Edition provenance; plus agent-written SELECTs against the flowleap.* semantic views for any aggregate the typed commands don't cover (landscapes, grant rates, citation impact, inventor analytics). Trigger when an agent needs a named applicant's filing portfolio, structured-criteria corpus counts (not free-text search), any other PATSTAT aggregate, or any number that must carry a PATSTAT edition citation.
---

# FlowLeap Patstat (Portfolio Analytics)

Auth and global flags: see `flowleap-shared`.

## Topic Analytics vs Portfolio Analytics — routing rule

FlowLeap runs two aggregate-analytics engines, split by *criteria shape*, not
by metric:

- **Topic Analytics** (`flowleap analytics`, the Google-Patents corpus
  engine) — the question's essential criterion is **free-text keywords** over
  title/abstract ("quantum computing filings over time"). Publication-level
  counts, substring name matching, per-query cost.
- **Portfolio Analytics** (`flowleap patstat`, this skill, the PATSTAT
  engine) — the question is expressible in **structured criteria**: named
  applicant (entity-resolved, harmonized names), CPC/IPC class, office, year,
  family, grant status. Family-level counting, zero marginal cost.

Routing rule: if the question needs free text, use `flowleap analytics`; if
it is structured criteria — especially a named company — use
`flowleap patstat`. Individual documents (one known publication or
application) are neither — use the search/retrieval skills (`flowleap-patent`,
`flowleap-uspto`, `flowleap-ops`).

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
