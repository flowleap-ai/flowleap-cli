---
name: recipe-find-better
description: "Find closer prior art than the examiners cited against a granted patent: read the Examiner Baseline (every document cited across the family, by office), run three logged search tracks (backward citations two levels deep, inventor and author networks, classifications with discriminating terms), and score the examiners' best art and the new art on the same claim elements as 'disclosed n of m', each row quoted from a tool result. Use it when the user asks whether earlier or closer prior art exists than the examiners cited, or what the examiners missed on a granted patent. Not for a novelty search before filing (recipe-prior-art-search) or for an invalidity case (recipe-invalidity-analysis)."
metadata:
  requires:
    skills: ["recipe-prior-art-search", "recipe-claim-analysis"]
---

# Recipe: Find Better

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is missing or not configured, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing and where the tool's answer says to add it. Never ask for a key value in the chat.

Given a granted patent, find prior art earlier or closer than the art the examining offices cited. Show, for each independent claim, the examiner's best art beside the best art found, as element counts. "No better art found" with the full Examiner Baseline and the full tracks log is a complete result.

Find Better makes no invalidity statement and gives no model-rated score. The only comparison is a count over quoted element rows.

Build each query with the search method of `recipe-prior-art-search` (`references/search-method.md`); `get_search_syntax` gives the field grammar of each provider.

**Text by office.** `get_claims` and `get_description` serve EP and WO. US claims come in English. US description is not served. For another office (CN, JP, KR, …) or a missing text, read a family member (`get_family`) in an office the tools serve, and label the text as the member's text.

## Step 1: Baseline

Call `examiner_baseline` (`publication`: the granted publication). Read `documents[]`, `gaps[]` and `membersWalked[]` (field guide: [references/baseline-json.md](references/baseline-json.md)). Record the critical date: the earliest `dates.priority[].date` from `get_bibliography` of the granted publication. Write it as `YYYY-MM-DD` in the report.

The **examiner's best art** for an independent claim is every document with an X or Y category on a citation that reaches that claim, whatever its `citedBy` says (only an examiner assigns a category). Rank X before Y, then by the number of offices that cite it, then by the number of independent claims it reaches. When no X or Y citation exists (a US-origin family often has no categories), take the documents with `citedBy: "examiner"` and no category, then the documents that a US office action rejected claims with under 102 or 103 (`source: "uspto_enriched"`). The report shows that basis beside each document. Never take a document that only the applicant cited, with no category and no rejection. Each entry in `gaps[]` goes in the report as a gap, never as "nothing cited".

Done when every independent claim has its examiner's best art list, or the words "no X or Y citation reaches this claim", and every gap is recorded.

## Step 2: Claims

Read the claims of the granted publication with `get_claims` (see **Text by office**). Divide each independent claim into elements (method: `recipe-claim-analysis`, Step 4). Give each element its **Discriminating Terms** (section 1 of the search method).

Done when each independent claim is an element list with its Discriminating Terms.

## Step 3: Tracks

Run all three tracks. Each one is mandatory. The tool calls are in [references/tracks.md](references/tracks.md).

1. **Backward citations, two hops** from every document of the examiner's best art (the X or Y documents, or, when none exists, the examiner-cited documents from Step 1).
2. **Inventor and author networks**: the target's inventors, the inventors of the X/Y patents, and the authors of the X/Y non-patent documents.
3. **Classification co-occurrence**: the classifications of the target and of its X/Y documents, combined with the Discriminating Terms. One query is not sufficient. Do the **term-drop pass**: run the first query again with ONE term removed at a time, and once with the classification alone when the first query gave 200 hits or fewer. Log each variant with its count, and read the titles of the top hits of each variant before the critical date. Then run one **title-phrase query** for each Discriminating Term, `ti="<two-word phrase from the claim>"`, because old US documents often have no abstract in the EPO data. When the target has 2000-series indexing codes, also run the main group with combinations of 2-3 indexing codes it shares with the examiner's documents, with no terms. Read every result set in full (page with `range: "101-200"` and so on) or narrow it until you can; never judge a set from its first page. Then log the **recall check**: how many Baseline documents, matched by family, the Track 3 queries returned. Show the statement as a Search Statement block (section 8 of the search method).

Keep only documents published before the critical date. Log every query with its tool and count in the working record, empty results too.

Done when each track has its log, Track 3 has its term-drop variants, its title-phrase queries, its indexing-code combinations and its recall check, and each kept candidate has its claims or full text read.

## Step 4: Compare

For each independent claim, score the top examiner's art and each candidate on the SAME element rows from Step 2. A row counts as disclosed only with a quoted passage from a tool result (`get_claims`, `get_description` or `get_abstract`) and its anchor (claim, paragraph, column and line, or figure). Write "disclosed n of m" for each document.

"Better" means more elements disclosed, or the same count with an earlier date. If no candidate beats the examiner's art, write "no better art found" for that claim. That is a complete result, but only when all three tracks ran at least one query and Track 1 has a hop 2. Otherwise write "Search incomplete for claim N: <reason>" (for example "Track 2 ran no query" or "Track 1 has no hop 2"). Name in the result line only the examiner documents you scored. List the other examiner's best art under "named but not scored". Flag a document with a P or E category, or published after the critical date, as "published after the critical date; not prior art for this claim unless the priority claim fails".

Done when every independent claim has both columns scored on the same rows.

## Step 5: Record

Write the report and the working record with the structure in [references/report-template.md](references/report-template.md): one row for each independent claim, the Baseline matrix, the gaps, and the tracks log. Mark each Track 1 query `hop 1` or `hop 2`. If hop 1 found no document outside the Baseline, log one `hop 2` entry that says so, with count 0.

In the FlowLeap Patent AI app, save the report with `write_patent_results` (`template: "find-better-report"`, `content: ""`, `challengedPublication`, `objective` with the critical date and its basis, `coverage` with one row for each element of each independent claim, `claimNumber` and its `examiner` and `found` sides, `baselinePath` (the path `examiner_baseline` returns) or `baseline`, `examinerBestArt`, `tracks`, `limitations`). The writer builds the report and the working record. Elsewhere, write the two Markdown files of the template.

Done when the report holds every independent claim, every gap and every track log.
