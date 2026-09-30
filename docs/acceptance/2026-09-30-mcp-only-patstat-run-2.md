# MCP-only acceptance run 2 for PATSTAT, 2026-09-30

Issue: #98. This is the second run of the phase 1 verdict, after the second wave of fixes. The first run is [2026-09-30-mcp-only-patstat.md](2026-09-30-mcp-only-patstat.md), verdict FAIL. The questions, the harness flags and the pass conditions are the same as in run 1.

**Overall verdict: PARTIAL.** Four questions pass, and question 4 passes in part. All four overall criteria of #98 hold: the right engine on all five, an edition on every number, at most one retry on question 5, and no web data. Question 4 still names two legacy CPC codes that no tool result contains. No tool result was redirected to a file.

| # | Question | Run 1 | Run 2 |
|---|---|---|---|
| 1 | Siemens filings by year and office | FAIL | PASS |
| 2 | Who dominates solid-state batteries | PASS | PASS (with the `patstat_cpc` limitation, #477) |
| 3 | Who cites EP3477840 and how | PARTIAL | PASS |
| 4 | CPC codes of perovskite tandem cell filings | PARTIAL | PARTIAL |
| 5 | Siemens EP oppositions by year (cold-timeout case) | PASS | PASS |

## Changes since run 1

| Issue | What changed | Question |
|---|---|---|
| backend #482 | The descriptions send "who cites X and how" to `patstat_patent`. `patstat_technology` names `patstat_cpc` as the way to find the code. | 3, 2 |
| backend #484 | The default `patstat_portfolio` reply is about 10k chars, with `offices: "top"`. The VQR portfolio example counts the one entity, not the name prefix. | 1, 5 |
| backend #485 | `patstat_docs part=index` is about 18.6k chars, and `view=<name>` is 16k or less. The guarded-sql workflow says to read the index, then each view. It also says that every code or number must come from a tool result or be marked unverified. | 4, 5 |
| CLI #106 | The bridge sends tool results as compact JSON. | all |
| CLI #110 | The bridge serves the index as a resource and each view through a resource template. | 4 |

Not fixed: backend #477 (CPC title search quality) and backend #483 (main-group prefix undercount).

## Harness setup

| Item | Value |
|---|---|
| Date | 2026-09-30, runs between 08:57Z and 09:05Z |
| Harness | Claude Code 2.1.285, `claude -p` (non-interactive) |
| Model reported | `claude-fable-5-1` (default, `--model` not set) |
| CLI | flowleap 0.8.8, commit `dee60f5`, `cargo build --release` |
| Backend `apiVersion` | `1.0.0+ce3bc186e44d76fa6bb66d35a3748bc2b66f8026` (50 tools) |
| Data Edition | PATSTAT 2026 Spring |
| `flowleap mcp --check` | 50 tools, 6 resources, 1 template, 3 prompts, stored session token |

MCP config, `acceptance/mcp.json`:

```json
{"mcpServers":{"flowleap":{"command":"/Users/abdullahatrash/flowleap/wt-98b/target/release/flowleap","args":["mcp"]}}}
```

The command line is the same as in run 1. Only the paths changed, to the `wt-98b` worktree and the `acceptance/runs-2/` output directory. The script is `acceptance/run.sh`.

```bash
cd /tmp/fl-accept
CLAUDE_CODE_DISABLE_CLAUDE_MDS=1 CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 claude -p "<question>" \
  --mcp-config /Users/abdullahatrash/flowleap/wt-98b/acceptance/mcp.json --strict-mcp-config \
  --disable-slash-commands --tools "ListMcpResourcesTool,ReadMcpResourceTool" \
  --allowedTools "mcp__flowleap__*" "ListMcpResourcesTool" "ReadMcpResourceTool" \
  --permission-mode dontAsk --output-format stream-json --verbose --max-turns 25 \
  --no-session-persistence > /Users/abdullahatrash/flowleap/wt-98b/acceptance/runs-2/q<N>.json \
  2> /Users/abdullahatrash/flowleap/wt-98b/acceptance/runs-2/q<N>.err
```

Isolation is the same as in run 1. The `init` event of each session shows 52 tools (the 50 `mcp__flowleap__*` tools and the two MCP resource readers), `skills: []` and one connected MCP server. No session used the resource readers. Each question ran once, in a fresh session. No run crashed, and each `q<N>.err` is empty.

Question 5 ran first, for the cold-cache case, as in run 1.

## Questions

"Largest result" is the size in chars of the largest tool result that the client received in that session. "Redirected" counts the tool results that Claude Code saved to a file as over its output limit. A grep of each stream for "exceeds maximum allowed tokens" finds zero in all five runs.

| # | Tool calls | Largest result | Redirected | Retries |
|---|---|---|---|---|
| 1 | 1 | 10,073 (`patstat_portfolio`) | 0 | 0 |
| 2 | 6 | 9,645 (`patstat_technology`) | 0 | 0 |
| 3 | 1 | 8,506 (`patstat_patent`) | 0 | 0 |
| 4 | 16 | 27,259 (`patstat_docs section=examples`) | 0 | 0 |
| 5 | 9 | 27,259 (`patstat_docs section=examples`) | 0 | 1 identical resend |

### 1. "Show Siemens filings by year and office since 2016."

Pass condition: `patstat_portfolio` first and its reply read by the agent, the numbers from it, the edition cited.

Tools called, in order:

1. `patstat_portfolio` `{applicant: "Siemens", from_year: 2016, to_year: 2026, offices: "top"}`. The reply was 10,073 chars and the agent read it.

**Verdict: PASS.** One call. The totals (32,497 applications, 49 offices, EP 9,391, WO 8,508, US 4,610) and the year-by-office table come from the reply. The answer cites PATSTAT 2026 Spring, keeps WO out of the grant count, and marks 2024 and 2025 as publication lag. One weakness remains from run 1: the answer gives the spin-offs of Siemens Healthineers, Energy and Gamesa as the cause of the drop after 2019, and the reply does not contain that cause (backend #486).

### 2. "Who dominates solid-state batteries?"

Pass condition: `patstat_cpc` tried first, no invented code (a code from a served example or from `patstat_docs` counts as sourced), the edition cited.

Tools called, in order:

1. `patstat_cpc` `{q: "solid-state electrolyte battery"}`, 0 candidates.
2. `patstat_cpc` `{q: "solid electrolyte"}`, 0 candidates.
3. `patstat_cpc` `{q: "solid state battery"}`, 0 candidates.
4. `patstat_technology` `{cpc: "H01M10/0562"}`. The area title is "Solid materials".
5. `patstat_technology` `{cpc: "H01M10/0585"}`. The area title is flat-cell construction. The agent read the title and dropped the code.
6. `patstat_technology` `{cpc: "H01M10/0565"}`. The area title is "Polymeric materials, e.g. gel-type or solid-type".

**Verdict: PASS.** `patstat_cpc` came first and found nothing (#477). H01M10/0562 comes from the served `patstat_cpc` and `patstat_technology` examples. H01M10/0585 and H01M10/0565 are in no served text, so the agent took them from model memory. The agent checked each one against the title in the tool result before it used it, and every code in the answer has its title in a tool result. The answer cites PATSTAT 2026 Spring. Two weaknesses:

- The answer does not say that `patstat_cpc` found nothing.
- The answer says that "QuantumScape's garnet family from 2013 is among the most-cited in the field". No tool result names QuantumScape or carries citation counts (backend #486).

### 3. "Who cites EP3477840 and how?"

Pass condition: `patstat_resolve` then `patstat_patent`, or `patstat_patent` directly; the origin stated; the edition cited.

Tools called, in order:

1. `patstat_patent` `{number: "EP3477840"}`.

**Verdict: PASS.** One call, and no OPS tool. The answer names the two citing documents, EP3796345A1 and CN112530684A, both from one Robert Bosch family, and both with origin `SEA` (examiner search report). It also gives the backward citations with their origin: five `SEA` and four `APP` of nine. The answer cites PATSTAT 2026 Spring.

### 4. "Which CPC codes do 'perovskite tandem cell' filings actually use?"

Pass condition: `patstat_docs part=index` and/or a view before `patstat_query`; text-discovery SQL; the codes read from tool results with their titles; no unsourced code; the edition cited.

Tools called, in order:

1. to 5. `patent_analytics`, five phrase variants ("perovskite tandem", "perovskite" + "tandem solar cell", "perovskite/silicon tandem", "perovskite-silicon tandem", "tandem perovskite").
6. `patstat_cpc` `{q: "perovskite"}`, 502 `upstream_error`, statement timeout (#477).
7. `patstat_cpc` `{q: "tandem solar cell"}`, 0 candidates.
8. `patstat_docs` `{section: "semantic-model", part: "index"}`, 18,546 chars, read.
9. `patstat_docs` `{section: "semantic-model", view: "application_texts"}`, 5,905 chars.
10. `patstat_docs` `{section: "semantic-model", view: "classifications"}`, 5,416 chars.
11. `patstat_docs` `{section: "examples"}`, 27,259 chars.
12. `patstat_query`: full-text match on `application_texts.title`, CPC codes with titles and family shares.
13. `patstat_query`: the same over `abstract`.
14. `patstat_query`: a cross-check seed ("perovskite laminated/stacked solar cell").
15. `patstat_query`: the corpus size of each candidate code.
16. `patstat_query`: the subclass mix on the abstract corpus, to look for legacy H01L codes.

**Verdict: PARTIAL.** The process is now correct. The agent read the index and the two views it queried before any SQL. The SQL is text discovery over the views. The answer reads 11 codes with their titles, shares and corpus sizes, and it does not take rank 1 blindly: it calls Y02E10/549 a tag. The answer cites PATSTAT 2026 Spring. Of the 18 codes in the answer, 16 are in tool results. The answer tells the user to add H01L31/078 and H01L51/42 for Google Patents. These two codes are in no tool result, and the answer does not mark them unverified. Run 1 named the same two codes. The served rule against this did not reach the agent (backend #486).

### 5. "How many of Siemens's EP grants filed since 2015 were opposed, by year? Use PATSTAT."

This is the VQR entry `ep_oppositions_by_year`, run as the first session of the run. Pass condition: one call, or one identical resend on a cold timeout; the edition cited.

Tools called, in order:

1. `patstat_resolve` `{q: "Siemens"}`.
2. `patstat_docs` `{section: "semantic-model", part: "index"}`.
3. `patstat_docs` `{section: "examples"}`.
4. `patstat_docs` `{section: "semantic-model", view: "legal_events"}`.
5. `patstat_query`: the VQR SQL, with Siemens and 2015 put in, the entity-key filter from the portfolio example added, and `ipr_type = 'PI'` added. It answered on the first call: 36 opposed grants, by opposition year.
6. `patstat_query`: a new query, grants and oppositions by filing year. 504 `patstat_sql_timeout`.
7. `patstat_query`: the same SQL resent once, unchanged, with `retry_of`. 504 again.
8. `patstat_query`: EP grants by filing year alone, as the error message says ("narrow it instead").
9. `patstat_query`: opposed grants by filing year alone.

**Verdict: PASS.** The VQR question answered on the first call, and the agent did not rewrite correct SQL. The one timeout came on a second, new query. It got exactly one identical resend, and then the agent split it into two smaller queries, as the error tells it to. The answer cites PATSTAT 2026 Spring and says that one query timed out twice. The count is now for the one harmonized entity: 36 opposed grants of 2,981. Run 1 counted 118 over the bare SIEMENS name prefix.

## Cross-question checks

| Check | Result |
|---|---|
| Right engine on all five | Yes. |
| Data Edition on every number | Yes. Every answer names PATSTAT 2026 Spring. Question 4 also uses the Google Patents slice of `patent_analytics` for its search tip. |
| At most one retry on question 5 | Yes. One identical resend of a timed-out query, then a narrower query. |
| No web data | Yes. No web tool was available, and `permission_denials` is empty in every run. |
| Tool results readable | Yes. No result was redirected to a file. The largest was 27,259 chars. |
| Every code or number from a tool result | No. Questions 1, 2 and 4 each state one or two facts that no tool result contains (backend #486). |
| Tokens in run files | None. A grep for `fl_pat_`, `Bearer` and the JWT prefix finds nothing. |

## Still open

- **CPC title search misses phrases and times out on broad words** (questions 2 and 4): [abdullahatrash/flowleap-backend#477](https://github.com/abdullahatrash/flowleap-backend/issues/477), with the run 2 data in [a new comment](https://github.com/abdullahatrash/flowleap-backend/issues/477#issuecomment-5907953397).
- **A main-group prefix from `patstat_cpc` covers the main group alone** (no question hit it in run 2, because `patstat_cpc` returned no main group): [abdullahatrash/flowleap-backend#483](https://github.com/abdullahatrash/flowleap-backend/issues/483).
- **New: the provenance rule reaches no MCP agent** (questions 1, 2 and 4). The rule is only in the `docs.usage` field of `patstat_query` and in the guarded-sql workflow. The MCP client shows only `description`, and no session read the workflow: [abdullahatrash/flowleap-backend#486](https://github.com/abdullahatrash/flowleap-backend/issues/486).

## Repeating the run

1. Build the CLI at the commit above and check that `flowleap mcp --check` shows 50 tools, 6 resources, 1 template and 3 prompts.
2. Edit the binary path in `acceptance/mcp.json` and the paths in `acceptance/run.sh`.
3. Run `acceptance/run.sh <N> "<question>"` once per question, one at a time, question 5 first.

The raw runs are in `acceptance/runs-2/`. Each `q<N>.json` is the event stream, `q<N>.err` is stderr (empty in all five), and `q<N>.started` is the UTC start time.
