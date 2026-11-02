---
name: recipe-patent-to-report
description: "Write a complete dossier on one patent: bibliography, abstract, claims, description, family with the status of each member, legal status, prosecution timeline, citations, the US family chain, figures and related art, in one structured report with its sources and gaps. Use it when the user asks for a full profile, dossier or report on a specific patent."
metadata:
  requires:
    skills: ["recipe-prior-art-search", "recipe-maintenance-fees"]
---

# Recipe: Patent to Report

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is missing or not configured, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing and where the tool's answer says to add it. Never ask for a key value in the chat.

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

### Step 3: Family, status and the US family chain

- `get_family` (`patent_number`): the extended INPADOC family. Read `get_legal_status` (`patent_number`) of each member in a state that matters, for its status and dates.
- For each US member: `get_us_grant` (`patent_number`) or `get_us_application` (`application_number`) for the serial number, publication and grant date; `get_continuity` (`application_number`) for the parent and child chain. Give the maintenance standing of each US grant as `recipe-maintenance-fees` reads it from `get_transactions`.

### Step 4: Citations

- `get_bibliography` (`patent_number`): `citedReferences`, the documents this patent cites and the search report categories.
- `get_citations` (`patent_number`, `type: "all"`): the documents that cite this patent.

### Step 5: Figures

Call `get_patent_image` (`patent_number`) for the drawing pages (`drawingStartPage`, `drawingPages`). To look at a page, call it again with `include_images: true`, `pages` (for example `[9]`) and `render: "png"`. A page is not a figure: one page can hold several figures.

### Step 6: Related patents

Call `search_patents` (`provider: "epo_ops"`, `query`, `limit: 10`) with key terms from the abstract and claim 1. Build each query with the search method of `recipe-prior-art-search` (`references/search-method.md`); `get_search_syntax` gives the field grammar of each provider. Give the query and the total with the hits.

## Output

A Markdown dossier for the patent with one section per heading below. Done when every section is present, or marked "not available" with the reason:

- **Bibliography**: title, applicant and inventor, filing and publication dates, classification.
- **Abstract**
- **Claims**: independent and dependent, with claim 1 verbatim.
- **Description summary**: condensed from the full description.
- **Figures**: the drawing pages, and the pages you looked at.
- **Family**: members by jurisdiction, with the status and dates of each.
- **Legal status**: active, expired or abandoned, and the estimated remaining term (an estimate with no term adjustment, SPC or annuity lapse).
- **Prosecution timeline**: register and INPADOC legal events.
- **Citations**: cited documents and citing documents.
- **US family chain**: the US applications and grants of the family in filing order, each with its serial number, publication, grant date and maintenance standing. When the family has no US member, say so under the heading.
- **Related art**: patents found from the abstract key terms, with the query.
- **Sources and limitations**: each tool call the dossier cites, and each gap (text not served, missing keys).

In the FlowLeap Patent AI app, save the report with `write_patent_results` (`template: "dossier"`, `filePath`, `matter`: the publication number, `subject`: the title or technology, `content`: the dossier body with the sections above). The writer refuses a dossier body without a heading that names the US family chain. Elsewhere, write the report as a Markdown file.
