---
name: recipe-prior-art-search
description: Comprehensive prior-art / novelty search before filing or on a published patent — per-element synonym rows, dual EPO/USPTO patent search with a classification-only track and a citation track, an NPL sweep (OpenAlex first), a recall check against the Examiner Baseline, and X/Y/A tagging of the closest hits. Trigger when the user asks to find prior art for an invention, run a novelty search, or check what already exists before filing.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-patent", "flowleap-uspto", "flowleap-npl", "flowleap-academic", "flowleap-ops", "flowleap-citation"]
---

# Recipe: Prior Art Search

A multi-step workflow for comprehensive prior-art searching. Each database uses
its own query syntax — see `flowleap-uspto` for the USPTO Lucene grammar.

In a chat client with the FlowLeap connector, call the tools named in the `flowleap-shared` connector table instead of these commands. The steps stay the same.

## Steps

### Step 1: Write the Search Queries

You write both queries yourself — the invention description never leaves the
machine to become a query. The method (from `flowleap-patent`, where the CQL
fields live):

1. **Extract the terms.** List every specific noun phrase in the invention
   description; for each one you leave out of the query, state why. Unsure of
   a phrasing ("glass ceramic" vs "glass-ceramic")? OR both forms — never drop
   the term.
   Build one synonym row **per claim element**, not per invention: the
   claim's own word, the structural equivalents another drafter would use,
   and the functional words for what the element does. Older art and other
   offices describe the same part differently, and a document that uses none
   of your words is invisible to every keyword query. Example (hydraulic
   actuator, EP4758353A1): pump: bidirectional, reversible, tiltable,
   over-centre, four-quadrant; chamber: cap side, rod side, piston side,
   annulus; pressure reservoir: accumulator, tank, pressure compensator. OR
   within one row (`(ta=a OR ta=b)`); never let one element's word stand in
   for the whole invention. `ta=` matches the English title and abstract, so
   English synonyms come first; add German or French terms when the scope
   includes DE or FR documents that may carry only original-language text.
   When the subject is a published document, read its full `cpc[]` from
   `flowleap --json ops biblio <publication>`, including the 2000-series
   **indexing codes** (e.g. `F15B2211/7053`): they describe the features
   independent of wording, and they feed the classification-only track in
   Step 2.
2. **Write the CQL around a discriminating term** — the specific subject
   matter, never just the technology area, never a CPC class alone. Grouping
   repeats the field: `(ta=X OR ta=Y)` is valid; `ta=(X OR Y)` is a hard OPS
   404. Verify any CPC code against the official scheme
   (`flowleap patstat query` on `flowleap.cpc_scheme`) — never guess codes.
3. **Probe the count** — mandatory. `patent search` shows no total, so run the
   same tool directly and read `total` from its payload. `details=false`
   skips the per-document bibliography fan-out, so the probe stays cheap:

```bash
flowleap --json tools run search_patents query='<self-written CQL>' range=1-1 details=false
```

Over ~1,000 hits: add the next discriminating term from your extraction list.
Under 10: broaden — this is a novelty pass, so start broad (drop the
classification, OR in synonyms) and narrow from the count, never the reverse.

The USPTO leg uses ODP Lucene over title + metadata (see `flowleap-uspto`):
same term extraction, same probe discipline, but the discrimination must be a
term that plausibly appears in an invention title.

Show the statement as a Search Statement block — see flowleap-patent `references/search-statement.md`.

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

**Run at least two independent tracks**, so a document one misses another can
catch:

- **Keyword track**: the queries above, built from the per-element rows.
- **Classification-only track**: the main group AND combinations of 2-3
  indexing codes that map to essential elements, with the date limit and no
  keywords, e.g. `cpc=F15B7/006 AND cpc=F15B2211/7053 AND cpc=F15B2211/785
  AND pd<20230810` (45 hits, probed live 2026-10-11). A single code is a
  neighbourhood; a conjunction of specific indexing codes is a set small
  enough to read in full, and it finds documents whose wording no keyword
  query matched. Try several combinations; log each with its count.
- **Citation track** (Step 4): one hop from the closest hits.

**Read the whole set, never page 1.** Results come back newest first, not by
relevance: older art sits at the end. `patent search --limit` stops at 100, so
page a larger set with `ops search` (`--start 101 --end 200`, …). When a set is
too large to read in full (over about 300), narrow it (the next synonym row,
another indexing code, a date window) until it is, and log each step. Never
judge a set, or record a document as absent, from its first page.

OPS returns **one publication per family**: a hit may be the CA, WO or JP
member of the US document you know (US2013333894A1 comes back as
CA2828987A1). Compare documents by family (`ops family`), not by number.

```bash
flowleap --json ops search --cql '<classification-only CQL>' --start 101 --end 200
```

Done when both databases have returned ranked results, or one is reported as an
open missing-key gap, and each set you rely on has been read in full.

### Step 3: Search Non-Patent Literature

```bash
flowleap --json npl "<discriminating terms>" --to-year <critical-year> --limit 25
flowleap --json academic search "<discriminating terms>" --to-year <critical-year> --limit 15
```

1. `npl` (OpenAlex, 250M+ works) first. Reuse the discriminating terms from
   the per-element rows in paper wording, run a variant per essential
   feature, and check each `publicationDate` against the exact critical date.
   Narrow with `--type` (`proceedings-article`, `dissertation`, `standard`, …)
   when a track calls for it. Log each query and its hit count.
2. `academic search` (Semantic Scholar + arXiv) as a second pass for
   preprints and CS/physics work; dedupe against step 1 by DOI or title. A
   source named in `sources[]` as `rate_limited` or `error` is a gap, not
   "no literature".
3. Papers an office already cited (the `npl` entries of a source or family
   publication's `citedReferences`) are resolved, not searched: `flowleap
   --json ops biblio <publication> --resolve-npl` resolves all of them,
   `flowleap --json npl-work "<cited string or DOI>"` one. Only a `matched`
   record is the cited paper.
4. Read past the abstract (the open-access URL or DOI) before relying on a
   paper; cite it by authors, title, venue, date and DOI.

### Step 4: Deep Dive on the Closest Hits

Deep-dive every hit whose abstract maps to at least one independent feature of
the invention — at minimum the top 5 by rank:

```bash
flowleap ops abstract <patent-number>
flowleap ops claims <patent-number>
flowleap ops family <patent-number>
```

**One hop from the closest hits** (the citation track): for each closest
candidate of the keyword and classification tracks, and for each Examiner
Baseline document once Step 5 has it, read its cited references (`ops biblio`
`citedReferences[]`) and its forward citations (`citation forward`). Keep the
documents published before the cutoff that share a CPC main group or indexing
code with the subject. When a disclosure names a source or family
publication, work its cited references first (for an EP family they sit on the
A3 search-report publication); those documents are **of record**, not found by
your search: keep them apart for the recall check.

```bash
flowleap --json ops biblio <closest-hit>                 # citedReferences[]
flowleap --json citation forward <closest-hit> --size 50
```

Record how deeply each document was read: `bibliography/abstract only` or
`full text read` (claims and description). Map an element to a passage only
after a full-text read; an abstract-only document can be named as a
candidate, never as supporting a row.

Done when each qualifying hit has its claims and family pulled and the
citation hop has run.

### Step 5: Recall Check against the Examiner Baseline

When the subject is a published patent or application, or the disclosure
names one, check the search against what the examiners cited, after the
keyword, classification and citation tracks have run:

```bash
flowleap --json patent examiner-baseline <publication>   # any family member
```

1. Take every document an examiner cited (X, Y and A categories, ISR and
   national searches); applicant-cited documents are listed separately.
2. A Baseline document counts as **found** only when a search query returned
   it or a member of its family. A `pn=` lookup, or reading the source's own
   cited references, does not count.
3. For each miss, find out why: compare its CPC codes and abstract wording
   with your queries (a synonym or indexing code the queries lacked, or a
   page you did not read). Run the query that fixes the cause and log it with
   its purpose. Repeat until each miss is found by a search or explained.
4. State the result as **"found n of m examiner-cited documents by search"**,
   list each miss with its reason, and put unexplained misses under
   limitations. A low recall means the queries are too narrow for new art
   too: say so rather than presenting new candidates as complete.

Then look for art beyond the Baseline, starting from the claim elements the
examiner documents do not cover.

### Step 6: Tag Each Reference X / Y / A

Against the invention's features, tag every retained reference:
- **X** — alone anticipates a feature (novelty-destroying)
- **Y** — anticipates only in combination with another reference
- **A** — general background

Done when every retained reference carries an X/Y/A tag, X-tagged first.

### Step 7: Map the X References Element by Element

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

## Rules

- Never judge a result set from its first page: read it in full or narrow it
  until you can.
- Always run the Step 5 recall check when the subject has an Examiner
  Baseline, and report "found n of m".

## Output

A prior-art table with:
- One row per patent **family** (the closest member represents the family; use
  `ops family` to collapse duplicates)
- Each row tagged X / Y / A, X-tagged references surfaced first
- The element-by-element mapping table for the X references (Step 7)
- Papers on the same topic, cited by authors, title, venue, date and DOI
- The tracks run, each query with its count, and the review depth per document
- The recall result "found n of m examiner-cited documents by search", with
  each miss and its reason, when the subject has an Examiner Baseline
- Claims and abstracts from the closest prior art
