---
name: recipe-prior-art-search
description: "Search for prior art before filing or against a published patent: synonym rows for each element, patent searches in the EPO and USPTO collections with a classification-only track and a citation track, a literature search (OpenAlex first), a recall check against the Examiner Baseline, and X/Y/A tags on the closest hits, each statement tied to the tool result it came from. Its search method is the one the other searching methods use. Use it when the user asks to find prior art for an invention, to run a novelty search, or to check what already exists before filing."
metadata:
  requires:
    skills: []
---

# Recipe: Prior Art Search

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is not configured or that the office rejected the stored key, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing or rejected. On the desktop app the user adds it in Settings > Patent data connectors; on the firm OS the firm administrator sets the keys, so tell the attorney to ask the administrator; in another client, relay the page that the tool's answer names. Never ask for a key value in the chat.

A multi-step method for a complete prior-art search. Build each query with the search method in [references/search-method.md](references/search-method.md); `get_search_syntax` gives the field grammar of each provider.

**Text by office.** `get_claims` and `get_description` serve EP and WO. US claims come in English. US description is not served. For another office (CN, JP, KR, …) or a missing text, read a family member (`get_family`) in an office the tools serve, and label the text as the member's text.

## Steps

### Step 1: Write the search queries

Follow sections 1 to 3 of the search method:

1. Extract the terms and write one synonym row for each claim element.
2. When the subject is a published document, read its full `cpc[]` with `get_bibliography`, including the indexing codes. They feed the classification-only track of Step 2.
3. Write the CQL around a Discriminating Term. Verify each CPC code before you use it.
4. Probe the count with `search_patents` (`range: "1-1"`, `details: false`) and read `total`. Over about 1,000: add the next discriminating term. Under 10: broaden. This is a novelty search, so start broad and narrow from the count.
5. Write the USPTO query (section 5 of the search method): the same terms, with a Discriminating Term that can be in a title.

Show the statement as a Search Statement block (section 8 of the search method).

Done when you have one probed EPO CQL query and one probed USPTO Lucene query.

### Step 2: Search patents (EPO and USPTO)

Run both collections. They are not interchangeable:

- `search_patents` with `provider: "epo_ops"` searches the EPO bibliographic data, which includes the EPO copy of US documents.
- `search_patents` with `provider: "uspto"` searches the USPTO Open Data Portal, which has the US application and prosecution data that the EPO copy does not have. For US prior art, run this leg; do not use `countries: ["US"]` on `epo_ops` in its place.

The USPTO leg searches titles and metadata only. When it returns 0, follow the zero-recall steps in section 5 of the search method: remove a `cpcClassificationBag` filter once, then run a title recall pass on the core device noun and triage the hits with `get_abstract`.

Run at least two independent tracks, so that one track catches a document the other misses:

- **Keyword track**: the queries of Step 1, built from the synonym rows.
- **Classification-only track**: the main group AND combinations of 2-3 indexing codes that map to essential elements, with the date limit and no keywords (section 2 of the search method). Try several combinations and log each with its count.
- **Citation track**: Step 4, one hop from the closest hits.

Read the whole set, never page 1 only (section 4 of the search method): page with `range: "101-200"` and so on, or narrow the query until you can read the set. Compare hits by family with `get_family`, not by number.

Done when both collections have returned results, or one is an open missing-key gap, and you read in full each set you rely on.

### Step 3: Search non-patent literature

Follow section 6 of the search method:

1. `search_npl` first, with `filter.to_year` set to the critical year. Run one variant for each essential feature. Check each publication date against the exact critical date.
2. `search_academic` as the second pass. Remove duplicates by DOI or title. A source with status `rate_limited` or `error` is a gap.
3. Resolve the papers an office already cited with `get_bibliography` (`resolve_npl: true`) or `get_npl_work`. Only a `matched` record is the cited paper.
4. Read past the abstract before you rely on a paper. Cite it by authors, title, venue, date and DOI.

Log each query with its count.

### Step 4: Read the closest hits

Read each hit whose abstract maps to at least one independent feature of the invention, and at minimum the top 5:

- `get_abstract`, `get_claims` and `get_description` (`patent_number`). For the office limits, see **Text by office** above.
- `get_family` (`patent_number`) to collapse the family.

**One hop from the closest hits** (the citation track). For each closest candidate of the keyword and classification tracks, and for each Examiner Baseline document after Step 5, read:

- its cited references: `citedReferences[]` of `get_bibliography`;
- its forward citations: `get_citations` (`type: "all"`) and, for a US document, `search_enriched_citations` (`cited_document`).

Keep the documents published before the cutoff that share a CPC main group or indexing code with the subject. When the disclosure names a source or family publication, read its cited references first (for an EP family they are on the A3 search-report publication). Those documents are of record, not found by your search: keep them apart for the recall check.

Record how deeply you read each document: `bibliography/abstract only` or `full text read` (claims and description). Map an element to a passage only after a full-text read. You can name an abstract-only document as a candidate, but it never supports a row.

Done when each qualifying hit has its claims and family read and the citation hop has run.

### Step 5: Recall check against the Examiner Baseline

When the subject is a published patent or application, or the disclosure names one, check the search against what the examiners cited, after the keyword, classification and citation tracks have run. Call `examiner_baseline` (`publication`: any family member).

1. Take every document an examiner cited (X, Y and A categories, international and national searches). Keep the documents only the applicant cited in a separate list.
2. A Baseline document counts as **found** only when a search query returned it or a member of its family. A `pn=` lookup, or a read of the source's own cited references, does not count.
3. For each miss, find the cause: compare its CPC codes and abstract wording with your queries (a missing synonym or indexing code, or a page you did not read). Run the query that removes the cause and log it with its purpose. Repeat until a search finds each miss or you can explain it.
4. State the result as **"found n of m examiner-cited documents by search"**. List each miss with its cause, and put each miss you cannot explain under limitations. A low recall means the queries are too narrow for new art too: say so, and do not present new candidates as complete.
5. Copy each entry of `gaps[]` into the result as a gap, never as "nothing cited".

Then search for art outside the Baseline, starting from the claim elements that the examiners' documents do not cover.

### Step 6: Tag each reference X / Y / A

Against the invention's features, tag every retained reference:

- **X**: alone anticipates a feature (novelty-destroying).
- **Y**: anticipates only together with another reference.
- **A**: general background.

Done when every retained reference has an X/Y/A tag, X-tagged first.

### Step 7: Map the X references element by element

For each X-tagged reference, show the anticipation for each element, not for the document as a whole. One table, one row for each invention feature (or claim element, when the user supplied a claim), one column for each X reference:

```
| Claim element | <Reference 1> | <Reference 2> |
```

Each cell quotes the disclosing passage from the tool result (`get_claims` or `get_description`) with its anchor: the original language and a translation where necessary. A feature that no X reference discloses is the novelty candidate; say so under the table.

Done when each element row has a quote from at least one X reference or is named as a possible point of novelty.

## Rules

- Never judge a result set from its first page: read it in full or narrow it until you can.
- Always run the Step 5 recall check when the subject has an Examiner Baseline, and report "found n of m".
- Tie each statement about a document to the tool result it came from: tool, input, document number and passage. Read dates from the record. State each gap as a gap.

## Output

A prior-art table with:

- One row for each patent family (the closest member represents the family; collapse duplicates with `get_family`).
- An X / Y / A tag on each row, X-tagged references first.
- The element-by-element mapping table for the X references (Step 7).
- Papers on the same subject, cited by authors, title, venue, date and DOI.
- The tracks that ran, each query with its tool and count, and the reading depth of each document.
- The recall result "found n of m examiner-cited documents by search", with each miss and its cause, when the subject has an Examiner Baseline.
- Claims and abstracts of the closest prior art.
- The gaps: missing-key gaps, literature sources that failed, offices with no text.

In the FlowLeap Patent AI app, save the report with `write_patent_results` (`template: "prior-art-report"`, `content: ""`, `subject`, `objective`, `searchStrategy`, `concepts`, `classifications`, `coverage` with one row for each feature and one essential combination row, `limitations`, `stopReason`). Copy each quote and source anchor in `coverage` verbatim from a `get_claims`, `get_description` or `get_abstract` result. Elsewhere, write the report as a Markdown file.
