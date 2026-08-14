---
name: recipe-audit-report
description: Governance recipe for producing an auditable record of AI-assisted patent research — reproducible command log, data provenance for every finding, portfolio status verification, and an AI-usage disclosure section suitable for internal or filing-adjacent records. Trigger when the user asks for an audit trail of patent research, an AI-assistance disclosure, or a verifiable methodology write-up of analysis work.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-tools", "recipe-custom-dashboard"]
---

# Recipe: Audit Report (AI-Assisted Research)

Make AI-assisted patent research verifiable: every finding traceable to a
command, every command re-runnable.

## Step 1: Record the Environment

```bash
flowleap --version
flowleap --json health api    # backend apiVersion (the server build)
flowleap --json doctor        # backend, auth mode, provider-key status
```

Record: CLI version, backend `apiVersion`, backend base URL, date/time of the
session, and the auth *mode* (OAuth session vs. API token) — **never token
values or key material**.

Both versions matter for re-runnability. The CLI sends its own build as the
**Client version header** (`X-FlowLeap-Client: cli/<version>`), so a later
reader can tell which client produced the log; `apiVersion` pins the server side
of the same pair.

## Step 2: Provenance Discipline During Research

For every substantive finding, log the exact command and keep the `--json`
output. The Tools facade makes provenance explicit — one surface, one envelope,
one named tool per capability, so "which system answered this" is never a guess:

```bash
flowleap --json tools list                     # capabilities available that day
flowleap --json tools run server_info          # backend identity/version
```

Note per result: the source system (EPO OPS, USPTO ODP, OpenAlex, Semantic
Scholar, legal RAG), the query string, limits and date filters, and the
`cached` flag from the response envelope (cached data may lag live records).

## Step 3: Verify Asset Status Claims

Any statement like "patent X is in force" must be backed by a dated check:

```bash
flowleap --json summary <patent-number>     # legal status + family + term
flowleap --json timeline <patent-number>    # event history behind the status
```

## Step 4: Reproducibility Pass

Re-run the load-bearing commands at report time and diff against the
original outputs. Patent registers move — note any finding that changed
between research and reporting, with both dates.

## Step 5: AI-Usage Disclosure Section

State plainly:
- Which steps were AI-assisted (search strategy, summarization, charting)
  and which tool/model performed them
- Which outputs a human verified, and how (e.g. claims read in full,
  legal-status events checked against the register)
- Known limitations: search recall is not exhaustive; OCR and summarization
  can introduce errors; results reflect database coverage on the query date

For filings, check the current duty-of-disclosure guidance:

```bash
flowleap --json legal search "duty of candor AI assisted tools" --jurisdiction uspto
```

## Visual deliverable

If the research being audited produced a `recipe-custom-dashboard` bundle,
cite it rather than re-deriving its numbers: the bundle's own provenance
footer and reproduce block already meet this recipe's Verified-Data Contract
bar (sources, parameters, Data Edition where applicable, timestamps),
so Steps 1–4 can point at that bundle instead of re-collecting the same
evidence by hand.

## Output

- Environment record (versions, backend, date, auth mode)
- Methodology section (sources, queries, filters — per finding)
- Status-verification table with check dates
- Command appendix: every command, in order, with output digests
- AI-usage disclosure with human-verification points and limitations
