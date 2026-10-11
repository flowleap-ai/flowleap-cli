---
name: flowleap-npl
description: Search non-patent literature (scholarly works via OpenAlex) with year, open-access and publication-type filters, and resolve a cited paper (DOI, OpenAlex id or an office's citation string) to its record. Trigger when an agent needs journal articles, conference papers, theses, standards or preprints as prior art or scientific background, or needs to identify a paper an examiner cited — the first NPL source; flowleap-academic (Semantic Scholar/arXiv) is the second pass.
---

# FlowLeap NPL Search (OpenAlex)

Auth and global flags: see `flowleap-shared`.

In a chat client with the FlowLeap connector, call the tools named in the `flowleap-shared` connector table instead of these commands.

```bash
flowleap --json npl "perovskite solar cell stability" --limit 5
flowleap --json npl "CRISPR delivery" --from-year 2020 --to-year 2024 --open-access
flowleap --json npl "transformer attention" --type preprint
flowleap --json npl "hydraulic accumulator bypass" --to-year 2022 --type dissertation
```

Flags: `--limit N`, `--page N`, `--from-year/--to-year`, `--open-access`,
`--type journal-article|proceedings-article|book-chapter|preprint|review|book|dissertation|report|standard|dataset`.

Order for an NPL sweep: `npl` first (OpenAlex, 250M+ works), then
`academic search` (Semantic Scholar + arXiv) as a second pass for preprints and
CS/physics work, deduplicated by DOI or title; a web search for what neither
reaches (product manuals, web disclosures, standards bodies' sites). A year
filter is coarse: check each `publicationDate` against the exact critical date.

Results include DOI, abstract, `citedByCount`, open-access URLs and author
lists — use DOI + title when citing prior art.

Tools-facade equivalent: `search_npl` (`query=`, `limit=`, `page=`, and a
`filter` object taking `from_year`, `to_year`, `open_access`, `type`).

## Resolve a cited paper

Papers an office already cited are resolved, not searched:

```bash
flowleap --json npl-work 10.1038/nature14539
flowleap --json npl-work "SMITH J: 'Hydraulic actuator control', IEEE TRANS., 2015"
flowleap --json ops biblio EP3000000A3 --resolve-npl   # every cited NPL entry at once
```

`npl-work` takes a DOI, an OpenAlex work id (`W…`) or a citation string
exactly as printed (a `citedReferences[].npl` entry of `ops biblio`, an NPL row
of `patent examiner-baseline`, an `isNPL` cited document of `citation
search`). Lookup order: OpenAlex id, then any DOI in the string, then the
quoted title. `status` decides what you may say:

- `matched` — `work` is the cited record (title, authors, venue, date, DOI,
  abstract, open-access link).
- `candidates` — similar works, none certain. Say so; never pick one as the
  cited paper.
- `not_found` — standards, product manuals and web pages are often absent from
  OpenAlex; cite the office's string as printed.

`ops biblio --resolve-npl` attaches the same result as `nplWork` to each
`citedReferences[].npl` entry (one OpenAlex read per reference). For an EP
family the citations sit on the A3 search-report publication.

Tools-facade equivalent: `get_npl_work` (`reference=`), and
`get_bibliography` with `resolve_npl=true`.

No patent-data key is needed, so this corpus stays live while EPO or USPTO is
key-gated. Papers are prior art in their own right, but they are a different
corpus from patents — offer them as *different* data, never as a substitute for
a gated office's search (see `flowleap-keys`).
