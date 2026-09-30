# MCP-only acceptance run for PATSTAT, 2026-09-30

Issue: #98. This is the phase 1 verdict. A fresh Claude Code session whose only FlowLeap surface is the `flowleap mcp` server, with no skills, answered five questions.

**Overall verdict: FAIL.** Two questions pass, two pass in part, and one fails. Question 1 delivered numbers from `patstat_query` instead of `patstat_portfolio`, because the portfolio reply was too large for the MCP client. No question used web data, and no question retried a query.

| # | Question | Verdict |
|---|---|---|
| 1 | Siemens filings by year and office | FAIL |
| 2 | Who dominates solid-state batteries | PASS (with the recorded `patstat_cpc` limitation) |
| 3 | Who cites EP3477840 and how | PARTIAL |
| 4 | CPC codes of perovskite tandem cell filings | PARTIAL |
| 5 | Siemens EP oppositions by year (cold-timeout case) | PASS (no timeout occurred) |

## Harness setup

| Item | Value |
|---|---|
| Date | 2026-09-30, runs between 07:06Z and 07:14Z |
| Harness | Claude Code 2.1.285, `claude -p` (non-interactive) |
| Model reported | `claude-fable-5-1` (default, `--model` not set) |
| CLI | flowleap 0.8.8, commit `756183d`, `cargo build --release` |
| Backend `apiVersion` | `1.0.0+25e659d180393c4bbad9ef28553d56fc05aabbf2` (50 tools) |
| Data Edition | PATSTAT 2026 Spring |
| `flowleap mcp --check` | 50 tools, 5 resources, 3 prompts, stored session token |

MCP config, `acceptance/mcp.json`:

```json
{"mcpServers":{"flowleap":{"command":"/Users/abdullahatrash/flowleap/wt-98/target/release/flowleap","args":["mcp"]}}}
```

Command line for each question, from the empty directory `/tmp/fl-accept` (no CLAUDE.md, no `.claude/`). The script is `acceptance/run.sh`.

```bash
cd /tmp/fl-accept
CLAUDE_CODE_DISABLE_CLAUDE_MDS=1 CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 claude -p "<question>" \
  --mcp-config /Users/abdullahatrash/flowleap/wt-98/acceptance/mcp.json --strict-mcp-config \
  --disable-slash-commands --tools "ListMcpResourcesTool,ReadMcpResourceTool" \
  --allowedTools "mcp__flowleap__*" "ListMcpResourcesTool" "ReadMcpResourceTool" \
  --permission-mode dontAsk --output-format stream-json --verbose --max-turns 25 \
  --no-session-persistence > /Users/abdullahatrash/flowleap/wt-98/acceptance/runs/q<N>.json \
  2> /Users/abdullahatrash/flowleap/wt-98/acceptance/runs/q<N>.err
```

Isolation, as the session `init` event reports it:

- **Tools**: the 50 `mcp__flowleap__*` tools and the two MCP resource readers. No built-in tool (no Bash, Read, WebSearch or WebFetch) and no other MCP server.
- **Skills**: `skills: []`. `--disable-slash-commands` hides the FlowLeap skills installed in `~/.claude/skills`.
- **Memory**: no CLAUDE.md and no auto-memory. A probe session confirmed that it saw no CLAUDE.md text.
- **Still loaded**: user plugins (no skills or MCP servers from them reach the session) and one user SessionStart hook that only prints a status line.

The resource readers are allowed because the served doctrine resources are part of the MCP surface. No run used them.

`--output-format stream-json` replaces the plain `json` format, because only the stream carries the tool calls. Each run file is one JSON event per line. Each question ran once. No run crashed.

Question 5 ran first. Question 1 also reads Siemens data, so running it first could warm the cache for the cold-timeout case.

## Questions

### 1. "Show Siemens filings by year and office since 2016."

Pass condition: `patstat_portfolio`, Data Edition cited.

Tools called, in order:

1. `patstat_portfolio` `{applicant: "Siemens", from_year: 2016, to_year: 2026}`. The reply was 53,644 chars. Claude Code rejected it as over its MCP output limit and saved it to a file that the agent had no tool to read.
2. `patstat_docs` `{section: "semantic-model"}`. The reply was 91,205 chars and was rejected in the same way.
3. `patstat_docs` `{section: "examples"}`.
4. `patstat_resolve` `{q: "Siemens"}`.
5. `patstat_query` with the VQR portfolio SQL (`UPPER(ap.name) LIKE 'SIEMENS%'`), top 8 offices plus OTHER.

**Verdict: FAIL.** The right tool was called first, but the delivered numbers come from `patstat_query`. They count every SIEMENS-prefixed entity: 68,965 applications for 2016 to 2026. The unread portfolio reply counts 32,497 for the one harmonized entity. The answer cites PATSTAT 2026 Spring and says that the prefix includes subsidiaries. It also adds reasons for trends (for example Siemens Gamesa for the Spanish drop) that no tool result supports.

### 2. "Who dominates solid-state batteries?"

Pass condition: `patstat_cpc` to find the code, then `patstat_technology`; no invented CPC code; edition cited. Known limitation (backend #477): `patstat_cpc` times out on broad words and misses phrases. The pass here is that the agent tried `patstat_cpc` first and did not invent a code.

Tools called, in order:

1. `patstat_cpc` `{q: "solid-state electrolyte"}`, 0 candidates. This is the tool's own schema example.
2. `patstat_cpc` `{q: "solid electrolyte"}`, 0 candidates.
3. `patstat_cpc` `{q: "solid state battery"}`, 0 candidates.
4. `patstat_technology` `{cpc: "H01M10/0562"}`. The area card returned the title "Solid materials".
5. `patstat_technology` `{cpc: "H01M10/056"}`.
6. `patent_analytics` `{phrases: ["solid-state battery", ...], dateFrom: "2016-01-01"}`.

**Verdict: PASS.** `patstat_cpc` came first. H01M10/0562 comes from the served `patstat_technology` example ("Solid-state electrolyte landscape"), not from model memory. The answer cites PATSTAT 2026 Spring for the family counts and the Google Patents slice date for the keyword counts. The answer does not say that `patstat_cpc` found nothing.

### 3. "Who cites EP3477840 and how?"

Pass condition: `patstat_resolve` then `patstat_patent`; examiner versus applicant origin stated; edition cited.

Tools called, in order:

1. `get_citations` `{patent_number: "EP3477840", type: "all"}` (EPO OPS). Two citing documents, no origin.
2. `patstat_explain` `{node: "EP3477840"}`. Two `cited_by` edges, both `origin: SEA`.
3. `get_bibliography` `{patent_number: "EP3796345A1"}`.
4. `get_bibliography` `{patent_number: "EP4726980A1"}`.
5. `get_patent_family` `{patent_number: "EP3796345A1"}`.

**Verdict: PARTIAL.** The answer is correct and states the origin: both citing families are examiner (search report) citations, and none is an applicant citation. The answer names the PATSTAT Spring 2026 edition. But the agent went to OPS first and used `patstat_explain`. It never called `patstat_resolve` or `patstat_patent`.

### 4. "Which CPC codes do 'perovskite tandem cell' filings actually use?"

Pass condition: `patstat_docs` (semantic model) before `patstat_query`; text-discovery SQL over the views; the top codes read with their titles, not rank 1 taken blindly; edition cited.

Tools called, in order:

1. `patent_analytics` `{phrases: ["perovskite tandem"]}`.
2. `patent_analytics` `{phrases: ["perovskite", "tandem"]}`.
3. `patent_analytics` `{phrases: ["perovskite/silicon tandem"]}`.
4. `patstat_cpc` `{q: "perovskite"}`, 502 statement timeout.
5. `patstat_cpc` `{q: "tandem solar cell"}`, 0 candidates.
6. `patstat_docs` `{section: "semantic-model"}`, rejected as over the MCP output limit.
7. `patstat_docs` `{endpoint: "query"}`.
8. `patstat_docs` `{section: "examples"}`.
9. `patstat_cpc` `{q: "perovskite solar cell"}`, 0 candidates.
10. `search_patents` `{query: "ta=\"perovskite tandem\"", provider: "epo_ops"}`.
11. `patstat_query`: full-text match on `flowleap.application_texts.title`, CPC codes counted by family, with titles.
12. `patstat_query`: the same over `abstract`, for recall.

**Verdict: PARTIAL.** The SQL is text discovery over the views, and the answer reads about 25 codes with their titles and family shares. The answer cites PATSTAT 2026 Spring. Two weaknesses:

- The agent asked for the semantic model before `patstat_query` but could not read it.
- The answer names three legacy codes (H01L31/0725, H01L31/078, H01L51/42) that no tool result contains.

### 5. "How many of Siemens's EP grants filed since 2015 were opposed, by year? Use PATSTAT."

This is the VQR entry `ep_oppositions_by_year`, run as the first session of the day's run. Pass condition: exactly one resend of the same SQL after a cold timeout, then the answer; no rewrite of correct SQL.

Tools called, in order:

1. `patstat_docs` `{section: "examples"}`.
2. `patstat_resolve` `{q: "Siemens"}`.
3. `patstat_query`: the VQR SQL verbatim, with `SIEMENS%` and 2015 substituted. It answered on the first call.
4. `patstat_query`: a second cut by filing year with the grant base. This is a new query, not a retry.

**Verdict: PASS.** No cold timeout occurred, so the resend path was not tested. The agent did not rewrite the correct SQL, and it made zero retries. The answer cites PATSTAT 2026 Spring: 118 opposed grants, by opposition year and by filing year.

## Cross-question checks

| Check | Result |
|---|---|
| Right engine on all five | No. Question 1 delivered `patstat_query` numbers. |
| Data Edition on every number | Yes. Every answer names PATSTAT 2026 Spring. Question 2 and 4 also date the Google Patents slice for the `patent_analytics` numbers. |
| At most one retry on question 5 | Yes, zero retries. |
| No web data | Yes. No web tool was available, and `permission_denials` is empty in every run. |
| Tokens in run files | None. A grep for `fl_pat_`, `Bearer` and JWT prefixes finds nothing. |

## Failures filed

- **Portfolio reply too large, and its VQR example counts 2 times the endpoint** (question 1): [abdullahatrash/flowleap-backend#479](https://github.com/abdullahatrash/flowleap-backend/issues/479).
- **Semantic model cannot be read over MCP** (questions 1 and 4, and the unsourced legacy codes): [abdullahatrash/flowleap-backend#480](https://github.com/abdullahatrash/flowleap-backend/issues/480).
- **Registry descriptions: citation-origin routing, and the `patstat_cpc` versus `patstat_technology` mismatch** (questions 2 and 3): [abdullahatrash/flowleap-backend#481](https://github.com/abdullahatrash/flowleap-backend/issues/481).
- **CPC search misses and timeouts**, new data added to the existing issue (questions 2 and 4): [abdullahatrash/flowleap-backend#477](https://github.com/abdullahatrash/flowleap-backend/issues/477#issuecomment-5906177422).
- **Bridge pretty-prints tool results**, which adds about a third to large replies (question 1): [flowleap-ai/flowleap-cli#104](https://github.com/flowleap-ai/flowleap-cli/issues/104).

## Repeating the run

1. Build the CLI at the commit above and check that `flowleap mcp --check` shows 50 tools, 5 resources and 3 prompts.
2. Edit the binary path in `acceptance/mcp.json` and in `acceptance/run.sh`.
3. Run `acceptance/run.sh <N> "<question>"` once per question, one at a time, question 5 first.

The raw runs are in `acceptance/runs/`. Each `q<N>.json` is the event stream, `q<N>.err` is stderr (empty in all five), and `q<N>.started` is the UTC start time.
