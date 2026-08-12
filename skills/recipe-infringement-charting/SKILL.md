---
name: recipe-infringement-charting
description: Litigation recipe for an element-by-element infringement claim chart — verify the patent is in force where it matters, decompose the asserted claims, extract accused-product evidence (OCR datasheets and manuals), and map every element with literal and doctrine-of-equivalents columns. Trigger when the user asks whether a product infringes a patent, wants a claim chart, or needs element-by-element infringement mapping.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-ops", "flowleap-legal", "flowleap-tools"]
---

# Recipe: Infringement Charting

Map asserted claims onto an accused product, element by element.

## Step 1: Asserted Patent Intake

```bash
flowleap --json summary <asserted-patent>       # biblio, legal status, family, term
flowleap --json ops claims <asserted-patent>
flowleap --json ops description <asserted-patent>   # claim-construction support
```

## Step 2: Enforceability Check

A chart against an expired or lapsed patent is wasted work — check first:

```bash
flowleap --json ops legal <asserted-patent>                              # status events
flowleap --json tools run get_patent_term patent_number=<asserted-patent>  # expiry estimate
flowleap --json ops family <asserted-patent>                             # where it is in force
```

Only chart in jurisdictions with a live family member.

## Step 3: Decompose the Asserted Claims

Decompose each asserted independent claim yourself: split on the
`comprising` transition and the semicolon/`wherein` boundaries, and keep the
claim language **verbatim** per element — the chart maps accused-product
evidence against the claim words, not a paraphrase (the method is in
`recipe-claim-analysis`, Step 4).

The element list becomes the chart's rows. Every element must be met —
one missing element defeats literal infringement of that claim.

## Step 4: Accused-Product Evidence Intake

```bash
flowleap ocr ./accused-product-datasheet.pdf > datasheet.md
flowleap ocr https://example.com/product-manual.pdf > manual.md
```

Collect: datasheets, manuals, teardown reports, marketing claims. Note the
provenance of every document — the chart cites evidence, not impressions.

## Step 5: Build the Chart

| Claim element | Claim language | Accused product feature | Evidence (doc + location) | Literal? | DoE? |
|---------------|----------------|-------------------------|---------------------------|----------|------|

- **Literal**: the element reads directly on the product feature
- **DoE** (doctrine of equivalents): insubstantial difference —
  function/way/result; flag prosecution-history estoppel risks from the
  file history

```bash
flowleap --json legal search "doctrine of equivalents function way result" --jurisdiction uspto
flowleap --json legal search "prosecution history estoppel" --jurisdiction uspto
```

## Step 6: Construction Disputes

For each element where the mapping is contested, pull the specification
passages that define the term (Step 1's description) and record both
constructions with the chart cell marked "disputed".

## Output

- Enforceability memo (status, term, jurisdictions)
- Element-by-element claim chart with literal/DoE columns and evidence cites
- List of disputed constructions with spec support for each side
- Caveat block: this is an analysis aid, not a legal opinion
