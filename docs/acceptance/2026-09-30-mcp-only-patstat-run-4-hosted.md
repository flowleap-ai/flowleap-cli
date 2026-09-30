# MCP-only acceptance run 4 for PATSTAT over the hosted MCP server, 2026-09-30

Issue: #98, and gate 1 of the hosted MCP server rollout, [abdullahatrash/flowleap-backend#497](https://github.com/abdullahatrash/flowleap-backend/issues/497). This run asks the five questions of run 3 through the **hosted MCP server** and not through the stdio bridge. Run 3 is [2026-09-30-mcp-only-patstat-run-3.md](2026-09-30-mcp-only-patstat-run-3.md), verdict PARTIAL. The questions, the harness flags and the pass conditions are the same as in run 3. Only the transport and the sign-in changed.

**Overall verdict: PARTIAL, gate 1 PASS (parity with run 3).** Each question has the same verdict as in run 3: four pass, and question 4 passes in part. All four overall criteria of #98 hold: the right engine on all five, an edition on every number, at most one retry on question 5, and no web data. No tool result was redirected to a file, and the largest result of each question has the same size as in run 3. One hosted-only divergence is new: the per-identity budget rejected 11 calls in questions 3 and 4, and the hosted server sends a short rejection to the agent, which the stdio bridge waits out in-band ([abdullahatrash/flowleap-backend#510](https://github.com/abdullahatrash/flowleap-backend/issues/510)).

| # | Question | Run 2 | Run 3 | Run 4 (hosted) |
|---|---|---|---|---|
| 1 | Siemens filings by year and office | PASS | PASS | PASS |
| 2 | Who dominates solid-state batteries | PASS | PASS | PASS (with the `patstat_cpc` limitation, #477) |
| 3 | Who cites EP3477840 and how | PASS | PASS | PASS (one resend after a rate limit, #510) |
| 4 | CPC codes of perovskite tandem cell filings | PARTIAL | PARTIAL | PARTIAL |
| 5 | Siemens EP oppositions by year (cold-timeout case) | PASS | PASS | PASS |

## Transport

| Item | Value |
|---|---|
| Transport | Hosted MCP server, stateless Streamable HTTP, JSON replies |
| URL | `https://api.flowleap.co/mcp` |
| Auth | Personal token (`fl_pat_`) as a bearer: `Authorization: Bearer ${FLOWLEAP_GATE_TOKEN}` |
| Backend `apiVersion` | `1.0.0+f965a6d26357f52aaf123d76a94a79bbb707f063` (from `/v1/health`, and the `serverInfo.version` of `initialize`) |
| Protocol version negotiated | `2025-06-18` (smoke `initialize` with curl) |
| Data Edition | PATSTAT 2026 Spring |
| CLI | flowleap 0.9.0. Not used by the runs. Only the harness script and config are from the CLI repo. |

The `mcp` block of `/v1/health`, verbatim:

```json
{"enabled": true, "tools": 50, "resources": 6, "prompts": 3, "protocolVersions": ["2026-07-28", "2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05", "2024-10-07"], "oauth": {"enabled": true, "issuer": "https://clerk.flowleap.co"}}
```

A smoke test with curl and the bearer, before the run, gave these counts. They are the same as `flowleap mcp --check` in run 3.

| Method | Count |
|---|---|
| `tools/list` | 50 |
| `resources/list` | 6 |
| `resources/templates/list` | 1 |
| `prompts/list` | 3 |

## Changes since run 3

The transport is the only change. The registry, the doctrine and the tool descriptions are the same.

| Backend PR | What changed |
|---|---|
| [#504](https://github.com/abdullahatrash/flowleap-backend/pull/504) | The hosted MCP server, tools only: `POST /mcp` behind `MCP_HOSTED_ENABLED`. |
| [#506](https://github.com/abdullahatrash/flowleap-backend/pull/506) | The doctrine over the hosted server: six resources, the view template and three prompts. |
| [#507](https://github.com/abdullahatrash/flowleap-backend/pull/507) | OAuth end to end on `/mcp`. This run does not use OAuth. It uses the personal-token branch of the same sign-in. |

## Harness setup

| Item | Value |
|---|---|
| Date | 2026-09-30, runs between 13:28Z and 13:34Z |
| Harness | Claude Code 2.1.285, `claude -p` (non-interactive) |
| Model reported | `claude-fable-5-1` (default, `--model` not set) |

MCP config, `acceptance/mcp-hosted.json`:

```json
{"mcpServers":{"flowleap":{"type":"http","url":"https://api.flowleap.co/mcp","headers":{"Authorization":"Bearer ${FLOWLEAP_GATE_TOKEN}"}}}}
```

Claude Code expands `${FLOWLEAP_GATE_TOKEN}` in the header at runtime. The file in the repo holds only the placeholder. `acceptance/run.sh` now takes `MCP_CONFIG` (default: the stdio `acceptance/mcp.json`), `RUNS_DIR` (default: `acceptance/runs-3`) and `FLOWLEAP_GATE_TOKEN_FILE`. The script reads the token from that file into the environment variable. The token file is outside the repo. The command line flags are the same as in run 3.

```bash
MCP_CONFIG=acceptance/mcp-hosted.json RUNS_DIR=acceptance/runs-4-hosted \
FLOWLEAP_GATE_TOKEN_FILE=/path/outside/the/repo/token.txt \
  acceptance/run.sh 5 "How many of Siemens's EP grants filed since 2015 were opposed, by year? Use PATSTAT."
```

Isolation is the same as in run 3. The `init` event of each session shows 52 tools (the 50 `mcp__flowleap__*` tools and the two MCP resource readers), `skills: []` and one connected MCP server, `flowleap`, status `connected`. No session used the resource readers or a prompt. Each question ran once, in a fresh session, and `permission_denials` is empty in all five. No run crashed, and all five `q<N>.err` files are empty.

Question 5 ran first, for the cold-cache case, as in runs 1 to 3. The other four ran directly after it, in the order 1, 2, 3, 4.

## Questions

"Largest result" is the size in chars of the largest tool result that the client received in that session. "Redirected" counts the tool results that Claude Code saved to a file as over its output limit. A grep of each stream for "exceeds maximum allowed tokens" finds zero in all five runs. "Rate limited" counts the `rate_limited` tool errors.

| # | Tool calls | Largest result | Same as run 3 | Redirected | Rate limited | Retries |
|---|---|---|---|---|---|---|
| 1 | 1 | 10,073 (`patstat_portfolio`) | yes | 0 | 0 | 0 |
| 2 | 5 | 9,546 (`patstat_technology`) | yes | 0 | 0 | 0 |
| 3 | 2 | 8,506 (`patstat_patent`) | yes | 0 | 1 | 1 resend after the rate limit |
| 4 | 21 | 18,658 (`patstat_docs part=index`) | yes | 0 | 10 | 10 resends after rate limits |
| 5 | 9 | 27,259 (`patstat_docs section=examples`) | yes | 0 | 0 | 1 identical resend |

### 1. "Show Siemens filings by year and office since 2016."

Pass condition: `patstat_portfolio` first and its reply read by the agent, the numbers from it, the edition cited.

Tools called, in order:

1. `patstat_portfolio` `{applicant: "Siemens", from_year: 2016, to_year: 2026, offices: "top"}`. The reply was 10,073 chars and the agent read it.

**Verdict: PASS.** One call, the same as in run 3. Every number in the answer is in the reply: 32,497 applications, 49 offices, the year-by-office table, the office totals and the five largest excluded entities. The answer cites PATSTAT 2026 Spring and marks 2024 and 2025 as publication lag. The run 3 weakness recurs and is not marked: the answer says that the 2019 to 2020 drop "coincides with Siemens spinning off Siemens Healthineers, Siemens Energy, and Siemens Mobility". The reply contains no "spin-off" and no "Healthineers" (backend #488).

### 2. "Who dominates solid-state batteries?"

Pass condition: `patstat_cpc` tried first, no invented code (a code from a served example or from `patstat_docs` counts as sourced), the edition cited.

Tools called, in order:

1. `patstat_cpc` `{q: "solid-state electrolyte"}`, 0 candidates.
2. `patstat_cpc` `{q: "solid electrolyte"}`, 0 candidates.
3. `patstat_cpc` `{q: "solid state battery"}`, 0 candidates.
4. `patstat_cpc` `{q: "electrolyte"}`, 502 `upstream_error`, statement timeout (#477).
5. `patstat_technology` `{cpc: "H01M10/0562"}`, 9,546 chars.

**Verdict: PASS.** `patstat_cpc` came first and found nothing (#477). H01M10/0562 comes from the served examples. This run did not try the model-memory code H01M10/0585 of run 3. The top-ten table, the country ranks, the trend, the new entrants and the grant rates are in the `patstat_technology` reply, and the answer cites PATSTAT 2026 Spring. The answer says that the CPC lookup found nothing and then timed out, and that the code misses polymer-electrolyte work. The run 3 weakness recurs and is not marked: the answer names PolyPlus and QuantumScape as the owners of the most-cited families, and neither name is in a tool result (backend #488).

### 3. "Who cites EP3477840 and how?"

Pass condition: `patstat_resolve` then `patstat_patent`, or `patstat_patent` directly; the origin stated; the edition cited.

Tools called, in order:

1. `patstat_patent` `{number: "EP3477840"}`. `rate_limited`, `retryAfterSeconds: 3`.
2. `patstat_patent` `{number: "EP3477840"}`, the same input, 8,506 chars.

**Verdict: PASS.** No OPS tool. The answer names the two citing documents, EP3796345A1 and CN112530684A, both from one Robert Bosch family, and both with origin `SEA` (examiner search report). It gives the backward citations as five examiner and four applicant of nine, and the two SI family members. Every fact is in the reply. The answer cites PATSTAT 2026 Spring. The first call got a rate limit, which is new and hosted-only: the stdio bridge waits in-band for a `Retry-After` of 5 s or less (#510).

### 4. "Which CPC codes do 'perovskite tandem cell' filings actually use?"

Pass condition: `patstat_docs part=index` and/or a view before `patstat_query`; text-discovery SQL; the codes read from tool results with their titles; no unsourced code; the edition cited. From run 3: every CPC code in the answer appears in a tool result or is marked unverified.

Tools called, in order:

1. `patent_analytics` `{phrases: ["perovskite tandem"]}`, subclass counts.
2. `patent_analytics` `{phrases: ["perovskite/silicon tandem", "perovskite-silicon tandem", "perovskite silicon tandem"]}`.
3. to 7. `patstat_cpc` (`perovskite`, `tandem solar cell`, `multijunction photovoltaic`, `perovskite`) and `patstat_docs part=index`, all five `rate_limited`, `retryAfterSeconds` 28 down to 21.
8. `search_patents` `{provider: "epo_ops", query: "ta=\"perovskite tandem\" and ab=\"perovskite tandem\""}`. `data_keys_required`: the trial access of this identity to shared OPS keys has ended. The answer does not use OPS data.
9. to 13. `patstat_cpc` (`perovskite` three times, `tandem solar cell`) and `patstat_docs part=index`, all five `rate_limited`, `retryAfterSeconds` 9 down to 1.
14. `patstat_cpc` `{q: "perovskite"}`, 502 `upstream_error`, statement timeout (#477).
15. `patstat_cpc` `{q: "perovskite solar cell"}`, 0 candidates.
16. `patstat_cpc` `{q: "tandem solar cell"}`, 0 candidates.
17. `patstat_docs` `{section: "semantic-model", part: "index"}`, 18,658 chars, read.
18. `patstat_docs` `{section: "semantic-model", view: "classifications"}`, 5,416 chars.
19. `patstat_query`: the scheme titles of 15 subclass and main-group symbols.
20. `patstat_query`: a title match on `application_texts` for "perovskite" and "tandem", CPC codes with their scheme titles and family counts, 40 rows.
21. `patstat_query`: the size of that title set, 85 families.

**Verdict: PARTIAL.** The process is correct and one step better than run 3: the agent read the index and the `classifications` view before the SQL, and the SQL is text discovery. The subgroup codes have their titles and family counts from the SQL rows. The answer calls Y02E10/549 and the other Y02 codes tags. The answer cites PATSTAT 2026 Spring and the Google Patents slice. Of the 20 code strings in the answer, 18 are in tool results. The answer says that "older filings may still carry H01L31 and H01L51 codes in other databases". H01L31 and H01L51 are in no tool result, and the answer does not mark them unverified. This is one more unsourced main group than in run 3, and the same kind of fault. The answer also says that H10F and H10K "only replaced them in 2023 and 2025" and that "the PATSTAT edition used here has already been reclassified". No result gives these dates. The served index says the opposite: legacy and current codes coexist on this edition, "H01L and H10F for photovoltaics" (backend #488). The ten rate limits and the failed OPS detour cost calls, but they did not change the answer (#510).

### 5. "How many of Siemens's EP grants filed since 2015 were opposed, by year? Use PATSTAT."

This is the VQR entry `ep_oppositions_by_year`, run as the first session of the run. Pass condition: one call, or one identical resend on a cold timeout; the edition cited.

Tools called, in order:

1. `patstat_resolve` `{q: "Siemens"}`.
2. `patstat_docs` `{section: "semantic-model", part: "index"}`.
3. `patstat_docs` `{section: "examples"}`.
4. `patstat_docs` `{section: "semantic-model", view: "legal_events"}`.
5. `patstat_query`: the VQR SQL with psn_id 30138991, filing year 2015 or later. It answered on the first call: 36 opposed grants, by opposition year.
6. `patstat_query`: a new query, grants and oppositions by filing year. 504 `patstat_sql_timeout`.
7. `patstat_query`: the SQL of call 6 resent once, unchanged, with `retry_of`. 504 again.
8. `patstat_query`: opposed grants by filing year alone. This new, narrower SQL carries a `retry_of` label.
9. `patstat_query`: EP grants by filing year alone.

**Verdict: PASS.** The same pattern as in run 3, with one call less. The VQR question answered on the first call. The one timeout came on a second, new query. It got exactly one identical resend, and then the agent split it into two smaller queries. The answer cites PATSTAT 2026 Spring and says that the combined query timed out twice. The count is for the one harmonized entity: 36 opposed grants of 2,981, and the by-filing-year split adds up to 36. The run 3 small point recurs: call 8 carries a `retry_of` label, but its SQL is not a resend.

## Cross-question checks

| Check | Result |
|---|---|
| Right engine on all five | Yes. Question 4 tried one OPS search after ten rate limits. It failed on data keys, and the answer uses PATSTAT and the Google Patents slice, as in run 3. |
| Data Edition on every number | Yes. Every answer names PATSTAT 2026 Spring. |
| At most one retry on question 5 | Yes. One identical resend of a timed-out query, then two narrower queries. No rate limit in question 5. |
| No web data | Yes. No web tool was available, and `permission_denials` is empty in every run. |
| Tool results readable | Yes. No result was redirected to a file. The largest was 27,259 chars, the same as in run 3. |
| Every code or number from a tool result | No. Every number is sourced. Question 4 names two legacy main groups, H01L31 and H01L51, that no result contains. |
| Prose facts from a tool result | No. Questions 1, 2 and 4 each state a prose fact that no result contains, and none is marked (backend #488). |
| Tokens in run files | None. A grep of `acceptance/runs-4-hosted/` for `fl_pat_`, `Bearer`, `Authorization` and the JWT prefix `eyJ` finds nothing. A grep for the token value finds nothing in the repo. The stream-json output does not echo the request headers of the HTTP MCP server, so no line needed redaction. |

## Divergences from run 3

- **Short rate limits reach the agent** (questions 3 and 4, 11 rejections). The PATSTAT graph tools and `patstat_docs` spend the `signed-in` budget, 30 calls per minute per identity, on every transport (ADR 0022 decision 6). The personal token of this run is the same user as other agent sessions that ran at the same time, so they shared the budget. The budget is not a divergence. The divergence is the short wait: the stdio bridge waits and resends in-band when `Retry-After` is 5 s or less, and the hosted server sends the error to the agent. In question 4 the agent resent before the wait that `retryAfterSeconds` asked for ran out, which spent ten calls. Filed: [abdullahatrash/flowleap-backend#510](https://github.com/abdullahatrash/flowleap-backend/issues/510).
- **OPS data keys** (question 4). The `search_patents` detour failed with `data_keys_required`. This is the trial state of the identity, not the transport, and no pass condition uses OPS. No issue filed.

No other divergence: the tool list, the result sizes, the error codes of the PATSTAT tools and the doctrine are the same as over the stdio bridge.

## Still open

- **CPC title search misses phrases and times out on broad words** (questions 2 and 4, two timeouts in this run): [abdullahatrash/flowleap-backend#477](https://github.com/abdullahatrash/flowleap-backend/issues/477).
- **A main-group prefix from `patstat_cpc` covers the main group alone** (no question hit it in run 4): [abdullahatrash/flowleap-backend#483](https://github.com/abdullahatrash/flowleap-backend/issues/483).
- **The provenance rule reaches the agent, but it does not hold** (questions 1, 2 and 4): [abdullahatrash/flowleap-backend#488](https://github.com/abdullahatrash/flowleap-backend/issues/488).
- **MCP transports drop the success-envelope extras** such as `trial_data_budget_low` (no question checks it): [abdullahatrash/flowleap-backend#505](https://github.com/abdullahatrash/flowleap-backend/issues/505).
- **Short rate limits reach the hosted MCP agent** (new in this run): [abdullahatrash/flowleap-backend#510](https://github.com/abdullahatrash/flowleap-backend/issues/510).

## Repeating the run

1. Check `/v1/health`: the `mcp` block must show 50 tools, 6 resources and 3 prompts.
2. Put a personal token in a file outside the repo.
3. Run `acceptance/run.sh <N> "<question>"` once per question, one at a time, question 5 first, with `MCP_CONFIG=acceptance/mcp-hosted.json`, `RUNS_DIR=acceptance/runs-4-hosted` and `FLOWLEAP_GATE_TOKEN_FILE` set. Do not run other sessions of the same identity at the same time, or the per-identity budget is shared.

The raw runs are in `acceptance/runs-4-hosted/`. Each `q<N>.json` is the event stream, `q<N>.err` is stderr, and `q<N>.started` is the UTC start time.
