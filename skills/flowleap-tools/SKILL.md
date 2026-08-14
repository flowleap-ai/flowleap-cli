---
name: flowleap-tools
description: Discover and run FlowLeap backend tools through the agent-first /v1/tools facade — one stable contract (search_patents, get_bibliography, get_claims, get_patent_summary, compare_patents, reference_search, …) instead of provider-specific endpoints. Trigger when an agent needs patent data and wants a uniform tool interface, needs to discover what the backend can do, or wants compound operations like patent summaries, comparisons, or prosecution timelines.
---

# FlowLeap Tools (agent-first facade)

The **Tools facade** is the single agent surface for patent data: named tools
invoked through `/v1/tools`, one success envelope, one error contract, a
self-describing registry. It is not one API among several — every JSON
patent-data capability is a tool.

`flowleap tools` is the direct way in. Every data command (`patent`, `ops`,
`uspto`, `citation`, `legal`, `npl`, `academic`, `analytics`, `ocr`, and the
one-call verbs) already runs on it, so use the named commands for ordinary work
and reach for `tools run` when you want a parameter the command does not expose,
a tool with no command of its own, or a schema you can read at runtime.

The per-source **provider routes** those commands used to call — the endpoints
from before the facade became canonical — are **retired endpoints**: permanently
removed, answering `410 Gone` with a machine-readable successor. Retirement is
forever, and a retired path is never reused. Named non-facade exceptions:
PATSTAT, auth/OAuth, key validation, and `api request`.

## Discover

```bash
flowleap --json tools list                     # all tools + descriptions
flowleap --json tools describe get_bibliography  # JSON input schema for one tool
flowleap --json tools openapi                  # full OpenAPI document
```

## Run

Inputs are JSON objects. Three equivalent styles — `key=value` pairs are easiest
for flat inputs; values parse as JSON when possible (numbers, booleans, arrays):

```bash
flowleap --json tools run get_bibliography patent_number=EP1000000
flowleap --json tools run search_patents --input '{"query":"ti=\"solid state battery\"","range":"1-10"}'
flowleap --json tools run compare_patents --input '{"patent_numbers":["EP1000000","US10123456"]}'
```

A run answers one envelope,
`{ success, tool, data, executionTimeMs, cached? }`; the CLI prints `data`.

## Errors — branch on the code

Failures answer `{ success: false, error: { code, message }, status }`.
**Branch on `error.code`, never on `message`.** Codes come from a closed
registry and never change once shipped; message wording is freely editable by
backend policy, so text matching is not a contract.

| Code | Status | Meaning |
|------|--------|---------|
| `INVALID_INPUT` | 422 | Input failed schema validation; carries `issues[]` |
| `UNKNOWN_TOOL` | 404 | No tool by that name — re-run `tools list` |
| `TOOL_EXECUTION_ERROR` | 422 | The tool ran and failed |
| `NOT_FOUND` | 404 | The upstream provider has no such document |
| `RATE_LIMITED` / `rate_limit_exceeded` | 429 | Back off; carries `retryAfterSeconds` |
| `INTERNAL_ERROR` | 500 | Unexpected tool failure |
| `subscription_required` | 402 | A human must subscribe; carries `upgradeUrl` |
| `data_keys_required` / `patent_provider_key_invalid` | 400 | Key gate, each carrying `provider` — see `flowleap-keys` |
| `odp_api_key_missing` | 503 | USPTO ODP key gate |
| `endpoint_gone` | 410 | Retired endpoint; the build is stale — see below |

A `410 endpoint_gone` means this CLI build called a retired endpoint. The CLI
exits **8** and prints an `endpointGoneHint` relaying the backend's `successor`
and `reason`. Never retry the same call: retirement is permanent. Run
`flowleap upgrade`, then `flowleap skills update`. Every request carries the
**Client version header** (`X-FlowLeap-Client: cli/<version>`), which is
observational only — logged as stale-client evidence, never used to reject a
request.

## Tool inventory

`flowleap --json tools list` is authoritative and discoverable at runtime —
read it rather than trusting this snapshot when the two disagree.

Search: `search_patents` (`provider=epo_ops` CQL | `uspto` Lucene),
`get_search_syntax`, `search_uspto_portfolio_by_customer_number`.

Retrieval (any publication number, EPO OPS worldwide): `get_bibliography`,
`get_abstract`, `get_claims`, `get_description`, `get_fulltext`, `get_family`
(INPADOC extended family), `get_patent_family` (simple family),
`get_legal_status`, `get_register_events`, `get_citations`, `get_patent_image`,
`convert_patent_number`.

US lookups and prosecution: `get_us_grant`, `get_us_application`,
`get_continuity`, `get_transactions`, `get_assignments`, `get_foreign_priority`,
`get_patent_term_adjustment`, `get_attorney`, `get_application_documents`,
`read_application_document`.

Citations: `search_office_action_citations` (by application),
`search_enriched_citations` (forward, by cited document), `get_citation_stats`.

Compound (one call, multiple sources): `get_patent_summary`, `compare_patents`
(2-10 patents), `get_prosecution_timeline`, `get_patent_term`.

Literature and reference: `search_academic` (Semantic Scholar + arXiv),
`search_npl` (OpenAlex), `reference_search` (patent-law RAG: EPC, EPO
Guidelines, MPEP, …), `get_legal_jurisdictions`.

Analytics and documents: `patent_analytics` (Topic Analytics, free-text
keywords), `ocr`. Meta: `server_info`.

Portfolio Analytics (structured-criteria applicant aggregation over the PATSTAT
snapshot) is **not** on the facade — it stays a named non-facade exception under
`flowleap patstat`. See `flowleap-patstat` for the routing rule against Topic
Analytics (`flowleap analytics`) and the `data_edition` contract.

## Recipes

Patent snapshot in one call:

```bash
flowleap --json tools run get_patent_summary patent_number=EP1000000
```

Portfolio triage:

```bash
flowleap --json tools run search_uspto_portfolio_by_customer_number customer_number=23456 limit=50
```

Search queries are written by you — `flowleap-patent` carries the CQL method
(term extraction, discriminating term, mandatory count probe). The probe is the
same tool with `details=false`, which skips the per-document bibliography
fan-out — bare document references and the `total` you came for, at a fraction
of the cost:

```bash
flowleap --json tools run search_patents query='ta="battery separator" AND ta="solid state"' range=1-1 details=false
flowleap --json tools run search_patents query='ta="battery separator" AND ta="solid state"' range=1-20
```

Reachability check — is the backend up, and which build?

```bash
flowleap --json health api    # public readiness probe; reports apiVersion
```

Never probe reachability with a search: it costs a provider call, needs a
subscription and a patent-data key, and tells you nothing `health api` does not.

## Auth

Auth, subscription, and rate limits: see `flowleap-shared`.
