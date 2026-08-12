---
name: recipe-invention-disclosure
description: Prosecution recipe for turning an inventor conversation into a complete invention disclosure form (IDF) — structured technical capture, bar-date audit, novelty pre-check across patents and literature, landscape context, and a filing recommendation. Trigger when the user asks to document an invention, create an invention disclosure, or run a pre-filing novelty sanity check.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-patent", "flowleap-academic", "flowleap-npl"]
---

# Recipe: Invention Disclosure (IDF)

Capture an invention completely enough that an attorney can draft from it.

## Step 1: Structured Capture

Interview for these sections (push past marketing language to mechanisms):

1. **Problem** — what fails today, and why existing approaches fall short
2. **Prior approaches** — what the inventors know exists, including their own
   earlier work
3. **The inventive concept** — the mechanism of the solution, not the benefit
4. **Key elements** — minimum set needed for the invention to work
5. **Variations** — alternative embodiments, ranges, materials, algorithms
6. **Evidence** — prototypes, test data, figures
7. **People and dates** — inventors and contribution; conception date;
   **any public disclosure, sale, offer, or publication with dates** (bar
   dates differ by jurisdiction — flag anything older than 12 months)

## Step 2: Novelty Pre-Check

Write the queries yourself from the Step 1 capture — the "key elements" list
is your term extraction. Keep the phrases that discriminate (the mechanism,
not the benefit), probe the count, then search both offices (method in
`flowleap-patent`; the disclosure never has to leave the machine to become a
query):

```bash
flowleap --json api request post /v1/patent-search --body '{"query":"<self-written CQL>","range":"1-1"}'
flowleap --json patent search --query "<self-written CQL>" --limit 20
flowleap --json uspto search --query "<self-written ODP Lucene>" --limit 20   # title + metadata only

# Literature — inventors' own field publishes here first
flowleap --json academic search "<concept keywords>" --limit 15
flowleap --json npl "<concept keywords>" --limit 10
```

For anything close, pull claims and record how the invention differs:

```bash
flowleap --json ops claims <close-patent>
```

This is a sanity check, not a legal search — say so in the IDF.

## Step 3: Landscape Context

```bash
flowleap --json analytics --keyword "<technology>" --date-from 2018-01-01
```

Filing trends and top assignees tell the reviewer whether this space is
crowded, growing, or abandoned — context for the filing decision.

## Step 4: Assemble the IDF

Sections: title; inventors + contributions; problem; prior approaches
(including Step 2 findings with the differences recorded); detailed
description of the inventive concept; key elements; variations; evidence;
disclosure/bar-date audit; landscape summary.

## Step 5: Recommendation

Close with one of: **file now** (novelty check clean, bar clock running),
**develop further** (concept not yet enabled), or **defensive publication**
(crowded space, low claim scope) — with the reasoning.

## Output

A complete IDF document plus the raw novelty pre-check data (queries used,
hits reviewed, differences noted) as an appendix.
