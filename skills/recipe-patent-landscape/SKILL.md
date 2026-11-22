---
name: recipe-patent-landscape
description: Patent-landscape analysis for a technology area — scoped dual-database search, key-player identification, full-corpus filing analytics, and CPC-versus-year white-space detection. Trigger when the user asks to map a technology space, identify who patents in an area, or report filing trends and white space.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-patent", "flowleap-uspto", "flowleap-ops", "recipe-custom-dashboard"]
---

# Recipe: Patent Landscape Analysis

Map the patent landscape for a technology area, identifying key players, trends,
and gaps. Each database uses its own query syntax — see `flowleap-uspto` for the
USPTO Lucene grammar.

In a chat client with the FlowLeap connector or in the FlowLeap Patent AI app, call the tools named in the `flowleap-shared` tool table instead of these commands, and never run a `flowleap` command there. The steps stay the same.

## Steps

### Step 1: Define Search Scope

Write the scoping queries yourself (method in `flowleap-patent`): extract the
area's specific noun phrases, keep at least one discriminating term (a CPC
class alone maps a continent, not a landscape), and probe the count — a
landscape query legitimately lands broader than a novelty query, but a count
in the tens of thousands still means the scope is the technology area, not
the landscape:

```bash
# Count probe (read `total` from the payload), then adjust scope
flowleap --json tools run search_patents query='<self-written CQL>' range=1-1 details=false
```

Verify any CPC code against the official scheme before scoping on it —
`flowleap patstat query` on `flowleap.cpc_scheme` (see `flowleap-patent`);
group titles carry the specific technology, the 4-char class only the
headline.

Done when you have a probed EPO CQL query and a USPTO ODP query for the area.

### Step 2: Broad Patent Search

```bash
flowleap --json patent search --query '<plain words>' --cpc <class> --limit 50
flowleap --json patent search --provider epo --query "<CQL from step 1>" --limit 50
flowleap --json uspto search --query "<ODP Lucene from step 1>" --limit 50
```

Start the candidate search in the publication index (the first command; see
`flowleap-patent`): it covers every office in one search and uses no office
quota. A landscape starts with this index search and with ONE
`flowleap patstat graph technology` call (see `flowleap-patstat-graph`), before
guarded PATSTAT SQL:

```bash
# One call: top applicants, filing trend, grant rate, new entrants, top CPC, offices
flowleap --json patstat graph technology \
  --cpc <prefix> --cpc <prefix> \
  --keywords '"<phrase>"' --keywords '<synonym> or <synonym>'
```

Use the CPC prefixes of Step 1 for `--cpc` (a document must carry ONE of them)
and the discriminating terms for `--keywords` (a document must match EVERY
entry; put the synonyms of one concept in one entry with `or`). One prefix
with no keywords gives the CPC card of that prefix. The keywords are matched
in English titles and abstracts only, so the counts are a lower bound: say so
in the deliverable, with the scope. When the answer is "too broad", make the
scope narrower as the hint says. Write `patstat query` SQL only for a
question this call does not answer. State the `asOf` date and the `attribution.text` of the
index in the deliverable, and add the EPO search (the second command) for
publications newer than `asOf` and for CQL field precision. When a live read answers
"not now" (`liveUnavailableHint`: the daily live data budget is spent), continue
with the index and `flowleap patstat`, and name the live reads that did not run.

If one office answers `provider_keys_required` (an older server; no user needs
a key on a current one), its patent-data key is missing:
map the live office in full, label the landscape as covering that office only
because of a missing key — never as the shape of the field — and ask for the
free key at the end. `flowleap patstat` aggregates stay available keyless, but
they are twice-yearly snapshot counts, not a substitute for live search. See
`flowleap-keys`.

Done when both databases have returned their result sets, or the gated office is
named as an open missing-key gap.

### Step 3: Corpus Analytics

```bash
# Filing trends by year, country and CPC breakdowns, top assignees
flowleap --json analytics --keyword "<technology>" --date-from 2015-01-01
flowleap --json analytics --cpc <cpc-prefix> --country US --date-from 2020-01-01
```

### Step 4: Identify Key Players

Scope the Step 1 query per applicant with `pa=` — wildcards catch name
variants (`pa=GOOGLE*` catches "Google LLC" and "Google Inc"), and
subsidiaries file separately (Google also files as Alphabet, DeepMind,
Waymo):

```bash
flowleap --json patent search --provider epo --query 'pa=<ASSIGNEE>* AND <discriminating terms from step 1>' --limit 30
```

### Step 5: Check Recent Activity

`patent search` returns relevance-ranked hits; `ops search --cql` adds CQL
date-range filtering (`pd>=2024`) for a time-sliced view the ranked search does
not expose:

```bash
flowleap ops search --cql "ti=<technology> AND pd>=2024" --start 1 --end 50
```

### Step 6: Flag White Space

Cross the CPC breakdown (`top_cpc` of the Step 2 corpus call, or the analytics
breakdown) against the filing-year trend to flag subclasses
that are sparse or declining while neighbours grow. Done when at least one
sparse/declining CPC subclass (or a confirmed absence) is identified.

## Output

A dataset segmented by database, applicant, and filing date, plus corpus-level
trend charts (filings per year, top assignees, CPC and country distributions).
When tallying players or counts from the search results, collapse to one entry
per patent **family** so multi-jurisdiction filings are not double-counted; the
corpus `analytics` figures are aggregate backend counts, reported as returned.

## Visual deliverable

To turn Step 3's filing-trend numbers and Step 6's white-space finding into a
shareable HTML dashboard, follow this recipe's analysis through to the end,
then render it with `recipe-custom-dashboard` — its **landscape white-space**
template (CPC × year heatmap) is built for this recipe's final step, and its
**filing-trends** template covers Step 3's year-over-year counts. Analysis
logic stays here; the dashboard skill only owns presentation.
