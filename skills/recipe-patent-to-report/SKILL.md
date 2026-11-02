---
name: recipe-patent-to-report
description: "Write a complete dossier on one patent: bibliography, abstract, claims, description, family with the status of each member, legal status, prosecution timeline, citations, the US family chain, figures and related art, in one structured report with its sources and gaps. Use it when the user asks for a full profile, dossier or report on a specific patent."
metadata:
  requires:
    skills: ["recipe-prior-art-search", "recipe-maintenance-fees"]
---

# Recipe: Patent to Report

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is not configured or that the office rejected the stored key, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing or rejected. On the desktop app the user adds it in Settings > Patent data connectors; on the firm OS the firm administrator sets the keys, so tell the attorney to ask the administrator; in another client, relay the page that the tool's answer names. Never ask for a key value in the chat.

Read all data of one patent document and assemble it into one structured dossier. Give each fact with the tool and the publication number it came from. A section with no data says why.

## Steps

### Step 1: Snapshot and timeline

- `get_patent_summary` (`patent_number`): bibliography, legal status, family and estimated term in one call.
- `get_prosecution_timeline` (`patent_number`): EP register events and INPADOC legal events in date order.

### Step 2: Full text

- `get_abstract` (`patent_number`)
- `get_claims` (`patent_number`)
- `get_description` (`patent_number`)

Office coverage: `get_claims` and `get_description` serve EP and WO; US claims come in English; US description is not served. For another office (CN, JP, KR, …) or a missing text, read a family member (`get_family`) in an office the tool serves, and label the text as the member's text.

For a US patent, read the as-filed specification from the file wrapper: `get_application_documents` (`application_number`, `document_code: "SPEC"`), then `read_application_document` (`application_number`, `document_id`: the `documentIdentifier`). Label it as the as-filed US specification, not as a family member's text. `read_application_document` reads the document with Mistral OCR: without a Mistral key this text is a missing-key gap.

### Step 3: Family, status and the US family chain

- `get_family` (`patent_number`): the extended INPADOC family. Cite each member by its `publication`. A `docdbApplication` (or `number`) value is a DOCDB application id: never print it as a US serial or as a filing or priority number.
- For each EP member: `get_bibliography` (`patent_number`, `include_designated_states: true`) for the designated and extension states, and `get_legal_status` (`patent_number`) for the status per state: annual-fee events (for example PGFP) and lapse events per contracting state (for example PG25), each with its date. For another member in a state that matters, read its `get_legal_status`.
- For each US member: `get_us_grant` (`patent_number`) or `get_us_application` (`application_number`) for the serial number (`applicationNumberText`), publication and grant date; `get_continuity` (`application_number`) for the parent and child chain. Give the maintenance standing of each US grant as `recipe-maintenance-fees` reads it from `get_transactions`.
- Expiry: state an expiry only from a retrieved term or adjustment record, or mark it as an estimate. `get_patent_term` counts 20 years from the member's own filing date: for a US continuation or divisional the term runs from the earliest US non-provisional filing date that the member claims (35 USC 154(a)(2); take it from `get_continuity`), and the estimate ignores a terminal disclaimer (a `DIST` event in `get_transactions`) and term adjustment. For each US grant, add `get_patent_term_adjustment` (`application_number`: the `applicationNumberText`) for the official adjustment days. A terminal disclaimer makes the expiry an open check item: the disclaimer text names the patent it ties the term to.

### Step 4: Citations

- `get_bibliography` (`patent_number`): `citedReferences`, the documents this patent cites and the search report categories.
- `get_citations` (`patent_number`, `type: "all"`, `range: "1-100"`): the documents that cite this patent, and `total`. When `total` is more than 100, read the next pages (`range: "101-200"`, …, at most 100 per page) and give `total` with the count read.
- For each US member: `search_enriched_citations` (`cited_document`: the US publication number as the office prints it, with spaces, for example `US 20150284881 A1`; the compact form returns 0) for the USPTO office actions that cite it. With FlowLeap Data, `patstat_patent` gives the citation origin (for example examiner search, applicant) of every citation.
- The forward-citation count names every route checked. A zero is a zero from the routes checked, not a verified absence.

### Step 5: Figures

Call `get_patent_image` (`patent_number`) for the drawing pages (`drawingStartPage`, `drawingPages`). To look at a page, call it again with `include_images: true`, `pages` (for example `[9]`) and `render: "png"`. A page is not a figure: one page can hold several figures.

### Step 6: Related patents

Call `search_patents` (`provider: "epo_ops"`, `query`, `limit: 10`) with key terms from the abstract and claim 1. Build each query with the search method of `recipe-prior-art-search` (`references/search-method.md`); `get_search_syntax` gives the field grammar of each provider. Give the query and the total with the hits.

## Output

A Markdown dossier for the patent. Write each section below as a second-level (`##`) heading, with these heading names. Done when every section is present, or marked "not available" with the reason:

- `## Bibliography`: title, applicant and inventor, filing, priority and publication dates, classification.
- `## Abstract`
- `## Claims`: independent and dependent, with claim 1 verbatim.
- `## Description summary`: condensed from the full description, labelled with the text it came from.
- `## Figures`: the drawing pages, and the pages you looked at.
- `## Family`: members by jurisdiction, each by its publication number, with the status and dates of each designated state.
- `## Legal status`: active, expired or abandoned, with the events and dates. An expiry comes from a retrieved term or adjustment record, or is marked as an estimate with what it leaves out (term adjustment, terminal disclaimer, SPC, annuity lapse).
- `## Prosecution timeline`: register and INPADOC legal events.
- `## Citations`: cited documents with their search report categories; citing documents, the forward-citation count with each route checked.
- `## US family chain`: the US applications and grants of the family in filing order, each with its serial number, publication, grant date and maintenance standing. When the family has no US member, say so under the heading.
- `## Related art`: patents found from the abstract key terms, with the query.
- `## Sources and limitations`: each tool call the dossier cites, and each gap (text not served, missing keys, check items).

In the FlowLeap Patent AI app, save the report with `write_patent_results` (`template: "dossier"`, `filePath`, `matter`: the publication number, `subject`: the title or technology, `content`: the dossier body with the `##` sections above). The writer keeps a body with three or more `##` headings as written, and then refuses it without a heading that names the US family chain; `## US family chain` meets that rule. Elsewhere, write the report as a Markdown file.
