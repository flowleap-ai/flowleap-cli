---
name: flowleap-patent
description: Search EPO patents with CQL and build CQL queries from natural language through the FlowLeap backend. Trigger when an agent needs European or worldwide patent search results, needs a plain-English invention description turned into a CQL query, or wants to tune query strategy (broad, precise, comprehensive). For US-specific ODP searches see flowleap-uspto.
---

# FlowLeap Patent

Auth and global flags: see `flowleap-shared`.

EP/WO search runs on the user's own EPO patent-data key. A
`provider_keys_required` error is a **user-action stop for that office, never an
exhausted route**: do not fall back to web-scraped patent data, deliver whatever
the live office (US via `flowleap uspto search`) returns in full, name the gap as
a missing-key gap, and ask for the free key at the end of the turn. A zero-result
search is not a gate — reformulate and persist as normal. See `flowleap-keys`.

## Commands

### Search Patents

```bash
flowleap patent search --query <query> [flags]
```

Posts to `/v1/patent-search`. Returns patent results with publication number, title, applicant, and date.

| Flag | Description | Default |
|------|-------------|---------|
| `--query`, `-q` | EPO CQL query (required) — e.g. `ti="battery separator"` | — |
| `--limit` | Maximum results (1-100) | `10` |
| `--countries` | Country filter, comma-separated (e.g. `EP,WO`) | none |

Jurisdiction is set with `--countries` — `patent search` has no `--source` flag
(that flag belongs to `academic search`, for `scholar` vs `arxiv`).

For US-specific searches use `flowleap uspto search` (ODP Lucene syntax).

CQL terms must be at least 3 characters — OPS rejects shorter prefixes with a
`400 CLIENT.PrefixTooShort`. Spell the word out (`ti="ultraviolet"`, not
`ti="uv"`); this applies to hand-written queries and to CQL from `build-query`.

#### Examples

```bash
# Basic search
flowleap patent search --query "solar panel efficiency"

# JSON output for agents
flowleap patent search --query "CRISPR gene editing" --json
```

#### Response Format (JSON)

Returns an array of results, each carrying a document identifier, title,
applicant(s), publication date, and abstract, for example:

```json
[
  {
    "docId": "EP1234567.A1",
    "title": "Solar Panel with Improved Efficiency",
    "applicants": ["SolarCorp Inc."],
    "publicationDate": "20240115",
    "abstract": "..."
  }
]
```

Field names are illustrative — inspect a live `--json` response for the exact
keys. Strip the kind suffix (`EP1234567.A1` → `EP1234567`) to use the document
identifier as the `<patent-number>` argument to `ops` and the tools facade.

### Build CQL Query

```bash
flowleap patent build-query <description> [flags]
```

Posts to `/v1/build-patent-query`. Converts natural language to CQL (Common Query Language) for EPO patent searches.
Use `--dry-run` to verify the request shape without calling the backend model.
Live calls send the description to FlowLeap and then to Anthropic or OpenAI,
so they require `--allow-external-processing`.

| Flag | Description | Default |
|------|-------------|---------|
| `--focus` | Strategy: `broad`, `precise`, `comprehensive` | `comprehensive` |
| `--allow-external-processing` | Consent to FlowLeap and external-LLM processing | `false` |

#### Examples

```bash
# Natural language to CQL
flowleap patent build-query "patents about lithium battery recycling filed by Tesla" --allow-external-processing

# With a strategy focus
flowleap patent build-query "renewable energy storage systems" --focus comprehensive --allow-external-processing

# JSON output
flowleap patent build-query --json "autonomous vehicle lidar sensors" --allow-external-processing
```

#### Response Format (JSON)

```json
{
  "query": "ti=lithium AND ti=battery AND ti=recycling AND pa=Tesla",
  "explanation": "Searches for patents with lithium, battery, and recycling in the title filed by Tesla."
}
```

## Workflow: Natural Language to Patent Results

1. Build a CQL query: `flowleap patent build-query "your description" --allow-external-processing`
2. Use the generated CQL in a search: `flowleap patent search --query "<CQL>"`
