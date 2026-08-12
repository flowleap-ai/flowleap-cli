---
name: recipe-claim-analysis
description: Decompose a patent's claims into elements, keywords, and search queries with full supporting context — claims text, abstract, bibliography, description, and family. Trigger when the user asks to analyze what a patent claims, break claims into elements, or interpret claim scope against its specification.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-ops"]
---

# Recipe: Claim Analysis

Extract a patent's claims with full context, then decompose them into elements.

## Steps

### Step 1: Extract Claims

```bash
flowleap --json ops claims <patent-number>
```

Done when you have the full claim set, independent claims identified.

### Step 2: Get Context

```bash
flowleap --json ops abstract <patent-number>
flowleap --json ops biblio <patent-number>
flowleap --json ops description <patent-number>
```

### Step 3: Check Related Patents

```bash
flowleap --json ops family <patent-number>
```

### Step 4: Decompose Every Independent Claim — Yourself

There is no backend claim analyzer: you do the decomposition, one independent
claim at a time.

1. **Split the claim** into preamble, transition (`comprising` / `consisting
   of`), and body. Element boundaries fall at the semicolons and at
   `wherein` / `configured to` clauses.
2. **Keep the claim language verbatim per element** — paraphrase in a second
   column if useful, but the verbatim text is what later maps against prior
   art. Tag each element structural or functional.
3. **Derive keywords and synonyms per element** from the element's own nouns,
   checking the description (Step 2) for the applicant's own alternative
   wording — the specification is the claim's dictionary.
4. **Write a search query per key element combination** with the self-written
   query method in `flowleap-patent` (extract terms, discriminating term,
   count probe). The element pairs, not single elements, usually carry the
   discrimination.

Repeat for every independent claim. For each dependent claim, note how it
narrows its parent (the added element). Done when every independent claim has an
element breakdown and each dependent claim's added limitation is recorded.

## Output

Complete claim data with supporting context:
- Full claims text (independent and dependent)
- Abstract, bibliographic data, and description for interpretation
- Family members for jurisdiction coverage
- Element breakdown (verbatim language, structural/functional tag, keywords)
  and self-written follow-up search queries per independent claim, plus the
  narrowing limitation added by each dependent claim
