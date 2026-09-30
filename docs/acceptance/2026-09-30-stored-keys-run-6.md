# Acceptance run 6: stored Patent-Data Keys over the hosted MCP server, 2026-09-30

Issue: backend [#519](https://github.com/abdullahatrash/flowleap-backend/issues/519), the acceptance ticket of spec [#511](https://github.com/abdullahatrash/flowleap-backend/issues/511) (stored Patent-Data Keys). This run proves the two ends of the feature over the **hosted MCP server**, with a personal token as sign-in and **no key headers**: an office tool works on a **stored key**, and after the key is deleted the refusal hands the human the keys page. Runs 4 and 5 are the PATSTAT runs; this run uses the EPO OPS office tools.

**Overall verdict: PASS.** Question 1 (stored key present) and question 3 (stored key deleted, uncached patent) meet the acceptance criteria of #519. Question 2 found one gap in a composite reply, filed as backend [#529](https://github.com/abdullahatrash/flowleap-backend/issues/529), and taught one fact about the document cache.

| # | Question | State of the account | Verdict |
|---|---|---|---|
| 1 | Summary of EP3477840B1: bibliography, first claim, legal status, family | EPO key stored, no headers | PASS |
| 2 | Same question again | EPO key deleted | PASS with a gap (#529): cached sections still answer; the uncached family block refuses without a link |
| 3 | Bibliography of EP4300000A1 (uncached) | EPO key deleted | PASS: `data_keys_required` with `keysPageUrl` and the human next step, relayed by the agent |

## Transport

| Item | Value |
|---|---|
| Transport | Hosted MCP server, `https://api.flowleap.co/mcp`, stateless JSON |
| Sign-in | Personal token `gate6-stored-keys-2026-09-30` as a bearer, read from a file outside the repo (`FLOWLEAP_GATE_TOKEN_FILE`); revoked after the run |
| Key headers | None. No `X-EPO-OPS-Key` on any request |
| Backend `apiVersion` | `1.0.0+69e7a2d7` (main after #527, #526, #525, #524, #523, #522) |
| `/v1/health` `storedKeys` | `{ "enabled": true, "keyId": "1" }` (the operator set the master key, #521) |
| Website | `https://www.flowleap.co/en/dashboard/keys` live behind sign-in (website #368); the /mcp page carries the "Add a key" sentence |
| Harness | `acceptance/run.sh` with `MCP_CONFIG=acceptance/mcp-hosted.json`, `RUNS_DIR=acceptance/runs-6-stored-keys`; `claude -p`, `--strict-mcp-config`, tools limited to the FlowLeap server, `--max-turns 25` |
| Raw sessions | `acceptance/runs-6-stored-keys/q1.json`, `q2.json`, `q3.json` (stream-json; no token value in any file) |

## Setup

1. The account had no stored key: `GET /v1/keys/stored` gave `present: false` for both offices.
2. The EPO OPS key and secret of the machine's CLI config were stored with `PUT /v1/keys/stored/epo` through `flowleap api request` from a temporary file (the value never appeared in a terminal or a log). The office check passed: HTTP 200, `storedAt` and `lastVerifiedAt` = `2026-09-30T17:24:21Z`.
3. Question 1 ran.
4. `DELETE /v1/keys/stored/epo` gave HTTP 200; `GET` then gave `present: false`.
5. Questions 2 and 3 ran.

## Question 1: stored key present

Tools called, all in one parallel turn: `get_bibliography`, `get_claims`, `get_legal_status`, `get_family`. No tool error. The answer gave the title (Welding transformer), the applicant (University of Maribor), the five inventors, the filing and grant dates, the priority (SI 201700288), the IPC and CPC classes, the first claim, the legal status and the family. Turns: 5.

This is the acceptance criterion of #519: an office tool succeeds over `/mcp` with no key headers, on the stored key.

## Question 2: stored key deleted, same patent

Tools called: `get_patent_summary`, `get_claims`, `get_legal_status`, `get_family`. No tool-level error. The bibliography, claims, legal status and family answered from the document cache, which serves a document read earlier by any caller before the key gate runs (the unified cached provider read). Only the `family` block inside `get_patent_summary` was not cached, and it refused with the text "EPO OPS data keys are required. Your trial's shared patent-data access has ended — add your own EPO OPS key to continue." and nothing else. The agent reported, correctly, "It gave no link".

Two conclusions:

- A delete takes effect on the next uncached read. A cached document keeps answering, which is the cache's contract and not a key leak: the cache holds documents, not credentials.
- A refused **section** inside a composite reply carries only message text: no `code`, no `keysPageUrl`, no `nextStep`. A whole-call refusal carries all of them (#516). Filed as backend #529.

## Question 3: stored key deleted, uncached patent

One tool call, `get_bibliography` for EP4300000A1, refused. The raw envelope over `/mcp`, captured with curl in the same state:

```json
{"success":false,"error":{"message":"EPO OPS data keys are required. Your trial's shared patent-data access has ended — add your own EPO OPS key to continue.","type":"invalid_request_error","code":"data_keys_required","provider":"epo","keysPageUrl":"https://www.flowleap.co/en/dashboard/keys","nextStep":{"id":"store-epo-keys","title":"Add your EPO OPS consumer key and secret on the FlowLeap Patent-data keys page","actor":"human","url":"https://www.flowleap.co/en/dashboard/keys"}},"status":400}
```

The agent's answer, in full: it named the refusal, quoted the message, gave the page URL as the action, and added "Do not paste the key here; it must be added on that page." Turns: 2.

This is the second acceptance criterion of #519: the refusal carries the page URL and the human next step, and the agent relays the link.

## Not verified in this run

- The patent activity rows (`key_source_epo` = `stored`, `transport` = `mcp` for question 1) live in the production database, which this harness cannot read. The operator can confirm with one query on the host:

```sql
select created_at, transport, key_source_epo, key_source_uspto, tool
from patent_activity where created_at > '2026-09-30T17:20:00Z' order by created_at;
```

- The keys page itself was not driven in the browser during this run; the key was stored and deleted over the API. The page's component tests passed in website PR #368, and the page answers behind sign-in in production. A manual add and delete on the page is the operator's check.

## Follow-ups

- Backend #529: section-level refusals in composite replies carry the typed shape and the page link.
- Backend #528: the validate reply carries `keysPageUrl` while stored keys are enabled, so `flowleap doctor` can point to the page.
- CLI #119: port the doctrine to the VS Code app skill.
- Backend #520: URL-mode elicitation opens the page from the chat.
