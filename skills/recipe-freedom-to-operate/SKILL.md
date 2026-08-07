---
name: recipe-freedom-to-operate
description: Freedom-to-operate clearance for a product or technology — per-feature query generation, dual-database blocking-patent search, and legal-status, family, and all-elements claim checks on every live candidate. Trigger when the user asks whether a product or technology can launch without infringing, or requests an FTO/clearance search.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-patent", "flowleap-uspto", "flowleap-ops"]
---

# Recipe: Freedom-to-Operate (FTO) Search

Search for potentially blocking patents for a product or technology. Each
database uses its own query syntax — see `flowleap-uspto` for the USPTO Lucene
grammar.

## Steps

### Step 1: Generate Targeted Searches

Build a query per key product feature, in each database's own syntax:

```bash
# EPO CQL
flowleap patent build-query "<feature 1 description>" --allow-external-processing
# USPTO ODP
flowleap uspto build-query "<feature 1 description>" --allow-external-processing
# Repeat for each feature
```

Done when every key feature has an EPO and a USPTO query.

### Step 2: Search for Potentially Blocking Patents

```bash
flowleap --json patent search --query "<CQL for feature 1>" --limit 20
flowleap --json uspto search --query "<recommended_query for feature 1>" --limit 20
# Repeat for each feature
```

**A missing patent-data key never narrows a clearance scope.** If one office
answers `provider_keys_required`, search the live office fully and carry the
gated office into the deliverable as an explicit missing-key gap — an
unsearched jurisdiction is not a cleared one, and web-scraped patent data is not
an acceptable substitute for it. Ask for the free key at the end of the turn;
when the user adds it, re-run only that office and merge. See `flowleap-keys`.

Done when both databases have been searched for every feature, or the gated
office is recorded as an open missing-key gap.

### Step 3: Screen to Live, In-Market Candidates

Take the one-call snapshot (bibliography + legal status + family + term) and
keep only patents that are **legally alive** and cover a **jurisdiction you
sell in**:

```bash
flowleap --json summary <patent-number>
```

Or, if you need the pieces separately:

```bash
flowleap --json ops legal <patent-number>     # is it active?
flowleap --json ops family <patent-number>    # where is it filed?
```

Done when every survivor is both in force and filed in a sales jurisdiction;
drop the rest.

### Step 4: All-Elements Claim Mapping

For each surviving blocker, pull its claims and map product features against
every element of each independent claim:

```bash
flowleap --json ops claims <patent-number>
```

A claim clears only if at least one of its elements is absent from the product.
Done when every independent claim of every survivor is either cleared (≥1 element
absent) or flagged as a live infringement risk.

## Output

FTO data package per live blocking patent:
- Legal status (active, expired, abandoned) and remaining term
- Geographic coverage (family members) against your sales jurisdictions
- Per-claim all-elements verdict: cleared or at-risk

If the full skill pack is installed, continue with `recipe-infringement-charting`
to chart the at-risk claims element by element.
