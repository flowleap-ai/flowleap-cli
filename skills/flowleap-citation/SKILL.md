---
name: flowleap-citation
description: USPTO enriched citation data from office actions — citations by application, forward citations of a document, citation statistics and X-category novelty-destroying references. Trigger when an agent assesses novelty risk, examiner-cited prior art, or how often a patent is cited against later applications.
---

# FlowLeap Citation Search (USPTO enriched citations)

Auth and global flags: see `flowleap-shared`.

## Which citation universe?

FlowLeap exposes two, and neither is a superset of the other:

- **This skill** — USPTO **office-action enriched** citations: US only, drawn
  from examiner reasoning, with X/Y/A relevance categories. Use it for novelty
  risk, what an examiner actually cited, and how often a document is cited
  against later US applications.
- **`flowleap-patstat-graph`** — the **worldwide DOCDB citation network** from
  the PATSTAT snapshot, with examiner-vs-applicant origin, confidence tags,
  and row-level provenance. Use it for citation structure across offices, the
  path between two patents, or a patent's whole citation picture in one call.

Pick by whether the question is about US examiner reasoning (here) or
worldwide citation structure (there). PATSTAT answers are snapshot data with a
Data Edition; these are live USPTO records.

## By application

```bash
flowleap --json citation search 16123456 --size 20
flowleap --json citation search 16123456 --category x --examiner-cited-only
flowleap --json citation search 16123456 --from 2020-01-01 --to 2023-12-31
```

`--from`/`--to` bound the **office-action date range** (`YYYY-MM-DD`) — when the
examiner cited the reference, not when either document published.

## Forward citations (who cites this document)

```bash
flowleap --json citation forward US10123456 --size 20
```

## Analysis shortcuts

```bash
flowleap --json citation stats 16123456      # counts by category/source
flowleap --json citation novelty 16123456    # X-rated novelty-destroying citations
```

Categories: `x` (novelty-destroying), `y` (inventive-step with combination),
`a` (background), `all`.

Tools-facade equivalents: `search_office_action_citations` (by application),
`search_enriched_citations` (forward, by cited document) and
`get_citation_stats` (aggregate counts only). `citation novelty` has no tool of
its own — it is a recipe over `search_office_action_citations` with
`category: "X"` and `examiner_cited_only: true`.
