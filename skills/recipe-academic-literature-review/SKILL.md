---
name: recipe-academic-literature-review
description: "Compare published research with filed patents for a technology: a literature search in Semantic Scholar, arXiv and OpenAlex beside a matching patent search, with the result organised around the gap between what is published and what is protected, not as a ranked novelty list. Use it when the user asks for a literature review, a state-of-the-art survey, or a comparison of academic research with patents."
metadata:
  requires:
    skills: ["recipe-prior-art-search", "recipe-custom-dashboard"]
---

# Recipe: Academic Literature Review

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is not configured or that the office rejected the stored key, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing or rejected. On the desktop app the user adds it in Settings > Patent data connectors; on the firm OS the firm administrator sets the keys, so tell the attorney to ask the administrator; in another client, relay the page that the tool's answer names. Never ask for a key value in the chat.

Compare what a field has published with what it has protected, using academic and patent searches together.

Use `search_academic` (Semantic Scholar and arXiv) first for computer science and machine learning papers and preprints. Use `search_npl` (OpenAlex) first for broad journal coverage across disciplines and for the open-access and publication-type filters.

## Step 1: Academic search

Call `search_academic` (`query`: the research topic, `max_results: 20`). Then call it again with `sources: ["arxiv"]` and `filter: {"from_year": 2020}` for recent preprints. Read the `sources` array of each answer: a source with status `rate_limited` or `error` gave no papers, so name it as not searched, not as empty.

## Step 2: Widen to OpenAlex

Call `search_npl` (`query`: the research topic, `filter: {"from_year": 2020}`, `limit: 10`). Add `open_access: true` or a publication `type` to the filter when the user needs them.

## Step 3: Patent search for the same topic

Build each query with the search method of `recipe-prior-art-search` (`references/search-method.md`); `get_search_syntax` gives the field grammar of each provider.

Write the CQL from the discriminating terms of the topic (the specific subject matter, not the name of the field). Probe the count with `search_patents` (`provider: "epo_ops"`, `query: "<CQL>"`, `range: "1-1"`, `details: false`) and read `total`, then search with `range: "1-20"`. Read the CPC codes of the hits from `get_bibliography` (`patent_number`) when the search result does not show them.

## Step 4: Find the gap

Align the academic themes with the CPC and applicant clusters of the patents. Flag the topics that are heavily published but lightly patented (open R&D space) and the topics that are heavily patented but lightly published (crowded IP). Base each classification on the counts and documents that the tools returned, and name them. Done when each major theme is classified on the published-against-protected axis.

## Output

- Academic papers: title, authors, year, source, citation count, each from the tool answer that gave it.
- Related patents: publication number, title, applicant, date, CPC.
- A published-against-protected map: each theme tagged as open R&D space or crowded IP, with the counts it rests on.
- The queries, the sources searched, and each source that failed or was not searched.

In the FlowLeap Patent AI app, save the review with `write_patent_results` without a template (`filePath`, `content`: the review). Elsewhere, write it as a Markdown file.

## Visual deliverable

Present the published-against-protected map of Step 4 as a table in the report. The `recipe-custom-dashboard` templates read PATSTAT and citation answers only, so they cannot plot academic output.
