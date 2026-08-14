---
name: flowleap-academic
description: Search academic literature (Semantic Scholar + arXiv) through the FlowLeap backend with per-source and publication-year filters. Trigger when an agent needs scholarly papers as prior art or technical background — topic searches, restricting to arXiv or Semantic Scholar, or bounding results by publication year. For OpenAlex-backed NPL search see flowleap-npl.
---

# FlowLeap Academic

Auth and global flags: see `flowleap-shared`.

## Usage

```bash
flowleap academic search <query> [flags]
```

Runs the `search_academic` tool on the Tools facade. Returns academic papers
with title, authors, year, and source. No patent-data key is needed, so this
corpus stays live while EPO or USPTO is key-gated — offer it as *different*
data (literature, not patents), never as a substitute (see `flowleap-keys`).

## Flags

| Flag | Description | Default |
|------|-------------|---------|
| `--limit` | Maximum results | `10` |
| `--source` | Source to search: `scholar` or `arxiv` (repeat the flag for both) | all sources |
| `--from-year` | Only papers published in or after this year | none |
| `--to-year` | Only papers published in or before this year | none |

## Examples

```bash
# Basic search
flowleap academic search "machine learning patent classification"

# With limit
flowleap academic search "CRISPR gene editing applications" --limit 20

# arXiv only, bounded by publication year
flowleap academic search "transformer attention mechanisms" --source arxiv --from-year 2020 --to-year 2024

# Both sources explicitly
flowleap academic search "solid state electrolyte" --source scholar --source arxiv

# JSON output for agents
flowleap academic search "neural network optimization" --json
```

Tools-facade equivalent: `search_academic` (`query=`, `sources=`,
`max_results=`, `filter=`). Its `sources` values are `semantic-scholar` and
`arxiv`; papers still come back tagged `scholar` or `arxiv`.

## Response Format (JSON)

The CLI prints the tool's `data` payload — `{ query, total, papers }`. The
envelope fields (`success`, `executionTimeMs`, `cached`) sit outside it; add
`--verbose` to see the cache verdict and timing on stderr.

```json
{
  "query": "machine learning patent classification",
  "total": 1,
  "papers": [
    {
      "title": "Machine Learning in Patent Analysis",
      "authors": ["Smith, J.", "Doe, A."],
      "year": "2024",
      "source": "arxiv",
      "url": "https://example.com/paper",
      "abstract": "..."
    }
  ]
}
```
