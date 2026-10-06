---
name: recipe-find-better
description: Find Better recipe for a granted patent — read the code-computed Examiner Baseline (every document cited across the family, by office), run three logged expansion tracks (two-hop backward citations, inventor and author networks, classifications with Discriminating Terms), and score the examiner's best art and the art found on the same claim elements as "disclosed n of m". Trigger when the user asks whether earlier or closer prior art exists than the examiners cited, or what the examiners missed on a granted patent. Not for a pre-filing novelty search (recipe-prior-art-search) or for venue, grounds and invalidity charts (recipe-invalidity-analysis).
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-patent", "flowleap-ops", "flowleap-citation", "flowleap-academic", "flowleap-patstat-graph", "recipe-claim-analysis"]
---

# Recipe: Find Better

Given a granted patent, find prior art earlier or closer than the art the
examining offices cited. Show, per independent claim, the examiner's best art
beside the best art found, as element counts. "No better art found" with the
full Examiner Baseline and the full tracks log is a complete result.

In a chat client with the FlowLeap connector, call the tools named in the `flowleap-shared` connector table instead of these commands. The steps stay the same.

Find Better makes no invalidity statement and gives no model-rated score. The
only comparison is a count over quoted element rows.

## Step 1: Baseline

```bash
flowleap --json patent examiner-baseline <granted-publication>
```

In a chat client call the examiner_baseline tool; it returns the same JSON.

Read `documents[]`, `gaps[]` and `membersWalked[]` (field guide:
[references/baseline-json.md](references/baseline-json.md)). Record the
critical date: the earliest `dates.priority[].date` from
`flowleap --json ops biblio <granted-publication>`. State it as `YYYY-MM-DD`
in the report.

The **examiner's best art** for an independent claim is every document with an
X or Y category on a citation that reaches that claim, whatever its `citedBy`
says (only an examiner assigns a category). Rank X before Y, then by how many
offices cite it, then by how many independent claims it reaches. When no X or
Y citation exists (a US-origin family often has no categories), take the
documents with `citedBy: "examiner"` and no category, then the documents a US
office action rejected claims with under 102 or 103
(`source: "uspto_enriched"`). The report shows that basis beside each
document. Never take a document that only the applicant cited, with no
category and no rejection. Each entry in `gaps[]` goes in the report as a gap,
never as "nothing cited".

Done when every independent claim has its examiner's best art list, or the
words "no X or Y citation reaches this claim", and every gap is recorded.

## Step 2: Claims

Decompose each independent claim of the granted publication into elements
(method: `recipe-claim-analysis`, Step 4). Give each element its
**Discriminating Terms** (see `flowleap-patent`).

Done when each independent claim is an element list with its Discriminating Terms.

## Step 3: Tracks

Run all three tracks. Each one is mandatory. Use the commands in
[references/tracks.md](references/tracks.md).

1. **Backward citations, two hops** from every document of the examiner's
   best art (the X or Y documents, or, when none exists, the examiner-cited
   documents from Step 1).
2. **Inventor and author networks**: the target's inventors, the inventors of
   the X/Y patents, and the authors of the X/Y non-patent documents.
3. **Classification co-occurrence**: the classifications of the target and of
   its X/Y documents, combined with the Discriminating Terms. One query is
   not enough. Do the **term-drop pass**: run the first query again with ONE
   term removed at a time, and once with the classification alone when the
   first query gave 200 hits or fewer. Log each variant with its count, and
   read the titles of the top hits of each variant before the critical date.
   Then run one **title-phrase query** per Discriminating Term,
   `ti="<two-word phrase from the claim>"`, because old US documents often
   have no abstract in OPS.
   Show the statement as a Search Statement block — see flowleap-patent `references/search-statement.md`.

Keep only documents published before the critical date. Log every query with
its count in the working record, empty results too.

Done when each track has its log, Track 3 has its term-drop variants and its
title-phrase queries, and each kept candidate has its claims or full text
pulled.

## Step 4: Compare

For each independent claim, score the top examiner's art and each candidate on
the SAME element rows from Step 2. A row counts as disclosed only with a quoted
passage and its anchor (claim, paragraph, column and line, or figure). Write
"disclosed n of m" for each document.

"Better" means more elements disclosed, or the same count with an earlier date.
If no candidate beats the examiner's art, write "no better art found" for that
claim. That is a complete result, but only when all three tracks ran at least
one query and Track 1 has a hop 2. Otherwise write "Search incomplete for
claim N: <reason>" (for example "Track 2 ran no query" or "Track 1 has no
hop 2"). Name in the result line only the examiner documents you scored. List
the other examiner's best art under "named but not scored". Flag a document
with a P or E category, or published after the critical date, as "published
after the critical date; not prior art for this claim unless the priority
claim fails".

Done when every independent claim has both columns scored on the same rows.

## Step 5: Record

Write the report and the working record with the structure in
[references/report-template.md](references/report-template.md): one row per
independent claim, the Baseline matrix, the gaps, and the tracks log. Mark each
Track 1 query `hop 1` or `hop 2`. If hop 1 found no document outside the
Baseline, log one `hop 2` entry that says so, with count 0.

Done when the report holds every independent claim, every gap and every track log.
