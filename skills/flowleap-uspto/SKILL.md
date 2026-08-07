---
name: flowleap-uspto
description: Search USPTO Open Data Portal records with Lucene queries, fetch granted patents, applications, continuity chains, and file-wrapper data (prosecution transactions, assignments, foreign priority, PTA, attorney of record), list IFW documents and read office actions as OCR-extracted text, and build ODP queries from natural language. Trigger when an agent needs US application/prosecution metadata, grant lookups, continuity (parent/child) chains, office-action text, chain of title, or USPTO-specific searches.
---

# FlowLeap USPTO (Open Data Portal)

Auth and global flags: see `flowleap-shared`.

## Search

ODP uses Lucene syntax over application metadata (get the grammar via
`flowleap --json tools run get_search_syntax provider=uspto`):

```bash
flowleap --json uspto search --query 'applicationMetaData.inventionTitle:"machine learning"' --limit 5
```

Results arrive in `patentFileWrapperDataBag`.

**ODP is title + metadata only — there is no abstract/claims full-text.** The
only free-text field is `applicationMetaData.inventionTitle`. A distinguishing
feature that lives in the abstract (e.g. "UV-C sterilization" on an earbud
charging case titled only "CHARGING CASE FOR EARBUDS") cannot be matched, so
never AND an abstract-only qualifier onto an ODP search. For a recall pass,
search the **core device noun** in the title (with singular/plural variants)
and triage abstracts afterwards with `flowleap ops abstract <number>`:

```bash
flowleap --json uspto search --query 'applicationMetaData.inventionTitle:earbuds AND applicationMetaData.inventionTitle:"charging case"' --limit 25
```

**Zero-recall fallback.** If a search returns nothing, the CLI does not hand
back a silent empty set: when the query carries a `cpcClassificationBag:`
constraint it strips that filter and retries once (a mis-guessed CPC class is a
common cause of zero recall), then, if still empty, prints guidance to broaden
to a title search. Watch stderr for these notes.

**Zero recall is not a key gate.** An empty result set, a truncated payload, or
a 5xx keeps the normal recovery path above. Only an explicit
`provider_keys_required` means the US office is gated on the user's USPTO ODP
patent-data key — and that is a user-action stop: do not substitute web-scraped
US data for it, deliver the other office's results in full, name the gap as a
missing-key gap, and ask for the free key at the end. See `flowleap-keys`.

## Search with a full request body

`uspto search` accepts a complete ODP request body via `--body` (inline JSON,
or `-` for stdin) or `--body-file` — this is how you submit the object that
`uspto build-query` generates (see below). `--query` and `--body`/`--body-file`
are mutually exclusive.

```bash
flowleap --json uspto search --body '{"q":"applicationMetaData.inventionTitle:\"machine learning\"","pagination":{"limit":5}}'
flowleap --json uspto search --body-file query.json
```

## Build a query from natural language

```bash
flowleap --json uspto build-query "quantum error correction filed after 2022" --focus precise --allow-external-processing
```

Live query generation sends the description to FlowLeap and then to Anthropic
or OpenAI. `--allow-external-processing` records explicit consent; use
`--dry-run --dry-run-redacted` to inspect only the request shape locally.

Returns `strategy.recommended_query` — a **complete ODP request body** (not a
string). Submit it directly with `--body`:

```bash
flowleap --json uspto search --body '<recommended_query JSON>'
```

`--focus` is one of `broad` | `precise` | `comprehensive`. Note that the CPC
class in a generated query is a heuristic guess and can be wrong; if a run
returns 0, rely on the zero-recall fallback above or a title recall pass rather
than trusting the guessed class.

## Lookups

```bash
flowleap --json uspto grant 11800000              # granted patent by number
flowleap --json uspto application 16123456        # application by number
flowleap --json uspto continuity 16123456         # parent/child chain
```

Continuity is also available as `flowleap tools run get_continuity application_number=16123456`.

## File wrapper

Targeted projections of the application record — each returns one bag without
the full wrapper:

```bash
flowleap --json uspto transactions 14412875       # prosecution events (filings, OAs, fees)
flowleap --json uspto assignments 14412875        # chain of title (reel/frame, assignees)
flowleap --json uspto foreign-priority 14412875   # foreign priority claims
flowleap --json uspto adjustment 14412875         # official PTA day counts
flowleap --json uspto attorney 14412875           # attorney/agent of record, customer number
flowleap --json uspto associated-documents 14412875  # grant/pgpub bulk XML pointers
```

Tools-facade equivalents: `get_transactions`, `get_assignments`,
`get_foreign_priority`, `get_patent_term_adjustment`, `get_attorney` (all take
`application_number=`).

## Read office actions (IFW documents + OCR)

List the Image File Wrapper documents, then fetch any of them as markdown text.
The backend downloads the PDF from USPTO and OCRs it server-side (most IFW
documents are scanned images with no text layer) — no manual PDF handling.

```bash
# List all documents; filter to office actions by document code
flowleap --json uspto documents 14412875 --code CTNF      # non-final rejections
flowleap --json uspto documents 14412875 --code CTFR      # final rejections
flowleap --json uspto documents 14412875 --direction incoming  # applicant filings

# Read one document as markdown (documentIdentifier from the listing)
flowleap uspto document-text 14412875 K5FCIIKNRXEAPX5 > final-rejection.md
```

Common document codes: `CTNF` non-final rejection, `CTFR` final rejection,
`NOA` notice of allowance, `CLM` claims, `REM` applicant remarks/arguments.
Human/table output prints the markdown itself on stdout (metadata goes to
stderr), so `document-text` pipes cleanly; `--json` wraps it in
`{ pageCount, markdown, model, cached }`.

First read of a long document can take tens of seconds (download + OCR);
results are cached server-side for 7 days. Check `pages` in the listing before
pulling very long documents.

Tools-facade equivalents: `get_application_documents`
(`application_number=`, optional `document_code=`/`direction=`) and
`read_application_document` (`application_number=`, `document_id=`).
