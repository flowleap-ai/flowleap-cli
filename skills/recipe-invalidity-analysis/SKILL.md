---
name: recipe-invalidity-analysis
description: Litigation recipe for building an invalidity case against a target patent — pin the priority date, decompose claims into elements, hunt patent and non-patent prior art per element, and assemble an invalidity chart with X/Y/A reference tagging (single-reference novelty vs. combination obviousness vs. background). Trigger when the user asks to invalidate a patent, find 102/103 or Art. 54/56 prior art against granted claims, or assess how strong a patent really is.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-patent", "flowleap-ops", "flowleap-citation", "flowleap-academic", "flowleap-npl", "flowleap-legal"]
---

# Recipe: Invalidity Analysis

Build a prior-art invalidity case against a target patent, claim by claim.

## Step 1: Target Intake — Fix the Critical Date

```bash
flowleap --json summary <target-patent>     # biblio, legal status, family, term
flowleap --json ops claims <target-patent>
flowleap --json timeline <target-patent>    # filing/priority events
```

The earliest priority date is the critical date: prior art must predate it
(mind grace periods and jurisdiction differences). Record it before searching.

## Step 2: Decompose the Asserted Claims

Decompose each asserted independent claim yourself: split on the
`comprising` transition and the semicolon/`wherein` boundaries, keep the claim
language verbatim per element, and derive keywords and synonyms per element
(the method is in `recipe-claim-analysis`, Step 4).

Every element in the list must be found in the art — track them as a checklist.

## Step 3: Hunt Prior Art Per Element Combination

Write a query per element combination yourself, tuned broad — invalidity is
a recall hunt, so drop classifications, OR in synonyms from Step 2, and probe
every query's count before trusting it (method in `flowleap-patent`):

```bash
# Patent art, both databases — element pairs carry the discrimination
flowleap --json tools run search_patents query='<CQL for the element combination>' range=1-1 details=false
flowleap --json patent search --query "<CQL for the element combination>" --limit 30
flowleap --json uspto search --query "<self-written ODP Lucene>" --limit 30   # title + metadata only

# Non-patent art — bound by the critical date
flowleap --json academic search "<element keywords>" --to-year <priority-year> --limit 20
flowleap --json npl "<element keywords>" --to-year <priority-year> --limit 20
```

A `provider_keys_required` on either office is a missing patent-data key, not an
exhausted search: hunt the live office and the non-patent art in full, record the
gated office as an open missing-key gap in the invalidity chart (an unsearched
office weakens the case, so it must never pass as a completed sweep), and ask for
the free key at the end. See `flowleap-keys`.

## Step 4: Mine the Prosecution Record

```bash
flowleap --json citation search <application-number> --category x    # what the examiner already found
flowleap --json citation forward <target-patent> --size 50           # who cites it since
```

Art the examiner never saw is stronger than art already of record.

## Step 5: Tag References X/Y/A

- **X** — one reference disclosing every element of a claim: anticipation
  (35 U.S.C. 102 / EPC Art. 54)
- **Y** — reference that invalidates only in combination with another:
  obviousness (35 U.S.C. 103 / EPC Art. 56); record the motivation to combine
- **A** — background/state of the art; supports the narrative, not the ground

For each X/Y candidate, pull the full text and verify against the element
checklist:

```bash
flowleap --json ops claims <reference>
flowleap --json ops description <reference>
```

## Step 6: Ground the Legal Standard

```bash
flowleap --json legal search "anticipation single reference disclosure" --jurisdiction uspto
flowleap --json legal search "inventive step problem solution approach" --jurisdiction epo --comprehensive
```

## Output

An invalidity chart per asserted claim:
- Rows: claim elements (from Step 2)
- Columns: references with X/Y/A tags and effective dates vs. the critical date
- Cells: pin cites (column/line or paragraph) quoting the disclosure
- Combination rationale for every Y-pair, with legal citations
