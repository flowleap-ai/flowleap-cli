# The three Find Better tracks

All three tracks are mandatory. Log every query and its count in the working
record, empty results too, so a reviewer sees what was searched and what was
not. Keep only documents published before the critical date (CQL:
`pd<YYYYMMDD`; academic and NPL: `--to-year`, then check the exact date).

Before you search, mark every document already in the Baseline as "of record".
A candidate is new only when it is not of record.

## Track 1: backward citations, two hops

Start from every document with an X or Y category in the Baseline.

```bash
flowleap --json ops biblio <xy-document>        # hop 1: its citedReferences[]
flowleap --json ops biblio <hop-1-document>     # hop 2: their citedReferences[]
```

- Read `citedReferences[].docId` and `.kind` for patents and `.npl` for
  non-patent citations.
- Hop 2 runs on every hop-1 document that is not of record and is published
  before the critical date.
- Optional, for a wider net: `flowleap --json patstat graph neighborhood
  <xy-document> --depth 2 --edge-types cites`. A `TRUNCATED` notice means the
  list is capped; narrow it, never read it as complete.
- Log: per X/Y document, hop-1 count, hop-2 count, kept count.

## Track 2: inventor and author networks

Inventors come from `inventors[]` in `ops biblio` of the target and of each X/Y
patent. Authors come from the X/Y non-patent documents.

```bash
flowleap --json tools run search_patents query='in="<SURNAME GIVEN>" AND pd<<critical-date>' range=1-1 details=false
flowleap --json patent search --query 'in="<SURNAME GIVEN>" AND pd<<critical-date>' --limit 50
flowleap --json academic search "<author name> <discriminating term>" --to-year <priority-year> --limit 20
flowleap --json npl "<author name> <discriminating term>" --to-year <priority-year> --limit 20
```

- The target's own inventors are the first names to run: their earlier
  publications are a frequent source of self-collision.
- Where an inventor is also an applicant, resolve the entity and read its
  portfolio: `flowleap --json patstat graph resolve "<name>"`, then
  `flowleap --json patstat graph applicant <psn_id>`. A name that does not
  resolve is a log line ("did not resolve"), not an error.
- Academic search has no author field. Pair the name with a Discriminating
  Term and check the author list of each hit.
- Log: per name, each query and its count.

## Track 3: classification co-occurrence

Collect the CPC codes (`cpc[]` in `ops biblio`) of the target and of each X/Y
document. Use the codes that the target shares with at least one X/Y document.

```bash
flowleap --json tools run search_patents query='cpc=<code> AND ta=<discriminating term> AND pd<<critical-date>' range=1-1 details=false
flowleap --json patent search --query 'cpc=<code> AND (ta=<term> OR ta=<synonym>) AND pd<<critical-date>' --limit 30
```

- A classification code is never discriminating. Every query carries at least
  one Discriminating Term from Step 2 (method and count probe:
  `flowleap-patent`, `recipe-prior-art-search` Step 1).
- Probe the count first. Over about 1,000: add the next Discriminating Term.
  Under 10: drop to the CPC main group or OR in synonyms.
- Example (EP2110298B1): `cpc=B62K25/02 AND ta="quick release" AND ta=cam AND
  pd<20080416` gave 35.
- Log: per code, each query and its count.

## Pull the candidates

For each candidate that maps to at least one claim element:

```bash
flowleap --json ops claims <candidate>
flowleap --json ops description <candidate>
```

A `provider_keys_required` answer is a missing patent-data key for that
office. Finish the live office, log the gated office as an open missing-key
gap, and ask for the key at the end (`flowleap-keys`).
