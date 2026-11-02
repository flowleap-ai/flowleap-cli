---
name: recipe-infringement-charting
description: "Build an element-by-element infringement claim chart: check that the patent is in force where it matters, break the asserted claims into elements, extract evidence about the accused product (with OCR of datasheets and manuals), and map each element with literal and doctrine-of-equivalents columns, each cell tied to its evidence. Use it when the user asks whether a product infringes a patent, wants a claim chart or evidence-of-use chart, or needs an element-by-element infringement map."
metadata:
  requires:
    skills: ["recipe-claim-analysis"]
---

# Recipe: Infringement Charting

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is not configured or that the office rejected the stored key, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing or rejected. On the desktop app the user adds it in Settings > Patent data connectors; on the firm OS the firm administrator sets the keys, so tell the attorney to ask the administrator; in another client, relay the page that the tool's answer names. Never ask for a key value in the chat.

Map asserted claims onto an accused product, element by element.

## Step 1: Asserted patent intake

- `get_patent_summary` (`patent_number`: the asserted patent): bibliography, legal status, family, term.
- `get_claims` (`patent_number`): the asserted claims.
- `get_description` (`patent_number`): the specification, for claim construction.

`get_claims` and `get_description` serve EP and WO; US claims come in English; US description is not served. For another office (CN, JP, KR, …) or a missing text, read a family member (`get_family`) in an office the tool serves, and label the text as the member's text.

Done when you have the asserted claims verbatim and the description (or the gap named).

## Step 2: Enforceability check

A chart against an expired or lapsed patent is wasted work. Check first:

- `get_legal_status` (`patent_number`): grant, lapse, opposition and other status events.
- `get_patent_term` (`patent_number`): the expiry estimate, 20 years from filing, with no term adjustment, SPC or annuity lapse. For a US patent, `get_patent_term_adjustment` (`application_number`) gives the official adjustment.
- `get_family` (`patent_number`): the extended family, to find where a member is in force. Read `get_legal_status` of each member in a jurisdiction that matters.

Chart only in jurisdictions with a live family member. Give each status and date with the tool that gave it.

## Step 3: Decompose the asserted claims

Decompose each asserted independent claim yourself: split on the `comprising` transition and the semicolon and `wherein` boundaries, and keep the claim language **verbatim** per element. The chart maps accused-product evidence against the claim words, not a paraphrase (the method is in `recipe-claim-analysis`, Step 4).

The element list becomes the rows of the chart. Every element must be met: one missing element defeats literal infringement of that claim.

## Step 4: Accused-product evidence intake

- A file the user gave: in the app, `read_pdf` (`path`: the workspace file); with the connector, `ocr` (`file` and `filename`).
- A PDF at a public URL: `ocr` (`url`).
- A web page (a product page, marketing text): fetch it with your web tool and quote only text the page returned, with its URL. The FlowLeap Patent AI app has no web tool: there, ask the user for the page as a file, and write that the page was not read.

Collect datasheets, manuals, teardown reports and marketing claims. Record the source of every document (file name or URL, page, date). The chart cites evidence, not impressions.

## Step 5: Build the chart

| Claim element | Claim language | Accused product feature | Evidence (doc + location) | Literal? | DoE? |
|---------------|----------------|-------------------------|---------------------------|----------|------|

- **Literal**: the element reads directly on the product feature.
- **DoE** (doctrine of equivalents): insubstantial difference by function, way and result. Flag prosecution-history estoppel risks from the file history (US: `get_application_documents` and `read_application_document` for the amendments and remarks; EP: `get_register_events`).
- An element with no evidence is "no evidence found", never "met".

The doctrine of equivalents and prosecution-history estoppel are US case law, not MPEP text. Read the MPEP section that names the decisions (MPEP 2186) from the office's own page with your web tool, quote it with its URL and edition, and name the decisions it cites. The FlowLeap Patent AI app has no web tool: there, write the legal basis as a "check: MPEP 2186" item and write that the page was not read.

## Step 6: Construction disputes

For each element where the mapping is contested, take the specification passages that define the term (the description from Step 1) and record both constructions, with the chart cell marked "disputed".

## Output

- Enforceability memo (status, term, jurisdictions), each fact with its tool.
- Element-by-element claim chart with literal and DoE columns and evidence cites.
- List of disputed constructions with the specification support for each side.
- Gaps: text not served, evidence not found, pages not read, missing keys.
- Caveat block: this is an analysis aid, not a legal opinion.

In the FlowLeap Patent AI app, save the report with `write_patent_results` (`template: "eou-infringement-chart"`, `filePath`, `matter`: the patent number and the asserted claims, `subject`: the accused product, `content`: the memo, the chart, the disputed constructions and the gaps). Elsewhere, write the report as a Markdown file.
