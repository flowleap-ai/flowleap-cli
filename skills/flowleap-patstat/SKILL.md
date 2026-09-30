---
name: flowleap-patstat
description: Portfolio Analytics AND guarded SQL over the PATSTAT snapshot — structured-criteria aggregates by applicant/CPC/office/year/family/grant status, priority and continuation chains, decoded legal events, concept-to-CPC text discovery, INPADOC extended-family coverage, every number carrying a Data Edition citation. Trigger when an agent needs a named applicant's filing portfolio, structured-criteria corpus counts, what descended from a filing, which codes a legal concept spells out to at an office, which CPC codes a concept actually uses, where an extended family reaches, any other PATSTAT aggregate, or any number that must carry a PATSTAT edition citation.
---

# FlowLeap Patstat (Portfolio Analytics)

Auth and global flags: see `flowleap-shared`.

Every `patstat` command runs one **PATSTAT tool** on the Tools facade. An
agent that also has `flowleap mcp` sees the same tool under the same name, so
the command and the tool are one capability. PATSTAT tools need no patent-data
key; each one publishes its own gate and rate limit in the registry
(`flowleap --json tools describe <tool>`).

| Command | Tool | Gate |
|---|---|---|
| `patstat portfolio <applicant> [--from-year Y] [--to-year Y] [--offices top\|all]` | `patstat_portfolio` | plan |
| `patstat query "<SQL>" --question "<question>" [--retry-of <code>]` | `patstat_query` | plan, 10/min |
| `patstat docs` with one of `--section S`, `--workflow W`, `--endpoint E`, `--compact` | `patstat_docs` | sign-in |
| `patstat graph <verb> …` | `patstat_resolve`, `patstat_cpc`, `patstat_patent`, `patstat_applicant`, `patstat_technology`, `patstat_neighborhood`, `patstat_path`, `patstat_explain` | sign-in; see `flowleap-patstat-graph` |

`--json` prints the tool data verbatim. There is no top-level `success`; every
result carries `data_edition` and `attribution`. Through `tools run` the inputs
are snake_case (`from_year`, `to_year`, `retry_of`). `patstat portfolio`
sends `offices: "all"` for every office in `by_year_office`; the tool itself
defaults to `top` (the 8 largest offices plus one `OTHER` row per year), and
`by_year_office_scope.truncated` says when the matrix is cut. The tool caps
`applicant.other_matches` at 10; `applicant.other_matches_total` is the full
count:

```bash
flowleap --json tools run patstat_portfolio applicant="<applicant name>"
```

## Which engine?

FlowLeap has three analytics engines, split by *criteria shape*, not by metric:

| The question's essential criterion | Engine | Go to |
|---|---|---|
| A trend or landscape over free-text keywords, and the keywords are the whole criterion | Topic Analytics | `flowleap analytics` |
| Structured criteria (a named applicant, CPC/IPC class, office, year, family, grant status) giving a table of counts | Portfolio Analytics | this skill |
| A named node and the relationships around it (who cites it, the path between two patents, co-applicants) | Graph Analytics | `flowleap-patstat-graph` |

A **concept** that must become identifiers, CPC codes or a candidate set for a
structured aggregate comes here too, through text discovery (see the recipes
below): the text is the way in, never the answer. One known document is none
of the three engines: use `flowleap-patent`, `flowleap-uspto` or `flowleap-ops`.

A CPC/IPC-class landscape is Portfolio Analytics.
`flowleap patstat graph technology <cpc>` is its fast path: find the code with
`graph cpc` first. Use `patstat query` when the composite does not answer.

The served statement of the rule is step 1 of every served workflow
(`flowleap patstat docs --workflow <portfolio-analysis|guarded-sql|graph>`).

**Keyless, but not a stand-in.** PATSTAT stays live when EPO OPS or USPTO ODP
answers `provider_keys_required`. You may offer it to keep work moving, framed
for what it is: aggregate counts from a twice-yearly snapshot, not documents and
not current. It never answers "what prior art exists for this claim", and a
PATSTAT table never closes a missing-key gap in a prior-art, FTO or invalidity
deliverable. See `flowleap-keys`.

`patstat portfolio` and `graph applicant` draw entity boundaries differently:
`portfolio` groups by name-prefix aliases, `graph applicant` takes one
harmonized `psn_id`. They may disagree about where one company ends and another
begins, so always say which one produced a number.

## Data Edition and attribution

PATSTAT is published in snapshot editions, about twice a year. Quote the
`data_edition` with every number from this skill, and compare two numbers only
within the same edition. Keep the `attribution` line with any table you hand on.

## Portfolio

```bash
flowleap --json patstat portfolio "Siemens AG" --from-year 2015 --to-year 2023
```

The result opens with a quotable `summary` line: relay it verbatim before any
narrative. The aggregate tables by year, office and grant status follow. The
served procedure, with the caveats to surface, is
`flowleap patstat docs --workflow portfolio-analysis`.

**Ambiguous applicant.** An applicant name that matches several distinct
entities answers 422 `patstat_applicant_ambiguous`; in `--json` the candidates
are at `error.details.candidates`. This is an **interaction step**: show every
candidate to the user and let the user pick. Re-run with the exact candidate
name and pin that string. A caller that repeats the query (for example a
`recipe-custom-dashboard` script) hard-codes the resolved name as a constant, so
the user picks once.

## Guarded SQL — aggregates beyond the typed commands

Grant rates, citation-impact rankings, inventor analytics, jurisdiction
coverage and other aggregates that no typed command answers take **one SQL
SELECT** against the `flowleap.*` semantic views.

The procedure is served, and it is the source of truth:
`flowleap patstat docs --workflow guarded-sql`. Its steps map to these commands:

| Served step | Command |
|---|---|
| Verified examples first; reuse a match, or use its `promoted_to` command | `flowleap patstat docs --section examples` |
| Semantic model and interpretation conventions, applied as served | `flowleap patstat docs --section semantic-model` (the YAML is at `.yaml` in `--json`) |
| One SELECT, with the user's question verbatim | `flowleap patstat query "<SQL>" --question "<question>"` |
| The one retry | the same command plus `--retry-of <error code>` |

```bash
flowleap patstat docs --section examples
flowleap patstat query "SELECT office, COUNT(DISTINCT family_id) AS inventions FROM flowleap.applications a JOIN flowleap.applicants ap ON ap.application_id = a.application_id WHERE UPPER(ap.name) LIKE 'SIEMENS%' GROUP BY office ORDER BY inventions DESC" --question "where does Siemens hold the most inventions?"
```

Read the semantic model from the served YAML each time, never from memory; this
skill does not restate it. A LIMIT is not necessary: past the row cap the
backend answers an error, never a truncated table.

**You own the retry.** The CLI sends `patstat query` exactly once. It does not
resend on a 5xx, a timeout or a connection failure, so every typed error reaches
you with `error.code`, `error.message` and `error.details`:

- `patstat_sql_timeout`: a cold cache fails like a heavy query. Send the
  **same SQL** once more with `--retry-of patstat_sql_timeout`.
- Every other `patstat_sql_*` error: rewrite the SQL once from the message and
  details, then send it with `--retry-of <error code>`.
- `patstat_busy` or `patstat_unreachable`: capacity, not a gate verdict
  (backend ADR 0010). Wait `error.details.retry_after` seconds (the
  Retry-After header), then resend the **same SQL** with
  `--retry-of patstat_busy`. This resend does not count as the one retry.
- A second gate failure after your one rewrite: stop and report the typed
  error.

Entity disambiguation in guarded SQL has no 422. Probe the candidates first with
a cheap `SELECT name … LIKE 'X%' GROUP BY name` query. When the candidates
diverge, let the user pick, as in the portfolio flow.

## PATSTAT recipes — chains, legal events, text discovery, INPADOC coverage

Four view families the typed commands do not reach. The worked recipes live in
`references/patstat-recipes.md`, with SQL byte-identical to the backend's
verified queries and every measured number, cost and anchor. Read it before
writing chain, legal-event, text or extended-family SQL of your own;
`patstat docs --section examples` serves the same SQL with its live status.
Four rules decide whether the number is right:

- **Chains** (`flowleap.priorities`, `flowleap.continuations`) — *count over
  the DOCDB family, walk the chain for descent.* Descent has no index, so bound
  the hops and dedupe on `application_id`; match on the decoded `link_kind`,
  never the raw `link_type`.
- **Legal events** (`flowleap.legal_events`, `flowleap.legal_event_codes`) —
  *opposition is not one category.* Enumerate the codes from
  `flowleap.legal_event_codes` for the office and the concept **first**, count
  DISTINCT applications rather than event rows, anchor on a bounded application
  set, and send the user to the `get_legal_status` tool because these events
  are as-of-edition.
- **Text discovery** (`flowleap.application_texts`) — *discovery returns
  identifiers, never document text as the answer.* The match idiom is exact:
  `to_tsvector('english', title) @@ plainto_tsquery('english', $q) AND title_lang = 'en'`.
  EXPLAIN does not bound a GIN seed, so bound it yourself with
  `ipr_type = 'PI'`, a year floor and/or an office. This lookup **outranks**
  the app's own CPC reference tables, which are the last-resort fallback for
  when PATSTAT is unavailable.
- **INPADOC coverage** (`flowleap.inpadoc_family_members`) — *count over
  DOCDB, cover over INPADOC.* Counting inventions over the extended family
  inflates the answer; keep `COUNT(DISTINCT family_id)` over
  `flowleap.family_members` as the invention count and name which family you
  used.

## patstat_unavailable

A backend with no PATSTAT database configured answers `patstat_unavailable`.
Report it plainly ("backend has no PATSTAT dataset configured") and stop. It is
a deployment gap, not a transient failure.
