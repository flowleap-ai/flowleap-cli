---
name: recipe-invalidity-analysis
description: Litigation recipe for building an invalidity case against a target patent — pin the critical date, build the Fallback Ladder (every dependent claim with the limitation it adds) from the claims text, hunt patent and non-patent prior art for every element and every rung, chart it with X/Y/A tags and Fallback Coverage counts, then write a Round Report and run one Search Round per expert answer. Trigger when the user asks to invalidate a patent, find 102/103 or Art. 54/56 prior art against granted claims, or assess how strong a patent really is.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-patent", "flowleap-ops", "flowleap-citation", "flowleap-academic", "flowleap-npl", "flowleap-legal"]
---

# Recipe: Invalidity Analysis

Build a prior-art invalidity case against a target patent, claim by claim, with every fallback the proprietor can retreat to.

In a chat client with the FlowLeap connector, call the tools named in the `flowleap-shared` connector table instead of these commands. The steps stay the same.

The CLI has no typed writer and no execution ledger. You write the **Fallback Ladder**, the **Fallback Coverage** counts and the round log by hand into the report. Mark each of them "unvalidated: written by the agent, not checked by a tool". The user is an expert. After the report, the expert decides what the next **Search Round** looks for.

## Step 1: Target Intake — Fix the Critical Date

```bash
flowleap --json summary <target-patent>     # biblio, legal status, family, term
flowleap --json ops claims <target-patent>
flowleap --json timeline <target-patent>    # filing/priority events
```

The earliest priority date is the critical date. Prior art must predate it (mind grace periods and jurisdiction differences). Record it before you search. Keep the `ops claims` output: Step 2 builds the ladder from it.

## Step 2: Decompose the Claims and Build the Fallback Ladder

Decompose each asserted independent claim yourself: split on the `comprising` transition and the semicolon/`wherein` boundaries, keep the claim language verbatim per element, and derive keywords and synonyms per element (the method is in `recipe-claim-analysis`, Step 4). Every element must be found in the art. Track them as a checklist.

Then build the **Fallback Ladder** from the `ops claims` output of Step 1. For each independent claim, list every dependent claim under it, directly or through other dependent claims, in claim order, with the limitation it adds. A dependent claim that refers to several parents is a rung under each parent. Copy the reference wording and the added limitation from the command output. Never type the ladder from memory. The method, the EN/DE/FR dependency wording and the table layout are in [references/fallback-coverage.md](references/fallback-coverage.md).

Done when each independent claim has an element checklist and a ladder, and each rung names its parent and its added limitation.

## Step 3: Hunt Prior Art Per Element Combination

Write a query per element combination yourself, tuned broad. Invalidity is a recall hunt: drop classifications, OR in synonyms from Step 2, and probe every query's count before you trust it (method in `flowleap-patent`). The concept table carries the added limitation of every rung as a secondary concept with its own **Discriminating Term**. The first search aims at the claims and every rung. The Search Round of Step 8 is only the retry.

```bash
# Patent art, both databases — element pairs carry the discrimination
flowleap --json tools run search_patents query='<CQL for the element combination>' range=1-1 details=false
flowleap --json patent search --query "<CQL for the element combination>" --limit 30
flowleap --json uspto search --query "<self-written ODP Lucene>" --limit 30   # title + metadata only

# Non-patent art — bound by the critical date. OpenAlex first, then
# Semantic Scholar + arXiv as a second pass; dedupe by DOI or title
flowleap --json npl "<element keywords>" --to-year <priority-year> --limit 20
flowleap --json academic search "<element keywords>" --to-year <priority-year> --limit 20
```

NPL hits hard here: printed publications are fully usable in IPR, and examiners rarely searched them. Resolve papers already of record before the sweep: run `flowleap --json ops biblio <target-patent> --resolve-npl` (for an EP family, on the A3 search-report publication) or `flowleap --json npl-work "<cited string or DOI>"` for one paper. The sweep then logs them as art of record and aims past them. Only a `matched` record is the cited paper.

Search both offices first. Ask about scope in the Round Report (Step 8), never before the first search. A `provider_keys_required` on either office is a missing patent-data key, not an exhausted search: hunt the live office and the non-patent art in full, record the gated office as an open missing-key gap in the chart, and ask for the free key at the end. See `flowleap-keys`.

Log every query with its count, empty results too. The round log of Step 7 is built from this log.

## Step 4: Mine the Prosecution Record

```bash
flowleap --json patent examiner-baseline <target-patent>             # art of record, every family member and office
flowleap --json citation forward <target-patent> --size 50           # who cites it since
```

Read the Examiner Baseline as `recipe-find-better` Step 1 does: any X or Y category is examiner's art, and each gap is a gap, not "nothing cited". Art the examiner never saw is stronger than art already of record.

## Step 5: Tag References X/Y/A

- **X** — one reference disclosing every element of a claim: anticipation (35 U.S.C. 102 / EPC Art. 54)
- **Y** — reference that invalidates only in combination with another: obviousness (35 U.S.C. 103 / EPC Art. 56); record the motivation to combine
- **A** — background/state of the art; supports the narrative, not the ground

For each X/Y candidate, pull the full text and verify it against the element checklist and against the added limitation of every rung:

```bash
flowleap --json ops claims <reference>
flowleap --json ops description <reference>
```

For an office with no text source, use the family-member workaround in `flowleap-shared`.

## Step 6: Ground the Legal Standard

Read the standard from the office's own page with `flowleap-legal`: MPEP 2131
(anticipation, every element in a single reference) and MPEP 2141 to 2145
(obviousness) for the US; EPC Art. 54 and 56 and Guidelines G-VI and G-VII, 5
(problem-solution approach) for the EPO. Quote each section with its URL and
edition.

## Step 7: Write the Report

Write `<target-patent>.invalidity.md` with the template in [references/fallback-coverage.md](references/fallback-coverage.md). Per independent claim:

- The invalidity chart: rows are the claim elements of Step 2; columns are the references with X/Y/A tags and effective dates against the critical date; cells are pin cites (column/line or paragraph) that quote the disclosure; a combination rationale for every Y-pair, with legal citations.
- The **Fallback Coverage** line, read off the chart rows: "fallbacks: k of j covered; not covered: claims 3, 5". A rung is covered when a quoted passage discloses its added limitation, partly covered when part of it, not covered otherwise.
- The ladder as a table. Every rung has a row in the chart, or the reason it has none.
- A "Search rounds" section: one entry per round with the ids it answered and its queries. Round 0 is the first search.
- A Limitations line: "The ladder, the coverage counts and the round log are unvalidated: written by the agent, not checked by a tool."

No tool checks the ladder, the coverage counts, the quotes or the round log in the CLI, the Agents window or the plugin pack. In the app's chat panel a writer and a completion guard enforce them. Here you write them by hand, and the reader checks them.

"Not covered" is a complete result, not a failure. Done when the report holds every section above for every independent claim.

## Step 8: Round Report, Then Wait

1. List the open points of the report as numbered questions. Each has its id in brackets and your recommended next step. The ids and their meaning are in [references/fallback-coverage.md](references/fallback-coverage.md): `rung-3`, `element-1.2`, `scope-<pub>`, `date-<pub>`, `search-N`.
2. End with the line "Reply with the ids to run (for example rung-3, element-1.3), 'all', or your own steer." Then wait. Do not search before the expert answers.
3. If the answer is bare numbers, ask "question numbers or claim numbers?" and run nothing.
4. On the expert's answer, echo "Running: [rung-3] <limitation>; [element-1.3] <element>." before any search. Then run one **Search Round** aimed only at those points: new Discriminating Terms for each limitation, both offices, then NPL, then the web. Update the chart, add the round to "Search rounds", rewrite the Round Report, and wait again.
5. One round per answer. The expert ends the loop. A rung still not covered after a round is a complete result.

Close every deliverable with: "AI-assisted analysis for review by a registered patent attorney — not legal advice."
