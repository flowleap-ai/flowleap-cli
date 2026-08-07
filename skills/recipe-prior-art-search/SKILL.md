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

### Step 1: Generate Search Queries

```bash
# --focus broad widens recall for a first novelty pass
flowleap patent build-query "<describe the invention in natural language>" --focus broad --allow-external-processing
flowleap uspto build-query "<describe the invention in natural language>" --focus broad --allow-external-processing
```

These live builders transmit the invention description to FlowLeap and its
configured Anthropic or OpenAI provider. Obtain informed user consent before
using `--allow-external-processing`. For a local preview, use
`--dry-run --dry-run-redacted`; if external processing is not permitted, write
the CQL/ODP query manually and skip the builder commands.

Done when you have one EPO CQL query and one USPTO ODP query for the invention.

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

**USPTO:** `uspto build-query` emits a **full ODP request body** (a JSON object
under `strategy.recommended_query`), not a query string. Submit that body — do
not paste it into `--query`:

```bash
flowleap --json uspto search --body '<recommended_query JSON from step 1>'
# or from a file: flowleap --json uspto search --body-file query.json
```

**USPTO recall caveat — ODP is title-only.** ODP search indexes the invention
title and a few metadata fields; there is **no abstract or claims full-text**.
A distinguishing feature that only appears in the abstract (e.g. "UV-C
sterilization") therefore cannot be matched, and a generated query that ANDs
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

## Output

A prior-art table with:
- One row per patent **family** (the closest member represents the family; use
  `ops family` to collapse duplicates)
- Each row tagged X / Y / A, X-tagged references surfaced first
- Academic papers on the same topic
- Claims and abstracts from the closest prior art
