---
name: recipe-invalidity-analysis
description: "Build an invalidity case against a patent: fix the critical date, build the Fallback Ladder (each dependent claim with the limitation it adds) from the claims, search patent and non-patent prior art for every element and every rung, chart it with X/Y/A tags, quoted passages and Fallback Coverage counts, then write a Round Report and run one search round for each expert answer. Use it when the user asks to invalidate a patent, to find 102/103 or Art. 54/56 prior art against granted claims, or to assess how strong a patent is."
metadata:
  requires:
    skills: ["recipe-prior-art-search", "recipe-claim-analysis", "recipe-find-better"]
---

# Recipe: Invalidity Analysis

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is not configured or that the office rejected the stored key, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing or rejected. On the desktop app the user adds it in Settings > Patent data connectors; on the firm OS the firm administrator sets the keys, so tell the attorney to ask the administrator; in another client, relay the page that the tool's answer names. Never ask for a key value in the chat.

Build a prior-art invalidity case against a target patent, claim by claim, with every fallback the proprietor can retreat to.

Build each query with the search method of `recipe-prior-art-search` (`references/search-method.md`); `get_search_syntax` gives the field grammar of each provider.

**Text by office.** `get_claims` and `get_description` serve EP and WO. US claims come in English. US description is not served. For another office (CN, JP, KR, …) or a missing text, read a family member (`get_family`) in an office the tools serve, and label the text as the member's text.

**Who checks the work.** In the FlowLeap Patent AI app, `write_patent_results` with `template: "invalidity-claim-chart"` builds the Fallback Ladder from the recorded `get_claims` call and checks the quotes, the coverage counts and the round log. In another client no tool checks them: you write the **Fallback Ladder**, the **Fallback Coverage** counts and the round log by hand, and mark each of them "unvalidated: written by the agent, not checked by a tool". The user is an expert. After the report, the expert decides what the next **Search Round** looks for.

## Step 1: Target intake and the critical date

- `get_patent_summary` (`patent_number`: the target): bibliography, legal status, family and term.
- `get_claims` (`patent_number`: the target). Keep the result: Step 2 builds the ladder from it.
- `get_bibliography` (`patent_number`: the target): the filing date and `dates.priority[]`. For a US target, also `get_foreign_priority` (`application_number`) for the foreign priority claims.

The earliest priority date is the critical date. Prior art must be earlier (mind grace periods and jurisdiction differences). Record it, with the tool result it came from, before you search.

## Step 2: Divide the claims and build the Fallback Ladder

Divide each asserted independent claim yourself: split on the `comprising` transition and the semicolon and `wherein` boundaries, keep the claim language of each element verbatim, and write keywords and synonyms for each element (method: `recipe-claim-analysis`, Step 4). The art must disclose every element. Track them as a checklist.

Then build the **Fallback Ladder** from the `get_claims` result of Step 1. For each independent claim, list every dependent claim under it, directly or through other dependent claims, in claim order, with the limitation it adds. A dependent claim that refers to several parents is a rung under each parent. Copy the reference wording and the added limitation from the tool result. Never type the ladder from memory. The method, the EN/DE/FR dependency wording and the table layout are in [references/fallback-coverage.md](references/fallback-coverage.md).

Done when each independent claim has an element checklist and a ladder, and each rung names its parent and its added limitation.

## Step 3: Search prior art for each element combination

Write a query for each element combination yourself, tuned broad. Invalidity is a recall search: remove classifications, OR in the synonyms of Step 2, and probe the count of every query before you trust it. The concept table holds the added limitation of every rung as a secondary concept with its own **Discriminating Term**. The first search covers the claims and every rung. The Search Round of Step 8 is only the retry.

- Patent art, both collections; element pairs carry the discrimination:
  - `search_patents` (`provider: "epo_ops"`, `query`: the CQL, `range: "1-1"`, `details: false`) to probe, then `range: "1-30"` and further pages to read.
  - `search_patents` (`provider: "uspto"`, `query`: your Lucene query, `limit: 30`). It searches titles and metadata only.
- Non-patent art, limited by the critical date: `search_npl` first (`filter.to_year`: the priority year), then `search_academic` as the second pass (`filter.to_year`). Remove duplicates by DOI or title.

Non-patent art is strong here: printed publications are fully usable in IPR, and examiners rarely searched them. Resolve the papers already of record before the search: `get_bibliography` (`patent_number`: the target, or for an EP family the A3 search-report publication, `resolve_npl: true`), or `get_npl_work` (`reference`: the cited string or DOI) for one paper. The search then logs them as art of record and looks past them. Only a `matched` record is the cited paper.

Search both offices first. Ask about scope in the Round Report (Step 8), never before the first search. A missing-key answer from one office is not an exhausted search: search the other office and the non-patent art in full, and record the gated office as an open missing-key gap in the chart.

Log every query with its tool and count, empty results too. The round log of Step 7 comes from this log.

## Step 4: Read the prosecution record

- `examiner_baseline` (`publication`: the target): the art of record, for every family member and office.
- `get_citations` (`patent_number`: the target, `type: "all"`) and, for a US target, `search_enriched_citations` (`cited_document`: the target as the office prints it, with spaces: `US 20150284881 A1`): the documents that cite it since.

Read the Examiner Baseline as `recipe-find-better` Step 1 does: any X or Y category is examiner's art, and each gap is a gap, not "nothing cited". Art the examiner never saw is stronger than art already of record.

## Step 5: Tag the references X / Y / A

- **X**: one reference that discloses every element of a claim: anticipation (35 U.S.C. 102 / EPC Art. 54).
- **Y**: a reference that invalidates only together with another: obviousness (35 U.S.C. 103 / EPC Art. 56). Record the motivation to combine.
- **A**: background or state of the art. It supports the narrative, not the ground.

For each X/Y candidate, read the full text with `get_claims` and `get_description` (`patent_number`: the reference; see **Text by office**), and check it against the element checklist and against the added limitation of every rung.

## Step 6: Ground the legal standard

Name the standard and its source: MPEP 2131 (anticipation, every element in a single reference) and MPEP 2141 to 2145 (obviousness) for the US; EPC Art. 54 and 56 and Guidelines G-VI and G-VII, 5 (problem-solution approach) for the EPO. In a client with a web tool, read each section on the office's own page and quote it with its URL and edition. The FlowLeap Patent AI app has no web tool: name the sections, and state in the report that the office pages were not read in this session.

## Step 7: Write the report

Write `<target-patent>.invalidity.md` with the template in [references/fallback-coverage.md](references/fallback-coverage.md). For each independent claim:

- The invalidity chart: rows are the claim elements of Step 2; columns are the references with X/Y/A tags and effective dates against the critical date; cells are pin cites (column and line, or paragraph) that quote the disclosure from the tool result; a combination rationale for every Y-pair, with legal citations.
- The **Fallback Coverage** line, read from the chart rows: "fallbacks: k of j covered; not covered: claims 3, 5". A rung is covered when a quoted passage discloses its added limitation, partly covered when it discloses part of it, not covered otherwise.
- The ladder as a table. Every rung has a row in the chart, or the reason it has none.
- A "Search rounds" section: one entry for each round, with the ids it answered and its queries. Round 0 is the first search.
- A Limitations line. Outside the app: "The ladder, the coverage counts and the round log are unvalidated: written by the agent, not checked by a tool."

In the FlowLeap Patent AI app, save the report with `write_patent_results` (`filePath`, `template: "invalidity-claim-chart"`, `content: ""`, `challengedPublication`, `criticalDate`, `objective` with the basis of the date, `coverage` with the element rows `"Claim 1 — element (b): …"`, one combination row for each independent claim and one `fallback` row for each rung with `claimNumber` and `fallbackOf`, `fallbacksNotCharted`, `rounds` from round 1, `limitations`). Copy each quote and source anchor verbatim from a `get_claims`, `get_description` or `get_abstract` result. In the app, a paper cannot be a coverage source: name it, with its quote, DOI and date, in the chart prose or the open points, as not checked by the writer. Each round in `rounds` needs at least one `search_patents` query exactly as it ran; the writer refuses a round of only `search_npl` or `search_academic` queries. Elsewhere, write the report as a Markdown file.

"Not covered" is a complete result, not a failure. Done when the report holds every section above for every independent claim.

## Step 8: Round Report, then wait

1. List the open points of the report as numbered questions. Each has its id in brackets and your recommended next step. The ids and their meaning are in [references/fallback-coverage.md](references/fallback-coverage.md): `rung-3`, `element-1.2`, `scope-<pub>`, `date-<pub>`, `search-N`.
2. End with the line "Reply with the ids to run (for example rung-3, element-1.3), 'all', or your own steer." Then wait. Do not search before the expert answers.
3. If the answer is bare numbers, ask "question numbers or claim numbers?" and run nothing.
4. On the expert's answer, echo "Running: [rung-3] <limitation>; [element-1.3] <element>." before any search. Then run one **Search Round** for those points only: new Discriminating Terms for each limitation, both offices, then the non-patent art, then a web search where the client has a web tool (in the app, name the web as not searched). Update the chart, add the round to "Search rounds", write the Round Report again, and wait again.
5. One round for each answer. The expert ends the loop. A rung that is still not covered after a round is a complete result.

Close every deliverable with: "AI-assisted analysis for review by a registered patent attorney — not legal advice."
