# The three Find Better tracks

All three tracks are mandatory. Log every query with its tool and count in the working record, empty results too, so that a reviewer sees what was searched and what was not. Keep only documents published before the critical date (CQL: `pd<YYYYMMDD`; literature: `filter.to_year`, then check the exact date).

Before you search, mark every document that is already in the Baseline as "of record". A candidate is new only when it is not of record.

## Track 1: backward citations, two hops

Start from every document of the examiner's best art: the X or Y documents in the Baseline, or, when none exists, the examiner-cited documents (Step 1).

- Hop 1: `get_bibliography` (`patent_number`: the X/Y document). Read `citedReferences[]`.
- Hop 2: `get_bibliography` of each hop-1 document that is not of record and is published before the critical date.
- Read `citedReferences[].docId` and `.kind` for patents and `.npl` for non-patent citations. `get_bibliography` with `resolve_npl: true` attaches the OpenAlex record of each `npl` entry as `nplWork` (DOI, authors, abstract); `get_npl_work` (`reference`: the cited string) resolves one. Only a `matched` record is the cited paper; its authors feed Track 2.
- Optional, with FlowLeap Data, for a wider net: `patstat_neighborhood` (`node`: the X/Y document, `depth: 2`, `edge_types: ["cites"]`). A truncation notice means the list is capped; narrow it, never read it as complete.
- Log: for each X/Y document, the hop-1 count, the hop-2 count and the kept count. Mark each query `hop 1` or `hop 2`. If hop 1 found no document outside the Baseline, log one `hop 2` entry that says so, with count 0.

## Track 2: inventor and author networks

Inventors come from `inventors[]` in `get_bibliography` of the target and of each X/Y patent. Authors come from the X/Y non-patent documents.

- Probe: `search_patents` (`query: 'in="<SURNAME GIVEN>" AND pd<<critical-date>'`, `range: "1-1"`, `details: false`), then read the set with `range: "1-50"`.
- Literature: `search_academic` (`query: "<author name> <discriminating term>"`, `filter.to_year`) and `search_npl` (same query, `filter.to_year`).
- Run the target's own inventors first: their earlier publications often collide with the patent.
- When an inventor is also an applicant and FlowLeap Data is available, resolve the entity with `patstat_resolve` (`q`: the name) and read its portfolio with `patstat_applicant` (`psn_id` from the resolve candidate). A name that does not resolve is a log line ("did not resolve"), not an error.
- The literature tools have no author field. Pair the name with a Discriminating Term and check the author list of each hit.
- Log: for each name, each query and its count.

## Track 3: classification co-occurrence

Collect the CPC codes (`cpc[]` in `get_bibliography`) of the target and of each X/Y document, including the 2000-series **indexing codes** (for example `F15B2211/7053`). Use the codes that the target shares with at least one X/Y document.

- Probe: `search_patents` (`query: 'cpc=<code> AND ta=<discriminating term> AND pd<<critical-date>'`, `range: "1-1"`, `details: false`).
- Read: `search_patents` (`query: 'cpc=<code> AND (ta=<term> OR ta=<synonym>) AND pd<<critical-date>'`, `range: "1-100"`).
- A classification code is never discriminating. Every query has at least one Discriminating Term from Step 2 (count probe: section 3 of the search method).
- Over about 1,000: add the next Discriminating Term. Under 10: go to the CPC main group or OR in synonyms.
- Example (EP2743895B1): `cpc=E05G1/026 AND ta=lock AND ta=door AND pd<20121217` gave 63.
- Log: for each code, each query and its count.

### Indexing-code combinations (mandatory when the target has indexing codes)

Indexing codes describe features independent of wording. Run the main group AND each combination of 2-3 indexing codes that the target shares with the examiner's documents, with the date limit and no terms, for example `cpc=F15B7/006 AND cpc=F15B2211/7053 AND cpc=F15B2211/785 AND pd<20230810` (45 hits, probed 2026-10-11; it holds three of the four ISR documents of WO2025031952A1, one as a CA family member). Log each combination with its count and read every title.

### Reading a result set

Results come back newest first, not by relevance, and one page holds at most 100. Read every title of a set you rely on by paging with `range: "101-200"` and so on, or narrow the query until you can. Never judge a set from its first page. The EPO data returns one publication per family: match a hit to a Baseline document by family (`get_family`), not by number.

### Recall check

After Track 3, count how many Baseline documents (X, Y and A, by family) its queries returned, and log "Track 3 retrieved n of m Baseline documents". When the examiners' own art is missing from the classification sets, the queries are too narrow for new art too: add the indexing codes or synonyms of the missing documents and run again before you trust a "no better art found".

### Term-drop pass (mandatory)

One query with two or more terms is over-specified: a reference that says "speed warning" instead of "limit" is not in its hits. After the first classification query, do these steps for each code:

1. Run the query again once for each term, with only that ONE term removed. Keep the code and the date limit.
2. If the first query gave 200 hits or fewer, also run the code with the date limit alone (`cpc=<code> AND pd<<critical-date>`).
3. Log every variant with its count, 0 included.
4. Read the titles of the top hits of every variant, published before the critical date. Read each document whose title matches a claim element. A variant with 101 to 200 hits needs a second page (`range: "101-200"`).

Probe each variant with `range: "1-1"` and `details: false`, then read it with `range: "1-100"`.

Example (US6778074B1, critical date 2002-03-18), counts probed on 2026-10-06. The first query `cpc=G01P1/10 AND ta=speedometer AND ta=limit AND pd<20020318` gave 15 and missed both IPR references. Without `ta=limit`, `cpc=G01P1/10 AND ta=speedometer AND pd<20020318` (54) has Evans US3980041 ("Speedometer with speed warning indicator and method of providing the same"). Without `ta=speedometer`, `cpc=G01P1/10 AND ta=limit AND pd<20020318` gave 56. The claim phrase `cpc=G01P1/10 AND ta="speed limit" AND pd<20020318` (36) has Wendt US2711153 ("Automobile speed limit indicator"). The classification with the date limit alone, `cpc=G01P1/10 AND pd<20020318`, gave 843.

### Title-phrase query (old art)

The EPO data has no abstract for many US documents published before 2000, so `ta=` finds them only by their title words. For each Discriminating Term, run one query on a two-word phrase from the claim: `ti="<two-word phrase>" AND pd<<critical-date>`, with no classification. Example: `ti="speed limit indicator" AND pd<20020318` (10) has Wendt US2711153, which has no abstract in the EPO data. Log each query and its count.

## Read the candidates

For each candidate that maps to at least one claim element, read `get_claims` and `get_description` (`patent_number`: the candidate). EP and WO text comes from these tools, US claims come in English, and US description is not served: for another office or a missing text, read a family member (`get_family`) in an office the tools serve, and label the text as the member's text.

An answer that says an office's key is missing or not configured is a missing-key gap for that office. Finish the other offices, log the gated office as an open missing-key gap, and tell the user at the end (see **Missing keys** in SKILL.md).
