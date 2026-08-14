---
name: flowleap-npl
description: Search non-patent literature (scholarly works via OpenAlex) with year, open-access and publication-type filters. Trigger when an agent needs journal articles, conference papers or preprints as prior art or scientific background — complementary to flowleap-academic (Semantic Scholar/arXiv).
---

# FlowLeap NPL Search (OpenAlex)

Auth and global flags: see `flowleap-shared`.

```bash
flowleap --json npl "perovskite solar cell stability" --limit 5
flowleap --json npl "CRISPR delivery" --from-year 2020 --to-year 2024 --open-access
flowleap --json npl "transformer attention" --type preprint
```

Flags: `--limit N`, `--page N`, `--from-year/--to-year`, `--open-access`,
`--type journal-article|book-chapter|proceedings-article|preprint`.

Results include DOI, abstract, `citedByCount`, open-access URLs and author
lists — use DOI + title when citing prior art.

Tools-facade equivalent: `search_npl` (`query=`, `limit=`, `page=`, and a
`filter` object taking `from_year`, `to_year`, `open_access`, `type`).

No patent-data key is needed, so this corpus stays live while EPO or USPTO is
key-gated. Papers are prior art in their own right, but they are a different
corpus from patents — offer them as *different* data, never as a substitute for
a gated office's search (see `flowleap-keys`).
