---
name: flowleap-ops
description: Direct EPO Open Patent Services access through the FlowLeap backend — CQL search plus per-document bibliography, claims, description, family, legal status, and abstract. Trigger when an agent needs authoritative EPO document data for a known publication number, full claims or description text, family members, legal-status events, or a raw OPS CQL search. For query-building or ranked discovery use flowleap-patent; use OPS for authoritative per-document data.
---

# FlowLeap OPS

Auth and global flags: see `flowleap-shared`.

Direct access to the European Patent Office (EPO) Open Patent Services API.

OPS needs the user's own EPO patent-data key. If a command returns
`provider_keys_required`, EP/WO is a **user-action stop** — decline that read
and name the free key as the fix; never serve the claims, description, or
bibliography from Google Patents, Espacenet, or a web search instead. Only that
explicit code means gated: an empty payload, a truncated response, or a 5xx is
an ordinary dead route with the usual fallbacks. See `flowleap-keys`.

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

### Document Commands

All document commands take a patent document number (e.g., `EP1234567`).
`claims` and `description` accept `--lang` (defaults to `en`):

```bash
flowleap ops biblio <doc>                  # Bibliographic data
flowleap ops claims <doc> --lang en        # Claims text
flowleap ops description <doc> --lang en   # Full description
flowleap ops family <doc>                  # Patent family members
flowleap ops legal <doc>                   # Legal status events
flowleap ops abstract <doc>                # Abstract text
```

Doc IDs are normalized server-side — `ep1.000.000` and `EP1000000` both resolve.

For a non-EP-style number (US grant/application, or an original-format KR/CN
number), convert it to DOCDB first so OPS can match it:

```bash
flowleap convert-number US5443036.A --to docdb
```

A `SERVER.EntityNotFound` on some KR/CN documents is an OPS coverage gap, not a
retryable error — don't retry it; fall back to another source for that document.

### Response envelope

OPS endpoints return data wrapped in a success/error envelope. The CLI unwraps
`data` automatically so `--json` prints just the payload. Pass `--verbose`
to see cache status and execution time. Errors use `code` values: `MISSING_PARAM`,
`NOT_FOUND`, `RATE_LIMITED`, `INTERNAL_ERROR`.

## Examples

```bash
# CQL search
flowleap ops search --cql "ti=solar AND pa=Tesla"

# Get bibliographic data
flowleap ops biblio EP1234567

# Get claims in German
flowleap ops claims US10123456 --lang de

# Get family members (JSON for agents)
flowleap ops family EP1234567 --json

# Search with pagination
flowleap ops search --cql "ti=battery" --start 1 --end 50

# Verbose shows cache status and timing
flowleap ops biblio EP1234567 --verbose
```

## Workflow: Deep Patent Analysis

1. Search: `flowleap ops search --cql "ti=solar AND pa=Tesla"`
2. Get details: `flowleap ops biblio EP1234567`
3. Read claims: `flowleap ops claims EP1234567`
4. Check family: `flowleap ops family EP1234567`
5. Check legal status: `flowleap ops legal EP1234567`

One-call alternative: `flowleap --json summary EP1234567` bundles biblio,
legal status, family, and term.
