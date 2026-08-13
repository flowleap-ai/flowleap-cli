---
name: recipe-prior-art-search
description: Comprehensive prior-art / novelty search before filing — natural-language query generation, dual EPO/USPTO patent search, an academic literature sweep, and X/Y/A tagging of the closest hits. Trigger when the user asks to find prior art for an invention, run a novelty search, or check what already exists before filing.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-patent", "flowleap-uspto", "flowleap-academic", "flowleap-ops"]
---

# Recipe: Prior Art Search

A multi-step workflow for comprehensive prior-art searching. Each database uses
its own query syntax — see `flowleap-uspto` for the USPTO Lucene grammar.

## Steps

### Step 1: Write the Search Queries

You write both queries yourself — the invention description never leaves the
machine to become a query. The method (from `flowleap-patent`, where the CQL
fields live):

1. **Extract the terms.** List every specific noun phrase in the invention
   description; for each one you leave out of the query, state why. Unsure of
   a phrasing ("glass ceramic" vs "glass-ceramic")? OR both forms — never drop
   the term.
2. **Write the CQL around a discriminating term** — the specific subject
   matter, never just the technology area, never a CPC class alone. Grouping
   repeats the field: `(ta=X OR ta=Y)` is valid; `ta=(X OR Y)` is a hard OPS
   404. Verify any CPC code against the official scheme
   (`flowleap patstat query` on `flowleap.cpc_scheme`) — never guess codes.
3. **Probe the count** — mandatory. `patent search` shows no total, so read
   `total` from the raw passthrough:

```bash
flowleap --json api request post /v1/patent-search --body '{"query":"<self-written CQL>","range":"1-1"}'
```

Over ~1,000 hits: add the next discriminating term from your extraction list.
Under 10: broaden — this is a novelty pass, so start broad (drop the
classification, OR in synonyms) and narrow from the count, never the reverse.

The USPTO leg uses ODP Lucene over title + metadata (see `flowleap-uspto`):
same term extraction, same probe discipline, but the discrimination must be a
term that plausibly appears in an invention title.

Done when you have one probed EPO CQL query and one USPTO ODP query.

### Step 2: Search Patents (EPO + USPTO)

Run **both** databases — they are not interchangeable:

- `patent search` hits EPO OPS: worldwide DOCDB coverage, including EPO's own
  bibliographic copy of US documents. `--countries US` narrows this EPO
  collection to US members; it does **not** reach USPTO's Open Data Portal.
- `uspto search` hits USPTO ODP directly, returning US application and
  prosecution metadata (the `patentFileWrapperDataBag`) that the EPO copy lacks.

For US prior art the USPTO leg must go through USPTO ODP; do **not** substitute
`patent search --countries US` (that returns EPO's copy, not ODP records).

**EPO:** the CQL from step 1 is a query string:

```bash
flowleap --json patent search --query "<CQL from step 1>" --limit 20
```

**USPTO:** the Lucene query from step 1 goes in `--query` (or wrap it in a
full ODP request body via `--body` when you need `fields`/`enrich`):

```bash
flowleap --json uspto search --query '<ODP Lucene from step 1>' --limit 20
```

**USPTO recall caveat — ODP is title-only.** ODP search indexes the invention
title and a few metadata fields; there is **no abstract or claims full-text**.
A distinguishing feature that only appears in the abstract (e.g. "UV-C
sterilization") therefore cannot be matched, and a query that ANDs
such a term onto the search returns 0. When the USPTO leg comes back empty
(the CLI prints a note and auto-retries once without the CPC filter), run a
**title recall pass on the core device noun** and triage abstracts afterwards:

```bash
# Recall on the device category, with singular/plural variants; drop the
# abstract-only qualifier. Then read abstracts to keep the on-point hits.
flowleap --json uspto search --query 'applicationMetaData.inventionTitle:earbuds AND applicationMetaData.inventionTitle:"charging case"' --limit 25
flowleap ops abstract <application-or-publication-number>
```

**If one office answers `provider_keys_required`**, its patent-data key is
missing. Do not narrow the search to the office whose key happens to be set, and
do not fill the gap with web-scraped results: run the live office in full,
report the other as a missing-key gap (not "no prior art found" and not "limited
coverage"), and ask for the free key at the end. See `flowleap-keys`.

Done when both databases have returned ranked results, or one is reported as an
open missing-key gap.

### Step 3: Search Academic Literature

```bash
flowleap --json academic search "<invention keywords>" --limit 15
flowleap --json npl "<invention keywords>" --limit 10
```

### Step 4: Deep Dive on the Closest Hits

Deep-dive every hit whose abstract maps to at least one independent feature of
the invention — at minimum the top 5 by rank:

```bash
flowleap ops abstract <patent-number>
flowleap ops claims <patent-number>
flowleap ops family <patent-number>
```

Done when each qualifying hit has its claims and family pulled.

### Step 5: Tag Each Reference X / Y / A

Against the invention's features, tag every retained reference:
- **X** — alone anticipates a feature (novelty-destroying)
- **Y** — anticipates only in combination with another reference
- **A** — general background

Done when every retained reference carries an X/Y/A tag, X-tagged first.

### Step 6: Map the X References Element by Element

For each X-tagged reference, show the anticipation per element rather than
asserting it per document. One table, one row per invention feature (or claim
element, when the user supplied a claim), one column per X reference:

```
| Claim element | <Reference 1> | <Reference 2> |
```

Each cell quotes the disclosing passage from the retrieved claims — original
language plus a translation where applicable. A feature no X reference
discloses is the novelty candidate; say so under the table.

Done when every element row is either quoted from at least one X reference or
named as a potential point of novelty.

## Output

A prior-art table with:
- One row per patent **family** (the closest member represents the family; use
  `ops family` to collapse duplicates)
- Each row tagged X / Y / A, X-tagged references surfaced first
- The element-by-element mapping table for the X references (Step 6)
- Academic papers on the same topic
- Claims and abstracts from the closest prior art
