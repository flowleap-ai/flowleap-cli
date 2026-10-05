# Reading the Examiner Baseline JSON

`flowleap --json patent examiner-baseline <publication>` prints one object.
Code computes every value from the offices' own records. Read it; do not
rebuild it from other calls. Fields are only added, never renamed.

## Top level

| Field | Meaning |
|---|---|
| `publication` | The publication you asked for. |
| `offices` | The offices of the family members walked. The requested office is first. These are the keys of each `cells` object. |
| `dedupe` | `"docdb-number"`: rows merge on country + number, kind dropped. Two family members of one cited document stay two rows. |
| `membersWalked[]` | The family members read, with the counts each office reported. |
| `documents[]` | One row per cited document. |
| `gaps[]` | What could not be read. A gap is "no record", never "nothing cited". |

## `membersWalked[]`

- `office`, `representativePublication`, `docdbApplication`.
- `publications[]`: one entry per publication of the member, with `status`
  (`read` or `no_citation_record`), `citedCount`, `examinerCount`,
  `applicantCount`. An EP B1 has no citation block; the EP citations come from
  the A2 and A3.
- `usptoEnriched` (US grants only): `applicationNumber`, `status`, `rows`,
  `total`, `unidentifiedRows`. An unidentified row is an office-action
  citation that names no document. Report the count; do not guess the document.

## `documents[]`

- `document`: DOCDB number without kind, for example `US5135330`.
- `kinds[]`: the kinds seen, for example `["A1"]`.
- `familyId`: always `null` today. Family dedupe is not applied.
- `cells.<office>.text`: the human cell text.
- `cells.<office>.citations[]`: each citation, with:
  - `source`: `ops_biblio` or `uspto_enriched`.
  - `citing`: the publication (OPS) or the US application number (USPTO) that
    carries the citation.
  - `citedBy`: `examiner` or `applicant`, copied from the source.
  - `category`: `X`, `Y`, `A`, or a combination such as `X,A`. Absent when the
    office gave none.
  - `relevantClaims`: for example `1-3,5,7,8`.
  - `relevantPassages[]` (OPS), `phase` (OPS), `officeActionDate` and
    `officeActionType` (USPTO).

## Rules for Find Better

1. **X or Y means examiner-assessed.** Only an examiner assigns a category.
   OPS can mark a categorised citation `citedBy: "applicant"` (example:
   EP1602570 on EP2110298A3, `X,A` claim 13). It is still examiner's art.
2. **Claim numbers belong to the citing claim set.** `relevantClaims` numbers
   the claims of `citing` as that office searched them: the application claims
   for an EP A3 search report, the pending claims for a US office action. Map
   each number to the granted independent claims by comparing claim text (a
   claim concordance). A searched claim with no granted counterpart reaches no
   granted claim; list it under "searched claims not granted".
3. **A combined category does not split its claims.** `X,A` with claims `1-5`
   does not say which claims are X. Treat the document as X for ranking and
   quote the category and claims verbatim in the report.
4. **Gaps stay gaps.** Copy each `gaps[].message` into the report. A CN, JP or
   KR member gives citations and English abstracts only; say that element
   mapping against those documents is not possible.
5. **Stops are not gaps.** A key gate, 401, 402, 429 or 410 stops the verb with
   its usual exit code and hint. Follow `flowleap-keys` or `flowleap-shared`;
   do not report a partial Baseline as complete.

## Ranking the examiner's best art per independent claim

1. Keep each citation whose `category` contains X or Y.
2. Map its `relevantClaims` to granted independent claims (rule 2).
3. Per granted independent claim, sort: X before Y; then more offices citing
   it with X or Y; then more independent claims reached.

## Worked example: EP2110298B1 (critical date 2008-04-16)

Granted independent claims 1, 6 and 10. Concordance: EP A2 claims 1+4 and US
application claims 1, 5 map to claim 1; EP A2 claim 9 and US claim 15 map to
claim 6; EP A2 claim 13 and US claim 22 map to claim 10. EP A2 claim 5 and US
claim 8 (the "rod and stem" system) were not granted.

| Granted claim | Examiner's best art | Evidence in the Baseline |
|---|---|---|
| 1 | US5135330 A | EP `X,A` A2 cl. 1-5; US `X` cl. 1-4,6 (OA 2009-08-18) |
| 1 | US4964287 A | EP `X,A` A2 cl. 1-3 |
| 1 | US2007052286 A1 | US `X` cl. 1 (OA 2009-08-18) |
| 6 | US5135330 A | US `X` cl. 15-21 (OA 2009-08-18); no EP X/Y on A2 cl. 9 |
| 10 | US4763957 A | EP `X,A` A2 cl. 13 |
| 10 | EP1602570 A1 | EP `X,A` A2 cl. 13, `citedBy: applicant` |

Searched claims not granted: US2007052285 A1 (EP `X,A` A2 cl. 5), US5385360 A
(US `X` cl. 10, dependent on cl. 8). Gap: no USPTO enriched-citation record for
US8056987B2 (application 12756531).
