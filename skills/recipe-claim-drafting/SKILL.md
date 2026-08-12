---
name: recipe-claim-drafting
description: Prosecution recipe for drafting patent claims grounded in prior art — search the closest art, study its claim language, decompose the invention into elements, iterate drafts through claim analysis, and check formal drafting rules against MPEP/EPO guidelines. Trigger when the user asks to draft or improve patent claims, write independent and dependent claims, or stress-test draft claims against known art and formal requirements.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-patent", "flowleap-ops", "flowleap-legal"]
---

# Recipe: Claim Drafting

Draft claims that are novel over the closest art and formally sound.

## Step 1: Find the Closest Art First

Claims drafted blind get rejected. Anchor on the art before writing:

Write the queries yourself, tuned precise — the goal is the closest few
documents to read in full, so add a second discriminating `ta` term and probe
the count down toward tens, not thousands (method in `flowleap-patent`):

```bash
flowleap --json api request post /v1/patent-search --body '{"query":"<self-written CQL>","range":"1-1"}'
flowleap --json patent search --query "<self-written CQL>" --limit 20
flowleap --json uspto search --query "<self-written ODP Lucene>" --limit 20   # title + metadata only
```

Pull the claims of the 3-5 closest hits and study their language:

```bash
flowleap --json ops claims <closest-patent>
flowleap --json ops abstract <closest-patent>
```

## Step 2: Element Inventory of the Invention

Write a one-paragraph invention summary, then decompose it yourself: list
every element the invention needs to work (the mechanism, not the benefit),
keep the language concrete, and note keywords and synonyms per element (the
decomposition method is in `recipe-claim-analysis`, Step 4).

Sort the elements: which are old (in the closest art), which are the point of
novelty. The independent claim needs the old elements for operability plus at
least one novel element for allowance.

## Step 3: Draft the Claim Set

- **Independent claim**: preamble (technical field) + transition
  (`comprising`) + minimum element set that is both operable and novel
- **Dependent claims**: one fallback feature each, ordered by commercial
  value — these are the amendment reservoir for prosecution
- Draft method + apparatus (and CRM where relevant) parallel sets

## Step 4: Check Formal Drafting Rules

```bash
# EPO clarity/support requirements
flowleap --json legal search "claim clarity conciseness support Article 84" --jurisdiction epo --comprehensive

# US definiteness and functional-language rules
flowleap --json legal search "claim definiteness functional language means plus function" --jurisdiction uspto --comprehensive
```

Check each claim against the found rules: antecedent basis, single-sentence
form, no result-only language without structure, support in the description.

## Step 5: Iterate Against the Art

For each draft independent claim, write a search from its novel element the
same way as Step 1 — the novel element's phrase is the discriminating term:

```bash
flowleap --json patent search --query "<CQL written from the novel element>" --limit 10
```

If the searches surface art reading on the draft, narrow the novel
element and repeat. Stop when the closest surfaced art clearly lacks an
element.

## Output

- Closest-art table (reference, claim 1 language, what it lacks)
- Element inventory split into old vs. novel
- Full claim set (independent + dependents) with a support map into the
  description
- Formal-rules checklist with legal citations per rule
