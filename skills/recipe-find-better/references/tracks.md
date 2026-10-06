# The three Find Better tracks

All three tracks are mandatory. Log every query and its count in the working
record, empty results too, so a reviewer sees what was searched and what was
not. Keep only documents published before the critical date (CQL:
`pd<YYYYMMDD`; academic and NPL: `--to-year`, then check the exact date).

Before you search, mark every document already in the Baseline as "of record".
A candidate is new only when it is not of record.

## Track 1: backward citations, two hops

Start from every document of the examiner's best art: the X or Y documents in
the Baseline, or, when none exists, the examiner-cited documents (Step 1).

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
- Log: per X/Y document, hop-1 count, hop-2 count, kept count. Mark each query
  `hop 1` or `hop 2`. If hop 1 found no document outside the Baseline, log one
  `hop 2` entry that says so, with count 0.

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
- Example (EP2743895B1): `cpc=E05G1/026 AND ta=lock AND ta=door AND
  pd<20121217` gave 63.
- Log: per code, each query and its count.

### Term-drop pass (mandatory)

One query with two or more terms is over-specified: a reference that says
"speed warning" instead of "limit" is not in its hits. After the first
classification query, do these steps for each code:

1. Run the query again once for each term, with only that ONE term removed.
   Keep the code and the date limit.
2. If the first query gave 200 hits or fewer, also run the code with the date
   limit alone (`cpc=<code> AND pd<<critical-date>`).
3. Log every variant with its count, 0 included.
4. Read the titles of the top hits of every variant, published before the
   critical date. Pull each title that matches a claim element. `--limit` is
   capped at 100: a variant with 101 to 200 hits needs a second page.

```bash
flowleap patent search --count-only --countries all -q '<variant>'
flowleap --json patent search --countries all --limit 100 -q '<variant>'
```

Example (US6778074B1, critical date 2002-03-18), counts probed live on
2026-10-06. The first query `cpc=G01P1/10 AND ta=speedometer AND ta=limit AND
pd<20020318` gave 15 and missed both IPR references. Without `ta=limit`,
`cpc=G01P1/10 AND ta=speedometer AND pd<20020318` (54) has Evans US3980041
("Speedometer with speed warning indicator and method of providing the
same"). Without `ta=speedometer`, `cpc=G01P1/10 AND ta=limit AND
pd<20020318` gave 56. The claim phrase `cpc=G01P1/10 AND ta="speed limit" AND
pd<20020318` (36) has Wendt US2711153 ("Automobile speed limit indicator").
The classification with the date limit alone, `cpc=G01P1/10 AND
pd<20020318`, gave 843.

### Title-phrase query (old art)

OPS has no abstract for many US documents published before 2000, so `ta=`
finds them only by their title words. For each Discriminating Term, run one
query on a two-word phrase from the claim:
`ti="<two-word phrase>" AND pd<<critical-date>`, with no classification.
Example: `ti="speed limit indicator" AND pd<20020318` (10) has Wendt
US2711153, which has no abstract in OPS. Log each query and its count.

## Pull the candidates

For each candidate that maps to at least one claim element:

```bash
flowleap --json ops claims <candidate>
flowleap --json ops description <candidate>
```

A `provider_keys_required` answer is a missing patent-data key for that
office. Finish the live office, log the gated office as an open missing-key
gap, and ask for the key at the end (`flowleap-keys`).
