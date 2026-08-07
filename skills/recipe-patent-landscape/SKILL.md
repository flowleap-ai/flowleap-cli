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

## Steps

### Step 1: Define Search Scope

```bash
flowleap patent build-query "<technology description>" --allow-external-processing
flowleap uspto build-query "<technology description>" --allow-external-processing
```

Done when you have an EPO CQL query and a USPTO ODP query for the area.

### Step 2: Broad Patent Search

```bash
flowleap --json patent search --query "<CQL from step 1>" --limit 50
flowleap --json uspto search --query "<recommended_query from step 1>" --limit 50
```

If one office answers `provider_keys_required`, its patent-data key is missing:
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

Build applicant-scoped queries rather than hand-writing CQL — see `flowleap-patent`
for the CQL fields (`pa=` applicant, `ti=` title):

```bash
flowleap patent build-query "<top assignee> patents in <technology>" --allow-external-processing
flowleap --json patent search --query "<CQL from build-query>" --limit 30
```

### Step 5: Check Recent Activity

`patent search` returns relevance-ranked hits; `ops search --cql` adds CQL
date-range filtering (`pd>=2024`) for a time-sliced view the ranked search does
not expose:

```bash
flowleap ops search --cql "ti=<technology> AND pd>=2024" --start 1 --end 50
```

### Step 6: Flag White Space

Cross the analytics CPC breakdown against the filing-year trend to flag subclasses
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
