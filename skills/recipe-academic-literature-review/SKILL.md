---
name: recipe-academic-literature-review
description: "Compare published research with filed patents for a technology: a literature search in Semantic Scholar, arXiv and OpenAlex beside a matching patent search, with the result organised around the gap between what is published and what is protected, not as a ranked novelty list. Use it when the user asks for a literature review, a state-of-the-art survey, or a comparison of academic research with patents."
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-academic", "flowleap-npl", "flowleap-patent", "recipe-custom-dashboard"]
---

# Recipe: Academic Literature Review

Combine academic and patent searches to map what a field has published against
what it has protected.

In a chat client with the FlowLeap connector or in the FlowLeap Patent AI app, call the tools named in the `flowleap-shared` tool table instead of these commands, and never run a `flowleap` command there. The steps stay the same.

Prefer `academic` (Semantic Scholar + arXiv) for CS/ML papers and preprints;
prefer `npl` (OpenAlex) for broad cross-disciplinary journal coverage and
open-access filtering.

## Steps

### Step 1: Academic Search

```bash
flowleap --json academic search "<research topic>" --limit 20
flowleap --json academic search "<research topic>" --source arxiv --from-year 2020
```

### Step 2: Widen to OpenAlex NPL

```bash
flowleap --json npl "<research topic>" --from-year 2020 --limit 10
```

### Step 3: Patent Search for the Same Topic

Write the CQL yourself from the topic's discriminating terms (the specific
subject matter, not the field name — method in `flowleap-patent`), probe the
count, then search:

```bash
flowleap --json tools run search_patents query='<self-written CQL>' range=1-1 details=false
flowleap --json patent search --query "<self-written CQL>" --limit 20
```

### Step 4: Synthesize the Gap

Align the academic themes against the patent CPC and assignee clusters. Flag
topics heavily published but lightly patented (open R&D space) and topics
heavily patented but lightly published (crowded IP). Done when each major theme
is classified on the published-versus-protected axis.

## Visual deliverable

To hand off Step 4's published-versus-protected map as a shareable artifact,
follow this recipe's analysis through to the end, then render it with
`recipe-custom-dashboard` — the **filing-trends** template (multi-series
comparison over time) is the closest fit, adapted to plot academic output
against patent filings per theme rather than per applicant. Analysis logic
stays here; the dashboard skill only owns presentation.

## Output

- Academic papers (title, authors, year, source, citation counts)
- Related patents (publication number, title, applicant, date, CPC)
- A published-versus-protected map: themes tagged as open R&D space or crowded IP
