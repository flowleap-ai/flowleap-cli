# MCP-only acceptance run 3 for PATSTAT, 2026-09-30

Issue: #98. This is the third run of the phase 1 verdict, after backend #486 put the provenance rule into the tool descriptions. Run 1 is [2026-09-30-mcp-only-patstat.md](2026-09-30-mcp-only-patstat.md), verdict FAIL. Run 2 is [2026-09-30-mcp-only-patstat-run-2.md](2026-09-30-mcp-only-patstat-run-2.md), verdict PARTIAL. The questions, the harness flags and the pass conditions are the same as in run 2.

**Overall verdict: PARTIAL.** Four questions pass, and question 4 passes in part. All four overall criteria of #98 hold: the right engine on all five, an edition on every number, at most one retry on question 5, and no web data. The two run 2 codes of question 4 are gone, but the answer names one legacy main group, H01L31, that no tool result contains and that the answer does not mark unverified. No tool result was redirected to a file.

| # | Question | Run 1 | Run 2 | Run 3 |
|---|---|---|---|---|
| 1 | Siemens filings by year and office | FAIL | PASS | PASS |
| 2 | Who dominates solid-state batteries | PASS | PASS | PASS (with the `patstat_cpc` limitation, #477) |
| 3 | Who cites EP3477840 and how | PARTIAL | PASS | PASS |
| 4 | CPC codes of perovskite tandem cell filings | PARTIAL | PARTIAL | PARTIAL |
| 5 | Siemens EP oppositions by year (cold-timeout case) | PASS | PASS | PASS |

## Changes since run 2

| Issue | What changed | Question |
|---|---|---|
| backend #486, PR #487 | The descriptions of `patstat_query`, `patstat_portfolio`, `patstat_cpc` and `patstat_technology` end with "Every CPC code or number in an answer must come from a tool result or be marked unverified." The semantic-model index carries the same sentence as `global_caveats.verified_values`. | 1, 2, 4, 5 |
| CLI 0.9.0 | The release of #107 and #110. The CLI surface of the MCP bridge is the same as in run 2. | all |

Before the run, the live `patstat_query` description ended with the rule sentence. The index result in the question 4 and 5 sessions carries `verified_values`.

## Harness setup

| Item | Value |
|---|---|
| Date | 2026-09-30, runs between 09:31Z and 09:40Z |
| Harness | Claude Code 2.1.285, `claude -p` (non-interactive) |
| Model reported | `claude-fable-5-1` (default, `--model` not set) |
| CLI | flowleap 0.9.0, commit `2976499`, `cargo build --release` |
| Backend `apiVersion` | `1.0.0+b66ff19794084e8d19b8f866bf0c8afa7d19629c` (50 tools) |
| Data Edition | PATSTAT 2026 Spring |
| `flowleap mcp --check` | 50 tools, 6 resources, 1 template, 3 prompts, stored session token |

MCP config, `acceptance/mcp.json`:

```json
{"mcpServers":{"flowleap":{"command":"/Users/abdullahatrash/flowleap/wt-98c/target/release/flowleap","args":["mcp"]}}}
```

The command line is the same as in run 2. Only the paths changed, to the `wt-98c` worktree and the `acceptance/runs-3/` output directory. The script is `acceptance/run.sh`.

Isolation is the same as in run 2. The `init` event of each session shows 52 tools (the 50 `mcp__flowleap__*` tools and the two MCP resource readers), `skills: []` and one connected MCP server. No session used the resource readers. Each question ran once, in a fresh session, and `permission_denials` is empty in all five. No run crashed. Four `q<N>.err` files are empty. `q2.err` holds one line from a local Claude Code session-end hook of the host terminal, which is not part of the run.

Question 5 ran first, for the cold-cache case, as in runs 1 and 2.

## Questions

"Largest result" is the size in chars of the largest tool result that the client received in that session. "Redirected" counts the tool results that Claude Code saved to a file as over its output limit. A grep of each stream for "exceeds maximum allowed tokens" finds zero in all five runs.

| # | Tool calls | Largest result | Redirected | Retries |
|---|---|---|---|---|
| 1 | 1 | 10,073 (`patstat_portfolio`) | 0 | 0 |
| 2 | 7 | 9,546 (`patstat_technology`) | 0 | 0 |
| 3 | 1 | 8,506 (`patstat_patent`) | 0 | 0 |
| 4 | 7 | 18,658 (`patstat_docs part=index`) | 0 | 0 |
| 5 | 10 | 27,259 (`patstat_docs section=examples`) | 0 | 1 identical resend |

### 1. "Show Siemens filings by year and office since 2016."

Pass condition: `patstat_portfolio` first and its reply read by the agent, the numbers from it, the edition cited.

Tools called, in order:

1. `patstat_portfolio` `{applicant: "Siemens", from_year: 2016, to_year: 2026, offices: "top"}`. The reply was 10,073 chars and the agent read it.

**Verdict: PASS.** One call. Every number in the answer is in the reply: 32,497 applications, 49 offices, the office totals, the year-by-office table and the five largest excluded entities. The answer cites PATSTAT 2026 Spring and marks 2024 and 2025 as publication lag, from the reply's caveat. The run 2 weakness recurs and is not marked: the answer says that the fall from 2016 to 2020 "is genuine and coincides with the spin-offs of Siemens Healthineers, Siemens Energy and Siemens Mobility". The reply lists these entities in `other_matches`, but it does not call them spin-offs or give a cause (backend #488).

### 2. "Who dominates solid-state batteries?"

Pass condition: `patstat_cpc` tried first, no invented code (a code from a served example or from `patstat_docs` counts as sourced), the edition cited.

Tools called, in order:

1. `patstat_cpc` `{q: "solid-state electrolyte"}`, 0 candidates.
2. `patstat_cpc` `{q: "solid electrolyte"}`, 0 candidates.
3. `patstat_cpc` `{q: "solid state battery"}`, 0 candidates.
4. `patstat_cpc` `{q: "electrolyte"}`, 502 `upstream_error`, statement timeout (#477).
5. `patstat_cpc` `{q: "secondary batteries"}`, 502 `upstream_error`, statement timeout (#477).
6. `patstat_technology` `{cpc: "H01M10/0562"}`. The area title is "Solid materials".
7. `patstat_technology` `{cpc: "H01M10/0585"}`. The area title is flat-cell construction. The agent read the title and dropped the code.

**Verdict: PASS.** `patstat_cpc` came first and found nothing (#477). H01M10/0562 comes from the served examples. H01M10/0585 is from model memory, but the agent checked it against the title in the tool result and excluded it. The top-ten table, the country shares, the trend and the grant rates are all in the `patstat_technology` reply, and the answer cites PATSTAT 2026 Spring. The answer now says that the CPC keyword lookup timed out, which run 2 did not say. The run 2 QuantumScape weakness recurs in a new form and is not marked. The citation counts are now sourced from `seminal_families`, but that list has no applicant. The answer puts "PolyPlus" and "QuantumScape" on two of the titles, and it says "Foundational patents are American", from model memory (backend #488).

### 3. "Who cites EP3477840 and how?"

Pass condition: `patstat_resolve` then `patstat_patent`, or `patstat_patent` directly; the origin stated; the edition cited.

Tools called, in order:

1. `patstat_patent` `{number: "EP3477840"}`.

**Verdict: PASS.** One call, and no OPS tool. The answer names the two citing documents, EP3796345A1 and CN112530684A, both from one Robert Bosch family, and both with origin `SEA` (examiner search report). It gives the backward citations as five `SEA` and four `APP` of nine, and the SI family members. Every fact is in the reply. The answer cites PATSTAT 2026 Spring.

### 4. "Which CPC codes do 'perovskite tandem cell' filings actually use?"

Pass condition: `patstat_docs part=index` and/or a view before `patstat_query`; text-discovery SQL; the codes read from tool results with their titles; no unsourced code; the edition cited. For run 3: every CPC code in the answer appears in a tool result or is marked unverified.

Tools called, in order:

1. `patent_analytics` `{phrases: ["perovskite tandem", "perovskite/silicon tandem", "perovskite-silicon tandem"]}`, subclass counts.
2. `patstat_cpc` `{q: "perovskite tandem solar cell"}`, 0 candidates.
3. `patstat_cpc` `{q: "perovskite"}`, 502 `upstream_error`, statement timeout (#477).
4. `patstat_cpc` `{q: "tandem solar cell"}`, 0 candidates.
5. `patstat_cpc` `{q: "multijunction photovoltaic"}`, 0 candidates.
6. `patstat_docs` `{section: "semantic-model", part: "index"}`, 18,658 chars, read. It carries `global_caveats.verified_values`.
7. `patstat_query`: a title match on `application_texts` for "perovskite" and "tandem", CPC codes with their scheme titles and family counts, 40 rows.

**Verdict: PARTIAL.** The process is correct. The agent read the index before the one SQL query, and the SQL is text discovery. The 12 subgroup codes in the table have their titles and counts from the SQL rows. The answer calls Y02E10/549 a tag and not a discriminator. The answer cites PATSTAT 2026 Spring. The two run 2 codes, H01L31/078 and H01L51/42, are gone, and H01G9/2072 is now from the SQL rows. Of the 24 code strings in the answer, 23 are in tool results, with H10K71 and H10F71 as prefixes of served subgroups. The answer says that H10F is "the former H01L31" and that "older filings carry H01L31 and H01G9/20 codes". H01L31 is in no tool result, and the answer does not mark it unverified. The answer also states a "2023 to 2025 reclassification into H10K and H10F" and gives subclass meanings that no result contains (backend #488).

### 5. "How many of Siemens's EP grants filed since 2015 were opposed, by year? Use PATSTAT."

This is the VQR entry `ep_oppositions_by_year`, run as the first session of the run. Pass condition: one call, or one identical resend on a cold timeout; the edition cited.

Tools called, in order:

1. `patstat_resolve` `{q: "Siemens"}`.
2. `patstat_docs` `{section: "semantic-model", part: "index"}`.
3. `patstat_docs` `{section: "examples"}`.
4. `patstat_docs` `{section: "semantic-model", view: "legal_events"}`.
5. `patstat_query`: the VQR SQL with psn_id 30138991, 2015 and `ipr_type = 'PI'`. It answered on the first call: 36 opposed grants, by opposition year.
6. `patstat_query`: a new query, grants and oppositions by filing year. 504 `patstat_sql_timeout`.
7. `patstat_query`: the EP opposition codes from `legal_event_codes`, 69 rows.
8. `patstat_query`: the SQL of call 6 resent once, unchanged, with `retry_of`. 504 again.
9. `patstat_query`: EP grants by filing year alone.
10. `patstat_query`: opposed grants by filing year alone. This new, narrower SQL also carries the `retry_of` label.

**Verdict: PASS.** The VQR question answered on the first call, and the agent did not rewrite correct SQL. The one timeout came on a second, new query. It got exactly one identical resend, and then the agent split it into two smaller queries, as the error tells it to. The answer cites PATSTAT 2026 Spring and says that the combined query timed out twice. The count is for the one harmonized entity: 36 opposed grants, and the by-filing-year split adds up to 36. One small point: call 10 carries a `retry_of` label, but its SQL is not a resend.

## Cross-question checks

| Check | Result |
|---|---|
| Right engine on all five | Yes. |
| Data Edition on every number | Yes. Every answer names PATSTAT 2026 Spring. Question 4 also uses the Google Patents slice of `patent_analytics` for its subclass table. |
| At most one retry on question 5 | Yes. One identical resend of a timed-out query, then two narrower queries. |
| No web data | Yes. No web tool was available, and `permission_denials` is empty in every run. |
| Tool results readable | Yes. No result was redirected to a file. The largest was 27,259 chars. |
| Every code or number from a tool result | No. Every number is sourced. Question 4 names one code, H01L31, that no result contains. |
| Prose facts from a tool result | No. Questions 1 and 2 each repeat an unsourced prose fact of run 2, and neither is marked (backend #488). |
| Tokens in run files | None. A grep for `fl_pat_`, `Bearer` and the JWT prefix finds nothing. |

## Still open

- **CPC title search misses phrases and times out on broad words** (questions 2 and 4, three timeouts in this run): [abdullahatrash/flowleap-backend#477](https://github.com/abdullahatrash/flowleap-backend/issues/477).
- **A main-group prefix from `patstat_cpc` covers the main group alone** (no question hit it in run 3): [abdullahatrash/flowleap-backend#483](https://github.com/abdullahatrash/flowleap-backend/issues/483).
- **The provenance rule reaches the agent, but it does not hold** (questions 1, 2 and 4). #486 delivered the rule to the description and the index, with [a run 3 comment](https://github.com/abdullahatrash/flowleap-backend/issues/486#issuecomment-5908570172). The new issue covers the one unsourced legacy code, prose facts outside the rule, and `seminal_families` rows with no applicant: [abdullahatrash/flowleap-backend#488](https://github.com/abdullahatrash/flowleap-backend/issues/488).

## Repeating the run

1. Build the CLI at the commit above and check that `flowleap mcp --check` shows 50 tools, 6 resources, 1 template and 3 prompts.
2. Edit the binary path in `acceptance/mcp.json` and the paths in `acceptance/run.sh`.
3. Run `acceptance/run.sh <N> "<question>"` once per question, one at a time, question 5 first.

The raw runs are in `acceptance/runs-3/`. Each `q<N>.json` is the event stream, `q<N>.err` is stderr, and `q<N>.started` is the UTC start time.
