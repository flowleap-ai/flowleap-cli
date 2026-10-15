---
name: flowleap-ops
description: Direct EPO Open Patent Services access through the FlowLeap backend — CQL search, EP Register search by representative or opponent, plus per-document bibliography, family, legal status and abstract worldwide, and claims and description for EP and WO (US claims from BigQuery, English only). Trigger when an agent needs authoritative EPO document data for a known publication number, claims or description text for EP, WO or US, family members, legal-status events, a raw OPS CQL search, or how many EP filings a professional representative handles. For query-building or ranked discovery use flowleap-patent; use OPS for authoritative per-document data.
---

# FlowLeap OPS

Auth and global flags: see `flowleap-shared`.

In a chat client with the FlowLeap connector, call the tools named in the `flowleap-shared` connector table instead of these commands.

Direct access to the European Patent Office (EPO) Open Patent Services API.

OPS needs the user's own EPO patent-data key. If a command returns the gate code
— the CLI's `providerKeysHint.code` `provider_keys_required`, raised from the
backend's `data_keys_required` or `patent_provider_key_invalid` (each carrying
`provider: "epo"`) — EP/WO is a **user-action stop**: decline that read and name
the free key as the fix; never serve the claims, description, or bibliography
from Google Patents, Espacenet, or a web search instead. Only those explicit
codes mean gated, and only the code — never the message wording — decides: an
empty payload, a truncated response, or a 5xx is an ordinary dead route with the
usual fallbacks. See `flowleap-keys`.

## Commands

### Search

```bash
flowleap ops search --cql <query> [flags]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--cql` | CQL query string (required) | — |
| `--start` | Start position | `1` |
| `--end` | End position | `25` |

### EP Register search

```bash
flowleap ops register-search --cql <query> [--start 1] [--end 25]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--cql` | EP Register CQL (required). Quote a multi-word value: `re="Muster Patentanwälte"` | — |
| `--start` | Start position | `1` |
| `--end` | End position (max 100 per page) | `25` |

`register-search` is the one search with a representative index. It searches
the EP Register (EP applications only) with register CQL and answers OPS's
`total` for the whole query plus one row per EP application: its application
number, earliest EP publication (`docId`, null while none is published),
title, applicants, representatives of record, status, opponents and dates.
Rows arrive in OPS register order, not ranked. Report `total` as the count and
the page as examples.

- Indexes that work: `re` (professional representative), `op` (opponent),
  `ia` (inventor or applicant), `pa`, `in`, `ti`, `pn`, `ap`, `pr`, `pd`, `ic`.
- `nm` = a name in any role (applicant, inventor, representative, opponent).
  Use it when a role index (`re`/`op`/`ia`/`pa`/`in`) returns 0, to find
  which role the person holds. The register stores names surname-first, so
  use the surname alone or `nm="Surname, First"`. Measured 2026-10-15:
  `re=Boniberger` 0, `op=Boniberger` 3, `nm=Boniberger` 5,
  `nm="Bernhard Boniberger"` 0, `nm="Boniberger, Bernhard"` 3.
- A zero-result role-index query can carry a top-level `hint` string from
  the backend (for example "No results for re=Boniberger. The name may
  appear in another role; retry with nm=Boniberger."). Human and table
  output print it as `Hint: <text>` after the Total line; `--json` keeps it
  in the data. Follow the hint before you retry a spelling.
- Not register indexes: `ag`, `rep`, `ab`, `cpc`, `st`, `ps`, `fd`, `ctr`.
  `flowleap --json tools run get_search_syntax provider=epo_register` lists
  both sets.
- `total` is capped at 10000.
- A representative count from here is EP-only: say so. `ops search` and
  `patent search` (`search_patents`) have no representative index at any
  office; a bare name there is a fuzzy full-text hit, not a representative
  count. The US attorney of record is a different thing — a USPTO-registered
  practitioner — read with `uspto attorney` per application or
  `uspto search` on `recordAttorney.attorneyBag.lastName`; an EP agent
  answers 0 there.

### Document Commands

All document commands take a patent document number (e.g., `EP1234567`).
`claims` and `description` accept `--lang` (defaults to `en`):

```bash
flowleap ops biblio <doc>                  # Bibliographic data
flowleap ops biblio <doc> --designated-states  # …plus the EPC states it designates
flowleap ops biblio <doc> --resolve-npl    # …plus the OpenAlex record of each cited paper
flowleap ops claims <doc> --lang en        # Claims text
flowleap ops description <doc> --lang en   # Full description
flowleap ops family <doc>                  # Patent family members
flowleap ops legal <doc>                   # Legal status events + designated states
flowleap ops abstract <doc>                # Abstract text
flowleap ops register-biblio <EP-doc>      # EP Register: representatives, opponents, status
flowleap ops register-search --cql <query>  # EP Register search: re=<representative>, op=<opponent>, nm=<name in any role>
```

`claims` serves EP and WO from EPO OPS and US from BigQuery (English only);
`description` serves EP and WO. For an office with no text source, use the family-member workaround in `flowleap-shared`.

`biblio --resolve-npl` looks each `citedReferences[].npl` string up in
OpenAlex and attaches `nplWork` (`status` `matched` | `candidates` |
`not_found` | `lookup_failed`, and on a match the title, DOI and abstract
preview). Only `matched` is the cited paper. It costs one OpenAlex read per
reference, so it is never on by default; see `flowleap-npl` for one reference
at a time (`npl-work`).

`register-biblio` reads the EP Register record (EP publications only): title,
applicants, status, filing and publication dates, the professional
representatives of record before the EPO, and the opponents named in any
opposition. For a US attorney of record use `uspto attorney`.

Doc IDs are normalized server-side — `ep1.000.000` and `EP1000000` both resolve.

For a non-EP-style number (US grant/application, or an original-format KR/CN
number), convert it to DOCDB first so OPS can match it:

```bash
flowleap convert-number US5443036.A --to docdb
```

A `SERVER.EntityNotFound` on some KR/CN documents is an OPS holding gap, not a
retryable error — don't retry it; fall back to another source for that document.

### Response envelope

Every `ops` command runs a tool on the Tools facade — `search_patents` for
`ops search`, `search_register` for `ops register-search`, then `get_bibliography`, `get_abstract`, `get_claims`,
`get_description`, `get_legal_status`, `get_family` and `get_register_biblio`
for the document reads.
The per-source provider route each used to call is a retired endpoint.

The facade answers one envelope,
`{ success, tool, data, executionTimeMs, cached? }`; the CLI unwraps it so
`--json` prints just `data`. Pass `--verbose` to see the cache verdict and
execution time on stderr.

Failures carry `{ success: false, error: { code, message }, status }`. **Branch
on `error.code`, never on `message`** — codes come from a closed registry and
never change once shipped, while wording is freely editable. The codes you will
see here: `INVALID_INPUT` (422, carries `issues[]`), `UNKNOWN_TOOL` (404),
`NOT_FOUND` (404), `RATE_LIMITED` (429, carries `retryAfterSeconds`),
`TOOL_EXECUTION_ERROR` (422), `INTERNAL_ERROR` (500), plus the key gate
(`data_keys_required` / `patent_provider_key_invalid`, each carrying
`provider`).

`ops family` returns the **INPADOC extended family**. `get_patent_family` is the
narrower simple-family tool — the same invention republished across offices,
without the divisionals and continuations. They answer different questions; pick
deliberately.

### Designated-state coverage: designated states, not family members

For a European regional filing the **designated contracting states** are its
designated-state coverage — which countries the right can reach. `ops family`
cannot answer that: it names the offices the invention published in (EP, TW,
WO), never the EPC states an EP application designates. Keep the two words
apart — bare "coverage" means the extended family elsewhere in FlowLeap.

`ops legal` carries them at the top level of its payload, rolled up from the
INPADOC AK and AX events:

```json
{
  "docId": "EP3804704",
  "designatedStates": ["AL", "AT", "BE", "BG", "CH", "..."],
  "extensionStates": ["BA", "ME"],
  "events": [ { "code": "AK", "designatedStates": ["AL", "AT", "..."] } ]
}
```

- `designatedStates` — the EPC states the filing designates (OPS AK event).
- `extensionStates` — extension/validation states (OPS AX event).
- Both are rolled up from the AK/AX event with the **latest date**: a designation
  set can be narrowed after publication, so the newest event is the current one
  and a union would report states no longer designated.
- Caveat: if that newest AK event carries no readable state list, an **older** AK
  answers instead, so the set can be a superseded, WIDER one. When the answer must
  be exact, read the `AK` rows in `events` yourself. For a clearance call the wide
  direction is the dangerous one.
- Both are always present and **empty for a document with no such event** —
  every non-EP publication. Empty means "designates no states", not "unknown".
- Designated ≠ still in force. Subtract the states lapsed per the `PG25`/`PGFP`
  events (each carries its `state`) to get the states still live.

`flowleap --json summary <doc>` carries the same two lists under `legalStatus`
— one call for the whole designation question. `ops biblio --designated-states`
joins them onto the bibliography instead; it costs a second EPO read, so prefer
`summary` or `ops legal` when you are reading legal status anyway. On that biblio
path a failed join lands in `errors.designatedStates` and the bibliography still
returns: read that key before you treat an absent list as "designates none".

## Examples

```bash
# CQL search
flowleap ops search --cql "ti=solar AND pa=Tesla"

# Get bibliographic data
flowleap ops biblio EP1234567

# Bibliography plus the EPC states the filing designates (extra EPO read)
flowleap ops biblio EP1234567 --designated-states

# Get claims in German (EP and WO only; US claims are BigQuery, English)
flowleap ops claims EP1234567 --lang de

# Get family members (JSON for agents)
flowleap ops family EP1234567 --json

# Search with pagination
flowleap ops search --cql "ti=battery" --start 1 --end 50

# How many EP applications a representative handles (JSON: total + one row per application)
flowleap --json ops register-search --cql 're=Kreuels' --end 3

# Oppositions one party filed, published since 2020
flowleap ops register-search --cql 'op=Siemens and pd>=20200101' --end 50

# A role index returned 0: find which role the person holds (surname alone, or surname-first)
flowleap --json ops register-search --cql 'nm=Boniberger' --end 5
flowleap --json ops register-search --cql 'nm="Boniberger, Bernhard"' --end 5

# Verbose shows cache status and timing
flowleap ops biblio EP1234567 --verbose
```

## Workflow: Deep Patent Analysis

1. Search: `flowleap ops search --cql "ti=solar AND pa=Tesla"`
2. Get details: `flowleap ops biblio EP1234567`
3. Read claims: `flowleap ops claims EP1234567`
4. Check family: `flowleap ops family EP1234567`
5. Check legal status: `flowleap ops legal EP1234567` — its `designatedStates`
   name the countries an EP filing designates

One-call alternative: `flowleap --json summary EP1234567` bundles biblio,
legal status, family, and term.
